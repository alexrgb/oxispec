//! Parsing a markdown file with frontmatter into a node.

use std::ops::Range;
use std::path::Path;

use crate::finding::Finding;
use crate::node::{
    ClaimLine, Frontmatter, Heading, Node, OneOrMany, RequirementBlock, Scenario, TextBlock,
};

/// Where the frontmatter sits in a file.
pub struct FrontmatterSpan {
    /// The YAML text between the two `---` lines.
    pub yaml: Range<usize>,
    /// Byte offset where the body starts.
    pub body_start: usize,
    /// Number of lines before the body, including both `---` lines.
    pub lines_before_body: usize,
}

pub fn frontmatter_span(text: &str) -> Option<FrontmatterSpan> {
    let mut lines = text.split_inclusive('\n');
    let first = lines.next()?;
    // Editors on Windows can put a byte order mark before the first line.
    if first.trim_start_matches('\u{feff}').trim_end() != "---" {
        return None;
    }
    let yaml_start = first.len();
    let mut offset = yaml_start;
    // `lines` starts at the second line of the file.
    for (line_no, line) in (2usize..).zip(lines) {
        if line.trim_end() == "---" {
            return Some(FrontmatterSpan {
                yaml: yaml_start..offset,
                body_start: offset + line.len(),
                lines_before_body: line_no,
            });
        }
        offset += line.len();
    }
    None
}

/// Splits `---` delimited frontmatter from the body.
/// Returns the YAML text, the body, and the number of lines before the body.
pub fn split_frontmatter(text: &str) -> Option<(&str, &str, usize)> {
    let span = frontmatter_span(text)?;
    Some((
        &text[span.yaml],
        &text[span.body_start..],
        span.lines_before_body,
    ))
}

fn has_normative(line: &str) -> bool {
    line.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|w| w == "SHALL" || w == "MUST")
}

/// The first `# ` heading outside code fences.
fn first_heading(body: &str) -> Option<String> {
    let mut in_fence = false;
    for line in body.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence
            && let Some(heading) = line.strip_prefix("# ")
            && !heading.trim().is_empty()
        {
            return Some(heading.trim().to_string());
        }
    }
    None
}

/// Splits a trailing `{#anchor}` off a scenario heading.
fn split_anchor(heading: &str) -> (String, Option<String>) {
    let s = heading.trim();
    if let Some(rest) = s.strip_suffix('}')
        && let Some(pos) = rest.rfind("{#")
    {
        let name = rest[..pos].trim_end().to_string();
        return (name, Some(rest[pos + 2..].to_string()));
    }
    (s.to_string(), None)
}

/// Finds `### Requirement:` blocks and their `#### Scenario:` children.
/// Lines inside code fences are ignored.
/// The `##` and `###` headings outside code blocks.
pub fn parse_headings(body: &str, line_offset: usize) -> Vec<Heading> {
    let mut headings = Vec::new();
    let mut in_fence = false;
    for (i, line) in body.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let level = line.chars().take_while(|&c| c == '#').count();
        if (2..=3).contains(&level)
            && let Some(text) = line[level..].strip_prefix(' ')
        {
            headings.push(Heading {
                level: level as u8,
                text: text.trim().trim_end_matches('#').trim().to_string(),
                line: line_offset + i + 1,
            });
        }
    }
    headings
}

pub fn parse_requirements(body: &str, line_offset: usize) -> Vec<RequirementBlock> {
    let mut blocks: Vec<RequirementBlock> = Vec::new();
    let mut in_fence = false;
    let mut in_description = false;
    for (i, line) in body.lines().enumerate() {
        let n = line_offset + i + 1;
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(name) = line.strip_prefix("### Requirement:") {
            blocks.push(RequirementBlock {
                name: name.trim().to_string(),
                line: n,
                has_normative: false,
                scenarios: Vec::new(),
            });
            in_description = true;
        } else if let Some(heading) = line.strip_prefix("#### Scenario:") {
            if let Some(b) = blocks.last_mut() {
                let (name, anchor) = split_anchor(heading);
                b.scenarios.push(Scenario {
                    name,
                    anchor,
                    line: n,
                });
            }
            in_description = false;
        } else if line.starts_with('#') {
            in_description = false;
        } else if in_description
            && has_normative(line)
            && let Some(b) = blocks.last_mut()
        {
            b.has_normative = true;
        }
    }
    blocks
}

