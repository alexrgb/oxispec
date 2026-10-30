//! `oxispec mcp`: a Model Context Protocol server on stdin and stdout.
//!
//! Messages are JSON-RPC 2.0, one per line. Reading tools call the same code as the
//! `--json` commands, so an agent and a person see the same answers. The server can
//! propose a change set. It cannot approve one.

use std::io::{BufRead, Write};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use oxispec_core::config::Config;
use oxispec_core::{Graph, config, id, index};
use serde_json::{Value, json};

use crate::{context, git};

/// Protocol versions this server speaks, newest first. All use the same stdio framing.
const VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "Specs, decisions and tests for this project, as a graph of linked nodes. \
Start with spec_search or spec_context, read nodes with spec_get, and run spec_check before you finish. spec_check also reports requirements whose claims contradict each other. \
spec_propose_change writes a draft change set. Only a person approves it. Quote node IDs exactly as returned.";

struct Server<'a> {
    root: &'a Path,
    cfg: Config,
    read_only: bool,
}

pub fn serve(root: &Path, read_only: bool) -> Result<()> {
    let cfg = config::load(root)?;
    let server = Server {
        root,
        cfg,
        read_only,
    };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.context("cannot read from stdin")?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(message) => server.handle(&message),
            Err(e) => Some(error(Value::Null, -32700, &format!("parse error: {e}"))),
        };
        if let Some(reply) = reply {
            writeln!(stdout, "{reply}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

fn result(id: &Value, value: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": value})
}

fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn tool(
    name: &str,
    description: &str,
    properties: Value,
    required: &[&str],
    writes: bool,
) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {"type": "object", "properties": properties, "required": required},
        "annotations": {"readOnlyHint": !writes, "destructiveHint": false, "openWorldHint": false},
    })
}

