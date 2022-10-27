use inflector::Inflector;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, ToTokens};
use std::iter;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::WhereClause;
use syn::{
    parse_macro_input, parse_quote, punctuated::Punctuated, token, Expr, ExprPath, FnArg,
    GenericArgument, GenericParam, Generics, Ident, ItemTrait, Pat, PatType, PathArguments,
    PathSegment, Receiver, ReturnType, Signature, Token, TraitItem, TraitItemMethod, Type,
    TypePath, Visibility,
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

        let method_call_args = self.methods().map(RpcMethod::method_call_args);

        let req_enum_path = request.path();
        let req_enum_variants = request.variant_paths();

        let receiver: Receiver = if self.methods().any(RpcMethod::has_ref_mut_self) {
            parse_quote!(&mut self)
        } else {
            parse_quote!(&self)
        };

        let handle_next_request_method = parse_quote! {
            /// Waits for the next request and calls appropriate trait method
            fn handle_next_request<ReadWriteStream>(#receiver, stream: &mut duty::DataStream<ReadWriteStream>) -> Result<(), duty::Error>
            where
                ReadWriteStream: std::io::Read + std::io::Write,
            {
                let request: #req_enum_path = stream.receive()?;
                match request {
                    #(
                        #req_enum_variants { #( #args ),* } => stream.send(&Self::#methods(#method_call_args)),
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
    path: syn::Path,
    vis: Visibility,
    ident: Ident,
    generics: Generics,
    variants: Vec<RequestVariant>,
}

impl Request {
    fn new(service: &Service) -> Request {
        let ident = format_ident!("{}Request", service.ident());

        let path = ident_to_path(&ident, Some(service.generics()));

        let variants = service.methods().map(RequestVariant::new).collect();

        Request {
            path,
            vis: service.vis().clone(),
            ident,
            generics: service.generics().clone(),
            variants,
        }
    }

    fn path(&self) -> &syn::Path {
        &self.path
    }

    fn variant_paths<'a>(&'a self) -> impl Iterator<Item = syn::Path> + 'a {
        self.variants
            .iter()
            .map(|variant| enum_variant_to_path(&self.ident, &self.generics, &variant.ident))
    }
}

impl ToTokens for Request {
    fn to_tokens(&self, output: &mut TokenStream2) {
        let vis = &self.vis;
        let ident = &self.ident;
        let variants = &self.variants;

        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();

        output.extend(quote!(
            #[derive(serde::Serialize, serde::Deserialize)]
            #vis enum #ident #ty_generics {
                #(
                    #variants,
                )*
            }

            impl #impl_generics #ident #ty_generics #where_clause {
            }
        ));
    }
}

struct RequestVariant {
    ident: Ident,
    fields: Vec<RpcArg>,
}

impl RequestVariant {
    fn new(method: &RpcMethod) -> RequestVariant {
        RequestVariant {
            ident: format_ident!("{}", method.ident().to_string().to_class_case()),
            fields: method.rpc_args().cloned().collect(),
        }
    }
}

impl ToTokens for RequestVariant {
    fn to_tokens(&self, output: &mut TokenStream2) {
        let ident = &self.ident;
        let field_idents = self.fields.iter().map(|field| &field.ident);
        let field_types = self.fields.iter().map(|field| &field.arg_type);

        output.extend(quote!(
            #ident { #( #field_idents: #field_types, )* }
        ))
    }
}

