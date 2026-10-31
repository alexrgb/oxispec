//! State machines (design sections 19 and 20, layer 2).
//!
//! A node can carry one fenced `statemachine` block, written in YAML:
//!
//! ```text
//! initial: created
//! states: [created, paid, shipped, cancelled]
//! final: [shipped, cancelled]
//! transitions:
//!   - {from: created, event: pay, to: paid, implements: [REQ-ORD-0003]}
//!   - {from: paid, event: ship, to: shipped}
//!   - {from: created, event: cancel, to: cancelled}
//! properties:
//!   - never cancel after shipped
//! ```
//!
//! Guards are free text and are not evaluated. Reachability treats every
//! transition as possible, so a counterexample is a path that the states and
//! events allow, which a guard may still forbid in practice.
//!
//! Properties are one of four forms, each checked on the whole graph:
//!
//! ```text
//! never reach S          no run enters S
//! never E after S        no run takes E once it has been in S
//! S leads to T           every run that enters S enters T later
//! E requires S           no run takes E unless it was in S before
//! ```
//!
//! A run is a path from the initial state. It may stop in a final state or in
//! a state with no transition. A violated property comes with the path that
//! breaks it.
//!
//! E020 the machine cannot be read      E021 a name is not a state of the machine
//! E022 two transitions overlap         E023 a property is violated
//! E024 a link target does not exist
//! W007 a state cannot be reached       W008 a state has no way out and is not final

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use serde::Deserialize;

use crate::check::{LinkResolution, resolve_link};
use crate::claims::is_inactive;
use crate::finding::Finding;
use crate::graph::Graph;
use crate::node::{Node, OneOrMany};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub from: String,
    pub event: String,
    pub to: String,
    pub guard: Option<String>,
    pub implements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    NeverReach(String),
    NeverAfter { event: String, state: String },
    LeadsTo { from: String, to: String },
    Requires { event: String, state: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub text: String,
    pub rule: Rule,
    /// The requirement the property comes from, if it says so.
    pub source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Machine {
    pub node: String,
    pub path: PathBuf,
    /// First line of the block's content.
    pub line: usize,
    pub active: bool,
    pub initial: String,
    pub states: Vec<String>,
    pub finals: Vec<String>,
    pub transitions: Vec<Transition>,
    pub properties: Vec<Property>,
}

#[derive(Debug, Default)]
pub struct Analysis {
    pub machines: Vec<Machine>,
    /// Machines that cannot be read (E020).
    pub findings: Vec<Finding>,
}

/// One step of a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub from: String,
    pub event: String,
    pub to: String,
}

/// A path from the initial state that breaks a property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counterexample {
    pub steps: Vec<Step>,
    /// The last step can be repeated, so the run never reaches its goal.
    pub repeats: bool,
}

/// One generated test: a path from the initial state that ends with a transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestCase {
    pub name: String,
    pub steps: Vec<Step>,
    pub implements: Vec<String>,
}

// ---------------------------------------------------------------- reading

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    initial: String,
    states: Vec<String>,
    #[serde(rename = "final", default)]
    finals: Vec<String>,
    #[serde(default)]
    transitions: Vec<RawTransition>,
    #[serde(default)]
    properties: Vec<RawProperty>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTransition {
    from: String,
    event: String,
    to: String,
    guard: Option<String>,
    implements: Option<OneOrMany>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawProperty {
    Text(String),
    Full {
        rule: String,
        #[serde(rename = "from")]
        source: Option<String>,
    },
}

fn is_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

fn parse_rule(text: &str) -> Result<Rule, String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let names_ok = |names: &[&str]| names.iter().all(|n| is_name(n));
    let rule = match words.as_slice() {
        ["never", "reach", s] if names_ok(&[s]) => Rule::NeverReach(s.to_string()),
        ["never", e, "after", s] if names_ok(&[e, s]) => Rule::NeverAfter {
            event: e.to_string(),
            state: s.to_string(),
        },
        [s, "leads", "to", t] if names_ok(&[s, t]) => Rule::LeadsTo {
            from: s.to_string(),
            to: t.to_string(),
        },
        [e, "requires", s] if names_ok(&[e, s]) => Rule::Requires {
            event: e.to_string(),
            state: s.to_string(),
        },
        _ => {
            return Err(format!(
                "property `{text}` is not one of `never reach S`, `never E after S`, `S leads to T`, `E requires S`"
            ));
        }
    };
    Ok(rule)
}

