use inflector::Inflector;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, parse_quote, punctuated::Punctuated, token, Attribute, Expr, ExprPath,
    ExprReference, ExprStruct, Field, FieldValue, Fields, FieldsNamed, FnArg, GenericArgument,
    GenericParam, Generics, Ident, ImplItem, ImplItemMethod, ItemEnum, ItemImpl, ItemStruct,
    ItemTrait, Member, Pat, PatType, PathArguments, PathSegment, Receiver, ReturnType, Signature,
    Token, TraitItem, TraitItemMethod, Type, TypePath, Variant, Visibility,
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
                .rpc_args()
                .map(|arg| arg.ident.clone())
                .collect::<Vec<_>>()
        });

        let method_args = self.methods().map(|method| method.method_args());

        let req_enum_path = request.path();
        let req_enum_variants = request.variant_paths();

        let handle_next_request_method = parse_quote! {
            fn handle_next_request(&self, stream: &mut duty::DataStream<std::net::TcpStream>) -> Result<(), duty::Error> {
                let request: #req_enum_path = stream.receive()?;
                match request {
                    #(
                        #req_enum_variants { #( #args ),* } => stream.send(&Self::#methods(#method_args)),
                    )*
                }
            }
        };

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
                            .rpc_args()
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
            .map(|variant| enum_variant_to_path(&self.req_enum, &variant.ident))
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
    methods: Vec<ImplItemMethod>,
    vis: Visibility,
    generics: Generics,
}

impl Client {
    fn new(service: &Service, request: &Request) -> Client {
        let ident = format_ident!("{}Client", service.ident());
        let vis = service.vis().clone();

        let methods = service
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
                        .rpc_args()
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

                let mut inputs = Punctuated::new();
                inputs.push(FnArg::Receiver(Receiver {
                    attrs: Vec::new(),
                    reference: Some((
                        token::And {
                            spans: [Span::call_site()],
                        },
                        None,
                    )),
                    mutability: None,
                    self_token: token::SelfValue {
                        span: Span::call_site(),
                    },
                }));

                for input in method.sig.inputs.iter() {
                    if matches!(input, FnArg::Typed(_)) {
                        inputs.push(input.clone());
                    }
                }

                let ret_type = match &method.sig.output {
                    ReturnType::Default => Box::new(parse_quote!(())),
                    ReturnType::Type(_, t) => t.clone(),
                };

                let sig = Signature {
                    constness: None,
                    asyncness: None,
                    unsafety: None,
                    abi: None,
                    fn_token: method.sig.fn_token.clone(),
                    ident: method.sig.ident.clone(),
                    generics: Default::default(),
                    paren_token: method.sig.paren_token.clone(),
                    inputs,
                    variadic: None,
                    output: parse_quote!(-> Result<#ret_type, duty::Error>),
                };

                ImplItemMethod {
                    attrs: method.attrs.clone(),
                    sig,
                    vis: vis.clone(),
                    defaultness: None,
                    block: parse_quote! {
                        {
                            self.stream
                                .borrow_mut()
                                .send_receive(&#req_inst)
                        }
                    },
                }
            })
            .collect();

        let generics = service.generics().clone();

        Client {
            ident,
            methods,
            vis,
            generics,
        }
    }
}

impl ToTokens for Client {
    fn to_tokens(&self, output: &mut TokenStream2) {
        let vis = &self.vis;

        let gen_args = generics_params_to_args(&self.generics.params);

        let fields = parse_quote!(
            {
                stream: std::cell::RefCell<duty::DataStream<std::net::TcpStream>>,
                phantom: std::marker::PhantomData<(#gen_args)>,
            }
        );

        let item_struct = ItemStruct {
            attrs: Vec::new(),
            vis: self.vis.clone(),
            struct_token: token::Struct {
                span: Span::call_site(),
            },
            ident: self.ident.clone(),
            generics: self.generics.clone(),
            fields: Fields::Named(fields),
            semi_token: None,
        };

        let mut methods = Vec::new();
        methods.push(parse_quote!(
            #vis fn new(stream: std::net::TcpStream) -> std::result::Result<Self, duty::Error> {
                let stream = duty::DataStream::new(stream);
                let stream = std::cell::RefCell::new(stream);
                Ok(Self { stream, phantom: std::marker::PhantomData {} })
            }
        ));

        methods.extend(self.methods.clone().into_iter());

        let item_impl = ItemImpl {
            attrs: Vec::new(),
            defaultness: None,
            unsafety: None,
            impl_token: token::Impl {
                span: Span::call_site(),
            },
            generics: self.generics.clone(),
            trait_: None,
            self_ty: Box::new(Type::Path(TypePath {
                qself: None,
                path: ident_to_path(&self.ident, Some(&self.generics)),
            })),
            brace_token: token::Brace {
                span: Span::call_site(),
            },
            items: methods.into_iter().map(ImplItem::Method).collect(),
        };

        output.extend(quote!(
            #item_struct

            #item_impl
        ));
    }
}

