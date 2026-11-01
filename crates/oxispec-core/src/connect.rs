//! MCP client configuration: the entry that starts `oxispec mcp`.
//!
//! Each client keeps its servers in a JSON file that often holds other
//! settings too. [`merge`] adds or replaces only the `oxispec` entry and
//! leaves everything else as it was (keys are written in alphabetical order).
//! A file that is not plain JSON is never rewritten.
//!
//! An entry starts `oxispec mcp` on the same machine, or points at a running
//! `oxispec serve` by URL. A URL entry never holds the token: it refers to the
//! environment variable [`TOKEN_ENV`] (VS Code asks for it once instead).

use serde_json::{Map, Value, json};

/// An AI client and where it reads MCP servers from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub id: &'static str,
    pub name: &'static str,
    /// Relative to the project root.
    pub file: &'static str,
    /// The object in the file that holds the servers.
    pub key: &'static str,
    /// Whether the entry needs `"type": "stdio"`.
    pub typed: bool,
}

pub const TARGETS: &[Target] = &[
    Target {
        id: "claude",
        name: "Claude Code",
        file: ".mcp.json",
        key: "mcpServers",
        typed: false,
    },
    Target {
        id: "cursor",
        name: "Cursor",
        file: ".cursor/mcp.json",
        key: "mcpServers",
        typed: false,
    },
    Target {
        id: "gemini",
        name: "Gemini CLI",
        file: ".gemini/settings.json",
        key: "mcpServers",
        typed: false,
    },
    Target {
        id: "copilot",
        name: "VS Code with Copilot",
        file: ".vscode/mcp.json",
        key: "servers",
        typed: true,
    },
];

/// The name the server is registered under.
pub const SERVER_NAME: &str = "oxispec";

/// The environment variable that holds the token of a server reached by URL.
pub const TOKEN_ENV: &str = "OXISPEC_TOKEN";

/// The id of the prompt VS Code shows for the token.
const VSCODE_INPUT: &str = "oxispec-token";

/// How a client reaches the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport<'a> {
    /// Start `oxispec mcp` as a child process.
    Stdio { read_only: bool },
    /// Talk to `oxispec serve` at this URL, with the token from [`TOKEN_ENV`].
    Http { url: &'a str },
}

/// Checks a server URL: http or https, a host, and nothing that carries a secret.
pub fn check_url(url: &str) -> Result<(), String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or("the URL starts with http:// or https://")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("the URL has a host and no spaces".into());
    }
    if authority.contains('@') {
        return Err(
            "do not put a user or password in the URL; the token comes from the environment".into(),
        );
    }
    if url.contains('?') || url.contains('#') {
        return Err(
            "the URL has no query or fragment; a token there would end up in a file".into(),
        );
    }
    Ok(())
}

/// Whether sending a token to this URL crosses a network in the clear.
pub fn is_cleartext_remote(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let authority = rest.split('/').next().unwrap_or("");
    let host = match authority.strip_prefix('[') {
        Some(inner) => inner.split(']').next().unwrap_or(""),
        None => authority.rsplit_once(':').map_or(authority, |(h, _)| h),
    };
    !matches!(host, "localhost" | "127.0.0.1" | "::1")
}

pub fn target(id: &str) -> Option<&'static Target> {
    TARGETS.iter().find(|t| t.id == id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Created,
    Updated,
    Unchanged,
}

/// The entry for `target`.
pub fn entry(target: &Target, transport: Transport) -> Value {
    match transport {
        Transport::Stdio { read_only } => {
            let mut args = vec!["mcp"];
            if read_only {
                args.push("--read-only");
            }
            let mut entry = json!({ "command": "oxispec", "args": args });
            if target.typed {
                entry["type"] = json!("stdio");
            }
            entry
        }
        Transport::Http { url } => {
            // Each client has its own spelling for "the value of an environment variable".
            let (key, variable) = match target.id {
                "cursor" => ("url", format!("${{env:{TOKEN_ENV}}}")),
                "gemini" => ("httpUrl", format!("${{{TOKEN_ENV}}}")),
                "copilot" => ("url", format!("${{input:{VSCODE_INPUT}}}")),
                _ => ("url", format!("${{{TOKEN_ENV}}}")),
            };
            let mut entry = json!({
                key: url,
                "headers": {"Authorization": format!("Bearer {variable}")},
            });
            if target.id == "claude" || target.id == "copilot" {
                entry["type"] = json!("http");
            }
            entry
        }
    }
}

