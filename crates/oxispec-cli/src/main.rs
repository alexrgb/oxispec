use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use oxispec_core::config::{self, Config};
use oxispec_core::{Graph, Severity, check, modules};

#[derive(Parser)]
#[command(
    name = "oxispec",
    version,
    about = "Linked specs, ADRs and checks for humans and agents"
)]
struct Cli {
    /// Project folder (default: the nearest parent with .oxispec/config.yaml)
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create .oxispec/config.yaml and the folders for the enabled modules
    Init {
        /// minimal, specs-tests, docs, standard or full
        #[arg(long, default_value = "minimal")]
        preset: String,
        /// Extra modules to enable, comma separated
        #[arg(long, value_delimiter = ',')]
        with: Vec<String>,
        /// Modules to leave out of the preset, comma separated
        #[arg(long, value_delimiter = ',')]
        without: Vec<String>,
    },
    /// List, add or remove modules
    Modules {
        #[command(subcommand)]
        action: Option<ModulesAction>,
    },
    /// Create a node from a template
    New {
        #[command(subcommand)]
        kind: NewKind,
    },
    /// Run structural checks. Exit code 1 when there are errors.
    Check {
        #[arg(long)]
        json: bool,
        /// Treat warnings as errors
        #[arg(long)]
        strict: bool,
        /// Also print informational findings
        #[arg(long, short)]
        verbose: bool,
    },
    /// Show a node, its links, and the nodes that link to it
    Show {
        id: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ModulesAction {
    /// List all modules (default)
    List,
    /// Enable modules, and the modules they need
    Add { names: Vec<String> },
    /// Disable modules. Existing files are never deleted.
    Remove { names: Vec<String> },
}

#[derive(Subcommand)]
enum NewKind {
    /// A requirement with one scenario, for example: new requirement auth "Admin session timeout"
    Requirement { domain: String, title: String },
    /// An architecture decision record
    Adr { title: String },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        Command::Init {
            preset,
            with,
            without,
        } => {
            let dir = cli.root.unwrap_or_else(|| PathBuf::from("."));
            init(&dir, &preset, &with, &without)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Modules { action } => {
            modules_command(cli.root.as_deref(), action.unwrap_or(ModulesAction::List))?;
            Ok(ExitCode::SUCCESS)
        }
        Command::New { kind } => {
            let root = project_root(cli.root.as_deref())?;
            let cfg = config::load(&root)?;
            let created = match kind {
                NewKind::Requirement { domain, title } => {
                    require_module(&cfg, "specs")?;
                    new_requirement(&root, &domain, &title)?
                }
                NewKind::Adr { title } => {
                    require_module(&cfg, "adrs")?;
                    new_adr(&root, &title)?
                }
            };
            println!("created {} ({})", created.0, created.1.display());
            Ok(ExitCode::SUCCESS)
        }
        Command::Check {
            json,
            strict,
            verbose,
        } => check_command(&project_root(cli.root.as_deref())?, json, strict, verbose),
        Command::Show { id, json } => {
            show(&project_root(cli.root.as_deref())?, &id, json)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn project_root(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(dir) = explicit {
        if dir.join(config::CONFIG_PATH).is_file() {
            return Ok(dir.to_path_buf());
        }
        bail!(
            "{} is not an OxiSpec project; run `oxispec init` there",
            dir.display()
        );
    }
    let cwd = std::env::current_dir()?;
    config::find_root(&cwd)
        .context("not an OxiSpec project here or in any parent folder; run `oxispec init`")
}

fn require_module(cfg: &Config, module: &str) -> Result<()> {
    if !cfg.has(module) {
        bail!("module `{module}` is not enabled; run `oxispec modules add {module}`");
    }
    Ok(())
}

/// Folders created for modules that keep files on disk.
fn ensure_module_dirs(root: &Path, cfg: &Config) -> Result<()> {
    if cfg.has("specs") {
        fs::create_dir_all(root.join("specs/requirements"))?;
    }
    if cfg.has("adrs") {
        fs::create_dir_all(root.join("specs/adr"))?;
    }
    Ok(())
}

fn init(dir: &Path, preset: &str, with: &[String], without: &[String]) -> Result<()> {
    if dir.join(config::CONFIG_PATH).exists() {
        bail!("{} is already initialized", dir.display());
    }
    let base = modules::preset(preset).with_context(|| {
        format!(
            "unknown preset `{preset}` (known: {})",
            modules::PRESETS.join(", ")
        )
    })?;
    let mut wanted: Vec<String> = base.iter().map(|s| s.to_string()).collect();
    wanted.extend(with.iter().cloned());
    for name in without {
        if name == "core" {
            bail!("`core` cannot be left out");
        }
        wanted.retain(|m| m != name);
    }
    let resolved = modules::resolve(&wanted).map_err(anyhow::Error::msg)?;
    for name in without {
        if let Some(needed_by) = resolved
            .iter()
            .find(|m| modules::info(m).is_some_and(|i| i.needs.contains(&name.as_str())))
        {
            bail!("cannot leave out `{name}`: module `{needed_by}` needs it");
        }
    }
    let cfg = Config {
        modules: resolved,
        topology: "single".to_string(),
    };
    fs::create_dir_all(dir)?;
    config::save(dir, &cfg)?;
    fs::write(dir.join(".oxispec/.gitignore"), "cache/\n")?;
    ensure_module_dirs(dir, &cfg)?;
    println!(
        "initialized {} with modules: {}",
        dir.display(),
        cfg.modules.join(", ")
    );
    warn_planned(&cfg.modules);
    Ok(())
}

fn warn_planned(names: &[String]) {
    let planned: Vec<&str> = names
        .iter()
        .filter(|n| modules::info(n).is_some_and(|i| !i.implemented))
        .map(String::as_str)
        .collect();
    if !planned.is_empty() {
        println!(
            "note: not implemented yet, recorded only: {}",
            planned.join(", ")
        );
    }
}

fn modules_command(explicit: Option<&Path>, action: ModulesAction) -> Result<()> {
    match action {
        ModulesAction::List => {
            let enabled = project_root(explicit)
                .ok()
                .and_then(|root| config::load(&root).ok())
                .map(|c| c.modules);
            for m in modules::MODULES {
                let state = match &enabled {
                    Some(list) if list.iter().any(|e| e == m.name) => "enabled",
                    Some(_) => "-",
                    None => "",
                };
                let status = if m.implemented { "" } else { " (planned)" };
                println!("{:<14} {:<8} {}{}", m.name, state, m.description, status);
            }
            Ok(())
        }
        ModulesAction::Add { names } => {
            let root = project_root(explicit)?;
            let mut cfg = config::load(&root)?;
            let before = cfg.modules.clone();
            let mut all = before.clone();
            all.extend(names);
            cfg.modules = modules::resolve(&all).map_err(anyhow::Error::msg)?;
            let added: Vec<String> = cfg
                .modules
                .iter()
                .filter(|m| !before.contains(m))
                .cloned()
                .collect();
            config::save(&root, &cfg)?;
            ensure_module_dirs(&root, &cfg)?;
            if added.is_empty() {
                println!("nothing to add");
            } else {
                println!("enabled: {}", added.join(", "));
                warn_planned(&added);
            }
            Ok(())
        }
        ModulesAction::Remove { names } => {
            let root = project_root(explicit)?;
            let mut cfg = config::load(&root)?;
            for name in &names {
                if name == "core" {
                    bail!("`core` cannot be removed");
                }
                if modules::info(name).is_none() {
                    bail!("unknown module `{name}`");
                }
                let remaining: Vec<String> = cfg
                    .modules
                    .iter()
                    .filter(|m| !names.contains(m))
                    .cloned()
                    .collect();
                let blockers = modules::dependents_of(name, &remaining);
                if !blockers.is_empty() {
                    bail!("cannot remove `{name}`: needed by {}", blockers.join(", "));
                }
            }
            cfg.modules.retain(|m| !names.contains(m));
            config::save(&root, &cfg)?;
            println!("disabled: {} (files are kept)", names.join(", "));
            Ok(())
        }
    }
}

fn check_command(root: &Path, json: bool, strict: bool, verbose: bool) -> Result<ExitCode> {
    let graph = Graph::load(root);
    let findings = check(&graph);
    let count = |s: Severity| findings.iter().filter(|f| f.severity == s).count();
    let (errors, warnings) = (count(Severity::Error), count(Severity::Warning));
    if json {
        let out = serde_json::json!({
            "nodes": graph.nodes.len(),
            "errors": errors,
            "warnings": warnings,
            "findings": findings,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        for f in &findings {
            if f.severity == Severity::Info && !verbose {
                continue;
            }
            let level = match f.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "info",
            };
            println!("{level}[{}]: {}", f.code, f.message);
            if let Some(path) = &f.path {
                match f.line {
                    Some(line) => println!("  --> {}:{line}", path.display()),
                    None => println!("  --> {}", path.display()),
                }
            }
        }
        println!(
            "{} nodes checked: {errors} errors, {warnings} warnings",
            graph.nodes.len()
        );
    }
    let failed = errors > 0 || (strict && warnings > 0);
    Ok(if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn show(root: &Path, id: &str, json: bool) -> Result<()> {
    let graph = Graph::load(root);
    let node = graph
        .get(id)
        .with_context(|| format!("no node with id {id}"))?;
    let incoming = graph.incoming(&node.id);
    if json {
        let incoming: Vec<_> = incoming
            .iter()
            .map(|(n, kind)| serde_json::json!({"id": n.id, "kind": kind, "path": n.path}))
            .collect();
        let out = serde_json::json!({"node": node, "incoming": incoming});
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    println!(
        "{}  {}  [{}]",
        node.id,
        node.ty,
        node.status.as_deref().unwrap_or("no status")
    );
    println!("path:  {}", node.path.display());
    if let Some(owner) = &node.owner {
        println!("owner: {owner}");
    }
    for r in &node.requirements {
        println!("requirement: {} ({} scenarios)", r.name, r.scenarios.len());
    }
    if !node.links.is_empty() {
        println!("links:");
        for (kind, targets) in &node.links {
            for t in targets {
                println!("  {kind} -> {t}");
            }
        }
    }
    if !incoming.is_empty() {
        println!("linked from:");
        for (n, kind) in incoming {
            println!("  {} ({kind})  {}", n.id, n.path.display());
        }
    }
    Ok(())
}

fn slug(title: &str) -> Result<String> {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        bail!("the title needs at least one letter or digit");
    }
    Ok(out)
}

fn next_number(graph: &Graph, prefix: &str) -> u32 {
    graph
        .nodes
        .iter()
        .filter_map(|n| n.id.strip_prefix(prefix))
        .filter_map(|s| s.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

/// Writes a new file, failing if it already exists.
fn write_new(root: &Path, rel: &Path, content: &str) -> Result<()> {
    let path = root.join(rel);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .with_context(|| format!("cannot create {}", rel.display()))?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

const REQUIREMENT_TEMPLATE: &str = "---
id: @ID@
type: requirement
status: draft
---
### Requirement: @TITLE@
The system SHALL <describe the behavior>.

#### Scenario: <name>
- **WHEN** <condition>
- **THEN** <outcome>
";

const ADR_TEMPLATE: &str = "---
id: @ID@
type: adr
status: proposed
---
# @ID@: @TITLE@

## Context

## Decision

## Consequences
";

fn new_requirement(root: &Path, domain: &str, title: &str) -> Result<(String, PathBuf)> {
    let domain = domain.to_ascii_uppercase();
    if domain.is_empty() || !domain.chars().all(|c| c.is_ascii_alphanumeric()) {
        bail!("the domain must be letters and digits, for example AUTH");
    }
    let prefix = format!("REQ-{domain}-");
    let id = format!("{prefix}{:04}", next_number(&Graph::load(root), &prefix));
    let rel = PathBuf::from("specs/requirements")
        .join(domain.to_ascii_lowercase())
        .join(format!("{}.md", slug(title)?));
    let body = REQUIREMENT_TEMPLATE
        .replace("@ID@", &id)
        .replace("@TITLE@", title);
    write_new(root, &rel, &body)?;
    Ok((id, rel))
}

fn new_adr(root: &Path, title: &str) -> Result<(String, PathBuf)> {
    let number = next_number(&Graph::load(root), "ADR-");
    let id = format!("ADR-{number:04}");
    let rel = PathBuf::from("specs/adr").join(format!("{number:04}-{}.md", slug(title)?));
    let body = ADR_TEMPLATE.replace("@ID@", &id).replace("@TITLE@", title);
    write_new(root, &rel, &body)?;
    Ok((id, rel))
}
