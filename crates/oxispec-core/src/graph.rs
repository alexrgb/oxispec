//! The graph: all nodes found under a project root, plus reverse links.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use crate::finding::Finding;
use crate::id;
use crate::node::Node;
use crate::parse::parse_file_text;

#[derive(Debug, Default, Clone)]
pub struct Graph {
    /// The nodes, the findings and the ID index, shared by clones of a graph so
    /// that a long-running process hands one to each call without copying it.
    /// The fields read as if they were the graph's own (`graph.nodes`).
    data: Arc<Data>,
    /// What [`crate::check::check`] found, shared by clones of this graph so a
    /// long-running process runs the checks once per version of the files.
    pub(crate) checked: Arc<OnceLock<Vec<Finding>>>,
    /// The results of the claims, state machine and architecture analyses,
    /// kept the same way.
    pub(crate) analyses: Arc<Analyses>,
    /// The format profiles of the project, which the checks use. A graph built
    /// from files that are not a project on disk has none.
    pub(crate) formats: Arc<crate::formats::Formats>,
    /// How the project judges tech debt.
    pub(crate) debt: Arc<crate::config::DebtConfig>,
    /// The traceability and sign-offs the project asks for.
    pub(crate) compliance: Arc<crate::config::ComplianceConfig>,
}

#[derive(Debug, Default)]
pub struct Data {
    pub nodes: Vec<Node>,
    /// Findings from reading files, before any check runs.
    pub findings: Vec<Finding>,
    /// Position of the first node with each ID, so lookups do not scan.
    by_id: HashMap<String, usize>,
}

impl std::ops::Deref for Graph {
    type Target = Data;

    fn deref(&self) -> &Data {
        &self.data
    }
}

/// Analyses that are costly to repeat, each made once per version of the files.
#[derive(Debug, Default)]
pub(crate) struct Analyses {
    pub(crate) claims: OnceLock<Arc<crate::claims::Analysis>>,
    pub(crate) machines: OnceLock<Arc<crate::machines::Analysis>>,
    pub(crate) architecture: OnceLock<Arc<crate::architecture::Analysis>>,
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
            data: Arc::new(Data {
                nodes,
                findings,
                by_id,
            }),
            checked: Arc::default(),
            analyses: Arc::default(),
            formats: Arc::default(),
            debt: Arc::default(),
            compliance: Arc::default(),
        }
    }

    /// The project's rules for tech debt.
    pub fn debt_config(&self) -> &crate::config::DebtConfig {
        &self.debt
    }

    /// The project's compliance settings.
    pub fn compliance_config(&self) -> &crate::config::ComplianceConfig {
        &self.compliance
    }

    /// The same graph, with the project's compliance settings.
    pub fn with_compliance(mut self, compliance: crate::config::ComplianceConfig) -> Graph {
        self.compliance = Arc::new(compliance);
        self
    }

    /// The same graph, with the project's rules for tech debt.
    pub fn with_debt(mut self, debt: crate::config::DebtConfig) -> Graph {
        self.debt = Arc::new(debt);
        self
    }

    /// The same graph, checked against these format profiles.
    pub fn with_formats(mut self, formats: crate::formats::Formats) -> Graph {
        self.formats = Arc::new(formats);
        self
    }

    /// Walks `root` for markdown files and parses all of them, without
    /// touching the local index. See [`crate::index::load`] for the cached version.
    pub fn load(root: &Path) -> Graph {
        crate::index::load(root, false).0
    }

    /// Builds a graph from files that are not on disk, for example read from
    /// an older git revision. Paths are relative to the project root.
    pub fn from_sources<I>(sources: I) -> Graph
    where
        I: IntoIterator<Item = (PathBuf, Vec<u8>)>,
    {
        let mut nodes = Vec::new();
        let mut findings = Vec::new();
        for (path, bytes) in sources {
            match String::from_utf8(bytes) {
                Ok(text) => {
                    let (node, parse_findings) = parse_file_text(&path, &text);
                    nodes.extend(node);
                    findings.extend(parse_findings);
                }
                Err(_) => findings.push(
                    Finding::warning("W003", "cannot read file: not valid UTF-8").at(&path, None),
                ),
            }
        }
        Graph::new(nodes, findings)
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
    // spec: REQ-FORMAT-0006
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

#[cfg(test)]
mod tests_of_memoized_analyses {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn a_clone_shares_the_nodes_and_does_not_copy_them() {
        let graph = Graph::from_sources([(
            PathBuf::from("specs/a.md"),
            b"---\nid: a\ntype: requirement\n---\n# A\n".to_vec(),
        )]);
        let copy = graph.clone();
        assert_eq!(copy.nodes.len(), 1);
        assert!(std::ptr::eq(graph.nodes.as_ptr(), copy.nodes.as_ptr()));
    }

    #[test]
    fn clones_of_a_graph_share_one_analysis_of_each_kind() {
        let graph = Graph::new(Vec::new(), Vec::new());
        let copy = graph.clone();
        assert!(Arc::ptr_eq(
            &crate::claims::analyze(&graph),
            &crate::claims::analyze(&copy)
        ));
        assert!(Arc::ptr_eq(
            &crate::machines::analyze(&graph),
            &crate::machines::analyze(&copy)
        ));
        assert!(Arc::ptr_eq(
            &crate::architecture::analyze(&graph),
            &crate::architecture::analyze(&copy)
        ));
    }
}
