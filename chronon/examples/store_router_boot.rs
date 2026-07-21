//! Register an in-memory store on the global router and build from it.
//!
//! ```bash
//! cargo run -p uf-chronon --example store_router_boot --features mem
//! ```

use chronon::prelude::*;
use chronon_backend_mem::install_default_mem_store;

fn main() -> chronon::Result<()> {
    let _store = install_default_mem_store();
    let chronon = ChrononBuilder::new()
        .scheduler_store_from_global()?
        .embedded()
        .build()?;

    assert_eq!(chronon.executor().script_count(), 0);
    eprintln!("Chronon booted from global mem store");
    Ok(())
}
