//! Decisions in other formats: converting one ADR format to another, and
//! reading the records of other tools.
//!
//! Every format is read the same way. The status, date and deciders go to front
//! matter, who supersedes whom becomes a link, and the text is cut into sections.
//! Sections that two formats share by meaning are matched (`Context` and
//! `Context and Problem Statement`, `Decision` and `Decision Outcome`); the rest
//! are kept as they are, so a conversion never drops text. A section that only
//! holds the placeholder of its format counts as empty and is not carried over.

use std::collections::BTreeMap;
use std::path::Path;

use crate::formats::{self, Profile};
use crate::parse;

/// Cannot convert or import.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Msg(String),
}

fn fail<T>(message: impl Into<String>) -> Result<T, Error> {
    Err(Error::Msg(message.into()))
}

/// A `##` or `###` section, with the text under its heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sec {
    pub name: String,
    pub text: String,
}

/// A record cut into the parts a conversion moves around.
#[derive(Debug, Default, Clone)]
pub struct Parts {
    /// The first `# ` heading, as written.
    pub title: String,
    /// Text between the title and the first section.
    pub preamble: String,
    pub sections: Vec<Sec>,
}

fn trim_blank(text: &str) -> String {
    text.trim_matches(|c| c == '\n' || c == '\r')
        .trim_end()
        .to_string()
}

/// Cuts a body into the title, the text before the first section, and the
/// sections. Headings inside code fences are text.
pub fn parse_body(body: &str) -> Parts {
    let mut parts = Parts::default();
    let mut in_fence = false;
    let mut current: Option<(String, Vec<&str>)> = None;
    let mut preamble: Vec<&str> = Vec::new();
    let mut seen_title = false;
    let flush = |current: &mut Option<(String, Vec<&str>)>, sections: &mut Vec<Sec>| {
        if let Some((name, lines)) = current.take() {
            sections.push(Sec {
                name,
                text: trim_blank(&lines.join("\n")),
            });
        }
    };
    for line in body.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
        }
        if !in_fence {
            let level = line.chars().take_while(|&c| c == '#').count();
            if let Some(text) = line[level..].strip_prefix(' ') {
                if level == 1 && !seen_title {
                    seen_title = true;
                    parts.title = text.trim().to_string();
                    continue;
                }
                if (2..=3).contains(&level) {
                    flush(&mut current, &mut parts.sections);
                    current = Some((text.trim().trim_end_matches('#').trim().to_string(), vec![]));
                    continue;
                }
            }
        }
        match &mut current {
            Some((_, lines)) => lines.push(line),
            None => preamble.push(line),
        }
    }
    flush(&mut current, &mut parts.sections);
    parts.preamble = trim_blank(&preamble.join("\n"));
    parts
}

/// The meaning of a section, for matching it between formats.
fn role(name: &str) -> String {
    let name = name.trim().to_lowercase();
    match name.as_str() {
        "context and problem statement" => "context".into(),
        "decision outcome" => "decision".into(),
        _ => name,
    }
}

/// True when a section has no text of its own: it is empty or is the
/// placeholder one of the formats writes.
fn placeholder(text: &str) -> bool {
    let text = text.trim();
    text.is_empty()
        || formats::builtin()
            .iter()
            .flat_map(|p| p.sections.iter())
            .any(|s| !s.hint.is_empty() && s.hint.trim() == text)
}

fn emit(out: &mut String, level: u8, name: &str, text: &str) {
    out.push_str(&format!("\n{} {name}\n", "#".repeat(usize::from(level))));
    if !text.trim().is_empty() {
        out.push_str(&format!("\n{}\n", text.trim_end()));
    }
}

/// What a conversion produced: the text, and what the author should look at.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub text: String,
    pub notes: Vec<String>,
}

