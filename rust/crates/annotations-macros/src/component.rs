//! `watch_component!("name")` — the crate-root component declaration.
//!
//! Expands to a `pub(crate) const __DRIFTWATCH_COMPONENT` that
//! `#[watch_operation]` references for its
//! `component` field. Invoke it ONCE at a crate root (lib.rs / main.rs / an
//! integration-test file root); omitting it in a crate that has annotations is a
//! compile-time error ("cannot find value `__DRIFTWATCH_COMPONENT`").

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

use crate::shared::NameArg;

pub fn expand(input: TokenStream) -> TokenStream {
    let NameArg(name) = parse_macro_input!(input as NameArg);
    quote! {
        #[allow(dead_code, reason = "referenced by macro-generated registry entries")]
        pub(crate) const __DRIFTWATCH_COMPONENT: &str = #name;
    }
    .into()
}
