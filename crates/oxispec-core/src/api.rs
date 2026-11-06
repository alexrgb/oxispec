//! An OpenAPI description made from the requirements, and a check of a
//! description kept elsewhere against them.
//!
//! A requirement block whose title is a call, such as `GET /orders/{id}`, is
//! an operation. Each scenario whose THEN says `responds with 200` is a
//! response. The requirements know which operations exist and what each
//! answers; they do not know schemas or security, so the skeleton has none and
//! says so. A description kept in the code stays the source of truth for
//! those, and [`drift`] shows where it and the requirements part.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::graph::Graph;
use crate::import::sources::{METHODS, parse_description};
use crate::import::{sections, text_of};

/// One operation as the requirements state it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub method: String,
    pub path: String,
    /// The domain of the requirement's ID, in lower case.
    pub tag: String,
    pub summary: String,
    /// Status code to its description.
    pub responses: BTreeMap<String, String>,
    /// The requirement node that states it.
    pub node: String,
    /// Its scenarios as written: a name and the steps, like `WHEN a user logs in`.
    pub scenarios: Vec<(String, Vec<String>)>,
}

impl Operation {
    pub fn call(&self) -> String {
        format!("{} {}", self.method.to_ascii_uppercase(), self.path)
    }
}

/// `GET /orders/{id}` as a method (lower case) and a path.
fn parse_call(title: &str) -> Option<(String, String)> {
    let (m, p) = title.trim().split_once(' ')?;
    let m = m.to_ascii_lowercase();
    let p = p.trim();
    (METHODS.contains(&m.as_str()) && p.starts_with('/') && !p.contains(' '))
        .then(|| (m, p.to_string()))
}

fn tag_of(id: &str) -> String {
    let rest = id.strip_prefix("REQ-").unwrap_or(id);
    rest.rsplit_once('-')
        .map_or(rest, |(domain, _)| domain)
        .to_ascii_lowercase()
}

/// The text after `provide `GET /x`:` in a statement, or the statement.
fn summary_of(statement: &str) -> String {
    let one: String = statement.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = match one.split_once("`: ") {
        Some((_, rest)) => rest.to_string(),
        None => one,
    };
    text.trim().trim_end_matches('.').to_string()
}

/// `the API responds with 404: No such order` as `("404", "No such order")`.
fn response_of(then: &str) -> Option<(String, String)> {
    let rest = then.split_once("responds with ")?.1.trim();
    let (code, what) = match rest.split_once(':') {
        Some((c, w)) => (c.trim(), w.trim()),
        None => (rest.split_whitespace().next()?, ""),
    };
    let ok =
        code.len() == 3 && code.bytes().all(|b| b.is_ascii_alphanumeric()) || code == "default";
    ok.then(|| (code.to_string(), what.to_string()))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Unreadable(String),
    NotADescription(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable(why) => write!(f, "cannot read {why}"),
            Self::NotADescription(p) => write!(f, "{p} is not an OpenAPI or Swagger description"),
        }
    }
}

impl std::error::Error for Error {}

