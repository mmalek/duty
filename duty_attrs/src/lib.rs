use inflector::Inflector;
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse, parse_macro_input, token, Block, ExprPath, ExprStruct, Field, FieldValue, Fields,
    FieldsNamed, FnArg, Ident, ImplItemMethod, ItemEnum, ItemTrait, Member, Pat, PatIdent, PatType,
    TraitItem, TraitItemMethod, Variant, Visibility,
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
        generics: service_trait.generics.clone(),
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

    let result = quote!(
        #service_trait

        pub struct #client_struct_ident {
            stream: std::cell::RefCell<duty::DataStream>,
        }

        impl #client_struct_ident {
            pub fn new<A>(addr: A) -> std::result::Result<Self, duty::Error>
            where A: std::net::ToSocketAddrs {
                let stream = duty::DataStream::connect(addr)?;
                let stream = std::cell::RefCell::new(stream);
                Ok(Self { stream })
            }
        }

        #[derive(serde::Serialize, serde::Deserialize)]
        #req_enum

        impl #service_trait_ident for #client_struct_ident {
            #(
                #methods
            )*
        }
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
    let req_enum_variants: Vec<&Ident> = req_enum
        .variants
        .iter()
        .map(|variant| &variant.ident)
        .collect();

    let handle_next_request_method = quote! {
        fn handle_next_request(&self, stream: &mut duty::DataStream) -> Result<(), Error> {
            let request: #req_enum_name = stream.receive()?;
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
