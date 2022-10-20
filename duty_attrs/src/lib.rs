use inflector::Inflector;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token, Attribute, Block, Expr, ExprPath, ExprStruct, Field, FieldValue, Fields, FieldsNamed,
    FnArg, GenericArgument, GenericParam, Generics, Ident, ImplItem, ImplItemMethod, ItemEnum,
    ItemImpl, ItemTrait, Member, Pat, PatType, PathArguments, PathSegment, Signature, TraitItem,
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
    methods: Vec<RpcMethod>,
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

    fn methods(&self) -> impl Iterator<Item = &RpcMethod> {
        self.methods.iter()
    }

    fn add_methods(&mut self, request: &Request) {
        let methods = self.methods().map(|method| method.ident());
        let args = self.methods().map(|method| {
            method
                .args()
                .map(|arg| arg.ident.clone())
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
        let service_trait: ItemTrait = input.parse()?;
        let methods = service_trait
            .items
            .iter()
            .filter_map(|item| match item {
                TraitItem::Method(method) => Some(method.try_into()),
                _ => None,
            })
            .collect::<syn::Result<_>>()?;

        Ok(Service {
            service_trait,
            methods,
        })
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
                        &method.ident().to_string().to_class_case(),
                        Span::call_site(),
                    ),
                    fields: Fields::Named(FieldsNamed {
                        brace_token: token::Brace {
                            span: Span::call_site(),
                        },
                        named: method
                            .args()
                            .map(|arg| Field {
                                attrs: arg.arg_type.attrs.clone(),
                                ident: Some(arg.ident.clone()),
                                colon_token: Some(arg.arg_type.colon_token.clone()),
                                ty: *arg.arg_type.ty.to_owned(),
                                vis: Visibility::Inherited,
                            })
                            .collect(),
                    }),
                    discriminant: None,
                })
                .collect(),
        };

        let path = ident_to_path(&req_enum.ident, Some(service.generics()));

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
                    fields: method
                        .args()
                        .map(|arg| FieldValue {
                            attrs: Vec::new(),
                            member: Member::Named(arg.ident.clone()),
                            colon_token: None,
                            expr: syn::Expr::Path(ExprPath {
                                attrs: Vec::new(),
                                qself: None,
                                path: ident_to_path(&arg.ident, None),
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
                ident_to_path(&service.ident(), Some(&service.generics())),
                token::For {
                    span: Span::call_site(),
                },
            )),
            self_ty: Box::new(syn::Type::Path(TypePath {
                qself: None,
                path: ident_to_path(&ident, None),
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

struct RpcMethod {
    attrs: Vec<Attribute>,
    sig: Signature,
    args: Vec<RpcArg>,
}

impl RpcMethod {
    fn ident(&self) -> &Ident {
        &self.sig.ident
    }

    fn args(&self) -> impl Iterator<Item = &RpcArg> {
        self.args.iter()
    }
}

impl TryFrom<&TraitItemMethod> for RpcMethod {
    type Error = syn::Error;

    fn try_from(method: &TraitItemMethod) -> Result<Self, Self::Error> {
        let args = method
            .sig
            .inputs
            .iter()
            .filter_map(|arg| match arg {
                FnArg::Typed(pat_type) => Some(pat_type.try_into()),
                _ => None,
            })
            .collect::<syn::Result<_>>()?;

        Ok(RpcMethod {
            attrs: method.attrs.clone(),
            sig: method.sig.clone(),
            args,
        })
    }
}

struct RpcArg {
    ident: Ident,
    arg_type: PatType,
}

impl TryFrom<&PatType> for RpcArg {
    type Error = syn::Error;

    fn try_from(arg_type: &PatType) -> Result<Self, Self::Error> {
        use syn::spanned::Spanned;
        match arg_type.pat.as_ref() {
            Pat::Ident(pat_ident) => Ok(RpcArg {
                ident: pat_ident.ident.to_owned(),
                arg_type: arg_type.to_owned(),
            }),
            _ => Err(syn::Error::new(
                arg_type.span(),
                format!("only basic patterns are supported in service trait methods"),
            )),
        }
    }
}

fn enum_variant_to_path(enum_ident: &Ident, variant_ident: &Ident) -> syn::Path {
    let mut segments = Punctuated::new();

    segments.push(PathSegment {
        ident: enum_ident.clone(),
        arguments: PathArguments::None,
    });

    segments.push(PathSegment {
        ident: variant_ident.clone(),
        arguments: PathArguments::None,
    });

    syn::Path {
        leading_colon: None,
        segments,
    }
}

fn ident_to_path(ident: &Ident, generics: Option<&Generics>) -> syn::Path {
    let mut segments = Punctuated::new();

    let arguments = generics
        .and_then(|g| Some((g.lt_token?, g.gt_token?, &g.params)))
        .map(|(lt_token, gt_token, params)| {
            let gen_args: Punctuated<GenericArgument, _> = params
                .iter()
                .map(|param| match param {
                    GenericParam::Const(param) => GenericArgument::Const(Expr::Path(ExprPath {
                        attrs: Vec::new(),
                        qself: None,
                        path: ident_to_path(&param.ident, None),
                    })),
                    GenericParam::Lifetime(lifetime_def) => {
                        GenericArgument::Lifetime(lifetime_def.lifetime.clone())
                    }
                    GenericParam::Type(param) => GenericArgument::Type(Type::Path(TypePath {
                        qself: None,
                        path: ident_to_path(&param.ident, None),
                    })),
                })
                .collect();

            PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                colon2_token: None,
                lt_token: lt_token.to_owned(),
                gt_token: gt_token.to_owned(),
                args: gen_args,
            })
        })
        .unwrap_or(PathArguments::None);

    let segment = PathSegment {
        ident: ident.clone(),
        arguments,
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
