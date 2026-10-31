//! `oxispec machine`: state machines, their properties, a diagram and generated tests.

use std::path::Path;
use std::process::ExitCode;

use anyhow::{Result, bail};
use oxispec_core::machines::{self, Counterexample, Machine, Step};
use oxispec_core::{Graph, OUTPUT_SCHEMA_VERSION, config, index};

use crate::git;

fn steps_json(steps: &[Step]) -> Vec<serde_json::Value> {
    steps
        .iter()
        .map(|s| serde_json::json!({"from": s.from, "event": s.event, "to": s.to}))
        .collect()
}

fn example_text(machine: &Machine, example: &Counterexample) -> String {
    let path = machines::render_path(&machine.initial, &example.steps);
    if example.repeats {
        format!("{path}, and the last step can repeat forever")
    } else {
        path
    }
}

pub fn machine_command(
    root: &Path,
    id: Option<&str>,
    diagram: bool,
    tests: bool,
    at: Option<&str>,
    json: bool,
    cache: bool,
) -> Result<ExitCode> {
    let cfg = config::load(root)?;
    if !cfg.has("statemachines") {
        bail!("module `statemachines` is not enabled; run `oxispec modules add statemachines`");
    }
    if (diagram || tests) && id.is_none() {
        bail!("`--diagram` and `--tests` need the ID of a state machine");
    }
    let revision = at.map(|spec| git::resolve_rev(root, spec)).transpose()?;
    let graph: Graph = match &revision {
        Some(rev) => git::graph_at(root, rev)?,
        None => index::load(root, cache).0,
    };
    let analysis = machines::analyze(&graph);
    let shown: Vec<&Machine> = analysis
        .machines
        .iter()
        .filter(|m| id.is_none_or(|id| m.node == id))
        .collect();
    if let Some(id) = id
        && shown.is_empty()
    {
        bail!("no state machine `{id}` (`oxispec machine` lists them)");
    }

    if diagram && let [machine] = shown.as_slice() {
        print!("{}", machines::diagram(machine));
        return Ok(ExitCode::SUCCESS);
    }
    if tests && let [machine] = shown.as_slice() {
        let cases = machines::tests(machine);
        if json {
            let list: Vec<_> = cases
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "name": c.name,
                        "steps": steps_json(&c.steps),
                        "implements": c.implements,
                    })
                })
                .collect();
            let out = serde_json::json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "machine": machine.node,
                "tests": list,
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            for case in &cases {
                println!("{}", case.name);
                println!("  {}", machines::render_path(&machine.initial, &case.steps));
                if !case.implements.is_empty() {
                    println!("  covers {}", case.implements.join(", "));
                }
            }
            println!(
                "{} tests, one for each transition that can be reached",
                cases.len()
            );
        }
        return Ok(ExitCode::SUCCESS);
    }

    let results: Vec<Vec<Option<Counterexample>>> = shown
        .iter()
        .map(|m| {
            m.properties
                .iter()
                .map(|p| machines::violation(m, &p.rule))
                .collect()
        })
        .collect();

    if json {
        let list: Vec<_> = shown
            .iter()
            .zip(&results)
            .map(|(m, found)| {
                serde_json::json!({
                    "id": m.node,
                    "path": m.path,
                    "active": m.active,
                    "initial": m.initial,
                    "states": m.states,
                    "final": m.finals,
                    "transitions": m.transitions.iter().map(|t| serde_json::json!({
                        "from": t.from,
                        "event": t.event,
                        "to": t.to,
                        "guard": t.guard,
                        "implements": t.implements,
                    })).collect::<Vec<_>>(),
                    "properties": m.properties.iter().zip(found).map(|(p, v)| serde_json::json!({
                        "text": p.text,
                        "from": p.source,
                        "holds": v.is_none(),
                        "counterexample": v.as_ref().map(|c| serde_json::json!({
                            "steps": steps_json(&c.steps),
                            "repeats": c.repeats,
                        })),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let out = serde_json::json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "at": revision.as_ref().map(crate::revision_json),
            "machines": list,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(ExitCode::SUCCESS);
    }

    if let Some(rev) = &revision {
        println!("reading {rev}");
    }
    if shown.is_empty() {
        println!(
            "no state machines yet: write a `statemachine` block (`oxispec new machine \"Name\"`)"
        );
        return Ok(ExitCode::SUCCESS);
    }
    let single = id.is_some();
    for (machine, found) in shown.iter().zip(&results) {
        let broken = found.iter().flatten().count();
        if !single {
            println!(
                "{:<14} {} states, {} transitions, {} properties{}",
                machine.node,
                machine.states.len(),
                machine.transitions.len(),
                machine.properties.len(),
                if broken > 0 {
                    format!(" ({broken} violated)")
                } else {
                    String::new()
                }
            );
            continue;
        }
        println!("{} ({})", machine.node, machine.path.display());
        println!(
            "  starts in {}; final: {}",
            machine.initial,
            if machine.finals.is_empty() {
                "none".to_string()
            } else {
                machine.finals.join(", ")
            }
        );
        for t in &machine.transitions {
            let guard = t
                .guard
                .as_ref()
                .map_or(String::new(), |g| format!("  [{g}]"));
            println!("  {} -{}-> {}{guard}", t.from, t.event, t.to);
        }
        for (p, v) in machine.properties.iter().zip(found) {
            match v {
                None => println!("  holds     {}", p.text),
                Some(c) => println!("  violated  {}: {}", p.text, example_text(machine, c)),
            }
        }
    }
    if !single {
        println!("`oxispec machine ID` shows one, `oxispec check` reports problems");
    }
    Ok(ExitCode::SUCCESS)
}
