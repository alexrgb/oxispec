//! Which nodes a change touches: the changed files, and the nodes that link to them.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::finding::Finding;
use crate::graph::Graph;
use crate::id;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Impact {
    /// Files whose findings count: the changed files, and files with a link to a touched node.
    pub paths: BTreeSet<PathBuf>,
    /// IDs of the changed nodes, as they are now and as they were before the change.
    pub ids: BTreeSet<String>,
    /// Number of nodes in the graph that are in scope.
    pub nodes: usize,
}

// spec: REQ-IMPACT-0001
/// `changed_paths` are the files that differ from the base, relative to the
/// project root. `old_ids` are the IDs the changed files had at the base,
/// which is how a deleted or renamed node still finds the nodes that linked to it.
pub fn impact(
    graph: &Graph,
    changed_paths: &BTreeSet<PathBuf>,
    old_ids: &BTreeSet<String>,
) -> Impact {
    let mut ids = old_ids.clone();
    ids.extend(
        graph
            .nodes
            .iter()
            .filter(|n| changed_paths.contains(&n.path))
            .map(|n| n.id.clone()),
    );
    let mut paths = changed_paths.clone();
    for node in &graph.nodes {
        let links_to_touched = node
            .links
            .values()
            .flatten()
            .any(|target| ids.contains(id::base_of(target)));
        if links_to_touched {
            paths.insert(node.path.clone());
        }
    }
    let nodes = graph
        .nodes
        .iter()
        .filter(|n| paths.contains(&n.path) || ids.contains(&n.id))
        .count();
    Impact { paths, ids, nodes }
}

/// Whether a finding belongs to a node in scope. A finding counts when it is
/// in an in-scope file, or about an in-scope ID. The second rule keeps a
/// duplicate ID visible when the changed file is not the one it is reported on.
pub fn in_scope(finding: &Finding, impact: &Impact) -> bool {
    finding
        .path
        .as_ref()
        .is_some_and(|p| impact.paths.contains(p))
        || finding.id.as_ref().is_some_and(|i| impact.ids.contains(i))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::check;
    use crate::parse::parse_file_text;
    use std::path::Path;

    fn graph(files: &[(&str, &str)]) -> Graph {
        let nodes = files
            .iter()
            .filter_map(|(name, text)| parse_file_text(Path::new(name), text).0)
            .collect();
        Graph::from_nodes(nodes)
    }

    fn set(items: &[&str]) -> BTreeSet<PathBuf> {
        items.iter().map(PathBuf::from).collect()
    }

    fn ids(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    const A: &str = "---\nid: REQ-A-0001\ntype: requirement\n---\n";
    const B_LINKS_A: &str =
        "---\nid: REQ-A-0002\ntype: requirement\nlinks:\n  refines: REQ-A-0001\n---\n";
    const C: &str = "---\nid: REQ-A-0003\ntype: requirement\n---\n";

    // verifies: REQ-IMPACT-0001.S1
    #[test]
    fn a_changed_node_brings_in_the_nodes_that_link_to_it() {
        let g = graph(&[("a.md", A), ("b.md", B_LINKS_A), ("c.md", C)]);
        let i = impact(&g, &set(&["a.md"]), &ids(&[]));
        assert_eq!(i.paths, set(&["a.md", "b.md"]));
        assert_eq!(i.ids, ids(&["REQ-A-0001"]));
        assert_eq!(i.nodes, 2);
    }

    // verifies: REQ-IMPACT-0001.S2
    #[test]
    fn a_changed_referrer_does_not_bring_in_what_it_links_to() {
        let g = graph(&[("a.md", A), ("b.md", B_LINKS_A), ("c.md", C)]);
        let i = impact(&g, &set(&["b.md"]), &ids(&[]));
        assert_eq!(i.paths, set(&["b.md"]));
        assert_eq!(i.nodes, 1);
    }

    // verifies: REQ-IMPACT-0001.S3
    #[test]
    fn a_deleted_node_is_found_through_its_old_id() {
        // a.md is gone, but b.md still links to REQ-A-0001.
        let g = graph(&[("b.md", B_LINKS_A), ("c.md", C)]);
        let i = impact(&g, &set(&["a.md"]), &ids(&["REQ-A-0001"]));
        assert!(i.paths.contains(Path::new("b.md")));
        assert!(!i.paths.contains(Path::new("c.md")));
    }

    // verifies: REQ-IMPACT-0001.S4
    #[test]
    fn links_to_a_scenario_count_as_links_to_the_node() {
        let b = "---\nid: REQ-A-0002\ntype: requirement\nlinks:\n  refines: REQ-A-0001.S1\n---\n";
        let g = graph(&[("a.md", A), ("b.md", b)]);
        let i = impact(&g, &set(&["a.md"]), &ids(&[]));
        assert_eq!(i.paths, set(&["a.md", "b.md"]));
    }

    // verifies: REQ-IMPACT-0001.S5
    #[test]
    fn findings_are_kept_only_for_nodes_in_scope() {
        let broken = "---\nid: ADR-0009\ntype: adr\nlinks:\n  refines: NOPE-0001\n---\n";
        let g = graph(&[("a.md", A), ("b.md", B_LINKS_A), ("broken.md", broken)]);
        let findings = check(&g);
        assert_eq!(findings.len(), 3, "{findings:?}"); // two requirement nodes without a requirement block, and a dangling link
        let i = impact(&g, &set(&["a.md"]), &ids(&[]));
        let kept: Vec<_> = findings.iter().filter(|f| in_scope(f, &i)).collect();
        assert!(
            kept.iter()
                .all(|f| f.path.as_deref() != Some(Path::new("broken.md")))
        );
        assert!(
            kept.iter()
                .any(|f| f.path.as_deref() == Some(Path::new("a.md")))
        );
    }

    // verifies: REQ-IMPACT-0001.S6
    #[test]
    fn a_duplicate_id_is_kept_even_when_it_is_reported_on_the_other_file() {
        let g = graph(&[("a.md", A), ("z.md", A)]);
        let findings = check(&g);
        let duplicate = findings.iter().find(|f| f.code == "E001").unwrap();
        assert_eq!(duplicate.path.as_deref(), Some(Path::new("z.md")));
        // Only a.md changed, but the duplicate is about its ID.
        let i = impact(&g, &set(&["a.md"]), &ids(&[]));
        assert!(in_scope(duplicate, &i));
    }
}
