use syn::parse::ParseStream;
use syn::{LitStr, Token};

/// `#[chronon::script(name = "...")]`
pub struct ScriptAttrs {
    pub name: String,
}

impl syn::parse::Parse for ScriptAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name: Option<String> = None;
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            if key == "name" {
                input.parse::<Token![=]>()?;
                let lit: LitStr = input.parse()?;
                if name.is_some() {
                    return Err(syn::Error::new(
                        key.span(),
                        "duplicate `name` in #[chronon::script]",
                    ));
                }
                name = Some(lit.value());
            } else {
                return Err(syn::Error::new(
                    key.span(),
                    "expected `name = \"...\"` in #[chronon::script]",
                ));
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }
        Ok(Self {
            name: name.ok_or_else(|| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "missing `name = \"...\"` for #[chronon::script]",
                )
            })?,
        })
    }
}
