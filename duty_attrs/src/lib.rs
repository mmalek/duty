use inflector::Inflector;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    parse, parse_macro_input, punctuated::Punctuated, spanned::Spanned, token, Block, Expr,
    ExprPath, ExprStruct, Field, FieldValue, Fields, FieldsNamed, FnArg, GenericArgument,
    GenericParam, Generics, Ident, ImplItem, ImplItemMethod, ItemEnum, ItemImpl, ItemTrait, Member,
    Pat, PatIdent, PatType, PathArguments, PathSegment, TraitItem, TraitItemMethod, Type, TypePath,
    Variant, Visibility,
};

#[proc_macro_attribute]
pub fn service(_args: TokenStream, item: TokenStream) -> TokenStream {
    let service_trait = parse_macro_input!(item as ItemTrait);
    let service_trait_ident = service_trait.ident.clone();
    let client_struct_ident = Ident::new(
        &format!("{}Client", service_trait_ident),
        service_trait_ident.span(),
    );
    let req_enum_ident = Ident::new(
        &format!("{}Request", service_trait_ident),
        service_trait_ident.span(),
    );

    let req_enum = ItemEnum {
        attrs: Vec::new(),
        vis: service_trait.vis.clone(),
        enum_token: token::Enum {
            span: service_trait_ident.span(),
        },
        ident: req_enum_ident.clone(),
        generics: strip_where_clause(&service_trait.generics),
        brace_token: token::Brace {
            span: service_trait_ident.span(),
        },
        variants: trait_methods(&service_trait)
            .map(|method| Variant {
                attrs: Vec::new(),
                ident: Ident::new(
                    &method.sig.ident.to_string().to_class_case(),
                    service_trait_ident.span(),
                ),
                fields: Fields::Named(FieldsNamed {
                    brace_token: token::Brace {
                        span: service_trait_ident.span(),
                    },
                    named: method_input_args(method)
                        .map(|(pat_type, pat_ident)| Field {
                            attrs: pat_type.attrs.clone(),
                            ident: Some(pat_ident.ident.clone()),
                            colon_token: Some(pat_type.colon_token.clone()),
                            ty: *pat_type.ty.to_owned(),
                            vis: Visibility::Inherited,
                        })
                        .collect(),
                }),
                discriminant: None,
            })
            .collect(),
    };

    let req_enum_impl = ItemImpl {
        attrs: Vec::new(),
        defaultness: None,
        unsafety: None,
        impl_token: token::Impl {
            span: Span::call_site(),
        },
        generics: service_trait.generics.clone(),
        trait_: None,
        self_ty: Box::new(Type::Path(TypePath {
            qself: None,
            path: ident_to_path_with_generics(&req_enum_ident, &service_trait.generics),
        })),
        brace_token: token::Brace {
            span: Span::call_site(),
        },
        items: Vec::new(),
    };

    let methods: Vec<ImplItemMethod> = trait_methods(&service_trait)
        .zip(req_enum.variants.iter())
        .map(|(method, variant)| -> ImplItemMethod {
            let req_inst = ExprStruct {
                attrs: Vec::new(),
                path: enum_variant_to_path(&req_enum_ident, &variant.ident),
                brace_token: token::Brace {
                    span: service_trait_ident.span(),
                },
                fields: method_input_args(method)
                    .map(|(_, arg)| FieldValue {
                        attrs: Vec::new(),
                        member: Member::Named(arg.ident.clone()),
                        colon_token: None,
                        expr: syn::Expr::Path(ExprPath {
                            attrs: Vec::new(),
                            qself: None,
                            path: ident_to_path(&arg.ident),
                        }),
                    })
                    .collect(),
                dot2_token: None,
                rest: None,
            };
            let body = quote! {
                {
                    self.stream
                        .borrow_mut()
                        .send_receive(&#req_inst)
                        .expect("Communication error")
                }
            };

            let block: Block = parse(body.into()).expect(&format!(
                "Cannot parse {service_trait_ident}::{client_struct_ident} body"
            ));

            ImplItemMethod {
                attrs: method.attrs.clone(),
                sig: method.sig.clone(),
                vis: service_trait.vis.clone(),
                defaultness: None,
                block,
            }
        })
        .collect();

    let service_trait = add_methods_to_service_trait(service_trait, &req_enum);

    let client_service_impl = ItemImpl {
        attrs: Vec::new(),
        defaultness: None,
        unsafety: None,
        impl_token: token::Impl {
            span: service_trait.span(),
        },
        generics: service_trait.generics.clone(),
        trait_: Some((
            None,
            ident_to_path_with_generics(&service_trait_ident, &service_trait.generics),
            token::For {
                span: service_trait.span(),
            },
        )),
        self_ty: Box::new(syn::Type::Path(TypePath {
            qself: None,
            path: ident_to_path(&client_struct_ident),
        })),
        brace_token: token::Brace {
            span: service_trait.span(),
        },
        items: methods.into_iter().map(ImplItem::Method).collect(),
    };

    let result = quote!(
        #service_trait

        pub struct #client_struct_ident {
            stream: std::cell::RefCell<duty::DataStream<std::net::TcpStream>>,
        }

        impl #client_struct_ident {
            pub fn new(stream: TcpStream) -> std::result::Result<Self, duty::Error> {
                let stream = duty::DataStream::new(stream);
                let stream = std::cell::RefCell::new(stream);
                Ok(Self { stream })
            }
        }

        #[derive(serde::Serialize, serde::Deserialize)]
        #req_enum

        #req_enum_impl

        #client_service_impl
    );

    // eprintln!("OUTPUT: {}", result);

    result.into()
}