/// The file content with the `oxispec` entry in place.
///
/// `existing` is the current content, or `None` when the file is missing.
/// Returns an error, and no content, when the file cannot be read as a JSON
/// object, so a person's settings are never replaced.
pub fn merge(
    existing: Option<&str>,
    target: &Target,
    transport: Transport,
) -> Result<(String, Outcome), String> {
    if let Transport::Http { url } = transport {
        check_url(url)?;
    }
    let mut root: Value = match existing {
        None => Value::Object(Map::new()),
        Some(text) if text.trim().is_empty() => Value::Object(Map::new()),
        Some(text) => serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| {
            format!(
                "{} is not plain JSON ({e}); add the `{SERVER_NAME}` entry to `{}` by hand",
                target.file, target.key
            )
        })?,
    };
    let Some(object) = root.as_object_mut() else {
        return Err(format!("{} is not a JSON object", target.file));
    };
    let servers = object
        .entry(target.key)
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(servers) = servers.as_object_mut() else {
        return Err(format!(
            "`{}` in {} is not an object",
            target.key, target.file
        ));
    };
    let wanted = entry(target, transport);
    // VS Code asks for the token through a declared input.
    let mut inputs_changed = false;
    let wants_input = matches!(transport, Transport::Http { .. }) && target.id == "copilot";
    let mut outcome = match servers.get(SERVER_NAME) {
        Some(current) if *current == wanted => Outcome::Unchanged,
        Some(_) => Outcome::Updated,
        None if existing.is_some() => Outcome::Updated,
        None => Outcome::Created,
    };
    servers.insert(SERVER_NAME.to_string(), wanted);
    if wants_input {
        let inputs = object.entry("inputs").or_insert_with(|| json!([]));
        let Some(list) = inputs.as_array_mut() else {
            return Err(format!("`inputs` in {} is not a list", target.file));
        };
        if !list.iter().any(|i| i["id"] == VSCODE_INPUT) {
            list.push(json!({
                "type": "promptString",
                "id": VSCODE_INPUT,
                "description": "OxiSpec server token (oxispec token add)",
                "password": true,
            }));
            inputs_changed = true;
        }
    }
    if inputs_changed && outcome == Outcome::Unchanged {
        outcome = Outcome::Updated;
    }
    let mut text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    text.push('\n');
    Ok((text, outcome))
}

#[cfg(test)]
mod tests_of_connect {
    use super::*;

    const STDIO: Transport<'static> = Transport::Stdio { read_only: false };
    const URL: &str = "http://127.0.0.1:7878/mcp";

