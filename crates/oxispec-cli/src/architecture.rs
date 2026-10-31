//! `oxispec architecture`: the C4 and deployment model, and views generated from it.

use std::path::Path;
use std::process::ExitCode;

use anyhow::{Result, bail};
use clap::ValueEnum;
use oxispec_core::architecture::{self, Model, ViewKind};
use oxispec_core::{Graph, OUTPUT_SCHEMA_VERSION, config, index};

use crate::git;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ViewArg {
    /// A system, with the people and systems around it
    Context,
    /// A system, with the containers inside it
    Container,
    /// A container, with the components inside it
    Component,
    /// An environment, with its nodes and what runs on them
    Deployment,
}

impl From<ViewArg> for ViewKind {
    fn from(arg: ViewArg) -> ViewKind {
        match arg {
            ViewArg::Context => ViewKind::Context,
            ViewArg::Container => ViewKind::Container,
            ViewArg::Component => ViewKind::Component,
            ViewArg::Deployment => ViewKind::Deployment,
        }
    }
}

fn load(root: &Path, at: Option<&str>, cache: bool) -> Result<(Model, Option<git::Revision>)> {
    let cfg = config::load(root)?;
    if !cfg.has("architecture") {
        bail!("module `architecture` is not enabled; run `oxispec modules add architecture`");
    }
    let revision = at.map(|spec| git::resolve_rev(root, spec)).transpose()?;
    let graph: Graph = match &revision {
        Some(rev) => git::graph_at(root, rev)?,
        None => index::load(root, cache).0,
    };
    Ok((architecture::analyze(&graph).model, revision))
}

pub fn list(root: &Path, at: Option<&str>, json: bool, cache: bool) -> Result<ExitCode> {
    let (model, revision) = load(root, at, cache)?;
    if json {
        let elements: Vec<_> = model
            .elements
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "type": e.ty,
                    "name": e.name,
                    "technology": e.technology,
                    "owner": e.owner,
                    "parent": e.parent,
                    "active": e.active,
                    "uses": e.uses.iter().map(|u| serde_json::json!({
                        "to": u.to,
                        "description": u.description,
                        "technology": u.technology,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let environments: Vec<_> = model
            .environments
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "name": e.name,
                    "active": e.active,
                    "nodes": e.nodes.iter().map(|n| serde_json::json!({
                        "name": n.name,
                        "kind": n.kind,
                        "region": n.region,
                        "hosts": n.hosts,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let rules: Vec<_> = model
            .rules
            .iter()
            .map(|r| serde_json::json!({"rule": r.text, "node": r.node}))
            .collect();
        let out = serde_json::json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "at": revision.as_ref().map(crate::revision_json),
            "elements": elements,
            "environments": environments,
            "rules": rules,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(ExitCode::SUCCESS);
    }
    if let Some(rev) = &revision {
        println!("reading {rev}");
    }
    if model.elements.is_empty() && model.environments.is_empty() {
        println!(
            "no architecture yet: `oxispec new architecture system \"Name\"` writes the first element"
        );
        return Ok(ExitCode::SUCCESS);
    }
    for e in &model.elements {
        let tech = e
            .technology
            .as_ref()
            .map_or(String::new(), |t| format!(" [{t}]"));
        let parent = e
            .parent
            .as_ref()
            .map_or(String::new(), |p| format!(" in {p}"));
        println!("{:<10} {:<12} {}{tech}{parent}", e.ty, e.id, e.name);
    }
    for env in &model.environments {
        println!(
            "{:<10} {:<12} {} ({} nodes)",
            "environment",
            env.id,
            env.name,
            env.nodes.len()
        );
    }
    println!(
        "{} rules; `oxispec architecture view` draws a view, `oxispec check` reports problems",
        model.rules.len()
    );
    Ok(ExitCode::SUCCESS)
}

pub fn view(
    root: &Path,
    kind: ViewArg,
    id: &str,
    at: Option<&str>,
    json: bool,
    cache: bool,
) -> Result<ExitCode> {
    let (model, _) = load(root, at, cache)?;
    let built = architecture::view(&model, kind.into(), id).map_err(anyhow::Error::msg)?;
    if json {
        let out = serde_json::json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "view": built.kind.name(),
            "root": built.root,
            "title": built.title,
            "groups": built.groups.iter().map(|g| serde_json::json!({
                "id": g.id, "label": g.label, "parent": g.parent,
            })).collect::<Vec<_>>(),
            "nodes": built.nodes.iter().map(|n| serde_json::json!({
                "id": n.id, "label": n.label, "class": n.class, "group": n.group,
            })).collect::<Vec<_>>(),
            "edges": built.edges.iter().map(|e| serde_json::json!({
                "from": e.from, "to": e.to, "label": e.label,
            })).collect::<Vec<_>>(),
            "mermaid": architecture::mermaid(&built),
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        print!("{}", architecture::mermaid(&built));
    }
    Ok(ExitCode::SUCCESS)
}
