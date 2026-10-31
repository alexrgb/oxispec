//! `oxispec release`: prepare, check, publish and list spec releases.
//!
//! A release is a manifest node (`specs/releases/<version>.md`, type
//! `release`) that is committed, and an annotated git tag `specs-v<version>`
//! on the commit that holds it. The release exists once the tag does, so
//! publishing never edits a file.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use oxispec_core::config::{self, Config};
use oxispec_core::diff::{GraphDiff, diff_graphs};
use oxispec_core::release::{self, Candidate, Gate, ResultsFacts, Summary, TraceFacts, Version};
use oxispec_core::{Graph, Node, OUTPUT_SCHEMA_VERSION, index};
use oxispec_core::{results, suspect, trace};
use serde_json::json;

use crate::git::{self, SpecTag};

/// A release that has been published: its version and its tag.
struct Released {
    version: Version,
    tag: SpecTag,
}

/// Spec tags named `v<semver>`, newest version first.
fn released(root: &Path) -> Result<Vec<Released>> {
    let mut found: Vec<Released> = git::list_tags(root)?
        .into_iter()
        .filter(|t| t.name.starts_with('v'))
        .filter_map(|tag| {
            Some(Released {
                version: Version::parse(&tag.name)?,
                tag,
            })
        })
        .collect();
    found.sort_by(|a, b| b.version.cmp(&a.version));
    Ok(found)
}

/// The release a new one is compared with: the newest published, final
/// version below `version`. Pre-releases are never a base.
fn base_for<'a>(all: &'a [Released], version: Option<&Version>) -> Option<&'a Released> {
    all.iter()
        .filter(|r| r.version.pre.is_none())
        .find(|r| version.is_none_or(|v| r.version < *v))
}

fn parse_version(text: &str) -> Result<Version> {
    Version::parse(text)
        .with_context(|| format!("`{text}` is not a version like 1.2.3 or 1.2.3-rc.1"))
}

fn require_releases(cfg: &Config) -> Result<()> {
    if !cfg.has("releases") {
        bail!("module `releases` is not enabled; run `oxispec modules add releases`");
    }
    Ok(())
}

fn manifest_for<'a>(graph: &'a Graph, version: &Version) -> Option<&'a Node> {
    graph.nodes.iter().find(|n| {
        n.ty == "release"
            && n.version
                .as_deref()
                .and_then(Version::parse)
                .is_some_and(|v| v == *version)
    })
}

/// The specs as they were at a published release.
fn graph_of(root: &Path, release: Option<&Released>) -> Result<Graph> {
    match release {
        Some(r) => {
            let revision = git::resolve_rev(root, &r.tag.tag)?;
            git::graph_at(root, &revision)
        }
        None => Ok(Graph::default()),
    }
}

fn manifest_path(version: &Version) -> PathBuf {
    PathBuf::from("specs/releases").join(format!("{version}.md"))
}

fn manifest_text(
    id: &str,
    version: &Version,
    base: Option<&Version>,
    summary: &Summary,
    change_sets: &[&Node],
) -> String {
    let mut text =
        format!("---\nid: {id}\ntype: release\nstatus: candidate\nversion: \"{version}\"\n");
    if !change_sets.is_empty() {
        let ids: Vec<&str> = change_sets.iter().map(|n| n.id.as_str()).collect();
        text.push_str(&format!("links:\n  includes: [{}]\n", ids.join(", ")));
    }
    text.push_str(&format!("---\n# {id}: Release {version}\n\n"));
    text.push_str(&release::notes(base, summary, change_sets));
    text
}

fn gate_line(gate: &Gate) -> String {
    let mark = match (gate.passed, gate.blocking) {
        (true, _) => "pass",
        (false, true) => "FAIL",
        (false, false) => "warn",
    };
    format!("{mark:<5}{:<24} {}", gate.name, gate.detail)
}

