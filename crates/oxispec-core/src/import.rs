//! Import: specs written for another tool, read as requirement nodes.
//!
//! Six sources are understood: OpenSpec (`openspec/specs/<capability>/spec.md`),
//! Spec Kit (`specs/<feature>/spec.md`), Kiro (`.kiro/specs/<feature>/requirements.md`),
//! plain markdown, a product requirements document, and an OpenAPI or Swagger file. Each requirement in a spec file becomes a requirement node of its own,
//! with the scenarios under it. What has no place in a
//! node (design notes, tasks, active changes) is listed, not dropped silently.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) mod sources;
use sources::{find_openapi, prd};

pub const NAMES: &[&str] = &["openspec", "speckit", "kiro", "markdown", "prd", "openapi"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    OpenSpec,
    SpecKit,
    Kiro,
    Markdown,
    /// A product requirements document written as markdown.
    Prd,
    /// An OpenAPI or Swagger description, in JSON or YAML.
    OpenApi,
}

impl Source {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "openspec" => Some(Self::OpenSpec),
            "speckit" | "spec-kit" => Some(Self::SpecKit),
            "kiro" => Some(Self::Kiro),
            "markdown" | "md" => Some(Self::Markdown),
            "prd" => Some(Self::Prd),
            "openapi" | "swagger" => Some(Self::OpenApi),
            _ => None,
        }
    }

    /// Where the tool keeps its specs. Plain markdown has no place of its own.
    pub fn default_dir(self) -> Option<&'static str> {
        match self {
            Self::OpenSpec => Some("openspec/specs"),
            Self::SpecKit => Some("specs"),
            Self::Kiro => Some(".kiro/specs"),
            Self::Markdown | Self::Prd | Self::OpenApi => None,
        }
    }
}

/// A spec file to read.
#[derive(Debug, Clone)]
pub struct Input {
    pub name: String,
    pub path: PathBuf,
    pub text: String,
}