/// The fenced blocks of a node that other modules read.
pub struct Blocks {
    pub claims: Vec<ClaimLine>,
    pub vocabulary: Vec<TextBlock>,
    pub machines: Vec<TextBlock>,
    pub architecture: Vec<TextBlock>,
}

/// Reads the lines of fenced `claims` blocks and the text of `vocabulary`,
/// `statemachine` and `architecture` blocks. Blocks inside other fences are not read.
pub fn parse_blocks(body: &str, line_offset: usize) -> Blocks {
    enum In {
        Nothing,
        Other,
        Claims,
        Vocabulary,
        Machine,
        Architecture,
    }
    let mut state = In::Nothing;
    let mut claims = Vec::new();
    let mut vocabulary: Vec<TextBlock> = Vec::new();
    let mut machines: Vec<TextBlock> = Vec::new();
    let mut architecture: Vec<TextBlock> = Vec::new();
    for (i, line) in body.lines().enumerate() {
        let n = line_offset + i + 1;
        let t = line.trim();
        let fence = t.starts_with("```") || t.starts_with("~~~");
        state = match (state, fence) {
            (In::Nothing, true) => match t.strip_prefix("```").map(str::trim) {
                Some("claims") => In::Claims,
                Some("vocabulary") => {
                    vocabulary.push(TextBlock {
                        line: n + 1,
                        text: String::new(),
                    });
                    In::Vocabulary
                }
                Some("statemachine") => {
                    machines.push(TextBlock {
                        line: n + 1,
                        text: String::new(),
                    });
                    In::Machine
                }
                Some("architecture") => {
                    architecture.push(TextBlock {
                        line: n + 1,
                        text: String::new(),
                    });
                    In::Architecture
                }
                _ => In::Other,
            },
            (In::Nothing, false) => In::Nothing,
            (_, true) => In::Nothing,
            (In::Claims, false) => {
                if !t.is_empty() && !t.starts_with('#') {
                    claims.push(ClaimLine {
                        line: n,
                        text: line.to_string(),
                    });
                }
                In::Claims
            }
            (In::Vocabulary, false) => {
                if let Some(block) = vocabulary.last_mut() {
                    block.text.push_str(line);
                    block.text.push('\n');
                }
                In::Vocabulary
            }
            (In::Machine, false) => {
                if let Some(block) = machines.last_mut() {
                    block.text.push_str(line);
                    block.text.push('\n');
                }
                In::Machine
            }
            (In::Architecture, false) => {
                if let Some(block) = architecture.last_mut() {
                    block.text.push_str(line);
                    block.text.push('\n');
                }
                In::Architecture
            }
            (In::Other, false) => In::Other,
        };
    }
    Blocks {
        claims,
        vocabulary,
        machines,
        architecture,
    }
}

/// Parses one file. Files without frontmatter, or with frontmatter that has
/// neither `id` nor `type`, are not nodes and produce nothing.
pub fn parse_file_text(path: &Path, text: &str) -> (Option<Node>, Vec<Finding>) {
    let Some((yaml, body, consumed)) = split_frontmatter(text) else {
        return (None, Vec::new());
    };
    let fm: Frontmatter = match serde_saphyr::from_str(yaml) {
        Ok(fm) => fm,
        Err(e) => {
            let f = Finding::warning(
                "W002",
                format!("frontmatter is not valid YAML, file skipped: {e}"),
            )
            .at(path, Some(1));
            return (None, vec![f]);
        }
    };
    if fm.id.is_none() && fm.ty.is_none() {
        return (None, Vec::new());
    }
    let (Some(id), Some(ty)) = (fm.id, fm.ty) else {
        let f = Finding::error("E003", "frontmatter needs both `id` and `type`").at(path, Some(1));
        return (None, vec![f]);
    };
    let requirements = if ty == "requirement" {
        parse_requirements(body, consumed)
    } else {
        Vec::new()
    };
    let Blocks {
        claims,
        vocabulary,
        machines,
        architecture,
    } = parse_blocks(body, consumed);
    let title = first_heading(body).map(|h| {
        h.strip_prefix(&format!("{id}: "))
            .map_or(h.clone(), str::to_string)
    });
    let node = Node {
        id,
        ty,
        status: fm.status,
        owner: fm.owner,
        approved_by: fm.approved_by,
        approved_on: fm.approved_on,
        version: fm.version,
        date: fm.date,
        decision_makers: fm
            .decision_makers
            .map(OneOrMany::into_vec)
            .unwrap_or_default(),
        severity: fm.severity,
        due: fm.due,
        cost: fm.cost,
        interest: fm.interest,
        title,
        path: path.to_path_buf(),
        links: fm
            .links
            .into_iter()
            .map(|(k, v)| (k, v.into_vec()))
            .collect(),
        requirements,
        headings: parse_headings(body, consumed),
        claims,
        vocabulary,
        machines,
        architecture,
        body_hash: blake3::hash(body.as_bytes()).to_hex().to_string(),
    };
    (Some(node), Vec::new())
}

