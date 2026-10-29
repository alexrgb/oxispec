//! `oxispec trace` and `oxispec coverage`: how code and tests tie back to the specs.

use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use oxispec_core::suspect::{self, State};
use oxispec_core::trace::{Annotation, Coverage, Kind, Scan, coverage, scan_tree};
use oxispec_core::{OUTPUT_SCHEMA_VERSION, config, id, index};
use serde_json::json;

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
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            for a in &scan.annotations {
                let kind = match a.kind {
                    Kind::Implements => "spec",
                    Kind::Verifies => "verifies",
                };
                println!("{:<34} {kind:<9} {}", place(a), a.target);
            }
            println!(
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
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    println!(
        "{}  {title}  ({})",
        node.id,
        node.status.as_deref().unwrap_or("no status")
    );
    println!("implemented by:");
    if implementers.is_empty() {
        println!("  nothing yet");
    }
    for a in implementers {
        println!("  {}", place(a));
    }
    println!("scenarios:");
    if scenarios.is_empty() {
        println!("  none");
    }
    for (anchor, name, tests) in scenarios {
        let label = anchor.unwrap_or_else(|| "(no anchor)".to_string());
        if tests.is_empty() {
            println!("  {label:<12} {name}: NOT VERIFIED");
        } else {
            let places: Vec<String> = tests.iter().map(|a| place(a)).collect();
            println!("  {label:<12} {name}: verified by {}", places.join(", "));
        }
    }
    Ok(())
}

pub struct CoverageOptions {
    pub status: Option<String>,
    pub fail_under: Option<f64>,
    pub all: bool,
    pub json: bool,
    pub cache: bool,
}

fn list(title: &str, items: &[String], all: bool) {
    if items.is_empty() {
        return;
    }
    println!("{title}:");
    let shown = if all {
        items.len()
    } else {
        items.len().min(20)
    };
    for item in &items[..shown] {
        println!("  {item}");
    }
    if shown < items.len() {
        println!(
            "  ... and {} more (--all shows everything)",
            items.len() - shown
        );
    }
}

pub fn coverage_command(root: &Path, options: &CoverageOptions) -> Result<ExitCode> {
    let scan = scan(root)?;
    let graph = index::load(root, options.cache).0;
    let result: Coverage = coverage(&graph, &scan.annotations, options.status.as_deref());
    let below = options
        .fail_under
        .is_some_and(|min| result.implemented_percent() < min || result.verified_percent() < min);
    let failed = below || !result.findings.is_empty();

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
            "failed": failed,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        for f in &result.findings {
            println!("{}", crate::finding_line(f));
            if let Some(path) = &f.path {
                println!("  --> {}:{}", path.display(), f.line.unwrap_or(1));
            }
        }
        list("not implemented", &result.unimplemented, options.all);
        list("not verified", &result.unverified, options.all);
        println!(
            "requirements: {} implemented of {} ({:.0}%)",
            result.implemented,
            result.requirements,
            result.implemented_percent()
        );
        println!(
            "scenarios:    {} verified of {} ({:.0}%)",
            result.verified,
            result.scenarios,
            result.verified_percent()
        );
        if result.unanchored_scenarios > 0 {
            println!(
                "{} scenarios have no {{#anchor}}, so no test can name them",
                result.unanchored_scenarios
            );
        }
        println!(
            "{} annotations in {} files",
            scan.annotations.len(),
            scan.files
        );
        if below {
            println!("below the --fail-under threshold");
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
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(if failed {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        });
    }
    for f in suspect::findings(&links) {
        println!("{}", crate::finding_line(&f));
        if let Some(path) = &f.path {
            println!("  --> {}:{}", path.display(), f.line.unwrap_or(1));
        }
    }
    for link in &links {
        let label = match link.state {
            State::Unconfirmed => "unconfirmed",
            State::Orphaned => "orphaned",
            State::Ok if all => "ok",
            _ => continue,
        };
        println!(
            "{label:<12} {} {} {}",
            link.target,
            place_of(link),
            link.symbol.as_deref().unwrap_or("")
        );
    }
    println!(
        "{} links: {ok} ok, {suspects} suspect, {unconfirmed} unconfirmed, {orphaned} orphaned",
        links.len() - orphaned
    );
    if suspects > 0 {
        println!("look at each one, then run `oxispec confirm ID` for those that are still right");
    } else if unconfirmed + orphaned > 0 {
        println!("`oxispec confirm --all` records the links as they are now");
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
        println!(
            "would confirm {} link(s) and drop {} confirmation(s) whose link is gone",
            done.confirmed, done.pruned
        );
        return Ok(());
    }
    if done.confirmed + done.pruned > 0 {
        suspect::save(root, &lock)?;
    }
    println!(
        "confirmed {} link(s), dropped {} confirmation(s) whose link is gone",
        done.confirmed, done.pruned
    );
    if done.confirmed + done.pruned > 0 {
        println!("commit {} to share it", suspect::LOCK_PATH);
    }
    Ok(())
}
