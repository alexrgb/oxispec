//! `oxispec mcp`: a Model Context Protocol server on stdin and stdout.
//!
//! Messages are JSON-RPC 2.0, one per line. Reading tools call the same code as the
//! `--json` commands, so an agent and a person see the same answers. The server can
//! offer nodes as resources and a few prompts, and propose a change set. It cannot approve one.

use std::io::{BufRead, Write};
use std::path::Path;

use anyhow::{Context, Result};
use oxispec_core::access::{Policy, REDACTED};
use oxispec_core::config::Config;
use oxispec_core::{Graph, config, id, index};
use serde_json::{Value, json};

use crate::{context, git};

/// Protocol versions this server speaks, newest first. All use the same stdio framing.
const VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "Specs, decisions and tests for this project, as a graph of linked nodes. \
Start with spec_search or spec_context, read nodes with spec_get, and run spec_check before you finish. spec_check also reports requirements whose claims contradict each other. \
spec_propose_change writes a draft change set. Only a person approves it. Quote node IDs exactly as returned.";

/// Resource URIs for nodes: `oxispec://node/REQ-AUTH-0001`.
const NODE_URI: &str = "oxispec://node/";
const RESOURCE_PAGE: usize = 500;

/// Prompts: a name, what it does, and the text sent with the node's ID filled in.
const PROMPTS: &[(&str, &str, &str)] = &[
    (
        "review-requirement",
        "Review a requirement for gaps, ambiguity and conflicts",
        "Review the requirement {id}. Call spec_context for {id}, read what it links to, and run spec_check. \
Report: wording that is ambiguous or cannot be tested, scenarios that are missing, conflicts with other requirements or decisions, and links that look wrong. Quote node IDs exactly. Do not edit anything.",
    ),
    (
        "implement-requirement",
        "Plan and write the code and tests for a requirement",
        "Implement the requirement {id}. Call spec_context for {id} and spec_trace for {id} to see what exists. \
Write the code with a `spec: {id}` comment and a test with a `verifies: {id}` comment for each scenario, then run spec_check. If the spec is wrong or unclear, say so and propose the change with spec_propose_change instead of working around it.",
    ),
    (
        "assess-impact",
        "Say what a change to a node would affect",
        "Assess the impact of changing {id}. Call spec_neighbors for {id} with depth 3, and spec_trace for each requirement you find. \
List the nodes, code and tests that would need to change, and what could break. Do not edit anything.",
    ),
];

fn prompt_list() -> Vec<Value> {
    PROMPTS
        .iter()
        .map(|(name, description, _)| {
            json!({
                "name": name,
                "description": description,
                "arguments": [{"name": "id", "description": "A node ID such as REQ-AUTH-0001", "required": true}],
            })
        })
        .collect()
}

fn prompt_get(params: &Value) -> Result<Value> {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let (_, description, template) = PROMPTS
        .iter()
        .find(|(n, _, _)| *n == name)
        .with_context(|| format!("unknown prompt `{name}`"))?;
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let node = node_id(&args, "id")?;
    Ok(json!({
        "description": description,
        "messages": [{
            "role": "user",
            "content": {"type": "text", "text": template.replace("{id}", &node)},
        }],
    }))
}

pub(crate) struct Server<'a> {
    root: &'a Path,
    cfg: Config,
    read_only: bool,
}