/// Writes the parts as a body in the target format. The sections of the
/// profile come first, in its order and at its levels, then the sections it
/// has no place for, as they were.
pub fn render(parts: &Parts, target: &Profile) -> Rendered {
    let mut pool: Vec<Option<Sec>> = parts.sections.iter().cloned().map(Some).collect();
    let mut notes = Vec::new();
    let mut out = format!("# {}\n", parts.title);
    if !parts.preamble.is_empty() {
        out.push_str(&format!("\n{}\n", parts.preamble));
    }
    for want in &target.sections {
        let key = role(&want.name);
        let found = pool
            .iter_mut()
            .find(|s| s.as_ref().is_some_and(|s| role(&s.name) == key))
            .and_then(Option::take);
        match found {
            Some(s) if !placeholder(&s.text) => emit(&mut out, want.level, &want.name, &s.text),
            _ if want.required => {
                emit(&mut out, want.level, &want.name, &want.hint);
                notes.push(format!("`{}` is new and still to be written", want.name));
            }
            _ => {}
        }
    }
    for s in pool.into_iter().flatten() {
        if placeholder(&s.text) {
            continue;
        }
        emit(&mut out, 2, &s.name, &s.text);
        // A common section that the text keeps whole; saying so for every record is noise.
        if !s.name.eq_ignore_ascii_case("alternatives considered") {
            notes.push(format!(
                "`{}` has no place in `{}`; kept as it was",
                s.name, target.name
            ));
        }
    }
    Rendered { text: out, notes }
}

/// Converts the text of a record to the format of `target`. The front matter
/// stays as it is; only the body changes.
pub fn convert(text: &str, target: &Profile) -> Result<Rendered, Error> {
    let text = text.replace("\r\n", "\n");
    let Some(span) = parse::frontmatter_span(&text) else {
        return fail("the record has no front matter");
    };
    let head = &text[..span.body_start];
    let parts = parse_body(&text[span.body_start..]);
    if parts.title.is_empty() {
        return fail("the record has no `# ` title");
    }
    let mut done = render(&parts, target);
    let front: BTreeMap<String, serde_json::Value> =
        serde_saphyr::from_str(&text[span.yaml.clone()]).unwrap_or_default();
    match front.get("status").and_then(|v| v.as_str()) {
        None if target.fields.iter().any(|f| f == "status") => {
            done.notes.push("it has no `status`".into());
        }
        Some(s) if !target.statuses.is_empty() && !target.statuses.iter().any(|t| t == s) => {
            done.notes.push(format!(
                "the status `{s}` is not one `{}` allows ({})",
                target.name,
                target.statuses.join(", ")
            ));
        }
        _ => {}
    }
    done.text = format!("{head}{}", done.text);
    Ok(done)
}

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

/// The tool whose records are read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// `adr-tools`: `0001-title.md`, `# 1. Title`, `Date:` and a `## Status` section.
    AdrTools,
    /// MADR: `0001-title.md`, front matter (4.0) or bullet lines (2.x, 3.x).
    Madr,
    /// log4brains: `20200101-title.md`, MADR 2 bullet lines and `Tags:`.
    Log4brains,
}

impl Dialect {
    pub fn parse(name: &str) -> Option<Dialect> {
        match name {
            "adr-tools" => Some(Dialect::AdrTools),
            "madr" => Some(Dialect::Madr),
            "log4brains" => Some(Dialect::Log4brains),
            _ => None,
        }
    }

    pub const NAMES: &'static [&'static str] = &["adr-tools", "madr", "log4brains"];

    /// Where the tool keeps its records, relative to a project.
    pub fn default_dir(self) -> &'static str {
        match self {
            Dialect::AdrTools => "doc/adr",
            Dialect::Madr => "docs/decisions",
            Dialect::Log4brains => "docs/adr",
        }
    }
}

/// How a record names another: by file, or only by number.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Ref {
    File(String),
    Number(u32),
}

/// A link line: `(kind, this record is the one acting, the other record)`.
#[derive(Debug, Clone)]
struct Edge {
    kind: &'static str,
    this_acts: bool,
    other: Ref,
}

#[derive(Debug, Default)]
struct Foreign {
    title: String,
    status: Option<String>,
    date: Option<String>,
    decision_makers: Vec<String>,
    edges: Vec<Edge>,
    /// `Key: value` lines with no field to go to, kept in a section.
    metadata: Vec<String>,
    parts: Parts,
}

