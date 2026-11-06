//! A product requirements document written from the nodes.
//!
//! A `prd` node holds the product's purpose, goals, non-goals and open
//! questions. The requirements that `refine` it are its functional
//! requirements, and their scenarios are the acceptance criteria. The document
//! is a view: it is made from the nodes each time, never edited, so it cannot
//! drift from them.

use std::fs;
use std::path::Path;

use crate::graph::Graph;
use crate::import::{sections, text_of};

pub const TEMPLATE_SECTIONS: &[&str] = &[
    "Overview",
    "Users",
    "Goals",
    "Non-goals",
    "Success metrics",
    "Open questions",
];

fn body_of(text: &str) -> &str {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    match text.strip_prefix("---") {
        Some(rest) => rest
            .split_once("\n---")
            .map_or(text, |(_, after)| after.trim_start_matches(['\r', '\n'])),
        None => text,
    }
}

/// A requirement node as acceptance criteria: each block a heading and its
/// text, each scenario a bullet with its steps.
fn requirement_text(text: &str) -> String {
    let (_, secs) = sections(body_of(text));
    let mut out = String::new();
    for s in &secs {
        if let Some(name) = s.title.strip_prefix("Requirement:") {
            out.push_str(&format!("\n#### {}\n\n{}\n", name.trim(), text_of(&s.body)));
        } else if let Some(name) = s.title.strip_prefix("Scenario:") {
            let name = match name.rfind("{#") {
                Some(i) => name[..i].trim(),
                None => name.trim(),
            };
            let steps: Vec<String> = s
                .body
                .iter()
                .filter_map(|l| l.trim().strip_prefix("- "))
                .map(|l| l.replace("**", ""))
                .collect();
            out.push_str(&format!("- *{name}*: {}\n", steps.join("; ")));
        }
    }
    out
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    NoSuchNode(String),
    NotAPrd(String),
    Unreadable(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSuchNode(id) => write!(f, "no node {id}"),
            Self::NotAPrd(id) => write!(f, "{id} is not a prd node"),
            Self::Unreadable(why) => write!(f, "cannot read a node: {why}"),
        }
    }
}

impl std::error::Error for Error {}

/// The document for the `prd` node `id`, read from the files under `root`.
// spec: REQ-PRD-0001
pub fn render(graph: &Graph, root: &Path, id: &str) -> Result<String, Error> {
    let node = graph
        .get(id)
        .ok_or_else(|| Error::NoSuchNode(id.to_string()))?;
    if node.ty != "prd" {
        return Err(Error::NotAPrd(node.id.clone()));
    }
    let read = |p: &Path| {
        fs::read_to_string(root.join(p))
            .map(|t| t.replace("\r\n", "\n"))
            .map_err(|e| Error::Unreadable(format!("{}: {e}", p.display())))
    };
    let own = read(&node.path)?;
    let title = node.title.clone().unwrap_or_else(|| node.id.clone());
    let mut out = format!("# {title}\n\n");
    out.push_str(&format!(
        "Status: {}. Made from `{}` and the requirements that refine it; do not edit.\n",
        node.status.as_deref().unwrap_or("none"),
        node.id
    ));
    let (_, secs) = sections(body_of(&own));
    for s in secs.iter().filter(|s| s.level == 2) {
        let text = text_of(&s.body);
        out.push_str(&format!("\n## {}\n\n", s.title));
        out.push_str(if text.is_empty() {
            "_Not written yet._"
        } else {
            &text
        });
        out.push('\n');
    }
    let mut reqs: Vec<_> = graph
        .incoming(&node.id)
        .into_iter()
        .filter(|(n, kind)| *kind == "refines" && n.ty == "requirement")
        .map(|(n, _)| n)
        .collect();
    reqs.sort_by(|a, b| a.id.cmp(&b.id));
    reqs.dedup_by(|a, b| a.id == b.id);
    out.push_str("\n## Functional requirements\n");
    if reqs.is_empty() {
        out.push_str(&format!(
            "\n_None yet. Link a requirement with `oxispec link <REQ-ID> refines {}`._\n",
            node.id
        ));
    }
    for r in reqs {
        let rt = r.title.clone().unwrap_or_else(|| r.id.clone());
        out.push_str(&format!(
            "\n### {}: {} ({})\n",
            r.id,
            rt,
            r.status.as_deref().unwrap_or("no status")
        ));
        out.push_str(&requirement_text(&read(&r.path)?));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn project() -> (tempfile::TempDir, Graph) {
        let dir = tempfile::tempdir().unwrap();
        let files = [
            (
                "specs/prd/0001-checkout.md",
                "---\nid: PRD-0001\ntype: prd\nstatus: draft\n---\n# PRD-0001: Checkout\n\n## Overview\nLet shoppers pay.\n\n## Goals\n- Fewer abandoned carts\n\n## Non-goals\n",
            ),
            (
                "specs/requirements/pay/card.md",
                "---\nid: REQ-PAY-0001\ntype: requirement\nstatus: draft\nlinks:\n  refines: PRD-0001\n---\n# REQ-PAY-0001: Card\n\n### Requirement: Pay by card\nThe system SHALL take card payments.\n\n#### Scenario: Valid card {#S1}\n- **WHEN** the card is valid\n- **THEN** the order is placed\n",
            ),
        ];
        let mut sources = Vec::new();
        for (rel, text) in files {
            let path = dir.path().join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, text).unwrap();
            sources.push((PathBuf::from(rel), text.as_bytes().to_vec()));
        }
        (dir, Graph::from_sources(sources))
    }

    // verifies: REQ-PRD-0001.S1
    #[test]
    fn the_document_has_the_sections_and_the_refining_requirements() {
        let (dir, graph) = project();
        let doc = render(&graph, dir.path(), "PRD-0001").unwrap();
        assert!(
            doc.starts_with("# PRD-0001: Checkout") || doc.starts_with("# Checkout"),
            "{doc}"
        );
        assert!(doc.contains("## Overview\n\nLet shoppers pay."), "{doc}");
        assert!(doc.contains("- Fewer abandoned carts"), "{doc}");
        assert!(doc.contains("## Non-goals\n\n_Not written yet._"), "{doc}");
        assert!(doc.contains("### REQ-PAY-0001: "), "{doc}");
        assert!(doc.contains("#### Pay by card"), "{doc}");
        assert!(
            doc.contains("- *Valid card*: WHEN the card is valid; THEN the order is placed"),
            "{doc}"
        );
    }

    // verifies: REQ-PRD-0001.S2
    #[test]
    fn only_a_prd_node_can_be_rendered() {
        let (dir, graph) = project();
        assert_eq!(
            render(&graph, dir.path(), "REQ-PAY-0001"),
            Err(Error::NotAPrd("REQ-PAY-0001".into()))
        );
        assert_eq!(
            render(&graph, dir.path(), "PRD-9"),
            Err(Error::NoSuchNode("PRD-9".into()))
        );
    }
}
