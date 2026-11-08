//! `oxispec docs reference`: the command reference and the check codes,
//! written from the program itself so that they cannot drift from it.

use std::fs;
use std::path::Path;

use anyhow::{Result, bail};
use clap::{Arg, Command};
use oxispec_core::finding::CODES;
use serde_json::Value;

/// Options every command has; they are described once at the top.
const GLOBAL: &[&str] = &["root", "no-cache", "help", "version"];

fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn describe(arg: &Arg) -> String {
    let mut names = Vec::new();
    match (arg.get_short(), arg.get_long()) {
        (Some(s), Some(l)) => names.push(format!("-{s}, --{l}")),
        (None, Some(l)) => names.push(format!("--{l}")),
        (Some(s), None) => names.push(format!("-{s}")),
        (None, None) => names.push(format!("<{}>", arg.get_id().as_str().to_uppercase())),
    }
    let mut head = names.remove(0);
    let is_flag = arg.get_long().is_some() || arg.get_short().is_some();
    if let (true, Some(values)) = (
        is_flag && arg.get_action().takes_values(),
        arg.get_value_names(),
    ) {
        for v in values {
            head.push_str(&format!(" <{v}>"));
        }
    }
    let mut text = format!("- `{head}`");
    if let Some(help) = arg.get_long_help().or(arg.get_help()) {
        let help = flat(&help.to_string());
        text.push_str(&format!(": {help}"));
        if !help.ends_with(['.', '!', '?']) {
            text.push('.');
        }
    }
    let values: Vec<String> = arg
        .get_possible_values()
        .iter()
        .filter(|v| !v.is_hide_set())
        .map(|v| format!("`{}`", v.get_name()))
        .collect();
    if !values.is_empty() && arg.get_long().is_some() {
        text.push_str(&format!(" One of {}.", values.join(", ")));
    }
    let defaults: Vec<String> = arg
        .get_default_values()
        .iter()
        .map(|v| v.to_string_lossy().into_owned())
        .collect();
    if !defaults.is_empty() && arg.get_action().takes_values() {
        text.push_str(&format!(" Default: `{}`.", defaults.join(", ")));
    }
    if arg.is_required_set() && arg.get_long().is_some() {
        text.push_str(" Required.");
    }
    text
}

fn section(cmd: &Command, depth: usize, out: &mut String) {
    let path = cmd
        .get_bin_name()
        .unwrap_or_else(|| cmd.get_name())
        .to_string();
    out.push_str(&format!("\n{} `{path}`\n\n", "#".repeat(depth)));
    if let Some(about) = cmd.get_long_about().or(cmd.get_about()) {
        out.push_str(&format!("{}\n\n", flat(&about.to_string())));
    }
    let mut copy = cmd.clone();
    let usage = copy.render_usage().to_string();
    out.push_str(&format!("```text\n{}\n```\n", usage.trim()));
    let args: Vec<&Arg> = cmd
        .get_arguments()
        .filter(|a| {
            !a.is_hide_set() && !GLOBAL.contains(&a.get_id().as_str().replace('_', "-").as_str())
        })
        .collect();
    if !args.is_empty() {
        out.push('\n');
        for a in args {
            out.push_str(&format!("{}\n", describe(a)));
        }
    }
    let subs: Vec<&Command> = cmd
        .get_subcommands()
        .filter(|c| !c.is_hide_set() && c.get_name() != "help")
        .collect();
    if !subs.is_empty() {
        out.push_str("\nSubcommands: ");
        let names: Vec<String> = subs.iter().map(|c| format!("`{}`", c.get_name())).collect();
        out.push_str(&names.join(", "));
        out.push('\n');
    }
    for c in subs {
        section(c, (depth + 1).min(6), out);
    }
}

pub fn commands(root: &Command) -> String {
    let mut root = root.clone();
    root.build();
    let mut out = String::from(
        "# Command reference\n\n\
         Written by `oxispec docs reference` from the program itself. Do not edit it by hand.\n\n\
         Every command also accepts `--root <ROOT>` (the project folder; default: the nearest parent with \
         `.oxispec/config.yaml`), `--no-cache` (do not read or write the local index) and `--help`.\n\n\
         Commands: ",
    );
    let top: Vec<&Command> = root
        .get_subcommands()
        .filter(|c| !c.is_hide_set() && c.get_name() != "help")
        .collect();
    out.push_str(
        &top.iter()
            .map(|c| format!("[`{}`](#oxispec-{})", c.get_name(), c.get_name()))
            .collect::<Vec<_>>()
            .join(", "),
    );
    out.push('\n');
    for c in top {
        section(c, 2, &mut out);
    }
    out
}

