//! `oxispec formats`: the format profiles a project can use.

use std::path::Path;

use anyhow::{Result, bail};
use oxispec_core::formats::{self, Profile};
use oxispec_core::{OUTPUT_SCHEMA_VERSION, config};
use serde_json::json;

fn describe(profile: &Profile, chosen: bool) -> serde_json::Value {
    let mut value = serde_json::to_value(profile).unwrap_or_default();
    value["chosen"] = chosen.into();
    value
}

// spec: REQ-FORMATS-0001
pub fn list(root: &Path, json: bool) -> Result<()> {
    let cfg = config::load(root)?;
    let (profiles, problems) = formats::available(&cfg);
    let chosen = |p: &Profile| cfg.formats.get(&p.ty) == Some(&p.name);
    if json {
        let items: Vec<_> = profiles.iter().map(|p| describe(p, chosen(p))).collect();
        let problems: Vec<_> = problems.iter().map(|p| p.message.clone()).collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "profiles": items,
                "problems": problems,
            }))?
        );
        return Ok(());
    }
    for p in &profiles {
        outln!(
            "{:<18} {:<14} {:<9} {}{}",
            p.name,
            p.ty,
            if p.builtin { "built-in" } else { "project" },
            p.description,
            if chosen(p) { "  (in use)" } else { "" }
        );
    }
    for problem in &problems {
        eprintln!("{}", problem.message);
    }
    Ok(())
}

pub fn show(root: &Path, name: &str, json: bool) -> Result<()> {
    let cfg = config::load(root)?;
    let (profiles, _) = formats::available(&cfg);
    let Some(profile) = profiles.iter().find(|p| p.name == name) else {
        let known: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
        bail!("no profile named `{name}`; there are {}", known.join(", "));
    };
    let chosen = cfg.formats.get(&profile.ty) == Some(&profile.name);
    if json {
        let mut value = describe(profile, chosen);
        value["schema_version"] = OUTPUT_SCHEMA_VERSION.into();
        value["template"] = profile.template().into();
        outln!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    outln!("{} (for {} nodes)", profile.name, profile.ty);
    outln!("{}", profile.description);
    for s in &profile.sections {
        outln!(
            "  section {}{}",
            s.name,
            if s.required { " (required)" } else { "" }
        );
    }
    if let Some(style) = profile.style {
        outln!("  style: {}", style.name());
    }
    if !profile.fields.is_empty() {
        outln!("  required fields: {}", profile.fields.join(", "));
    }
    if !profile.statuses.is_empty() {
        outln!("  statuses: {}", profile.statuses.join(", "));
    }
    outln!("\ntemplate:\n{}", profile.template());
    Ok(())
}
