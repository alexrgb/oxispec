//! Entities: the things the specs are about, kept as nodes of type `entity`.
//!
//! An entity is an order, a user, a session, a release: a thing that many
//! requirements talk about. Its node has a fenced `entity` block with a `name`
//! and, if it has them, `attributes`, written like the variables of a
//! vocabulary:
//!
//! ````markdown
//! ```entity
//! name: order
//! attributes:
//!   total: {type: int}
//!   status: {type: enum, values: [open, paid, shipped]}
//! ```
//! ````
//!
//! Each attribute is a variable named `ENTITY.ATTRIBUTE` (`order.total`), so
//! claims can talk about it. A requirement, a test or a decision says what it is
//! about with an `about` link. When an entity changes, [`affected`] finds what
//! depends on it and [`changes`] says what changed in it.
//!
//! The code that represents an entity says so with a `// spec: ENT-0001` comment above the
//! struct, class or interface (see [`crate::trace`]). [`code`] lists it, and [`check_code`]
//! looks for a field for each attribute.
//!
//! E041 an entity block cannot be read, has no name, or shares its name
//! W024 a node is `about` something that is not an entity
//! W025 an attribute of an entity has no field in the code that represents it

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::claims::{Spec, Variable, Vocabulary, variable_from};
use crate::finding::Finding;
use crate::graph::Graph;
use crate::id;
use crate::node::Node;
use crate::trace::{Annotation, Kind};

/// How far from an entity a change is followed, by default.
pub const DEFAULT_DEPTH: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Attribute {
    /// `int`, `duration`, `bool` or `enum`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The values of an enum, or `false` and `true`.
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub title: Option<String>,
    pub path: PathBuf,
    pub attributes: BTreeMap<String, Attribute>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Block {
    name: String,
    attributes: Option<BTreeMap<String, Spec>>,
}

/// Reads the `entity` block of one node, and the variables its attributes are.
fn read_block(node: &Node) -> Result<(Entity, Vec<Variable>), String> {
    let block = match node.entities.as_slice() {
        [] => return Err("an entity needs an `entity` block with a `name`".to_string()),
        [one] => one,
        _ => return Err("an entity has one `entity` block, and this has several".to_string()),
    };
    let parsed: Block =
        serde_saphyr::from_str(&block.text).map_err(|e| format!("the entity is not valid: {e}"))?;
    let name = parsed.name.trim().to_string();
    let probe = Spec {
        kind: crate::claims::Kind::Bool,
        values: None,
    };
    variable_from(&format!("{name}.x"), probe, &node.id).map_err(|_| {
        format!(
            "`{name}` is not an entity name: use letters, digits, `_` and `.`, and not a keyword"
        )
    })?;
    let mut attributes = BTreeMap::new();
    let mut variables = Vec::new();
    for (attribute, spec) in parsed.attributes.unwrap_or_default() {
        let var = variable_from(&format!("{name}.{attribute}"), spec, &node.id)?;
        attributes.insert(
            attribute,
            Attribute {
                kind: var.kind.name().to_string(),
                values: var.values.clone(),
            },
        );
        variables.push(var);
    }
    let entity = Entity {
        id: node.id.clone(),
        name,
        title: node.title.clone(),
        path: node.path.clone(),
        attributes,
    };
    Ok((entity, variables))
}

/// Every entity of the graph, and what is wrong with the ones that cannot be read.
// spec: REQ-ENTITY-0001
pub fn read(graph: &Graph) -> (Vec<Entity>, Vec<Finding>) {
    let mut entities: Vec<Entity> = Vec::new();
    let mut findings = Vec::new();
    for node in graph.nodes.iter().filter(|n| n.ty == "entity") {
        let at = node.entities.first().map(|b| b.line);
        match read_block(node) {
            Err(message) => findings.push(
                Finding::error("E041", message)
                    .at(&node.path, at.or(Some(1)))
                    .node(&node.id),
            ),
            Ok((entity, _)) => {
                if let Some(first) = entities.iter().find(|e| e.name == entity.name) {
                    findings.push(
                        Finding::error(
                            "E041",
                            format!(
                                "the entity name `{}` is also used by {}",
                                entity.name, first.id
                            ),
                        )
                        .at(&node.path, at)
                        .node(&node.id),
                    );
                } else {
                    entities.push(entity);
                }
            }
        }
    }
    (entities, findings)
}

/// Declares the attributes of an entity as variables of the vocabulary.
pub(crate) fn declare(node: &Node, vocab: &mut Vocabulary, out: &mut Vec<Finding>) {
    if node.ty != "entity" {
        return;
    }
    // An entity that cannot be read is reported by `check`, once.
    let Ok((_, variables)) = read_block(node) else {
        return;
    };
    for var in variables {
        if let Some(first) = vocab.variables.get(&var.name) {
            out.push(
                Finding::error(
                    "E018",
                    format!(
                        "variable `{}` is declared twice (first in {})",
                        var.name, first.node
                    ),
                )
                .at(&node.path, node.entities.first().map(|b| b.line))
                .node(&node.id),
            );
        } else {
            vocab.variables.insert(var.name.clone(), var);
        }
    }
}

// spec: REQ-ENTITY-0001
pub fn check(graph: &Graph) -> Vec<Finding> {
    let (_, mut findings) = read(graph);
    for node in &graph.nodes {
        for target in node.links.get("about").into_iter().flatten() {
            if let Some(found) = graph.get(target)
                && found.ty != "entity"
            {
                findings.push(
                    Finding::warning(
                        "W024",
                        format!(
                            "`about` points at {}, which is a {}, not an entity",
                            found.id, found.ty
                        ),
                    )
                    .at(&node.path, None)
                    .node(&node.id),
                );
            }
        }
    }
    findings
}

/// A node that depends on an entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Affected {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub title: Option<String>,
    /// 1 for a node that is about the entity or claims something about it.
    pub distance: u32,
    /// How it is connected, for example `about ENT-0001` or `claims order.total`.
    pub via: String,
}

