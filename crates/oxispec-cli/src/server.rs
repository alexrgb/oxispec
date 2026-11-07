//! `oxispec serve`: the MCP server over HTTP, shared by many clients, and
//! `oxispec token`, which manages who may connect.
//!
//! One process holds the graph in memory and answers every client, so a team
//! does not pay for a process and a project load per client. The server is
//! read-only. Each request carries a bearer token; the token decides which
//! nodes and tools the caller sees (see [`oxispec_core::access`]).
//!
//! The transport is the "streamable HTTP" one in its plain form: a client POSTs
//! one JSON-RPC message to `/mcp` and gets one JSON answer, or `202` for a
//! notification. There is no event stream and no session.

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use oxispec_core::access::{Policy, Store, TOKEN_PREFIX};
use serde_json::{Value, json};
use tiny_http::{Header, Method, Request, Response, StatusCode};

use crate::mcp::{Server, error};

/// The largest request body.
const MAX_BODY: u64 = 1024 * 1024;
const WORKERS: usize = 4;

pub struct ServeOptions {
    pub listen: String,
    pub behind_tls_proxy: bool,
    pub audit_log: Option<PathBuf>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---- tokens ---------------------------------------------------------------

/// A new token: the prefix and 32 random bytes in hex.
fn new_secret() -> Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow!("cannot read randomness: {e}"))?;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!("{TOKEN_PREFIX}{hex}"))
}

// spec: REQ-SERVE-0001
pub fn token_add(root: &Path, name: &str, types: Vec<String>, json: bool) -> Result<()> {
    let mut store = Store::load(root)?;
    let secret = new_secret()?;
    let types = (!types.is_empty()).then_some(types);
    store.add(name, types.clone(), &secret, now())?;
    store.save(root)?;
    if json {
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({"name": name, "types": types, "token": secret}))?
        );
    } else {
        outln!("{secret}");
        eprintln!(
            "token `{name}` created. This is the only time it is shown; the project keeps a hash.{}",
            match &types {
                Some(t) => format!(
                    " It reads only {} nodes, with spec_search and spec_get.",
                    t.join(", ")
                ),
                None => String::new(),
            }
        );
    }
    Ok(())
}

pub fn token_list(root: &Path, json: bool) -> Result<()> {
    let store = Store::load(root)?;
    if json {
        let tokens: Vec<Value> = store
            .tokens
            .iter()
            .map(|t| json!({"name": t.name, "types": t.types, "created": t.created}))
            .collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({"tokens": tokens}))?
        );
        return Ok(());
    }
    if store.tokens.is_empty() {
        outln!("no tokens; create one with `oxispec token add NAME`");
    }
    for t in &store.tokens {
        let scope = match &t.types {
            Some(types) => format!("only {}", types.join(", ")),
            None => "everything".to_string(),
        };
        outln!("{}  {scope}", t.name);
    }
    Ok(())
}

pub fn token_revoke(root: &Path, name: &str) -> Result<()> {
    let mut store = Store::load(root)?;
    if !store.revoke(name) {
        bail!("no token named `{name}`");
    }
    store.save(root)?;
    outln!("revoked `{name}`; a running server stops accepting it on the next request");
    Ok(())
}

// ---- the server -----------------------------------------------------------

struct Audit(Mutex<std::fs::File>);

impl Audit {
    fn open(path: &Path) -> Result<Audit> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("cannot open the audit log {}", path.display()))?;
        Ok(Audit(Mutex::new(file)))
    }

    /// One JSON line. A failure to write is reported but does not stop the server.
    fn record(&self, entry: Value) {
        let mut file = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = writeln!(file, "{entry}") {
            eprintln!("cannot write the audit log: {e}");
        }
    }
}

// spec: REQ-SERVE-0002
// spec: REQ-SERVE-0003
// spec: REQ-SERVE-0004
pub fn serve(root: &Path, options: &ServeOptions) -> Result<()> {
    let addr: SocketAddr = options
        .listen
        .parse()
        .with_context(|| format!("`{}` is not an address like 127.0.0.1:7878", options.listen))?;
    if !addr.ip().is_loopback() && !options.behind_tls_proxy {
        bail!(
            "refusing to listen on {addr}: tokens would cross the network unencrypted. \
             Put a TLS proxy in front and pass --behind-tls-proxy, or listen on 127.0.0.1"
        );
    }
    if Store::load(root)?.tokens.is_empty() {
        bail!("no tokens yet; create one with `oxispec token add NAME`");
    }
    let audit_path = options
        .audit_log
        .clone()
        .unwrap_or_else(|| root.join(".oxispec/server/audit.jsonl"));
    let audit = Audit::open(&audit_path)?;
    let mcp = Server::new(root, true)?;
    let http =
        tiny_http::Server::http(addr).map_err(|e| anyhow!("cannot listen on {addr}: {e}"))?;
    // With port 0 the system picks one, so say which.
    let bound = http.server_addr().to_ip().unwrap_or(addr);
    eprintln!(
        "serving {} on http://{bound}/mcp (read-only; audit log {})",
        root.display(),
        audit_path.display()
    );
    let state = State {
        root,
        mcp: &mcp,
        audit: &audit,
        loopback_only: !options.behind_tls_proxy,
    };
    std::thread::scope(|scope| {
        for _ in 0..WORKERS {
            // Handling a call runs a command, which needs more stack than the default.
            std::thread::Builder::new()
                .stack_size(crate::STACK_BYTES)
                .spawn_scoped(scope, || {
                    for request in http.incoming_requests() {
                        state.handle(request);
                    }
                })
                .expect("a thread can be started");
        }
    });
    Ok(())
}