/// The operations the requirements state, in path then method order.
// spec: REQ-API-0001
pub fn operations(graph: &Graph, root: &Path) -> Result<Vec<Operation>, Error> {
    let mut out: BTreeMap<(String, usize, String), Operation> = BTreeMap::new();
    for node in graph.nodes.iter().filter(|n| n.ty == "requirement") {
        let text = fs::read_to_string(root.join(&node.path))
            .map_err(|e| Error::Unreadable(format!("{}: {e}", node.path.display())))?
            .replace("\r\n", "\n");
        let (_, secs) = sections(&text);
        let mut current: Option<Operation> = None;
        let mut flush = |op: Option<Operation>| {
            if let Some(op) = op {
                let order = METHODS.iter().position(|m| *m == op.method).unwrap_or(0);
                out.entry((op.path.clone(), order, op.node.clone()))
                    .or_insert(op);
            }
        };
        for s in &secs {
            if let Some(name) = s.title.strip_prefix("Requirement:") {
                flush(current.take());
                current = parse_call(name).map(|(method, path)| Operation {
                    method,
                    path,
                    tag: tag_of(&node.id),
                    summary: summary_of(&text_of(&s.body)),
                    responses: BTreeMap::new(),
                    node: node.id.clone(),
                    scenarios: Vec::new(),
                });
            } else if let Some(scenario) = s.title.strip_prefix("Scenario:")
                && let Some(op) = current.as_mut()
            {
                let scenario = match scenario.rfind("{#") {
                    Some(i) => scenario[..i].trim(),
                    None => scenario.trim(),
                };
                let mut steps = Vec::new();
                for line in &s.body {
                    let step = line.trim().trim_start_matches("- ").replace("**", "");
                    if !step.trim().is_empty() {
                        steps.push(step.trim().to_string());
                    }
                    if let Some(then) = step.strip_prefix("THEN ")
                        && let Some((code, what)) = response_of(then)
                    {
                        op.responses.entry(code).or_insert(what);
                    }
                }
                op.scenarios.push((scenario.to_string(), steps));
            }
        }
        flush(current.take());
    }
    // One operation per call; the first requirement (by ID) wins.
    let mut seen = BTreeSet::new();
    let mut ops: Vec<Operation> = out.into_values().collect();
    ops.retain(|o| seen.insert((o.method.clone(), o.path.clone())));
    Ok(ops)
}

fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

fn path_params(path: &str) -> Vec<&str> {
    path.split('/')
        .filter_map(|seg| seg.strip_prefix('{')?.strip_suffix('}'))
        .collect()
}

/// An OpenAPI 3 skeleton in YAML. No schemas and no security: the
/// requirements do not state them.
pub fn render(ops: &[Operation], title: &str, version: &str) -> String {
    let mut out = format!(
        "# Made by `oxispec docs openapi` from the requirements. Do not edit.\n# It has no schemas or security; those are not in the requirements.\nopenapi: 3.0.3\ninfo:\n  title: {}\n  version: {}\npaths:",
        q(title),
        q(version)
    );
    if ops.is_empty() {
        out.push_str(" {}\n");
        return out;
    }
    out.push('\n');
    let mut by_path: BTreeMap<&str, Vec<&Operation>> = BTreeMap::new();
    for o in ops {
        by_path.entry(&o.path).or_default().push(o);
    }
    for (path, list) in by_path {
        out.push_str(&format!("  {}:\n", q(path)));
        for o in list {
            out.push_str(&format!("    {}:\n", o.method));
            out.push_str(&format!("      tags: [{}]\n", q(&o.tag)));
            out.push_str(&format!("      summary: {}\n", q(&o.summary)));
            out.push_str(&format!("      x-oxispec-requirement: {}\n", q(&o.node)));
            let params = path_params(path);
            if !params.is_empty() {
                out.push_str("      parameters:\n");
                for p in params {
                    out.push_str(&format!(
                        "        - name: {}\n          in: path\n          required: true\n          schema: {{ type: string }}\n",
                        q(p)
                    ));
                }
            }
            out.push_str("      responses:\n");
            if o.responses.is_empty() {
                out.push_str(
                    "        default:\n          description: \"Not stated in the requirement\"\n",
                );
            }
            for (code, what) in &o.responses {
                let what = if what.is_empty() { "Response" } else { what };
                out.push_str(&format!(
                    "        {}:\n          description: {}\n",
                    q(code),
                    q(what)
                ));
            }
        }
    }
    out
}

/// Where a description and the requirements part.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Drift {
    /// Stated by a requirement, absent from the description.
    pub only_in_requirements: Vec<String>,
    /// In the description, with no requirement.
    pub only_in_description: Vec<String>,
    /// Calls on both sides whose response codes differ: the call, the codes
    /// only the requirements state, the codes only the description has.
    pub codes: Vec<(String, Vec<String>, Vec<String>)>,
}

impl Drift {
    pub fn is_empty(&self) -> bool {
        self.only_in_requirements.is_empty()
            && self.only_in_description.is_empty()
            && self.codes.is_empty()
    }
}

