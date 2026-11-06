//! Compliance profiles: traceability and sign-offs a project must have.
//!
//! A rule says that every node of a type (in a status that counts) has a link
//! of some kind from or to a node of another type, or has been signed off. The
//! built-in standards turn the expectations that IEC 62304, ISO 26262 and
//! DO-178C share into such rules, in three tiers that grow with the safety
//! level. They help to collect evidence; they do not make a project compliant.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::{CONFIG_PATH, ComplianceConfig};
use crate::debt::day_number;
use crate::finding::Finding;
use crate::graph::Graph;
use crate::node::{KNOWN_LINK_KINDS, KNOWN_TYPES, Node};

/// A link a node must have: from a node of type `from` (an incoming link) or
/// to a node of type `to` (an outgoing one), of kind `link`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Needs {
    pub link: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: String,
    /// The statuses the rule applies to. Empty: every status past draft.
    #[serde(default, rename = "status", skip_serializing_if = "Vec::is_empty")]
    pub statuses: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs: Option<Needs>,
    /// The node has `approved_by` and `approved_on`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub signoff: bool,
}

/// Statuses of nodes that are not yet part of what is promised.
const NOT_YET: &[&str] = &[
    "draft",
    "proposed",
    "rejected",
    "deprecated",
    "superseded",
    "withdrawn",
    "obsolete",
];

pub struct Standard {
    pub name: &'static str,
    pub title: &'static str,
    /// Level names and the tier each one asks for.
    pub levels: &'static [(&'static str, u8)],
}

pub const STANDARDS: &[Standard] = &[
    Standard {
        name: "iec-62304",
        title: "IEC 62304, medical device software: safety classes A to C",
        levels: &[("A", 1), ("B", 2), ("C", 3)],
    },
    Standard {
        name: "iso-26262",
        title: "ISO 26262, road vehicles: ASIL A to D",
        levels: &[("A", 1), ("B", 1), ("C", 2), ("D", 3)],
    },
    Standard {
        name: "do-178c",
        title: "DO-178C, airborne software: design assurance levels E to A",
        levels: &[("E", 0), ("D", 1), ("C", 1), ("B", 2), ("A", 3)],
    },
];

fn trace(id: &str, ty: &str, link: &str, from: &str) -> Rule {
    Rule {
        id: id.into(),
        ty: ty.into(),
        statuses: Vec::new(),
        needs: Some(Needs {
            link: link.into(),
            from: Some(from.into()),
            to: None,
        }),
        signoff: false,
    }
}

fn signed(id: &str, ty: &str) -> Rule {
    Rule {
        id: id.into(),
        ty: ty.into(),
        statuses: Vec::new(),
        needs: None,
        signoff: true,
    }
}

// spec: REQ-COMPLIANCE-0001
/// The rules of a tier: each tier has those of the one below.
pub fn tier_rules(tier: u8) -> Vec<Rule> {
    let mut rules = Vec::new();
    if tier >= 1 {
        rules.push(trace(
            "requirement-verified",
            "requirement",
            "verifies",
            "test-case",
        ));
        rules.push(signed("requirement-signed-off", "requirement"));
    }
    if tier >= 2 {
        rules.push(trace(
            "requirement-allocated",
            "requirement",
            "satisfies",
            "component",
        ));
        rules.push(signed("adr-signed-off", "adr"));
    }
    if tier >= 3 {
        rules.push(signed("test-signed-off", "test-case"));
    }
    rules
}

fn find_standard(name: &str) -> Option<&'static Standard> {
    STANDARDS.iter().find(|s| s.name.eq_ignore_ascii_case(name))
}

