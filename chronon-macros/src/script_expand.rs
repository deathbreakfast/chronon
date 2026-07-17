use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use syn::spanned::Spanned;
use syn::{ItemFn, LitStr, Pat, PatType, Signature};

use crate::script_attrs::ScriptAttrs;
use crate::script_validate::collect_script_params;

pub(crate) fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

pub(crate) fn expand_script(attrs: ScriptAttrs, input: ItemFn) -> syn::Result<TokenStream2> {
    let script_name = attrs.name;
    let fn_name = &input.sig.ident;
    let fn_vis = &input.vis;
    let fn_block = &input.block;
    let fn_attrs = &input.attrs;
    let fn_sig = &input.sig;
    let params = collect_script_params(fn_sig)?;
    let (signature_json, signature_hash) = build_signature_metadata(&params)?;

    let fn_name_pascal = to_pascal_case(&fn_name.to_string());
    let params_struct_name = syn::Ident::new(&format!("{}Params", fn_name_pascal), fn_name.span());
    let script_type_name_str = if fn_name_pascal.ends_with("Script") {
        fn_name_pascal
    } else {
        format!("{}Script", fn_name_pascal)
    };
    let script_type_name = syn::Ident::new(&script_type_name_str, fn_name.span());
    let is_unit_struct = params.is_empty();

    let params_struct =
        generate_params_struct(fn_vis, &params_struct_name, &params, is_unit_struct);
    let handle_fn = generate_handle_function(fn_vis, fn_name, &params_struct_name, &script_name);
    let script_type_api =
        generate_script_type_api(fn_vis, &script_type_name, &params_struct_name, &script_name);
    let internal_sig = generate_internal_signature(fn_sig, fn_name);
    let internal_fn_name = &internal_sig.ident;
    let deserialize_code = generate_deserialization_code(&params_struct_name, is_unit_struct);
    let invoke_script = generate_invoke_script(internal_fn_name, &params, is_unit_struct)?;
    let signature_json_lit = LitStr::new(&signature_json, fn_name.span());
    let script_name_lit = LitStr::new(&script_name, fn_name.span());

    Ok(quote! {
        #params_struct

        #handle_fn

        #script_type_api

        #(#fn_attrs)*
        #fn_vis #internal_sig #fn_block

        ::quark::inventory::submit! {
            ::chronon_executor::ScriptDescriptor::with_signature(
                #script_name_lit,
                |ctx, params_json| {
                    ::std::boxed::Box::pin(async move {
                        #deserialize_code
                        #invoke_script
                    })
                },
                #signature_json_lit,
                #signature_hash,
            )
        }
    })
}

fn build_signature_metadata(params: &[&PatType]) -> syn::Result<(String, u64)> {
    let mut signature = BTreeMap::new();
    for pat_type in params {
        let Pat::Ident(pat_ident) = pat_type.pat.as_ref() else {
            return Err(syn::Error::new(
                pat_type.pat.span(),
                "#[chronon::script] parameters after ScriptContext must be simple identifiers",
            ));
        };
        let name = pat_ident.ident.to_string();
        let ty_tokens = &pat_type.ty;
        let ty = quote! { #ty_tokens }.to_string();
        signature.insert(name, ty);
    }
    let signature_json = serde_json::to_string(&signature).map_err(|e| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("failed to build script signature metadata: {}", e),
        )
    })?;
    let mut hasher = DefaultHasher::new();
    signature_json.hash(&mut hasher);
    Ok((signature_json, hasher.finish()))
}

fn generate_params_struct(
    fn_vis: &syn::Visibility,
    params_struct_name: &syn::Ident,
    params: &[&PatType],
    is_unit_struct: bool,
) -> TokenStream2 {
    if is_unit_struct {
        quote! {
            #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
            #fn_vis struct #params_struct_name;
        }
    } else {
        quote! {
            #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
            #fn_vis struct #params_struct_name {
                #(#fn_vis #params),*
            }
        }
    }
}

fn generate_handle_function(
    fn_vis: &syn::Visibility,
    fn_name: &syn::Ident,
    params_struct_name: &syn::Ident,
    script_name: &str,
) -> TokenStream2 {
    quote! {
        #fn_vis fn #fn_name() -> ::chronon_core::ScriptHandle<#params_struct_name> {
            ::chronon_core::ScriptHandle::new(#script_name)
        }
    }
}

fn generate_script_type_api(
    fn_vis: &syn::Visibility,
    script_type_name: &syn::Ident,
    params_struct_name: &syn::Ident,
    script_name: &str,
) -> TokenStream2 {
    quote! {
        #fn_vis struct #script_type_name;

        impl #script_type_name {
            pub const NAME: &'static str = #script_name;

            pub fn handle() -> ::chronon_core::ScriptHandle<#params_struct_name> {
                ::chronon_core::ScriptHandle::new(Self::NAME)
            }
        }
    }
}

fn generate_internal_signature(fn_sig: &Signature, fn_name: &syn::Ident) -> Signature {
    let mut internal_sig = fn_sig.clone();
    internal_sig.ident = syn::Ident::new(&format!("__{}_impl", fn_name), fn_name.span());
    internal_sig
}

fn generate_deserialization_code(
    params_struct_name: &syn::Ident,
    is_unit_struct: bool,
) -> TokenStream2 {
    if is_unit_struct {
        quote! {
            let params: #params_struct_name =
                if params_json.is_object() && params_json.as_object().map(|o| o.is_empty()).unwrap_or(false) {
                    serde_json::from_value(serde_json::Value::Null)?
                } else {
                    serde_json::from_value(params_json)?
                };
        }
    } else {
        quote! {
            let params: #params_struct_name = serde_json::from_value(params_json)?;
        }
    }
}

fn generate_invoke_script(
    internal_fn_name: &syn::Ident,
    params: &[&PatType],
    is_unit_struct: bool,
) -> syn::Result<TokenStream2> {
    if is_unit_struct {
        return Ok(quote! {
            #internal_fn_name(ctx).await
        });
    }

    let param_accessors = param_accessors(params)?;
    Ok(quote! {
        #internal_fn_name(ctx, #(#param_accessors),*).await
    })
}

fn param_accessors(params: &[&PatType]) -> syn::Result<Vec<TokenStream2>> {
    params
        .iter()
        .map(|pat_type| {
            if let Pat::Ident(pat_ident) = pat_type.pat.as_ref() {
                let ident = &pat_ident.ident;
                Ok(quote! { params.#ident })
            } else {
                Err(syn::Error::new(
                    pat_type.pat.span(),
                    "#[chronon::script] parameters after ScriptContext must be simple identifiers",
                ))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn script_expand_produces_inventory_submit() {
        let attrs = ScriptAttrs {
            name: "daily_cleanup".into(),
        };
        let input: ItemFn = parse_quote! {
            pub async fn daily_cleanup(
                ctx: Box<dyn chronon_core::ScriptContext>,
                dry_run: bool,
            ) -> chronon_core::Result<()> {
                let _ = (ctx, dry_run);
                Ok(())
            }
        };
        let tokens = expand_script(attrs, input).expect("expand");
        let expanded = tokens.to_string();
        assert!(expanded.contains("ScriptDescriptor"));
        assert!(expanded.contains("inventory"));
        assert!(expanded.contains("DailyCleanupParams"));
    }
}