fn read_machine(node: &Node, out: &mut Vec<Finding>) -> Option<Machine> {
    let bad = |line: usize, message: String| {
        Finding::error("E020", format!("state machine {}: {message}", node.id))
            .at(&node.path, Some(line))
            .node(&node.id)
    };
    let first = node.machines.first()?;
    if node.machines.len() > 1 {
        out.push(bad(
            node.machines[1].line,
            "a node holds one `statemachine` block".to_string(),
        ));
        return None;
    }
    let raw: Raw = match serde_saphyr::from_str(&first.text) {
        Ok(raw) => raw,
        Err(e) => {
            out.push(bad(first.line, format!("the block is not valid: {e}")));
            return None;
        }
    };
    let bad_name = {
        let mut named = std::iter::once(&raw.initial)
            .chain(&raw.states)
            .chain(&raw.finals)
            .chain(
                raw.transitions
                    .iter()
                    .flat_map(|t| [&t.from, &t.event, &t.to]),
            );
        named.find(|n| !is_name(n)).cloned()
    };
    if let Some(name) = bad_name {
        out.push(bad(
            first.line,
            format!("`{name}` is not a name: use letters, digits, `_`, `.` and `-`"),
        ));
        return None;
    }
    let mut seen = HashSet::new();
    if let Some(state) = raw.states.iter().find(|s| !seen.insert(s.as_str())) {
        out.push(bad(first.line, format!("state `{state}` is listed twice")));
        return None;
    }
    let mut properties = Vec::new();
    for raw_property in raw.properties {
        let (text, source) = match raw_property {
            RawProperty::Text(t) => (t, None),
            RawProperty::Full { rule, source } => (rule, source),
        };
        match parse_rule(&text) {
            Ok(rule) => properties.push(Property {
                text: text.split_whitespace().collect::<Vec<_>>().join(" "),
                rule,
                source,
            }),
            Err(message) => {
                out.push(bad(first.line, message));
                return None;
            }
        }
    }
    Some(Machine {
        node: node.id.clone(),
        path: node.path.clone(),
        line: first.line,
        active: !is_inactive(node.status.as_deref()),
        initial: raw.initial,
        states: raw.states,
        finals: raw.finals,
        transitions: raw
            .transitions
            .into_iter()
            .map(|t| Transition {
                from: t.from,
                event: t.event,
                to: t.to,
                guard: t
                    .guard
                    .map(|g| g.trim().to_string())
                    .filter(|g| !g.is_empty()),
                implements: t.implements.map(OneOrMany::into_vec).unwrap_or_default(),
            })
            .collect(),
        properties,
    })
}

pub fn analyze(graph: &Graph) -> Analysis {
    let mut analysis = Analysis::default();
    for node in graph.nodes.iter().filter(|n| !n.machines.is_empty()) {
        if let Some(machine) = read_machine(node, &mut analysis.findings) {
            analysis.machines.push(machine);
        }
    }
    analysis
}

// ---------------------------------------------------------------- the graph of states

/// A machine as numbers, with only the transitions between known states.
struct Net<'a> {
    machine: &'a Machine,
    index: HashMap<&'a str, usize>,
    /// Per state, the transitions out of it: (transition position, target).
    out: Vec<Vec<(usize, usize)>>,
    initial: usize,
    is_final: Vec<bool>,
}

