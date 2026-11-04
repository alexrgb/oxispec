//! An import plan: what to make of each file of another tool, decided one
//! item at a time, looked at as a draft, and only then written.
//!
//! `plan` proposes a kind for each item (a spec becomes requirements, open
//! changes and design notes become change sets). A person or an agent can
//! change the kind, and the render functions show what each would write.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::adr::{self, Parts, Sec};
use crate::formats::Profile;
use crate::import::{self, Input, Source, sections, text_of};

/// What an item becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Requirement,
    Change,
    Adr,
    Skip,
}

impl Kind {
    pub const NAMES: &'static [&'static str] = &["requirement", "change", "adr", "skip"];

    pub fn parse(name: &str) -> Option<Kind> {
        match name {
            "requirement" => Some(Kind::Requirement),
            "change" => Some(Kind::Change),
            "adr" => Some(Kind::Adr),
            "skip" => Some(Kind::Skip),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }
}

/// What the item is in the other tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// A spec file.
    Spec,
    /// Design, tasks and plans that sit next to a spec.
    Notes,
    /// A change: a proposal with design, tasks and spec deltas.
    Change,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub n: usize,
    pub kind: Kind,
    pub role: Role,
    pub title: String,
    /// The files, as paths from the project root where they are inside it.
    pub sources: Vec<String>,
    /// Why this kind was proposed.
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub from: String,
    pub items: Vec<Item>,
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// `add-2fa` as `Add 2fa`.
pub fn humanize(name: &str) -> String {
    let words = name.replace(['-', '_'], " ");
    let mut chars = words.trim().chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            markdown_files(&p, out)?;
        } else if file_name(&p).ends_with(".md") {
            out.push(p);
        }
    }
    Ok(())
}

/// Proposes what to make of what `import::find` finds under `dir`. `shown`
/// turns a path into the text the plan keeps.
pub fn propose(
    dir: &Path,
    source: Source,
    from: &str,
    shown: &dyn Fn(&Path) -> String,
) -> io::Result<Plan> {
    let found = import::find(dir, source)?;
    let mut items = Vec::new();
    let mut add = |kind, role, title: String, files: &[PathBuf], why: &str| {
        let n = items.len() + 1;
        items.push(Item {
            n,
            kind,
            role,
            title,
            sources: files.iter().map(|f| shown(f)).collect(),
            why: why.to_string(),
        });
    };
    for i in &found.inputs {
        add(
            Kind::Requirement,
            Role::Spec,
            i.name.clone(),
            std::slice::from_ref(&i.path),
            "a spec: its requirements and scenarios become a requirement node",
        );
    }
    let mut notes: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
    let mut changes: Vec<(PathBuf, Vec<PathBuf>)> = Vec::new();
    for u in &found.unmapped {
        if u.reason == import::ALREADY_NODE {
            continue;
        }
        let parent = u.path.parent().map(Path::to_path_buf).unwrap_or_default();
        if u.path.is_dir() {
            if file_name(&parent) == "changes" {
                let mut files = Vec::new();
                markdown_files(&u.path, &mut files)?;
                changes.push((u.path.clone(), files));
            }
        } else if file_name(&u.path).ends_with(".md") {
            notes.entry(parent).or_default().push(u.path.clone());
        }
    }
    for (folder, files) in &notes {
        add(
            Kind::Change,
            Role::Notes,
            humanize(&file_name(folder)),
            files,
            "design, tasks and plans have no node type of their own; a change set keeps them",
        );
    }
    for (folder, files) in &changes {
        let name = file_name(folder);
        if files.is_empty() {
            continue;
        }
        if name == "archive" {
            add(
                Kind::Skip,
                Role::Change,
                "Archived changes".into(),
                files,
                "archived changes are history, and git keeps it",
            );
        } else {
            add(
                Kind::Change,
                Role::Change,
                humanize(&name),
                files,
                "an open change: its proposal, design, tasks and spec deltas become a change set",
            );
        }
    }
    Ok(Plan {
        from: from.to_string(),
        items,
    })
}

/// A requirement node that exists or will, with the names of its blocks.
#[derive(Debug, Clone)]
pub struct Known {
    pub id: String,
    pub titles: Vec<String>,
}

/// What an item would write.
#[derive(Debug, Clone)]
pub struct Made {
    pub text: String,
    pub notes: Vec<String>,
    /// The names of the requirement blocks, for a requirement.
    pub titles: Vec<String>,
}