/// A file or folder that is not imported, and why.
#[derive(Debug, Clone)]
pub struct Unmapped {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Default)]
pub struct Found {
    pub inputs: Vec<Input>,
    pub unmapped: Vec<Unmapped>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scenario {
    pub name: String,
    /// Keyword in capitals (`WHEN`) and the rest of the line.
    pub steps: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub title: String,
    pub statement: String,
    pub scenarios: Vec<Scenario>,
}

/// One spec file read as a requirement node, before it has an ID.
#[derive(Debug, Clone)]
pub struct Draft {
    pub name: String,
    pub path: PathBuf,
    pub intro: Option<String>,
    pub blocks: Vec<Block>,
    pub notes: Vec<String>,
}

fn sorted_entries(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    out.sort();
    Ok(out)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

fn is_node(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("---") else {
        return false;
    };
    rest.split("\n---")
        .next()
        .is_some_and(|head| head.lines().any(|l| l.starts_with("id:")))
}

fn why(name: &str) -> &'static str {
    match name {
        "design.md" => "design notes have no node type here",
        "tasks.md" => "tasks are not imported; a change set holds them",
        "proposal.md" => "a proposal belongs to a change set",
        "plan.md" | "research.md" | "data-model.md" | "quickstart.md" => {
            "planning notes are not imported"
        }
        _ => "has no matching node type",
    }
}

fn push_input(found: &mut Found, name: String, path: &Path) -> io::Result<()> {
    let text = fs::read_to_string(path)?;
    if is_node(&text) {
        found.unmapped.push(Unmapped {
            path: path.to_path_buf(),
            reason: ALREADY_NODE.into(),
        });
    } else {
        found.inputs.push(Input {
            name,
            path: path.to_path_buf(),
            text,
        });
    }
    Ok(())
}

fn collect_markdown(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for path in sorted_entries(dir)? {
        let name = file_name(&path);
        if path.is_dir() {
            if !name.starts_with('.') && name != "node_modules" {
                collect_markdown(&path, out)?;
            }
        } else if name.ends_with(".md") {
            out.push(path);
        }
    }
    Ok(())
}

/// Finds the spec files under `dir`, and what next to them is left out.
pub fn find(dir: &Path, source: Source) -> io::Result<Found> {
    let mut found = Found::default();
    if source == Source::OpenApi {
        return find_openapi(dir);
    }
    if matches!(source, Source::Markdown | Source::Prd) {
        let mut files = Vec::new();
        if dir.is_file() {
            files.push(dir.to_path_buf());
        } else {
            collect_markdown(dir, &mut files)?;
        }
        for f in files {
            let stem = f
                .file_stem()
                .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
            push_input(&mut found, stem, &f)?;
        }
        return Ok(found);
    }
    let main = if source == Source::Kiro {
        "requirements.md"
    } else {
        "spec.md"
    };
    for d in sorted_entries(dir)?.into_iter().filter(|p| p.is_dir()) {
        let spec = d.join(main);
        if !spec.is_file() {
            continue;
        }
        push_input(&mut found, file_name(&d), &spec)?;
        for other in sorted_entries(&d)? {
            if other == spec {
                continue;
            }
            let reason = if other.is_dir() {
                "a folder of supporting files".to_string()
            } else {
                why(&file_name(&other)).to_string()
            };
            found.unmapped.push(Unmapped {
                path: other,
                reason,
            });
        }
    }
    if source == Source::OpenSpec
        && let Some(changes) = dir
            .parent()
            .map(|p| p.join("changes"))
            .filter(|p| p.is_dir())
    {
        for d in sorted_entries(&changes)?.into_iter().filter(|p| p.is_dir()) {
            let reason = if file_name(&d) == "archive" {
                "archived changes are history, not specs"
            } else {
                "an open change; start a change set with `oxispec new change`"
            };
            found.unmapped.push(Unmapped {
                path: d,
                reason: reason.into(),
            });
        }
    }
    Ok(found)
}

pub(crate) struct Section {
    pub(crate) level: usize,
    pub(crate) title: String,
    pub(crate) body: Vec<String>,
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let level = line.bytes().take_while(|&b| b == b'#').count();
    let rest = line.get(level..)?;
    (1..=6)
        .contains(&level)
        .then(|| rest.strip_prefix(' '))
        .flatten()
        .map(|t| (level, t.trim()))
}

/// The lines before the first heading, and the sections. Headings in code
/// fences are text.
pub(crate) fn sections(text: &str) -> (Vec<String>, Vec<Section>) {
    let mut preamble = Vec::new();
    let mut out: Vec<Section> = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
        }
        let head = if fenced { None } else { heading(line) };
        match (head, out.last_mut()) {
            (Some((level, title)), _) => out.push(Section {
                level,
                title: title.to_string(),
                body: Vec::new(),
            }),
            (None, Some(s)) => s.body.push(line.to_string()),
            (None, None) => preamble.push(line.to_string()),
        }
    }
    (preamble, out)
}

pub(crate) fn text_of(lines: &[String]) -> String {
    lines.join("\n").trim().to_string()
}

fn unbold(s: &str) -> String {
    s.replace("**", "")
}

fn strip_anchor(s: &str) -> &str {
    match s.rfind("{#") {
        Some(i) if s.ends_with('}') => s[..i].trim_end(),
        _ => s,
    }
}

fn list_item(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if let Some(r) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
        return Some(r.trim());
    }
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    t.get(digits..)
        .and_then(|r| r.strip_prefix(". "))
        .filter(|_| digits > 0)
        .map(str::trim)
}

fn keyword(word: &str) -> Option<&'static str> {
    ["GIVEN", "WHEN", "THEN", "AND", "BUT"]
        .into_iter()
        .find(|k| word.eq_ignore_ascii_case(k))
}