struct Client {
    ident: Ident,
    methods: Vec<ClientMethod>,
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
            .map(|(method, variant_path)| {
                let req_fields = method.rpc_args().map(|arg| arg.ident.clone()).collect();

                ClientMethod {
                    vis: vis.clone(),
                    sig: method.sig.clone(),
                    req_variant: variant_path,
                    req_fields,
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
        let ident = &self.ident;
        let vis = &self.vis;
        let methods = &self.methods;

        let mut generics = Generics {
            lt_token: Some(Default::default()),
            params: iter::once::<GenericParam>(parse_quote!(ReadWriteStream)).collect(),
            gt_token: Some(Default::default()),
            where_clause: Some(WhereClause {
                where_token: Default::default(),
                predicates: parse_quote!(ReadWriteStream: std::io::Read + std::io::Write,),
            }),
        };

        generics.params.extend(self.generics.params.iter().cloned());

        if let Some((where_clause, service_where)) = generics
            .where_clause
            .as_mut()
            .zip(self.generics.where_clause.as_ref())
        {
            where_clause
                .predicates
                .extend(service_where.predicates.iter().cloned());
        }

        let gen_args = generics_params_to_args(&self.generics.params);

        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        output.extend(quote!(
            #vis struct #ident #ty_generics {
                stream: std::cell::RefCell<duty::DataStream<ReadWriteStream>>,
                phantom: std::marker::PhantomData<(#gen_args)>,
            }

            impl #impl_generics #ident #ty_generics #where_clause {
                #vis fn new(stream: ReadWriteStream) -> std::result::Result<Self, duty::Error> {
                    let stream = duty::DataStream::new(stream);
                    let stream = std::cell::RefCell::new(stream);
                    Ok(Self { stream, phantom: std::marker::PhantomData {} })
                }

                #(
                    #methods
                )*
            }
        ));
    }
}

struct ClientMethod {
    vis: Visibility,
    sig: Signature,
    req_variant: syn::Path,
    req_fields: Vec<Ident>,
}

impl ToTokens for ClientMethod {
    fn to_tokens(&self, output: &mut TokenStream2) {
        let vis = &self.vis;
        let ident = &self.sig.ident;
        let args = self
            .sig
            .inputs
            .iter()
            .filter(|arg| matches!(arg, FnArg::Typed(_)));
        let req_variant = &self.req_variant;
        let req_fields = &self.req_fields;

        let unit_type = Box::new(parse_quote!(()));

        let ret_type = match &self.sig.output {
            ReturnType::Default => &unit_type,
            ReturnType::Type(_, t) => &t,
        };

        output.extend(quote!(
            #vis fn #ident (&self #(, #args)* ) -> Result<#ret_type, duty::Error> {
                self.stream
                    .borrow_mut()
                    .send_receive(& #req_variant {#( #req_fields, )*})
            }
        ));
    }
}

struct RpcMethod {
    sig: Signature,
    rpc_args: Vec<RpcArg>,
    method_call_args: Punctuated<Expr, token::Comma>,
}

impl RpcMethod {
    fn ident(&self) -> &Ident {
        &self.sig.ident
    }

    fn rpc_args(&self) -> impl Iterator<Item = &RpcArg> {
        self.rpc_args.iter()
    }

    fn method_call_args(&self) -> &Punctuated<Expr, token::Comma> {
        &self.method_call_args
    }

    fn has_ref_mut_self(&self) -> bool {
        matches!(
            self.sig.receiver(),
            Some(&FnArg::Receiver(Receiver {
                reference: Some(_),
                mutability: Some(_),
                ..
            }))
        )
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

        let method_call_args = method
            .sig
            .inputs
            .iter()
            .map(|arg| match arg {
                FnArg::Receiver(_) => Ok(Expr::Path(ExprPath {
                    attrs: Vec::new(),
                    qself: None,
                    path: ident_to_path(&format_ident!("self"), None),
                })),
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
            sig: method.sig.clone(),
            rpc_args,
            method_call_args,
        })
    }
}

#[derive(Clone)]
struct RpcArg {
    ident: Ident,
    arg_type: Box<Type>,
}

impl TryFrom<&PatType> for RpcArg {
    type Error = syn::Error;

    fn try_from(arg_type: &PatType) -> Result<Self, Self::Error> {
        match arg_type.pat.as_ref() {
            Pat::Ident(pat_ident) => Ok(RpcArg {
                ident: pat_ident.ident.to_owned(),
                arg_type: arg_type.ty.clone(),
            }),
            _ => Err(syn::Error::new(
                arg_type.span(),
                format!("only basic patterns are supported in service trait methods"),
            )),
        }
    }
}

fn enum_variant_to_path(
    enum_ident: &Ident,
    generics: &Generics,
    variant_ident: &Ident,
) -> syn::Path {
    let mut segments = Punctuated::new();

    segments.push(PathSegment {
        ident: enum_ident.clone(),
        arguments: generics_to_path_args(Some(&generics), true),
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
                Some(Default::default())
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
