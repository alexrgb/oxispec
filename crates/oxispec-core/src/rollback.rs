//! Planning a rollback: put nodes back to how they were at an earlier
//! revision. This module only decides what to do. Reading old files from git
//! and writing the working tree is done by the command line.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Serialize;

use crate::diff::{Change, diff_graphs, diff_nodes};
use crate::finding::{Finding, Severity};
use crate::graph::Graph;

/// What happens to one node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// The node exists now and at the revision, and is different. Its file is
    /// put back as it was, at the path it had then.
    Restore {
        id: String,
        /// Where the file is now.
        from: PathBuf,
        /// Where it was at the revision.
        to: PathBuf,
        /// What the rollback undoes, seen from now: the changes from the
        /// current node to the old one.
        changes: Vec<Change>,
    },
    /// The node was deleted since the revision. Its file comes back.
    Recreate { id: String, path: PathBuf },
    /// The node did not exist at the revision. Its file is deleted.
    Remove { id: String, path: PathBuf },
    /// The node is the same as it was. Nothing to do.
    Unchanged { id: String },
}

impl Action {
    pub fn id(&self) -> &str {
        match self {
            Action::Restore { id, .. }
            | Action::Recreate { id, .. }
            | Action::Remove { id, .. }
            | Action::Unchanged { id } => id,
        }
    }

    pub fn changes_the_tree(&self) -> bool {
        !matches!(self, Action::Unchanged { .. })
    }