fn add_methods_to_service_trait(mut service_trait: ItemTrait, req_enum: &ItemEnum) -> ItemTrait {
    let methods: Vec<&Ident> = trait_methods(&service_trait)
        .map(|method| &method.sig.ident)
        .collect();
    let args: Vec<Vec<&PatIdent>> = trait_methods(&service_trait)
        .map(|method| method_input_args(method).map(|(_, arg)| arg).collect())
        .collect();

    let req_enum_name = &req_enum.ident;
    let req_enum_path = ident_to_path_with_generics(&req_enum.ident, &req_enum.generics);
    let req_enum_variants: Vec<&Ident> = req_enum
        .variants
        .iter()
        .map(|variant| &variant.ident)
        .collect();

    let handle_next_request_method = quote! {
        fn handle_next_request(&self, stream: &mut duty::DataStream<std::net::TcpStream>) -> Result<(), duty::Error> {
            let request: #req_enum_path = stream.receive()?;
            match request {
                #(
                    #req_enum_name::#req_enum_variants { #( #args ),* } => stream.send(&self.#methods(#( #args ),*)),
                )*
            }
        }
    };

    let handle_next_request_method = parse(handle_next_request_method.into()).expect(&format!(
        "Cannot parse handle_next_request method definition"
    ));

    service_trait
        .items
        .push(TraitItem::Method(handle_next_request_method));

    service_trait
}

fn trait_methods(item_trait: &ItemTrait) -> impl Iterator<Item = &TraitItemMethod> {
    item_trait.items.iter().filter_map(|item| match item {
        TraitItem::Method(method) => Some(method),
        _ => None,
    })
}

fn method_input_args(method: &TraitItemMethod) -> impl Iterator<Item = (&PatType, &PatIdent)> {
    method.sig.inputs.iter().filter_map(|arg| match arg {
        FnArg::Typed(pat_type) => {
            if let Pat::Ident(pat_ident) = pat_type.pat.as_ref() {
                Some((pat_type, pat_ident))
            } else {
                None
            }
        }
        _ => None,
    })
}

fn enum_variant_to_path(enum_ident: &Ident, variant_ident: &Ident) -> syn::Path {
    syn::parse_str::<syn::Path>(&format!("{enum_ident}::{variant_ident}")).expect(&format!(
        "Cannot create path out of '{enum_ident}::{variant_ident}' enum"
    ))
}

fn ident_to_path(ident: &Ident) -> syn::Path {
    syn::parse_str::<syn::Path>(&ident.to_string())
        .expect(&format!("Cannot create path out of '{ident}' identifier"))
}

fn ident_to_path_with_generics(ident: &Ident, generics: &Generics) -> syn::Path {
    let mut segments = Punctuated::new();

    let gen_args: Punctuated<GenericArgument, _> = generics
        .params
        .iter()
        .map(|param| match param {
            GenericParam::Const(param) => GenericArgument::Const(Expr::Path(ExprPath {
                attrs: Vec::new(),
                qself: None,
                path: ident_to_path(&param.ident),
            })),
            GenericParam::Lifetime(lifetime_def) => {
                GenericArgument::Lifetime(lifetime_def.lifetime.clone())
            }
            GenericParam::Type(param) => GenericArgument::Type(Type::Path(TypePath {
                qself: None,
                path: ident_to_path(&param.ident),
            })),
        })
        .collect();

    let segment = PathSegment {
        ident: ident.clone(),
        arguments: if let Some((lt_token, gt_token)) = generics.lt_token.zip(generics.gt_token) {
            PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                colon2_token: None,
                lt_token: lt_token.to_owned(),
                gt_token: gt_token.to_owned(),
                args: gen_args,
            })
        } else {
            PathArguments::None
        },
    };

    segments.push(segment);

    syn::Path {
        leading_colon: None,
        segments,
    }
}

fn strip_where_clause(generics: &Generics) -> Generics {
    let mut generics = generics.clone();
    generics.where_clause = None;
    generics
}