impl<'a> Net<'a> {
    /// None when the initial state is not a state, which E021 reports.
    fn new(machine: &'a Machine) -> Option<Net<'a>> {
        let index: HashMap<&str, usize> = machine
            .states
            .iter()
            .enumerate()
            .map(|(i, s)| (s.as_str(), i))
            .collect();
        let initial = *index.get(machine.initial.as_str())?;
        let mut out = vec![Vec::new(); machine.states.len()];
        for (position, t) in machine.transitions.iter().enumerate() {
            if let (Some(&from), Some(&to)) = (index.get(t.from.as_str()), index.get(t.to.as_str()))
            {
                out[from].push((position, to));
            }
        }
        let mut is_final = vec![false; machine.states.len()];
        for f in &machine.finals {
            if let Some(&i) = index.get(f.as_str()) {
                is_final[i] = true;
            }
        }
        Some(Net {
            machine,
            index,
            out,
            initial,
            is_final,
        })
    }

    fn state(&self, name: &str) -> Option<usize> {
        self.index.get(name).copied()
    }

    fn step(&self, position: usize) -> Step {
        let t = &self.machine.transitions[position];
        Step {
            from: t.from.clone(),
            event: t.event.clone(),
            to: t.to.clone(),
        }
    }

    /// Breadth-first search from `start` over the states allowed by `allowed`.
    /// Returns, per state, the transition that first reached it.
    fn search(&self, start: usize, allowed: impl Fn(usize) -> bool) -> Vec<Option<Option<usize>>> {
        // None: not reached. Some(None): the start. Some(Some(p)): reached by transition p.
        let mut reached: Vec<Option<Option<usize>>> = vec![None; self.out.len()];
        reached[start] = Some(None);
        let mut queue = VecDeque::from([start]);
        while let Some(s) = queue.pop_front() {
            for &(position, to) in &self.out[s] {
                if reached[to].is_none() && allowed(to) {
                    reached[to] = Some(Some(position));
                    queue.push_back(to);
                }
            }
        }
        reached
    }

    /// The shortest path from the search's start to `to`.
    fn path(&self, reached: &[Option<Option<usize>>], to: usize) -> Vec<Step> {
        let mut steps = Vec::new();
        let mut at = to;
        while let Some(Some(position)) = reached[at] {
            steps.push(self.step(position));
            at = self.index[self.machine.transitions[position].from.as_str()];
        }
        steps.reverse();
        steps
    }

    fn reachable(&self) -> Vec<Option<Option<usize>>> {
        self.search(self.initial, |_| true)
    }
}

// ---------------------------------------------------------------- properties

/// The path that breaks the property, or None when it holds.
pub fn violation(machine: &Machine, rule: &Rule) -> Option<Counterexample> {
    let net = Net::new(machine)?;
    let from_start = net.reachable();
    let found = |steps: Vec<Step>| Counterexample {
        steps,
        repeats: false,
    };
    match rule {
        Rule::NeverReach(s) => {
            let s = net.state(s)?;
            from_start[s].map(|_| found(net.path(&from_start, s)))
        }
        Rule::NeverAfter { event, state } => {
            let s = net.state(state)?;
            from_start[s]?;
            let after = net.search(s, |_| true);
            for (x, reach) in after.iter().enumerate() {
                if reach.is_none() {
                    continue;
                }
                if let Some(&(position, _)) = net.out[x]
                    .iter()
                    .find(|&&(p, _)| machine.transitions[p].event == *event)
                {
                    let mut steps = net.path(&from_start, s);
                    steps.extend(net.path(&after, x));
                    steps.push(net.step(position));
                    return Some(found(steps));
                }
            }
            None
        }
        Rule::Requires { event, state } => {
            let s = net.state(state)?;
            // Search over (state, has been in `s`).
            let n = net.out.len();
            let key = |state: usize, seen: bool| state * 2 + usize::from(seen);
            let mut how: Vec<Option<Option<(usize, usize)>>> = vec![None; n * 2];
            let start = key(net.initial, net.initial == s);
            how[start] = Some(None);
            let mut queue = VecDeque::from([(net.initial, net.initial == s)]);
            while let Some((x, seen)) = queue.pop_front() {
                for &(position, to) in &net.out[x] {
                    if !seen && machine.transitions[position].event == *event {
                        let mut steps = Vec::new();
                        let mut at = key(x, seen);
                        while let Some(Some((previous, p))) = how[at] {
                            steps.push(net.step(p));
                            at = previous;
                        }
                        steps.reverse();
                        steps.push(net.step(position));
                        return Some(found(steps));
                    }
                    let next_seen = seen || to == s;
                    let k = key(to, next_seen);
                    if how[k].is_none() {
                        how[k] = Some(Some((key(x, seen), position)));
                        queue.push_back((to, next_seen));
                    }
                }
            }
            None
        }
        Rule::LeadsTo { from, to } => {
            let s = net.state(from)?;
            let t = net.state(to)?;
            if s == t {
                return None;
            }
            from_start[s]?;
            // Explore from `s` without entering `t`. A run that ends there, or
            // goes round a cycle there, never reaches `t`.
            let region = net.search(s, |x| x != t);
            let mut prefix = net.path(&from_start, s);
            let ends = |x: usize| net.is_final[x] || net.out[x].is_empty();
            if let Some(x) = (0..net.out.len()).find(|&x| region[x].is_some() && ends(x)) {
                prefix.extend(net.path(&region, x));
                return Some(found(prefix));
            }
            // A cycle inside the region, found with a depth-first search.
            let mut color = vec![0u8; net.out.len()];
            let mut stack: Vec<(usize, usize)> = vec![(s, 0)];
            let mut trail: Vec<usize> = Vec::new();
            color[s] = 1;
            while let Some(&(x, next)) = stack.last() {
                if let Some(&(position, y)) = net.out[x].get(next) {
                    if let Some(top) = stack.last_mut() {
                        top.1 += 1;
                    }
                    if y == t {
                        continue;
                    }
                    if color[y] == 1 {
                        // Back edge x -> y: the trail leads to x, then the edge repeats.
                        let mut steps = prefix;
                        steps.extend(trail.iter().map(|&p| net.step(p)));
                        steps.push(net.step(position));
                        return Some(Counterexample {
                            steps,
                            repeats: true,
                        });
                    }
                    if color[y] == 0 {
                        color[y] = 1;
                        trail.push(position);
                        stack.push((y, 0));
                    }
                } else {
                    color[x] = 2;
                    stack.pop();
                    trail.pop();
                }
            }
            None
        }
    }
}

/// `created -pay-> paid -ship-> shipped`
pub fn render_path(initial: &str, steps: &[Step]) -> String {
    let mut out = steps
        .first()
        .map_or(initial, |s| s.from.as_str())
        .to_string();
    for step in steps {
        out.push_str(&format!(" -{}-> {}", step.event, step.to));
    }
    out
}

// ---------------------------------------------------------------- checks

fn at_machine(finding: Finding, machine: &Machine) -> Finding {
    finding
        .at(&machine.path, Some(machine.line))
        .node(&machine.node)
}

fn guards_overlap(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => true,
    }
}

