//! `oxispec move-docs DIR`: moves the documents of a project to another folder and says so in
//! the config. IDs and links do not name paths, so nothing inside a node changes.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use oxispec_core::OUTPUT_SCHEMA_VERSION;
use oxispec_core::config;
use serde_json::json;

const SKIPPED_FOLDERS: &[&str] = &["target", "node_modules", "vendor"];
const TEXT_FILES: &[&str] = &["md", "mdx", "yml", "yaml", "toml", "json", "txt"];
const MOST_FILES: usize = 5000;
const BIGGEST_FILE: u64 = 1_000_000;

/// A file outside the config that names the old folder, so that a person can look at it.
struct Mention {
    path: PathBuf,
    count: usize,
}

/// A path as it is shown: with `/` on every system.
fn shown(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn is_empty_dir(path: &Path) -> bool {
    path.is_dir() && fs::read_dir(path).is_ok_and(|mut d| d.next().is_none())
}

/// How many times `text` names `folder/` as the start of a path.
fn mentions_in(text: &str, folder: &str) -> usize {
    let needle = format!("{folder}/");
    let mut count = 0;
    let mut from = 0;
    while let Some(at) = text[from..].find(&needle) {
        let start = from + at;
        let before = text[..start].chars().next_back();
        if before.is_none_or(|c| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '-')))
        {
            count += 1;
        }
        from = start + needle.len();
    }
    count
}

fn scan(dir: &Path, root: &Path, folder: &str, seen: &mut usize, out: &mut Vec<Mention>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            if !SKIPPED_FOLDERS.contains(&name.as_str()) {
                scan(&path, root, folder, seen, out);
            }
            continue;
        }
        let text_file = path
            .extension()
            .is_some_and(|x| TEXT_FILES.iter().any(|t| x == *t));
        if !text_file || *seen >= MOST_FILES {
            continue;
        }
        *seen += 1;
        if e.metadata().is_ok_and(|m| m.len() > BIGGEST_FILE) {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            let count = mentions_in(&text, folder);
            if count > 0 {
                out.push(Mention {
                    path: path.strip_prefix(root).unwrap_or(&path).to_path_buf(),
                    count,
                });
            }
        }
    }
}

fn undo(done: &[(PathBuf, PathBuf)]) {
    for (from, to) in done.iter().rev() {
        let _ = fs::rename(to, from);
    }
}

/// One folder of the documents: where it is, where it would go, and what is in the way.
struct Row {
    kind: &'static str,
    src: PathBuf,
    dst: PathBuf,
    /// Why it cannot move: something is already at the new place.
    clash: Option<String>,
}

/// What is at the new folder now.
struct Target {
    exists: bool,
    is_file: bool,
    /// The names in it, with whether each is a folder.
    entries: Vec<(String, bool)>,
}

