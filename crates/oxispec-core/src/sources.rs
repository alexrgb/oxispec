//! Nodes that are a part of a document: a row of a table that the config says holds them.
//!
//! A project that wrote its test cases and user stories before oxispec keeps them as rows of a table in
//! one file per feature. `sources:` in the config says which files, which column holds the ID, and what
//! the other columns mean. A row then is a node like any other for `check`, `trace`, `coverage`, `show`,
//! `list` and `search`, and the document stays the only copy of the text. Such a node is read-only:
//! the commands that write a node refuse it, with the file and the line.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::finding::Finding;
use crate::id;
use crate::node::{Node, Origin};

/// One kind of document that holds nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceConfig {
    /// A glob of files, relative to the project: `docs/features/*/test-cases.md`.
    pub files: String,
    /// A row of a table is a node.
    pub table: TableConfig,
}

/// How a table is read. Columns are counted from 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableConfig {
    /// The column of the ID.
    pub id: usize,
    /// A regular expression for the ID inside the cell. Without it the whole cell is the ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_pattern: Option<String>,
    /// The type of the nodes, such as `test-case`.
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<usize>,
    /// Link kind to the column that holds the IDs it points at. A range is expanded.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub links: BTreeMap<String, usize>,
    /// Name to column. `status` and `owner` fill those fields of the node; the others are kept in `fields`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, usize>,
}

/// Whether the settings can be used, with the reason when not.
// spec: REQ-SOURCES-0001
pub fn validate(sources: &[SourceConfig]) -> Result<(), String> {
    for (i, s) in sources.iter().enumerate() {
        let at = format!("sources[{i}]");
        let mut builder = ignore::overrides::OverrideBuilder::new(".");
        if s.files.trim().is_empty() || builder.add(&s.files).is_err() {
            return Err(format!("{at}.files `{}` is not a glob", s.files));
        }
        let t = &s.table;
        if t.ty.trim().is_empty() {
            return Err(format!("{at}.table.type is empty"));
        }
        let columns = std::iter::once(("id", t.id))
            .chain(t.title.map(|c| ("title", c)))
            .chain(t.links.iter().map(|(k, c)| (k.as_str(), *c)))
            .chain(t.fields.iter().map(|(k, c)| (k.as_str(), *c)));
        for (name, column) in columns {
            if column == 0 {
                return Err(format!(
                    "{at}.table: column `{name}` is 0; columns start at 1"
                ));
            }
        }
        if let Some(p) = &t.id_pattern
            && let Err(e) = Regex::new(p)
        {
            return Err(format!("{at}.table.id_pattern: {e}"));
        }
    }
    Ok(())
}

/// Why a command that writes a node cannot write this one, when it is a row of a table.
// spec: REQ-SOURCES-0002
pub fn write_refusal(node: &Node) -> Option<String> {
    let origin = node.origin.as_ref()?;
    Some(format!(
        "{} is a row of a table ({}:{}), not a file, so oxispec reads it but does not change it; edit the table",
        node.id,
        node.path.display(),
        origin.line
    ))
}

/// The nodes in the documents the sources name, and what was wrong with them.
// spec: REQ-SOURCES-0001
pub fn load(root: &Path, sources: &[SourceConfig]) -> (Vec<Node>, Vec<Finding>) {
    let mut nodes = Vec::new();
    let mut findings = Vec::new();
    for source in sources {
        for rel in files(root, &source.files) {
            let Ok(text) = std::fs::read_to_string(root.join(&rel)) else {
                continue;
            };
            let (n, f) = read_tables(&rel, &text, &source.table);
            nodes.extend(n);
            findings.extend(f);
        }
    }
    (nodes, findings)
}

fn files(root: &Path, glob: &str) -> Vec<PathBuf> {
    let mut builder = ignore::overrides::OverrideBuilder::new(root);
    if builder.add(glob).is_err() {
        return Vec::new();
    }
    let Ok(overrides) = builder.build() else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = ignore::WalkBuilder::new(root)
        .require_git(false)
        .overrides(overrides)
        .build()
        .flatten()
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .filter_map(|e| e.path().strip_prefix(root).ok().map(Path::to_path_buf))
        .collect();
    found.sort();
    found
}

/// One row of a table: its line and its cells, as written.
struct Row {
    line: usize,
    cells: Vec<String>,
}