#[cfg(test)]
mod tests {

    #[test]
    fn headings_of_level_two_and_three_outside_code_are_kept_with_their_lines() {
        let body = "# T\n\n## Context\n\n```\n## Not a heading\n```\n\n### Detail ##\n#### Deep\n##NoSpace\n";
        let h = parse_headings(body, 4);
        let seen: Vec<_> = h
            .iter()
            .map(|h| (h.level, h.text.as_str(), h.line))
            .collect();
        assert_eq!(seen, [(2, "Context", 7), (3, "Detail", 13)]);
    }
    use super::*;

    const GOOD: &str = "---\nid: REQ-AUTH-0001\ntype: requirement\nlinks:\n  decided-by: [ADR-0001]\n  governed-by: STD-GO-001\n---\n### Requirement: Login\nThe system SHALL issue a token.\n\n#### Scenario: Valid\n- **WHEN** a user logs in\n- **THEN** a token is returned\n";

    #[test]
    fn a_byte_order_mark_before_the_frontmatter_is_ignored() {
        let text = format!("\u{feff}{GOOD}");
        let (node, findings) = parse_file_text(Path::new("a.md"), &text);
        assert!(findings.is_empty());
        assert_eq!(node.expect("a node").id, "REQ-AUTH-0001");
    }

    #[test]
    fn parses_node_and_requirements() {
        let (node, findings) = parse_file_text(Path::new("a.md"), GOOD);
        assert!(findings.is_empty());
        let node = node.unwrap();
        assert_eq!(node.id, "REQ-AUTH-0001");
        assert_eq!(node.links["decided-by"], vec!["ADR-0001"]);
        assert_eq!(node.links["governed-by"], vec!["STD-GO-001"]);
        assert_eq!(node.requirements.len(), 1);
        assert!(node.requirements[0].has_normative);
        assert_eq!(node.requirements[0].scenarios[0].name, "Valid");
        assert_eq!(node.requirements[0].scenarios[0].anchor, None);
        assert_eq!(node.requirements[0].line, 8);
        assert_eq!(node.requirements[0].scenarios[0].line, 11);
    }

    #[test]
    fn scenario_anchors_are_split_from_the_name() {
        let body = "### Requirement: A\nIt SHALL.\n#### Scenario: Idle admin  {#S1}\n#### Scenario: Plain\n#### Scenario: Odd {#bad-id}\n#### Scenario: Empty {#}\n";
        let s = &parse_requirements(body, 0)[0].scenarios;
        assert_eq!(s[0].name, "Idle admin");
        assert_eq!(s[0].anchor.as_deref(), Some("S1"));
        assert_eq!(s[1].name, "Plain");
        assert_eq!(s[1].anchor, None);
        assert_eq!(s[2].anchor.as_deref(), Some("bad-id"));
        assert_eq!(s[3].anchor.as_deref(), Some(""));
    }

    #[test]
    fn ignores_files_without_frontmatter() {
        let (node, findings) = parse_file_text(Path::new("README.md"), "# Title\n");
        assert!(node.is_none() && findings.is_empty());
    }