struct RpcMethod {
    attrs: Vec<Attribute>,
    sig: Signature,
    rpc_args: Vec<RpcArg>,
    method_args: Punctuated<Expr, token::Comma>,
}

impl RpcMethod {
    fn ident(&self) -> &Ident {
        &self.sig.ident
    }

    fn rpc_args(&self) -> impl Iterator<Item = &RpcArg> {
        self.rpc_args.iter()
    }

    fn method_args(&self) -> &Punctuated<Expr, token::Comma> {
        &self.method_args
    }
}

impl TryFrom<&TraitItemMethod> for RpcMethod {
    type Error = syn::Error;

    fn try_from(method: &TraitItemMethod) -> Result<Self, Self::Error> {
        if let Some(lt_token) = &method.sig.generics.lt_token {
            let span = method
                .sig
                .generics
                .gt_token
                .as_ref()
                .and_then(|gt| lt_token.span.join(gt.span))
                .unwrap_or_else(|| lt_token.span);

            return Err(syn::Error::new(
                span,
                "generic methods are not supported in service trait",
            ));
        }

        let rpc_args = method
            .sig
            .inputs
            .iter()
            .filter_map(|arg| match arg {
                FnArg::Typed(pat_type) => Some(pat_type.try_into()),
                _ => None,
            })
            .collect::<syn::Result<_>>()?;

        let method_args = method
            .sig
            .inputs
            .iter()
            .map(|arg| match arg {
                FnArg::Receiver(receiver) => {
                    if receiver.reference.is_some() {
                        Ok(Expr::Reference(ExprReference {
                            attrs: Vec::new(),
                            and_token: token::And {
                                spans: [Span::call_site()],
                            },
                            raw: Default::default(),
                            mutability: receiver.mutability.clone(),
                            expr: Box::new(Expr::Path(ExprPath {
                                attrs: Vec::new(),
                                qself: None,
                                path: ident_to_path(&format_ident!("self"), None),
                            })),
                        }))
                    } else {
                        Ok(Expr::Path(ExprPath {
                            attrs: Vec::new(),
                            qself: None,
                            path: ident_to_path(&format_ident!("self"), None),
                        }))
                    }
                }
                FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
                    Pat::Ident(pat_ident) => Ok(Expr::Path(ExprPath {
                        attrs: Vec::new(),
                        qself: None,
                        path: ident_to_path(&pat_ident.ident, None),
                    })),
                    _ => Err(syn::Error::new(
                        pat_type.span(),
                        format!("only basic patterns are supported in service trait methods"),
                    )),
                },
            })
            .collect::<syn::Result<_>>()?;

        Ok(RpcMethod {
            attrs: method.attrs.clone(),
            sig: method.sig.clone(),
            rpc_args,
            method_args,
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

fn enum_variant_to_path(item_enum: &ItemEnum, variant_ident: &Ident) -> syn::Path {
    let mut segments = Punctuated::new();

    segments.push(PathSegment {
        ident: item_enum.ident.clone(),
        arguments: generics_to_path_args(Some(&item_enum.generics), true),
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

    let segment = PathSegment {
        ident: ident.clone(),
        arguments: generics_to_path_args(generics, false),
    };

    segments.push(segment);

    syn::Path {
        leading_colon: None,
        segments,
    }
}

fn generics_params_to_args(
    params: &Punctuated<GenericParam, Token![,]>,
) -> Punctuated<GenericArgument, Token![,]> {
    params
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
        .collect()
}

fn generics_to_path_args(generics: Option<&Generics>, leading_colon: bool) -> PathArguments {
    generics
        .and_then(|g| Some((g.lt_token?, g.gt_token?, &g.params)))
        .map(|(lt_token, gt_token, params)| {
            let gen_args = generics_params_to_args(params);

            let colon2_token = if leading_colon {
                Some(token::Colon2 {
                    spans: [Span::call_site(), Span::call_site()],
                })
            } else {
                None
            };

            PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                colon2_token,
                lt_token: lt_token.to_owned(),
                gt_token: gt_token.to_owned(),
                args: gen_args,
            })
        })
        .unwrap_or(PathArguments::None)
}

fn strip_where_clause(generics: &Generics) -> Generics {
    let mut generics = generics.clone();
    generics.where_clause = None;
    generics
}