/// The tables of a document, outside code blocks: each a list of rows without the header.
fn tables(text: &str) -> Vec<Vec<Row>> {
    let mut out: Vec<Vec<Row>> = Vec::new();
    let mut current: Option<Vec<Row>> = None;
    let mut header_seen = false;
    let mut fence = false;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = !fence;
        }
        if fence || !trimmed.starts_with('|') {
            if let Some(t) = current.take() {
                out.push(t);
            }
            header_seen = false;
            continue;
        }
        let cells = split_cells(trimmed);
        match (&mut current, header_seen) {
            (None, _) => {
                current = Some(Vec::new());
                header_seen = false;
            }
            (Some(_), false) => {
                // The line after the header is the separator: `|---|:--:|`.
                header_seen = cells
                    .iter()
                    .all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')));
                if !header_seen {
                    current = None;
                }
            }
            (Some(rows), true) => rows.push(Row {
                line: index + 1,
                cells,
            }),
        }
    }
    if let Some(t) = current {
        out.push(t);
    }
    out
}

/// The cells of a `| a | b |` line, with `\|` kept as a pipe.
fn split_cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|');
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let mut cells = vec![String::new()];
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                chars.next();
                cells.last_mut().unwrap().push('|');
            }
            '|' => cells.push(String::new()),
            _ => cells.last_mut().unwrap().push(c),
        }
    }
    cells.iter().map(|c| c.trim().to_string()).collect()
}

/// A cell as plain text: no code ticks or bold, a link as its text, line breaks as spaces.
fn plain(cell: &str) -> String {
    let mut text = cell
        .replace("<br>", " ")
        .replace("<br/>", " ")
        .replace("<br />", " ");
    text = text.replace(['`', '*'], "");
    // `[text](url)` is `text`.
    while let (Some(open), Some(mid)) = (text.find('['), text.find("](")) {
        let Some(close) = text[mid..].find(')') else {
            break;
        };
        if open >= mid {
            break;
        }
        let label = text[open + 1..mid].to_string();
        text.replace_range(open..mid + close + 1, &label);
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cell(row: &Row, column: usize) -> &str {
    row.cells.get(column - 1).map_or("", String::as_str)
}

/// The IDs a cell holds: each word that is an ID, a range expanded.
fn ids_in(cell: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in plain(cell).split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '/')) {
        let word = word.trim_matches(|c: char| matches!(c, '(' | ')' | '.' | ':'));
        if let Some(range) = crate::refs::expand_range(word) {
            out.extend(range);
        } else if id::parse(word).is_some() {
            out.push(word.to_string());
        }
    }
    out
}