fn unknown_states(machine: &Machine, out: &mut Vec<Finding>) {
    let known: HashSet<&str> = machine.states.iter().map(String::as_str).collect();
    let mut report = |what: String, name: &str| {
        if !known.contains(name) {
            out.push(at_machine(
                Finding::error(
                    "E021",
                    format!(
                        "state machine {}: {what} names `{name}`, which is not in `states`",
                        machine.node
                    ),
                ),
                machine,
            ));
        }
    };
    report("`initial`".to_string(), &machine.initial);
    for f in &machine.finals {
        report("`final`".to_string(), f);
    }
    for t in &machine.transitions {
        let what = format!("the transition `{}` on `{}`", t.from, t.event);
        report(what.clone(), &t.from);
        report(what, &t.to);
    }
    for p in &machine.properties {
        let names: Vec<&String> = match &p.rule {
            Rule::NeverReach(s)
            | Rule::NeverAfter { state: s, .. }
            | Rule::Requires { state: s, .. } => {
                vec![s]
            }
            Rule::LeadsTo { from, to } => vec![from, to],
        };
        for name in names {
            report(format!("the property `{}`", p.text), name);
        }
    }
}

fn nondeterminism(machine: &Machine, out: &mut Vec<Finding>) {
    let mut by_key: HashMap<(&str, &str), Vec<&Transition>> = HashMap::new();
    for t in &machine.transitions {
        by_key.entry((&t.from, &t.event)).or_default().push(t);
    }
    let mut keys: Vec<_> = by_key.keys().copied().collect();
    keys.sort_unstable();
    for key in keys {
        let list = &by_key[&key];
        for (i, a) in list.iter().enumerate() {
            for b in &list[i + 1..] {
                if guards_overlap(&a.guard, &b.guard) {
                    let why = match (&a.guard, &b.guard) {
                        (Some(g), _) => format!("both are guarded by `{g}`"),
                        _ => "one of them has no guard".to_string(),
                    };
                    out.push(at_machine(
                        Finding::error(
                            "E022",
                            format!(
                                "state machine {}: in `{}` the event `{}` leads to `{}` and to `{}`, and {why}",
                                machine.node, a.from, a.event, a.to, b.to
                            ),
                        ),
                        machine,
                    ));
                }
            }
        }
    }
}