/// The nodes that depend on an entity: those that link to it (any kind, `about` first
/// among them), those whose claims name one of its attributes, and, up to `depth` hops,
/// the nodes that link to those. Nearest first, then by ID.
// spec: REQ-ENTITY-0002
pub fn affected(graph: &Graph, entity: &str, depth: u32) -> Vec<Affected> {
    let Some(start) = graph.get(entity) else {
        return Vec::new();
    };
    let name = read_block(start).ok().map(|(e, _)| e.name);
    let mut found: BTreeMap<String, Affected> = BTreeMap::new();
    let mut queue: VecDeque<(String, u32)> = VecDeque::new();
    let mut seen: BTreeSet<String> = BTreeSet::from([start.id.clone()]);

    let mut add = |node: &Node, distance: u32, via: String, queue: &mut VecDeque<(String, u32)>| {
        if node.id == start.id {
            return;
        }
        let better = found.get(&node.id).is_none_or(|a| distance < a.distance);
        if better {
            found.insert(
                node.id.clone(),
                Affected {
                    id: node.id.clone(),
                    ty: node.ty.clone(),
                    title: node.title.clone(),
                    distance,
                    via,
                },
            );
        }
        if seen.insert(node.id.clone()) {
            queue.push_back((node.id.clone(), distance));
        }
    };

    if let Some(name) = &name {
        let prefix = format!("{name}.");
        let analysis = crate::claims::analyze(graph);
        for claim in analysis
            .claims
            .iter()
            .filter(|c| c.subject.starts_with(&prefix))
        {
            if let Some(node) = graph.get(&claim.node) {
                add(node, 1, format!("claims {}", claim.subject), &mut queue);
            }
        }
    }
    for (node, kind) in graph.incoming(&start.id) {
        add(node, 1, format!("{kind} {}", start.id), &mut queue);
    }
    while let Some((current, distance)) = queue.pop_front() {
        if distance >= depth {
            continue;
        }
        for (node, kind) in graph.incoming(&current) {
            add(node, distance + 1, format!("{kind} {current}"), &mut queue);
        }
    }
    let mut out: Vec<Affected> = found.into_values().collect();
    out.sort_by(|a, b| (a.distance, &a.id).cmp(&(b.distance, &b.id)));
    out
}

/// One difference between two states of an entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    /// `renamed`, `attribute-added`, `attribute-removed`, `type-changed`, `values-added`,
    /// `values-removed` or `removed`.
    pub kind: &'static str,
    pub attribute: Option<String>,
    pub detail: String,
    /// True when a reader of the entity can stop working: a removed or renamed thing, a
    /// type that changed, an enum value that is gone.
    pub breaking: bool,
}

impl Change {
    fn new(kind: &'static str, attribute: Option<&str>, detail: String, breaking: bool) -> Self {
        Change {
            kind,
            attribute: attribute.map(str::to_string),
            detail,
            breaking,
        }
    }
}