fn read(root: &Path, path: &str) -> io::Result<String> {
    Ok(fs::read_to_string(root.join(path))?.replace("\r\n", "\n"))
}

fn joined(root: &Path, item: &Item) -> io::Result<String> {
    let mut out = Vec::new();
    for s in &item.sources {
        out.push(read(root, s)?);
    }
    Ok(out.join("\n\n"))
}

/// Makes every heading `by` levels deeper, so text can sit inside a section.
pub fn demote(text: &str, by: usize) -> String {
    let mut fenced = false;
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
        }
        let level = line.bytes().take_while(|&b| b == b'#').count();
        if !fenced && (1..=6).contains(&level) && line[level..].starts_with(' ') {
            out.push(format!(
                "{}{}",
                "#".repeat((level + by).min(6)),
                &line[level..]
            ));
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

/// A requirement node from a spec, in the way `import specs` makes one.
#[allow(clippy::too_many_arguments)]
pub fn render_requirement(
    root: &Path,
    item: &Item,
    plan_source: Source,
    id: &str,
    status: &str,
    date: Option<&str>,
    normative: bool,
) -> io::Result<Made> {
    let source = if item.role == Role::Spec {
        plan_source
    } else {
        Source::Markdown
    };
    let input = Input {
        name: item.title.clone(),
        path: PathBuf::from(item.sources.first().cloned().unwrap_or_default()),
        text: joined(root, item)?,
    };
    let draft = import::draft(&input, source, normative);
    Ok(Made {
        text: draft.render(id, status, date, &item.sources.join(", ")),
        titles: draft.blocks.iter().map(|b| b.title.clone()).collect(),
        notes: draft.notes,
    })
}

/// The domain a requirement node of a capability has in its ID.
pub fn domain_prefix(capability: &str) -> String {
    format!("REQ-{}-", import::domain_of(capability))
}

fn file_rank(path: &str) -> (u8, String) {
    let stem = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(".md")
        .to_ascii_lowercase();
    let rank = match stem.as_str() {
        "proposal" => 0,
        "design" => 1,
        "tasks" => 2,
        "spec" => 4,
        _ => 3,
    };
    (rank, path.to_string())
}

/// The requirements a delta names, with the kind of change.
fn delta_entries(text: &str) -> Vec<(&'static str, String)> {
    let (_, secs) = sections(text);
    let mut op = "";
    let mut out = Vec::new();
    for s in &secs {
        if s.level == 2 {
            let t = s.title.to_ascii_uppercase();
            op = ["ADDED", "MODIFIED", "REMOVED", "RENAMED"]
                .into_iter()
                .find(|o| t.starts_with(o))
                .unwrap_or("");
        } else if let (3, false, Some(name)) =
            (s.level, op.is_empty(), s.title.strip_prefix("Requirement:"))
        {
            out.push((op, name.trim().to_string()));
        }
    }
    out
}

/// A change set from a proposal, design, tasks and the spec deltas of a change,
/// or from any files: each file that is not one of these is a section.
pub fn render_change(root: &Path, item: &Item, id: &str, known: &[Known]) -> io::Result<Made> {
    let mut files: Vec<&String> = item.sources.iter().collect();
    files.sort_by_key(|f| file_rank(f));
    let (mut why, mut what, mut impact) = (String::new(), String::new(), String::new());
    let mut extra: Vec<(String, String)> = Vec::new();
    let mut links: Vec<String> = Vec::new();
    let mut notes = Vec::new();
    for path in files {
        let text = read(root, path)?;
        let parts: Vec<&str> = path.split('/').collect();
        let stem = parts
            .last()
            .map_or("", |f| f.trim_end_matches(".md"))
            .to_string();
        let is_delta = stem == "spec"
            && parts
                .iter()
                .position(|p| *p == "changes")
                .zip(parts.iter().rposition(|p| *p == "specs"))
                .is_some_and(|(c, s)| c < s);
        if stem == "proposal" {
            let (pre, secs) = sections(&text);
            why.push_str(&text_of(&pre));
            for s in &secs {
                let title = s.title.to_ascii_lowercase();
                let body = text_of(&s.body);
                let target = match title.as_str() {
                    "why" => Some(&mut why),
                    "what changes" | "what" => Some(&mut what),
                    "impact" => Some(&mut impact),
                    _ if s.level == 1 => Some(&mut why),
                    _ => None,
                };
                match target {
                    Some(t) => {
                        if !t.is_empty() && !body.is_empty() {
                            t.push_str("\n\n");
                        }
                        t.push_str(&body);
                    }
                    None => what.push_str(&format!("\n\n### {}\n\n{body}", s.title)),
                }
            }
        } else if is_delta {
            let capability = parts
                .len()
                .checked_sub(2)
                .and_then(|i| parts.get(i))
                .copied()
                .unwrap_or("");
            let prefix = domain_prefix(capability);
            let mut listing = String::new();
            for (op, name) in delta_entries(&text) {
                listing.push_str(&format!("- {op}: {name}\n"));
                if op == "ADDED" || op == "RENAMED" {
                    continue;
                }
                let hit = known.iter().find(|k| {
                    k.id.starts_with(&prefix)
                        && k.titles.iter().any(|t| t.eq_ignore_ascii_case(&name))
                });
                match hit {
                    Some(k) => {
                        if !links.contains(&k.id) {
                            links.push(k.id.clone());
                        }
                    }
                    None => notes.push(format!(
                        "{op} requirement `{name}` of `{capability}` is not among the requirements, so it is not linked"
                    )),
                }
            }
            extra.push((
                format!("Delta: {capability}"),
                format!("{listing}\n{}", demote(&text, 2)),
            ));
        } else if stem == "design" {
            extra.push(("Design".into(), demote(&text, 2)));
        } else if stem == "tasks" {
            extra.push(("Tasks".into(), demote(&text, 2)));
        } else {
            extra.push((import_title(&stem), demote(&text, 2)));
        }
    }
    let mut out = format!("---\nid: {id}\ntype: change\nstatus: draft\n");
    if !links.is_empty() {
        out.push_str("links:\n  changes:\n");
        for l in &links {
            out.push_str(&format!("    - {l}\n"));
        }
    }
    out.push_str(&format!(
        "---\n# {id}: {}\n<!-- imported from {} -->\n",
        item.title,
        item.sources.join(", ")
    ));
    for (name, text, hint) in [
        ("Why", &why, "<why>"),
        ("What changes", &what, "<what changes>"),
        ("Impact", &impact, "<impact>"),
    ] {
        let text = text.trim();
        if text.is_empty() {
            notes.push(format!("`{name}` has no text in the source; fill it in"));
        }
        out.push_str(&format!(
            "\n## {name}\n\n{}\n",
            if text.is_empty() { hint } else { text }
        ));
    }
    for (name, text) in &extra {
        out.push_str(&format!("\n## {name}\n\n{}\n", text.trim_end()));
    }
    Ok(Made {
        text: out,
        notes,
        titles: Vec::new(),
    })
}

fn import_title(stem: &str) -> String {
    humanize(stem)
}

/// An ADR from the text of the files: it becomes the context, and the rest of
/// the profile's sections are left to write.
pub fn render_adr(
    root: &Path,
    item: &Item,
    id: &str,
    target: &Profile,
    status: &str,
    date: &str,
) -> io::Result<Made> {
    let text = joined(root, item)?;
    let body: Vec<&str> = text.lines().collect();
    // The first `# ` heading is the title of the source, not part of the context.
    let skip = body
        .iter()
        .position(|l| l.starts_with("# "))
        .map_or(0, |_| {
            body.iter().position(|l| l.starts_with("# ")).unwrap_or(0) + 1
        });
    let context = demote(&body[skip..].join("\n"), 2);
    let parts = Parts {
        title: format!("{id}: {}", item.title),
        preamble: String::new(),
        sections: vec![Sec {
            name: "Context".into(),
            text: context.trim().to_string(),
        }],
    };
    let done = adr::render(&parts, target);
    let mut notes = done.notes;
    notes.push("the decision itself is still to be written".into());
    Ok(Made {
        text: format!(
            "---\nid: {id}\ntype: adr\nstatus: {status}\ndate: {date}\n---\n<!-- imported from {} -->\n{}",
            item.sources.join(", "),
            done.text
        ),
        notes,
        titles: Vec::new(),
    })
}

#[cfg(test)]
mod tests_of_plan {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    fn shown(root: &Path) -> impl Fn(&Path) -> String + '_ {
        move |p| {
            p.strip_prefix(root)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/")
        }
    }

    fn openspec(root: &Path) {
        write(
            root,
            "openspec/specs/auth/spec.md",
            "### Requirement: Login\nThe system SHALL sign in.\n#### Scenario: ok\n- **WHEN** a\n- **THEN** b\n",
        );
        write(
            root,
            "openspec/specs/auth/design.md",
            "# Design\n\n## Notes\nUse sessions.\n",
        );
        write(
            root,
            "openspec/changes/add-2fa/proposal.md",
            "## Why\nPasswords leak.\n\n## What Changes\n- add a second step\n\n## Impact\n- auth\n\n## Rollout\nSlowly.\n",
        );
        write(
            root,
            "openspec/changes/add-2fa/design.md",
            "## Context\nTOTP.\n",
        );
        write(root, "openspec/changes/add-2fa/tasks.md", "- [ ] build\n");
        write(
            root,
            "openspec/changes/add-2fa/specs/auth/spec.md",
            "## ADDED Requirements\n### Requirement: Second step\nThe system SHALL ask.\n\n## MODIFIED Requirements\n### Requirement: Login\nThe system SHALL sign in with a code.\n\n## REMOVED Requirements\n### Requirement: Ghost\nx\n",
        );
        write(
            root,
            "openspec/changes/archive/2024-old/proposal.md",
            "## Why\nold\n",
        );
    }

    #[test]
    fn a_plan_proposes_requirements_for_specs_and_change_sets_for_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        openspec(root);
        let plan = propose(
            &root.join("openspec/specs"),
            Source::OpenSpec,
            "openspec",
            &shown(root),
        )
        .unwrap();
        let kinds: Vec<_> = plan
            .items
            .iter()
            .map(|i| (i.kind, i.role, i.title.as_str()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                (Kind::Requirement, Role::Spec, "auth"),
                (Kind::Change, Role::Notes, "Auth"),
                (Kind::Change, Role::Change, "Add 2fa"),
                (Kind::Skip, Role::Change, "Archived changes"),
            ]
        );
        assert_eq!(plan.items[0].sources, vec!["openspec/specs/auth/spec.md"]);
        assert_eq!(plan.items[2].sources.len(), 4);
    }

    #[test]
    fn a_change_set_keeps_the_proposal_design_tasks_and_links_the_modified_requirements() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        openspec(root);
        let plan = propose(
            &root.join("openspec/specs"),
            Source::OpenSpec,
            "openspec",
            &shown(root),
        )
        .unwrap();
        let known = vec![Known {
            id: "REQ-AUTH-0001".into(),
            titles: vec!["Login".into()],
        }];
        let made = render_change(root, &plan.items[2], "CHG-0001", &known).unwrap();
        let t = &made.text;
        assert!(
            t.contains("type: change\nstatus: draft\nlinks:\n  changes:\n    - REQ-AUTH-0001\n"),
            "{t}"
        );
        assert!(t.contains("## Why\n\nPasswords leak."), "{t}");
        assert!(t.contains("## What changes\n\n- add a second step"), "{t}");
        assert!(t.contains("### Rollout"), "{t}");
        assert!(t.contains("## Design\n\n#### Context"), "{t}");
        assert!(t.contains("## Tasks\n\n- [ ] build"), "{t}");
        assert!(t.contains("## Delta: auth"), "{t}");
        assert!(t.contains("- ADDED: Second step"), "{t}");
        assert!(
            !t.contains("\n### Requirement:"),
            "a requirement heading would be read as a block: {t}"
        );
        assert!(
            made.notes.iter().any(|n| n.contains("Ghost")),
            "{:?}",
            made.notes
        );
    }

    #[test]
    fn notes_can_become_an_adr_with_the_text_as_context() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        openspec(root);
        let plan = propose(
            &root.join("openspec/specs"),
            Source::OpenSpec,
            "openspec",
            &shown(root),
        )
        .unwrap();
        let (profiles, _) = crate::formats::available(&crate::config::Config::new(
            Vec::new(),
            crate::config::IdScheme::Sequential,
        ));
        let target = profiles.into_iter().find(|p| p.name == "default").unwrap();
        let made = render_adr(
            root,
            &plan.items[1],
            "ADR-0001",
            &target,
            "proposed",
            "2026-11-05",
        )
        .unwrap();
        assert!(
            made.text.contains("type: adr\nstatus: proposed"),
            "{}",
            made.text
        );
        assert!(made.text.contains("Use sessions."), "{}", made.text);
        assert!(made.text.contains("## Context"), "{}", made.text);
    }

    #[test]
    fn demote_leaves_fences_alone() {
        assert_eq!(
            demote("# A\n```\n# b\n```\n## C", 2),
            "### A\n```\n# b\n```\n#### C"
        );
    }

    #[test]
    fn kinds_have_names() {
        for n in Kind::NAMES {
            assert_eq!(Kind::parse(n).unwrap().name(), *n);
        }
    }
}