fn links(graph: &Graph, machine: &Machine, out: &mut Vec<Finding>) {
    let transition_targets = machine
        .transitions
        .iter()
        .flat_map(|t| t.implements.iter().map(move |i| (i, "implements")));
    let property_sources = machine
        .properties
        .iter()
        .filter_map(|p| p.source.as_ref().map(|s| (s, "from")));
    for (target, field) in transition_targets.chain(property_sources) {
        let problem = match resolve_link(graph, target) {
            LinkResolution::Ok | LinkResolution::External => continue,
            LinkResolution::Invalid => format!("`{target}` is not a valid ID"),
            LinkResolution::MissingNode => format!("`{target}` does not exist"),
            LinkResolution::MissingScenario(anchor) => {
                format!("`{target}` has no scenario `{anchor}`")
            }
        };
        out.push(at_machine(
            Finding::error(
                "E024",
                format!("state machine {}: `{field}` {problem}", machine.node),
            ),
            machine,
        ));
    }
}

fn structure(machine: &Machine, net: &Net, out: &mut Vec<Finding>) {
    let reached = net.reachable();
    for (i, state) in machine.states.iter().enumerate() {
        if reached[i].is_none() {
            out.push(at_machine(
                Finding::warning(
                    "W007",
                    format!(
                        "state machine {}: state `{state}` cannot be reached from `{}`",
                        machine.node, machine.initial
                    ),
                ),
                machine,
            ));
        } else if net.out[i].is_empty() && !net.is_final[i] {
            out.push(at_machine(
                Finding::warning(
                    "W008",
                    format!(
                        "state machine {}: state `{state}` has no way out and is not final",
                        machine.node
                    ),
                ),
                machine,
            ));
        }
    }
}

fn properties(machine: &Machine, out: &mut Vec<Finding>) {
    for property in &machine.properties {
        let Some(example) = violation(machine, &property.rule) else {
            continue;
        };
        let source = property
            .source
            .as_ref()
            .map_or(String::new(), |s| format!(" (from {s})"));
        let repeats = if example.repeats {
            ", and the last step can repeat forever"
        } else {
            ""
        };
        out.push(at_machine(
            Finding::error(
                "E023",
                format!(
                    "state machine {}: the property `{}`{source} is violated by {}{repeats}",
                    machine.node,
                    property.text,
                    render_path(&machine.initial, &example.steps)
                ),
            ),
            machine,
        ));
    }
}

pub fn check(graph: &Graph) -> Vec<Finding> {
    let analysis = analyze(graph);
    let mut findings = analysis.findings;
    for machine in analysis.machines.iter().filter(|m| m.active) {
        unknown_states(machine, &mut findings);
        nondeterminism(machine, &mut findings);
        links(graph, machine, &mut findings);
        if let Some(net) = Net::new(machine) {
            structure(machine, &net, &mut findings);
        }
        properties(machine, &mut findings);
    }
    findings
}

// ---------------------------------------------------------------- outputs

/// A Mermaid `stateDiagram-v2`.
pub fn diagram(machine: &Machine) -> String {
    let mut out = String::from("stateDiagram-v2\n");
    out.push_str(&format!("    [*] --> {}\n", machine.initial));
    for t in &machine.transitions {
        let label = match &t.guard {
            Some(g) => format!("{} [{}]", t.event, g.replace(':', " ")),
            None => t.event.clone(),
        };
        out.push_str(&format!("    {} --> {}: {label}\n", t.from, t.to));
    }
    for f in &machine.finals {
        out.push_str(&format!("    {f} --> [*]\n"));
    }
    out
}

