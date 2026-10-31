//! A one-screen summary of a project: what is in it and what needs attention.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::finding::{Finding, Severity};
use crate::graph::Graph;

#[derive(Debug, Serialize, PartialEq)]
pub struct Summary {
    pub nodes: usize,
    /// Node type to count.
    pub types: BTreeMap<String, usize>,
    /// Node type to status to count, for nodes that have a status.
    pub statuses: BTreeMap<String, BTreeMap<String, usize>>,
    pub errors: usize,
    pub warnings: usize,
    /// Finding code to count, errors and warnings only.
    pub codes: BTreeMap<String, usize>,
    /// Change sets that are not merged or reverted yet.
    pub changes_in_flight: Vec<String>,
    pub requirements: usize,
    pub requirements_with_claims: usize,
    pub scenarios: usize,
    /// Links from nodes, and links nothing resolves.
    pub links: usize,
    pub latest_release: Option<String>,
}

pub fn summarize(graph: &Graph, findings: &[Finding]) -> Summary {
    let mut types: BTreeMap<String, usize> = BTreeMap::new();
    let mut statuses: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut changes_in_flight = Vec::new();
    let (mut requirements, mut with_claims, mut scenarios, mut links) = (0, 0, 0, 0);
    let mut releases: Vec<(&str, &str)> = Vec::new();

    for node in &graph.nodes {
        *types.entry(node.ty.clone()).or_default() += 1;
        if let Some(status) = &node.status {
            *statuses
                .entry(node.ty.clone())
                .or_default()
                .entry(status.clone())
                .or_default() += 1;
        }
        links += node.links.values().map(Vec::len).sum::<usize>();
        match node.ty.as_str() {
            "requirement" => {
                requirements += 1;
                if !node.claims.is_empty() {
                    with_claims += 1;
                }
                scenarios += node
                    .requirements
                    .iter()
                    .map(|r| r.scenarios.len())
                    .sum::<usize>();
            }
            "change" => {
                if !matches!(node.status.as_deref(), Some("merged" | "reverted")) {
                    changes_in_flight.push(node.id.clone());
                }
            }
            "release" => {
                if let Some(version) = &node.version {
                    releases.push((version, &node.id));
                }
            }
            _ => {}
        }
    }
    changes_in_flight.sort();

    let mut codes: BTreeMap<String, usize> = BTreeMap::new();
    let (mut errors, mut warnings) = (0, 0);
    for f in findings {
        match f.severity {
            Severity::Error => errors += 1,
            Severity::Warning => warnings += 1,
            Severity::Info => continue,
        }
        *codes.entry(f.code.to_string()).or_default() += 1;
    }

    // The highest version, compared as numbers where they parse.
    let key = |v: &str| -> Vec<u64> { v.split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    let latest_release = releases
        .iter()
        .max_by_key(|(v, _)| key(v))
        .map(|(v, _)| v.to_string());

    Summary {
        nodes: graph.nodes.len(),
        types,
        statuses,
        errors,
        warnings,
        codes,
        changes_in_flight,
        requirements,
        requirements_with_claims: with_claims,
        scenarios,
        links,
        latest_release,
    }
}

#[cfg(test)]
mod tests_of_status {
    use super::*;
    use crate::check::check;

    fn file(id: &str, ty: &str, extra: &str) -> (std::path::PathBuf, Vec<u8>) {
        let text = format!("---\nid: {id}\ntype: {ty}\n{extra}---\n\n# {id}: title\n");
        (format!("specs/{id}.md").into(), text.into_bytes())
    }

    #[test]
    fn counts_types_statuses_and_changes_in_flight() {
        let graph = Graph::from_sources([
            file("REQ-A-0001", "requirement", "status: active\n"),
            file("REQ-A-0002", "requirement", "status: draft\n"),
            file("CHG-0001", "change", "status: draft\n"),
            file("CHG-0002", "change", "status: merged\n"),
            file("REL-0001", "release", "version: 0.10.0\n"),
            file("REL-0002", "release", "version: 0.9.0\n"),
        ]);
        let s = summarize(&graph, &check(&graph));
        assert_eq!(s.nodes, 6);
        assert_eq!(s.types["requirement"], 2);
        assert_eq!(s.statuses["requirement"]["draft"], 1);
        assert_eq!(s.changes_in_flight, ["CHG-0001"]);
        assert_eq!(s.latest_release.as_deref(), Some("0.10.0"));
        assert_eq!(s.errors + s.warnings, s.codes.values().sum::<usize>());
    }

    #[test]
    fn an_empty_project_has_nothing_to_report() {
        let s = summarize(&Graph::from_nodes(Vec::new()), &[]);
        assert_eq!(s.nodes, 0);
        assert!(s.latest_release.is_none());
    }
}