fn look_at(path: &Path) -> Target {
    let mut entries: Vec<(String, bool)> = fs::read_dir(path)
        .map(|d| {
            d.flatten()
                .map(|e| {
                    (
                        e.file_name().to_string_lossy().into_owned(),
                        e.path().is_dir(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    entries.sort();
    Target {
        exists: path.exists(),
        is_file: path.is_file(),
        entries,
    }
}

fn clash_of(dst: &Path) -> Option<String> {
    if dst.is_file() {
        Some("a file is there".to_string())
    } else if dst.is_dir() && !is_empty_dir(dst) {
        let n = fs::read_dir(dst).map_or(0, |d| d.count());
        Some(format!("a folder with {n} item(s) is there"))
    } else {
        None
    }
}

fn name_of(entry: &(String, bool)) -> String {
    if entry.1 {
        format!("{}/", entry.0)
    } else {
        entry.0.clone()
    }
}

// spec: REQ-PROJECT-0005
pub fn move_docs(root: &Path, to: &str, dry_run: bool, json: bool) -> Result<()> {
    let mut cfg = config::load(root)?;
    config::check_docs_dir(to).map_err(anyhow::Error::msg)?;
    let to = to.trim_end_matches('/').to_string();
    let from = cfg.docs_dir.trim_end_matches('/').to_string();
    if to == from {
        bail!("the documents are in `{from}` already");
    }
    if Path::new(&to).starts_with(&from) || Path::new(&from).starts_with(&to) {
        bail!(
            "`{to}` and `{from}` are one inside the other; pick a folder that is not part of the other"
        );
    }
    let mut target = cfg.clone();
    target.docs_dir = to.clone();

    // What moves: the folders of the kinds of nodes, from where they are to where they will be.
    let mut rows: Vec<Row> = Vec::new();
    for kind in config::KIND_DIRS {
        let src = root.join(cfg.dir(kind));
        if !src.is_dir() {
            continue;
        }
        let dst = root.join(target.dir(kind));
        let clash = clash_of(&dst);
        rows.push(Row {
            kind,
            src,
            dst,
            clash,
        });
    }
    let at_target = look_at(&root.join(&to));
    if at_target.is_file {
        bail!("`{to}` is a file; pick a folder");
    }
    let clashes: Vec<&Row> = rows.iter().filter(|r| r.clash.is_some()).collect();
    let rel = |p: &Path| shown(p.strip_prefix(root).unwrap_or(p));

    let mut mentions = Vec::new();
    scan(root, root, &from, &mut 0, &mut mentions);

    let mut error = None;
    if clashes.is_empty() && !dry_run {
        let pairs: Vec<(PathBuf, PathBuf)> = rows
            .iter()
            .map(|r| (r.src.clone(), r.dst.clone()))
            .collect();
        let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
        for (src, dst) in &pairs {
            let step = (|| -> std::io::Result<()> {
                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent)?;
                }
                if is_empty_dir(dst) {
                    fs::remove_dir(dst)?;
                }
                fs::rename(src, dst)
            })();
            match step {
                Ok(()) => done.push((src.clone(), dst.clone())),
                Err(e) => {
                    undo(&done);
                    bail!(
                        "cannot move {} to {}: {e}; the moves so far were undone",
                        rel(src),
                        rel(dst)
                    );
                }
            }
        }
        cfg.docs_dir = to.clone();
        if let Err(e) = config::save(root, &cfg) {
            undo(&done);
            bail!("cannot save the config: {e}; the moves were undone");
        }
        // The folders the documents left are removed when nothing else is in them.
        for (src, _) in &pairs {
            let mut dir = src.parent();
            while let Some(d) = dir {
                if d == root || !d.starts_with(root) || fs::remove_dir(d).is_err() {
                    break;
                }
                dir = d.parent();
            }
        }
        crate::ensure_module_dirs(root, &cfg)?;
    } else if !clashes.is_empty() {
        error = Some(format!(
            "nothing was moved: {} of {} folder(s) clash with what is in {to}/",
            clashes.len(),
            rows.len()
        ));
    }
    let status = |r: &Row| match (&r.clash, dry_run) {
        (Some(_), _) => "clash",
        (None, true) => "can move",
        (None, false) => "moved",
    };

    if json {
        let folders: Vec<_> = rows
            .iter()
            .map(|r| {
                json!({
                    "kind": r.kind,
                    "from": rel(&r.src),
                    "to": rel(&r.dst),
                    "status": status(r),
                    "reason": r.clash,
                })
            })
            .collect();
        let named: Vec<_> = mentions
            .iter()
            .map(|m| json!({"path": shown(&m.path), "count": m.count}))
            .collect();
        let entries: Vec<_> = at_target
            .entries
            .iter()
            .map(|(name, dir)| json!({"name": name, "folder": dir}))
            .collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "from": from,
                "to": to,
                "dry_run": dry_run,
                "can_move": clashes.is_empty(),
                "target": {"exists": at_target.exists, "entries": entries},
                "folders": folders,
                "mentions": named,
            }))?
        );
        if let Some(e) = error {
            bail!(e);
        }
        return Ok(());
    }

    outln!("documents: {from}/ -> {to}/");
    outln!(
        "{to}/ {}",
        match (at_target.exists, at_target.entries.is_empty()) {
            (false, _) => "does not exist; it will be created".to_string(),
            (true, true) => "exists and is empty".to_string(),
            (true, false) => format!(
                "exists and has {} item(s): {}",
                at_target.entries.len(),
                at_target
                    .entries
                    .iter()
                    .map(name_of)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    );
    if rows.is_empty() {
        outln!("no document folders to move");
    }
    let width = rows.iter().map(|r| rel(&r.src).len()).max().unwrap_or(0);
    let width_to = rows.iter().map(|r| rel(&r.dst).len()).max().unwrap_or(0);
    for r in &rows {
        outln!(
            "  {:<width$}  ->  {:<width_to$}   {}",
            rel(&r.src),
            rel(&r.dst),
            match &r.clash {
                Some(why) => format!("CLASH, {why}"),
                None => status(r).to_string(),
            }
        );
    }
    if clashes.is_empty() {
        if dry_run {
            outln!(
                "all {} folder(s) can move; nothing was changed. Run without --dry-run to move them",
                rows.len()
            );
        } else {
            outln!("the documents are in {to}/ now, and the config says so");
            if root.join(".git").exists() {
                outln!("the files moved on disk; `git add -A` stages them as renames");
            }
        }
    } else {
        outln!(
            "{} of {} folder(s) clash with what is in {to}/. The move is all or nothing, so that `docs_dir` can name one folder, and nothing was changed.",
            clashes.len(),
            rows.len()
        );
        outln!(
            "Rename or move these in {to}/ (the names of OxiSpec's folders stay as they are), then run the command again:"
        );
        for r in &clashes {
            outln!("    {}", rel(&r.dst));
        }
    }
    if !mentions.is_empty() {
        outln!("these files name `{from}/` and are not changed; look at them:");
        for m in mentions.iter().take(20) {
            outln!("    {} ({})", shown(&m.path), m.count);
        }
        if mentions.len() > 20 {
            outln!("    and {} more", mentions.len() - 20);
        }
    }
    if let Some(e) = error {
        bail!(e);
    }
    Ok(())
}
