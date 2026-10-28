//! Text edits that change one thing and leave the rest of a file as it was.

use std::collections::BTreeMap;

use crate::node::Frontmatter;
use crate::parse::frontmatter_span;

#[derive(Debug, PartialEq, Eq)]
pub enum LinkEdit {
    /// The new file text.
    Added(String),
    AlreadyLinked,
}

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("the file has no frontmatter")]
    NoFrontmatter,
    #[error("the frontmatter is not valid YAML: {0}")]
    Yaml(String),
}

/// Adds `kind: target` to the `links:` block of the file's frontmatter.
///
/// Only the `links:` block is rewritten, one target per line so that two
/// people adding links on different branches rarely conflict. Other keys,
/// comments outside that block, and the whole body are kept byte for byte.
pub fn add_link(text: &str, kind: &str, target: &str) -> Result<LinkEdit, EditError> {
    let span = frontmatter_span(text).ok_or(EditError::NoFrontmatter)?;
    let yaml = &text[span.yaml.clone()];
    let frontmatter: Frontmatter =
        serde_saphyr::from_str(yaml).map_err(|e| EditError::Yaml(e.to_string()))?;

    let mut links: BTreeMap<String, Vec<String>> = frontmatter
        .links
        .into_iter()
        .map(|(k, v)| (k, v.into_vec()))
        .collect();
    let targets = links.entry(kind.to_string()).or_default();
    if targets.iter().any(|t| t == target) {
        return Ok(LinkEdit::AlreadyLinked);
    }
    targets.push(target.to_string());

    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let block = render_links(&links, newline);

    let mut new_yaml = String::with_capacity(yaml.len() + block.len());
    let mut lines = yaml.split_inclusive('\n').peekable();
    let mut replaced = false;
    while let Some(line) = lines.next() {
        if !replaced && line.starts_with("links:") {
            // Drop the old block: indented lines and blank lines that follow.
            while let Some(next) = lines.peek() {
                let content = next.trim_end_matches(['\r', '\n']);
                if content.is_empty() || content.starts_with([' ', '\t']) {
                    lines.next();
                } else {
                    break;
                }
            }
            new_yaml.push_str(&block);
            replaced = true;
        } else {
            new_yaml.push_str(line);
        }
    }
    if !replaced {
        if !new_yaml.is_empty() && !new_yaml.ends_with('\n') {
            new_yaml.push_str(newline);
        }
        new_yaml.push_str(&block);
    }

    let mut out = String::with_capacity(text.len() + block.len());
    out.push_str(&text[..span.yaml.start]);
    out.push_str(&new_yaml);
    out.push_str(&text[span.yaml.end..]);
    Ok(LinkEdit::Added(out))
}

/// A value as it is written in the frontmatter: plain when it is a simple
/// lowercase word such as `in-review`, otherwise in double quotes.
fn yaml_scalar(value: &str) -> String {
    let simple = value.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if simple {
        value.to_string()
    } else {
        let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    }
}

/// Sets, adds or removes top-level single-line keys in the frontmatter,
/// for example `status` or `approved_by`. `Some(value)` sets the key and
/// `None` removes it. Everything else in the file is kept byte for byte.
pub fn set_fields(text: &str, updates: &[(&str, Option<&str>)]) -> Result<String, EditError> {
    let span = frontmatter_span(text).ok_or(EditError::NoFrontmatter)?;
    let yaml = &text[span.yaml.clone()];
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = yaml.split_inclusive('\n').map(str::to_string).collect();

    for (key, value) in updates {
        let prefix = format!("{key}:");
        let found = lines.iter().position(|l| l.starts_with(&prefix));
        match (found, value) {
            (Some(i), Some(v)) => lines[i] = format!("{key}: {}{newline}", yaml_scalar(v)),
            (Some(i), None) => {
                lines.remove(i);
            }
            (None, Some(v)) => {
                if let Some(last) = lines.last_mut()
                    && !last.ends_with('\n')
                {
                    last.push_str(newline);
                }
                lines.push(format!("{key}: {}{newline}", yaml_scalar(v)));
            }
            (None, None) => {}
        }
    }

    let mut out = String::with_capacity(text.len() + 64);
    out.push_str(&text[..span.yaml.start]);
    out.extend(lines.iter().map(String::as_str));
    out.push_str(&text[span.yaml.end..]);
    Ok(out)
}