/// The rules a config asks for, and what is wrong with it.
pub fn resolve(config: &ComplianceConfig) -> (Vec<Rule>, Vec<String>) {
    let mut rules = Vec::new();
    let mut problems = Vec::new();
    match (&config.standard, &config.level) {
        (None, None) => {}
        (None, Some(_)) => problems.push("`level` needs a `standard`".to_string()),
        (Some(name), level) => match find_standard(name) {
            None => problems.push(format!(
                "unknown standard `{name}`; the standards are {}",
                STANDARDS
                    .iter()
                    .map(|s| s.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            Some(standard) => {
                let names = standard.levels.iter().map(|l| l.0).collect::<Vec<_>>();
                match level.as_deref().and_then(|l| {
                    standard
                        .levels
                        .iter()
                        .find(|(n, _)| n.eq_ignore_ascii_case(l))
                }) {
                    Some((_, tier)) => rules.extend(tier_rules(*tier)),
                    None => problems.push(format!(
                        "{} needs a `level`: {}",
                        standard.name,
                        names.join(", ")
                    )),
                }
            }
        },
    }
    for rule in &config.rules {
        if let Some(why) = invalid(rule, &rules) {
            problems.push(format!("rule `{}` {why}", rule.id));
        } else {
            rules.push(rule.clone());
        }
    }
    (rules, problems)
}

fn invalid(rule: &Rule, taken: &[Rule]) -> Option<String> {
    if rule.id.trim().is_empty() {
        return Some("has no id".into());
    }
    if taken.iter().any(|r| r.id == rule.id) {
        return Some("repeats the id of another rule".into());
    }
    if !KNOWN_TYPES.contains(&rule.ty.as_str()) {
        return Some(format!("is for the unknown type `{}`", rule.ty));
    }
    match (&rule.needs, rule.signoff) {
        (None, false) => Some("needs either `needs` or `signoff`".into()),
        (Some(_), true) => Some("has both `needs` and `signoff`".into()),
        (None, true) => None,
        (Some(n), false) => {
            if !KNOWN_LINK_KINDS.contains(&n.link.as_str()) {
                return Some(format!("uses the unknown link kind `{}`", n.link));
            }
            let other = match (&n.from, &n.to) {
                (Some(t), None) | (None, Some(t)) => t,
                _ => return Some("needs exactly one of `from` and `to`".into()),
            };
            (!KNOWN_TYPES.contains(&other.as_str()))
                .then(|| format!("names the unknown type `{other}`"))
        }
    }
}

fn applies(rule: &Rule, node: &Node) -> bool {
    if node.ty != rule.ty {
        return false;
    }
    match (node.status.as_deref(), rule.statuses.is_empty()) {
        (Some(s), true) => !NOT_YET.contains(&s),
        (Some(s), false) => rule.statuses.iter().any(|x| x == s),
        (None, _) => false,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Gap {
    pub id: String,
    pub path: PathBuf,
    pub problem: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleReport {
    pub rule: Rule,
    pub statement: String,
    pub applies: usize,
    pub met: usize,
    pub gaps: Vec<Gap>,
}

fn statement(rule: &Rule) -> String {
    let who = if rule.statuses.is_empty() {
        format!("Every {} past draft", rule.ty)
    } else {
        format!("Every {} that is {}", rule.ty, rule.statuses.join(" or "))
    };
    match &rule.needs {
        Some(Needs {
            link,
            from: Some(t),
            ..
        }) => format!("{who} has a `{link}` link from a {t}"),
        Some(Needs {
            link, to: Some(t), ..
        }) => format!("{who} has a `{link}` link to a {t}"),
        _ => format!("{who} has `approved_by` and `approved_on`"),
    }
}

fn signoff_gap(node: &Node) -> Option<String> {
    let by = node
        .approved_by
        .as_deref()
        .is_some_and(|b| !b.trim().is_empty());
    let on = node.approved_on.as_deref();
    match (by, on) {
        (true, Some(d)) if day_number(d).is_some() => None,
        (true, Some(d)) => Some(format!("`approved_on` {d:?} is not a date as YYYY-MM-DD")),
        (true, None) => Some("has no `approved_on`".into()),
        (false, Some(_)) => Some("has no `approved_by`".into()),
        (false, None) => Some("has not been signed off".into()),
    }
}

/// How each rule fares in the graph.
pub fn report(graph: &Graph) -> Vec<RuleReport> {
    let (rules, _) = resolve(graph.compliance_config());
    if rules.is_empty() {
        return Vec::new();
    }
    let mut incoming: HashMap<&str, Vec<(&Node, &str)>> = HashMap::new();
    for n in &graph.nodes {
        for (kind, targets) in &n.links {
            for t in targets {
                incoming
                    .entry(crate::id::base_of(t))
                    .or_default()
                    .push((n, kind.as_str()));
            }
        }
    }
    rules
        .into_iter()
        .map(|rule| {
            let mut out = RuleReport {
                statement: statement(&rule),
                applies: 0,
                met: 0,
                gaps: Vec::new(),
                rule: rule.clone(),
            };
            for node in graph.nodes.iter().filter(|n| applies(&rule, n)) {
                out.applies += 1;
                let problem = match &rule.needs {
                    None => signoff_gap(node),
                    Some(Needs {
                        link,
                        from: Some(t),
                        ..
                    }) => {
                        let found = incoming
                            .get(node.id.as_str())
                            .is_some_and(|v| v.iter().any(|(n, k)| *k == link && n.ty == *t));
                        (!found).then(|| format!("has no `{link}` link from a {t}"))
                    }
                    Some(Needs {
                        link, to: Some(t), ..
                    }) => {
                        let found = node.links.get(link).is_some_and(|targets| {
                            targets
                                .iter()
                                .any(|x| graph.get(x).is_some_and(|n| n.ty == *t))
                        });
                        (!found).then(|| format!("has no `{link}` link to a {t}"))
                    }
                    Some(_) => None,
                };
                match problem {
                    None => out.met += 1,
                    Some(problem) => out.gaps.push(Gap {
                        id: node.id.clone(),
                        path: node.path.clone(),
                        problem,
                    }),
                }
            }
            out
        })
        .collect()
}

// spec: REQ-COMPLIANCE-0002
// spec: REQ-COMPLIANCE-0003
/// E035 for a missing link, E036 for a missing sign-off, E037 for a bad config.
pub fn check(graph: &Graph) -> Vec<Finding> {
    let mut out = Vec::new();
    let (_, problems) = resolve(graph.compliance_config());
    for p in problems {
        out.push(
            Finding::error("E037", format!("compliance: {p}"))
                .at(std::path::Path::new(CONFIG_PATH), None),
        );
    }
    for r in report(graph) {
        let code = if r.rule.signoff { "E036" } else { "E035" };
        for gap in r.gaps {
            out.push(
                Finding::error(
                    code,
                    format!("{} ({}): {}", r.rule.id, r.statement, gap.problem),
                )
                .at(&gap.path, Some(1))
                .node(&gap.id),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests_of_compliance {
    use super::*;

    fn graph(files: &[(&str, &str)], config: ComplianceConfig) -> Graph {
        Graph::from_sources(
            files
                .iter()
                .map(|(p, t)| (PathBuf::from(p), t.as_bytes().to_vec())),
        )
        .with_compliance(config)
    }

    fn standard(name: &str, level: &str) -> ComplianceConfig {
        ComplianceConfig {
            standard: Some(name.into()),
            level: Some(level.into()),
            rules: Vec::new(),
        }
    }

    const REQ: &str = "---\nid: REQ-A-0001\ntype: requirement\nstatus: accepted\n---\n### Requirement: X\nThe system SHALL x.\n#### Scenario: s {#S1}\n- **WHEN** a\n- **THEN** b\n";

    // verifies: REQ-COMPLIANCE-0001.S1
    #[test]
    fn tiers_grow_with_the_level() {
        assert_eq!(tier_rules(0).len(), 0);
        assert_eq!(tier_rules(1).len(), 2);
        assert_eq!(tier_rules(2).len(), 4);
        assert_eq!(tier_rules(3).len(), 5);
    }

    // verifies: REQ-COMPLIANCE-0002.S1
    #[test]
    fn a_requirement_without_a_test_or_sign_off_is_reported() {
        let g = graph(&[("r.md", REQ)], standard("iec-62304", "A"));
        let codes: Vec<_> = check(&g).iter().map(|f| f.code).collect();
        assert_eq!(codes, vec!["E035", "E036"]);
    }

    // verifies: REQ-COMPLIANCE-0002.S2
    #[test]
    fn a_test_on_a_scenario_and_a_sign_off_meet_the_rules() {
        let signed = REQ.replace(
            "status: accepted\n",
            "status: accepted\napproved_by: Ann Lee\napproved_on: \"2026-10-28\"\n",
        );
        let test = "---\nid: TC-A-0001\ntype: test-case\nlinks:\n  verifies:\n    - REQ-A-0001.S1\n---\n# t\n";
        let g = graph(
            &[("r.md", &signed), ("t.md", test)],
            standard("iec-62304", "A"),
        );
        assert!(check(&g).is_empty(), "{:?}", check(&g));
    }

    // verifies: REQ-COMPLIANCE-0002.S3
    #[test]
    fn a_draft_is_left_alone() {
        let g = graph(
            &[("r.md", &REQ.replace("accepted", "draft"))],
            standard("iec-62304", "A"),
        );
        assert!(check(&g).is_empty());
    }

    // verifies: REQ-COMPLIANCE-0003.S1
    #[test]
    fn a_bad_config_is_e037() {
        let g = graph(&[], standard("iec-62304", "Z"));
        assert_eq!(check(&g)[0].code, "E037");
        let g = graph(&[], standard("nope", "A"));
        assert_eq!(check(&g)[0].code, "E037");
    }

    // verifies: REQ-COMPLIANCE-0003.S2
    #[test]
    fn own_rules_are_checked_and_can_name_a_status() {
        let mut cfg = ComplianceConfig::default();
        cfg.rules.push(Rule {
            id: "decided".into(),
            ty: "requirement".into(),
            statuses: vec!["accepted".into()],
            needs: Some(Needs {
                link: "decided-by".into(),
                from: None,
                to: Some("adr".into()),
            }),
            signoff: false,
        });
        let g = graph(&[("r.md", REQ)], cfg.clone());
        assert_eq!(check(&g)[0].code, "E035");
        cfg.rules[0].needs = None;
        let g = graph(&[("r.md", REQ)], cfg);
        assert_eq!(check(&g)[0].code, "E037");
    }
}
