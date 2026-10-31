//! The architecture model (design section 18): describe elements and
//! relationships once, and generate every view from that.
//!
//! An element is a node of type `system`, `person`, `container` or `component`,
//! with one fenced `architecture` block:
//!
//! ```text
//! name: Orders API
//! technology: Go, net/http
//! parent: C4-0001
//! owner: team-orders
//! uses:
//!   - {to: C4-0003, description: reads and writes orders, technology: SQL}
//! rules:
//!   - forbid C4-0002 -> C4-0004
//! ```
//!
//! A container's `parent` is its system and a component's is its container. A node
//! of type `deployment-environment` lists where containers run:
//!
//! ```text
//! name: Production
//! nodes:
//!   - {id: eks, kind: kubernetes-cluster, region: eu-west-1, hosts: [C4-0002]}
//! ```
//!
//! Rules (`forbid A -> B` for a direct use, `forbid A ->* B` for a use through
//! any number of elements) can be written in any of these nodes.
//!
//! E025 the block cannot be read     E026 a reference is wrong
//! E027 an architecture rule is broken
//! W009 a container has no owner or technology
//! W010 a container runs in no environment

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use serde::Deserialize;

use crate::claims::is_inactive;
use crate::finding::Finding;
use crate::graph::Graph;
use crate::node::Node;

const ELEMENT_TYPES: &[&str] = &["system", "person", "container", "component"];
const ENVIRONMENT: &str = "deployment-environment";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub from: String,
    pub to: String,
    pub description: Option<String>,
    pub technology: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Element {
    pub id: String,
    pub ty: String,
    pub name: String,
    pub technology: Option<String>,
    pub owner: Option<String>,
    pub parent: Option<String>,
    pub path: PathBuf,
    pub line: usize,
    pub active: bool,
    pub uses: Vec<Relationship>,
}

#[derive(Debug, Clone)]
pub struct DeployNode {
    pub name: String,
    pub kind: Option<String>,
    pub region: Option<String>,
    pub hosts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub line: usize,
    pub active: bool,
    pub nodes: Vec<DeployNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// `->`: a relationship between the two.
    Direct,
    /// `->*`: a path of relationships, of any length.
    Path,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub text: String,
    pub from: String,
    pub to: String,
    pub reach: Reach,
    /// The node the rule is written in.
    pub node: String,
    pub path: PathBuf,
    pub line: usize,
}

#[derive(Debug, Default)]
pub struct Model {
    pub elements: Vec<Element>,
    pub environments: Vec<Environment>,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Default)]
pub struct Analysis {
    pub model: Model,
    /// Blocks that cannot be read (E025).
    pub findings: Vec<Finding>,
}

impl Model {
    pub fn element(&self, id: &str) -> Option<&Element> {
        self.elements.iter().find(|e| e.id == id)
    }

    pub fn environment(&self, id: &str) -> Option<&Environment> {
        self.environments.iter().find(|e| e.id == id)
    }

    /// The element, its parent, and so on up.
    fn chain(&self, id: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let mut at = self.element(id);
        while let Some(e) = at {
            if out.contains(&e.id.as_str()) {
                break;
            }
            out.push(e.id.as_str());
            at = e.parent.as_deref().and_then(|p| self.element(p));
        }
        out
    }

    fn relationships(&self) -> impl Iterator<Item = &Relationship> {
        self.elements
            .iter()
            .filter(|e| e.active)
            .flat_map(|e| &e.uses)
    }
}

// ---------------------------------------------------------------- reading

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    name: Option<String>,
    technology: Option<String>,
    parent: Option<String>,
    owner: Option<String>,
    #[serde(default)]
    uses: Vec<RawUse>,
    #[serde(default)]
    nodes: Vec<RawNode>,
    #[serde(default)]
    rules: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUse {
    to: String,
    description: Option<String>,
    technology: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNode {
    id: String,
    kind: Option<String>,
    region: Option<String>,
    #[serde(default)]
    hosts: Vec<String>,
}

fn parse_rule(text: &str) -> Result<(String, String, Reach), String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    match words.as_slice() {
        ["forbid", a, arrow @ ("->" | "->*"), b] => {
            let reach = if *arrow == "->" {
                Reach::Direct
            } else {
                Reach::Path
            };
            Ok((a.to_string(), b.to_string(), reach))
        }
        _ => Err(format!(
            "rule `{text}` is not `forbid A -> B` or `forbid A ->* B`"
        )),
    }
}

