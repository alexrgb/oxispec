//! The graph: all nodes found under a project root, plus reverse links.

use std::collections::HashMap;
use std::path::Path;

use crate::finding::Finding;
use crate::id;
use crate::node::Node;

#[derive(Debug, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    /// Findings from reading files, before any check runs.
    pub findings: Vec<Finding>,
    /// Position of the first node with each ID, so lookups do not scan.
    by_id: HashMap<String, usize>,
}

impl Graph {
    /// Sorts the nodes by ID and path, and indexes them.
    pub fn new(mut nodes: Vec<Node>, findings: Vec<Finding>) -> Graph {
        nodes.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.path.cmp(&b.path)));
        let mut by_id = HashMap::with_capacity(nodes.len());
        for (position, node) in nodes.iter().enumerate() {
            by_id.entry(node.id.clone()).or_insert(position);
        }
        Graph {
            nodes,
            findings,
            by_id,
        }
    }

    /// Walks `root` for markdown files and parses all of them, without
    /// touching the local index. See [`crate::index::load`] for the cached version.
    pub fn load(root: &Path) -> Graph {
        crate::index::load(root, false).0
    }

    pub fn from_nodes(nodes: Vec<Node>) -> Graph {
        Graph::new(nodes, Vec::new())
    }

    pub fn get(&self, node_id: &str) -> Option<&Node> {
        let base = id::base_of(node_id);
        self.by_id.get(base).map(|&position| &self.nodes[position])
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
