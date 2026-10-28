//! OxiSpec core: node model, parser, graph and structural checks.

pub mod check;
pub mod config;
pub mod finding;
pub mod graph;
pub mod id;
pub mod modules;
pub mod node;
pub mod parse;

pub use check::check;
pub use finding::{Finding, Severity};
pub use graph::Graph;
pub use node::{Node, RequirementBlock};
