use inflector::Inflector;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token, Block, Expr, ExprPath, ExprStruct, Field, FieldValue, Fields, FieldsNamed, FnArg,
    GenericArgument, GenericParam, Generics, Ident, ImplItem, ImplItemMethod, ItemEnum, ItemImpl,
    ItemTrait, Member, Pat, PatIdent, PatType, PathArguments, PathSegment, TraitItem,
    TraitItemMethod, Type, TypePath, Variant, Visibility,
};

#[proc_macro_attribute]
pub fn service(_args: TokenStream, item: TokenStream) -> TokenStream {
    let mut service = parse_macro_input!(item as Service);
    let request = Request::new(&service);
    let client = Client::new(&service, &request);

    service.add_methods(&request);

    let result = quote!(
        #service

        #request

        #client
    );

    // eprintln!("OUTPUT: {}", result);

    result.into()
}

struct Service {
    service_trait: ItemTrait,
}

impl Service {
    fn vis(&self) -> &Visibility {
        &self.service_trait.vis
    }

    fn ident(&self) -> &Ident {
        &self.service_trait.ident
    }

    fn generics(&self) -> &Generics {
        &self.service_trait.generics
    }

    fn methods(&self) -> impl Iterator<Item = &TraitItemMethod> {
        self.service_trait
            .items
            .iter()
            .filter_map(|item| match item {
                TraitItem::Method(method) => Some(method),
                _ => None,
            })
    }

    fn add_methods(&mut self, request: &Request) {
        let methods = self.methods().map(|method| &method.sig.ident);
        let args = self.methods().map(|method| {
            method_input_args(method)
                .map(|(_, arg)| arg)
                .collect::<Vec<_>>()
        });

        let req_enum_path = request.path();
        let req_enum_variants = request.variant_paths();

        let handle_next_request_method = quote! {
            fn handle_next_request(&self, stream: &mut duty::DataStream<std::net::TcpStream>) -> Result<(), duty::Error> {
                let request: #req_enum_path = stream.receive()?;
                match request {
                    #(
                        #req_enum_variants { #( #args ),* } => stream.send(&self.#methods(#( #args ),*)),
                    )*
                }
            }
        };

        let handle_next_request_method = parse(handle_next_request_method.into()).expect(&format!(
            "Cannot parse handle_next_request method definition"
        ));

        self.service_trait
            .items
            .push(TraitItem::Method(handle_next_request_method));
    }
}

impl Parse for Service {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let service_trait = input.parse()?;
        Ok(Service { service_trait })
    }
}

impl ToTokens for Service {
    fn to_tokens(&self, output: &mut TokenStream2) {
        self.service_trait.to_tokens(output);
    }
}

struct Request {
    req_enum: ItemEnum,
    req_impl: ItemImpl,
    path: syn::Path,
}

impl Request {
    fn new(service: &Service) -> Request {
        let req_enum = ItemEnum {
            attrs: Vec::new(),
            vis: service.vis().clone(),
            enum_token: token::Enum {
                span: Span::call_site(),
            },
            ident: format_ident!("{}Request", service.ident()),
            generics: strip_where_clause(service.generics()),
            brace_token: token::Brace {
                span: Span::call_site(),
            },
            variants: service
                .methods()
                .map(|method| Variant {
                    attrs: Vec::new(),
                    ident: Ident::new(
                        &method.sig.ident.to_string().to_class_case(),
                        Span::call_site(),
                    ),
                    fields: Fields::Named(FieldsNamed {
                        brace_token: token::Brace {
                            span: Span::call_site(),
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

        let path = ident_to_path_with_generics(&req_enum.ident, service.generics());

        let req_impl = ItemImpl {
            attrs: Vec::new(),
            defaultness: None,
            unsafety: None,
            impl_token: token::Impl {
                span: Span::call_site(),
            },
            generics: service.generics().clone(),
            trait_: None,
            self_ty: Box::new(Type::Path(TypePath {
                qself: None,
                path: path.clone(),
            })),
            brace_token: token::Brace {
                span: Span::call_site(),
            },
            items: Vec::new(),
        };

        Request {
            req_enum,
            req_impl,
            path,
        }
    }

    fn path(&self) -> &syn::Path {
        &self.path
    }

    fn variant_paths<'a>(&'a self) -> impl Iterator<Item = syn::Path> + 'a {
        self.req_enum
            .variants
            .iter()
            .map(|variant| enum_variant_to_path(&self.req_enum.ident, &variant.ident))
    }
}

impl ToTokens for Request {
    fn to_tokens(&self, output: &mut TokenStream2) {
        let req_enum = &self.req_enum;
        let req_impl = &self.req_impl;

        output.extend(quote!(
            #[derive(serde::Serialize, serde::Deserialize)]
            #req_enum

            #req_impl
        ));
    }
}

struct Client {
    ident: Ident,
    item_impl: ItemImpl,
}

impl Client {
    fn new(service: &Service, request: &Request) -> Client {
        let ident = format_ident!("{}Client", service.ident());

        let methods: Vec<ImplItemMethod> = service
            .methods()
            .zip(request.variant_paths())
            .map(|(method, variant_path)| -> ImplItemMethod {
                let req_inst = ExprStruct {
                    attrs: Vec::new(),
                    path: variant_path,
                    brace_token: token::Brace {
                        span: Span::call_site(),
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

                let block: Block = parse(body.into()).expect(&format!("Cannot parse {ident} body"));

                ImplItemMethod {
                    attrs: method.attrs.clone(),
                    sig: method.sig.clone(),
                    vis: service.vis().clone(),
                    defaultness: None,
                    block,
                }
            })
            .collect();

        let item_impl = ItemImpl {
            attrs: Vec::new(),
            defaultness: None,
            unsafety: None,
            impl_token: token::Impl {
                span: Span::call_site(),
            },
            generics: service.generics().clone(),
            trait_: Some((
                None,
                ident_to_path_with_generics(&service.ident(), &service.generics()),
                token::For {
                    span: Span::call_site(),
                },
            )),
            self_ty: Box::new(syn::Type::Path(TypePath {
                qself: None,
                path: ident_to_path(&ident),
            })),
            brace_token: token::Brace {
                span: Span::call_site(),
            },
            items: methods.into_iter().map(ImplItem::Method).collect(),
        };

        Client { ident, item_impl }
    }
}

impl ToTokens for Client {
    fn to_tokens(&self, output: &mut TokenStream2) {
        let ident = &self.ident;
        let item_impl = &self.item_impl;

        output.extend(quote!(
            pub struct #ident {
                stream: std::cell::RefCell<duty::DataStream<std::net::TcpStream>>,
            }

            impl #ident {
                pub fn new(stream: TcpStream) -> std::result::Result<Self, duty::Error> {
                    let stream = duty::DataStream::new(stream);
                    let stream = std::cell::RefCell::new(stream);
                    Ok(Self { stream })
                }
            }

            #item_impl
        ));
    }
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