/// What changed from `old` to `new`.
// spec: REQ-ENTITY-0003
pub fn changes(old: &Entity, new: &Entity) -> Vec<Change> {
    let mut out = Vec::new();
    if old.name != new.name {
        out.push(Change::new(
            "renamed",
            None,
            format!("renamed from `{}` to `{}`", old.name, new.name),
            true,
        ));
    }
    for (name, was) in &old.attributes {
        match new.attributes.get(name) {
            None => out.push(Change::new(
                "attribute-removed",
                Some(name),
                format!("attribute `{name}` removed"),
                true,
            )),
            Some(now) if now.kind != was.kind => out.push(Change::new(
                "type-changed",
                Some(name),
                format!("`{name}` changed from {} to {}", was.kind, now.kind),
                true,
            )),
            Some(now) if now.values != was.values => {
                let gone: Vec<&str> = was
                    .values
                    .iter()
                    .filter(|v| !now.values.contains(v))
                    .map(String::as_str)
                    .collect();
                let added: Vec<&str> = now
                    .values
                    .iter()
                    .filter(|v| !was.values.contains(v))
                    .map(String::as_str)
                    .collect();
                if !gone.is_empty() {
                    out.push(Change::new(
                        "values-removed",
                        Some(name),
                        format!("`{name}` no longer allows {}", gone.join(", ")),
                        true,
                    ));
                }
                if !added.is_empty() {
                    out.push(Change::new(
                        "values-added",
                        Some(name),
                        format!("`{name}` now also allows {}", added.join(", ")),
                        false,
                    ));
                }
            }
            Some(_) => {}
        }
    }
    for name in new.attributes.keys() {
        if !old.attributes.contains_key(name) {
            out.push(Change::new(
                "attribute-added",
                Some(name),
                format!("attribute `{name}` added"),
                false,
            ));
        }
    }
    out
}

/// The change for an entity that is gone.
pub fn removed(old: &Entity) -> Vec<Change> {
    vec![Change::new(
        "removed",
        None,
        format!("entity `{}` removed", old.name),
        true,
    )]
}

/// Code that says it is about an entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Code {
    pub path: PathBuf,
    pub line: usize,
    /// The name of the struct, class, function or other code the comment is above.
    pub symbol: Option<String>,
    /// The tree-sitter kind of it, for example `struct_item`.
    pub kind: Option<String>,
    /// The fields of a type, which attributes are looked for in.
    pub fields: Vec<String>,
}

/// The code that carries `spec: ENT-...` for this entity, in the order it was found.
// spec: REQ-ENTITY-0007
pub fn code(entity: &str, annotations: &[Annotation]) -> Vec<Code> {
    annotations
        .iter()
        .filter(|a| a.kind == Kind::Implements && id::base_of(&a.target) == entity)
        .map(|a| Code {
            path: a.path.clone(),
            line: a.line,
            symbol: a.symbol.as_ref().map(|s| s.name.clone()),
            kind: a.symbol.as_ref().map(|s| s.kind.clone()),
            fields: a
                .symbol
                .as_ref()
                .map(|s| s.fields.clone())
                .unwrap_or_default(),
        })
        .collect()
}