const LINK_WORDS: &[(&str, &str, bool)] = &[
    ("supercedes", "supersedes", true),
    ("supersedes", "supersedes", true),
    ("superceded by", "supersedes", false),
    ("superseded by", "supersedes", false),
    ("amends", "amends", true),
    ("amended by", "amends", false),
    ("clarifies", "clarifies", true),
    ("clarified by", "clarifies", false),
];

const STATUSES: &[&str] = &[
    "proposed",
    "accepted",
    "rejected",
    "deprecated",
    "superseded",
];

fn reference(rest: &str) -> Option<Ref> {
    if let Some(start) = rest.find("](") {
        let inner = &rest[start + 2..];
        let target = inner.split(')').next().unwrap_or("");
        let target = target.split('#').next().unwrap_or("");
        let name = target.rsplit('/').next().unwrap_or("");
        if !name.is_empty() {
            return Some(Ref::File(name.to_string()));
        }
    }
    let digits: String = rest
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok().map(Ref::Number)
}

/// Reads one status line: a status word, or a link to another record.
fn status_line(line: &str, into: &mut Foreign) {
    let line = line
        .trim()
        .trim_start_matches(['*', '-'])
        .trim()
        .to_string();
    if line.is_empty() {
        return;
    }
    let lower = line.to_ascii_lowercase();
    for (word, kind, this_acts) in LINK_WORDS {
        if let Some(rest) = lower.strip_prefix(word) {
            let rest = &line[line.len() - rest.len()..];
            if let Some(other) = reference(rest) {
                into.edges.push(Edge {
                    kind,
                    this_acts: *this_acts,
                    other,
                });
                if *kind == "supersedes" && !this_acts {
                    into.status = Some("superseded".into());
                }
                return;
            }
        }
    }
    // A status word may lead a longer line: `Accepted (built)`, `Accepted, amended by ADR-0032`.
    let end = lower
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(lower.len());
    let word = &lower[..end];
    if STATUSES.contains(&word) {
        if into.status.as_deref() != Some("superseded") {
            into.status = Some(word.to_string());
        }
        let rest = lower[end..].trim_matches(|c: char| c.is_whitespace() || ".,;:-—–".contains(c));
        if !rest.is_empty() {
            // What follows is kept as a note; a link word in it still makes a link.
            for (link, kind, this_acts) in LINK_WORDS {
                if let Some(at) = lower[end..].find(link)
                    && let Some(other) = reference(&line[end + at + link.len()..])
                {
                    into.edges.push(Edge {
                        kind,
                        this_acts: *this_acts,
                        other,
                    });
                    break;
                }
            }
            into.metadata.push(format!("Status: {line}"));
        }
    } else {
        into.metadata.push(format!("Status: {line}"));
    }
}

