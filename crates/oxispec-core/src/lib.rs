//! OxiSpec core: node model, parser, graph and structural checks.

pub mod access;
pub mod adr;
pub mod agents;
pub mod architecture;
pub mod change;
pub mod check;
pub mod claims;
pub mod compliance;
pub mod config;
pub mod connect;
pub mod context;
pub mod debt;
pub mod diff;
pub mod edit;
pub mod finding;
pub mod formats;
pub mod graph;
pub mod id;
pub mod impact;
pub mod import;
pub mod index;
pub mod machines;
pub mod modules;
pub mod node;
pub mod parse;
pub mod release;
pub mod report;
pub mod results;
pub mod rollback;
pub mod search;
pub mod site;
pub mod status;
pub mod styles;
pub mod suspect;
pub mod symbol;
pub mod trace;

pub use check::check;
pub use finding::{Finding, Severity};
pub use graph::Graph;
pub use node::{Node, RequirementBlock, Scenario};

/// Version of the JSON output of `check` and `show`. Adding fields does not
/// change it. Removing, renaming or retyping a field does. See docs/versioning.md.
pub const OUTPUT_SCHEMA_VERSION: u32 = 1;
