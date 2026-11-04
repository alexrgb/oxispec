//! `oxispec import specs`: specs written for OpenSpec, Spec Kit, Kiro or plain
//! markdown, brought in as requirement nodes.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use oxispec_core::OUTPUT_SCHEMA_VERSION;
use oxispec_core::config::{self, IdScheme};
use oxispec_core::formats;
use oxispec_core::import::{self, Source};
use oxispec_core::styles::Style;
use serde_json::json;

use crate::{allocate_id, load_graph, next_number, require_module, slug, write_new};

/// What `oxispec import specs` was asked to do.
pub struct Options<'a> {
    pub from: &'a str,
    pub dir: Option<&'a Path>,
    pub format: Option<&'a str>,
    pub scheme: Option<IdScheme>,
    pub dry_run: bool,
    pub json: bool,
}

fn shown(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn import_specs(root: &Path, opts: &Options, cache: bool) -> Result<()> {
    let cfg = config::load(root)?;
    require_module(&cfg, "specs")?;
    let Some(source) = Source::parse(opts.from) else {
        bail!(
            "unknown source `{}`; the sources are {}",
            opts.from,
            import::NAMES.join(", ")
        );
    };
    let dir: PathBuf = match (opts.dir, source.default_dir()) {
        (Some(d), _) => d.to_path_buf(),
        (None, Some(d)) => root.join(d),
        (None, None) => {
            bail!("plain markdown has no default place; pass `--dir` with a file or folder")
        }
    };
    if !dir.exists() {
        bail!("{} does not exist; pass `--dir`", dir.display());
    }
    let (profiles, _) = formats::available(&cfg);
    let asked = opts
        .format
        .or_else(|| cfg.formats.get("requirement").map(String::as_str))
        .unwrap_or("shall");
    let profile = match profiles.iter().find(|p| p.name == asked) {
        Some(p) if p.ty == "requirement" => p,
        Some(p) => bail!(
            "the profile `{asked}` is for {} nodes, not requirements",
            p.ty
        ),
        None => bail!("no profile named `{asked}`; see `oxispec formats list`"),
    };
    let normative = profile.style.is_none_or(Style::needs_normative);
    let status = profile.statuses.first().map_or("draft", String::as_str);
    let dated = profile
        .fields
        .iter()
        .chain(&profile.optional_fields)
        .any(|f| f == "date");
    let today = oxispec_core::debt::today();

    let found =
        import::find(&dir, source).with_context(|| format!("cannot read {}", dir.display()))?;
    let drafts: Vec<_> = found
        .inputs
        .iter()
        .map(|i| import::draft(i, source, normative))
        .collect();

    let graph = load_graph(root, cache);
    let scheme = opts.scheme.unwrap_or(cfg.id_scheme);
    let mut counters: HashMap<String, u32> = HashMap::new();
    let mut taken: HashSet<String> = HashSet::new();
    let mut planned = Vec::new();
    for d in &drafts {
        let domain = d.domain();
        let prefix = format!("REQ-{domain}-");
        let id = match scheme {
            IdScheme::Sequential => {
                let n = counters
                    .entry(prefix.clone())
                    .or_insert_with(|| next_number(&graph, &prefix));
                let id = format!("{prefix}{n:04}");
                *n += 1;
                id
            }
            IdScheme::Random => loop {
                let id = allocate_id(&graph, &prefix, scheme)?;
                if taken.insert(id.clone()) {
                    break id;
                }
            },
        };
        let name = slug(&d.name).unwrap_or_else(|_| domain.to_ascii_lowercase());
        let rel = PathBuf::from("specs/requirements")
            .join(domain.to_ascii_lowercase())
            .join(format!("{name}.md"));
        if root.join(&rel).exists() {
            bail!(
                "{} already exists; move it or import from a smaller folder",
                rel.display()
            );
        }
        let text = d.render(
            &id,
            status,
            dated.then_some(today.as_str()),
            &shown(root, &d.path),
        );
        planned.push((id, rel, text));
    }
    if !opts.dry_run {
        for (_, rel, text) in &planned {
            write_new(root, rel, text)?;
        }
    }

    let skipped: Vec<_> = found
        .unmapped
        .iter()
        .map(|u| (shown(root, &u.path), u.reason.as_str()))
        .collect();
    if opts.json {
        let items: Vec<_> = drafts
            .iter()
            .zip(&planned)
            .map(|(d, (id, rel, _))| {
                json!({
                    "id": id,
                    "path": shown(root, rel),
                    "source": shown(root, &d.path),
                    "requirements": d.blocks.len(),
                    "notes": d.notes,
                })
            })
            .collect();
        let skipped: Vec<_> = skipped
            .iter()
            .map(|(p, r)| json!({"path": p, "reason": r}))
            .collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "from": opts.from,
                "format": profile.name,
                "dry_run": opts.dry_run,
                "nodes": items,
                "not_imported": skipped,
            }))?
        );
        return Ok(());
    }
    for (d, (id, rel, _)) in drafts.iter().zip(&planned) {
        outln!(
            "{} {} -> {} ({}, {} requirement(s))",
            if opts.dry_run {
                "would import"
            } else {
                "imported"
            },
            shown(root, &d.path),
            shown(root, rel),
            id,
            d.blocks.len()
        );
        for note in &d.notes {
            outln!("    {note}");
        }
    }
    for (path, reason) in &skipped {
        outln!("not imported {path}: {reason}");
    }
    outln!(
        "{} node(s) in the `{}` format; run `oxispec check` to see what is left to fill in",
        planned.len(),
        profile.name
    );
    Ok(())
}
