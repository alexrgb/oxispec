//! Parsing a markdown file with frontmatter into a node.

use std::path::Path;

use crate::finding::Finding;
use crate::node::{Frontmatter, Node, RequirementBlock};

/// Splits `---` delimited frontmatter from the body.
/// Returns the YAML text, the body, and the number of lines before the body.
pub fn split_frontmatter(text: &str) -> Option<(&str, &str, usize)> {
    let mut lines = text.split_inclusive('\n');
    let first = lines.next()?;
    if first.trim_end() != "---" {
        return None;
    }
    let mut offset = first.len();
    let mut line_no = 1usize;
    for line in lines {
        line_no += 1;
        if line.trim_end() == "---" {
            let yaml = &text[first.len()..offset];
            let body = &text[offset + line.len()..];
            return Some((yaml, body, line_no));
        }
        offset += line.len();
    }
    None
}

fn has_normative(line: &str) -> bool {
    line.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|w| w == "SHALL" || w == "MUST")
}

/// Finds `### Requirement:` blocks and their `#### Scenario:` children.
/// Lines inside code fences are ignored.
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
        } else if let Some(name) = line.strip_prefix("#### Scenario:") {
            if let Some(b) = blocks.last_mut() {
                b.scenarios.push(name.trim().to_string());
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
    let node = Node {
        id,
        ty,
        status: fm.status,
        owner: fm.owner,
        path: path.to_path_buf(),
        links: fm
            .links
            .into_iter()
            .map(|(k, v)| (k, v.into_vec()))
            .collect(),
        requirements,
    };
    (Some(node), Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "---\nid: REQ-AUTH-0001\ntype: requirement\nlinks:\n  decided-by: [ADR-0001]\n  governed-by: STD-GO-001\n---\n### Requirement: Login\nThe system SHALL issue a token.\n\n#### Scenario: Valid\n- **WHEN** a user logs in\n- **THEN** a token is returned\n";

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
        assert_eq!(node.requirements[0].scenarios, vec!["Valid"]);
        assert_eq!(node.requirements[0].line, 8);
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
}
