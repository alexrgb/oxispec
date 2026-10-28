//! The graph: all nodes found under a project root, plus reverse links.

use std::path::Path;

use crate::finding::{Finding, Severity};
use crate::id;
use crate::node::Node;
use crate::parse::parse_file_text;

#[derive(Debug, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    /// Findings from reading files, before any check runs.
    pub findings: Vec<Finding>,
}

impl Graph {
    /// Walks `root` for markdown files. Hidden folders and anything ignored
    /// by `.gitignore` are skipped, so `.oxispec/` and `target/` are not read.
    pub fn load(root: &Path) -> Graph {
        let mut graph = Graph::default();
        let walker = ignore::WalkBuilder::new(root).require_git(false).build();
        for entry in walker.flatten() {
            let path = entry.path();
            let is_md = path.extension().is_some_and(|e| e == "md");
            if !is_md || !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let rel = path.strip_prefix(root).unwrap_or(path);
            match std::fs::read_to_string(path) {
                Ok(text) => {
                    let (node, findings) = parse_file_text(rel, &text);
                    graph.findings.extend(findings);
                    graph.nodes.extend(node);
                }
                Err(e) => graph.findings.push(
                    Finding::new("W003", Severity::Warning, format!("cannot read file: {e}"))
                        .at(rel, None),
                ),
            }
        }
        graph
            .nodes
            .sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.path.cmp(&b.path)));
        graph
    }

    pub fn from_nodes(nodes: Vec<Node>) -> Graph {
        Graph {
            nodes,
            findings: Vec::new(),
        }
    }

    pub fn get(&self, node_id: &str) -> Option<&Node> {
        let base = id::base_of(node_id);
        self.nodes.iter().find(|n| n.id == base)
    }

    /// Nodes that link to `node_id`, with the link kind. Reverse links are
    /// computed, never stored.
    pub fn incoming(&self, node_id: &str) -> Vec<(&Node, &str)> {
        let base = id::base_of(node_id);
        let mut out = Vec::new();
        for n in &self.nodes {
            for (kind, targets) in &n.links {
                if targets.iter().any(|t| id::base_of(t) == base) {
                    out.push((n, kind.as_str()));
                }
            }
        }
        out
    }
}