/// Reads an OpenAPI or Swagger description from text: JSON, else YAML.
// spec: REQ-API-0005
pub fn parse(text: &str, name: &str) -> Result<Value, Error> {
    let value: Option<Value> = serde_json::from_str(text)
        .ok()
        .or_else(|| serde_saphyr::from_str(text).ok());
    value
        .filter(|v| v.get("openapi").is_some() || v.get("swagger").is_some())
        .ok_or_else(|| Error::NotADescription(name.to_string()))
}

/// Reads an OpenAPI or Swagger file (JSON or YAML).
pub fn load(path: &Path) -> Result<Value, Error> {
    let text = fs::read_to_string(path)
        .map_err(|e| Error::Unreadable(format!("{}: {e}", path.display())))?;
    parse_description(path, &text).ok_or_else(|| Error::NotADescription(path.display().to_string()))
}

fn described(doc: &Value) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    if let Some(paths) = doc.get("paths").and_then(Value::as_object) {
        for (path, item) in paths {
            for m in METHODS {
                if let Some(op) = item.get(*m) {
                    let codes = op
                        .get("responses")
                        .and_then(Value::as_object)
                        .map(|r| r.keys().cloned().collect())
                        .unwrap_or_default();
                    out.insert(format!("{} {path}", m.to_ascii_uppercase()), codes);
                }
            }
        }
    }
    out
}

/// Compares the requirements' operations with a description.
// spec: REQ-API-0002
pub fn drift(ops: &[Operation], doc: &Value) -> Drift {
    let file = described(doc);
    let mut d = Drift::default();
    let mut stated = BTreeSet::new();
    for o in ops {
        let call = o.call();
        stated.insert(call.clone());
        match file.get(&call) {
            None => d.only_in_requirements.push(call),
            Some(codes) => {
                let want: BTreeSet<String> = o.responses.keys().cloned().collect();
                let missing: Vec<String> = want.difference(codes).cloned().collect();
                let extra: Vec<String> = codes.difference(&want).cloned().collect();
                if !missing.is_empty() || !extra.is_empty() {
                    d.codes.push((call, missing, extra));
                }
            }
        }
    }
    d.only_in_description = file
        .keys()
        .filter(|c| !stated.contains(*c))
        .cloned()
        .collect();
    d
}

/// Everything the project says about one operation.
#[derive(Debug)]
pub struct Card<'a> {
    pub operation: &'a Operation,
    pub status: Option<String>,
    /// The scenarios of the operation's block, like `REQ-ORDERS-0001.S1`.
    pub scenarios: Vec<String>,
    /// Product requirements documents the requirement refines.
    pub prds: Vec<String>,
    /// Decisions it was made by (`decided-by`).
    pub decisions: Vec<String>,
    /// Tech debt that arises from it.
    pub debt: Vec<String>,
    /// Risks that arise from it or that it mitigates, with their level.
    pub risks: Vec<(String, Option<&'static str>)>,
    /// Test plans that cover it.
    pub plans: Vec<String>,
}

/// The operations a target names: a call such as `GET /orders/{id}` (the
/// method in any case), or a requirement ID for all of its operations.
pub fn select<'a>(ops: &'a [Operation], target: &str) -> Vec<&'a Operation> {
    let target = target.trim();
    if let Some((m, p)) = target.split_once(' ') {
        let m = m.to_ascii_lowercase();
        return ops
            .iter()
            .filter(|o| o.method == m && o.path == p.trim())
            .collect();
    }
    let base = crate::id::base_of(target);
    ops.iter().filter(|o| o.node == base).collect()
}

