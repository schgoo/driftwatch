//! `watch_point!("name", &expr)` — an inline checkpoint.
//!
//! Emits one `conformance.observation` event named `"name"` carrying `expr`'s
//! value (via [`ToValue`]) on the current operation span. A bare `watch_point!`
//! outside any operation emits nothing.
//!
//! [`ToValue`]: runtime::ToValue

use proc_macro::TokenStream;
use quote::quote_spanned;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, LitStr, parse_macro_input};

use crate::shared::rt;

/// The `"name", &expr` argument pair.
pub struct PointCall {
    name: LitStr,
    expr: Expr,
}

impl Parse for PointCall {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: LitStr = input.parse()?;
        let _: syn::Token![,] = input.parse()?;
        let expr: Expr = input.parse()?;
        Ok(PointCall { name, expr })
    }
}

pub fn expand(input: TokenStream) -> TokenStream {
    let PointCall { name, expr } = parse_macro_input!(input as PointCall);
    let rt = rt();
    quote_spanned! { name.span() =>
        #rt::push_observation(#name, #rt::ToValue::to_value(&#expr))
    }
    .into()
}
