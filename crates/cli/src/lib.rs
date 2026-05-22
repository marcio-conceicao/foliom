//! `foliom-cli` library surface — re-exports the modules needed by integration
//! tests so they can build in-process axum routers without going through the
//! `main()` binary entry point.
//!
//! The `[[bin]]` target (`src/main.rs`) depends on the same `cmd` tree via
//! `mod cmd;`. Integration tests (`tests/`) depend on this `[lib]` target via
//! `use foliom_cli::cmd::serve::...`.

pub mod cmd;
