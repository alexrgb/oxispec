//! `oxispec adr convert` and `oxispec import adr`: decisions in other formats.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use oxispec_core::OUTPUT_SCHEMA_VERSION;
use oxispec_core::adr::{self, Dialect};
use oxispec_core::config::{self, Config};
use oxispec_core::formats::{self, Profile};
use serde_json::json;

use crate::{load_graph, require_module, write_new};

/// The profile of the ADR format with this name, or the one the config chooses.
fn adr_profile(cfg: &Config, name: Option<&str>) -> Result<Profile> {
    let (profiles, _) = formats::available(cfg);
    let name = name
        .or_else(|| cfg.formats.get("adr").map(String::as_str))
        .unwrap_or("default");
    match profiles.into_iter().find(|p| p.name == name) {
        Some(p) if p.ty == "adr" => Ok(p),
        Some(p) => bail!("the profile `{name}` is for {} nodes, not ADRs", p.ty),
        None => bail!("no profile named `{name}`; see `oxispec formats list`"),
    }
}

pub fn convert(
    root: &Path,
    ids: &[String],
    to: &str,
    dry_run: bool,
    json: bool,
    cache: bool,
) -> Result<()> {
    let cfg = config::load(root)?;
    require_module(&cfg, "adrs")?;
    let target = adr_profile(&cfg, Some(to))?;
    let graph = load_graph(root, cache);
    let mut chosen = Vec::new();
    if ids.is_empty() {
        chosen.extend(graph.nodes.iter().filter(|n| n.ty == "adr"));
    } else {
        for id in ids {
            match graph.get(id) {
                Some(n) if n.ty == "adr" => chosen.push(n),
                Some(n) => bail!("{id} is a {}, not an ADR", n.ty),
                None => bail!("no node {id}"),
            }
        }
    }
    // Convert everything first, so a record that cannot be read stops the run
    // before any file is touched.
    let mut done = Vec::new();
    for node in chosen {
        let path = root.join(&node.path);
        let text = fs::read_to_string(&path)?;
        let out = adr::convert(&text, &target)
            .map_err(|e| anyhow::anyhow!("{}: {e}", node.path.display()))?;
        done.push((node, text.replace("\r\n", "\n") != out.text, out));
    }
    if !dry_run {
        for (node, changed, out) in &done {
            if *changed {
                fs::write(root.join(&node.path), &out.text)?;
            }
        }
    }
    let in_use = cfg.formats.get("adr").map(String::as_str);
    let hint = (in_use != Some(target.name.as_str())).then(|| {
        format!(
            "the project checks ADRs as `{}`; set `formats: {{adr: {}}}` in .oxispec/config.yaml",
            in_use.unwrap_or("nothing"),
            target.name
        )
    });
    if json {
        let items: Vec<_> = done
            .iter()
            .map(|(n, changed, out)| {
                json!({
                    "id": n.id,
                    "path": n.path,
                    "changed": changed,
                    "notes": out.notes,
                })
            })
            .collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "format": target.name,
                "dry_run": dry_run,
                "records": items,
                "hint": hint,
            }))?
        );
        return Ok(());
    }
    for (node, changed, out) in &done {
        let verb = match (changed, dry_run) {
            (false, _) => "unchanged",
            (true, true) => "would convert",
            (true, false) => "converted",
        };
        outln!("{verb:<13} {} ({})", node.id, node.path.display());
        for note in &out.notes {
            outln!("              {note}");
        }
    }
    if let Some(hint) = hint {
        outln!("{hint}");
    }
    Ok(())
}

/// What `oxispec import adr` was asked to do.
pub struct ImportOptions<'a> {
    pub from: &'a str,
    pub dir: Option<&'a Path>,
    pub format: Option<&'a str>,
    pub offset: u32,
    pub dry_run: bool,
    pub json: bool,
}

pub fn import(root: &Path, opts: &ImportOptions, cache: bool) -> Result<()> {
    let ImportOptions {
        from,
        dir,
        format,
        offset,
        dry_run,
        json,
    } = *opts;
    let cfg = config::load(root)?;
    require_module(&cfg, "adrs")?;
    let Some(dialect) = Dialect::parse(from) else {
        bail!(
            "unknown source `{from}`; the sources are {}",
            Dialect::NAMES.join(", ")
        );
    };
    let target = adr_profile(&cfg, format)?;
    let dir: PathBuf = match dir {
        Some(d) => d.to_path_buf(),
        None => root.join(dialect.default_dir()),
    };
    if !dir.is_dir() {
        bail!(
            "{} is not a folder; pass the folder of records",
            dir.display()
        );
    }
    let files = adr::read_dir(&dir)?;
    let records = adr::import(&files, dialect, offset, &target)?;
    let graph = load_graph(root, cache);
    let clashes: Vec<String> = records
        .iter()
        .filter(|r| graph.get(&r.id).is_some() || root.join(&r.path).exists())
        .map(|r| r.id.clone())
        .collect();
    if !clashes.is_empty() {
        bail!(
            "the project already has {}; pass `--offset N` to number the records after them",
            clashes.join(", ")
        );
    }
    if !dry_run {
        for r in &records {
            write_new(root, Path::new(&r.path), &r.text)?;
        }
    }
    if json {
        let items: Vec<_> = records
            .iter()
            .map(|r| json!({"id": r.id, "path": r.path, "source": r.source, "notes": r.notes}))
            .collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "from": from,
                "format": target.name,
                "dry_run": dry_run,
                "records": items,
            }))?
        );
        return Ok(());
    }
    for r in &records {
        outln!(
            "{} {} -> {} ({})",
            if dry_run { "would import" } else { "imported" },
            r.source,
            r.path,
            r.id
        );
        for note in &r.notes {
            outln!("    {note}");
        }
    }
    outln!(
        "{} record(s) in the `{}` format; run `oxispec check` to see what is left to fill in",
        records.len(),
        target.name
    );
    Ok(())
}