/// One test per transition: the shortest path from the initial state, then the
/// transition itself. Transitions that cannot be reached have no test.
pub fn tests(machine: &Machine) -> Vec<TestCase> {
    let Some(net) = Net::new(machine) else {
        return Vec::new();
    };
    let reached = net.reachable();
    let mut cases = Vec::new();
    for (position, t) in machine.transitions.iter().enumerate() {
        let (Some(from), Some(_)) = (net.state(&t.from), net.state(&t.to)) else {
            continue;
        };
        if reached[from].is_none() {
            continue;
        }
        let mut steps = net.path(&reached, from);
        steps.push(net.step(position));
        cases.push(TestCase {
            name: format!("{}: {} on {} to {}", machine.node, t.from, t.event, t.to),
            steps,
            implements: t.implements.clone(),
        });
    }
    cases
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests_of_machines {
    use super::*;
    use crate::parse::parse_file_text;
    use std::path::Path;

    const ORDER: &str = "initial: created
states: [created, paid, shipped, delivered, cancelled]
final: [delivered, cancelled]
transitions:
  - {from: created, event: pay, to: paid}
  - {from: paid, event: ship, to: shipped}
  - {from: shipped, event: deliver, to: delivered}
  - {from: created, event: cancel, to: cancelled}
  - {from: paid, event: cancel, to: cancelled}
";

    fn node(id: &str, status: &str, block: &str) -> Node {
        let text = format!(
            "---\nid: {id}\ntype: state-machine\nstatus: {status}\n---\n# M\n\n```statemachine\n{block}```\n"
        );
        let (node, findings) = parse_file_text(Path::new(&format!("{id}.md")), &text);
        assert!(findings.is_empty(), "{findings:?}");
        node.expect("a node")
    }

    fn graph(block: &str) -> Graph {
        Graph::from_nodes(vec![node("SM-ORDER", "accepted", block)])
    }

    fn machine(block: &str) -> Machine {
        let g = graph(block);
        let a = analyze(&g);
        assert!(a.findings.is_empty(), "{:?}", a.findings);
        a.machines.into_iter().next().expect("a machine")
    }

    fn codes(findings: &[Finding]) -> Vec<&'static str> {
        findings.iter().map(|f| f.code).collect()
    }

    fn with_properties(extra: &str) -> String {
        format!("{ORDER}properties:\n{extra}")
    }

    fn broken(rule: &str) -> Option<String> {
        let m = machine(&with_properties(&format!("  - {rule}\n")));
        let rule = m.properties[0].rule.clone();
        violation(&m, &rule).map(|c| {
            let text = render_path(&m.initial, &c.steps);
            if c.repeats {
                format!("{text} (repeats)")
            } else {
                text
            }
        })
    }

    #[test]
    fn a_sound_machine_has_no_findings() {
        assert!(check(&graph(ORDER)).is_empty());
    }

    #[test]
    fn unreadable_blocks_are_e020() {
        for block in [
            "initial: a\n",
            "initial: a\nstates: [a]\nwhat: 1\n",
            "initial: a\nstates: [a, a]\n",
            "initial: a b\nstates: [a]\n",
            "initial: a\nstates: [a]\nproperties:\n  - sometimes reach a\n",
            "states: [\n",
        ] {
            let found = check(&graph(block));
            assert_eq!(codes(&found), vec!["E020"], "{block:?}: {found:?}");
        }
    }

    #[test]
    fn two_blocks_in_one_node_are_e020() {
        let text = "---\nid: SM-A\ntype: state-machine\n---\n```statemachine\ninitial: a\nstates: [a]\n```\n```statemachine\ninitial: a\nstates: [a]\n```\n";
        let (node, _) = parse_file_text(Path::new("a.md"), text);
        let found = check(&Graph::from_nodes(vec![node.unwrap()]));
        assert_eq!(codes(&found), vec!["E020"]);
    }

    #[test]
    fn unknown_states_are_named_where_they_appear() {
        let block = "initial: start\nstates: [a]\nfinal: [b]\ntransitions:\n  - {from: a, event: go, to: c}\nproperties:\n  - never reach d\n";
        let found = check(&graph(block));
        assert_eq!(codes(&found).iter().filter(|c| **c == "E021").count(), 4);
        let text: Vec<_> = found.iter().map(|f| f.message.as_str()).collect();
        assert!(text.iter().any(|m| m.contains("`initial` names `start`")));
        assert!(text.iter().any(|m| m.contains("`final` names `b`")));
        assert!(text.iter().any(|m| m.contains("on `go` names `c`")));
        assert!(text.iter().any(|m| m.contains("`never reach d` names `d`")));
    }

    #[test]
    fn unreachable_and_stuck_states_are_warnings() {
        let block = "initial: a\nstates: [a, b, c, d]\nfinal: [d]\ntransitions:\n  - {from: a, event: go, to: b}\n  - {from: c, event: go, to: d}\n";
        let found = check(&graph(block));
        let messages: Vec<_> = found.iter().map(|f| (f.code, f.message.clone())).collect();
        assert!(
            messages
                .iter()
                .any(|(c, m)| *c == "W007" && m.contains("`c`"))
        );
        assert!(
            messages
                .iter()
                .any(|(c, m)| *c == "W007" && m.contains("`d`"))
        );
        assert!(
            messages
                .iter()
                .any(|(c, m)| *c == "W008" && m.contains("`b`"))
        );
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn same_event_with_overlapping_guards_is_nondeterministic() {
        let base = "initial: a\nstates: [a, b, c]\nfinal: [b, c]\ntransitions:\n";
        let no_guards =
            format!("{base}  - {{from: a, event: go, to: b}}\n  - {{from: a, event: go, to: c}}\n");
        assert_eq!(codes(&check(&graph(&no_guards))), vec!["E022"]);
        let one = format!(
            "{base}  - {{from: a, event: go, to: b, guard: ok}}\n  - {{from: a, event: go, to: c}}\n"
        );
        assert_eq!(codes(&check(&graph(&one))), vec!["E022"]);
        let same = format!(
            "{base}  - {{from: a, event: go, to: b, guard: Ok}}\n  - {{from: a, event: go, to: c, guard: ok}}\n"
        );
        assert_eq!(codes(&check(&graph(&same))), vec!["E022"]);
        let apart = format!(
            "{base}  - {{from: a, event: go, to: b, guard: ok}}\n  - {{from: a, event: go, to: c, guard: not ok}}\n"
        );
        assert!(check(&graph(&apart)).is_empty());
    }

    #[test]
    fn never_reach_gives_the_shortest_path() {
        assert_eq!(
            broken("never reach delivered").as_deref(),
            Some("created -pay-> paid -ship-> shipped -deliver-> delivered")
        );
        assert_eq!(broken("never reach created").as_deref(), Some("created"));
    }

    #[test]
    fn never_after_finds_the_event_once_the_state_was_visited() {
        assert_eq!(broken("never cancel after delivered"), None);
        assert_eq!(broken("never cancel after shipped"), None);
        assert_eq!(
            broken("never cancel after paid").as_deref(),
            Some("created -pay-> paid -cancel-> cancelled")
        );
        assert_eq!(
            broken("never ship after created").as_deref(),
            Some("created -pay-> paid -ship-> shipped")
        );
    }

    #[test]
    fn requires_finds_a_run_that_skips_the_state() {
        assert_eq!(broken("ship requires paid"), None);
        assert_eq!(broken("deliver requires paid"), None);
        assert_eq!(
            broken("cancel requires paid").as_deref(),
            Some("created -cancel-> cancelled")
        );
        assert_eq!(broken("pay requires created"), None);
    }

    #[test]
    fn leads_to_finds_runs_that_end_or_loop() {
        assert_eq!(
            broken("paid leads to delivered").as_deref(),
            Some("created -pay-> paid -cancel-> cancelled")
        );
        assert_eq!(broken("shipped leads to delivered"), None);
        assert_eq!(broken("created leads to created"), None);

        let looping = "initial: a\nstates: [a, b, c]\nfinal: [c]\ntransitions:\n  - {from: a, event: go, to: b}\n  - {from: b, event: back, to: a}\n  - {from: b, event: done, to: c}\nproperties:\n  - a leads to c\n";
        let m = machine(looping);
        let c = violation(&m, &m.properties[0].rule).expect("a loop");
        assert!(c.repeats);
        assert_eq!(render_path(&m.initial, &c.steps), "a -go-> b -back-> a");
    }

    #[test]
    fn leads_to_holds_when_every_way_out_reaches_the_goal() {
        let block = "initial: a\nstates: [a, b, c]\nfinal: [c]\ntransitions:\n  - {from: a, event: x, to: b}\n  - {from: a, event: y, to: c}\n  - {from: b, event: z, to: c}\nproperties:\n  - a leads to c\n";
        assert!(check(&graph(block)).is_empty());
    }

    #[test]
    fn properties_about_unreachable_states_hold_vacuously() {
        let block = "initial: a\nstates: [a, b, c]\nfinal: [a]\ntransitions:\n  - {from: b, event: x, to: c}\nproperties:\n  - never reach c\n  - b leads to a\n  - never x after b\n  - x requires a\n";
        let found = check(&graph(block));
        assert!(found.iter().all(|f| f.code != "E023"), "{found:?}");
    }

    #[test]
    fn a_violated_property_is_e023_and_names_the_requirement() {
        let block = format!(
            "{ORDER}properties:\n  - {{rule: never cancel after paid, from: REQ-ORD-0009}}\n"
        );
        let mut nodes = vec![node("SM-ORDER", "accepted", &block)];
        let text = "---\nid: REQ-ORD-0009\ntype: requirement\n---\n### Requirement: R\nIt MUST.\n\n#### Scenario: S\n- **WHEN** a\n- **THEN** b\n";
        nodes.push(parse_file_text(Path::new("r.md"), text).0.unwrap());
        let found = check(&Graph::from_nodes(nodes));
        assert_eq!(codes(&found), vec!["E023"]);
        assert_eq!(
            found[0].message,
            "state machine SM-ORDER: the property `never cancel after paid` (from REQ-ORD-0009) is violated by created -pay-> paid -cancel-> cancelled"
        );
        assert!(found[0].message.chars().all(|c| c != '\n'));
    }

    #[test]
    fn links_to_missing_nodes_are_e024() {
        let block = "initial: a\nstates: [a, b]\nfinal: [b]\ntransitions:\n  - {from: a, event: go, to: b, implements: [REQ-NOPE-0001]}\n";
        assert_eq!(codes(&check(&graph(block))), vec!["E024"]);
    }

    #[test]
    fn machines_that_are_not_active_are_not_checked() {
        let g = Graph::from_nodes(vec![node(
            "SM-OLD",
            "superseded",
            "initial: a\nstates: [a, b]\n",
        )]);
        assert!(check(&g).is_empty());
    }

    #[test]
    fn the_diagram_lists_every_transition() {
        let m = machine(ORDER);
        let d = diagram(&m);
        assert!(d.starts_with("stateDiagram-v2\n    [*] --> created\n"));
        assert!(d.contains("    created --> paid: pay\n"));
        assert!(d.contains("    delivered --> [*]\n"));
        let guarded = machine(
            "initial: a\nstates: [a, b]\ntransitions:\n  - {from: a, event: go, to: b, guard: \"x: y\"}\n",
        );
        assert!(diagram(&guarded).contains("a --> b: go [x  y]"));
    }

    #[test]
    fn there_is_one_test_per_reachable_transition() {
        let m = machine(ORDER);
        let cases = tests(&m);
        assert_eq!(cases.len(), 5);
        let last = &cases[2];
        assert_eq!(last.name, "SM-ORDER: shipped on deliver to delivered");
        assert_eq!(
            render_path(&m.initial, &last.steps),
            "created -pay-> paid -ship-> shipped -deliver-> delivered"
        );
        let cut = machine(
            "initial: a\nstates: [a, b, c]\ntransitions:\n  - {from: b, event: x, to: c}\n",
        );
        assert!(tests(&cut).is_empty());
    }

    #[test]
    fn the_longest_machines_stay_fast() {
        let n = 3000;
        let states: Vec<String> = (0..n).map(|i| format!("s{i}")).collect();
        let mut block = format!(
            "initial: s0\nstates: [{}]\nfinal: [s{}]\ntransitions:\n",
            states.join(", "),
            n - 1
        );
        for i in 0..n - 1 {
            block.push_str(&format!(
                "  - {{from: s{i}, event: next, to: s{}}}\n",
                i + 1
            ));
            block.push_str(&format!(
                "  - {{from: s{i}, event: skip, to: s{}}}\n",
                (i + 7).min(n - 1)
            ));
        }
        block.push_str("properties:\n  - s0 leads to s2999\n  - next requires s0\n  - never skip after s2999\n  - never reach s2999\n");
        let found = check(&graph(&block));
        assert_eq!(codes(&found), vec!["E023"]);
    }
}