    fn claude() -> &'static Target {
        target("claude").unwrap()
    }

    #[test]
    fn a_missing_file_is_created_with_the_entry() {
        let (text, outcome) = merge(None, claude(), STDIO).unwrap();
        assert_eq!(outcome, Outcome::Created);
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["mcpServers"]["oxispec"]["command"], "oxispec");
        assert_eq!(v["mcpServers"]["oxispec"]["args"], json!(["mcp"]));
    }

    #[test]
    fn other_settings_and_servers_are_kept() {
        let before = r#"{"theme":"dark","mcpServers":{"other":{"command":"x"}}}"#;
        let (text, outcome) =
            merge(Some(before), claude(), Transport::Stdio { read_only: true }).unwrap();
        assert_eq!(outcome, Outcome::Updated);
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["theme"], "dark");
        assert_eq!(v["mcpServers"]["other"]["command"], "x");
        assert_eq!(
            v["mcpServers"]["oxispec"]["args"],
            json!(["mcp", "--read-only"])
        );
    }

    #[test]
    fn running_twice_changes_nothing() {
        let (once, _) = merge(None, claude(), STDIO).unwrap();
        let (twice, outcome) = merge(Some(&once), claude(), STDIO).unwrap();
        assert_eq!(outcome, Outcome::Unchanged);
        assert_eq!(once, twice);
    }

    #[test]
    fn a_different_entry_is_replaced() {
        let before = r#"{"mcpServers":{"oxispec":{"command":"old"}}}"#;
        let (text, outcome) = merge(Some(before), claude(), STDIO).unwrap();
        assert_eq!(outcome, Outcome::Updated);
        assert!(!text.contains("old"));
    }

    #[test]
    fn copilot_uses_servers_and_a_type() {
        let t = target("copilot").unwrap();
        let (text, _) = merge(Some("  \n"), t, STDIO).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["servers"]["oxispec"]["type"], "stdio");
        assert!(v.get("mcpServers").is_none());
    }

    #[test]
    fn a_file_with_comments_or_the_wrong_shape_is_refused() {
        for bad in ["{ // comment\n}", "[]", r#"{"mcpServers": 3}"#, "not json"] {
            assert!(merge(Some(bad), claude(), STDIO).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_byte_order_mark_is_tolerated() {
        let before = "\u{feff}{\"a\":1}";
        let (text, _) = merge(Some(before), claude(), STDIO).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn each_client_gets_its_own_remote_entry_with_the_token_from_the_environment() {
        let http = Transport::Http { url: URL };
        let entry_of = |id: &str| entry(target(id).unwrap(), http);
        assert_eq!(
            entry_of("claude"),
            json!({"type": "http", "url": URL, "headers": {"Authorization": "Bearer ${OXISPEC_TOKEN}"}})
        );
        assert_eq!(
            entry_of("cursor"),
            json!({"url": URL, "headers": {"Authorization": "Bearer ${env:OXISPEC_TOKEN}"}})
        );
        assert_eq!(
            entry_of("gemini"),
            json!({"httpUrl": URL, "headers": {"Authorization": "Bearer ${OXISPEC_TOKEN}"}})
        );
        assert_eq!(
            entry_of("copilot"),
            json!({"type": "http", "url": URL, "headers": {"Authorization": "Bearer ${input:oxispec-token}"}})
        );
    }

    #[test]
    fn vs_code_declares_the_token_input_once_and_keeps_other_inputs() {
        let t = target("copilot").unwrap();
        let http = Transport::Http { url: URL };
        let before = r#"{"inputs":[{"id":"other","type":"promptString"}],"servers":{}}"#;
        let (once, outcome) = merge(Some(before), t, http).unwrap();
        assert_eq!(outcome, Outcome::Updated);
        let v: Value = serde_json::from_str(&once).unwrap();
        assert_eq!(v["inputs"].as_array().unwrap().len(), 2);
        assert_eq!(v["inputs"][1]["password"], true);
        let (twice, outcome) = merge(Some(&once), t, http).unwrap();
        assert_eq!(outcome, Outcome::Unchanged);
        assert_eq!(once, twice);
        assert!(merge(Some(r#"{"inputs": 3}"#), t, http).is_err());
    }

    #[test]
    fn switching_between_a_local_and_a_remote_server_replaces_the_entry() {
        let (local, _) = merge(None, claude(), STDIO).unwrap();
        let (remote, outcome) =
            merge(Some(&local), claude(), Transport::Http { url: URL }).unwrap();
        assert_eq!(outcome, Outcome::Updated);
        assert!(remote.contains(URL) && !remote.contains("\"command\""));
    }

    #[test]
    fn a_url_that_could_carry_a_secret_is_refused() {
        for bad in [
            "ftp://x/mcp",
            "http://",
            "http://user:pass@host/mcp",
            "https://host/mcp?token=abc",
            "https://host/mcp#x",
            "https://host/m cp",
            "host/mcp",
        ] {
            assert!(check_url(bad).is_err(), "{bad}");
            let http = Transport::Http { url: bad };
            assert!(merge(None, claude(), http).is_err(), "{bad}");
        }
        assert!(check_url("https://specs.example.com/mcp").is_ok());
    }

    #[test]
    fn plain_http_to_another_machine_is_flagged() {
        assert!(is_cleartext_remote("http://specs.example.com/mcp"));
        assert!(!is_cleartext_remote("https://specs.example.com/mcp"));
        for local in [
            "http://localhost:7878/mcp",
            "http://127.0.0.1/mcp",
            "http://[::1]:1/mcp",
        ] {
            assert!(!is_cleartext_remote(local), "{local}");
        }
    }
}
