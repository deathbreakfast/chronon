//! Chronon proc-macro crate.
//!
//! Provides the [`script`] attribute macro for auto-registering scheduled Rust functions.
//!
//! # Quick start
//!
//! 1. Annotate an async function with `#[chronon::script(name = "...")]`.
//! 2. First parameter must be `Box<dyn ScriptContext>`.
//! 3. Link the defining crate into your worker binary.
//! 4. Boot Chronon with `.auto_registry()` and a `ContextFactory` from `chronon-core`.
//!
//! # Identity
//!
//! Handlers receive `Box<dyn ScriptContext>`. At boot, install a factory on `ChrononBuilder`:
//!
//! - `JsonScriptContextFactory` for examples and actor-json-only handlers.
//! - A custom `ContextFactory` when handlers need application-specific session state rebuilt
//!   from `actor_json`.
//!
//! See `chronon-core` rustdoc for factory types. Runnable samples:
//! - `examples/script_macro.rs` — register script, `Job::new`, upsert, tick
//! - `examples/script_handle_job.rs` — typed `ScriptHandle` defaults

use proc_macro::TokenStream;

mod script;
mod script_attrs;
mod script_expand;
mod script_validate;

/// Marks an async function as a Chronon script, enabling automatic registration
/// and typed parameter handling.
///
/// # Requirements
///
/// - Function must be `async`
/// - First parameter must be `Box<dyn ScriptContext>`
/// - Return type must be `Result<()>` (for example `chronon_core::Result<()>`)
/// - `name` attribute is required and must be unique
/// - Parameters after `ScriptContext` must be simple identifiers
///
/// # Examples
///
/// ```ignore
/// #[chronon::script(name = "nightly_cleanup")]
/// async fn nightly_cleanup(
///     ctx: Box<dyn ScriptContext>,
///     retention_days: u32,
/// ) -> chronon::Result<()> {
///     let _ = (ctx.label(), retention_days);
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn script(attr: TokenStream, item: TokenStream) -> TokenStream {
    script::script_impl(attr, item)
}
