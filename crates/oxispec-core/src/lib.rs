//! OxiSpec core: node model, parser, graph and structural checks.

pub mod agents;
pub mod change;
pub mod check;
pub mod claims;
pub mod config;
pub mod context;
pub mod diff;
pub mod edit;
pub mod finding;
pub mod graph;
pub mod id;
pub mod impact;
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