// spec: REQ-SOURCES-0001
fn read_tables(rel: &Path, text: &str, config: &TableConfig) -> (Vec<Node>, Vec<Finding>) {
    let pattern = config
        .id_pattern
        .as_deref()
        .and_then(|p| Regex::new(p).ok());
    let id_of = |row: &Row| -> Option<String> {
        let cell = plain(cell(row, config.id));
        match &pattern {
            Some(re) => re.find(&cell).map(|m| m.as_str().to_string()),
            None => (!cell.is_empty()).then_some(cell),
        }
    };
    let mut nodes = Vec::new();
    let mut findings = Vec::new();
    for rows in tables(text) {
        // A table is a source when some row has an ID in the column. Another table of the file is not.
        if !rows.iter().any(|r| id_of(r).is_some()) {
            continue;
        }
        for row in &rows {
            let Some(node_id) = id_of(row) else {
                findings.push(
                    Finding::warning(
                        "W030",
                        format!(
                            "a row of a table has no ID in column {}, so it is not a node",
                            config.id
                        ),
                    )
                    .at(rel, Some(row.line)),
                );
                continue;
            };
            let mut node_links: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for (kind, column) in &config.links {
                let targets = ids_in(cell(row, *column));
                if !targets.is_empty() {
                    node_links.insert(kind.clone(), targets);
                }
            }
            let mut fields = BTreeMap::new();
            for (name, column) in &config.fields {
                let value = plain(cell(row, *column));
                if !value.is_empty() {
                    fields.insert(name.clone(), value);
                }
            }
            let raw = row.cells.join("|");
            nodes.push(Node {
                id: node_id,
                ty: config.ty.clone(),
                status: fields.remove("status"),
                owner: fields.remove("owner"),
                approved_by: None,
                approved_on: None,
                version: None,
                date: None,
                decision_makers: Vec::new(),
                severity: None,
                due: None,
                cost: None,
                interest: None,
                likelihood: None,
                impact: None,
                review: None,
                title: config
                    .title
                    .map(|c| plain(cell(row, c)))
                    .filter(|t| !t.is_empty()),
                path: rel.to_path_buf(),
                links: node_links,
                aliases: Vec::new(),
                requirements: Vec::new(),
                claims: Vec::new(),
                vocabulary: Vec::new(),
                machines: Vec::new(),
                architecture: Vec::new(),
                entities: Vec::new(),
                tasks: Vec::new(),
                headings: Vec::new(),
                body_hash: blake3::hash(raw.as_bytes()).to_hex().to_string(),
                origin: Some(Origin {
                    kind: "table".to_string(),
                    line: row.line,
                    end_line: row.line,
                }),
                fields,
            });
        }
    }
    (nodes, findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "# Cases\n\n| Legend | Meaning |\n|---|---|\n| P1 | must |\n\n| ID | Title | Verifies | Priority |\n|---|---|---|:-:|\n| **TC-CAT-001** | Lists programs | [US-CAT-001](x.md), US-CAT-002 | P1 |\n| TC-CAT-1200..TC-CAT-1201 | A range is not an ID | | P2 |\n| TC-CAT-002 | Pipes \\| in a title | US-CAT-1..3 | P2 |\n| (note) | no id here | | |\n\n```\n| TC-CAT-999 | in a code block |\n|---|---|\n```\n";

    fn config() -> TableConfig {
        TableConfig {
            id: 1,
            id_pattern: Some("TC-[A-Z]+-[0-9]+".into()),
            ty: "test-case".into(),
            title: Some(2),
            links: BTreeMap::from([("verifies".to_string(), 3)]),
            fields: BTreeMap::from([("priority".to_string(), 4)]),
        }
    }

    // verifies: REQ-SOURCES-0001.S1
    #[test]
    fn a_row_with_an_id_is_a_node_with_its_cells() {
        let (nodes, _) = read_tables(Path::new("c.md"), DOC, &config());
        let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["TC-CAT-001", "TC-CAT-1200", "TC-CAT-002"]);
        let n = &nodes[0];
        assert_eq!(n.ty, "test-case");
        assert_eq!(n.title.as_deref(), Some("Lists programs"));
        assert_eq!(n.links["verifies"], ["US-CAT-001", "US-CAT-002"]);
        assert_eq!(n.fields["priority"], "P1");
        assert_eq!(n.origin.as_ref().unwrap().line, 9);
        assert_eq!(nodes[2].title.as_deref(), Some("Pipes | in a title"));
    }

    // verifies: REQ-SOURCES-0001.S2
    #[test]
    fn a_range_in_a_link_cell_is_expanded_and_a_table_without_ids_is_left_alone() {
        let (nodes, findings) = read_tables(Path::new("c.md"), DOC, &config());
        assert_eq!(
            nodes[2].links["verifies"],
            ["US-CAT-1", "US-CAT-2", "US-CAT-3"]
        );
        // The legend table and the code block give no node, and the note row is the one warning.
        assert!(nodes.iter().all(|n| n.id != "TC-CAT-999"));
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].code, "W030");
        assert_eq!(findings[0].line, Some(12));
    }

    // verifies: REQ-SOURCES-0001.S3
    #[test]
    fn a_source_with_a_column_of_zero_or_a_bad_pattern_is_refused() {
        let ok = SourceConfig {
            files: "docs/*.md".into(),
            table: config(),
        };
        assert!(validate(std::slice::from_ref(&ok)).is_ok());
        let mut zero = ok.clone();
        zero.table.title = Some(0);
        assert!(validate(&[zero]).unwrap_err().contains("title"));
        let mut bad = ok.clone();
        bad.table.id_pattern = Some("TC-[".into());
        assert!(validate(&[bad]).unwrap_err().contains("id_pattern"));
        let mut empty = ok;
        empty.table.ty = " ".into();
        assert!(validate(&[empty]).is_err());
    }
}