pub fn check_codes() -> String {
    let mut out = String::from(
        "# Check codes\n\n\
         Written by `oxispec docs reference` from the program itself. Do not edit it by hand.\n\n\
         A code is never reused. `E` is an error (`oxispec check` exits with 1), `W` a warning and `I` a note. \
         What each check looks at is described in [the file format](../format.md#check-codes).\n\n\
         | Code | Severity | Meaning |\n|---|---|---|\n",
    );
    for (code, meaning) in CODES {
        let severity = match code.chars().next() {
            Some('E') => "error",
            Some('W') => "warning",
            _ => "info",
        };
        out.push_str(&format!(
            "| {code} | {severity} | {} |\n",
            flat(meaning).replace('|', "\\|")
        ));
    }
    out
}

fn parameter(name: &str, schema: &Value, required: bool) -> String {
    let mut out = format!("- `{name}`");
    let mut facts = Vec::new();
    match schema["type"].as_str() {
        Some("array") => facts.push(format!(
            "array of {}",
            schema["items"]["type"].as_str().unwrap_or("values")
        )),
        Some(t) => facts.push(t.to_string()),
        None => {}
    }
    facts.push(if required { "required" } else { "optional" }.to_string());
    if let Some(d) = schema.get("default") {
        facts.push(format!("default {d}"));
    }
    if let (Some(lo), Some(hi)) = (schema.get("minimum"), schema.get("maximum")) {
        facts.push(format!("{lo} to {hi}"));
    }
    out.push_str(&format!(" ({})", facts.join(", ")));
    if let Some(d) = schema["description"].as_str() {
        out.push_str(&format!(": {}", flat(d)));
        if !d.ends_with(['.', '!', '?']) {
            out.push('.');
        }
    }
    out
}

pub fn mcp_tools() -> String {
    let mut out = String::from(
        "# MCP tools\n\n\
         Written by `oxispec docs reference` from the server itself. Do not edit it by hand.\n\n\
         These are the tools `oxispec mcp` offers when every module is on. A tool whose module is off is not \
         offered. How to connect a client, and how the server behaves, is in [MCP](../mcp.md).\n",
    );
    for (tool, module) in crate::mcp::tool_catalog() {
        let name = tool["name"].as_str().unwrap_or("");
        out.push_str(&format!("\n## `{name}`\n\n"));
        let desc = tool["description"].as_str().unwrap_or("");
        out.push_str(&format!("{}\n\n", flat(desc)));
        let writes = tool["annotations"]["readOnlyHint"] == Value::Bool(false);
        let mut notes = Vec::new();
        if let Some(m) = &module {
            notes.push(format!("Needs the `{m}` module."));
        }
        if writes {
            notes.push(
                "Writes files. Not offered with `--read-only`, or by `oxispec serve`.".to_string(),
            );
        } else {
            notes.push("Read only.".to_string());
        }
        out.push_str(&format!("{}\n", notes.join(" ")));
        let required: Vec<&str> = tool["inputSchema"]["required"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if let Some(props) = tool["inputSchema"]["properties"].as_object() {
            if !props.is_empty() {
                out.push_str("\nArguments:\n\n");
            }
            let mut keys: Vec<&String> = props.keys().collect();
            keys.sort_by_key(|k| (!required.contains(&k.as_str()), k.as_str().to_string()));
            for key in keys {
                let schema = &props[key];
                out.push_str(&format!(
                    "{}\n",
                    parameter(key, schema, required.contains(&key.as_str()))
                ));
            }
        }
    }
    out
}

/// Writes the reference into `dir`, or with `check` compares it with what is there.
// spec: REQ-DOCS-0001
pub fn run(root: &Command, dir: &Path, check: bool) -> Result<()> {
    let files = [
        ("commands.md", commands(root)),
        ("check-codes.md", check_codes()),
        ("mcp-tools.md", mcp_tools()),
    ];
    let mut stale = Vec::new();
    for (name, text) in &files {
        let path = dir.join(name);
        let have = fs::read_to_string(&path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        if have == *text {
            continue;
        }
        if check {
            stale.push(path.display().to_string());
        } else {
            fs::create_dir_all(dir)?;
            fs::write(&path, text)?;
            outln!("wrote {}", path.display());
        }
    }
    if !stale.is_empty() {
        bail!(
            "{} out of date; run `oxispec docs reference --out {}`",
            stale.join(", "),
            dir.display()
        );
    }
    if check {
        outln!("the reference in {} is up to date", dir.display());
    }
    Ok(())
}
