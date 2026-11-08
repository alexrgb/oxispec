//! `oxispec connect`: write the MCP server entry into AI clients' config files.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use oxispec_core::config;
use oxispec_core::connect::{self, Outcome, TARGETS, Target, Transport};

// spec: REQ-CONNECT-0001
// spec: REQ-CONNECT-0002
pub fn connect_command(
    root: &Path,
    asked: &[String],
    transport: Transport,
    dry_run: bool,
) -> Result<()> {
    let cfg = config::load(root)?;
    let targets = resolve(asked, &cfg.agents)?;

    // Read and merge everything first, so a file that cannot be merged stops
    // the command before anything is written.
    let mut plan = Vec::new();
    for target in targets {
        let path = root.join(target.file);
        let existing = match fs::read_to_string(&path) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e).with_context(|| format!("reading {}", target.file)),
        };
        let (text, outcome) =
            connect::merge(existing.as_deref(), target, transport).map_err(anyhow::Error::msg)?;
        plan.push((target, text, outcome));
    }

    for (target, text, outcome) in &plan {
        let verb = match (outcome, dry_run) {
            (Outcome::Unchanged, _) => "unchanged",
            (Outcome::Created, true) => "would create",
            (Outcome::Updated, true) => "would update",
            (Outcome::Created, false) => "created",
            (Outcome::Updated, false) => "updated",
        };
        outln!("{verb} {} ({})", target.file, target.name);
        if dry_run || *outcome == Outcome::Unchanged {
            continue;
        }
        let path = root.join(target.file);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&path, text).with_context(|| format!("writing {}", target.file))?;
    }
    if let Transport::Http { url } = transport {
        eprintln!(
            "the entries read the token from ${} (VS Code asks for it); create one with `oxispec token add NAME` on the server",
            connect::TOKEN_ENV
        );
        if connect::is_cleartext_remote(url) {
            eprintln!(
                "warning: {url} is plain http to another machine, so the token is sent unencrypted; use https"
            );
        }
    }
    Ok(())
}

/// The targets asked for, else the project's clients that have a config file, else all.
fn resolve(asked: &[String], project: &[String]) -> Result<Vec<&'static Target>> {
    let ids: Vec<&str> = if !asked.is_empty() {
        asked.iter().map(String::as_str).collect()
    } else {
        let known: Vec<&str> = project
            .iter()
            .map(String::as_str)
            .filter(|id| connect::target(id).is_some())
            .collect();
        if known.is_empty() {
            TARGETS.iter().map(|t| t.id).collect()
        } else {
            known
        }
    };
    let mut out: Vec<&'static Target> = Vec::new();
    for id in ids {
        let Some(target) = connect::target(id) else {
            bail!(
                "unknown client `{id}` (known: {})",
                TARGETS.iter().map(|t| t.id).collect::<Vec<_>>().join(", ")
            );
        };
        if !out.contains(&target) {
            out.push(target);
        }
    }
    Ok(out)
}