fn read_node(node: &Node, model: &mut Model, out: &mut Vec<Finding>) {
    let Some(first) = node.architecture.first() else {
        return;
    };
    let bad = |message: String| {
        Finding::error("E025", format!("architecture {}: {message}", node.id))
            .at(&node.path, Some(first.line))
            .node(&node.id)
    };
    if node.architecture.len() > 1 {
        out.push(bad("a node holds one `architecture` block".to_string()));
        return;
    }
    let is_element = ELEMENT_TYPES.contains(&node.ty.as_str());
    if !is_element && node.ty != ENVIRONMENT {
        out.push(bad(format!(
            "a block belongs in a node of type system, person, container, component or {ENVIRONMENT}, not `{}`",
            node.ty
        )));
        return;
    }
    let raw: Raw = match serde_saphyr::from_str(&first.text) {
        Ok(raw) => raw,
        Err(e) => {
            out.push(bad(format!("the block is not valid: {e}")));
            return;
        }
    };
    let active = !is_inactive(node.status.as_deref());
    let name = raw
        .name
        .clone()
        .or_else(|| node.title.clone())
        .unwrap_or_else(|| node.id.clone());
    for text in &raw.rules {
        match parse_rule(text) {
            Ok((from, to, reach)) => {
                if active {
                    model.rules.push(Rule {
                        text: text.split_whitespace().collect::<Vec<_>>().join(" "),
                        from,
                        to,
                        reach,
                        node: node.id.clone(),
                        path: node.path.clone(),
                        line: first.line,
                    });
                }
            }
            Err(message) => {
                out.push(bad(message));
                return;
            }
        }
    }
    if is_element {
        if !raw.nodes.is_empty() {
            out.push(bad(
                "`nodes` belongs in a deployment environment".to_string()
            ));
            return;
        }
        model.elements.push(Element {
            id: node.id.clone(),
            ty: node.ty.clone(),
            name,
            technology: raw.technology,
            owner: raw.owner.or_else(|| node.owner.clone()),
            parent: raw.parent,
            path: node.path.clone(),
            line: first.line,
            active,
            uses: raw
                .uses
                .into_iter()
                .map(|u| Relationship {
                    from: node.id.clone(),
                    to: u.to,
                    description: u.description,
                    technology: u.technology,
                })
                .collect(),
        });
    } else {
        if !raw.uses.is_empty() || raw.parent.is_some() || raw.technology.is_some() {
            out.push(bad(
                "an environment lists `nodes`, and has no `uses`, `parent` or `technology`"
                    .to_string(),
            ));
            return;
        }
        model.environments.push(Environment {
            id: node.id.clone(),
            name,
            path: node.path.clone(),
            line: first.line,
            active,
            nodes: raw
                .nodes
                .into_iter()
                .map(|n| DeployNode {
                    name: n.id,
                    kind: n.kind,
                    region: n.region,
                    hosts: n.hosts,
                })
                .collect(),
        });
    }
}

pub fn analyze(graph: &Graph) -> Analysis {
    let mut analysis = Analysis::default();
    for node in graph.nodes.iter().filter(|n| !n.architecture.is_empty()) {
        read_node(node, &mut analysis.model, &mut analysis.findings);
    }
    analysis
}

// ---------------------------------------------------------------- checks

fn at_element(finding: Finding, id: &str, path: &std::path::Path, line: usize) -> Finding {
    finding.at(path, Some(line)).node(id)
}

