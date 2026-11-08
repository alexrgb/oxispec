//! OxiSpec core: the model, parser, graph and checks that the `oxispec` command is made of.
//!
//! Specifications are markdown files with a front matter. A [`Graph`] holds them as
//! [`Node`]s with typed links between them, and everything else reads the graph.
//!
//! ```
//! use std::path::PathBuf;
//! use oxispec_core::{Graph, Severity, check};
//!
//! // Lines are joined with newlines. (A line of a doc test that starts with `#` is hidden.)
//! let text = [
//!     "---",
//!     "id: REQ-A-0001",
//!     "type: requirement",
//!     "status: accepted",
//!     "---",
//!     "### Requirement: Lock",
//!     "The system SHALL lock an account after 5 failed logins.",
//!     "",
//!     "#### Scenario: Five failures {#S1}",
//!     "- **WHEN** a user fails 5 logins",
//!     "- **THEN** the account is locked",
//! ]
//! .join("\n");
//! let graph = Graph::from_sources([(PathBuf::from("specs/lock.md"), text.as_bytes().to_vec())]);
//!
//! assert_eq!(graph.nodes.len(), 1);
//! assert_eq!(graph.get("REQ-A-0001").unwrap().ty, "requirement");
//! assert!(check(&graph).iter().all(|f| f.severity != Severity::Error));
//! ```
//!
//! # Where things are
//!
//! | To do this | Use |
//! |---|---|
//! | Read a project, or text that is not on disk | [`Graph::load`], [`Graph::from_sources`] |
//! | Find what is wrong | [`check::check`] |
//! | Tie code and tests to requirements, and count coverage | [`trace`] |
//! | Compare two states of the specs | [`diff::diff_graphs`] |
//! | Pick text for an AI agent within a budget | [`context::pack`] |
//! | Search the text of the nodes | [`search`] |
//! | Read an API from requirements, and search it | [`api`], [`apiindex`] |
//! | Risks, debt, test plans, releases | [`risks`], [`debt`], [`testplan`], [`release`] |
//!
//! Runnable examples are in the `examples` folder of this crate, and
//! `docs/library.md` explains them. The API is not stable before 1.0: a minor
//! version may change it.

pub mod access;
pub mod adr;
pub mod agents;
pub mod api;
pub mod apiindex;
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
pub mod entities;
pub mod finding;
pub mod formats;
pub mod graph;
pub mod id;
pub mod impact;
pub mod import;
pub mod import_plan;
pub mod index;
pub mod languages;
pub mod machines;
pub mod modules;
pub mod node;
pub mod parse;
pub mod prd;
pub mod release;
pub mod report;
pub mod results;
pub mod risks;
pub mod rollback;
pub mod search;
pub mod site;
pub mod status;
pub mod styles;
pub mod suspect;
pub mod symbol;
pub mod testplan;
pub mod trace;

pub use check::check;
pub use finding::{Finding, Severity};
pub use graph::Graph;
pub use node::{Node, RequirementBlock, Scenario};

/// Version of the JSON output of `check` and `show`. Adding fields does not
/// change it. Removing, renaming or retyping a field does. See docs/versioning.md.
pub const OUTPUT_SCHEMA_VERSION: u32 = 1;
