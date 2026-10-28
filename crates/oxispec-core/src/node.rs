//! The node model: one markdown file with YAML frontmatter is one node.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Frontmatter as written by authors. Unknown keys are ignored.
#[derive(Debug, Default, Deserialize)]
pub struct Frontmatter {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub ty: Option<String>,
    pub status: Option<String>,
    pub owner: Option<String>,
    #[serde(default)]
    pub links: BTreeMap<String, OneOrMany>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    pub fn into_vec(self) -> Vec<String> {
        match self {
            OneOrMany::One(s) => vec![s],
            OneOrMany::Many(v) => v,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RequirementBlock {
    pub name: String,
    pub line: usize,
    /// The description contains SHALL or MUST.
    pub has_normative: bool,
    pub scenarios: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub status: Option<String>,
    pub owner: Option<String>,
    /// Path relative to the project root.
    pub path: PathBuf,
    /// Link kind to target IDs, for example `decided-by` to `[ADR-0007]`.
    pub links: BTreeMap<String, Vec<String>>,
    pub requirements: Vec<RequirementBlock>,
}

/// Node types known to this version. Others get a warning, not an error,
/// because later modules add types.
pub const KNOWN_TYPES: &[&str] = &[
    "requirement",
    "adr",
    "decision",
    "test-case",
    "standard",
    "api",
    "state-machine",
    "diagram",
    "term",
    "change",
    "system",
    "container",
    "component",
    "person",
    "deployment-environment",
    "release",
];