fn references(model: &Model, out: &mut Vec<Finding>) {
    for e in model.elements.iter().filter(|e| e.active) {
        let bad = |message: String| {
            at_element(
                Finding::error("E026", format!("{} {}: {message}", e.ty, e.id)),
                &e.id,
                &e.path,
                e.line,
            )
        };
        match (e.ty.as_str(), e.parent.as_deref()) {
            ("container" | "component", None) => {
                let want = if e.ty == "container" {
                    "system"
                } else {
                    "container"
                };
                out.push(bad(format!("has no `parent`, which must be a {want}")));
            }
            ("container" | "component", Some(p)) => {
                let want = if e.ty == "container" {
                    "system"
                } else {
                    "container"
                };
                match model.element(p) {
                    None => out.push(bad(format!("`parent` {p} is not an element"))),
                    Some(parent) if parent.ty != want => out.push(bad(format!(
                        "`parent` {p} is a {}, and must be a {want}",
                        parent.ty
                    ))),
                    Some(_) => {}
                }
            }
            (_, Some(_)) => out.push(bad("takes no `parent`".to_string())),
            _ => {}
        }
        for u in &e.uses {
            if model.element(&u.to).is_none() {
                out.push(bad(format!("`uses` {} is not an element", u.to)));
            }
        }
    }
    for env in model.environments.iter().filter(|e| e.active) {
        for node in &env.nodes {
            for host in &node.hosts {
                let problem = match model.element(host) {
                    None => Some("is not an element".to_string()),
                    Some(h) if !matches!(h.ty.as_str(), "container" | "component") => Some(
                        format!("is a {}, and only containers and components run", h.ty),
                    ),
                    Some(_) => None,
                };
                if let Some(problem) = problem {
                    out.push(at_element(
                        Finding::error(
                            "E026",
                            format!(
                                "environment {}: `{}` hosts {host}, which {problem}",
                                env.id, node.name
                            ),
                        ),
                        &env.id,
                        &env.path,
                        env.line,
                    ));
                }
            }
        }
    }
    for rule in &model.rules {
        for id in [&rule.from, &rule.to] {
            if model.element(id).is_none() {
                out.push(at_element(
                    Finding::error(
                        "E026",
                        format!(
                            "rule in {}: `{}` names {id}, which is not an element",
                            rule.node, rule.text
                        ),
                    ),
                    &rule.node,
                    &rule.path,
                    rule.line,
                ));
            }
        }
    }
}

/// The relationships from `from` to `to`, one after another, if there are any.
fn find_path<'a>(model: &'a Model, from: &str, to: &str) -> Option<Vec<&'a Relationship>> {
    let mut out_edges: HashMap<&str, Vec<&Relationship>> = HashMap::new();
    for r in model.relationships() {
        out_edges.entry(&r.from).or_default().push(r);
    }
    let mut how: HashMap<&str, &Relationship> = HashMap::new();
    let mut seen: HashSet<&str> = HashSet::from([from]);
    let mut queue: VecDeque<&str> = VecDeque::from([from]);
    while let Some(at) = queue.pop_front() {
        for r in out_edges.get(at).into_iter().flatten() {
            if !seen.insert(&r.to) {
                continue;
            }
            how.insert(&r.to, r);
            if r.to == to {
                let mut path = Vec::new();
                let mut cur = to;
                while let Some(r) = how.get(cur) {
                    path.push(*r);
                    cur = &r.from;
                }
                path.reverse();
                return Some(path);
            }
            queue.push_back(&r.to);
        }
    }
    None
}

fn rules(model: &Model, out: &mut Vec<Finding>) {
    for rule in &model.rules {
        if model.element(&rule.from).is_none() || model.element(&rule.to).is_none() {
            continue;
        }
        let path = match rule.reach {
            Reach::Direct => model
                .relationships()
                .find(|r| r.from == rule.from && r.to == rule.to)
                .map(|r| vec![r]),
            Reach::Path => find_path(model, &rule.from, &rule.to),
        };
        if let Some(path) = path {
            let mut via = path[0].from.clone();
            for r in &path {
                via.push_str(&format!(" -> {}", r.to));
            }
            out.push(
                Finding::error(
                    "E027",
                    format!(
                        "architecture rule `{}` in {} is broken by {via}",
                        rule.text, rule.node
                    ),
                )
                .at(&rule.path, Some(rule.line))
                .node(&rule.node),
            );
        }
    }
}

