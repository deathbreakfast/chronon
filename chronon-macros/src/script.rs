//! Implementation of the `#[chronon::script]` proc macro.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use syn::ItemFn;

use crate::script_attrs::ScriptAttrs;
use crate::script_expand::expand_script;
use crate::script_validate::validate_signature;

pub fn script_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    match script_impl_impl(attr.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn script_impl_impl(attr: TokenStream2, item: TokenStream2) -> syn::Result<TokenStream2> {
    let attrs: ScriptAttrs = syn::parse2(attr)?;
    let input: ItemFn = syn::parse2(item)?;
    validate_signature(&input.sig)?;
    expand_script(attrs, input)
}