/// The links around an operation's requirement, read from the graph.
// spec: REQ-API-0003
pub fn card<'a>(graph: &Graph, op: &'a Operation) -> Card<'a> {
    let node = graph.get(&op.node);
    let outgoing = |kind: &str, ty: &str| -> Vec<String> {
        node.map(|n| {
            n.links
                .get(kind)
                .into_iter()
                .flatten()
                .map(|t| crate::id::base_of(t).to_string())
                .filter(|t| graph.get(t).is_some_and(|x| x.ty == ty))
                .collect()
        })
        .unwrap_or_default()
    };
    let mut debt = Vec::new();
    let mut plans = Vec::new();
    let mut risk_nodes: Vec<&crate::node::Node> = Vec::new();
    for (other, kind) in graph.incoming(&op.node) {
        match (other.ty.as_str(), kind) {
            ("debt", "arises-from") => debt.push(other.id.clone()),
            ("risk", "arises-from") => risk_nodes.push(other),
            ("test-plan", "covers") => plans.push(other.id.clone()),
            _ => {}
        }
    }
    for r in graph.nodes.iter().filter(|n| n.ty == "risk") {
        let mitigated = r
            .links
            .get("mitigated-by")
            .into_iter()
            .flatten()
            .any(|t| crate::id::base_of(t) == op.node);
        if mitigated {
            risk_nodes.push(r);
        }
    }
    let mut risks: Vec<(String, Option<&'static str>)> = risk_nodes
        .into_iter()
        .map(|r| {
            (
                r.id.clone(),
                crate::risks::level(r.likelihood.as_deref(), r.impact.as_deref()),
            )
        })
        .collect();
    for list in [&mut debt, &mut plans] {
        list.sort();
        list.dedup();
    }
    risks.sort();
    risks.dedup();
    let scenarios = node
        .into_iter()
        .flat_map(|n| {
            n.requirements
                .iter()
                .filter(|b| b.name == op.call())
                .flat_map(|b| &b.scenarios)
                .filter_map(|s| s.anchor.as_deref())
                .filter(|a| crate::id::is_valid_segment(a))
                .map(|a| format!("{}.{a}", n.id))
        })
        .collect();
    Card {
        operation: op,
        status: node.and_then(|n| n.status.clone()),
        scenarios,
        prds: outgoing("refines", "prd"),
        decisions: outgoing("decided-by", "adr"),
        debt,
        risks,
        plans,
    }
}