impl Server<'_> {
    fn handle(&self, message: &Value) -> Option<Value> {
        let id = message.get("id")?.clone();
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            // A response to something we never asked.
            return None;
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        Some(match method {
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(Value::as_str);
                let version = VERSIONS
                    .iter()
                    .find(|v| Some(**v) == asked)
                    .unwrap_or(&VERSIONS[0]);
                result(
                    &id,
                    json!({
                        "protocolVersion": version,
                        "capabilities": {"tools": {"listChanged": false}},
                        "serverInfo": {"name": "oxispec", "version": env!("CARGO_PKG_VERSION")},
                        "instructions": INSTRUCTIONS,
                    }),
                )
            }
            "ping" => result(&id, json!({})),
            "tools/list" => result(&id, json!({"tools": self.tools()})),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                if !self.tools().iter().any(|t| t["name"] == name) {
                    return Some(error(id, -32602, &format!("unknown tool `{name}`")));
                }
                let (text, is_error) = match self.call(name, &args) {
                    Ok(text) => (text, false),
                    Err(e) => (format!("{e:#}"), true),
                };
                result(
                    &id,
                    json!({"content": [{"type": "text", "text": text}], "isError": is_error}),
                )
            }
            _ => error(id, -32601, &format!("method not found: {method}")),
        })
    }

    fn tools(&self) -> Vec<Value> {
        let rev = json!({"type": "string", "description": "A git revision: a branch, tag, hash or date:YYYY-MM-DD. Default: the working tree."});
        let id_property = json!({"type": "string", "description": "A node ID such as REQ-AUTH-0001, or a scenario such as REQ-AUTH-0001.S1"});
        let mut tools = vec![
            tool(
                "spec_search",
                "Full-text search of every node. Returns IDs, titles and snippets, best first.",
                json!({
                    "query": {"type": "string", "description": "Words to find. All must match. Quotes make a phrase."},
                    "type": {"type": "string", "description": "Only this node type, for example requirement or adr"},
                    "status": {"type": "string", "description": "Only this status, for example accepted"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 50, "default": 10},
                }),
                &["query"],
                false,
            ),
            tool(
                "spec_get",
                "One node: its text, its links, and the nodes that link to it.",
                json!({"id": id_property, "at": rev}),
                &["id"],
                false,
            ),
            tool(
                "spec_neighbors",
                "Nodes within a number of link hops of a node, in both directions, nearest first.",
                json!({
                    "id": id_property,
                    "depth": {"type": "integer", "minimum": 1, "maximum": 4, "default": 1},
                    "kinds": {"type": "array", "items": {"type": "string"}, "description": "Only follow these link kinds, for example decided-by"},
                }),
                &["id"],
                false,
            ),
            tool(
                "spec_context",
                "The text of the given nodes and what is near them, nearest first, sized to a token budget. What does not fit is listed by ID.",
                json!({
                    "ids": {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": 20},
                    "budget": {"type": "integer", "minimum": 500, "maximum": 100000, "default": 8000, "description": "Tokens, counted as one per four characters"},
                    "depth": {"type": "integer", "minimum": 0, "maximum": 4, "default": 2},
                    "at": rev,
                }),
                &["ids"],
                false,
            ),
            tool(
                "spec_check",
                "Run the structural checks. Returns every finding with its code, file and message.",
                json!({"at": rev}),
                &[],
                false,
            ),
            tool(
                "spec_history",
                "How a node changed over time: commits, authors and dates.",
                json!({"id": id_property, "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 20}}),
                &["id"],
                false,
            ),
            tool(
                "spec_diff",
                "How the specs differ between two revisions: nodes added, removed and changed, and findings that appeared or went away.",
                json!({
                    "from": {"type": "string", "description": "A git revision"},
                    "to": {"type": "string", "description": "A git revision. Default: the working tree."},
                }),
                &["from"],
                false,
            ),
        ];
        if self.cfg.has("tests") {
            tools.push(tool(
                "spec_trace",
                "Where a node is implemented and tested, from spec: and verifies: comments. Without an ID, every annotation.",
                json!({"id": id_property}),
                &[],
                false,
            ));
        }
        if self.cfg.has("consistency") {
            tools.push(tool(
                "spec_claims",
                "The vocabulary and the claims on each variable, with the share of requirements that have claims. Conflicts between claims come from spec_check.",
                json!({"variable": {"type": "string", "description": "Only this variable, for example session.idle_timeout"}}),
                &[],
                false,
            ));
        }
        if self.cfg.has("statemachines") {
            tools.push(tool(
                "spec_machine",
                "State machines. Without an ID, every machine. With an ID, its transitions and each property with a counterexample path if it is violated, or a Mermaid diagram or one test per transition. Problems come from spec_check.",
                json!({
                    "id": {"type": "string", "description": "A state machine ID such as SM-ORDER"},
                    "view": {"type": "string", "enum": ["summary", "diagram", "tests"], "description": "What to return for an ID. Default: summary."},
                }),
                &[],
                false,
            ));
        }
        if self.cfg.has("specs") && !self.read_only {
            tools.push(tool(
                "spec_propose_change",
                "Write a draft change set that proposes a change to the specs, linked to the nodes it changes. It is not approved: a person reviews it. Edit the nodes themselves in the files.",
                json!({
                    "title": {"type": "string", "description": "A short title, one line"},
                    "why": {"type": "string", "description": "Why the change is needed"},
                    "changes": {"type": "array", "items": {"type": "string"}, "description": "IDs of the nodes this change modifies"},
                }),
                &["title"],
                true,
            ));
        }
        tools
    }

    fn call(&self, name: &str, args: &Value) -> Result<String> {
        match name {
            "spec_search" => {
                let query = text_arg(args, "query")?;
                let limit = number_arg(args, "limit", 10, 1, 50)?.to_string();
                let mut cmd = vec!["search", "--json", "--limit", &limit];
                let (ty, status);
                if let Some(t) = optional_word(args, "type")? {
                    ty = t;
                    cmd.extend(["--type", &ty]);
                }
                if let Some(s) = optional_word(args, "status")? {
                    status = s;
                    cmd.extend(["--status", &status]);
                }
                cmd.extend(["--", &query]);
                self.cli(&cmd)
            }
            "spec_get" => self.get(&node_id(args, "id")?, optional_rev(args, "at")?.as_deref()),
            "spec_neighbors" => self.neighbors(args),
            "spec_context" => {
                let ids = id_list(args, "ids", 20)?;
                let budget = number_arg(args, "budget", 8000, 500, 100_000)? as usize;
                let depth = number_arg(args, "depth", 2, 0, 4)?;
                let at = optional_rev(args, "at")?;
                let pack = context::build(self.root, &ids, depth, budget, at.as_deref(), true)?;
                Ok(context::markdown(&pack))
            }
            "spec_check" => {
                let mut cmd = vec!["check", "--json"];
                let at = optional_rev(args, "at")?;
                if let Some(at) = &at {
                    cmd.extend(["--at", at]);
                }
                self.cli(&cmd)
            }
            "spec_history" => {
                let node = node_id(args, "id")?;
                let limit = number_arg(args, "limit", 20, 1, 100)?.to_string();
                self.cli(&["history", "--json", "--limit", &limit, &node])
            }
            "spec_diff" => {
                let from = required_rev(args, "from")?;
                let mut cmd = vec!["diff", "--json", &from];
                let to = optional_rev(args, "to")?;
                if let Some(to) = &to {
                    cmd.push(to);
                }
                self.cli(&cmd)
            }
            "spec_trace" => {
                let mut cmd = vec!["trace", "--json"];
                let node = optional_id(args, "id")?;
                if let Some(node) = &node {
                    cmd.push(node);
                }
                self.cli(&cmd)
            }
            "spec_claims" => {
                let mut cmd = vec!["claims", "--json"];
                let variable = optional_variable(args, "variable")?;
                if let Some(variable) = &variable {
                    cmd.extend(["--var", variable]);
                }
                self.cli(&cmd)
            }
            "spec_machine" => {
                let mut cmd = vec!["machine"];
                let node = optional_id(args, "id")?;
                if let Some(node) = &node {
                    cmd.push(node);
                }
                match optional_word(args, "view")?.as_deref() {
                    None | Some("summary") => cmd.push("--json"),
                    Some("diagram") if node.is_some() => cmd.push("--diagram"),
                    Some("tests") if node.is_some() => cmd.extend(["--tests", "--json"]),
                    Some("diagram" | "tests") => anyhow::bail!("`view` needs an `id`"),
                    Some(other) => {
                        anyhow::bail!("`view` is summary, diagram or tests, not `{other}`")
                    }
                }
                self.cli(&cmd)
            }
            "spec_propose_change" => self.propose(args),
            _ => anyhow::bail!("unknown tool `{name}`"),
        }
    }

    /// Runs this same program with `--json`. Exit code 1 means findings, which is an answer.
    fn cli(&self, args: &[&str]) -> Result<String> {
        let exe = std::env::current_exe().context("cannot find the oxispec program")?;
        let out = Command::new(exe)
            .arg("--root")
            .arg(self.root)
            .args(args)
            .output()
            .context("cannot run oxispec")?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        match out.status.code() {
            Some(0 | 1) if !stdout.trim().is_empty() => Ok(stdout),
            _ => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                anyhow::bail!("{}", stderr.trim().trim_start_matches("error: "))
            }
        }
    }

    fn graph(&self, at: Option<&str>) -> Result<Graph> {
        match at {
            Some(spec) => {
                let rev = git::resolve_rev(self.root, spec)?;
                git::graph_at(self.root, &rev)
            }
            None => Ok(index::load(self.root, true).0),
        }
    }

    fn get(&self, node_id: &str, at: Option<&str>) -> Result<String> {
        let graph = self.graph(at)?;
        let node = graph
            .get(node_id)
            .with_context(|| format!("no node with id {node_id}"))?;
        let text = match at {
            Some(spec) => {
                let rev = git::resolve_rev(self.root, spec)?;
                git::file_at(self.root, &rev.oid, &node.path)?
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
            }
            None => std::fs::read_to_string(self.root.join(&node.path)).ok(),
        };
        let incoming: Vec<Value> = graph
            .incoming(&node.id)
            .iter()
            .map(|(n, kind)| json!({"id": n.id, "kind": kind, "type": n.ty, "title": n.title}))
            .collect();
        Ok(serde_json::to_string_pretty(&json!({
            "node": node,
            "incoming": incoming,
            "text": text,
        }))?)
    }

    fn neighbors(&self, args: &Value) -> Result<String> {
        let node = node_id(args, "id")?;
        let depth = number_arg(args, "depth", 1, 1, 4)?;
        let kinds: Option<Vec<String>> = match args.get("kinds") {
            None | Some(Value::Null) => None,
            Some(v) => Some(
                v.as_array()
                    .context("`kinds` is a list of strings")?
                    .iter()
                    .map(|k| {
                        k.as_str()
                            .map(str::to_string)
                            .context("`kinds` is a list of strings")
                    })
                    .collect::<Result<_>>()?,
            ),
        };
        let graph = self.graph(None)?;
        if graph.get(&node).is_none() {
            anyhow::bail!("no node with id {node}");
        }
        let found = oxispec_core::context::neighbors(&graph, &node, depth, kinds.as_deref(), 200);
        let rows: Vec<Value> = found
            .iter()
            .map(|n| {
                let other = graph.get(&n.id);
                json!({
                    "id": n.id,
                    "type": other.map(|o| o.ty.as_str()),
                    "title": other.and_then(|o| o.title.as_deref()),
                    "status": other.and_then(|o| o.status.as_deref()),
                    "distance": n.distance,
                    "link": n.via,
                    "from": n.from,
                    "incoming": n.incoming,
                })
            })
            .collect();
        Ok(serde_json::to_string_pretty(&json!({"neighbors": rows}))?)
    }

    fn propose(&self, args: &Value) -> Result<String> {
        if self.read_only {
            anyhow::bail!("the server is read-only");
        }
        let title = text_arg(args, "title")?;
        if title.contains('\n') || title.len() > 200 || title.starts_with('-') {
            anyhow::bail!(
                "the title is one line of up to 200 characters and does not start with `-`"
            );
        }
        let why = match args.get("why") {
            None | Some(Value::Null) => None,
            Some(v) => Some(v.as_str().context("`why` is text")?.trim().to_string()),
        };
        let targets = if args.get("changes").is_some_and(|v| !v.is_null()) {
            id_list(args, "changes", 50)?
        } else {
            Vec::new()
        };
        // Check everything before writing anything.
        let graph = self.graph(None)?;
        for target in &targets {
            if graph.get(target).is_none() {
                anyhow::bail!("no node with id {target}, so nothing was written");
            }
        }
        let created = self.cli(&["new", "change", &title])?;
        let words: Vec<&str> = created.split_whitespace().collect();
        let change_id = words.get(1).context("cannot read the new ID")?.to_string();
        let path = created
            .split_once('(')
            .and_then(|(_, rest)| rest.rsplit_once(')'))
            .map(|(p, _)| p.to_string())
            .context("cannot read the new path")?;
        if let Some(why) = why.filter(|w| !w.is_empty()) {
            let file = self.root.join(&path);
            let text = std::fs::read_to_string(&file)?;
            let text = text.replacen("## Why\n", &format!("## Why\n\n{why}\n"), 1);
            std::fs::write(&file, text)?;
        }
        let mut linked = Vec::new();
        for target in &targets {
            self.cli(&["link", &change_id, "changes", target])
                .with_context(|| format!("{change_id} was created, but linking {target} failed"))?;
            linked.push(target.clone());
        }
        Ok(serde_json::to_string_pretty(&json!({
            "id": change_id,
            "path": path,
            "status": "draft",
            "changes": linked,
            "next": "Edit the nodes it changes, run spec_check, then ask a person to review and approve the change set.",
        }))?)
    }
}

fn text_arg(args: &Value, name: &str) -> Result<String> {
    let value = args
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .with_context(|| format!("`{name}` is required and is text"))?;
    Ok(value.to_string())
}

fn number_arg(args: &Value, name: &str, default: u32, min: u32, max: u32) -> Result<u32> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => {
            let n = v
                .as_u64()
                .with_context(|| format!("`{name}` is a whole number"))?;
            if n < u64::from(min) || n > u64::from(max) {
                anyhow::bail!("`{name}` is between {min} and {max}");
            }
            Ok(n as u32)
        }
    }
}