fn completeness(model: &Model, out: &mut Vec<Finding>) {
    let deployed: BTreeSet<&str> = model
        .environments
        .iter()
        .filter(|e| e.active)
        .flat_map(|e| &e.nodes)
        .flat_map(|n| n.hosts.iter().map(String::as_str))
        .collect();
    let any_environment = model.environments.iter().any(|e| e.active);
    for e in model
        .elements
        .iter()
        .filter(|e| e.active && e.ty == "container")
    {
        let missing = match (e.owner.is_none(), e.technology.is_none()) {
            (true, true) => Some("owner and no technology"),
            (true, false) => Some("owner"),
            (false, true) => Some("technology"),
            (false, false) => None,
        };
        if let Some(missing) = missing {
            out.push(at_element(
                Finding::warning("W009", format!("container {} has no {missing}", e.id)),
                &e.id,
                &e.path,
                e.line,
            ));
        }
        if any_environment && !deployed.contains(e.id.as_str()) {
            out.push(at_element(
                Finding::warning(
                    "W010",
                    format!("container {} runs in no deployment environment", e.id),
                ),
                &e.id,
                &e.path,
                e.line,
            ));
        }
    }
}

pub fn check(graph: &Graph) -> Vec<Finding> {
    let analysis = analyze(graph);
    let mut findings = analysis.findings;
    references(&analysis.model, &mut findings);
    rules(&analysis.model, &mut findings);
    completeness(&analysis.model, &mut findings);
    findings
}

// ---------------------------------------------------------------- views

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewKind {
    Context,
    Container,
    Component,
    Deployment,
}