struct State<'a> {
    root: &'a Path,
    mcp: &'a Server<'a>,
    audit: &'a Audit,
    /// Only accept requests addressed to localhost, which stops a web page from
    /// reaching the server through a DNS name that points at this machine.
    loopback_only: bool,
}

fn header(request: &Request, name: &str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str().to_string())
}

fn host_is_loopback(host: &str) -> bool {
    let name = match host.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or(""),
        None => host.rsplit_once(':').map_or(host, |(name, _)| name),
    };
    matches!(name, "localhost" | "127.0.0.1" | "::1")
}

fn json_header() -> Header {
    Header::from_bytes("Content-Type", "application/json").expect("a header")
}

fn plain(request: Request, status: u16, text: &str) {
    let _ = request.respond(Response::from_string(text).with_status_code(StatusCode(status)));
}

impl State<'_> {
    fn handle(&self, mut request: Request) {
        let peer = request.remote_addr().map(|a| a.to_string());
        let path = request.url().split('?').next().unwrap_or("").to_string();

        if self.loopback_only && !header(&request, "Host").is_some_and(|h| host_is_loopback(&h)) {
            return plain(request, 403, "forbidden host");
        }
        if path == "/health" && *request.method() == Method::Get {
            return plain(request, 200, "ok");
        }
        if path != "/mcp" {
            return plain(request, 404, "not found");
        }
        if *request.method() != Method::Post {
            let allow = Header::from_bytes("Allow", "POST").expect("a header");
            let _ = request.respond(
                Response::from_string("use POST")
                    .with_status_code(405)
                    .with_header(allow),
            );
            return;
        }

        // Who is asking. The file is read each time, so a revoked token stops working at once.
        let store = match Store::load(self.root) {
            Ok(store) => store,
            Err(e) => {
                eprintln!("{e}");
                return plain(request, 500, "the token file cannot be read");
            }
        };
        let presented = header(&request, "Authorization")
            .and_then(|v| v.strip_prefix("Bearer ").map(|t| t.trim().to_string()));
        let Some(token) = presented.as_deref().and_then(|t| store.authenticate(t)) else {
            self.audit.record(json!({
                "time": now(), "principal": null, "peer": peer, "outcome": "unauthenticated",
            }));
            let challenge = Header::from_bytes("WWW-Authenticate", "Bearer").expect("a header");
            let _ = request.respond(
                Response::from_string("a valid bearer token is required")
                    .with_status_code(401)
                    .with_header(challenge),
            );
            return;
        };
        let policy = Policy::for_token(token);

        let mut body = String::new();
        let read = request
            .as_reader()
            .take(MAX_BODY + 1)
            .read_to_string(&mut body);
        if read.is_err() {
            return plain(request, 400, "the body is not text");
        }
        if body.len() as u64 > MAX_BODY {
            return plain(request, 413, "the request is too large");
        }

        let reply = match serde_json::from_str::<Value>(&body) {
            Ok(Value::Array(_)) => Some(error(
                Value::Null,
                -32600,
                "batches are not supported; send one message per request",
            )),
            Ok(message) => {
                let reply = self.mcp.handle(&message, &policy);
                self.audit
                    .record(audit_entry(&token.name, &message, &policy, reply.as_ref()));
                reply
            }
            Err(e) => Some(error(Value::Null, -32700, &format!("parse error: {e}"))),
        };
        let _ = match reply {
            Some(reply) => {
                request.respond(Response::from_string(reply.to_string()).with_header(json_header()))
            }
            None => request.respond(Response::from_string("").with_status_code(202)),
        };
    }
}

/// What to record about a message: who, which method, which tool and node,
/// and how it went. Search words and other arguments are not kept.
fn audit_entry(principal: &str, message: &Value, policy: &Policy, reply: Option<&Value>) -> Value {
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let params = message.get("params");
    let tool = (method == "tools/call")
        .then(|| params.and_then(|p| p.get("name")).and_then(Value::as_str))
        .flatten();
    let node = params
        .and_then(|p| p.get("arguments"))
        .and_then(|a| a.get("id").or_else(|| a.get("ids")))
        .cloned();
    let outcome = match (tool, reply) {
        (Some(t), _) if !policy.allows_tool(t) => "denied",
        (_, Some(r)) if r.get("error").is_some() => "error",
        (_, Some(r)) if r["result"]["isError"] == json!(true) => "error",
        _ => "ok",
    };
    json!({
        "time": now(),
        "principal": principal,
        "method": method,
        "tool": tool,
        "node": node,
        "outcome": outcome,
    })
}

#[cfg(test)]
mod tests_of_server {
    use super::*;

    #[test]
    fn only_localhost_names_are_loopback_hosts() {
        for ok in ["localhost", "localhost:7878", "127.0.0.1:80", "[::1]:7878"] {
            assert!(host_is_loopback(ok), "{ok}");
        }
        for bad in [
            "evil.example",
            "evil.example:7878",
            "127.0.0.1.evil.example",
            "",
        ] {
            assert!(!host_is_loopback(bad), "{bad}");
        }
    }

    #[test]
    fn a_new_secret_has_the_prefix_and_is_different_each_time() {
        let (a, b) = (new_secret().unwrap(), new_secret().unwrap());
        assert!(a.starts_with(TOKEN_PREFIX));
        assert_eq!(a.len(), TOKEN_PREFIX.len() + 64);
        assert_ne!(a, b);
    }
}