/// `oxispec release prepare`: work out the version and the notes, and write
/// the manifest. Nothing is committed or tagged.
pub fn prepare(
    root: &Path,
    version: Option<&str>,
    force: bool,
    dry_run: bool,
    json_output: bool,
    cache: bool,
) -> Result<()> {
    let cfg = config::load(root)?;
    require_releases(&cfg)?;
    git::require_repository(root, "release")?;
    let requested = version.map(parse_version).transpose()?;

    let all = released(root)?;
    let base = base_for(&all, requested.as_ref());
    let base_version = base.map(|r| &r.version);
    let current = index::load(root, cache).0;
    let diff = diff_graphs(&graph_of(root, base)?, &current);
    let summary = release::summarize(&current, &diff);
    let change_sets = release::included_change_sets(&current, &diff);
    let suggested = release::suggest(base_version, summary.bump());

    let version = match (requested, &suggested) {
        (Some(v), _) => v,
        (None, Some(s)) => s.clone(),
        (None, None) => bail!(
            "nothing has changed since {}, so there is nothing to release",
            base_version.map_or_else(String::new, ToString::to_string)
        ),
    };
    if all.iter().any(|r| r.version == version) {
        bail!("{version} is already released; tags are never moved");
    }
    let existing = manifest_for(&current, &version);
    if existing.is_some() && !force && !dry_run {
        bail!(
            "{} already describes {version}; use --force to write the notes again",
            existing.map_or_else(String::new, |n| n.path.display().to_string())
        );
    }

    let (id, path) = match existing {
        Some(node) => (node.id.clone(), node.path.clone()),
        None => (
            crate::allocate_id(&current, "REL-", cfg.id_scheme)?,
            manifest_path(&version),
        ),
    };
    let text = manifest_text(&id, &version, base_version, &summary, &change_sets);
    if !dry_run {
        if existing.is_some() {
            fs::write(root.join(&path), &text)
                .with_context(|| format!("cannot write {}", path.display()))?;
        } else {
            crate::write_new(root, &path, &text)?;
        }
    }

    let warning = suggested
        .as_ref()
        .filter(|s| (version.major, version.minor, version.patch) < (s.major, s.minor, s.patch))
        .map(|s| format!("the changes suggest {s}, and {version} is lower"));
    if json_output {
        let out = json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "version": version.to_string(),
            "base": base_version.map(ToString::to_string),
            "suggested": suggested.as_ref().map(ToString::to_string),
            "bump": summary.bump(),
            "id": id,
            "path": path,
            "written": !dry_run,
            "warning": warning,
            "summary": summary,
            "change_sets": change_sets.iter().map(|n| json!({"id": n.id, "status": n.status})).collect::<Vec<_>>(),
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    match base_version {
        Some(b) => outln!("since {b}: a {:?} change", summary.bump()),
        None => outln!("no earlier release"),
    }
    if let Some(s) = &suggested {
        outln!("suggested version: {s}");
    }
    if let Some(w) = &warning {
        outln!("warning: {w}");
    }
    if dry_run {
        outln!("\n{text}");
        outln!("(dry run: nothing written)");
    } else {
        outln!("wrote {id} ({})", path.display());
        outln!(
            "review the notes, commit the file, then run `oxispec release check {version}` and `oxispec release publish {version}`"
        );
    }
    Ok(())
}

/// The gates for a release, run against `graph`.
struct Evaluation {
    version: Version,
    base: Option<Version>,
    gates: Vec<Gate>,
}

/// What the code and tests say, for a project that uses trace annotations.
/// `at` reads the code from a revision. Without it the working tree is read.
fn trace_facts(
    root: &Path,
    cfg: &Config,
    graph: &Graph,
    at: Option<&git::Revision>,
) -> Result<Option<TraceFacts>> {
    if !cfg.has("tests") {
        return Ok(None);
    }
    let scan = match at {
        Some(rev) => git::scan_at(root, rev, &cfg.trace.exclude)?,
        None => trace::scan_tree(root, &cfg.trace.exclude)?,
    };
    let lock = match at {
        Some(rev) => match git::file_at(root, &rev.oid, Path::new(suspect::LOCK_PATH))? {
            Some(bytes) => suspect::parse(
                Path::new(suspect::LOCK_PATH),
                &String::from_utf8_lossy(&bytes),
            )?,
            None => suspect::Lock::default(),
        },
        None => suspect::load(root)?,
    };
    let imported = results::load(root)?;
    if scan.annotations.is_empty() && lock.links.is_empty() && imported.is_none() {
        return Ok(None);
    }
    let suspects = suspect::links(graph, &scan.annotations, &lock)
        .iter()
        .filter(|l| l.state.is_suspect())
        .count();
    let head = git::resolve_rev(root, "HEAD")?;
    let (failing, facts) = match &imported {
        Some(found) => {
            let failing = results::scenario_status(graph, &scan.annotations, found)
                .into_iter()
                .filter(|(_, status)| *status == results::Status::Failing)
                .map(|(scenario, _)| scenario)
                .collect();
            (
                failing,
                Some(ResultsFacts {
                    current: found.commit.as_deref() == Some(head.oid.as_str()),
                }),
            )
        }
        None => (Vec::new(), None),
    };
    Ok(Some(TraceFacts {
        suspect: suspects,
        failing,
        results: facts,
    }))
}

fn evaluate(
    root: &Path,
    version: &Version,
    graph: &Graph,
    at: Option<&git::Revision>,
) -> Result<Evaluation> {
    let cfg = config::load(root)?;
    let manifest = manifest_for(graph, version).with_context(|| {
        format!("no release manifest for {version}; run `oxispec release prepare {version}`")
    })?;
    let all = released(root)?;
    let base = base_for(&all, Some(version));
    let diff: GraphDiff = diff_graphs(&graph_of(root, base)?, graph);
    let tag_exists = all.iter().any(|r| r.version == *version);
    let base_version = base.map(|r| r.version.clone());
    let gates = release::gates(&Candidate {
        version,
        base: base_version.as_ref(),
        manifest,
        graph,
        diff: &diff,
        tag_exists,
        trace: trace_facts(root, &cfg, graph, at)?,
    });
    Ok(Evaluation {
        version: version.clone(),
        base: base_version,
        gates,
    })
}

fn print_gates(evaluation: &Evaluation, json_output: bool) -> Result<()> {
    let ok = release::can_release(&evaluation.gates);
    if json_output {
        let out = json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "version": evaluation.version.to_string(),
            "base": evaluation.base.as_ref().map(ToString::to_string),
            "can_release": ok,
            "gates": evaluation.gates,
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        outln!("release {}", evaluation.version);
        for gate in &evaluation.gates {
            outln!("{}", gate_line(gate));
        }
        outln!(
            "{}",
            if ok {
                "ready to publish"
            } else {
                "not ready: fix the failed gates"
            }
        );
    }
    Ok(())
}

/// `oxispec release check`: the gates against the working tree. Returns
/// false when the release is not ready.
pub fn check(root: &Path, version: &str, json_output: bool, cache: bool) -> Result<bool> {
    let cfg = config::load(root)?;
    require_releases(&cfg)?;
    git::require_repository(root, "release")?;
    let version = parse_version(version)?;
    let graph = index::load(root, cache).0;
    let evaluation = evaluate(root, &version, &graph, None)?;
    print_gates(&evaluation, json_output)?;
    Ok(release::can_release(&evaluation.gates))
}

/// `oxispec release publish`: runs the gates on the last commit and, when
/// they pass, tags it `specs-v<version>`. Returns false when a gate failed.
///
/// The gates run on HEAD, not on the working tree, so the tag is on exactly
/// what was checked. The manifest must be committed.
pub fn publish(root: &Path, version: &str, json_output: bool) -> Result<bool> {
    let cfg = config::load(root)?;
    require_releases(&cfg)?;
    git::require_repository(root, "release")?;
    let version = parse_version(version)?;
    let head = git::resolve_rev(root, "HEAD")?;
    let graph = git::graph_at(root, &head)?;
    if manifest_for(&graph, &version).is_none() {
        let working = index::load(root, false).0;
        if manifest_for(&working, &version).is_some() {
            bail!("the manifest for {version} is not committed yet; commit it, then publish");
        }
    }
    let evaluation = evaluate(root, &version, &graph, Some(&head))?;
    if !release::can_release(&evaluation.gates) {
        print_gates(&evaluation, json_output)?;
        return Ok(false);
    }
    let message = format!("Spec release {version}");
    let tag = git::create_tag(root, &format!("v{version}"), &head.oid, &message)?;
    let loose = git::uncommitted_spec_files(root)?;
    if json_output {
        let out = json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "version": version.to_string(),
            "tag": tag,
            "commit": head.oid,
            "uncommitted_spec_files": loose,
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        outln!("released {version}: tagged {head} as {tag}");
        outln!("push it with `git push origin {tag}`");
        if loose > 0 {
            outln!(
                "note: {loose} spec file(s) have changes that are not committed, so the release does not include them"
            );
        }
    }
    Ok(true)
}

/// `oxispec release list`: the manifests, and whether each is published.
pub fn list(root: &Path, json_output: bool, cache: bool) -> Result<()> {
    let cfg = config::load(root)?;
    require_releases(&cfg)?;
    git::require_repository(root, "release")?;
    let all = released(root)?;
    let graph = index::load(root, cache).0;
    let mut rows: Vec<(Version, &Node)> = graph
        .nodes
        .iter()
        .filter(|n| n.ty == "release")
        .filter_map(|n| Some((Version::parse(n.version.as_deref()?)?, n)))
        .collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0));

    let describe = |version: &Version| all.iter().find(|r| r.version == *version);
    if json_output {
        let list: Vec<_> = rows
            .iter()
            .map(|(version, node)| {
                let tag = describe(version);
                json!({
                    "version": version.to_string(),
                    "id": node.id,
                    "path": node.path,
                    "state": if tag.is_some() { "released" } else { "candidate" },
                    "tag": tag.map(|r| &r.tag.tag),
                    "commit": tag.map(|r| &r.tag.commit),
                    "date": tag.map(|r| &r.tag.date),
                })
            })
            .collect();
        let out = json!({"schema_version": OUTPUT_SCHEMA_VERSION, "releases": list});
        outln!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    if rows.is_empty() {
        outln!("no releases yet; start one with `oxispec release prepare`");
    }
    for (version, node) in rows {
        match describe(&version) {
            Some(r) => outln!(
                "{:<14} {:<10} released {} ({})  {}",
                version.to_string(),
                node.id,
                r.tag.date,
                r.tag.commit,
                r.tag.tag
            ),
            None => outln!(
                "{:<14} {:<10} candidate  {}",
                version.to_string(),
                node.id,
                node.path.display()
            ),
        }
    }
    Ok(())
}