/// A name with case, `_` and `-` left out, so `total_amount` and `totalAmount` are one.
fn normalized(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The attributes of an entity that none of its code has a field for. Nothing is said when
/// no code that names the entity defines a type with fields, since then there is nothing to
/// look in.
// spec: REQ-ENTITY-0008
pub fn missing_in_code(entity: &Entity, code: &[Code]) -> Vec<String> {
    if code.iter().all(|c| c.fields.is_empty()) {
        return Vec::new();
    }
    let have: BTreeSet<String> = code
        .iter()
        .flat_map(|c| c.fields.iter())
        .map(|f| normalized(f))
        .collect();
    entity
        .attributes
        .keys()
        .filter(|a| !have.contains(&normalized(a)))
        .cloned()
        .collect()
}

/// W025 for each attribute of an entity that the code representing it has no field for.
// spec: REQ-ENTITY-0008
pub fn check_code(graph: &Graph, annotations: &[Annotation]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for entity in read(graph).0 {
        let found = code(&entity.id, annotations);
        let Some(place) = found.iter().find(|c| !c.fields.is_empty()) else {
            continue;
        };
        for attribute in missing_in_code(&entity, &found) {
            findings.push(
                Finding::warning(
                    "W025",
                    format!(
                        "attribute `{attribute}` of {} has no field in {}",
                        entity.id,
                        place
                            .symbol
                            .as_deref()
                            .unwrap_or("the code that represents it")
                    ),
                )
                .at(&place.path, Some(place.line))
                .node(&entity.id),
            );
        }
    }
    findings
}

/// What a change to one entity touches, for a report.
#[derive(Debug, Clone, Serialize)]
pub struct Impact {
    pub id: String,
    pub name: String,
    pub changes: Vec<Change>,
    pub affected: Vec<Affected>,
    /// The code that represents the entity, which a change to it may need to follow.
    pub code: Vec<Code>,
}

/// The entities that differ between two graphs, with what depends on each. A new
/// entity depends on nothing yet, so it is not listed.
// spec: REQ-ENTITY-0002
pub fn impacts(old: &Graph, new: &Graph, depth: u32) -> Vec<Impact> {
    let (before, _) = read(old);
    let (after, _) = read(new);
    let mut out = Vec::new();
    for was in &before {
        let now = after.iter().find(|e| e.id == was.id);
        let (changes, name) = match now {
            Some(now) if now != was => (changes(was, now), now.name.clone()),
            Some(_) => continue,
            None => (removed(was), was.name.clone()),
        };
        if changes.is_empty() {
            continue;
        }
        // A claim on an attribute that is gone no longer reads in the new graph, so it
        // is found in the old one. Only the nodes that are still there are listed.
        let mut found: BTreeMap<String, Affected> = BTreeMap::new();
        for a in affected(new, &was.id, depth).into_iter().chain(
            affected(old, &was.id, depth)
                .into_iter()
                .filter(|a| new.get(&a.id).is_some()),
        ) {
            let keep = found.get(&a.id).is_none_or(|f| a.distance < f.distance);
            if keep {
                found.insert(a.id.clone(), a);
            }
        }
        let mut affected: Vec<Affected> = found.into_values().collect();
        affected.sort_by(|a, b| (a.distance, &a.id).cmp(&(b.distance, &b.id)));
        out.push(Impact {
            id: was.id.clone(),
            name,
            changes,
            affected,
            code: Vec::new(),
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// The ID of the base of a link target, for callers that match links by hand.
pub fn target_id(target: &str) -> &str {
    id::base_of(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn graph(files: &[(&str, &str)]) -> Graph {
        Graph::from_sources(
            files
                .iter()
                .map(|(p, t)| (PathBuf::from(p), t.as_bytes().to_vec())),
        )
    }

    fn entity(id: &str, block: &str) -> String {
        format!(
            "---\nid: {id}\ntype: entity\nstatus: accepted\n---\n# {id}: Thing\n\n```entity\n{block}```\n"
        )
    }

    fn requirement(id: &str, links: &str, claims: &str) -> String {
        format!(
            "---\nid: {id}\ntype: requirement\nstatus: accepted\n{links}---\n### Requirement: A rule\nThe system SHALL do it.\n\n#### Scenario: One {{#S1}}\n- **WHEN** a\n- **THEN** b\n{claims}"
        )
    }

    const ORDER: &str = "name: order\nattributes:\n  total: {type: int}\n  status: {type: enum, values: [open, paid]}\n";

    // verifies: REQ-ENTITY-0001.S1
    #[test]
    fn an_entity_declares_its_attributes_as_variables() {
        let g = graph(&[
            ("specs/e.md", &entity("ENT-0001", ORDER)),
            (
                "specs/r.md",
                &requirement(
                    "REQ-A-0001",
                    "links:\n  about: [ENT-0001]\n",
                    "\n```claims\norder.total <= 100\n```\n",
                ),
            ),
        ]);
        let (entities, findings) = read(&g);
        assert!(findings.is_empty(), "{findings:?}");
        assert_eq!(entities[0].name, "order");
        assert_eq!(entities[0].attributes["status"].values, ["open", "paid"]);
        let analysis = crate::claims::analyze(&g);
        assert!(analysis.findings.is_empty(), "{:?}", analysis.findings);
        assert!(analysis.vocabulary.variables.contains_key("order.total"));
        assert!(crate::check(&g).iter().all(|f| f.code != "E017"));
    }

    // verifies: REQ-ENTITY-0001.S2
    #[test]
    fn an_entity_that_cannot_be_read_or_repeats_a_name_is_an_error() {
        let g = graph(&[
            ("specs/a.md", &entity("ENT-0001", "attributes: {}\n")),
            ("specs/b.md", &entity("ENT-0002", ORDER)),
            ("specs/c.md", &entity("ENT-0003", ORDER)),
            (
                "specs/d.md",
                "---\nid: ENT-0004\ntype: entity\nstatus: accepted\n---\n# No block\n",
            ),
            (
                "specs/e.md",
                &entity("ENT-0005", "name: x\nattributes:\n  a: {type: enum}\n"),
            ),
        ]);
        let (_, findings) = read(&g);
        let codes: Vec<(&str, Option<&str>)> =
            findings.iter().map(|f| (f.code, f.id.as_deref())).collect();
        assert!(codes.contains(&("E041", Some("ENT-0001"))), "{codes:?}");
        assert!(codes.contains(&("E041", Some("ENT-0003"))), "{codes:?}");
        assert!(codes.contains(&("E041", Some("ENT-0004"))), "{codes:?}");
        assert!(codes.contains(&("E041", Some("ENT-0005"))), "{codes:?}");
        assert!(!codes.contains(&("E041", Some("ENT-0002"))), "{codes:?}");
    }

    // verifies: REQ-ENTITY-0001.S3
    #[test]
    fn about_a_node_that_is_not_an_entity_is_a_warning() {
        let g = graph(&[
            ("specs/e.md", &entity("ENT-0001", ORDER)),
            (
                "specs/r.md",
                &requirement("REQ-A-0001", "links:\n  about: [ENT-0001]\n", ""),
            ),
            (
                "specs/s.md",
                &requirement("REQ-A-0002", "links:\n  about: [REQ-A-0001]\n", ""),
            ),
        ]);
        let found = check(&g);
        let codes: Vec<(&str, Option<&str>)> =
            found.iter().map(|f| (f.code, f.id.as_deref())).collect();
        assert_eq!(codes, [("W024", Some("REQ-A-0002"))]);
    }

    // verifies: REQ-ENTITY-0002.S1
    #[test]
    fn what_depends_on_an_entity_is_found_nearest_first() {
        let g = graph(&[
            ("specs/e.md", &entity("ENT-0001", ORDER)),
            (
                "specs/r1.md",
                &requirement("REQ-A-0001", "links:\n  about: [ENT-0001]\n", ""),
            ),
            (
                "specs/r2.md",
                &requirement("REQ-A-0002", "", "\n```claims\norder.total <= 5\n```\n"),
            ),
            (
                "specs/r3.md",
                &requirement("REQ-A-0003", "links:\n  refines: [REQ-A-0001]\n", ""),
            ),
            ("specs/r4.md", &requirement("REQ-A-0004", "", "")),
        ]);
        let found = affected(&g, "ENT-0001", 2);
        let ids: Vec<(&str, u32)> = found.iter().map(|a| (a.id.as_str(), a.distance)).collect();
        assert_eq!(
            ids,
            [("REQ-A-0001", 1), ("REQ-A-0002", 1), ("REQ-A-0003", 2)]
        );
        assert_eq!(found[0].via, "about ENT-0001");
        assert_eq!(found[1].via, "claims order.total");
        assert_eq!(affected(&g, "ENT-0001", 1).len(), 2);
        assert!(affected(&g, "ENT-9999", 2).is_empty());
    }

    // verifies: REQ-ENTITY-0003.S1
    #[test]
    fn a_change_to_an_entity_says_what_changed_and_what_can_break() {
        let make = |block: &str| {
            let g = graph(&[("specs/e.md", &entity("ENT-0001", block))]);
            read(&g).0.remove(0)
        };
        let old = make(ORDER);
        let new = make(
            "name: order\nattributes:\n  total: {type: duration}\n  status: {type: enum, values: [paid, shipped]}\n  note: {type: bool}\n",
        );
        let found: Vec<(&str, bool)> = changes(&old, &new)
            .iter()
            .map(|c| (c.kind, c.breaking))
            .collect();
        assert_eq!(
            found,
            [
                ("values-removed", true),
                ("values-added", false),
                ("type-changed", true),
                ("attribute-added", false)
            ]
        );
        let gone = make("name: order\nattributes:\n  status: {type: enum, values: [open, paid]}\n");
        assert_eq!(changes(&old, &gone)[0].kind, "attribute-removed");
        assert!(changes(&old, &old).is_empty());
        let renamed = make(
            "name: purchase\nattributes:\n  total: {type: int}\n  status: {type: enum, values: [open, paid]}\n",
        );
        assert_eq!(changes(&old, &renamed)[0].kind, "renamed");
    }

    // verifies: REQ-ENTITY-0002.S2
    #[test]
    fn a_claim_on_an_attribute_that_is_gone_still_counts_as_a_dependent() {
        let claim = requirement("REQ-A-0002", "", "\n```claims\norder.total <= 5\n```\n");
        let old = graph(&[
            ("specs/e.md", &entity("ENT-0001", ORDER)),
            ("specs/r.md", &claim),
        ]);
        let new = graph(&[
            (
                "specs/e.md",
                &entity(
                    "ENT-0001",
                    "name: order\nattributes:\n  status: {type: enum, values: [open, paid]}\n",
                ),
            ),
            ("specs/r.md", &claim),
        ]);
        let found = impacts(&old, &new, DEFAULT_DEPTH);
        assert_eq!(found[0].changes[0].kind, "attribute-removed");
        assert_eq!(found[0].affected.len(), 1, "{:?}", found[0].affected);
        assert_eq!(found[0].affected[0].id, "REQ-A-0002");
    }

    // verifies: REQ-ENTITY-0002.S2
    #[test]
    fn impacts_list_changed_and_removed_entities_but_not_new_or_unchanged_ones() {
        let req = requirement("REQ-A-0001", "links:\n  about: [ENT-0001]\n", "");
        let old = graph(&[
            ("specs/e1.md", &entity("ENT-0001", ORDER)),
            ("specs/e2.md", &entity("ENT-0002", "name: user\n")),
            ("specs/e3.md", &entity("ENT-0003", "name: same\n")),
            ("specs/r.md", &req),
        ]);
        let new = graph(&[
            (
                "specs/e1.md",
                &entity(
                    "ENT-0001",
                    "name: order\nattributes:\n  total: {type: int}\n",
                ),
            ),
            ("specs/e3.md", &entity("ENT-0003", "name: same\n")),
            ("specs/e4.md", &entity("ENT-0004", "name: fresh\n")),
            ("specs/r.md", &req),
        ]);
        let found = impacts(&old, &new, DEFAULT_DEPTH);
        let ids: Vec<&str> = found.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["ENT-0001", "ENT-0002"]);
        assert_eq!(found[0].changes[0].kind, "attribute-removed");
        assert_eq!(found[0].affected[0].id, "REQ-A-0001");
        assert_eq!(found[1].changes[0].kind, "removed");
    }

    fn annotation(target: &str, line: usize, fields: &[&str], symbol: Option<&str>) -> Annotation {
        Annotation {
            target: target.to_string(),
            kind: Kind::Implements,
            path: PathBuf::from("src/order.rs"),
            line,
            symbol: symbol.map(|name| crate::symbol::Symbol {
                name: name.to_string(),
                kind: "struct_item".to_string(),
                start_line: line,
                end_line: line + 3,
                hash: "h".to_string(),
                fields: fields.iter().map(|f| f.to_string()).collect(),
            }),
        }
    }

    // verifies: REQ-ENTITY-0008.S1
    #[test]
    fn an_attribute_with_no_field_is_warned_about_ignoring_case_and_underscores() {
        let block = "name: order\nattributes:\n  total_amount: {type: int}\n  status: {type: bool}\n  note: {type: bool}\n";
        let g = graph(&[("specs/e.md", &entity("ENT-0001", block))]);
        let notes = [
            annotation("ENT-0001", 1, &["totalAmount", "Status"], Some("Order")),
            annotation("ENT-0002", 9, &["note"], Some("Other")),
        ];
        let code = code("ENT-0001", &notes);
        assert_eq!(code.len(), 1);
        let found = check_code(&g, &notes);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].code, "W025");
        assert!(found[0].message.contains("`note`"), "{}", found[0].message);
        assert_eq!(found[0].line, Some(1));
        assert_eq!(found[0].id.as_deref(), Some("ENT-0001"));
    }

    // verifies: REQ-ENTITY-0008.S2
    #[test]
    fn nothing_is_said_when_there_is_no_type_to_look_in() {
        let g = graph(&[("specs/e.md", &entity("ENT-0001", ORDER))]);
        let function = [annotation("ENT-0001", 1, &[], Some("pay"))];
        assert!(check_code(&g, &function).is_empty());
        assert!(check_code(&g, &[]).is_empty());
        let unbound = [annotation("ENT-0001", 1, &[], None)];
        assert!(check_code(&g, &unbound).is_empty());
    }
}
