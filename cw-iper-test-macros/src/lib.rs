use proc_macro::TokenStream;
use proc_macro2::TokenTree;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Expr, Lit, Meta};

#[proc_macro_derive(IbcPort, attributes(ibc_port))]
pub fn derive_ibc_port(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let struct_name = &input.ident;

    let port = get_attr("ibc_port", &input.attrs).expect("ibc_port attribute not found");

    let mut f: String = "".to_string();

    if let Meta::NameValue(a) = &port.meta {
        if let Expr::Lit(b) = &a.value {
            if let Lit::Str(c) = &b.lit {
                f = c.value();
            }
        }
    };

    if f == *"" {
        panic!("port attributes not in corrected format. Requested in format #[port = 'port']")
    }

    let trait_path: syn::Path = syn::parse_quote!(IbcPortInterface);

    let expanded = quote! {
        impl #struct_name {
            pub const IBC_PORT: &'static str = #f;
        }
        impl #trait_path for #struct_name {
            fn port_name(&self) -> String {
                #f.to_string()
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(Stargate, attributes(stargate))]
pub fn derive_stargate(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    let attributes = get_attr("stargate", &input.attrs).expect("stargate attribute not found");

    let mut query = None;

    let mut msgs = None;

    let mut name = None;

    if let Meta::List(list) = &attributes.meta {
        for (index, token) in list.tokens.clone().into_iter().enumerate() {
            if let TokenTree::Ident(ident) = token {
                if ident == "name" {
                    let a: Vec<TokenTree> = list.tokens.clone().into_iter().collect();
                    let a = a[index + 2].clone();

                    name = Some(quote! {#a})
                }
                if ident == "query_urls" {
                    let a: Vec<TokenTree> = list.tokens.clone().into_iter().collect();
                    let a = a[index + 2].clone();

                    query = Some(quote! {#a})
                }

                if ident == "msgs_urls" {
                    let a: Vec<TokenTree> = list.tokens.clone().into_iter().collect();
                    let a = a[index + 2].clone();
                    msgs = Some(quote! {#a})
                }
            }
        }
    }

    let query = query.expect("query_urls attribute not found");
    let msgs = msgs.expect("msgs_urls attribute not found");
    let name = name.expect("name attribute not found");

    let expanded = quote! {

        use strum::IntoEnumIterator;

        impl #struct_name {
            pub const STARGATE_NAME: &'static str = #name;
        }

        impl StargateUrls for #struct_name {

            fn stargate_name(&self) -> String {
                #name.to_string()
            }

            fn is_query_type_url(&self, type_url: String) -> bool {
                #query::from_str(&type_url).is_ok()
            }

            fn is_msg_type_url(&self, type_url: String) -> bool {
                #msgs::from_str(&type_url).is_ok()
            }

            fn type_urls(&self) -> Vec<String> {
                let mut urls = Vec::new();
                urls.extend(#query::iter().map(|url| url.to_string()));
                urls.extend(#msgs::iter().map(|url| url.to_string()));
                urls
            }
        }
    };

    TokenStream::from(expanded)
}

/// Implements following derive:
///
/// ```ignore
/// // Example
/// #[derive(strum_macros::EnumString, strum_macros::EnumIter, strum_macros::Display)]
/// pub enum Ics20MsgUrls {
///     #[strum(serialize = "/ibc.applications.transfer.v1.MsgTransfer")]
///     MsgTransfer,
///     ... // Others enum fields
/// }
#[proc_macro_attribute]
pub fn urls(_attr: proc_macro::TokenStream, input: proc_macro::TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let expanded = quote! {
        #[derive(strum_macros::EnumString, strum_macros::EnumIter, strum_macros::Display)]
        #input
    };
    TokenStream::from(expanded)
}

fn get_attr<'a>(attr_ident: &str, attrs: &'a [syn::Attribute]) -> Option<&'a syn::Attribute> {
    attrs.iter().find(|&attr| {
        attr.path().segments.len() == 1 && attr.path().segments[0].ident == attr_ident
    })
}