fn render_links(links: &BTreeMap<String, Vec<String>>, newline: &str) -> String {
    let mut out = format!("links:{newline}");
    for (kind, targets) in links {
        if targets.is_empty() {
            continue;
        }
        out.push_str(&format!("  {kind}:{newline}"));
        for target in targets {
            // IDs from other projects contain a colon, so quote them.
            if target.contains(':') {
                out.push_str(&format!("    - \"{target}\"{newline}"));
            } else {
                out.push_str(&format!("    - {target}{newline}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_file_text;
    use std::path::Path;

    fn links_of(text: &str) -> BTreeMap<String, Vec<String>> {
        let (node, findings) = parse_file_text(Path::new("x.md"), text);
        assert!(findings.is_empty(), "{findings:?}");
        node.expect("a node").links
    }

    fn added(text: &str, kind: &str, target: &str) -> String {
        match add_link(text, kind, target).unwrap() {
            LinkEdit::Added(s) => s,
            LinkEdit::AlreadyLinked => panic!("expected a change"),
        }
    }

    const PLAIN: &str = "---\nid: REQ-A-0001\ntype: requirement\nstatus: draft\n---\n### Requirement: X\nIt SHALL work.\n";

    #[test]
    fn adds_a_links_block_when_there_is_none() {
        let out = added(PLAIN, "decided-by", "ADR-0001");
        assert!(
            out.contains("status: draft\nlinks:\n  decided-by:\n    - ADR-0001\n---\n"),
            "{out}"
        );
        assert_eq!(links_of(&out)["decided-by"], vec!["ADR-0001"]);
    }

    #[test]
    fn extends_an_existing_block_and_keeps_other_keys() {
        let text = "---\nid: REQ-A-0001\ntype: requirement\nlinks:\n  decided-by: ADR-0001\nstatus: draft\n---\nbody\n";
        let out = added(text, "satisfies", "REQ-B-0002");
        let links = links_of(&out);
        assert_eq!(links["decided-by"], vec!["ADR-0001"]);
        assert_eq!(links["satisfies"], vec!["REQ-B-0002"]);
        assert!(out.contains("\nstatus: draft\n---\nbody\n"), "{out}");
    }

    #[test]
    fn appends_to_an_existing_kind() {
        let text = "---\nid: A-1\ntype: adr\nlinks:\n  decided-by: [ADR-0001]\n---\n";
        let out = added(text, "decided-by", "ADR-0002");
        assert_eq!(links_of(&out)["decided-by"], vec!["ADR-0001", "ADR-0002"]);
    }

    #[test]
    fn replaces_an_inline_empty_block() {
        let text = "---\nid: A-1\ntype: adr\nlinks: {}\nstatus: draft\n---\n";
        let out = added(text, "refines", "ADR-0002");
        assert_eq!(links_of(&out)["refines"], vec!["ADR-0002"]);
        assert!(!out.contains("{}"), "{out}");
        assert!(out.contains("status: draft"), "{out}");
    }

    #[test]
    fn is_idempotent() {
        let out = added(PLAIN, "decided-by", "ADR-0001");
        assert_eq!(
            add_link(&out, "decided-by", "ADR-0001").unwrap(),
            LinkEdit::AlreadyLinked
        );
    }

    #[test]
    fn quotes_ids_from_other_projects() {
        let out = added(PLAIN, "satisfies", "product/checkout:REQ-CHK-0004.S2");
        assert!(out.contains("- \"product/checkout:REQ-CHK-0004.S2\""));
        assert_eq!(
            links_of(&out)["satisfies"],
            vec!["product/checkout:REQ-CHK-0004.S2"]
        );
    }

    #[test]
    fn keeps_crlf_line_endings() {
        let text = PLAIN.replace('\n', "\r\n");
        let out = added(&text, "decided-by", "ADR-0001");
        assert!(
            out.contains("links:\r\n  decided-by:\r\n    - ADR-0001\r\n---\r\n"),
            "{out:?}"
        );
        assert!(
            !out.replace("\r\n", "").contains('\n'),
            "bare newline in {out:?}"
        );
    }

    #[test]
    fn leaves_the_body_untouched() {
        let body = "### Requirement: X\nIt SHALL work.\n\n```\nlinks:\n  not: frontmatter\n```\n";
        let text = format!("---\nid: A-1\ntype: requirement\n---\n{body}");
        let out = added(&text, "refines", "ADR-0002");
        assert!(out.ends_with(body), "{out}");
    }

    #[test]
    fn needs_frontmatter() {
        assert!(matches!(
            add_link("# Title\n", "refines", "A-1"),
            Err(EditError::NoFrontmatter)
        ));
    }

    const CHANGE: &str = "---\nid: CHG-0001\ntype: change\nstatus: draft\nlinks:\n  changes: REQ-A-0001\n---\n# CHG-0001: T\n";

    #[test]
    fn set_fields_replaces_adds_and_removes() {
        let out = set_fields(
            CHANGE,
            &[
                ("status", Some("approved")),
                ("approved_by", Some("Ann Lee")),
                ("approved_on", Some("2026-10-28")),
            ],
        )
        .unwrap();
        assert!(out.contains("\nstatus: approved\n"), "{out}");
        assert!(out.contains("\napproved_by: \"Ann Lee\"\n"), "{out}");
        assert!(out.contains("\napproved_on: \"2026-10-28\"\n"), "{out}");
        // The links block and the body are untouched, and the file still parses.
        assert!(out.contains("links:\n  changes: REQ-A-0001\n"), "{out}");
        assert!(out.ends_with("---\n# CHG-0001: T\n"), "{out}");
        let node = parse_file_text(Path::new("c.md"), &out).0.unwrap();
        assert_eq!(node.status.as_deref(), Some("approved"));
        assert_eq!(node.approved_by.as_deref(), Some("Ann Lee"));
        assert_eq!(node.approved_on.as_deref(), Some("2026-10-28"));
        assert_eq!(node.links["changes"], vec!["REQ-A-0001"]);

        let back = set_fields(
            &out,
            &[
                ("status", Some("in-review")),
                ("approved_by", None),
                ("approved_on", None),
            ],
        )
        .unwrap();
        assert!(
            !back.contains("approved_by") && !back.contains("approved_on"),
            "{back}"
        );
        assert!(back.contains("status: in-review"), "{back}");
    }

    #[test]
    fn set_fields_does_not_confuse_keys_that_share_a_prefix() {
        let text = "---\nid: A-1\ntype: change\nstatus_note: keep\nstatus: draft\n---\n";
        let out = set_fields(text, &[("status", Some("merged"))]).unwrap();
        assert!(out.contains("status_note: keep"), "{out}");
        assert!(out.contains("\nstatus: merged\n"), "{out}");
        assert_eq!(set_fields(text, &[("missing", None)]).unwrap(), text);
    }

    #[test]
    fn set_fields_quotes_awkward_values_and_keeps_crlf() {
        let text = "---\r\nid: A-1\r\ntype: change\r\n---\r\nbody\r\n";
        let out = set_fields(text, &[("approved_by", Some("Ann \"A\" Lee: lead # 1"))]).unwrap();
        assert!(
            out.contains("approved_by: \"Ann \\\"A\\\" Lee: lead # 1\"\r\n"),
            "{out:?}"
        );
        assert!(!out.replace("\r\n", "").contains('\n'), "{out:?}");
        let node = parse_file_text(Path::new("c.md"), &out).0.unwrap();
        assert_eq!(node.approved_by.as_deref(), Some("Ann \"A\" Lee: lead # 1"));
    }

    #[test]
    fn set_fields_needs_frontmatter() {
        assert!(matches!(
            set_fields("# Title\n", &[("status", Some("draft"))]),
            Err(EditError::NoFrontmatter)
        ));
    }
}
