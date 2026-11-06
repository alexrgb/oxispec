//! `oxispec trace` and `oxispec coverage`: how code and tests tie back to the specs.

use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use oxispec_core::results;
use oxispec_core::suspect::{self, State};
use oxispec_core::trace::{Annotation, Coverage, Kind, Scan, coverage, scan_tree};
use oxispec_core::{OUTPUT_SCHEMA_VERSION, config, id, index};
use serde_json::json;
use std::path::PathBuf;

use crate::git;

// spec: REQ-TRACE-0001
// spec: REQ-TRACE-0003
fn scan(root: &Path) -> Result<Scan> {
    let cfg = config::load(root)?;
    if !cfg.has("tests") {
        bail!("module `tests` is not enabled; run `oxispec modules add tests`");
    }
    let scan = scan_tree(root, &cfg.trace.exclude)?;
    Ok(scan)
}

fn place(a: &Annotation) -> String {
    format!("{}:{}", a.path.display(), a.line)
}

pub fn trace_command(
    root: &Path,
    target: Option<&str>,
    json_output: bool,
    cache: bool,
) -> Result<()> {
    let scan = scan(root)?;
    let graph = index::load(root, cache).0;
    let Some(target) = target else {
        if json_output {
            let out = json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "files_scanned": scan.files,
                "annotations": scan.annotations,
            });
            outln!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            for a in &scan.annotations {
                let kind = match a.kind {
                    Kind::Implements => "spec",
                    Kind::Verifies => "verifies",
                };
                outln!("{:<34} {kind:<9} {}", place(a), a.target);
            }
            outln!(
                "{} annotations in {} files",
                scan.annotations.len(),
                scan.files
            );
        }
        return Ok(());
    };

    let node = graph
        .get(id::base_of(target))
        .with_context(|| format!("no node with id {target}"))?;
    let mine: Vec<&Annotation> = scan
        .annotations
        .iter()
        .filter(|a| id::base_of(&a.target) == node.id)
        .collect();
    let implementers: Vec<&&Annotation> =
        mine.iter().filter(|a| a.kind == Kind::Implements).collect();
    let title = node
        .title
        .clone()
        .or_else(|| node.requirements.first().map(|r| r.name.clone()))
        .unwrap_or_default();

    // Per scenario: the tests that name it, or the requirement as a whole.
    let mut scenarios = Vec::new();
    for scenario in node.requirements.iter().flat_map(|r| &r.scenarios) {
        let anchor = scenario.anchor.clone();
        let tests: Vec<&&Annotation> = mine
            .iter()
            .filter(|a| a.kind == Kind::Verifies)
            .filter(|a| {
                let child = id::parse(&a.target).and_then(|p| p.child);
                child.is_none() || child == anchor.as_deref()
            })
            .collect();
        scenarios.push((anchor, scenario.name.clone(), tests));
    }

    if json_output {
        let out = json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "id": node.id,
            "title": title,
            "status": node.status,
            "implemented_by": implementers,
            "scenarios": scenarios.iter().map(|(anchor, name, tests)| json!({
                "anchor": anchor, "name": name, "verified_by": tests,
            })).collect::<Vec<_>>(),
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    outln!(
        "{}  {title}  ({})",
        node.id,
        node.status.as_deref().unwrap_or("no status")
    );
    outln!("implemented by:");
    if implementers.is_empty() {
        outln!("  nothing yet");
    }
    for a in implementers {
        outln!("  {}", place(a));
    }
    outln!("scenarios:");
    if scenarios.is_empty() {
        outln!("  none");
    }
    for (anchor, name, tests) in scenarios {
        let label = anchor.unwrap_or_else(|| "(no anchor)".to_string());
        if tests.is_empty() {
            outln!("  {label:<12} {name}: NOT VERIFIED");
        } else {
            let places: Vec<String> = tests.iter().map(|a| place(a)).collect();
            outln!("  {label:<12} {name}: verified by {}", places.join(", "));
        }
    }
    Ok(())
}

pub struct CoverageOptions {
    pub status: Option<String>,
    pub fail_under: Option<f64>,
    /// Only scenarios whose tests passed count as covered.
    pub require_pass: bool,
    pub all: bool,
    pub json: bool,
    pub cache: bool,
}

fn list(title: &str, items: &[String], all: bool) {
    if items.is_empty() {
        return;
    }
    outln!("{title}:");
    let shown = if all {
        items.len()
    } else {
        items.len().min(20)
    };
    for item in &items[..shown] {
        outln!("  {item}");
    }
    if shown < items.len() {
        outln!(
            "  ... and {} more (--all shows everything)",
            items.len() - shown
        );
    }
}

