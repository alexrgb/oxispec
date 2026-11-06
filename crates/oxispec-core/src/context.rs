//! What is near a node in the graph, and a bundle of that text sized for an
//! agent's context window (design section 10, "context packs").

use std::collections::{HashMap, VecDeque};

use crate::graph::Graph;
use crate::id;
use crate::node::Node;

/// A node found by walking links from a starting node.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Neighbor {
    pub id: String,
    /// Link hops from the starting node.
    pub distance: u32,
    /// The kind of the link that was followed to get here.
    pub via: String,
    /// The node on the other end of that link.
    pub from: String,
    /// True when `from` links to this node, false when this node links to `from`.
    pub incoming: bool,
}

// spec: REQ-CONTEXT-0001
/// Nodes within `depth` link hops of `seed`, following links in both directions,
/// nearest first. `kinds` limits the links that are followed. At most `limit` are returned.
pub fn neighbors(
    graph: &Graph,
    seed: &str,
    depth: u32,
    kinds: Option<&[String]>,
    limit: usize,
) -> Vec<Neighbor> {
    let Some(start) = graph.get(seed) else {
        return Vec::new();
    };
    let allowed = |kind: &str| kinds.is_none_or(|k| k.iter().any(|x| x == kind));
    let mut seen: HashMap<String, ()> = HashMap::new();
    seen.insert(start.id.clone(), ());
    let mut queue: VecDeque<(String, u32)> = VecDeque::from([(start.id.clone(), 0)]);
    let mut out = Vec::new();
    while let Some((current, distance)) = queue.pop_front() {
        if distance >= depth || out.len() >= limit {
            break;
        }
        let Some(node) = graph.get(&current) else {
            continue;
        };
        let mut next: Vec<Neighbor> = Vec::new();
        for (kind, targets) in &node.links {
            if !allowed(kind) {
                continue;
            }
            for target in targets {
                if let Some(other) = graph.get(id::base_of(target)) {
                    next.push(Neighbor {
                        id: other.id.clone(),
                        distance: distance + 1,
                        via: kind.clone(),
                        from: current.clone(),
                        incoming: false,
                    });
                }
            }
        }
        for (other, kind) in graph.incoming(&current) {
            if allowed(kind) {
                next.push(Neighbor {
                    id: other.id.clone(),
                    distance: distance + 1,
                    via: kind.to_string(),
                    from: current.clone(),
                    incoming: true,
                });
            }
        }
        next.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.via.cmp(&b.via)));
        for n in next {
            if seen.insert(n.id.clone(), ()).is_none() {
                queue.push_back((n.id.clone(), distance + 1));
                out.push(n);
                if out.len() >= limit {
                    break;
                }
            }
        }
    }
    out
}

/// A rough size in tokens: one for every four characters.
pub fn tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

/// A node whose text is in the pack.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Item {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub title: Option<String>,
    pub status: Option<String>,
    /// 0 for a node that was asked for.
    pub distance: u32,
    /// How it is connected, for example `decided-by from REQ-AUTH-0001`.
    pub connection: Option<String>,
    pub text: String,
    pub tokens: usize,
    /// The text was cut to fit the budget.
    pub truncated: bool,
}

/// A related node that did not fit, so that the reader knows it exists.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Pointer {
    pub id: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub title: Option<String>,
    pub distance: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Pack {
    pub budget: usize,
    pub used: usize,
    pub items: Vec<Item>,
    pub pointers: Vec<Pointer>,
    /// Related nodes beyond the pointers that are listed.
    pub more: usize,
}

/// At most this many pointers are listed.
const MAX_POINTERS: usize = 50;
/// At most this many nodes are ranked, which keeps a very connected node cheap.
const MAX_CANDIDATES: usize = 2000;

/// What matters first when two nodes are equally near: the requirement itself,
/// the decision behind it, the rules it follows, then the rest.
fn rank(ty: &str) -> u8 {
    match ty {
        "requirement" => 0,
        "adr" => 1,
        "standard" => 2,
        "decision" => 3,
        "change" => 5,
        _ => 4,
    }
}

/// Cuts `text` to about `max_tokens`, at a line end when there is one nearby.
fn cut(text: &str, max_tokens: usize) -> String {
    // Leave room for the note that says the text was cut.
    let max_chars = max_tokens.saturating_sub(8) * 4;
    let head: String = text.chars().take(max_chars).collect();
    let end = head
        .rfind('\n')
        .filter(|&i| i > head.len() / 2)
        .unwrap_or(head.len());
    format!("{}\n[cut to fit the budget]\n", head[..end].trim_end())
}

