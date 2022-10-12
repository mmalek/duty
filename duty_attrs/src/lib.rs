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
    let service_trait_ident = &service_trait.ident;
    let client_struct_ident = Ident::new(
        &format!("{}Client", service_trait_ident),
        service_trait_ident.span(),
    );
    let msg_enum_ident = Ident::new(
        &format!("{}Msg", service_trait_ident),
        service_trait_ident.span(),
    );

    let msg_enum = ItemEnum {
        attrs: Vec::new(),
        vis: service_trait.vis.clone(),
        enum_token: token::Enum {
            span: service_trait_ident.span(),
        },
        ident: msg_enum_ident.clone(),
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
        .zip(msg_enum.variants.iter())
        .map(|(method, variant)| -> ImplItemMethod {
            let msg_inst = ExprStruct {
                attrs: Vec::new(),
                path: enum_variant_to_path(&msg_enum_ident, &variant.ident),
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
                        .borrow()
                        .send(&#msg_inst)
                        .expect("Sending message error");
                    self.stream
                        .borrow_mut()
                        .receive()
                        .expect("Receiving message error")
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
        #msg_enum

        impl #service_trait_ident for #client_struct_ident {
            #(
                #methods
            )*
        }
    );

    // eprintln!("OUTPUT: {}", result);

    result.into()
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

#[cfg(test)]
mod tests {
    // use super::*;

    // #[test]
    // fn it_works() {
    //     let result = add(2, 2);
    //     assert_eq!(result, 4);
    // }
}
