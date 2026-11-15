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
    let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut conflicts: Vec<PathBuf> = Vec::new();
    for kind in config::KIND_DIRS {
        let src = root.join(cfg.dir(kind));
        let dst = root.join(target.dir(kind));
        if !src.is_dir() {
            continue;
        }
        if dst.exists() && !is_empty_dir(&dst) {
            conflicts.push(target.dir(kind));
        }
        pairs.push((src, dst));
    }
    if !conflicts.is_empty() {
        let list: Vec<_> = conflicts.iter().map(|c| c.display().to_string()).collect();
        bail!(
            "{} exists already, so nothing was moved; move it away or pick another folder",
            list.join(", ")
        );
    }

    let mut mentions = Vec::new();
    scan(root, root, &from, &mut 0, &mut mentions);

    if !dry_run {
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
                        src.display(),
                        dst.display()
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
    }

    if json {
        let moved: Vec<_> = pairs
            .iter()
            .map(|(s, d)| {
                json!({
                    "from": s.strip_prefix(root).unwrap_or(s).to_string_lossy().replace('\\', "/"),
                    "to": d.strip_prefix(root).unwrap_or(d).to_string_lossy().replace('\\', "/"),
                })
            })
            .collect();
        let named: Vec<_> = mentions
            .iter()
            .map(|m| json!({"path": m.path.to_string_lossy().replace('\\', "/"), "count": m.count}))
            .collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "from": from,
                "to": to,
                "dry_run": dry_run,
                "moved": moved,
                "mentions": named,
            }))?
        );
        return Ok(());
    }
    for (s, d) in &pairs {
        outln!(
            "{} {} -> {}",
            if dry_run { "would move" } else { "moved" },
            s.strip_prefix(root).unwrap_or(s).display(),
            d.strip_prefix(root).unwrap_or(d).display()
        );
    }
    if pairs.is_empty() {
        outln!("no document folders to move");
    }
    if dry_run {
        outln!("nothing was changed; run without --dry-run to move the documents to {to}/");
    } else {
        outln!("the documents are in {to}/ now, and the config says so");
        if root.join(".git").exists() {
            outln!("the files moved on disk; `git add -A` stages them as renames");
        }
    }
    if !mentions.is_empty() {
        outln!("these files name `{from}/` and are not changed; look at them:");
        for m in mentions.iter().take(20) {
            outln!("    {} ({})", m.path.display(), m.count);
        }
        if mentions.len() > 20 {
            outln!("    and {} more", mentions.len() - 20);
        }
    }
    Ok(())
}
