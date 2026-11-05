//! `oxispec docs reference`: the command reference and the check codes,
//! written from the program itself so that they cannot drift from it.

use std::fs;
use std::path::Path;

use anyhow::{Result, bail};
use clap::{Arg, Command};
use oxispec_core::finding::CODES;

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

/// Writes the reference into `dir`, or with `check` compares it with what is there.
pub fn run(root: &Command, dir: &Path, check: bool) -> Result<()> {
    let files = [
        ("commands.md", commands(root)),
        ("check-codes.md", check_codes()),
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