/// Operations that state no 401 or 403 response, in a project where other
/// operations do. Once a project models who may call (some operation
/// answers 401 or 403), an operation that never says what happens to a
/// caller who may not is a gap, and a place where broken access control
/// hides. An operation whose statement says it is public is left alone.
// spec: REQ-API-0006
pub fn check(graph: &Graph) -> Vec<crate::finding::Finding> {
    struct Op<'a> {
        node: &'a crate::node::Node,
        call: &'a str,
        denies: bool,
        public: bool,
        line: usize,
    }
    let mut ops: Vec<Op> = Vec::new();
    for node in graph.nodes.iter().filter(|n| n.ty == "requirement") {
        for b in &node.requirements {
            if parse_call(&b.name).is_none() {
                continue;
            }
            ops.push(Op {
                node,
                call: &b.name,
                denies: b
                    .scenarios
                    .iter()
                    .flat_map(|s| &s.responds)
                    .any(|c| c == "401" || c == "403"),
                public: b.statement.to_ascii_lowercase().contains("public"),
                line: b.line,
            });
        }
    }
    if !ops.iter().any(|o| o.denies) {
        return Vec::new();
    }
    ops.iter()
        .filter(|o| !o.denies && !o.public)
        .map(|o| {
            crate::finding::Finding::warning(
                "W023",
                format!(
                    "{} says nothing about a caller who may not use it: no scenario responds with 401 or 403 (say `public` in its statement if anyone may call it)",
                    o.call
                ),
            )
            .at(&o.node.path, Some(o.line))
            .node(&o.node.id)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn project() -> (tempfile::TempDir, Graph) {
        let dir = tempfile::tempdir().unwrap();
        let text = "---\nid: REQ-ORDERS-0001\ntype: requirement\nstatus: draft\n---\n# REQ-ORDERS-0001: Orders\n\n### Requirement: GET /orders/{id}\nThe API SHALL provide `GET /orders/{id}`: fetch an order.\n\n#### Scenario: Found {#S1}\n- **WHEN** GET /orders/{id} is called\n- **THEN** the API responds with 200: The order\n\n#### Scenario: Missing {#S2}\n- **WHEN** the order does not exist\n- **THEN** the API responds with 404: No such order\n\n### Requirement: Orders are kept\nThe system SHALL keep orders.\n\n#### Scenario: Kept {#S3}\n- **WHEN** an order is placed\n- **THEN** it is stored\n";
        let rel = "specs/requirements/orders/orders.md";
        let path = dir.path().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        let graph = Graph::from_sources([(PathBuf::from(rel), text.as_bytes().to_vec())]);
        (dir, graph)
    }

    // verifies: REQ-API-0001.S1
    #[test]
    fn a_requirement_titled_as_a_call_is_an_operation_with_its_responses() {
        let (dir, graph) = project();
        let ops = operations(&graph, dir.path()).unwrap();
        assert_eq!(ops.len(), 1, "{ops:?}");
        let o = &ops[0];
        assert_eq!(o.call(), "GET /orders/{id}");
        assert_eq!(o.tag, "orders");
        assert_eq!(o.summary, "fetch an order");
        assert_eq!(o.responses["404"], "No such order");
        let yaml = render(&ops, "Shop", "1.0.0");
        assert!(yaml.contains("openapi: 3.0.3"), "{yaml}");
        assert!(yaml.contains("\"/orders/{id}\":"), "{yaml}");
        assert!(yaml.contains("name: \"id\""), "{yaml}");
        assert!(yaml.contains("\"404\":"), "{yaml}");
        let again: Value = serde_saphyr::from_str(&yaml).unwrap();
        assert!(again.get("openapi").is_some());
    }

    // verifies: REQ-API-0001.S2
    #[test]
    fn a_requirement_that_is_not_a_call_is_left_out() {
        let (dir, graph) = project();
        let ops = operations(&graph, dir.path()).unwrap();
        assert!(ops.iter().all(|o| o.path.starts_with('/')));
        assert_eq!(parse_call("Orders are kept"), None);
        assert_eq!(parse_call("FETCH /x"), None);
    }

    // verifies: REQ-API-0002.S1
    #[test]
    fn drift_names_what_each_side_lacks() {
        let (dir, graph) = project();
        let ops = operations(&graph, dir.path()).unwrap();
        let doc: Value = serde_json::from_str(
            "{\"openapi\":\"3.0.0\",\"paths\":{\"/orders/{id}\":{\"get\":{\"responses\":{\"200\":{},\"500\":{}}}},\"/health\":{\"get\":{\"responses\":{\"200\":{}}}}}}",
        )
        .unwrap();
        let d = drift(&ops, &doc);
        assert_eq!(d.only_in_description, ["GET /health"]);
        assert!(d.only_in_requirements.is_empty());
        assert_eq!(
            d.codes,
            [(
                "GET /orders/{id}".to_string(),
                vec!["404".to_string()],
                vec!["500".to_string()]
            )]
        );
        let same: Value = serde_saphyr::from_str(&render(&ops, "Shop", "1")).unwrap();
        assert!(drift(&ops, &same).is_empty());
    }

    // verifies: REQ-API-0002.S2
    #[test]
    fn a_requirement_with_no_operation_in_the_file_is_reported() {
        let (dir, graph) = project();
        let ops = operations(&graph, dir.path()).unwrap();
        let empty: Value = serde_json::from_str("{\"openapi\":\"3.0.0\",\"paths\":{}}").unwrap();
        assert_eq!(
            drift(&ops, &empty).only_in_requirements,
            ["GET /orders/{id}"]
        );
    }

    // verifies: REQ-API-0003.S1
    #[test]
    fn a_card_gathers_what_surrounds_an_operation() {
        let dir = tempfile::tempdir().unwrap();
        let files = [
            (
                "specs/prd/1.md",
                "---\nid: PRD-0001\ntype: prd\nstatus: draft\n---\n# PRD-0001: P\n",
            ),
            (
                "specs/adr/1.md",
                "---\nid: ADR-0001\ntype: adr\nstatus: accepted\n---\n# ADR-0001: A\n",
            ),
            (
                "specs/debt/1.md",
                "---\nid: DEBT-0001\ntype: debt\nstatus: open\nseverity: low\nlinks:\n  arises-from: REQ-ORDERS-0001\n---\n# DEBT-0001: D\n",
            ),
            (
                "specs/risks/1.md",
                "---\nid: RISK-0001\ntype: risk\nstatus: open\nlikelihood: high\nimpact: high\nlinks:\n  mitigated-by: [REQ-ORDERS-0001]\n---\n# RISK-0001: R\n",
            ),
            (
                "specs/tp/1.md",
                "---\nid: TP-0001\ntype: test-plan\nstatus: draft\nlinks:\n  covers: [REQ-ORDERS-0001]\n---\n# TP-0001: T\n",
            ),
            (
                "specs/requirements/orders/orders.md",
                "---\nid: REQ-ORDERS-0001\ntype: requirement\nstatus: draft\nlinks:\n  refines: PRD-0001\n  decided-by: ADR-0001\n---\n# REQ-ORDERS-0001: Orders\n\n### Requirement: GET /orders/{id}\nThe API SHALL provide `GET /orders/{id}`: fetch an order.\n\n#### Scenario: Found {#S1}\n- **WHEN** a\n- **THEN** the API responds with 200: The order\n\n#### Scenario: Missing {#S2}\n- **WHEN** b\n- **THEN** the API responds with 404: No such order\n",
            ),
        ];
        let mut sources = Vec::new();
        for (rel, text) in files {
            let path = dir.path().join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, text).unwrap();
            sources.push((PathBuf::from(rel), text.as_bytes().to_vec()));
        }
        let graph = Graph::from_sources(sources);
        let ops = operations(&graph, dir.path()).unwrap();
        assert_eq!(select(&ops, "get /orders/{id}").len(), 1);
        assert_eq!(select(&ops, "REQ-ORDERS-0001").len(), 1);
        assert!(select(&ops, "POST /nowhere").is_empty());
        let c = card(&graph, &ops[0]);
        assert_eq!(c.prds, ["PRD-0001"]);
        assert_eq!(c.decisions, ["ADR-0001"]);
        assert_eq!(c.debt, ["DEBT-0001"]);
        assert_eq!(c.risks, [("RISK-0001".to_string(), Some("critical"))]);
        assert_eq!(c.plans, ["TP-0001"]);
        assert_eq!(c.scenarios.len(), 2);
        assert!(
            c.scenarios
                .iter()
                .all(|s| s.starts_with("REQ-ORDERS-0001.S"))
        );
    }

    // verifies: REQ-API-0006.S1
    #[test]
    fn an_operation_without_a_denial_is_warned_about_once_the_project_models_one() {
        let req = |id: &str, call: &str, extra: &str, code: &str| {
            format!(
                "---\nid: {id}\ntype: requirement\nstatus: draft\n---\n# {id}: R\n\n### Requirement: {call}\nThe API SHALL provide `{call}`{extra}.\n\n#### Scenario: Ok {{#S1}}\n- **WHEN** a\n- **THEN** the API responds with {code}\n"
            )
        };
        let graph = |files: &[(&str, String)]| {
            Graph::from_sources(
                files
                    .iter()
                    .map(|(p, t)| (PathBuf::from(p), t.clone().into_bytes()))
                    .collect::<Vec<_>>(),
            )
        };
        // No operation answers 401 or 403: nothing is asked.
        let none = graph(&[("a.md", req("REQ-A-0001", "GET /orders", "", "200"))]);
        assert!(check(&none).is_empty());
        // One does, so the others are asked.
        let some = graph(&[
            ("a.md", req("REQ-A-0001", "GET /orders", "", "200")),
            ("b.md", req("REQ-A-0002", "DELETE /orders", "", "403")),
            (
                "c.md",
                req("REQ-A-0003", "GET /health", ": a public check", "200"),
            ),
            ("d.md", req("REQ-A-0004", "Orders are kept", "", "200")),
        ]);
        let found = check(&some);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].code, "W023");
        assert!(found[0].message.contains("GET /orders"));
    }
}