impl<'a> Server<'a> {
    /// A server for the project at `root`. It keeps the graph in memory between calls.
    pub(crate) fn new(root: &'a Path, read_only: bool) -> Result<Server<'a>> {
        let cfg = config::load(root)?;
        index::keep_graph_in_memory(root);
        Ok(Server {
            root,
            cfg,
            read_only,
        })
    }
}

/// The tools a server offers for a project with these modules on.
fn tools_for(modules: Vec<String>, read_only: bool) -> Vec<Value> {
    let server = Server {
        root: Path::new("."),
        cfg: Config::new(modules, config::IdScheme::Sequential),
        read_only,
    };
    server.tools()
}

/// Every tool the server can offer, each with the module it needs (if any)
/// and whether it writes. Found by turning one module off at a time.
pub(crate) fn tool_catalog() -> Vec<(Value, Option<String>)> {
    let all: Vec<String> = oxispec_core::modules::MODULES
        .iter()
        .map(|m| m.name.to_string())
        .collect();
    let name_of = |t: &Value| t["name"].as_str().unwrap_or("").to_string();
    let mut needs: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for module in &all {
        let without: Vec<String> = all.iter().filter(|m| *m != module).cloned().collect();
        let kept: std::collections::HashSet<String> =
            tools_for(without, false).iter().map(name_of).collect();
        for t in tools_for(all.clone(), false) {
            let n = name_of(&t);
            if !kept.contains(&n) {
                needs.entry(n).or_insert_with(|| module.clone());
            }
        }
    }
    tools_for(all, false)
        .into_iter()
        .map(|t| {
            let module = needs.get(&name_of(&t)).cloned();
            (t, module)
        })
        .collect()
}

// spec: REQ-MCP-0006
pub fn serve(root: &Path, read_only: bool) -> Result<()> {
    let server = Server::new(root, read_only)?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.context("cannot read from stdin")?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(message) => server.handle(&message, &Policy::full()),
            Err(e) => Some(error(Value::Null, -32700, &format!("parse error: {e}"))),
        };
        if let Some(reply) = reply {
            writeln!(stdout, "{reply}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

pub(crate) fn result(id: &Value, value: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": value})
}

pub(crate) fn error(id: Value, code: i32, message: &str) -> Value {
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
    /// Answers one message for a caller who may see what `policy` allows.
    pub(crate) fn handle(&self, message: &Value, policy: &Policy) -> Option<Value> {
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
                        "capabilities": {
                            "tools": {"listChanged": false},
                            "resources": {"subscribe": false, "listChanged": false},
                            "prompts": {"listChanged": false},
                        },
                        "serverInfo": {"name": "oxispec", "version": env!("CARGO_PKG_VERSION")},
                        "instructions": INSTRUCTIONS,
                    }),
                )
            }
            "ping" => result(&id, json!({})),
            "tools/list" => {
                let tools: Vec<Value> = self
                    .tools()
                    .into_iter()
                    .filter(|t| t["name"].as_str().is_some_and(|n| policy.allows_tool(n)))
                    .collect();
                result(&id, json!({"tools": tools}))
            }
            "resources/list" => match self.resources(&params, policy) {
                Ok(value) => result(&id, value),
                Err(e) => error(id, -32602, &format!("{e:#}")),
            },
            "resources/templates/list" => result(
                &id,
                json!({"resourceTemplates": [{
                    "uriTemplate": format!("{NODE_URI}{{id}}"),
                    "name": "node",
                    "title": "A spec node",
                    "description": "The markdown of one node, by ID, for example REQ-AUTH-0001",
                    "mimeType": "text/markdown",
                }]}),
            ),
            "resources/read" => match self.read_resource(&params, policy) {
                Ok(value) => result(&id, value),
                Err(e) => error(id, -32002, &format!("{e:#}")),
            },
            "prompts/list" => result(&id, json!({"prompts": prompt_list()})),
            "prompts/get" => match prompt_get(&params) {
                Ok(value) => result(&id, value),
                Err(e) => error(id, -32602, &format!("{e:#}")),
            },
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                // A tool the caller may not use looks like a tool that does not exist.
                if !policy.allows_tool(name) || !self.tools().iter().any(|t| t["name"] == name) {
                    return Some(error(id, -32602, &format!("unknown tool `{name}`")));
                }
                let (text, is_error) = match self.call(name, &args, policy) {
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

    // spec: REQ-MCP-0002
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
                "spec_api_search",
                "Find the operations of the API (requirements titled `GET /orders`) nearest in meaning to a question, best first, each with its call, requirement, score and the text it was indexed from. Needs the index built with `oxispec api index`; an error says so when it is missing or made with another embedder.",
                json!({
                    "query": {"type": "string", "description": "What the operation does, in words."},
                    "top": {"type": "integer", "minimum": 1, "maximum": 50, "default": 5},
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
                "spec_status",
                "A summary of the project: nodes by type and status, error and warning counts by code, change sets in flight, the share of requirements with claims and the latest release. Start here to see what needs attention.",
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
            tools.push(tool(
                "spec_entities",
                "The entities the specs are about (orders, users, sessions) with their typed attributes, and for each the nodes that depend on it: those that link to it with `about` or another link, those whose claims use its attributes, and the nodes that link to those. Use it before changing a requirement or an entity, to see what else the change touches.",
                json!({
                    "id": {"type": "string", "description": "Only this entity, for example ENT-0001. Without it, every entity with the number of dependents."},
                    "depth": {"type": "integer", "minimum": 1, "maximum": 5, "default": 2, "description": "How many links away a dependency is followed"},
                }),
                &[],
                false,
            ));
        }
        if self.cfg.has("architecture") {
            tools.push(tool(
                "spec_architecture",
                "The architecture model: its elements, environments and rules, or with `view` and `id` one view as a Mermaid flowchart (context and container views are of a system, a component view of a container, a deployment view of an environment). Problems come from spec_check.",
                json!({
                    "view": {"type": "string", "enum": ["context", "container", "component", "deployment"], "description": "Which view to draw. Needs `id`."},
                    "id": {"type": "string", "description": "The system, container or environment the view is of"},
                }),
                &[],
                false,
            ));
        }
        if self.cfg.has("debt") {
            tools.push(tool(
                "spec_debt",
                "The tech debt backlog, the most serious and the oldest first: each item's status, severity, owner, due date, age and what it arises from, flags for overdue and stale items, and the limits a release is held to. Problems come from spec_check.",
                json!({
                    "status": {"type": "string", "enum": ["open", "planned", "in-progress", "resolved", "accepted"], "description": "Only items with this status"},
                    "severity": {"type": "string", "enum": ["low", "medium", "high", "critical"], "description": "Only items with this severity"},
                    "all": {"type": "boolean", "description": "Include resolved and accepted items. Default: false."},
                }),
                &[],
                false,
            ));
        }
        if self.cfg.has("risks") {
            tools.push(tool(
                "spec_risks",
                "The risk register, the most serious level first and the oldest first: each risk's status, likelihood, impact, level, owner, review date, what mitigates it and what it arises from, and the live critical risks that hold a release back. Problems come from spec_check.",
                json!({
                    "status": {"type": "string", "enum": ["open", "mitigating", "mitigated", "accepted", "closed"], "description": "Only risks with this status"},
                    "level": {"type": "string", "enum": ["low", "medium", "high", "critical"], "description": "Only risks at this level"},
                    "all": {"type": "boolean", "description": "Include risks that are not live (mitigated, accepted, closed). Default: false."},
                }),
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

    fn call(&self, name: &str, args: &Value, policy: &Policy) -> Result<String> {
        match name {
            "spec_search" => {
                let query = text_arg(args, "query")?;
                let limit = number_arg(args, "limit", 10, 1, 50)?.to_string();
                let mut cmd = vec!["search", "--json", "--limit", &limit];
                let (ty, status);
                let asked = optional_word(args, "type")?;
                if policy.is_limited() {
                    // The index query itself is narrowed to the types the caller may see.
                    match &asked {
                        Some(t) if !policy.allows_type(t) => return Ok(no_hits(&query)),
                        Some(t) => cmd.extend(["--type", t.as_str()]),
                        None => {
                            for t in policy.types() {
                                cmd.extend(["--type", t]);
                            }
                        }
                    }
                } else if let Some(t) = asked {
                    ty = t;
                    cmd.extend(["--type", &ty]);
                }
                if let Some(s) = optional_word(args, "status")? {
                    status = s;
                    cmd.extend(["--status", &status]);
                }
                cmd.extend(["--", &query]);
                let out = self.cli(&cmd)?;
                // How many nodes the index holds would say how many are hidden.
                if policy.is_limited() {
                    return without_index_counts(&out);
                }
                Ok(out)
            }
            // spec: REQ-MCP-0005
            "spec_api_search" => {
                let query = text_arg(args, "query")?;
                let top = number_arg(args, "top", 5, 1, 50)?.to_string();
                self.cli(&["api", "search", "--json", "--top", &top, "--", &query])
            }
            "spec_get" => self.get(
                &node_id(args, "id")?,
                optional_rev(args, "at")?.as_deref(),
                policy,
            ),
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
            "spec_status" => {
                let mut cmd = vec!["status", "--json"];
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
            // spec: REQ-ENTITY-0006
            "spec_entities" => {
                let depth = number_arg(args, "depth", 2, 1, 5)?.to_string();
                let mut cmd = vec!["entities", "--json", "--depth", &depth];
                let node = optional_id(args, "id")?;
                if let Some(node) = &node {
                    cmd.push(node);
                }
                self.cli(&cmd)
            }
            "spec_architecture" => {
                let view = optional_word(args, "view")?;
                let node = optional_id(args, "id")?;
                match (view, node) {
                    (None, None) => self.cli(&["architecture", "--json"]),
                    (Some(view), Some(node)) => {
                        if !["context", "container", "component", "deployment"]
                            .contains(&view.as_str())
                        {
                            anyhow::bail!("`view` is context, container, component or deployment");
                        }
                        self.cli(&["architecture", "view", &view, &node])
                    }
                    _ => anyhow::bail!("`view` and `id` go together"),
                }
            }
            "spec_debt" => {
                let mut cmd = vec!["debt", "--json"];
                let status = optional_word(args, "status")?;
                if let Some(status) = &status {
                    cmd.extend(["--status", status]);
                }
                let severity = optional_word(args, "severity")?;
                if let Some(severity) = &severity {
                    cmd.extend(["--severity", severity]);
                }
                if args.get("all").and_then(Value::as_bool) == Some(true) {
                    cmd.push("--all");
                }
                self.cli(&cmd)
            }
            // spec: REQ-RISK-0006
            "spec_risks" => {
                let mut cmd = vec!["risks", "--json"];
                let status = optional_word(args, "status")?;
                if let Some(status) = &status {
                    cmd.extend(["--status", status]);
                }
                let level = optional_word(args, "level")?;
                if let Some(level) = &level {
                    cmd.extend(["--level", level]);
                }
                if args.get("all").and_then(Value::as_bool) == Some(true) {
                    cmd.push("--all");
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
                    Some("tests") if node.is_some() => {
                        cmd.extend(["--tests", "--limit", "200", "--json"])
                    }
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
    /// One page of node resources, `RESOURCE_PAGE` at a time. The cursor is an offset.
    // spec: REQ-MCP-0007
    fn resources(&self, params: &Value, policy: &Policy) -> Result<Value> {
        let start = match params.get("cursor").and_then(Value::as_str) {
            None => 0,
            Some(c) => c.parse::<usize>().context("bad cursor")?,
        };
        let graph = self.graph(None)?;
        let mut nodes: Vec<_> = graph.nodes.iter().filter(|n| policy.allows(n)).collect();
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        let page: Vec<Value> = nodes
            .iter()
            .skip(start)
            .take(RESOURCE_PAGE)
            .map(|n| {
                json!({
                    "uri": format!("{NODE_URI}{}", n.id),
                    "name": n.id,
                    "title": n.title,
                    "description": format!("{}{}", n.ty, n.status.as_ref().map(|s| format!(", {s}")).unwrap_or_default()),
                    "mimeType": "text/markdown",
                })
            })
            .collect();
        let end = start + page.len();
        let mut out = json!({"resources": page});
        if end < nodes.len() {
            out["nextCursor"] = json!(end.to_string());
        }
        Ok(out)
    }

    fn read_resource(&self, params: &Value, policy: &Policy) -> Result<Value> {
        let uri = params
            .get("uri")
            .and_then(Value::as_str)
            .context("`uri` is required")?;
        let node_id = uri
            .strip_prefix(NODE_URI)
            .with_context(|| format!("`{uri}` is not an oxispec resource"))?;
        valid_id(node_id)?;
        let graph = self.graph(None)?;
        let node = graph
            .get(node_id)
            .filter(|n| policy.allows(n))
            .with_context(|| format!("no node with id {node_id}"))?;
        let text = std::fs::read_to_string(self.root.join(&node.path))
            .with_context(|| format!("cannot read {}", node.path.display()))?;
        Ok(json!({"contents": [{"uri": uri, "mimeType": "text/markdown", "text": text}]}))
    }

    /// Runs an `oxispec` command in this process and returns what it printed.
    ///
    /// The arguments are parsed like a command line, so a tool and the command
    /// it stands for cannot drift apart, and no process is started.
    fn cli(&self, args: &[&str]) -> Result<String> {
        use clap::Parser;
        let line = ["oxispec", "--root"]
            .into_iter()
            .map(String::from)
            .chain([self.root.display().to_string()])
            .chain(args.iter().map(|a| a.to_string()));
        let cli = crate::Cli::try_parse_from(line).map_err(|e| anyhow::anyhow!("{e}"))?;
        let (text, outcome) = crate::output::capture(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| crate::run(cli)))
        });
        match outcome {
            Ok(Ok(_)) if !text.trim().is_empty() => Ok(text),
            Ok(Ok(_)) => anyhow::bail!("the command printed nothing"),
            Ok(Err(e)) => Err(e),
            Err(_) => anyhow::bail!("the command stopped unexpectedly"),
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

    // spec: REQ-MCP-0001
    fn get(&self, node_id: &str, at: Option<&str>, policy: &Policy) -> Result<String> {
        let graph = self.graph(at)?;
        let node = graph
            .get(node_id)
            .filter(|n| policy.allows(n))
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
            .map(|(n, kind)| {
                if policy.allows(n) {
                    json!({"id": n.id, "kind": kind, "type": n.ty, "title": n.title})
                } else {
                    json!({"id": REDACTED, "kind": kind})
                }
            })
            .collect();
        let mut shown = serde_json::to_value(node)?;
        if policy.is_limited() {
            redact_links(&mut shown, &graph, policy);
        }
        Ok(serde_json::to_string_pretty(&json!({
            "node": shown,
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

    // spec: REQ-MCP-0004
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

/// The answer to a search that cannot match anything the caller may see.
fn no_hits(query: &str) -> String {
    json!({
        "schema_version": oxispec_core::OUTPUT_SCHEMA_VERSION,
        "query": query,
        "hits": [],
    })
    .to_string()
}

/// A search answer without the index statistics.
fn without_index_counts(out: &str) -> Result<String> {
    let mut value: Value = serde_json::from_str(out).context("the search answer is not JSON")?;
    if let Some(object) = value.as_object_mut() {
        object.remove("index");
    }
    Ok(serde_json::to_string_pretty(&value)?)
}

/// Replaces each link target the caller may not see with a stub. A target is
/// an ID, or an ID and a scenario such as `REQ-AUTH-0001.S1`.
fn redact_links(node: &mut Value, graph: &Graph, policy: &Policy) {
    let Some(links) = node.get_mut("links").and_then(Value::as_object_mut) else {
        return;
    };
    for targets in links.values_mut().filter_map(Value::as_array_mut) {
        for target in targets {
            let hidden = target.as_str().is_some_and(|t| {
                let base = t.split('.').next().unwrap_or(t);
                graph
                    .get(t)
                    .or_else(|| graph.get(base))
                    .is_some_and(|n| !policy.allows(n))
            });
            if hidden {
                *target = json!(REDACTED);
            }
        }
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

// spec: REQ-MCP-0003
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