impl ViewKind {
    pub fn name(self) -> &'static str {
        match self {
            ViewKind::Context => "context",
            ViewKind::Container => "container",
            ViewKind::Component => "component",
            ViewKind::Deployment => "deployment",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewNode {
    pub id: String,
    pub label: String,
    /// `person`, `system`, `container` or `component`.
    pub class: String,
    pub group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub id: String,
    pub label: String,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewEdge {
    pub from: String,
    pub to: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub kind: ViewKind,
    pub root: String,
    pub title: String,
    pub groups: Vec<Group>,
    pub nodes: Vec<ViewNode>,
    pub edges: Vec<ViewEdge>,
}

fn label_of(e: &Element) -> String {
    match &e.technology {
        Some(t) => format!("{}<br/>[{t}]", e.name),
        None => e.name.clone(),
    }
}

fn edge_label(r: &Relationship) -> String {
    match (&r.description, &r.technology) {
        (Some(d), Some(t)) => format!("{d} [{t}]"),
        (Some(d), None) => d.clone(),
        (None, Some(t)) => format!("[{t}]"),
        (None, None) => String::new(),
    }
}

/// Builds a view, or says why it cannot.
pub fn view(model: &Model, kind: ViewKind, root: &str) -> Result<View, String> {
    if kind == ViewKind::Deployment {
        return deployment_view(model, root);
    }
    let want = match kind {
        ViewKind::Context | ViewKind::Container => "system",
        _ => "container",
    };
    let r = model
        .element(root)
        .ok_or_else(|| format!("no element `{root}`"))?;
    if r.ty != want {
        return Err(format!(
            "a {} view is of a {want}, and {root} is a {}",
            kind.name(),
            r.ty
        ));
    }
    // What an element is shown as: for a context view the top of its chain, for the
    // others the first element on its chain that is not part of the root's own chain.
    let root_chain: HashSet<&str> = model.chain(root).into_iter().collect();
    let shown = |id: &str| -> Option<String> {
        let mut chain = model.chain(id);
        chain.reverse();
        if kind == ViewKind::Context {
            return chain.first().map(|s| s.to_string());
        }
        chain
            .into_iter()
            .find(|c| !root_chain.contains(c))
            .map(str::to_string)
    };
    let inside = |id: &str| model.chain(id).contains(&root);

    let mut nodes: BTreeMap<String, ViewNode> = BTreeMap::new();
    let mut edges: Vec<ViewEdge> = Vec::new();
    let add = |id: &str, nodes: &mut BTreeMap<String, ViewNode>| {
        if let Some(e) = model.element(id) {
            let in_group = kind != ViewKind::Context && e.parent.as_deref() == Some(root);
            nodes.entry(id.to_string()).or_insert(ViewNode {
                id: id.to_string(),
                label: label_of(e),
                class: e.ty.clone(),
                group: in_group.then(|| root.to_string()),
            });
        }
    };
    if kind == ViewKind::Context {
        add(root, &mut nodes);
    }
    for rel in model.relationships() {
        if !inside(&rel.from) && !inside(&rel.to) {
            continue;
        }
        let (Some(a), Some(b)) = (shown(&rel.from), shown(&rel.to)) else {
            continue;
        };
        if a == b {
            continue;
        }
        add(&a, &mut nodes);
        add(&b, &mut nodes);
        let label = edge_label(rel);
        match edges
            .iter_mut()
            .find(|e: &&mut ViewEdge| e.from == a && e.to == b)
        {
            // Relationships that lift to the same pair become one edge.
            Some(edge) => {
                if !label.is_empty() && !edge.label.split("; ").any(|l| l == label) {
                    if !edge.label.is_empty() {
                        edge.label.push_str("; ");
                    }
                    edge.label.push_str(&label);
                }
            }
            None => edges.push(ViewEdge {
                from: a,
                to: b,
                label,
            }),
        }
    }
    if kind != ViewKind::Context {
        // Children with no relationship still belong in the view.
        for e in model
            .elements
            .iter()
            .filter(|e| e.active && e.parent.as_deref() == Some(root))
        {
            add(&e.id, &mut nodes);
        }
    }
    let groups = if kind == ViewKind::Context {
        Vec::new()
    } else {
        vec![Group {
            id: r.id.clone(),
            label: label_of(r),
            parent: None,
        }]
    };
    Ok(View {
        kind,
        root: root.to_string(),
        title: format!("{} view of {}", kind.name(), r.name),
        groups,
        nodes: nodes.into_values().collect(),
        edges,
    })
}

fn deployment_view(model: &Model, root: &str) -> Result<View, String> {
    let env = model
        .environment(root)
        .ok_or_else(|| format!("no deployment environment `{root}`"))?;
    let mut groups = vec![Group {
        id: env.id.clone(),
        label: env.name.clone(),
        parent: None,
    }];
    let mut nodes = Vec::new();
    for node in &env.nodes {
        let gid = format!("{}.{}", env.id, node.name);
        let detail: Vec<&str> = [node.kind.as_deref(), node.region.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        groups.push(Group {
            id: gid.clone(),
            label: if detail.is_empty() {
                node.name.clone()
            } else {
                format!("{} ({})", node.name, detail.join(", "))
            },
            parent: Some(env.id.clone()),
        });
        for host in &node.hosts {
            if let Some(e) = model.element(host) {
                nodes.push(ViewNode {
                    id: format!("{gid}.{host}"),
                    label: label_of(e),
                    class: e.ty.clone(),
                    group: Some(gid.clone()),
                });
            }
        }
    }
    let mut edges = Vec::new();
    // An element can run on several nodes. Edges go between the first instances.
    let mut instance: HashMap<&str, &str> = HashMap::new();
    for n in &nodes {
        if let Some(element) = n.id.rsplit('.').next() {
            instance.entry(element).or_insert(&n.id);
        }
    }
    for rel in model.relationships() {
        if let (Some(from), Some(to)) = (
            instance.get(rel.from.as_str()),
            instance.get(rel.to.as_str()),
        ) {
            edges.push(ViewEdge {
                from: from.to_string(),
                to: to.to_string(),
                label: edge_label(rel),
            });
        }
    }
    Ok(View {
        kind: ViewKind::Deployment,
        root: root.to_string(),
        title: format!("deployment view of {}", env.name),
        groups,
        nodes,
        edges,
    })
}

fn mermaid_id(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn mermaid_text(text: &str) -> String {
    text.replace('"', "'")
}

/// A Mermaid `flowchart`, which renders in GitHub, GitLab and most viewers.
pub fn mermaid(view: &View) -> String {
    let mut out = format!(
        "---\ntitle: {}\n---\nflowchart LR\n",
        mermaid_text(&view.title)
    );
    fn emit(view: &View, parent: Option<&str>, depth: usize, out: &mut String) {
        let pad = "    ".repeat(depth);
        for g in view.groups.iter().filter(|g| g.parent.as_deref() == parent) {
            out.push_str(&format!(
                "{pad}subgraph {}[\"{}\"]\n",
                mermaid_id(&g.id),
                mermaid_text(&g.label)
            ));
            emit(view, Some(&g.id), depth + 1, out);
            for n in view
                .nodes
                .iter()
                .filter(|n| n.group.as_deref() == Some(&g.id))
            {
                out.push_str(&format!(
                    "{pad}    {}[\"{}\"]:::{}\n",
                    mermaid_id(&n.id),
                    mermaid_text(&n.label),
                    n.class
                ));
            }
            out.push_str(&format!("{pad}end\n"));
        }
    }
    emit(view, None, 1, &mut out);
    for n in view.nodes.iter().filter(|n| n.group.is_none()) {
        out.push_str(&format!(
            "    {}[\"{}\"]:::{}\n",
            mermaid_id(&n.id),
            mermaid_text(&n.label),
            n.class
        ));
    }
    for e in &view.edges {
        if e.label.is_empty() {
            out.push_str(&format!(
                "    {} --> {}\n",
                mermaid_id(&e.from),
                mermaid_id(&e.to)
            ));
        } else {
            out.push_str(&format!(
                "    {} -->|\"{}\"| {}\n",
                mermaid_id(&e.from),
                mermaid_text(&e.label),
                mermaid_id(&e.to)
            ));
        }
    }
    out.push_str("    classDef person fill:#08427b,color:#fff\n");
    out.push_str("    classDef system fill:#1168bd,color:#fff\n");
    out.push_str("    classDef container fill:#438dd5,color:#fff\n");
    out.push_str("    classDef component fill:#85bbf0,color:#000\n");
    out
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_file_text;
    use std::path::Path;

    fn node(id: &str, ty: &str, status: &str, block: &str) -> Node {
        let text = format!(
            "---\nid: {id}\ntype: {ty}\nstatus: {status}\n---\n# {id}\n\n```architecture\n{block}```\n"
        );
        let (node, findings) = parse_file_text(Path::new(&format!("{id}.md")), &text);
        assert!(findings.is_empty(), "{findings:?}");
        node.expect("a node")
    }

    /// Shop (system) with a web and an api container and a database, a customer, and a payment system.
    fn shop() -> Vec<Node> {
        vec![
            node("C4-0001", "system", "accepted", "name: Shop\n"),
            node(
                "C4-0002",
                "container",
                "accepted",
                "name: Web\ntechnology: React\nparent: C4-0001\nowner: team-web\nuses:\n  - {to: C4-0003, description: calls, technology: HTTPS}\n",
            ),
            node(
                "C4-0003",
                "container",
                "accepted",
                "name: Orders API\ntechnology: Go\nparent: C4-0001\nowner: team-orders\nuses:\n  - {to: C4-0004, description: reads and writes}\n  - {to: C4-0006, description: charges cards}\n",
            ),
            node(
                "C4-0004",
                "container",
                "accepted",
                "name: Orders DB\ntechnology: Postgres\nparent: C4-0001\nowner: team-orders\n",
            ),
            node(
                "C4-0005",
                "person",
                "accepted",
                "name: Customer\nuses:\n  - {to: C4-0002, description: browses}\n",
            ),
            node("C4-0006", "system", "accepted", "name: Payments\n"),
            node(
                "C4-0007",
                "component",
                "accepted",
                "name: Order handler\nparent: C4-0003\nuses:\n  - {to: C4-0004, description: stores}\n  - {to: C4-0006}\n",
            ),
            node(
                "DEP-0001",
                ENVIRONMENT,
                "accepted",
                "name: Production\nnodes:\n  - {id: eks, kind: kubernetes, region: eu-west-1, hosts: [C4-0002, C4-0003]}\n  - {id: rds, kind: database, hosts: [C4-0004]}\n",
            ),
        ]
    }

    fn graph(nodes: Vec<Node>) -> Graph {
        Graph::from_nodes(nodes)
    }

    fn codes(findings: &[Finding]) -> Vec<&'static str> {
        findings.iter().map(|f| f.code).collect()
    }

    fn model() -> Model {
        let a = analyze(&graph(shop()));
        assert!(a.findings.is_empty(), "{:?}", a.findings);
        a.model
    }

    #[test]
    fn a_sound_model_has_no_findings() {
        assert!(check(&graph(shop())).is_empty());
    }

    #[test]
    fn unreadable_blocks_are_e025() {
        for (ty, block) in [
            ("system", "what: 1\n"),
            ("system", "nodes:\n  - {id: a}\n"),
            ("deployment-environment", "uses:\n  - {to: X-1}\n"),
            ("system", "rules:\n  - allow A -> B\n"),
            ("requirement", "name: x\n"),
            ("system", "uses: [\n"),
        ] {
            let found = check(&graph(vec![node("C4-0001", ty, "accepted", block)]));
            assert_eq!(codes(&found), vec!["E025"], "{block:?}: {found:?}");
        }
    }

    #[test]
    fn parents_and_references_must_point_at_the_right_things() {
        let mut nodes = shop();
        nodes.push(node(
            "C4-0008",
            "container",
            "accepted",
            "technology: Go\nowner: x\nparent: C4-0003\nuses:\n  - {to: C4-9999}\n",
        ));
        nodes.push(node("C4-0009", "component", "accepted", "technology: Go\n"));
        nodes.push(node("C4-0010", "system", "accepted", "parent: C4-0001\n"));
        let found = check(&graph(nodes));
        let messages: Vec<_> = found
            .iter()
            .filter(|f| f.code == "E026")
            .map(|f| f.message.as_str())
            .collect();
        assert!(
            messages.contains(
                &"container C4-0008: `parent` C4-0003 is a container, and must be a system"
            )
        );
        assert!(messages.contains(&"container C4-0008: `uses` C4-9999 is not an element"));
        assert!(
            messages.contains(&"component C4-0009: has no `parent`, which must be a container")
        );
        assert!(messages.contains(&"system C4-0010: takes no `parent`"));
    }

    #[test]
    fn environments_host_only_containers_and_components() {
        let mut nodes = shop();
        nodes.push(node(
            "DEP-0002",
            ENVIRONMENT,
            "accepted",
            "nodes:\n  - {id: a, hosts: [C4-0001, C4-9999, C4-0004]}\n",
        ));
        let found = check(&graph(nodes));
        let messages: Vec<_> = found.iter().map(|f| f.message.as_str()).collect();
        assert!(messages.contains(&"environment DEP-0002: `a` hosts C4-0001, which is a system, and only containers and components run"));
        assert!(
            messages.contains(&"environment DEP-0002: `a` hosts C4-9999, which is not an element")
        );
    }

    #[test]
    fn rules_find_direct_and_indirect_use_with_the_path() {
        let mut nodes = shop();
        nodes.push(node(
            "C4-0011",
            "system",
            "accepted",
            "rules:\n  - forbid C4-0002 -> C4-0004\n  - forbid C4-0002 ->* C4-0004\n  - forbid C4-0004 ->* C4-0002\n  - forbid C4-0003 -> C4-0004\n",
        ));
        let found = check(&graph(nodes));
        let broken: Vec<_> = found
            .iter()
            .filter(|f| f.code == "E027")
            .map(|f| f.message.as_str())
            .collect();
        assert_eq!(
            broken,
            vec![
                "architecture rule `forbid C4-0002 ->* C4-0004` in C4-0011 is broken by C4-0002 -> C4-0003 -> C4-0004",
                "architecture rule `forbid C4-0003 -> C4-0004` in C4-0011 is broken by C4-0003 -> C4-0004",
            ]
        );
    }

    #[test]
    fn rules_about_unknown_elements_are_e026() {
        let mut nodes = shop();
        nodes.push(node(
            "C4-0011",
            "system",
            "accepted",
            "rules:\n  - forbid C4-0002 -> C4-4444\n",
        ));
        let found = check(&graph(nodes));
        assert_eq!(codes(&found), vec!["E026"]);
    }

    #[test]
    fn containers_need_an_owner_a_technology_and_a_place_to_run() {
        let mut nodes = shop();
        nodes.push(node(
            "C4-0012",
            "container",
            "accepted",
            "parent: C4-0001\n",
        ));
        let found = check(&graph(nodes));
        let warned: Vec<_> = found.iter().map(|f| (f.code, f.message.as_str())).collect();
        assert!(warned.contains(&("W009", "container C4-0012 has no owner and no technology")));
        assert!(warned.contains(&(
            "W010",
            "container C4-0012 runs in no deployment environment"
        )));
        // Without any environment, nothing is deployed anywhere, so there is nothing to say.
        let mut nodes = shop();
        nodes.pop();
        assert!(check(&graph(nodes)).is_empty());
    }

    #[test]
    fn inactive_elements_are_not_checked() {
        let mut nodes = shop();
        nodes.push(node("C4-0013", "container", "superseded", "name: Old\n"));
        assert!(check(&graph(nodes)).is_empty());
    }

    #[test]
    fn the_context_view_shows_people_and_systems_around_one_system() {
        let m = model();
        let v = view(&m, ViewKind::Context, "C4-0001").unwrap();
        let ids: Vec<_> = v.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["C4-0001", "C4-0005", "C4-0006"]);
        assert!(v.groups.is_empty());
        let edges: Vec<_> = v
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect();
        assert!(edges.contains(&("C4-0005", "C4-0001")));
        assert!(edges.contains(&("C4-0001", "C4-0006")));
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn the_container_view_lifts_outside_elements_and_hides_components() {
        let m = model();
        let v = view(&m, ViewKind::Container, "C4-0001").unwrap();
        let ids: Vec<_> = v.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["C4-0002", "C4-0003", "C4-0004", "C4-0005", "C4-0006"]
        );
        assert_eq!(v.groups.len(), 1);
        assert!(
            v.nodes
                .iter()
                .filter(|n| n.group.is_some())
                .all(|n| ["C4-0002", "C4-0003", "C4-0004"].contains(&n.id.as_str()))
        );
        assert!(!ids.contains(&"C4-0007"));
    }

    #[test]
    fn the_component_view_shows_one_container_with_what_it_talks_to() {
        let m = model();
        let v = view(&m, ViewKind::Component, "C4-0003").unwrap();
        let ids: Vec<_> = v.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["C4-0004", "C4-0006", "C4-0007"]);
        assert_eq!(
            v.nodes
                .iter()
                .find(|n| n.id == "C4-0007")
                .unwrap()
                .group
                .as_deref(),
            Some("C4-0003")
        );
    }

    #[test]
    fn the_deployment_view_nests_instances_in_nodes() {
        let m = model();
        let v = view(&m, ViewKind::Deployment, "DEP-0001").unwrap();
        assert_eq!(v.groups.len(), 3);
        assert_eq!(v.nodes.len(), 3);
        let text = mermaid(&v);
        assert!(text.contains("subgraph DEP_0001[\"Production\"]"));
        assert!(text.contains("subgraph DEP_0001_eks[\"eks (kubernetes, eu-west-1)\"]"));
        assert!(text.contains("DEP_0001_eks_C4_0002[\"Web<br/>[React]\"]:::container"));
        assert!(
            text.contains("DEP_0001_eks_C4_0003 -->|\"reads and writes\"| DEP_0001_rds_C4_0004")
        );
    }

    #[test]
    fn views_say_what_is_wrong_with_the_root() {
        let m = model();
        assert!(
            view(&m, ViewKind::Container, "C4-0002")
                .unwrap_err()
                .contains("is of a system")
        );
        assert!(
            view(&m, ViewKind::Component, "C4-0001")
                .unwrap_err()
                .contains("is of a container")
        );
        assert!(
            view(&m, ViewKind::Context, "C4-9999")
                .unwrap_err()
                .contains("no element")
        );
        assert!(
            view(&m, ViewKind::Deployment, "C4-0001")
                .unwrap_err()
                .contains("no deployment environment")
        );
    }

    #[test]
    fn mermaid_has_groups_nodes_edges_and_styles() {
        let m = model();
        let text = mermaid(&view(&m, ViewKind::Container, "C4-0001").unwrap());
        assert!(text.starts_with("---\ntitle: container view of Shop\n---\nflowchart LR\n"));
        assert!(text.contains("    subgraph C4_0001[\"Shop\"]\n"));
        assert!(text.contains("        C4_0003[\"Orders API<br/>[Go]\"]:::container\n"));
        assert!(text.contains("    C4_0002 -->|\"calls [HTTPS]\"| C4_0003\n"));
        assert!(text.contains("    C4_0005 -->|\"browses\"| C4_0002\n"));
        assert!(text.contains("classDef person"));
    }

    #[test]
    fn a_cycle_of_parents_does_not_hang() {
        let nodes = vec![
            node(
                "C4-0001",
                "container",
                "accepted",
                "parent: C4-0002\ntechnology: x\nowner: y\n",
            ),
            node(
                "C4-0002",
                "container",
                "accepted",
                "parent: C4-0001\ntechnology: x\nowner: y\n",
            ),
        ];
        let found = check(&graph(nodes));
        assert!(found.iter().all(|f| f.code == "E026"));
    }
}
