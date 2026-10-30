//! Findings reported by the parser and the checks.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub path: Option<PathBuf>,
    pub line: Option<usize>,
    pub id: Option<String>,
}

impl Finding {
    pub fn new(code: &'static str, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            code,
            severity,
            message: message.into(),
            path: None,
            line: None,
            id: None,
        }
    }

    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Error, message)
    }

    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Warning, message)
    }

    pub fn info(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Info, message)
    }

    pub fn at(mut self, path: &Path, line: Option<usize>) -> Self {
        self.path = Some(path.to_path_buf());
        self.line = line;
        self
    }

    pub fn node(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }
}

/// Every check code with a one-line meaning, for reports. Kept in step with
/// the table in docs/format.md by a test.
pub const CODES: &[(&str, &str)] = &[
    ("E001", "Two nodes have the same ID"),
    ("E002", "A link points to a node that does not exist"),
    ("E003", "Frontmatter has `id` or `type`, but not both"),
    ("E004", "`supersedes` links form a cycle"),
    ("E005", "An ID is not valid"),
    ("E006", "A requirement has no scenario"),
    ("E007", "A requirement has no SHALL or MUST"),
    ("E008", "A requirement node has no `### Requirement:` block"),
    ("E009", "A link points to a scenario that does not exist"),
    ("E010", "A scenario anchor is used twice in one file"),
    ("E011", "A scenario anchor is not letters and digits"),
    (
        "E012",
        "A change set has no status, or one that is not in the lifecycle",
    ),
    (
        "E013",
        "A change set is approved or merged but has no `approved_by`",
    ),
    (
        "E014",
        "A code annotation refers to a node that does not exist",
    ),
    (
        "E015",
        "A code annotation refers to a scenario that does not exist",
    ),
    (
        "E016",
        "A link between a spec and code is suspect: either one changed since it was confirmed",
    ),
    ("E017", "A claim cannot be read"),
    ("E018", "A vocabulary block cannot be read"),
    (
        "E019",
        "Claims conflict: in some context, no value satisfies them all",
    ),
    ("W001", "Unknown node type"),
    (
        "W002",
        "Frontmatter is not valid YAML, so the file was skipped",
    ),
    ("W003", "A file could not be read"),
    ("W004", "Unknown link kind"),
    (
        "W005",
        "A SHOULD claim cannot hold in a context where other claims apply",
    ),
    (
        "W006",
        "A claim never applies, because its scope can never hold",
    ),
    ("I001", "A link to another project is not checked"),
    (
        "I002",
        "A claim says nothing that another claim does not already say",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Codes written as string literals in the sources.
    fn codes_in(source: &str) -> Vec<String> {
        let bytes = source.as_bytes();
        let mut found = Vec::new();
        for i in 0..bytes.len().saturating_sub(6) {
            let w = &bytes[i..i + 6];
            if w[0] == b'"'
                && matches!(w[1], b'E' | b'W' | b'I')
                && w[2..5].iter().all(u8::is_ascii_digit)
                && w[5] == b'"'
            {
                found.push(String::from_utf8_lossy(&w[1..5]).into_owned());
            }
        }
        found
    }

    #[test]
    fn the_table_matches_the_checks_and_the_docs() {
        let sources = [
            include_str!("check.rs"),
            include_str!("parse.rs"),
            include_str!("graph.rs"),
            include_str!("index.rs"),
            include_str!("trace.rs"),
            include_str!("suspect.rs"),
        ];
        let docs = include_str!("../../../docs/format.md");
        let known: Vec<&str> = CODES.iter().map(|(c, _)| *c).collect();
        for source in sources {
            for code in codes_in(source) {
                assert!(known.contains(&code.as_str()), "{code} is not in CODES");
            }
        }
        for (code, _) in CODES {
            assert!(
                docs.contains(&format!("| {code} |")),
                "{code} is not in docs/format.md"
            );
        }
    }
}