// spec: REQ-CONTEXT-0002
/// The nodes asked for, then their neighbors up to `depth`, nearest first, with their
/// text, until `budget` tokens are used. What does not fit is listed as a pointer.
/// `text_of` gives the text of a node, or `None` when it cannot be read.
pub fn pack(
    graph: &Graph,
    seeds: &[String],
    depth: u32,
    budget: usize,
    text_of: &dyn Fn(&Node) -> Option<String>,
) -> Pack {
    struct Candidate<'a> {
        node: &'a Node,
        distance: u32,
        connection: Option<String>,
    }
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for seed in seeds {
        if let Some(node) = graph.get(seed)
            && !index.contains_key(&node.id)
        {
            index.insert(node.id.clone(), candidates.len());
            candidates.push(Candidate {
                node,
                distance: 0,
                connection: None,
            });
        }
    }
    for seed in seeds {
        for n in neighbors(graph, seed, depth, None, MAX_CANDIDATES) {
            let Some(node) = graph.get(&n.id) else {
                continue;
            };
            let connection = if n.incoming {
                format!("{} links to {} as {}", n.id, n.from, n.via)
            } else {
                format!("{} {} {}", n.from, n.via, n.id)
            };
            match index.get(&n.id) {
                Some(&i) if candidates[i].distance <= n.distance => {}
                Some(&i) => {
                    candidates[i].distance = n.distance;
                    candidates[i].connection = Some(connection);
                }
                None => {
                    index.insert(n.id.clone(), candidates.len());
                    candidates.push(Candidate {
                        node,
                        distance: n.distance,
                        connection: Some(connection),
                    });
                }
            }
        }
    }
    candidates.sort_by(|a, b| {
        a.distance
            .cmp(&b.distance)
            .then_with(|| rank(&a.node.ty).cmp(&rank(&b.node.ty)))
            .then_with(|| a.node.id.cmp(&b.node.id))
    });

    let mut pack = Pack {
        budget,
        used: 0,
        items: Vec::new(),
        pointers: Vec::new(),
        more: 0,
    };
    for c in candidates {
        let remaining = budget - pack.used;
        let text = text_of(c.node).unwrap_or_default();
        let size = tokens(&text);
        let fits = size <= remaining;
        // A node that was asked for is never only a pointer, unless almost nothing is left.
        let cut_seed = !fits && c.distance == 0 && remaining >= 50;
        if fits || cut_seed {
            let text = if fits { text } else { cut(&text, remaining) };
            let size = tokens(&text);
            pack.used += size;
            pack.items.push(Item {
                id: c.node.id.clone(),
                ty: c.node.ty.clone(),
                title: c.node.title.clone(),
                status: c.node.status.clone(),
                distance: c.distance,
                connection: c.connection,
                text,
                tokens: size,
                truncated: !fits,
            });
        } else if pack.pointers.len() < MAX_POINTERS {
            pack.pointers.push(Pointer {
                id: c.node.id.clone(),
                ty: c.node.ty.clone(),
                title: c.node.title.clone(),
                distance: c.distance,
            });
        } else {
            pack.more += 1;
        }
    }
    pack
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

    fn requirement(id: &str, links: &str, words: usize) -> String {
        let filler = "word ".repeat(words);
        format!(
            "---\nid: {id}\ntype: requirement\nstatus: accepted\n{links}---\n### Requirement: {id}\nThe system SHALL work. {filler}\n\n#### Scenario: One {{#S1}}\n- **WHEN** a\n- **THEN** b\n"
        )
    }

    fn adr(id: &str) -> String {
        format!(
            "---\nid: {id}\ntype: adr\nstatus: accepted\n---\n# {id}: Decision\n\n## Decision\nUse it.\n"
        )
    }

    /// REQ-A-0001 decided-by ADR-0001; REQ-A-0002 refines REQ-A-0001; REQ-A-0003 refines REQ-A-0002.
    fn sample() -> Graph {
        graph(&[
            (
                "a1.md",
                &requirement("REQ-A-0001", "links:\n  decided-by: ADR-0001\n", 0),
            ),
            (
                "a2.md",
                &requirement("REQ-A-0002", "links:\n  refines: REQ-A-0001\n", 0),
            ),
            (
                "a3.md",
                &requirement("REQ-A-0003", "links:\n  refines: REQ-A-0002\n", 0),
            ),
            ("adr1.md", &adr("ADR-0001")),
        ])
    }

    fn text(n: &Node) -> Option<String> {
        Some(format!("text of {}", n.id))
    }

    // verifies: REQ-CONTEXT-0001.S1
    #[test]
    fn neighbors_follow_links_in_both_directions() {
        let g = sample();
        let near = neighbors(&g, "REQ-A-0001", 1, None, 100);
        let ids: Vec<_> = near
            .iter()
            .map(|n| (n.id.as_str(), n.via.as_str(), n.incoming))
            .collect();
        assert_eq!(
            ids,
            vec![
                ("ADR-0001", "decided-by", false),
                ("REQ-A-0002", "refines", true)
            ]
        );
        assert!(
            near.iter()
                .all(|n| n.distance == 1 && n.from == "REQ-A-0001")
        );
    }

    // verifies: REQ-CONTEXT-0001.S2
    #[test]
    fn neighbors_respect_depth_kinds_and_limit() {
        let g = sample();
        let two = neighbors(&g, "REQ-A-0001", 2, None, 100);
        assert_eq!(two.len(), 3);
        assert_eq!(two[2].id, "REQ-A-0003");
        assert_eq!(two[2].distance, 2);
        assert_eq!(neighbors(&g, "REQ-A-0001", 0, None, 100), vec![]);
        let only = neighbors(&g, "REQ-A-0001", 3, Some(&["decided-by".to_string()]), 100);
        assert_eq!(only.len(), 1);
        assert_eq!(neighbors(&g, "REQ-A-0001", 3, None, 2).len(), 2);
        assert!(neighbors(&g, "REQ-NOPE-0001", 3, None, 2).is_empty());
        // A scenario ID walks from its node.
        assert_eq!(neighbors(&g, "REQ-A-0001.S1", 1, None, 100).len(), 2);
    }

    // verifies: REQ-CONTEXT-0002.S1
    #[test]
    fn a_pack_lists_the_nearest_nodes_first_and_the_requirement_before_the_decision() {
        let g = sample();
        let p = pack(&g, &["REQ-A-0002".to_string()], 2, 10_000, &text);
        let ids: Vec<_> = p.items.iter().map(|i| i.id.as_str()).collect();
        // The node asked for, the two requirements that link to it directly, then the ADR.
        assert_eq!(
            ids,
            vec!["REQ-A-0002", "REQ-A-0001", "REQ-A-0003", "ADR-0001"]
        );
        assert_eq!(p.items[0].distance, 0);
        assert!(p.items[0].connection.is_none());
        assert_eq!(
            p.items[1].connection.as_deref(),
            Some("REQ-A-0002 refines REQ-A-0001")
        );
        assert!(p.pointers.is_empty());
        assert!(p.used <= p.budget);
    }

    #[test]
    fn at_equal_distance_a_requirement_comes_before_an_adr() {
        let g = sample();
        let p = pack(&g, &["REQ-A-0001".to_string()], 1, 10_000, &text);
        let ids: Vec<_> = p.items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, vec!["REQ-A-0001", "REQ-A-0002", "ADR-0001"]);
    }

    // verifies: REQ-CONTEXT-0002.S2
    #[test]
    fn what_does_not_fit_becomes_a_pointer() {
        let g = sample();
        let big = |n: &Node| {
            Some(if n.id == "REQ-A-0002" {
                "y".repeat(4000)
            } else {
                format!("small {}", n.id)
            })
        };
        let p = pack(&g, &["REQ-A-0001".to_string()], 2, 100, &big);
        let ids: Vec<_> = p.items.iter().map(|i| i.id.as_str()).collect();
        assert!(
            ids.contains(&"REQ-A-0001") && ids.contains(&"ADR-0001"),
            "{ids:?}"
        );
        assert!(!ids.contains(&"REQ-A-0002"));
        assert_eq!(p.pointers.len(), 1);
        assert_eq!(p.pointers[0].id, "REQ-A-0002");
        assert!(p.used <= 100);
        // A smaller node after the one that did not fit still gets in.
        assert!(ids.contains(&"REQ-A-0003"), "{ids:?}");
    }

    // verifies: REQ-CONTEXT-0002.S3
    #[test]
    fn an_oversized_node_that_was_asked_for_is_cut_not_dropped() {
        let g = sample();
        let big = |_: &Node| Some(format!("{}\n", "line of text\n".repeat(500)));
        let p = pack(&g, &["REQ-A-0001".to_string()], 1, 200, &big);
        assert_eq!(p.items.len(), 1);
        assert!(p.items[0].truncated);
        assert!(p.items[0].text.contains("[cut to fit the budget]"));
        assert!(p.used <= 200, "{}", p.used);
        assert_eq!(p.pointers.len(), 2);
    }

    // verifies: REQ-CONTEXT-0002.S4
    #[test]
    fn several_seeds_share_one_budget_and_unknown_seeds_are_skipped() {
        let g = sample();
        let seeds = vec![
            "REQ-A-0003".to_string(),
            "ADR-0001".to_string(),
            "NOPE-1".to_string(),
        ];
        let p = pack(&g, &seeds, 0, 10_000, &text);
        let ids: Vec<_> = p.items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, vec!["REQ-A-0003", "ADR-0001"]);
    }

    // verifies: REQ-CONTEXT-0002.S5
    #[test]
    fn a_token_is_about_four_characters() {
        assert_eq!(tokens(""), 0);
        assert_eq!(tokens("abcd"), 1);
        assert_eq!(tokens("abcde"), 2);
    }
}