    #[test]
    fn ignores_unrelated_frontmatter() {
        let (node, findings) = parse_file_text(Path::new("p.md"), "---\ntitle: Page\n---\nbody\n");
        assert!(node.is_none() && findings.is_empty());
    }

    #[test]
    fn missing_type_is_an_error() {
        let (node, findings) = parse_file_text(Path::new("p.md"), "---\nid: REQ-A-1\n---\n");
        assert!(node.is_none());
        assert_eq!(findings[0].code, "E003");
    }

    #[test]
    fn code_fences_hide_headings() {
        let body = "```\n### Requirement: Fake\n```\n### Requirement: Real\nIt MUST work.\n";
        let blocks = parse_requirements(body, 0);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "Real");
        assert!(blocks[0].has_normative);
    }

    #[test]
    fn must_not_counts_as_normative_but_lowercase_does_not() {
        let blocks = parse_requirements("### Requirement: A\nIt must work.\n", 0);
        assert!(!blocks[0].has_normative);
        let blocks = parse_requirements("### Requirement: A\nIt MUST NOT fail.\n", 0);
        assert!(blocks[0].has_normative);
    }

    #[test]
    fn claims_and_vocabulary_blocks_are_read() {
        let text = "---\nid: REQ-A-1\ntype: requirement\n---\ntext\n\n```claims\n# note\nfor role = admin: x <= 1\n\nmust y\n```\n\n```vocabulary\nx: {type: int}\n```\n\n```rust\n```claims\nnot: read\n```\n";
        let (node, _) = parse_file_text(Path::new("a.md"), text);
        let node = node.unwrap();
        let lines: Vec<(usize, &str)> = node
            .claims
            .iter()
            .map(|c| (c.line, c.text.as_str()))
            .collect();
        assert_eq!(lines, [(9, "for role = admin: x <= 1"), (11, "must y")]);
        assert_eq!(node.vocabulary.len(), 1);
        assert_eq!(node.vocabulary[0].line, 15);
        assert_eq!(node.vocabulary[0].text, "x: {type: int}\n");
    }

    #[test]
    fn span_points_at_the_yaml_and_the_body() {
        let text = "---\nid: A-1\n---\nbody\n";
        let span = frontmatter_span(text).unwrap();
        assert_eq!(&text[span.yaml.clone()], "id: A-1\n");
        assert_eq!(&text[span.body_start..], "body\n");
        assert_eq!(span.lines_before_body, 3);
        assert!(frontmatter_span("no frontmatter\n").is_none());
        assert!(frontmatter_span("---\nnever closed\n").is_none());
    }

    #[test]
    fn titles_come_from_the_first_heading_without_the_id_prefix() {
        let text = "---\nid: ADR-0001\ntype: adr\n---\n```\n# not this\n```\n# ADR-0001: Use Postgres\n\n# Another\n";
        let node = parse_file_text(Path::new("a.md"), text).0.unwrap();
        assert_eq!(node.title.as_deref(), Some("Use Postgres"));
        let plain = parse_file_text(
            Path::new("a.md"),
            "---\nid: ADR-0002\ntype: adr\n---\n# Plain title\n",
        )
        .0
        .unwrap();
        assert_eq!(plain.title.as_deref(), Some("Plain title"));
        let none = parse_file_text(
            Path::new("a.md"),
            "---\nid: ADR-0003\ntype: adr\n---\nno heading\n",
        )
        .0
        .unwrap();
        assert_eq!(none.title, None);
    }

    #[test]
    fn approval_fields_are_read_when_quoted_or_not() {
        let quoted = "---\nid: CHG-1\ntype: change\nstatus: approved\napproved_by: Ann Lee\napproved_on: \"2026-10-28\"\n---\n";
        let node = parse_file_text(Path::new("c.md"), quoted).0.unwrap();
        assert_eq!(node.approved_by.as_deref(), Some("Ann Lee"));
        assert_eq!(node.approved_on.as_deref(), Some("2026-10-28"));
        let bare = quoted.replace("\"2026-10-28\"", "2026-10-28");
        let node = parse_file_text(Path::new("c.md"), &bare).0.unwrap();
        assert_eq!(node.approved_on.as_deref(), Some("2026-10-28"));
    }
}