/// A step such as `- **WHEN** a user logs in`.
fn step(line: &str) -> Option<(String, String)> {
    let item = unbold(list_item(line)?);
    let item = item.trim();
    let (first, rest) = item.split_once(char::is_whitespace).unwrap_or((item, ""));
    let kw = keyword(first.trim_end_matches(':'))?;
    Some((kw.into(), rest.trim().to_string()))
}

fn steps_of(lines: &[String]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in lines {
        if let Some(s) = step(line) {
            out.push(s);
        } else if let (Some(last), true) = (out.last_mut(), !line.trim().is_empty()) {
            last.1.push(' ');
            last.1.push_str(line.trim());
        }
    }
    out
}

/// `Given a, When b, Then c` split at the capitalised keywords.
fn given_when_then(text: &str) -> Vec<(String, String)> {
    let text = unbold(text);
    let mut marks: Vec<(usize, &'static str, usize)> = Vec::new();
    for (kw, word) in [("GIVEN", "Given"), ("WHEN", "When"), ("THEN", "Then")] {
        let mut from = 0;
        while let Some(i) = text[from..].find(word) {
            let at = from + i;
            let before_ok = at == 0 || text[..at].ends_with([' ', ',']);
            let after_ok = text[at + word.len()..].starts_with(' ');
            if before_ok && after_ok {
                marks.push((at, kw, word.len()));
            }
            from = at + word.len();
        }
    }
    marks.sort_by_key(|m| m.0);
    let mut out = Vec::new();
    for (n, (at, kw, len)) in marks.iter().enumerate() {
        let end = marks.get(n + 1).map_or(text.len(), |m| m.0);
        let body = text[at + len..end].trim().trim_end_matches(',').trim();
        out.push(((*kw).to_string(), body.to_string()));
    }
    out
}

fn openspec(text: &str) -> (Option<String>, Vec<Block>, Vec<String>) {
    let (_, secs) = sections(text);
    let mut intro = None;
    let mut blocks: Vec<Block> = Vec::new();
    let mut notes = Vec::new();
    for s in &secs {
        if s.level == 2 && s.title.eq_ignore_ascii_case("purpose") {
            intro = Some(text_of(&s.body)).filter(|t| !t.is_empty());
        } else if let (3, Some(title)) = (s.level, s.title.strip_prefix("Requirement:")) {
            blocks.push(Block {
                title: title.trim().to_string(),
                statement: text_of(&s.body),
                scenarios: Vec::new(),
            });
        } else if let (4, Some(name), Some(b)) = (
            s.level,
            s.title.strip_prefix("Scenario:"),
            blocks.last_mut(),
        ) {
            b.scenarios.push(Scenario {
                name: strip_anchor(name.trim()).to_string(),
                steps: steps_of(&s.body),
            });
        } else if s.level >= 2 && !s.title.eq_ignore_ascii_case("requirements") {
            notes.push(format!("section `{}` was left out", s.title));
        }
    }
    (intro, blocks, notes)
}

fn kiro_title(heading: &str, story: &str) -> String {
    let rest = heading
        .trim_start_matches(|c: char| c.is_alphabetic() || c.is_whitespace())
        .trim_start_matches(|c: char| c.is_ascii_digit())
        .trim_start_matches([':', '-', '.', ' ']);
    if !rest.is_empty() {
        return rest.to_string();
    }
    if let Some((_, goal)) = story.split_once("I want ") {
        let goal = goal.split(", so that").next().unwrap_or(goal);
        let goal: String = goal.chars().take(60).collect();
        let goal = goal.trim().trim_end_matches('.').to_string();
        let mut cs = goal.chars();
        if let Some(c) = cs.next() {
            return c.to_uppercase().chain(cs).collect();
        }
    }
    heading.to_string()
}

/// A criterion such as `WHEN a user logs in THEN the system SHALL greet them`.
fn criterion(n: usize, text: &str) -> Scenario {
    let text = unbold(text);
    let (when, then) = match text.split_once(" THEN ") {
        Some((w, t)) => (w.trim(), t.trim()),
        None => ("the criterion applies", text.trim()),
    };
    let when = ["WHEN ", "IF ", "WHILE ", "GIVEN "]
        .iter()
        .find_map(|p| when.strip_prefix(p))
        .unwrap_or(when);
    Scenario {
        name: format!("Criterion {n}"),
        steps: vec![("WHEN".into(), when.into()), ("THEN".into(), then.into())],
    }
}

fn kiro(text: &str) -> (Option<String>, Vec<Block>, Vec<String>) {
    let (_, secs) = sections(text);
    let mut intro = None;
    let mut blocks: Vec<Block> = Vec::new();
    let mut notes = Vec::new();
    for s in &secs {
        let lower = s.title.to_ascii_lowercase();
        if (2..=3).contains(&s.level) && lower.starts_with("requirement") && lower != "requirements"
        {
            let story = s
                .body
                .iter()
                .map(|l| unbold(l))
                .find_map(|l| {
                    l.trim()
                        .strip_prefix("User Story:")
                        .map(|r| r.trim().to_string())
                })
                .unwrap_or_default();
            blocks.push(Block {
                title: kiro_title(&s.title, &story),
                statement: story,
                scenarios: Vec::new(),
            });
        } else if s.level >= 3 && lower.contains("acceptance") && !blocks.is_empty() {
            let Some(b) = blocks.last_mut() else { continue };
            for line in &s.body {
                if let Some(item) = list_item(line) {
                    let n = b.scenarios.len() + 1;
                    b.scenarios.push(criterion(n, item));
                }
            }
        } else if s.level == 2 && lower.starts_with("introduction") {
            intro = Some(text_of(&s.body)).filter(|t| !t.is_empty());
        } else if s.level == 2 && lower != "requirements" {
            notes.push(format!("section `{}` was left out", s.title));
        }
    }
    (intro, blocks, notes)
}

/// `FR-001: System MUST ...` in a bullet, with or without bold.
fn functional(line: &str) -> Option<(String, String)> {
    let item = unbold(list_item(line)?);
    let (id, rest) = item.trim().split_once(':')?;
    let digits = id.strip_prefix("FR-")?;
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| (id.to_string(), rest.trim().to_string()))
}

