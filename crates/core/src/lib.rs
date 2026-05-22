//! Foliom core — pure parser, storage, scanner and indexer logic.
//!
//! This crate intentionally exposes no IO bindings (no HTTP, no UI). It is
//! consumed by `foliom-cli` (and, in later phases, by `foliom-server` and
//! the Tauri desktop shell).

pub mod indexer;
pub mod inventory;
pub mod mutation;
pub mod parser;
pub mod path;
pub mod query;
pub mod rename;
pub mod scanner;
pub mod storage;
pub mod sync;
