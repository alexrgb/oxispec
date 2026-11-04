//! `oxispec compliance`: how the project stands against its compliance rules.

use std::path::Path;
use std::process::ExitCode;

use anyhow::Result;
use oxispec_core::OUTPUT_SCHEMA_VERSION;
use oxispec_core::compliance::{self, STANDARDS};
use oxispec_core::config;
use serde_json::json;

use crate::load_graph;

pub fn report(root: &Path, json: bool, cache: bool) -> Result<ExitCode> {
    let cfg = config::load(root)?;
    let (rules, problems) = compliance::resolve(&cfg.compliance);
    let graph = load_graph(root, cache);
    let reports = compliance::report(&graph);
    let gaps: usize = reports.iter().map(|r| r.gaps.len()).sum();
    let failed = gaps > 0 || !problems.is_empty();
    if json {
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "standard": cfg.compliance.standard,
                "level": cfg.compliance.level,
                "problems": problems,
                "rules": reports,
                "gaps": gaps,
                "met": !failed,
            }))?
        );
    } else {
        for p in &problems {
            outln!("config: {p}");
        }
        if rules.is_empty() && problems.is_empty() {
            outln!("no compliance rules; set `compliance:` in .oxispec/config.yaml");
            outln!("see `oxispec compliance standards`");
        }
        if let (Some(s), Some(l)) = (&cfg.compliance.standard, &cfg.compliance.level) {
            outln!("{s}, level {l}");
        }
        for r in &reports {
            outln!(
                "{} {:<24} {}/{} met. {}",
                if r.gaps.is_empty() { "ok " } else { "GAP" },
                r.rule.id,
                r.met,
                r.applies,
                r.statement
            );
            for g in &r.gaps {
                outln!("      {} ({}): {}", g.id, g.path.display(), g.problem);
            }
        }
    }
    Ok(if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

pub fn standards(json: bool) -> Result<()> {
    if json {
        let rows: Vec<_> = STANDARDS
            .iter()
            .map(|s| {
                let levels: Vec<_> = s
                    .levels
                    .iter()
                    .map(|(name, tier)| {
                        json!({"level": name, "tier": tier, "rules": compliance::tier_rules(*tier)})
                    })
                    .collect();
                json!({"name": s.name, "title": s.title, "levels": levels})
            })
            .collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"schema_version": OUTPUT_SCHEMA_VERSION, "standards": rows})
            )?
        );
        return Ok(());
    }
    for s in STANDARDS {
        outln!("{}  {}", s.name, s.title);
        for (level, tier) in s.levels {
            let ids: Vec<_> = compliance::tier_rules(*tier)
                .into_iter()
                .map(|r| r.id)
                .collect();
            outln!(
                "    level {level}: {}",
                if ids.is_empty() {
                    "no rules".to_string()
                } else {
                    ids.join(", ")
                }
            );
        }
    }
    Ok(())
}