fn speckit(text: &str) -> (Option<String>, Vec<Block>, Vec<String>) {
    let (_, secs) = sections(text);
    let mut stories: Vec<Block> = Vec::new();
    let mut reqs: Vec<Block> = Vec::new();
    let mut notes = Vec::new();
    for s in &secs {
        let lower = s.title.to_ascii_lowercase();
        if s.level == 3 && lower.starts_with("user story") {
            let title = s
                .title
                .split(" (")
                .next()
                .unwrap_or(&s.title)
                .replace(" - ", ": ");
            let story = s
                .body
                .iter()
                .map(|l| l.trim())
                .find(|l| !l.is_empty() && !l.starts_with("**") && list_item(l).is_none())
                .unwrap_or_default()
                .to_string();
            let scenarios = s
                .body
                .iter()
                .filter_map(|l| list_item(l))
                .map(given_when_then)
                .filter(|steps| !steps.is_empty())
                .enumerate()
                .map(|(i, steps)| Scenario {
                    name: format!("Acceptance {}", i + 1),
                    steps,
                })
                .collect();
            stories.push(Block {
                title,
                statement: story,
                scenarios,
            });
        } else if s.level == 2
            && !lower.contains("user scenarios")
            && !lower.contains("requirements")
        {
            notes.push(format!("section `{}` was left out", s.title));
        }
        let mut fenced = false;
        for line in &s.body {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
            }
            if let (false, Some((id, rest))) = (fenced, functional(line)) {
                reqs.push(Block {
                    title: id,
                    statement: rest,
                    scenarios: Vec::new(),
                });
            }
        }
    }
    stories.extend(reqs);
    (None, stories, notes)
}