    /// One line for people.
    pub fn describe(&self) -> String {
        match self {
            Action::Restore { id, changes, .. } => {
                let what: Vec<String> = changes.iter().map(Change::describe).collect();
                format!("restore {id} ({})", what.join("; "))
            }
            Action::Recreate { id, path } => {
                format!("recreate {id} ({} did not survive)", path.display())
            }
            Action::Remove { id, path } => {
                format!(
                    "remove {id} ({} did not exist at that revision)",
                    path.display()
                )
            }
            Action::Unchanged { id } => format!("keep {id} (already the same)"),
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RollbackError {
    #[error("no node {0} at that revision or now")]
    UnknownNode(String),
}

/// The nodes that differ between now and the revision: added, removed or changed.
pub fn differing_ids(current: &Graph, target: &Graph) -> Vec<String> {
    // `diff_graphs` goes from old to new, and the old state here is the target.
    let diff = diff_graphs(target, current);
    let mut ids: BTreeSet<String> = BTreeSet::new();
    ids.extend(diff.added.into_iter().map(|n| n.id));
    ids.extend(diff.removed.into_iter().map(|n| n.id));
    ids.extend(diff.changed.into_iter().map(|n| n.id));
    ids.into_iter().collect()
}

/// What to do for each node in `ids`, in the order given.
pub fn plan(current: &Graph, target: &Graph, ids: &[String]) -> Result<Vec<Action>, RollbackError> {
    let mut actions = Vec::new();
    let mut seen = BTreeSet::new();
    for given in ids {
        let (now, then) = (current.get(given), target.get(given));
        let id = match (now, then) {
            (Some(node), _) | (None, Some(node)) => node.id.clone(),
            (None, None) => return Err(RollbackError::UnknownNode(given.clone())),
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        actions.push(match (now, then) {
            (Some(now), Some(then)) => {
                let changes = diff_nodes(now, then);
                if changes.is_empty() {
                    Action::Unchanged { id }
                } else {
                    Action::Restore {
                        id,
                        from: now.path.clone(),
                        to: then.path.clone(),
                        changes,
                    }
                }
            }
            (None, Some(then)) => Action::Recreate {
                id,
                path: then.path.clone(),
            },
            (Some(now), None) => Action::Remove {
                id,
                path: now.path.clone(),
            },
            (None, None) => unreachable!("handled above"),
        });
    }
    Ok(actions)
}

/// The graph as it would be after the actions: the current nodes, with the
/// restored ones swapped for their old versions.
pub fn would_be(current: &Graph, target: &Graph, actions: &[Action]) -> Graph {
    let touched: BTreeSet<&str> = actions
        .iter()
        .filter(|a| a.changes_the_tree())
        .map(Action::id)
        .collect();
    let mut nodes: Vec<_> = current
        .nodes
        .iter()
        .filter(|n| !touched.contains(n.id.as_str()))
        .cloned()
        .collect();
    for action in actions {
        if matches!(action, Action::Restore { .. } | Action::Recreate { .. })
            && let Some(old) = target.get(action.id())
        {
            nodes.push(old.clone());
        }
    }
    Graph::from_nodes(nodes)
}

/// Errors that the actions would introduce: they are in the would-be graph
/// and not in the current one. Warnings are not counted.
pub fn new_errors(current: &Graph, would_be: &Graph) -> Vec<Finding> {
    diff_graphs(current, would_be)
        .new_findings
        .into_iter()
        .filter(|f| f.severity == Severity::Error)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_file_text;
    use std::path::Path;

    const BODY: &str = "### Requirement: X\nThe system SHALL work.\n\n#### Scenario: S {#S1}\n- **WHEN** a\n- **THEN** b\n";

    fn text(id: &str, extra: &str) -> String {
        format!("---\nid: {id}\ntype: requirement\n{extra}---\n{BODY}")
    }

    fn graph(files: &[(&str, String)]) -> Graph {
        Graph::from_nodes(
            files
                .iter()
                .filter_map(|(p, t)| parse_file_text(Path::new(p), t).0)
                .collect(),
        )
    }

    fn ids(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn each_kind_of_difference_gets_its_action() {
        let target = graph(&[
            ("a.md", text("REQ-A-0001", "status: draft\n")),
            ("gone.md", text("REQ-A-0002", "")),
            ("same.md", text("REQ-A-0004", "")),
        ]);
        let current = graph(&[
            ("moved/a.md", text("REQ-A-0001", "status: accepted\n")),
            ("new.md", text("REQ-A-0003", "")),
            ("same.md", text("REQ-A-0004", "")),
        ]);
        let all = differing_ids(&current, &target);
        assert_eq!(all, ids(&["REQ-A-0001", "REQ-A-0002", "REQ-A-0003"]));

        let actions = plan(
            &current,
            &target,
            &ids(&["REQ-A-0001", "REQ-A-0002", "REQ-A-0003", "REQ-A-0004"]),
        )
        .unwrap();
        match &actions[0] {
            Action::Restore {
                from, to, changes, ..
            } => {
                assert_eq!(from, Path::new("moved/a.md"));
                assert_eq!(to, Path::new("a.md"));
                let described: Vec<String> = changes.iter().map(Change::describe).collect();
                assert!(described.contains(&"status: accepted -> draft".to_string()));
                assert!(described.contains(&"moved: moved/a.md -> a.md".to_string()));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            actions[1],
            Action::Recreate {
                id: "REQ-A-0002".into(),
                path: "gone.md".into()
            }
        );
        assert_eq!(
            actions[2],
            Action::Remove {
                id: "REQ-A-0003".into(),
                path: "new.md".into()
            }
        );
        assert_eq!(
            actions[3],
            Action::Unchanged {
                id: "REQ-A-0004".into()
            }
        );
        assert!(!actions[3].changes_the_tree());
    }

    #[test]
    fn an_unknown_node_is_an_error_and_repeats_are_ignored() {
        let g = graph(&[("a.md", text("REQ-A-0001", ""))]);
        assert_eq!(
            plan(&g, &g, &ids(&["REQ-NOPE-0001"])).unwrap_err(),
            RollbackError::UnknownNode("REQ-NOPE-0001".into())
        );
        let actions = plan(&g, &g, &ids(&["REQ-A-0001", "REQ-A-0001.S1"])).unwrap();
        assert_eq!(actions.len(), 1);
    }

    #[test]
    fn the_would_be_graph_swaps_restored_nodes_and_drops_removed_ones() {
        let target = graph(&[
            ("a.md", text("REQ-A-0001", "status: draft\n")),
            ("gone.md", text("REQ-A-0002", "")),
        ]);
        let current = graph(&[
            ("a.md", text("REQ-A-0001", "status: accepted\n")),
            ("new.md", text("REQ-A-0003", "")),
        ]);
        let actions = plan(
            &current,
            &target,
            &ids(&["REQ-A-0001", "REQ-A-0002", "REQ-A-0003"]),
        )
        .unwrap();
        let after = would_be(&current, &target, &actions);
        let found: Vec<(&str, Option<&str>)> = after
            .nodes
            .iter()
            .map(|n| (n.id.as_str(), n.status.as_deref()))
            .collect();
        assert_eq!(
            found,
            vec![("REQ-A-0001", Some("draft")), ("REQ-A-0002", None)]
        );
    }

    #[test]
    fn removing_a_node_that_others_link_to_is_a_new_error() {
        let target = graph(&[("a.md", text("REQ-A-0001", ""))]);
        let current = graph(&[
            ("a.md", text("REQ-A-0001", "")),
            ("b.md", text("REQ-A-0002", "")),
            (
                "c.md",
                text("REQ-A-0003", "links:\n  refines: REQ-A-0002\n"),
            ),
        ]);
        let actions = plan(&current, &target, &ids(&["REQ-A-0002"])).unwrap();
        let after = would_be(&current, &target, &actions);
        let errors = new_errors(&current, &after);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "E002");
    }

    #[test]
    fn a_rollback_that_changes_nothing_adds_no_errors() {
        let g = graph(&[("a.md", text("REQ-A-0001", ""))]);
        let actions = plan(&g, &g, &ids(&["REQ-A-0001"])).unwrap();
        assert!(new_errors(&g, &would_be(&g, &g, &actions)).is_empty());
    }

    #[test]
    fn actions_describe_themselves() {
        let target = graph(&[("a.md", text("REQ-A-0001", "status: draft\n"))]);
        let current = graph(&[("a.md", text("REQ-A-0001", "status: accepted\n"))]);
        let actions = plan(&current, &target, &ids(&["REQ-A-0001"])).unwrap();
        assert_eq!(
            actions[0].describe(),
            "restore REQ-A-0001 (status: accepted -> draft)"
        );
    }
}
