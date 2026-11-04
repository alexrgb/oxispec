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
    pub approved_by: Option<String>,
    pub approved_on: Option<String>,
    /// The version a release node stands for.
    pub version: Option<String>,
    /// When a decision was last updated (`YYYY-MM-DD`).
    pub date: Option<String>,
    #[serde(rename = "decision-makers")]
    pub decision_makers: Option<OneOrMany>,
    /// How serious a debt item is.
    pub severity: Option<String>,
    /// When a debt item is due (`YYYY-MM-DD`).
    pub due: Option<String>,
    /// What it takes to pay a debt item off, and what it costs while it stays.
    pub cost: Option<String>,
    pub interest: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    /// Raw text of a trailing `{#anchor}` on the heading. The checks validate it.
    pub anchor: Option<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementBlock {
    pub name: String,
    pub line: usize,
    /// The description contains SHALL or MUST.
    pub has_normative: bool,
    pub scenarios: Vec<Scenario>,
}

/// One line of a fenced `claims` block, as written. [`crate::claims`] reads it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimLine {
    pub line: usize,
    pub text: String,
}

/// A `##` or `###` heading, for the checks that need to know what sections a node has.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub line: usize,
}

/// The text of a fenced `vocabulary` block and the line it starts on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub status: Option<String>,
    pub owner: Option<String>,
    /// Who approved a change set, and when (`YYYY-MM-DD`).
    pub approved_by: Option<String>,
    pub approved_on: Option<String>,
    /// The version of a release node, as written in its frontmatter.
    pub version: Option<String>,
    /// When a decision was last updated, as written in its frontmatter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// Who made a decision, as written in its frontmatter.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decision_makers: Vec<String>,
    /// For a debt item: how serious it is, when it is due, and what it costs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interest: Option<String>,
    /// The first `# ` heading, without a leading `ID: `.
    pub title: Option<String>,
    /// Path relative to the project root.
    pub path: PathBuf,
    /// Link kind to target IDs, for example `decided-by` to `[ADR-0007]`.
    pub links: BTreeMap<String, Vec<String>>,
    pub requirements: Vec<RequirementBlock>,
    /// Lines of `claims` blocks, for the consistency checks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claims: Vec<ClaimLine>,
    /// `vocabulary` blocks, which declare the variables claims talk about.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vocabulary: Vec<TextBlock>,
    /// `statemachine` blocks, read by [`crate::machines`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub machines: Vec<TextBlock>,
    /// `architecture` blocks, read by [`crate::architecture`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub architecture: Vec<TextBlock>,
    /// Headings of level 2 and 3 outside code blocks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headings: Vec<Heading>,
    /// BLAKE3 hash of everything after the frontmatter, to see that the text changed.
    #[serde(default)]
    pub body_hash: String,
}

impl Node {
    /// True when some scenario heading in this node carries the anchor.
    pub fn has_scenario(&self, anchor: &str) -> bool {
        self.requirements
            .iter()
            .flat_map(|r| &r.scenarios)
            .any(|s| s.anchor.as_deref() == Some(anchor))
    }
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
    "vocabulary",
    "debt",
];

/// Link kinds known to this version (design section 4). Others get a
/// warning, which catches typos while leaving room for later modules.
pub const KNOWN_LINK_KINDS: &[&str] = &[
    "satisfies",
    "verifies",
    "implements",
    "decided-by",
    "supersedes",
    "amends",
    "clarifies",
    "refines",
    "governed-by",
    "describes",
    "overrides",
    "conflicts-with",
    "depends-on",
    "changes",
    "includes",
    "arises-from",
];
