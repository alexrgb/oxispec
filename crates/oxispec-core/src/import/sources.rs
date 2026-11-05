//! Two sources that are not spec folders: a product requirements document
//! (PRD) written as markdown, and an OpenAPI or Swagger description.

use std::collections::BTreeMap;

use serde_json::Value;

use super::{
    Block, Found, Input, Unmapped, criterion, file_name, is_node, kiro_title, list_item, sections,
    sorted_entries, text_of, unbold,
};
use std::fs;
use std::io;
use std::path::Path;

const OVERVIEW: &[&str] = &[
    "overview",
    "summary",
    "background",
    "problem",
    "goals",
    "objective",
    "purpose",
    "introduction",
];
const REQUIREMENTS: &[&str] = &[
    "requirement",
    "feature",
    "functional",
    "user stor",
    "scope",
    "capabilit",
];

/// Sections that name what is not wanted; they are not the overview or the
/// requirements even when they contain "goals" or "scope".
const EXCLUDED: &[&str] = &["non-goal", "non goal", "out of scope", "out-of-scope"];

fn has_any(title: &str, words: &[&str]) -> bool {
    let t = title.to_ascii_lowercase();
    !EXCLUDED.iter().any(|x| t.contains(x)) && words.iter().any(|w| t.contains(w))
}

/// The first words of a requirement, for a title: `FR-1: Sign in` gives
/// `FR-1`, `Users can sign in. More text` gives `Users can sign in`.
fn item_title(text: &str) -> String {
    let text = unbold(text);
    let text = text.trim();
    if text.starts_with("As ") && text.contains(" I want ") {
        return kiro_title("", text);
    }
    if let Some((id, rest)) = text.split_once(':') {
        let looks_like_id = !id.contains(' ') && id.len() <= 12;
        if looks_like_id {
            let rest = rest.trim();
            if rest.is_empty() {
                return id.to_string();
            }
            return format!("{id} {}", short(rest));
        }
    }
    short(text)
}

fn short(text: &str) -> String {
    let first = text
        .split(['.', ':'])
        .next()
        .unwrap_or(text)
        .trim()
        .to_string();
    let cut: String = first.chars().take(60).collect();
    cut.trim().to_string()
}

/// A PRD: its overview becomes the purpose, each bullet under a requirements
/// section a requirement, and bullets nested under it its criteria.
pub(super) fn prd(text: &str) -> (Option<String>, Vec<Block>, Vec<String>) {
    let (pre, secs) = sections(text);
    let mut intro: Vec<String> = Vec::new();
    let mut blocks: Vec<Block> = Vec::new();
    let mut notes = Vec::new();
    let mut under: Option<usize> = None;
    let mut title_seen = false;
    for s in &secs {
        if s.level == 1 && !title_seen {
            title_seen = true;
            intro.extend(s.body.iter().cloned());
            continue;
        }
        if under.is_some_and(|l| s.level <= l) {
            under = None;
        }
        let acceptance = has_any(&s.title, &["acceptance"]);
        if under.is_none() && has_any(&s.title, REQUIREMENTS) && !acceptance {
            under = Some(s.level);
        } else if under.is_none() && s.level <= 2 && has_any(&s.title, OVERVIEW) {
            intro.push(format!("**{}**", s.title));
            intro.extend(s.body.iter().cloned());
            continue;
        } else if under.is_none() && s.level == 2 {
            notes.push(format!("section `{}` was left out", s.title));
            continue;
        }
        if under.is_none() {
            continue;
        }
        let mut fenced = false;
        for line in &s.body {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
            }
            if fenced || line.trim().is_empty() {
                continue;
            }
            let indented = line.starts_with("  ") || line.starts_with('\t');
            match (list_item(line), indented, blocks.last_mut()) {
                (Some(item), true, Some(b)) => {
                    let n = b.scenarios.len() + 1;
                    b.scenarios.push(criterion(n, item));
                }
                (Some(item), _, _) if acceptance && !indented => {
                    if let Some(b) = blocks.last_mut() {
                        let n = b.scenarios.len() + 1;
                        b.scenarios.push(criterion(n, item));
                    }
                }
                (Some(item), false, _) => blocks.push(Block {
                    title: item_title(item),
                    statement: unbold(item),
                    scenarios: Vec::new(),
                }),
                (None, true, Some(b)) => {
                    b.statement.push(' ');
                    b.statement.push_str(line.trim());
                }
                _ => {}
            }
        }
    }
    let mut pre = pre;
    pre.extend(intro);
    let intro = Some(text_of(&pre)).filter(|t| !t.is_empty());
    if blocks.is_empty() {
        notes.push(
            "no section named requirements, features or user stories with a list was found".into(),
        );
    }
    (intro, blocks, notes)
}

fn is_openapi_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("json" | "yaml" | "yml")
    )
}

fn parse_description(path: &Path, text: &str) -> Option<Value> {
    let value: Value = if path.extension().and_then(|e| e.to_str()) == Some("json") {
        serde_json::from_str(text).ok()?
    } else {
        serde_saphyr::from_str(text).ok()?
    };
    (value.get("openapi").is_some() || value.get("swagger").is_some()).then_some(value)
}

