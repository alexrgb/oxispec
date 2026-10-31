//! `oxispec agents`: skills for AI clients, written into their folders.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use oxispec_core::OUTPUT_SCHEMA_VERSION;
use oxispec_core::agents::{self, CLIENTS, Client, DEFAULT_CLIENTS, Planned, SKILLS, State};
use oxispec_core::config::{self, Config};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ExportTarget {
    /// `skills/<name>/SKILL.md`, the layout `npx skills add <owner>/<repo>` installs from
    SkillsRepo,
    /// `.ruler/skills/<name>/SKILL.md`, which `ruler apply` copies to each agent
    Ruler,
}

impl ExportTarget {
    fn dir(self) -> &'static str {
        match self {
            ExportTarget::SkillsRepo => "skills",
            ExportTarget::Ruler => ".ruler/skills",
        }
    }
}

/// The clients to write for: the ones asked for, else the project's, else the defaults.
fn resolve_clients(asked: &[String], cfg: &Config) -> Result<Vec<&'static Client>> {
    let ids: Vec<String> = if !asked.is_empty() {
        asked.to_vec()
    } else if !cfg.agents.is_empty() {
        cfg.agents.clone()
    } else {
        DEFAULT_CLIENTS.iter().map(|s| s.to_string()).collect()
    };
    let mut out: Vec<&'static Client> = Vec::new();
    for id in &ids {
        let client = agents::client(id).with_context(|| {
            format!(
                "unknown client `{id}` (known: {})",
                CLIENTS.iter().map(|c| c.id).collect::<Vec<_>>().join(", ")
            )
        })?;
        if !out.contains(&client) {
            out.push(client);
        }
    }
    Ok(out)
}

/// Checks a list given to `init --agents`. `none` means no client.
pub fn parse_clients(list: &[String]) -> Result<Vec<String>> {
    if list.iter().any(|c| c == "none") {
        if list.len() > 1 {
            bail!("`none` cannot be combined with other clients");
        }
        return Ok(Vec::new());
    }
    for id in list {
        if agents::client(id).is_none() {
            bail!(
                "unknown client `{id}` (known: {})",
                CLIENTS.iter().map(|c| c.id).collect::<Vec<_>>().join(", ")
            );
        }
    }
    Ok(list.to_vec())
}

fn shown(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn need_agents(cfg: &Config) -> Result<()> {
    if !cfg.has("agents") {
        bail!("module `agents` is not enabled; run `oxispec modules add agents`");
    }
    Ok(())
}

fn state_name(state: State) -> &'static str {
    match state {
        State::Missing => "missing",
        State::Current => "current",
        State::Outdated => "outdated",
        State::Edited => "edited",
    }
}

fn write_file(root: &Path, planned: &Planned) -> Result<()> {
    let path = root.join(&planned.path);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    fs::write(&path, &planned.content).with_context(|| format!("cannot write {}", path.display()))
}

/// What `update` did, for the closing line.
#[derive(Default)]
struct Tally {
    written: usize,
    unchanged: usize,
    skipped: usize,
}

fn apply(root: &Path, plan: &[Planned], force: bool, dry_run: bool) -> Result<Tally> {
    let mut tally = Tally::default();
    for p in plan {
        let path = shown(&p.path);
        match p.state {
            State::Current => tally.unchanged += 1,
            State::Edited if !force => {
                tally.skipped += 1;
                outln!("skipped {path} (edited by hand; `--force` replaces it)");
            }
            _ => {
                if dry_run {
                    outln!("would write {path}");
                } else {
                    write_file(root, p)?;
                    outln!("wrote {path}");
                }
                tally.written += 1;
            }
        }
    }
    Ok(tally)
}

