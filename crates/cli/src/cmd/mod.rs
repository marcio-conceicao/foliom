//! CLI subcommand implementations. One module per subcommand keeps
//! `main.rs` skinny and lets each command own its argument struct.

pub mod dump_tree;
pub mod index;
pub mod inventory;
pub mod reindex;
pub mod search;
pub mod serve;