fn collect_description_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> io::Result<()> {
    for path in sorted_entries(dir)? {
        let name = file_name(&path);
        if path.is_dir() {
            if !name.starts_with('.') && name != "node_modules" {
                collect_description_files(&path, out)?;
            }
        } else if is_openapi_file(&path) {
            out.push(path);
        }
    }
    Ok(())
}

const METHODS: &[&str] = &["get", "put", "post", "delete", "patch", "head", "options"];

fn ref_name(v: &Value) -> Option<&str> {
    v.get("$ref")?.as_str()?.rsplit('/').next()
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The OpenSpec-layout text of one tag: one requirement per operation, one
/// scenario per response.
fn operations_text(title: &str, tag: &str, ops: &[(String, String, &Value)]) -> String {
    let mut out = format!("## Purpose\n\n{title}: the `{tag}` operations.\n\n## Requirements\n");
    for (method, path, op) in ops {
        let call = format!("{} {path}", method.to_ascii_uppercase());
        let summary = op
            .get("summary")
            .or_else(|| op.get("description"))
            .and_then(Value::as_str)
            .map(one_line)
            .filter(|s| !s.is_empty());
        out.push_str(&format!("\n### Requirement: {call}\n"));
        match &summary {
            Some(s) => out.push_str(&format!(
                "The API SHALL provide `{call}`: {}\n",
                s.trim_end_matches('.')
            )),
            None => out.push_str(&format!("The API SHALL provide `{call}`.\n")),
        }
        if op.get("deprecated").and_then(Value::as_bool) == Some(true) {
            out.push_str("\nThis operation is deprecated.\n");
        }
        let required: Vec<&str> = op
            .get("parameters")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|p| p.get("required").and_then(Value::as_bool) == Some(true))
            .filter_map(|p| {
                p.get("name")
                    .and_then(Value::as_str)
                    .or_else(|| ref_name(p))
            })
            .collect();
        let mut when = format!("{call} is called");
        if !required.is_empty() {
            when.push_str(&format!(" with {}", required.join(", ")));
        }
        if op.get("requestBody").is_some() {
            when.push_str(" and a request body");
        }
        let empty = serde_json::Map::new();
        let responses = op
            .get("responses")
            .and_then(Value::as_object)
            .unwrap_or(&empty);
        if responses.is_empty() {
            out.push_str(&format!(
                "\n#### Scenario: Call\n- **WHEN** {when}\n- **THEN** the API answers\n"
            ));
        }
        for (code, resp) in responses {
            let what = resp
                .get("description")
                .and_then(Value::as_str)
                .map(one_line)
                .or_else(|| ref_name(resp).map(str::to_string))
                .unwrap_or_default();
            let then = if what.is_empty() {
                format!("the API responds with {code}")
            } else {
                format!("the API responds with {code}: {what}")
            };
            out.push_str(&format!(
                "\n#### Scenario: Responds {code}\n- **WHEN** {when}\n- **THEN** {then}\n"
            ));
        }
    }
    out
}

/// Reads OpenAPI and Swagger descriptions under `dir` (one file, or a folder)
/// and makes one input for each tag.
pub(super) fn find_openapi(dir: &Path) -> io::Result<Found> {
    let mut found = Found::default();
    let mut files = Vec::new();
    if dir.is_file() {
        files.push(dir.to_path_buf());
    } else {
        collect_description_files(dir, &mut files)?;
    }
    for f in files {
        let text = fs::read_to_string(&f)?;
        if is_node(&text) {
            continue;
        }
        let Some(doc) = parse_description(&f, &text) else {
            found.unmapped.push(Unmapped {
                path: f,
                reason: "not an OpenAPI or Swagger description".into(),
            });
            continue;
        };
        let title = doc
            .pointer("/info/title")
            .and_then(Value::as_str)
            .unwrap_or("The API")
            .to_string();
        let mut by_tag: BTreeMap<String, Vec<(String, String, &Value)>> = BTreeMap::new();
        if let Some(paths) = doc.get("paths").and_then(Value::as_object) {
            for (path, item) in paths {
                for m in METHODS {
                    if let Some(op) = item.get(*m) {
                        let tag = op
                            .get("tags")
                            .and_then(Value::as_array)
                            .and_then(|t| t.first())
                            .and_then(Value::as_str)
                            .unwrap_or("api")
                            .to_string();
                        by_tag
                            .entry(tag)
                            .or_default()
                            .push(((*m).to_string(), path.clone(), op));
                    }
                }
            }
        }
        if by_tag.is_empty() {
            found.unmapped.push(Unmapped {
                path: f,
                reason: "an API description with no operations".into(),
            });
            continue;
        }
        for (tag, ops) in &by_tag {
            found.inputs.push(Input {
                name: tag.clone(),
                path: f.clone(),
                text: operations_text(&title, tag, ops),
            });
        }
        for key in ["webhooks", "callbacks"] {
            if doc.get(key).is_some() {
                found.unmapped.push(Unmapped {
                    path: f.clone(),
                    reason: format!("`{key}` are not imported"),
                });
            }
        }
    }
    Ok(found)
}