fn scalar_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => {
            items.iter().map(scalar_text).collect::<Vec<_>>().join(", ")
        }
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn people(text: &str) -> Vec<String> {
    text.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// A `Key: value` line of the header, with or without a bullet.
fn header_line(line: &str) -> Option<(String, String)> {
    let bullet = line.trim_start().starts_with(['*', '-']);
    let text = line.trim().trim_start_matches(['*', '-']).trim();
    let (key, value) = text.split_once(':')?;
    let key = key.trim().trim_matches('*').trim();
    let known = [
        "date",
        "status",
        "deciders",
        "decision-makers",
        "decision makers",
        "tags",
        "technical story",
        "consulted",
        "informed",
    ];
    let plausible = !key.is_empty() && key.split_whitespace().count() <= 4;
    if (bullet && plausible) || known.contains(&key.to_lowercase().as_str()) {
        Some((key.to_string(), value.trim().to_string()))
    } else {
        None
    }
}

fn field(into: &mut Foreign, key: &str, value: &str) {
    match key.to_lowercase().as_str() {
        "status" => {
            for part in value.split('\n') {
                status_line(part, into);
            }
        }
        "date" if !value.is_empty() => into.date = Some(value.to_string()),
        "deciders" | "decision-makers" | "decision makers" => {
            into.decision_makers.extend(people(value))
        }
        _ if value.is_empty() => {}
        _ => into.metadata.push(format!("{key}: {value}")),
    }
}

fn read_foreign(text: &str) -> Result<Foreign, Error> {
    let text = text.replace("\r\n", "\n");
    let mut into = Foreign::default();
    let body = match parse::frontmatter_span(&text) {
        Some(span) => {
            let front: BTreeMap<String, serde_json::Value> =
                serde_saphyr::from_str(&text[span.yaml.clone()]).unwrap_or_default();
            for (key, value) in &front {
                field(&mut into, key, &scalar_text(value));
            }
            &text[span.body_start..]
        }
        None => &text[..],
    };
    let mut parts = parse_body(body);
    if parts.title.is_empty() {
        return fail("no `# ` title");
    }
    // `# 1. Title` of adr-tools.
    let numbered = parts.title.trim_start_matches(|c: char| c.is_ascii_digit());
    if let Some(rest) = numbered.strip_prefix(". ")
        && numbered.len() != parts.title.len()
    {
        parts.title = rest.trim().to_string();
    }
    // The header: lines of `Key: value` before the first section.
    let mut keep = Vec::new();
    for line in parts.preamble.lines() {
        match header_line(line) {
            Some((key, value)) => field(&mut into, &key, &value),
            None => keep.push(line),
        }
    }
    parts.preamble = trim_blank(&keep.join("\n"));
    // A `## Status` section holds the status and the links.
    if let Some(at) = parts
        .sections
        .iter()
        .position(|s| s.name.eq_ignore_ascii_case("status"))
    {
        let section = parts.sections.remove(at);
        for line in section.text.lines() {
            status_line(line, &mut into);
        }
    }
    into.title = parts.title.clone();
    into.parts = parts;
    Ok(into)
}

/// One record ready to be written.
#[derive(Debug, Clone)]
pub struct Imported {
    pub id: String,
    /// The path to write, relative to the project: `specs/adr/0001-title.md`.
    pub path: String,
    /// The file it was read from.
    pub source: String,
    pub text: String,
    pub notes: Vec<String>,
}

/// The number at the start of a file name, after an optional `adr-`.
fn leading_number(file: &str) -> Option<(u32, &str)> {
    let name = file.strip_prefix("adr-").unwrap_or(file);
    let digits = name.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits > 9 {
        return None;
    }
    let rest = name[digits..].trim_start_matches('-');
    Some((name[..digits].parse().ok()?, rest))
}

fn slug_of(stem: &str, title: &str) -> String {
    let mut out = String::new();
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if !out.is_empty() {
        return out;
    }
    let mut out = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn quoted(text: &str) -> String {
    let plain = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' '))
        && !text.starts_with(' ')
        && !text.ends_with(' ');
    if plain {
        text.to_string()
    } else {
        format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

fn front_matter(id: &str, rec: &Foreign, links: &BTreeMap<&str, Vec<String>>) -> String {
    let mut out = format!("---\nid: {id}\ntype: adr\n");
    if let Some(status) = &rec.status {
        out.push_str(&format!("status: {status}\n"));
    }
    if let Some(date) = &rec.date {
        out.push_str(&format!("date: {}\n", quoted(date)));
    }
    if !rec.decision_makers.is_empty() {
        let list: Vec<String> = rec.decision_makers.iter().map(|p| quoted(p)).collect();
        out.push_str(&format!("decision-makers: [{}]\n", list.join(", ")));
    }
    if !links.is_empty() {
        out.push_str("links:\n");
        for (kind, targets) in links {
            out.push_str(&format!("  {kind}: [{}]\n", targets.join(", ")));
        }
    }
    out.push_str("---\n");
    out
}

/// Reads the records of another tool and writes them in the format of
/// `target`. `files` are `(file name, text)`. Ids come from the numbers in the
/// file names (log4brains: from their order), plus `offset`.
pub fn import(
    files: &[(String, String)],
    dialect: Dialect,
    offset: u32,
    target: &Profile,
) -> Result<Vec<Imported>, Error> {
    let mut skipped = Vec::new();
    let mut records: Vec<(String, u32, String, Foreign)> = Vec::new();
    let mut sorted: Vec<&(String, String)> = files.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, text) in sorted {
        let Some(stem) = name.strip_suffix(".md") else {
            continue;
        };
        let Some((number, rest)) = leading_number(stem) else {
            skipped.push(name.clone());
            continue;
        };
        let rec = read_foreign(text).map_err(|Error::Msg(m)| Error::Msg(format!("{name}: {m}")))?;
        let n = match dialect {
            Dialect::Log4brains => records.len() as u32 + 1,
            _ => number,
        };
        records.push((name.clone(), n + offset, rest.to_string(), rec));
    }
    if records.is_empty() {
        return fail("no records found; a record is a `.md` file whose name starts with a number");
    }
    let mut seen = BTreeMap::new();
    for (name, n, _, _) in &records {
        if let Some(other) = seen.insert(*n, name) {
            return fail(format!("{other} and {name} have the same number {n}"));
        }
    }
    let id_of = |n: u32| format!("ADR-{n:04}");
    let by_file: BTreeMap<&str, u32> = records
        .iter()
        .map(|(name, n, _, _)| (name.as_str(), *n))
        .collect();
    let resolve = |other: &Ref| -> Option<u32> {
        match other {
            Ref::File(file) => by_file.get(file.as_str()).copied(),
            Ref::Number(n) if dialect != Dialect::Log4brains => {
                let n = n + offset;
                seen.contains_key(&n).then_some(n)
            }
            Ref::Number(_) => None,
        }
    };
    // Every link becomes a link of the record that acts.
    let mut links: BTreeMap<u32, BTreeMap<&str, Vec<String>>> = BTreeMap::new();
    let mut unresolved: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for (_, n, _, rec) in &records {
        for edge in &rec.edges {
            let Some(other) = resolve(&edge.other) else {
                unresolved.entry(*n).or_default().push(format!(
                    "a link to {} could not be resolved; it is not among the records",
                    match &edge.other {
                        Ref::File(f) => f.clone(),
                        Ref::Number(k) => k.to_string(),
                    }
                ));
                continue;
            };
            let (actor, subject) = if edge.this_acts {
                (*n, other)
            } else {
                (other, *n)
            };
            let list = links
                .entry(actor)
                .or_default()
                .entry(edge.kind)
                .or_default();
            let id = id_of(subject);
            if !list.contains(&id) {
                list.push(id);
            }
        }
    }
    let mut out = Vec::new();
    for (name, n, rest, rec) in &records {
        let id = id_of(*n);
        let mut parts = rec.parts.clone();
        parts.title = format!("{id}: {}", rec.title);
        if !rec.metadata.is_empty() {
            let lines: Vec<String> = rec.metadata.iter().map(|m| format!("- {m}")).collect();
            parts.sections.push(Sec {
                name: "Imported metadata".into(),
                text: lines.join("\n"),
            });
        }
        let mut done = render(&parts, target);
        // The importer made this section itself; there is nothing to warn about.
        done.notes.retain(|n| !n.starts_with("`Imported metadata`"));
        if rec.status.is_none() {
            done.notes.push("no status found; set one".into());
        }
        done.notes.extend(unresolved.remove(n).unwrap_or_default());
        let own = links.remove(n).unwrap_or_default();
        let path = format!("specs/adr/{n:04}-{}.md", slug_of(rest, &rec.title));
        out.push(Imported {
            id: id.clone(),
            path,
            source: name.clone(),
            text: format!("{}{}", front_matter(&id, rec, &own), done.text),
            notes: done.notes,
        });
    }
    if !skipped.is_empty() {
        let note = format!("skipped without a number: {}", skipped.join(", "));
        if let Some(first) = out.first_mut() {
            first.notes.push(note);
        }
    }
    Ok(out)
}

/// The `.md` files of a directory, as `(file name, text)`, for [`import`].
pub fn read_dir(dir: &Path) -> std::io::Result<Vec<(String, String)>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "md")
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
        {
            files.push((name.to_string(), std::fs::read_to_string(&path)?));
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests_of_adr {
    use super::*;

    fn profile(name: &str) -> Profile {
        formats::builtin()
            .into_iter()
            .find(|p| p.name == name)
            .expect("a built-in profile")
    }

    const NYGARD: &str = "---\nid: ADR-0007\ntype: adr\nstatus: accepted\ndate: 2026-01-02\n---\n# ADR-0007: Use Postgres\n\n## Context\n\nWe need transactions.\n\n## Decision\n\nWe will use Postgres.\n\n## Consequences\n\nOps must run it.\n";

    #[test]
    fn nygard_to_madr_maps_the_sections_and_adds_the_ones_madr_requires() {
        let done = convert(NYGARD, &profile("madr")).unwrap();
        assert!(
            done.text
                .contains("## Context and Problem Statement\n\nWe need transactions.")
        );
        assert!(
            done.text
                .contains("## Decision Outcome\n\nWe will use Postgres.")
        );
        assert!(done.text.contains("### Consequences\n\nOps must run it."));
        assert!(done.text.contains("## Considered Options"));
        assert!(done.notes.iter().any(|n| n.contains("Considered Options")));
        assert!(
            done.text.starts_with("---\nid: ADR-0007"),
            "front matter stays"
        );
    }

    #[test]
    fn a_round_trip_keeps_the_text_of_the_shared_sections() {
        let there = convert(NYGARD, &profile("madr")).unwrap();
        let back = convert(&there.text, &profile("nygard")).unwrap();
        assert_eq!(back.text, NYGARD);
        assert!(back.notes.is_empty(), "{:?}", back.notes);
    }

    #[test]
    fn to_a_y_statement_the_old_text_stays_below_the_skeleton() {
        let done = convert(NYGARD, &profile("y-statement")).unwrap();
        let statement = done.text.find("## Statement").unwrap();
        let context = done.text.find("## Context").unwrap();
        assert!(statement < context, "the skeleton comes first");
        assert!(done.text.contains("We need transactions."));
        let back = convert(&done.text, &profile("nygard")).unwrap();
        assert_eq!(
            back.text, NYGARD,
            "the unwritten skeleton does not come back"
        );
    }

    #[test]
    fn a_written_statement_is_not_lost_going_to_another_format() {
        let text = "---\nid: ADR-0001\ntype: adr\nstatus: accepted\n---\n# ADR-0001: Cache\n\n## Statement\n\nIn the context of reads, we decided for Redis.\n";
        let done = convert(text, &profile("nygard")).unwrap();
        assert!(
            done.text
                .contains("In the context of reads, we decided for Redis.")
        );
        assert!(done.notes.iter().any(|n| n.contains("Statement")));
    }

    #[test]
    fn a_status_the_target_does_not_allow_is_reported() {
        let text = NYGARD.replace("accepted", "rejected");
        let done = convert(&text, &profile("nygard")).unwrap();
        assert!(done.notes.iter().any(|n| n.contains("rejected")));
    }

    #[test]
    fn headings_in_code_fences_are_text() {
        let text = "---\nid: ADR-0001\ntype: adr\n---\n# ADR-0001: T\n\n## Context\n\n```\n## Decision\n```\n";
        let rendered = convert(text, &profile("madr")).unwrap();
        assert!(rendered.text.contains("```\n## Decision\n```"));
    }

    fn files(items: &[(&str, &str)]) -> Vec<(String, String)> {
        items
            .iter()
            .map(|(n, t)| (n.to_string(), t.to_string()))
            .collect()
    }

    const ADR_TOOLS_1: &str = "# 1. Record architecture decisions\n\nDate: 2016-02-12\n\n## Status\n\nSuperceded by [2. Use Rust](0002-use-rust.md)\n\n## Context\n\nWe need to record decisions.\n\n## Decision\n\nWe will record them.\n\n## Consequences\n\nSee Michael Nygard's article.\n";
    const ADR_TOOLS_2: &str = "# 2. Use Rust\n\nDate: 2016-03-01\n\n## Status\n\nAccepted\n\nSupercedes [1. Record architecture decisions](0001-record-architecture-decisions.md)\n\nAmended by [3. Use Rust 2024](0003-use-rust-2024.md)\n\n## Context\n\nSpeed.\n\n## Decision\n\nRust.\n\n## Consequences\n\nSlower builds.\n";
    const ADR_TOOLS_3: &str = "# 3. Use Rust 2024\n\nDate: 2016-04-01\n\n## Status\n\nAccepted\n\nAmends [2. Use Rust](0002-use-rust.md)\n\n## Context\n\nEditions.\n\n## Decision\n\nUse 2024.\n";

    #[test]
    fn adr_tools_records_become_nodes_with_links() {
        let got = import(
            &files(&[
                ("0001-record-architecture-decisions.md", ADR_TOOLS_1),
                ("0002-use-rust.md", ADR_TOOLS_2),
                ("0003-use-rust-2024.md", ADR_TOOLS_3),
            ]),
            Dialect::AdrTools,
            0,
            &profile("nygard"),
        )
        .unwrap();
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].id, "ADR-0001");
        assert_eq!(
            got[0].path,
            "specs/adr/0001-record-architecture-decisions.md"
        );
        assert!(got[0].text.contains("status: superseded"));
        assert!(got[0].text.contains("date: 2016-02-12"));
        assert!(
            got[0]
                .text
                .contains("# ADR-0001: Record architecture decisions")
        );
        assert!(
            got[0]
                .text
                .contains("## Context\n\nWe need to record decisions.")
        );
        // 2 supersedes 1 (said on both sides, written once); 3 amends 2.
        assert!(
            got[1].text.contains("supersedes: [ADR-0001]"),
            "{}",
            got[1].text
        );
        assert_eq!(
            got[1].text.matches("ADR-0001").count(),
            1,
            "said on both sides, written once"
        );
        assert!(
            got[2].text.contains("amends: [ADR-0002]"),
            "{}",
            got[2].text
        );
        assert!(
            !got[0].text.contains("Superceded"),
            "the status section is gone"
        );
    }

    #[test]
    fn a_link_to_a_record_that_is_not_there_is_reported() {
        let got = import(
            &files(&[("0002-use-rust.md", ADR_TOOLS_2)]),
            Dialect::AdrTools,
            0,
            &profile("nygard"),
        )
        .unwrap();
        assert!(
            got[0]
                .notes
                .iter()
                .any(|n| n.contains("could not be resolved"))
        );
        assert!(!got[0].text.contains("supersedes:"));
    }

    #[test]
    fn madr_four_reads_front_matter_and_keeps_what_has_no_field() {
        let madr = "---\nstatus: accepted\ndate: 2024-09-17\ndecision-makers: Ann, Bo\nconsulted: Cy\n---\n# Use Markdown\n\n## Context and Problem Statement\n\nWhy?\n\n## Considered Options\n\n* A\n* B\n\n## Decision Outcome\n\nChosen option: \"A\", because it is simple.\n\n### Consequences\n\n* Good, because easy\n";
        let got = import(
            &files(&[("0001-use-markdown.md", madr)]),
            Dialect::Madr,
            0,
            &profile("madr"),
        )
        .unwrap();
        let text = &got[0].text;
        assert!(text.contains("decision-makers: [Ann, Bo]"), "{text}");
        assert!(
            text.contains("## Imported metadata\n\n- consulted: Cy"),
            "{text}"
        );
        assert!(text.contains("### Consequences\n\n* Good, because easy"));
        assert!(text.contains("# ADR-0001: Use Markdown"));
    }

    #[test]
    fn madr_two_bullets_and_superseded_by_a_link() {
        let old = "# Old choice\n\n* Status: superseded by [ADR-0002](0002-new.md)\n* Deciders: Ann\n* Date: 2020-05-06\n\n## Context and Problem Statement\n\nWhy?\n\n## Decision Outcome\n\nChosen option: \"A\".\n";
        let new = "# New choice\n\n* Status: accepted\n* Date: 2021-01-01\n\n## Context and Problem Statement\n\nWhy again?\n\n## Decision Outcome\n\nChosen option: \"B\".\n";
        let got = import(
            &files(&[("0001-old.md", old), ("0002-new.md", new)]),
            Dialect::Madr,
            0,
            &profile("madr"),
        )
        .unwrap();
        assert!(got[0].text.contains("status: superseded"));
        assert!(got[0].text.contains("decision-makers: [Ann]"));
        assert!(
            got[1].text.contains("supersedes: [ADR-0001]"),
            "{}",
            got[1].text
        );
    }

    #[test]
    fn log4brains_records_are_numbered_by_date_order() {
        let a = "# Later\n\n- Status: accepted\n- Date: 2021-02-02\n- Tags: db\n\n## Context and Problem Statement\n\nC.\n";
        let b = "# Earlier\n\n- Status: accepted\n- Date: 2020-01-01\n\n## Context and Problem Statement\n\nC.\n";
        let got = import(
            &files(&[("20210202-later.md", a), ("20200101-earlier.md", b)]),
            Dialect::Log4brains,
            0,
            &profile("madr"),
        )
        .unwrap();
        assert_eq!(got[0].source, "20200101-earlier.md");
        assert_eq!(got[0].id, "ADR-0001");
        assert_eq!(got[1].id, "ADR-0002");
        assert!(got[1].text.contains("- Tags: db"));
    }

    #[test]
    fn the_offset_moves_every_number_and_link() {
        let got = import(
            &files(&[
                ("0001-record-architecture-decisions.md", ADR_TOOLS_1),
                ("0002-use-rust.md", ADR_TOOLS_2),
            ]),
            Dialect::AdrTools,
            10,
            &profile("nygard"),
        )
        .unwrap();
        assert_eq!(got[0].id, "ADR-0011");
        assert!(got[1].text.contains("supersedes: [ADR-0011]"));
    }

    #[test]
    fn files_without_a_number_are_skipped_and_said_so() {
        let got = import(
            &files(&[
                ("0001-record-architecture-decisions.md", ADR_TOOLS_1),
                ("template.md", "# Template\n"),
                ("README.md", "# Index\n"),
            ]),
            Dialect::AdrTools,
            0,
            &profile("nygard"),
        )
        .unwrap();
        assert_eq!(got.len(), 1);
        assert!(got[0].notes.iter().any(|n| n.contains("template.md")));
    }

    #[test]
    fn two_files_with_one_number_are_refused() {
        let err = import(
            &files(&[("0001-a.md", ADR_TOOLS_1), ("adr-0001-b.md", ADR_TOOLS_1)]),
            Dialect::Madr,
            0,
            &profile("nygard"),
        )
        .unwrap_err();
        assert!(err.to_string().contains("same number"));
    }

    // verifies: REQ-ADR-0002.S2
    #[test]
    fn a_status_word_with_a_tail_still_gives_the_status() {
        let one = "# 1. A\n\n**Status:** Accepted (built)\n\n## Context\n\nC.\n";
        let two =
            "# 2. B\n\n**Status:** Accepted, amended by ADR-0003 (now off)\n\n## Context\n\nC.\n";
        let three =
            "# 3. C\n\n**Status:** Accepted \u{2014} updated 2026-09-12\n\n## Context\n\nC.\n";
        let got = import(
            &files(&[("0001-a.md", one), ("0002-b.md", two), ("0003-c.md", three)]),
            Dialect::Madr,
            0,
            &profile("nygard"),
        )
        .unwrap();
        for g in &got {
            assert!(g.text.contains("status: accepted"), "{}", g.text);
        }
        assert!(got[0].text.contains("Status: Accepted (built)"));
        assert!(got[2].text.contains("updated 2026-09-12"));
        // ADR-0003 amends ADR-0002.
        assert!(got[2].text.contains("amends:"), "{}", got[2].text);
        assert!(got[2].text.contains("ADR-0002"));
    }

    #[test]
    fn an_unknown_status_line_is_kept_not_dropped() {
        let text = "# 1. T\n\nDate: 2020-01-01\n\n## Status\n\nAccepted\n\nRelates to [2. X](0002-x.md)\n\n## Context\n\nC.\n";
        let got = import(
            &files(&[("0001-t.md", text)]),
            Dialect::AdrTools,
            0,
            &profile("nygard"),
        )
        .unwrap();
        assert!(
            got[0]
                .text
                .contains("- Status: Relates to [2. X](0002-x.md)")
        );
    }
}
