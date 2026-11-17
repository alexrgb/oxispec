//! `oxispec onboard`: fix the kinds of documents a project keeps and the format of each, so a
//! later `oxispec new` needs no question about which document or which format.

use std::io::BufRead;
use std::path::Path;

use anyhow::{Result, bail};
use oxispec_core::config::{self, Config};
use oxispec_core::documents::{self, KINDS};
use oxispec_core::formats::{self, Profile};
use oxispec_core::modules;

/// What the person asked for: the kinds of documents, each with the format named for it, if any.
pub type Choice = Vec<(String, Option<String>)>;

/// The kinds of documents and formats given as `type[:format]`, as `--docs` writes them.
pub fn parse(items: &[String], profiles: &[Profile]) -> Result<Choice> {
    let mut out: Choice = Vec::new();
    for item in items {
        let (ty, format) = match item.split_once(':') {
            Some((t, f)) => (t.trim(), Some(f.trim().to_string())),
            None => (item.trim(), None),
        };
        let Some(kind) = documents::kind(ty) else {
            let known: Vec<&str> = KINDS.iter().map(|k| k.ty).collect();
            bail!(
                "`{ty}` is not a kind of document (known: {})",
                known.join(", ")
            );
        };
        if let Some(name) = &format {
            match profiles.iter().find(|p| &p.name == name) {
                Some(p) if p.ty == kind.ty => {}
                Some(p) => bail!("the profile `{name}` is for {} nodes, not {ty}", p.ty),
                None => bail!("no profile named `{name}`; see `oxispec formats list`"),
            }
        }
        if out.iter().any(|(t, _)| t == ty) {
            bail!("`{ty}` is listed twice");
        }
        out.push((ty.to_string(), format));
    }
    Ok(out)
}

/// The kinds the project keeps now: the `documents` list, or the default set for a project that
/// has none.
fn current(cfg: &Config) -> Choice {
    let types: Vec<String> = if cfg.documents.is_empty() {
        documents::DEFAULT.iter().map(|s| s.to_string()).collect()
    } else {
        cfg.documents.clone()
    };
    types
        .into_iter()
        .map(|t| {
            let f = cfg.formats.get(&t).cloned();
            (t, f)
        })
        .collect()
}

/// Asks about every kind of document, reading the answers from `input`. An empty answer, or the
/// end of the input, keeps what the project has now.
fn ask(cfg: &Config, profiles: &[Profile], input: &mut impl BufRead) -> Result<Choice> {
    let now = current(cfg);
    let mut line = String::new();
    let mut read = |line: &mut String| -> Result<String> {
        line.clear();
        input.read_line(line)?;
        Ok(line.trim().to_string())
    };
    let mut out: Choice = Vec::new();
    outln!("Which documents does the project keep? Enter keeps the answer in brackets.");
    for kind in KINDS {
        let kept = now.iter().find(|(t, _)| t == kind.ty);
        outln!(
            "{} ({}): keep? [{}]",
            kind.label,
            kind.ty,
            if kept.is_some() { "Y/n" } else { "y/N" }
        );
        let answer = read(&mut line)?.to_ascii_lowercase();
        let keep = match answer.as_str() {
            "" => kept.is_some(),
            "y" | "yes" => true,
            _ => false,
        };
        if !keep {
            continue;
        }
        let options: Vec<&Profile> = profiles.iter().filter(|p| p.ty == kind.ty).collect();
        let mut format = kept.and_then(|(_, f)| f.clone());
        if !options.is_empty() {
            outln!(
                "  format for {}: {} [{}]",
                kind.ty,
                options
                    .iter()
                    .map(|p| p.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                format.as_deref().unwrap_or("none")
            );
            let name = read(&mut line)?;
            if !name.is_empty() {
                match options.iter().find(|p| p.name == name) {
                    Some(p) => format = Some(p.name.clone()),
                    None => bail!("no profile named `{name}` for {}", kind.ty),
                }
            }
        }
        out.push((kind.ty.to_string(), format));
    }
    Ok(out)
}

/// What `onboard` was asked to do.
pub struct Options<'a> {
    pub docs: &'a [String],
    pub yes: bool,
    pub show: bool,
}

// spec: REQ-PROJECT-0006
pub fn onboard(
    dir: &Path,
    opts: &Options,
    init: impl FnOnce(&Path, &[String]) -> Result<()>,
    input: &mut impl BufRead,
) -> Result<()> {
    if opts.show {
        let root = config::find_root(dir).ok_or_else(|| {
            anyhow::anyhow!(
                "not an OxiSpec project here or in any parent folder; run `oxispec onboard`"
            )
        })?;
        summary(&config::load(&root)?);
        return Ok(());
    }
    let existing = config::find_root(dir);
    let (profiles, cfg) = match &existing {
        Some(root) => {
            let cfg = config::load(root)?;
            (formats::available(&cfg).0, cfg)
        }
        None => {
            let cfg = Config::new(vec!["core".into()], Default::default());
            (formats::builtin(), cfg)
        }
    };
    let choice = if !opts.docs.is_empty() {
        parse(opts.docs, &profiles)?
    } else if opts.yes {
        current(&cfg)
    } else {
        ask(&cfg, &profiles, input)?
    };
    if choice.is_empty() {
        bail!("no document kind was chosen; name at least one, such as `--docs requirement,adr`");
    }
    let needed: Vec<String> = choice
        .iter()
        .filter_map(|(t, _)| documents::kind(t))
        .map(|k| k.module.to_string())
        .collect();
    let root = match existing {
        Some(root) => root,
        None => {
            init(dir, &needed)?;
            dir.to_path_buf()
        }
    };
    let mut cfg = config::load(&root)?;
    let mut wanted = cfg.modules.clone();
    wanted.extend(needed);
    cfg.modules = modules::resolve(&wanted).map_err(anyhow::Error::msg)?;
    let kept: Vec<&str> = choice.iter().map(|(t, _)| t.as_str()).collect();
    cfg.formats.retain(|t, _| kept.contains(&t.as_str()));
    for (ty, format) in &choice {
        if let Some(f) = format {
            cfg.formats.insert(ty.clone(), f.clone());
        }
    }
    cfg.documents = choice.iter().map(|(t, _)| t.clone()).collect();
    config::save(&root, &cfg)?;
    crate::ensure_module_dirs(&root, &cfg)?;
    summary(&cfg);
    Ok(())
}

/// The documents of the project, the format of each and the command that writes it.
fn summary(cfg: &Config) {
    if cfg.documents.is_empty() {
        outln!("no documents are fixed; run `oxispec onboard` to choose them");
        return;
    }
    outln!("documents the project keeps:");
    for ty in &cfg.documents {
        let Some(kind) = documents::kind(ty) else {
            continue;
        };
        outln!(
            "  {:<24} {:<14} oxispec {} \"Title\"",
            ty,
            cfg.formats.get(ty).map_or("(default)", String::as_str),
            kind.command
        );
    }
}

/// Fails when the project fixed its documents and `ty` is not one of them.
pub fn require_document(cfg: &Config, ty: &str) -> Result<()> {
    if !cfg.documents.is_empty() && !cfg.documents.iter().any(|d| d == ty) {
        bail!(
            "`{ty}` is not among the documents of this project ({}); add it with `oxispec onboard --docs {},{ty}`",
            cfg.documents.join(", "),
            cfg.documents.join(",")
        );
    }
    Ok(())
}
