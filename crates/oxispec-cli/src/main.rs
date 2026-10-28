use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand, ValueEnum};
use oxispec_core::check::{LinkResolution, resolve_link};
use oxispec_core::config::{self, Config, IdScheme};
use oxispec_core::edit::{LinkEdit, add_link};
use oxispec_core::impact::{impact, in_scope};
use oxispec_core::index;
use oxispec_core::node::KNOWN_LINK_KINDS;
use oxispec_core::{Finding, Graph, OUTPUT_SCHEMA_VERSION, Severity, check, id, modules};

mod changes;
mod git;
mod history;
mod releases;
mod rollback;

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
    /// Do not read or write the local index in .oxispec/cache
    #[arg(long, global = true)]
    no_cache: bool,
    #[command(subcommand)]
    command: Command,
}

/// How new IDs are numbered.
#[derive(Clone, Copy, ValueEnum)]
enum IdSchemeArg {
    /// REQ-AUTH-0001: the next free number
    Sequential,
    /// REQ-AUTH-7K3F: a random suffix, safe with parallel branches
    Random,
}

impl From<IdSchemeArg> for IdScheme {
    fn from(arg: IdSchemeArg) -> Self {
        match arg {
            IdSchemeArg::Sequential => IdScheme::Sequential,
            IdSchemeArg::Random => IdScheme::Random,
        }
    }
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
        /// How new IDs are numbered. Use random when several branches add specs at once.
        #[arg(long, value_enum, default_value_t = IdSchemeArg::Sequential)]
        id_scheme: IdSchemeArg,
    },
    /// List, add or remove modules
    Modules {
        #[command(subcommand)]
        action: Option<ModulesAction>,
    },
    /// Create a node from a template
    New {
        /// Override the project's ID scheme for this node
        #[arg(long, global = true, value_enum)]
        id_scheme: Option<IdSchemeArg>,
        #[command(subcommand)]
        kind: NewKind,
    },
    /// Work with change sets
    Change {
        #[command(subcommand)]
        action: ChangeAction,
    },
    /// Prepare, check and publish releases of the specs
    Release {
        #[command(subcommand)]
        action: ReleaseAction,
    },
    /// Add a typed link from one node to another, for example: link REQ-AUTH-0001 decided-by ADR-0001
    Link {
        /// The node that holds the link
        from: String,
        /// satisfies, verifies, implements, decided-by, supersedes, refines, governed-by,
        /// describes, overrides, conflicts-with, depends-on, changes or includes
        kind: String,
        /// A node, or a scenario of a node such as REQ-AUTH-0001.S1
        to: String,
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
        /// Only report findings for spec files that changed since BASE, and the nodes that link to them
        #[arg(long)]
        changed: bool,
        /// The git ref to compare with, for --changed
        #[arg(long, default_value = "HEAD", requires = "changed")]
        base: String,
        /// Check the specs as they were at a git revision (a branch, tag, hash or date:YYYY-MM-DD)
        #[arg(long, conflicts_with = "changed")]
        at: Option<String>,
    },
    /// Show how a node changed over time, from git history
    History {
        id: String,
        /// Look at history up to this revision, and find nodes that no longer exist
        #[arg(long)]
        at: Option<String>,
        /// Number of commits to show
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Put nodes back to how they were at a git revision, after checking nothing breaks.
    /// Nothing is committed: review the result and commit it.
    Rollback {
        /// A branch, tag, hash, or date:YYYY-MM-DD
        revision: String,
        /// The nodes to restore
        ids: Vec<String>,
        /// Restore every node that differs from that revision
        #[arg(long)]
        all: bool,
        /// Go ahead although it would introduce errors or lose uncommitted work
        #[arg(long)]
        force: bool,
        /// Show what would happen and change nothing
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Name a revision with a tag (specs-NAME), or list the tags when no name is given
    Tag {
        name: Option<String>,
        /// The revision to tag (default: HEAD)
        #[arg(long)]
        at: Option<String>,
        /// The tag's message
        #[arg(short, long)]
        message: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show how the specs differ between two revisions (default for the second: the working tree)
    Diff {
        /// A branch, tag, hash, or date:YYYY-MM-DD
        from: String,
        to: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Build or refresh the local index in .oxispec/cache
    Index {
        /// Delete the cache first, so every file is parsed again
        #[arg(long)]
        rebuild: bool,
        #[arg(long)]
        json: bool,
    },
    /// Show a node, its links, and the nodes that link to it
    Show {
        id: String,
        #[arg(long)]
        json: bool,
        /// Show the node as it was at a git revision (a branch, tag, hash or date:YYYY-MM-DD)
        #[arg(long)]
        at: Option<String>,
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
    /// A change set: a proposed change to the specs, with a lifecycle
    Change { title: String },
}

#[derive(Subcommand)]
enum ChangeAction {
    /// Move a change set along draft, in-review, approved, merged, reverted
    Status {
        id: String,
        status: String,
        /// Who approves, when moving to approved (default: git user.name)
        #[arg(long)]
        by: Option<String>,
        /// Skip a step, or set a status that is not the next one
        #[arg(long)]
        force: bool,
    },
    /// List change sets
    List {
        /// Only change sets with this status
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ReleaseAction {
    /// Work out the version and notes since the last release, and write the manifest
    Prepare {
        /// The version, for example 0.2.0 (default: the suggested one)
        version: Option<String>,
        /// Write the notes again for a version that already has a manifest
        #[arg(long)]
        force: bool,
        /// Show the manifest and write nothing
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Run the gates for a release. Exits with 1 when it is not ready
    Check {
        version: String,
        #[arg(long)]
        json: bool,
    },
    /// Run the gates on the last commit and tag it specs-vVERSION
    Publish {
        version: String,
        #[arg(long)]
        json: bool,
    },
    /// List releases and whether they are published
    List {
        #[arg(long)]
        json: bool,
    },
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
    let cache = !cli.no_cache;
    match cli.command {
        Command::Init {
            preset,
            with,
            without,
            id_scheme,
        } => {
            let dir = cli.root.unwrap_or_else(|| PathBuf::from("."));
            init(&dir, &preset, &with, &without, id_scheme.into())?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Modules { action } => {
            modules_command(cli.root.as_deref(), action.unwrap_or(ModulesAction::List))?;
            Ok(ExitCode::SUCCESS)
        }
        Command::New { id_scheme, kind } => {
            let root = project_root(cli.root.as_deref())?;
            let cfg = config::load(&root)?;
            let scheme = id_scheme.map_or(cfg.id_scheme, IdScheme::from);
            let created = match kind {
                NewKind::Requirement { domain, title } => {
                    require_module(&cfg, "specs")?;
                    new_requirement(&root, &domain, &title, scheme, cache)?
                }
                NewKind::Adr { title } => {
                    require_module(&cfg, "adrs")?;
                    new_adr(&root, &title, scheme, cache)?
                }
                NewKind::Change { title } => {
                    require_module(&cfg, "specs")?;
                    new_change(&root, &title, scheme, cache)?
                }
            };
            println!("created {} ({})", created.0, created.1.display());
            Ok(ExitCode::SUCCESS)
        }
        Command::Change { action } => {
            let root = project_root(cli.root.as_deref())?;
            match action {
                ChangeAction::Status {
                    id,
                    status,
                    by,
                    force,
                } => changes::change_status(&root, &id, &status, by.as_deref(), force, cache)?,
                ChangeAction::List { status, json } => {
                    changes::change_list(&root, status.as_deref(), json, cache)?
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Release { action } => {
            let root = project_root(cli.root.as_deref())?;
            let ready = match action {
                ReleaseAction::Prepare {
                    version,
                    force,
                    dry_run,
                    json,
                } => {
                    releases::prepare(&root, version.as_deref(), force, dry_run, json, cache)?;
                    true
                }
                ReleaseAction::Check { version, json } => {
                    releases::check(&root, &version, json, cache)?
                }
                ReleaseAction::Publish { version, json } => {
                    releases::publish(&root, &version, json)?
                }
                ReleaseAction::List { json } => {
                    releases::list(&root, json, cache)?;
                    true
                }
            };
            Ok(if ready {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        Command::Link { from, kind, to } => {
            let root = project_root(cli.root.as_deref())?;
            config::load(&root)?;
            link_command(&root, &from, &kind, &to, cache)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Check {
            json,
            strict,
            verbose,
            changed,
            base,
            at,
        } => {
            let options = CheckOptions {
                json,
                strict,
                verbose,
                changed_since: changed.then_some(base),
                at,
            };
            check_command(&project_root(cli.root.as_deref())?, &options, cache)
        }
        Command::Index { rebuild, json } => {
            if !cache {
                bail!("`index` builds the local index, so it cannot be used with --no-cache");
            }
            index_command(&project_root(cli.root.as_deref())?, rebuild, json)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Show { id, json, at } => {
            show(
                &project_root(cli.root.as_deref())?,
                &id,
                json,
                cache,
                at.as_deref(),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Command::History {
            id,
            at,
            limit,
            json,
        } => {
            let root = project_root(cli.root.as_deref())?;
            history::history_command(&root, &id, at.as_deref(), limit, json, cache)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Rollback {
            revision,
            ids,
            all,
            force,
            dry_run,
            json,
        } => {
            let root = project_root(cli.root.as_deref())?;
            let options = rollback::Options {
                revision: &revision,
                ids,
                all,
                force,
                dry_run,
                json,
                cache,
            };
            rollback::rollback_command(&root, &options)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Tag {
            name,
            at,
            message,
            json,
        } => {
            let root = project_root(cli.root.as_deref())?;
            history::tag_command(
                &root,
                name.as_deref(),
                at.as_deref(),
                message.as_deref(),
                json,
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Diff { from, to, json } => {
            let root = project_root(cli.root.as_deref())?;
            history::diff_command(&root, &from, to.as_deref(), json, cache)?;
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
        fs::create_dir_all(root.join("specs/changes"))?;
    }
    if cfg.has("adrs") {
        fs::create_dir_all(root.join("specs/adr"))?;
    }
    if cfg.has("releases") {
        fs::create_dir_all(root.join("specs/releases"))?;
    }
    Ok(())
}

fn init(
    dir: &Path,
    preset: &str,
    with: &[String],
    without: &[String],
    id_scheme: IdScheme,
) -> Result<()> {
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
    let cfg = Config::new(resolved, id_scheme);
    fs::create_dir_all(dir)?;
    config::save(dir, &cfg)?;
    fs::write(dir.join(".oxispec/.gitignore"), "cache/\n")?;
    ensure_module_dirs(dir, &cfg)?;
    let scheme = match cfg.id_scheme {
        IdScheme::Sequential => "sequential",
        IdScheme::Random => "random",
    };
    println!(
        "initialized {} with modules: {} (ids: {scheme})",
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

fn link_command(root: &Path, from: &str, kind: &str, to: &str, cache: bool) -> Result<()> {
    let graph = load_graph(root, cache);
    let parsed = id::parse(from).with_context(|| format!("{from:?} is not a valid id"))?;
    if parsed.namespace.is_some() || parsed.child.is_some() {
        bail!("a link starts at a node of this project, not at {from}");
    }
    let node = graph
        .get(parsed.base)
        .with_context(|| format!("no node with id {from}"))?;
    if !KNOWN_LINK_KINDS.contains(&kind) {
        bail!(
            "unknown link kind `{kind}` (known: {})",
            KNOWN_LINK_KINDS.join(", ")
        );
    }
    match resolve_link(&graph, to) {
        LinkResolution::Ok | LinkResolution::External => {}
        LinkResolution::Invalid => bail!("{to:?} is not a valid id"),
        LinkResolution::MissingNode => bail!("{to} does not exist"),
        LinkResolution::MissingScenario(anchor) => bail!(
            "{} has no scenario with anchor {{#{anchor}}}; add it to the scenario heading first",
            id::base_of(to)
        ),
    }
    if id::base_of(to) == node.id {
        bail!("a node cannot link to itself");
    }

    let path = root.join(&node.path);
    let text =
        fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
    match add_link(&text, kind, to)
        .with_context(|| format!("cannot edit {}", node.path.display()))?
    {
        LinkEdit::Added(updated) => {
            fs::write(&path, updated)
                .with_context(|| format!("cannot write {}", path.display()))?;
            println!("linked {} -[{kind}]-> {to}", node.id);
        }
        LinkEdit::AlreadyLinked => println!("{} already has {kind} -> {to}", node.id),
    }
    Ok(())
}

struct CheckOptions {
    json: bool,
    strict: bool,
    verbose: bool,
    /// `Some(base)` limits the report to files changed since that git ref.
    changed_since: Option<String>,
    /// Check the specs as they were at this git revision.
    at: Option<String>,
}

/// `error[E002]: message`, the first line of a finding.
pub(crate) fn finding_line(f: &Finding) -> String {
    let level = match f.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    };
    format!("{level}[{}]: {}", f.code, f.message)
}

fn revision_json(revision: &git::Revision) -> serde_json::Value {
    serde_json::json!({"commit": revision.oid, "short": revision.short, "subject": revision.subject})
}

fn load_graph(root: &Path, cache: bool) -> Graph {
    index::load(root, cache).0
}

fn index_command(root: &Path, rebuild: bool, json: bool) -> Result<()> {
    config::load(root)?;
    if rebuild {
        index::clear(root).context("cannot delete the old cache")?;
    }
    let (graph, stats) = index::load(root, true);
    if json {
        let out = serde_json::json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "nodes": graph.nodes.len(),
            "files": stats.files,
            "read": stats.read,
            "parsed": stats.parsed,
            "reused": stats.reused,
            "removed": stats.removed,
            "cache_written": stats.cache_written,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!(
            "indexed {} markdown files ({} nodes): {} parsed, {} reused, {} removed",
            stats.files,
            graph.nodes.len(),
            stats.parsed,
            stats.reused,
            stats.removed
        );
        if stats.parsed > 0 && !stats.cache_written {
            println!(
                "note: the cache could not be written, so the next run parses everything again"
            );
        }
    }
    Ok(())
}

fn check_command(root: &Path, options: &CheckOptions, cache: bool) -> Result<ExitCode> {
    config::load(root)?;
    let revision = options
        .at
        .as_deref()
        .map(|spec| git::resolve_rev(root, spec))
        .transpose()?;
    let graph = match &revision {
        Some(rev) => git::graph_at(root, rev)?,
        None => load_graph(root, cache),
    };
    let mut findings = check(&graph);
    let total = graph.nodes.len();
    let mut checked = total;
    let mut scope = None;
    if let Some(base) = &options.changed_since {
        let changes = git::changes(root, base)?;
        let touched = impact(&graph, &changes.paths, &changes.old_ids);
        findings.retain(|f| in_scope(f, &touched));
        checked = touched.nodes;
        scope = Some((base.as_str(), changes.paths.len()));
    }
    let count = |s: Severity| findings.iter().filter(|f| f.severity == s).count();
    let (errors, warnings) = (count(Severity::Error), count(Severity::Warning));
    if options.json {
        let out = serde_json::json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "at": revision.as_ref().map(revision_json),
            "nodes": total,
            "checked_nodes": checked,
            "scope": scope.map(|(base, files)| serde_json::json!({"base": base, "changed_files": files})),
            "errors": errors,
            "warnings": warnings,
            "findings": findings,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        if let Some(rev) = &revision {
            println!("checking {rev}");
        }
        for f in &findings {
            if f.severity == Severity::Info && !options.verbose {
                continue;
            }
            println!("{}", finding_line(f));
            if let Some(path) = &f.path {
                match f.line {
                    Some(line) => println!("  --> {}:{line}", path.display()),
                    None => println!("  --> {}", path.display()),
                }
            }
        }
        match scope {
            Some((base, files)) => println!(
                "{checked} of {total} nodes checked ({files} changed files since {base}, and the nodes that link to them): {errors} errors, {warnings} warnings"
            ),
            None => println!("{total} nodes checked: {errors} errors, {warnings} warnings"),
        }
    }
    let failed = errors > 0 || (options.strict && warnings > 0);
    Ok(if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn show(root: &Path, id: &str, json: bool, cache: bool, at: Option<&str>) -> Result<()> {
    config::load(root)?;
    let revision = at.map(|spec| git::resolve_rev(root, spec)).transpose()?;
    let graph = match &revision {
        Some(rev) => git::graph_at(root, rev)?,
        None => load_graph(root, cache),
    };
    let node = graph
        .get(id)
        .with_context(|| format!("no node with id {id}"))?;
    let incoming = graph.incoming(&node.id);
    if json {
        let incoming: Vec<_> = incoming
            .iter()
            .map(|(n, kind)| serde_json::json!({"id": n.id, "kind": kind, "path": n.path}))
            .collect();
        let out = serde_json::json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "at": revision.as_ref().map(revision_json),
            "node": node,
            "incoming": incoming,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    if let Some(rev) = &revision {
        println!("at {rev}");
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
        let anchors: Vec<&str> = r
            .scenarios
            .iter()
            .filter_map(|s| s.anchor.as_deref())
            .collect();
        let count = r.scenarios.len();
        let noun = if count == 1 { "scenario" } else { "scenarios" };
        if anchors.is_empty() {
            println!("requirement: {} ({count} {noun})", r.name);
        } else {
            println!(
                "requirement: {} ({count} {noun}, anchors {})",
                r.name,
                anchors.join(", ")
            );
        }
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

/// Picks a free ID starting with `prefix`. Random IDs are checked against
/// the nodes on disk, and another branch can still pick the same one, but
/// with about a million values that is rare and `oxispec check` reports it.
fn allocate_id(graph: &Graph, prefix: &str, scheme: IdScheme) -> Result<String> {
    match scheme {
        IdScheme::Sequential => Ok(format!("{prefix}{:04}", next_number(graph, prefix))),
        IdScheme::Random => {
            for _ in 0..50 {
                let mut bytes = [0u8; 4];
                getrandom::fill(&mut bytes).map_err(|e| anyhow!("cannot read randomness: {e}"))?;
                let candidate = format!("{prefix}{}", id::random_suffix(u32::from_le_bytes(bytes)));
                if graph.get(&candidate).is_none() {
                    return Ok(candidate);
                }
            }
            bail!("could not find a free random id; try again")
        }
    }
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

#### Scenario: <name> {#S1}
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

const CHANGE_TEMPLATE: &str = "---
id: @ID@
type: change
status: draft
---
# @ID@: @TITLE@

## Why

## What changes

## Impact
";

fn new_requirement(
    root: &Path,
    domain: &str,
    title: &str,
    scheme: IdScheme,
    cache: bool,
) -> Result<(String, PathBuf)> {
    let domain = domain.to_ascii_uppercase();
    if domain.is_empty() || !domain.chars().all(|c| c.is_ascii_alphanumeric()) {
        bail!("the domain must be letters and digits, for example AUTH");
    }
    let id = allocate_id(&load_graph(root, cache), &format!("REQ-{domain}-"), scheme)?;
    let rel = PathBuf::from("specs/requirements")
        .join(domain.to_ascii_lowercase())
        .join(format!("{}.md", slug(title)?));
    let body = REQUIREMENT_TEMPLATE
        .replace("@ID@", &id)
        .replace("@TITLE@", title);
    write_new(root, &rel, &body)?;
    Ok((id, rel))
}

fn new_adr(root: &Path, title: &str, scheme: IdScheme, cache: bool) -> Result<(String, PathBuf)> {
    let id = allocate_id(&load_graph(root, cache), "ADR-", scheme)?;
    let suffix = id.strip_prefix("ADR-").unwrap_or(&id);
    let rel = PathBuf::from("specs/adr").join(format!("{suffix}-{}.md", slug(title)?));
    let body = ADR_TEMPLATE.replace("@ID@", &id).replace("@TITLE@", title);
    write_new(root, &rel, &body)?;
    Ok((id, rel))
}

fn new_change(
    root: &Path,
    title: &str,
    scheme: IdScheme,
    cache: bool,
) -> Result<(String, PathBuf)> {
    let id = allocate_id(&load_graph(root, cache), "CHG-", scheme)?;
    let suffix = id.strip_prefix("CHG-").unwrap_or(&id);
    let rel = PathBuf::from("specs/changes").join(format!("{suffix}-{}.md", slug(title)?));
    let body = CHANGE_TEMPLATE
        .replace("@ID@", &id)
        .replace("@TITLE@", title);
    write_new(root, &rel, &body)?;
    Ok((id, rel))
}