fn markdown(text: &str, name: &str) -> (Option<String>, Vec<Block>, Vec<String>) {
    let (pre, secs) = sections(text);
    let level = secs.iter().map(|s| s.level).filter(|&l| l >= 2).min();
    let Some(level) = level else {
        let body = text_of(&pre)
            + &secs
                .iter()
                .map(|s| text_of(&s.body))
                .collect::<Vec<_>>()
                .join("\n");
        let title = secs.first().map_or(name, |s| s.title.as_str()).to_string();
        let block = Block {
            title,
            statement: body.trim().to_string(),
            scenarios: Vec::new(),
        };
        return (None, vec![block], Vec::new());
    };
    let mut intro_lines = pre;
    let mut blocks: Vec<Block> = Vec::new();
    for s in &secs {
        if s.level < level {
            intro_lines.extend(s.body.iter().cloned());
        } else if s.level == level {
            blocks.push(Block {
                title: s.title.clone(),
                statement: text_of(&s.body),
                scenarios: Vec::new(),
            });
        } else if let Some(b) = blocks.last_mut() {
            let sub = format!("**{}**\n{}", s.title, text_of(&s.body));
            b.statement = format!("{}\n\n{}", b.statement, sub).trim().to_string();
        }
    }
    let intro = Some(text_of(&intro_lines)).filter(|t| !t.is_empty());
    (intro, blocks, Vec::new())
}

fn has_normative(s: &str) -> bool {
    s.split(|c: char| !c.is_ascii_alphabetic())
        .any(|w| w == "SHALL" || w == "MUST")
}

// spec: REQ-IMPORT-0001
/// Reads one spec file. With `normative`, a statement without SHALL or MUST
/// gets a sentence in front of it that has one.
pub fn draft(input: &Input, source: Source, normative: bool) -> Draft {
    let (intro, mut blocks, mut notes) = match source {
        Source::OpenSpec => openspec(&input.text),
        Source::SpecKit => speckit(&input.text),
        Source::Kiro => kiro(&input.text),
        Source::Markdown => markdown(&input.text, &input.name),
        Source::Prd => prd(&input.text),
        // `find` turned the description into the OpenSpec layout.
        Source::OpenApi => openspec(&input.text),
    };
    for b in &mut blocks {
        if b.statement.is_empty() {
            notes.push(format!("`{}` has no text; fill it in", b.title));
            b.statement = "<describe the behavior>".into();
        }
        if normative && !has_normative(&b.statement) {
            notes.push(format!(
                "`{}` has no SHALL or MUST; a sentence with SHALL was put before it",
                b.title
            ));
            b.statement = format!("The system SHALL satisfy this requirement: {}", b.statement);
        }
        if b.scenarios.is_empty() {
            notes.push(format!(
                "`{}` has no scenario; a placeholder was written",
                b.title
            ));
            b.scenarios.push(Scenario {
                name: "<to fill in>".into(),
                steps: vec![
                    ("WHEN".into(), "<condition>".into()),
                    ("THEN".into(), "<outcome>".into()),
                ],
            });
        }
    }
    if blocks.is_empty() {
        notes.push("no requirements were found in this file".into());
    }
    Draft {
        name: input.name.clone(),
        path: input.path.clone(),
        intro,
        blocks,
        notes,
    }
}

/// The domain of the IDs for a spec of this name.
pub fn domain_of(name: &str) -> String {
    let name = name.trim_start_matches(|c: char| !c.is_ascii_alphabetic());
    let domain: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if domain.is_empty() {
        "IMPORTED".into()
    } else {
        domain
    }
}

/// Why a file is not imported when it already is an OxiSpec node.
pub const ALREADY_NODE: &str = "already an OxiSpec node";