/// A lowercase word such as a node type or a status.
fn optional_word(args: &Value, name: &str) -> Result<Option<String>> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let word = v.as_str().with_context(|| format!("`{name}` is text"))?;
            if word.is_empty() || !word.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
                anyhow::bail!("`{name}` is lowercase letters and dashes");
            }
            Ok(Some(word.to_string()))
        }
    }
}

/// A vocabulary variable such as `session.idle_timeout`. It cannot start with a dash.
fn optional_variable(args: &Value, name: &str) -> Result<Option<String>> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let text = v.as_str().with_context(|| format!("`{name}` is text"))?;
            let ok = text
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && text.len() <= 100
                && text
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.'));
            if !ok {
                anyhow::bail!("`{name}` is a variable name such as session.idle_timeout");
            }
            Ok(Some(text.to_string()))
        }
    }
}

fn valid_id(value: &str) -> Result<String> {
    if id::parse(value).is_none() {
        anyhow::bail!("`{value}` is not a node ID");
    }
    Ok(value.to_string())
}

fn node_id(args: &Value, name: &str) -> Result<String> {
    valid_id(&text_arg(args, name)?)
}

fn optional_id(args: &Value, name: &str) -> Result<Option<String>> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => node_id(args, name).map(Some),
    }
}

fn id_list(args: &Value, name: &str, max: usize) -> Result<Vec<String>> {
    let list = args
        .get(name)
        .and_then(Value::as_array)
        .with_context(|| format!("`{name}` is a list of node IDs"))?;
    if list.is_empty() || list.len() > max {
        anyhow::bail!("`{name}` has between 1 and {max} IDs");
    }
    list.iter()
        .map(|v| {
            v.as_str()
                .with_context(|| format!("`{name}` is a list of node IDs"))
                .and_then(valid_id)
        })
        .collect()
}

/// A revision never starts with `-`, so it cannot be read as an option by git.
fn check_rev(rev: &str) -> Result<String> {
    if rev.is_empty() || rev.starts_with('-') || rev.contains(['\n', '\0']) {
        anyhow::bail!("`{rev}` is not a revision");
    }
    Ok(rev.to_string())
}

fn required_rev(args: &Value, name: &str) -> Result<String> {
    check_rev(&text_arg(args, name)?)
}

fn optional_rev(args: &Value, name: &str) -> Result<Option<String>> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => required_rev(args, name).map(Some),
    }
}