// spec: REQ-TRACE-0002
pub fn coverage_command(root: &Path, options: &CoverageOptions) -> Result<ExitCode> {
    let scan = scan(root)?;
    let graph = index::load(root, options.cache).0;
    let result: Coverage = coverage(&graph, &scan.annotations, options.status.as_deref());

    // What the imported test results say, for the scenarios that are in scope.
    let imported = results::load(root)?;
    if options.require_pass && imported.is_none() {
        bail!("no test results were imported; run `oxispec results import FILE` first");
    }
    let in_scope: std::collections::BTreeSet<String> = graph
        .nodes
        .iter()
        .filter(|n| n.ty == "requirement")
        .filter(|n| {
            options
                .status
                .as_deref()
                .is_none_or(|s| n.status.as_deref() == Some(s))
        })
        .flat_map(|n| {
            n.requirements
                .iter()
                .flat_map(|r| &r.scenarios)
                .filter_map(|s| s.anchor.as_ref())
                .map(|a| format!("{}.{a}", n.id))
        })
        .collect();
    let by_status = |wanted: results::Status| -> Vec<String> {
        imported.as_ref().map_or_else(Vec::new, |found| {
            results::scenario_status(&graph, &scan.annotations, found)
                .into_iter()
                .filter(|(s, st)| *st == wanted && in_scope.contains(s))
                .map(|(s, _)| s)
                .collect()
        })
    };
    let passing = by_status(results::Status::Passing);
    let failing = by_status(results::Status::Failing);
    let skipped = by_status(results::Status::Skipped);
    let no_result = by_status(results::Status::NoResult);
    let passing_percent = if result.scenarios == 0 {
        100.0
    } else {
        passing.len() as f64 * 100.0 / result.scenarios as f64
    };
    let head = git::resolve_rev(root, "HEAD").ok().map(|r| r.oid);
    let stale = imported
        .as_ref()
        .is_some_and(|found| found.commit.is_some() && found.commit != head);

    let scenario_percent = if options.require_pass {
        passing_percent
    } else {
        result.verified_percent()
    };
    let below = options
        .fail_under
        .is_some_and(|min| result.implemented_percent() < min || scenario_percent < min);
    let failed =
        below || !result.findings.is_empty() || (options.require_pass && !failing.is_empty());

    if options.json {
        let out = json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "files_scanned": scan.files,
            "annotations": scan.annotations.len(),
            "requirements": result.requirements,
            "implemented": result.implemented,
            "implemented_percent": result.implemented_percent(),
            "unimplemented": result.unimplemented,
            "scenarios": result.scenarios,
            "verified": result.verified,
            "verified_percent": result.verified_percent(),
            "unverified": result.unverified,
            "unanchored_scenarios": result.unanchored_scenarios,
            "findings": result.findings,
            "results": imported.as_ref().map(|found| json!({
                "tests": found.tests.len(),
                "commit": found.commit,
                "stale": stale,
                "passing": passing.len(),
                "passing_percent": passing_percent,
                "failing": failing,
                "skipped": skipped,
                "no_result": no_result,
            })),
            "failed": failed,
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        for f in &result.findings {
            outln!("{}", crate::finding_line(f));
            if let Some(path) = &f.path {
                outln!("  --> {}:{}", path.display(), f.line.unwrap_or(1));
            }
        }
        list("not implemented", &result.unimplemented, options.all);
        list("not verified", &result.unverified, options.all);
        outln!(
            "requirements: {} implemented of {} ({:.0}%)",
            result.implemented,
            result.requirements,
            result.implemented_percent()
        );
        outln!(
            "scenarios:    {} verified of {} ({:.0}%)",
            result.verified,
            result.scenarios,
            result.verified_percent()
        );
        if imported.is_some() {
            list("failing", &failing, options.all);
            list("no matching result", &no_result, options.all);
            outln!(
                "passing:      {} of {} ({:.0}%){}",
                passing.len(),
                result.scenarios,
                passing_percent,
                if skipped.is_empty() {
                    String::new()
                } else {
                    format!(", {} skipped", skipped.len())
                }
            );
            if stale {
                outln!("note: the test results are from another commit than HEAD");
            }
        }
        if result.unanchored_scenarios > 0 {
            outln!(
                "{} scenarios have no {{#anchor}}, so no test can name them",
                result.unanchored_scenarios
            );
        }
        outln!(
            "{} annotations in {} files",
            scan.annotations.len(),
            scan.files
        );
        if below {
            outln!("below the --fail-under threshold");
        }
    }
    Ok(if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn place_of(link: &suspect::Link) -> String {
    match (&link.path, link.line) {
        (Some(path), Some(line)) => format!("{}:{line}", path.display()),
        _ => "(annotation gone)".to_string(),
    }
}

fn count(links: &[suspect::Link], state: State) -> usize {
    links.iter().filter(|l| l.state == state).count()
}

// spec: REQ-SUSPECT-0001
pub fn suspect_command(
    root: &Path,
    all: bool,
    strict: bool,
    json_output: bool,
    cache: bool,
) -> Result<ExitCode> {
    let scan = scan(root)?;
    let graph = index::load(root, cache).0;
    let lock = suspect::load(root)?;
    let links = suspect::links(&graph, &scan.annotations, &lock);
    let suspects = links.iter().filter(|l| l.state.is_suspect()).count();
    let unconfirmed = count(&links, State::Unconfirmed);
    let orphaned = count(&links, State::Orphaned);
    let ok = count(&links, State::Ok);
    let failed = suspects > 0 || (strict && unconfirmed + orphaned > 0);

    if json_output {
        let shown: Vec<&suspect::Link> = links
            .iter()
            .filter(|l| all || l.state != State::Ok)
            .collect();
        let out = json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "ok": ok,
            "suspect": suspects,
            "unconfirmed": unconfirmed,
            "orphaned": orphaned,
            "links": shown,
            "failed": failed,
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(if failed {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        });
    }
    for f in suspect::findings(&links) {
        outln!("{}", crate::finding_line(&f));
        if let Some(path) = &f.path {
            outln!("  --> {}:{}", path.display(), f.line.unwrap_or(1));
        }
    }
    for link in &links {
        let label = match link.state {
            State::Unconfirmed => "unconfirmed",
            State::Orphaned => "orphaned",
            State::Ok if all => "ok",
            _ => continue,
        };
        outln!(
            "{label:<12} {} {} {}",
            link.target,
            place_of(link),
            link.symbol.as_deref().unwrap_or("")
        );
    }
    outln!(
        "{} links: {ok} ok, {suspects} suspect, {unconfirmed} unconfirmed, {orphaned} orphaned",
        links.len() - orphaned
    );
    if suspects > 0 {
        outln!("look at each one, then run `oxispec confirm ID` for those that are still right");
    } else if unconfirmed + orphaned > 0 {
        outln!("`oxispec confirm --all` records the links as they are now");
    }
    Ok(if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

pub fn confirm_command(
    root: &Path,
    ids: &[String],
    all: bool,
    dry_run: bool,
    cache: bool,
) -> Result<()> {
    if ids.is_empty() && !all {
        bail!("say which nodes to confirm, or use --all");
    }
    let scan = scan(root)?;
    let graph = index::load(root, cache).0;
    let mut lock = suspect::load(root)?;
    for given in ids {
        let base = id::base_of(given);
        if graph.get(base).is_none() {
            bail!("no node with id {given}");
        }
    }
    let bases: Vec<String> = ids.iter().map(|i| id::base_of(i).to_string()).collect();
    let filter = (!all).then_some(bases.as_slice());
    let done = suspect::confirm(&graph, &scan.annotations, &mut lock, filter);
    if dry_run {
        outln!(
            "would confirm {} link(s) and drop {} confirmation(s) whose link is gone",
            done.confirmed,
            done.pruned
        );
        return Ok(());
    }
    if done.confirmed + done.pruned > 0 {
        suspect::save(root, &lock)?;
    }
    outln!(
        "confirmed {} link(s), dropped {} confirmation(s) whose link is gone",
        done.confirmed,
        done.pruned
    );
    if done.confirmed + done.pruned > 0 {
        outln!("commit {} to share it", suspect::LOCK_PATH);
    }
    Ok(())
}

/// `oxispec results import`: read test results from files and keep them.
pub fn results_import(root: &Path, files: &[PathBuf], replace: bool) -> Result<()> {
    scan_module(root)?;
    let mut stored = if replace {
        results::Results::default()
    } else {
        results::load(root)?.unwrap_or_default()
    };
    let mut read = 0;
    for file in files {
        let text = std::fs::read_to_string(file)
            .with_context(|| format!("cannot read {}", file.display()))?;
        let found = results::parse(&file.display().to_string(), &text)?;
        read += found.len();
        stored.merge(found);
    }
    stored.commit = git::resolve_rev(root, "HEAD").ok().map(|r| r.oid);
    results::save(root, &stored)?;
    outln!(
        "imported {read} results from {} file(s); now {} tests: {} passed, {} failed, {} skipped",
        files.len(),
        stored.tests.len(),
        stored.count(results::Outcome::Pass),
        stored.count(results::Outcome::Fail),
        stored.count(results::Outcome::Skip)
    );
    Ok(())
}

pub fn results_show(root: &Path, json_output: bool) -> Result<()> {
    scan_module(root)?;
    let Some(stored) = results::load(root)? else {
        bail!("no test results were imported; run `oxispec results import FILE`");
    };
    if json_output {
        let out = json!({"schema_version": OUTPUT_SCHEMA_VERSION, "commit": stored.commit, "tests": stored.tests});
        outln!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    for t in &stored.tests {
        let outcome = match t.outcome {
            results::Outcome::Pass => "pass",
            results::Outcome::Fail => "FAIL",
            results::Outcome::Skip => "skip",
        };
        outln!("{outcome:<5}{}", t.name);
    }
    outln!(
        "{} tests: {} passed, {} failed, {} skipped",
        stored.tests.len(),
        stored.count(results::Outcome::Pass),
        stored.count(results::Outcome::Fail),
        stored.count(results::Outcome::Skip)
    );
    Ok(())
}

pub fn results_clear(root: &Path) -> Result<()> {
    scan_module(root)?;
    results::clear(root)?;
    outln!("removed the imported test results");
    Ok(())
}

fn scan_module(root: &Path) -> Result<()> {
    let cfg = config::load(root)?;
    if !cfg.has("tests") {
        bail!("module `tests` is not enabled; run `oxispec modules add tests`");
    }
    Ok(())
}