impl Draft {
    /// Letters and digits of the name, in capitals, without a leading number:
    /// `001-user-auth` gives `USERAUTH`.
    pub fn domain(&self) -> String {
        let name = self
            .name
            .trim_start_matches(|c: char| !c.is_ascii_alphabetic());
        let domain: String = name
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_uppercase())
            .collect();
        if domain.is_empty() {
            "IMPORTED".into()
        } else {
            domain
        }
    }

    /// How many nodes the file makes: one for each requirement, and one for a
    /// file with none, so that its notes have a place.
    pub fn node_count(&self) -> usize {
        self.blocks.len().max(1)
    }

    /// The notes about requirement `index`: those that name it, and the ones
    /// that name no requirement, which go with the first.
    pub fn notes_for(&self, index: usize) -> Vec<String> {
        let named = |b: &Block| format!("`{}`", b.title);
        let own = self.blocks.get(index).map(named);
        self.notes
            .iter()
            .filter(|n| {
                let mine = own.as_ref().is_some_and(|t| n.contains(t.as_str()));
                let other = self
                    .blocks
                    .iter()
                    .enumerate()
                    .any(|(i, b)| i != index && n.contains(&named(b)));
                mine || (index == 0 && !other)
            })
            .cloned()
            .collect()
    }

    /// The node file of requirement `index`: front matter, the purpose of the
    /// spec (in the first node only) and the requirement with its scenarios,
    /// which are numbered from S1 in each node.
    pub fn render_block(
        &self,
        index: usize,
        id: &str,
        status: &str,
        date: Option<&str>,
        source: &str,
    ) -> String {
        let mut out = format!("---\nid: {id}\ntype: requirement\nstatus: {status}\n");
        if let Some(date) = date {
            out.push_str(&format!("date: {date}\n"));
        }
        out.push_str(&format!("---\n<!-- imported from {source} -->\n"));
        if index == 0
            && let Some(intro) = &self.intro
        {
            out.push_str(&format!("\n## Purpose\n\n{intro}\n"));
        }
        if let Some(b) = self.blocks.get(index) {
            out.push_str(&format!(
                "\n### Requirement: {}\n{}\n",
                b.title, b.statement
            ));
            for (i, s) in b.scenarios.iter().enumerate() {
                out.push_str(&format!("\n#### Scenario: {} {{#S{}}}\n", s.name, i + 1));
                for (kw, text) in &s.steps {
                    out.push_str(&format!("- **{kw}** {text}\n"));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests_of_import {
    use super::*;

    fn input(name: &str, text: &str) -> Input {
        Input {
            name: name.into(),
            path: PathBuf::from(format!("{name}.md")),
            text: text.into(),
        }
    }

    // verifies: REQ-IMPORT-0001.S1
    #[test]
    fn openspec_blocks_and_scenarios_are_kept() {
        let text = "# Auth Specification\n\n## Purpose\nLet people in.\n\n## Requirements\n\n### Requirement: Login\nThe system SHALL sign users in.\n\n#### Scenario: Good password\n- **WHEN** a user gives the right password\n- **THEN** a session starts\n- **AND** it is logged\n";
        let d = draft(&input("auth", text), Source::OpenSpec, true);
        assert_eq!(d.intro.as_deref(), Some("Let people in."));
        assert_eq!(d.blocks.len(), 1);
        let b = &d.blocks[0];
        assert_eq!(b.title, "Login");
        assert_eq!(b.scenarios[0].steps.len(), 3);
        assert_eq!(b.scenarios[0].steps[2].0, "AND");
        assert!(d.notes.is_empty(), "{:?}", d.notes);
    }

    // verifies: REQ-IMPORT-0001.S2
    #[test]
    fn kiro_criteria_split_at_then() {
        let text = "# Requirements\n\n## Introduction\nA tool.\n\n## Requirements\n\n### Requirement 1\n\n**User Story:** As a user, I want to log in, so that I can work.\n\n#### Acceptance Criteria\n\n1. WHEN a user gives a password THEN the system SHALL start a session\n2. IF the password is wrong THEN the system SHALL refuse\n";
        let d = draft(&input("login", text), Source::Kiro, true);
        let b = &d.blocks[0];
        assert_eq!(b.title, "To log in");
        assert!(b.statement.starts_with("The system SHALL satisfy"));
        assert_eq!(b.scenarios.len(), 2);
        assert_eq!(
            b.scenarios[1].steps[0],
            ("WHEN".to_string(), "the password is wrong".to_string())
        );
    }

    // verifies: REQ-IMPORT-0001.S3
    #[test]
    fn speckit_stories_and_functional_requirements() {
        let text = "# Feature Specification: Search\n\n## User Scenarios & Testing\n\n### User Story 1 - Find a file (Priority: P1)\n\nA user types a word and sees the files.\n\n**Acceptance Scenarios**:\n\n1. **Given** an index, **When** the user types a word, **Then** matching files are listed\n\n## Requirements\n\n- **FR-001**: System MUST rank by title.\n\n## Key Entities\n- File\n";
        let d = draft(&input("001-search", text), Source::SpecKit, true);
        assert_eq!(d.domain(), "SEARCH");
        assert_eq!(d.blocks.len(), 2);
        assert_eq!(d.blocks[0].title, "User Story 1: Find a file");
        assert_eq!(d.blocks[0].scenarios[0].steps.len(), 3);
        assert_eq!(d.blocks[1].title, "FR-001");
        assert_eq!(d.blocks[1].statement, "System MUST rank by title.");
        assert!(d.notes.iter().any(|n| n.contains("Key Entities")));
        assert!(
            d.notes
                .iter()
                .any(|n| n.contains("FR-001") && n.contains("scenario"))
        );
    }

    #[test]
    fn plain_markdown_headings_become_blocks() {
        let text = "# Notes\n\nIntro line.\n\n## Retry\nFailed jobs are retried.\n\n### Limit\nThree times.\n\n## Timeout\nJobs stop after a minute.\n";
        let d = draft(&input("notes", text), Source::Markdown, false);
        assert_eq!(d.blocks.len(), 2);
        assert_eq!(d.blocks[0].title, "Retry");
        assert!(d.blocks[0].statement.contains("Three times."));
        assert_eq!(d.intro.as_deref(), Some("Intro line."));
    }

    #[test]
    fn a_rendered_node_has_numbered_anchors() {
        let text = "### Requirement: A\nThe system SHALL a.\n#### Scenario: one\n- WHEN x\n- THEN y\n#### Scenario: two\n- WHEN x\n- THEN z\n";
        let d = draft(&input("a", text), Source::OpenSpec, true);
        let out = d.render_block(0, "REQ-A-0001", "draft", None, "a.md");
        assert!(out.contains("#### Scenario: one {#S1}"));
        assert!(out.contains("#### Scenario: two {#S2}"));
        assert!(out.contains("- **WHEN** x"));
    }

    // verifies: REQ-IMPORT-0001.S6
    #[test]
    fn each_requirement_of_a_file_is_a_node_of_its_own() {
        let text = "## Purpose\nWhy.\n\n### Requirement: A\nThe system SHALL a.\n#### Scenario: one\n- WHEN x\n- THEN y\n#### Scenario: two\n- WHEN x\n- THEN z\n### Requirement: B\nThe system SHALL b.\n#### Scenario: three\n- WHEN p\n- THEN q\n";
        let d = draft(&input("a", text), Source::OpenSpec, true);
        assert_eq!(d.node_count(), 2);
        let first = d.render_block(0, "REQ-A-0001", "draft", None, "a.md");
        let second = d.render_block(1, "REQ-A-0002", "draft", None, "a.md");
        assert!(first.contains("### Requirement: A") && !first.contains("Requirement: B"));
        assert!(second.contains("### Requirement: B") && !second.contains("Requirement: A"));
        // The numbers start again in each node, and the purpose is in the first only.
        assert!(second.contains("#### Scenario: three {#S1}"), "{second}");
        assert!(first.contains("## Purpose") && !second.contains("## Purpose"));
    }

    #[test]
    fn notes_go_with_the_requirement_they_name() {
        let text = "### Requirement: A\nThe system SHALL a.\n### Requirement: B\nThe system SHALL b.\n#### Scenario: s\n- WHEN p\n- THEN q\n";
        let d = draft(&input("a", text), Source::OpenSpec, true);
        assert!(
            d.notes_for(0).iter().all(|n| n.contains("`A`")),
            "{:?}",
            d.notes
        );
        assert!(!d.notes_for(0).is_empty());
        assert!(d.notes_for(1).is_empty(), "{:?}", d.notes_for(1));
        let empty = draft(&input("e", "# Nothing\n"), Source::OpenSpec, true);
        assert_eq!(empty.node_count(), 1);
        assert_eq!(empty.notes_for(0), empty.notes);
    }

    #[test]
    fn a_file_with_an_id_is_already_a_node() {
        assert!(is_node("---\nid: REQ-A-0001\ntype: requirement\n---\nx"));
        assert!(!is_node("# Title\n---\nid: x"));
    }

    // verifies: REQ-IMPORT-0001.S4
    #[test]
    fn prd_bullets_become_requirements_with_criteria() {
        let text = "# Checkout PRD\n\n## Overview\nLet shoppers pay.\n\n## Non-goals\nRefunds.\n\n## Functional requirements\n- FR-1: Shoppers can pay by card\n  - WHEN the card is valid THEN the order is placed\n- As a shopper, I want to save my cart, so that I can return later\n\n## Out of scope\n- Gift cards\n\n## Open questions\n- Which PSP?\n";
        let d = draft(&input("checkout", text), Source::Prd, true);
        assert_eq!(d.blocks.len(), 2, "{:?}", d.blocks);
        assert_eq!(d.blocks[0].title, "FR-1 Shoppers can pay by card");
        assert_eq!(d.blocks[0].scenarios.len(), 1);
        assert_eq!(d.blocks[1].title, "To save my cart");
        assert!(
            d.intro
                .as_deref()
                .is_some_and(|i| i.contains("Let shoppers pay"))
        );
        assert!(d.notes.iter().any(|n| n.contains("Non-goals")));
        assert!(d.notes.iter().any(|n| n.contains("Open questions")));
        assert!(d.notes.iter().any(|n| n.contains("Out of scope")));
        assert_eq!(d.blocks.len(), 2);
    }

    // verifies: REQ-IMPORT-0001.S5
    #[test]
    fn openapi_operations_become_requirements_by_tag() {
        let dir = std::env::temp_dir().join(format!("oxispec-openapi-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("api.yaml");
        std::fs::write(
            &file,
            "openapi: 3.0.0\ninfo:\n  title: Shop\npaths:\n  /orders/{id}:\n    get:\n      tags: [orders]\n      summary: Fetch an order\n      parameters:\n        - name: id\n          in: path\n          required: true\n      responses:\n        '200':\n          description: The order\n        '404':\n          description: No such order\n  /health:\n    get:\n      responses:\n        '200':\n          description: Fine\n",
        )
        .unwrap();
        std::fs::write(dir.join("notes.yaml"), "a: b\n").unwrap();
        let found = find(&dir, Source::OpenApi).unwrap();
        assert_eq!(found.unmapped.len(), 1);
        let names: Vec<_> = found.inputs.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["api", "orders"]);
        let orders = found.inputs.iter().find(|i| i.name == "orders").unwrap();
        let d = draft(orders, Source::OpenApi, true);
        assert_eq!(d.blocks.len(), 1);
        assert_eq!(d.blocks[0].title, "GET /orders/{id}");
        assert!(d.blocks[0].statement.contains("Fetch an order"));
        assert_eq!(d.blocks[0].scenarios.len(), 2);
        assert!(d.notes.is_empty(), "{:?}", d.notes);
        std::fs::remove_dir_all(&dir).ok();
    }
}