/// `oxispec init`: writes skills for the clients the project chose.
pub fn write_after_init(root: &Path, cfg: &Config) -> Result<()> {
    // No clients means `--agents none`.
    if !cfg.has("agents") || cfg.agents.is_empty() {
        return Ok(());
    }
    let clients = resolve_clients(&cfg.agents, cfg)?;
    let plan = agents::plan(root, &clients, &cfg.modules);
    let tally = apply(root, &plan, false, false)?;
    outln!(
        "agent skills: {} files for {}",
        tally.written,
        clients
            .iter()
            .map(|c| c.name)
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(())
}

pub fn update(root: &Path, asked: &[String], force: bool, dry_run: bool) -> Result<ExitCode> {
    let cfg = config::load(root)?;
    need_agents(&cfg)?;
    let clients = resolve_clients(asked, &cfg)?;
    let plan = agents::plan(root, &clients, &cfg.modules);
    let tally = apply(root, &plan, force, dry_run)?;
    let verb = if dry_run { "would write" } else { "written" };
    outln!(
        "{} {verb}, {} unchanged, {} skipped",
        tally.written,
        tally.unchanged,
        tally.skipped
    );
    Ok(if tally.skipped > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

pub fn check(root: &Path, asked: &[String], json: bool) -> Result<ExitCode> {
    let cfg = config::load(root)?;
    need_agents(&cfg)?;
    let clients = resolve_clients(asked, &cfg)?;
    let plan = agents::plan(root, &clients, &cfg.modules);
    let drift = plan.iter().filter(|p| p.state != State::Current).count();
    if json {
        let files: Vec<_> = plan
            .iter()
            .map(|p| {
                serde_json::json!({
                    "client": p.client,
                    "skill": p.skill,
                    "path": shown(&p.path),
                    "state": state_name(p.state),
                })
            })
            .collect();
        let out = serde_json::json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "ok": drift == 0,
            "files": files,
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        for p in plan.iter().filter(|p| p.state != State::Current) {
            let why = match p.state {
                State::Missing => "missing",
                State::Outdated => "out of date",
                _ => "edited by hand, or not written by oxispec",
            };
            outln!("{}: {why}", shown(&p.path));
        }
        if drift == 0 {
            outln!("{} skill files are up to date", plan.len());
        } else {
            outln!(
                "{drift} of {} skill files differ; run `oxispec agents update`",
                plan.len()
            );
        }
    }
    Ok(if drift == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

pub fn list(root: &Path, json: bool) -> Result<()> {
    let cfg = config::load(root)?;
    let chosen = resolve_clients(&[], &cfg)?;
    let enabled = agents::skills_for(&cfg.modules);
    if json {
        let clients: Vec<_> = CLIENTS
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "name": c.name,
                    "dir": c.dir,
                    "selected": chosen.contains(&c),
                })
            })
            .collect();
        let skills: Vec<_> = SKILLS
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "requires": s.requires,
                    "enabled": enabled.iter().any(|e| e.name == s.name),
                })
            })
            .collect();
        let out = serde_json::json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "module_enabled": cfg.has("agents"),
            "clients": clients,
            "skills": skills,
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    if !cfg.has("agents") {
        outln!("module `agents` is not enabled; run `oxispec modules add agents`");
    }
    outln!("clients:");
    for c in CLIENTS {
        let mark = if chosen.contains(&c) { "*" } else { " " };
        outln!("  {mark} {:<8} {:<42} {}", c.id, c.name, c.dir);
    }
    outln!("skills:");
    for s in SKILLS {
        let note = if enabled.iter().any(|e| e.name == s.name) {
            String::new()
        } else {
            format!("  (needs module {})", s.requires.join(", "))
        };
        outln!("  {}{note}", s.name);
    }
    Ok(())
}

/// Writes the skills in the layout another tool reads. With `all`, every skill and
/// no project notes, which needs no project: that is how `skills/` in this repository is made.
pub fn export(root: Option<&Path>, to: ExportTarget, out: &Path, all: bool) -> Result<()> {
    let (skills, notes) = if all {
        (SKILLS.iter().collect::<Vec<_>>(), String::new())
    } else {
        let root = root.context("not an OxiSpec project; run `oxispec init`, or use --all")?;
        let cfg = config::load(root)?;
        (agents::skills_for(&cfg.modules), agents::overlay(root, ""))
    };
    let base: PathBuf = out.join(to.dir());
    for skill in &skills {
        let dir = base.join(skill.name);
        fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
        let file = dir.join("SKILL.md");
        fs::write(&file, agents::render(skill, &notes))
            .with_context(|| format!("cannot write {}", file.display()))?;
    }
    outln!("wrote {} skills to {}", skills.len(), base.display());
    Ok(())
}
