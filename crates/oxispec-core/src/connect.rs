//! MCP client configuration: the entry that starts `oxispec mcp`.
//!
//! Each client keeps its servers in a JSON file that often holds other
//! settings too. [`merge`] adds or replaces only the `oxispec` entry and
//! leaves everything else as it was (keys are written in alphabetical order).
//! A file that is not plain JSON is never rewritten.

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
pub fn entry(target: &Target, read_only: bool) -> Value {
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

/// The file content with the `oxispec` entry in place.
///
/// `existing` is the current content, or `None` when the file is missing.
/// Returns an error, and no content, when the file cannot be read as a JSON
/// object, so a person's settings are never replaced.
pub fn merge(
    existing: Option<&str>,
    target: &Target,
    read_only: bool,
) -> Result<(String, Outcome), String> {
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
    let wanted = entry(target, read_only);
    let outcome = match servers.get(SERVER_NAME) {
        Some(current) if *current == wanted => Outcome::Unchanged,
        Some(_) => Outcome::Updated,
        None if existing.is_some() => Outcome::Updated,
        None => Outcome::Created,
    };
    servers.insert(SERVER_NAME.to_string(), wanted);
    let mut text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    text.push('\n');
    Ok((text, outcome))
}

#[cfg(test)]
mod tests_of_connect {
    use super::*;

    fn claude() -> &'static Target {
        target("claude").unwrap()
    }

    #[test]
    fn a_missing_file_is_created_with_the_entry() {
        let (text, outcome) = merge(None, claude(), false).unwrap();
        assert_eq!(outcome, Outcome::Created);
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["mcpServers"]["oxispec"]["command"], "oxispec");
        assert_eq!(v["mcpServers"]["oxispec"]["args"], json!(["mcp"]));
    }

    #[test]
    fn other_settings_and_servers_are_kept() {
        let before = r#"{"theme":"dark","mcpServers":{"other":{"command":"x"}}}"#;
        let (text, outcome) = merge(Some(before), claude(), true).unwrap();
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
        let (once, _) = merge(None, claude(), false).unwrap();
        let (twice, outcome) = merge(Some(&once), claude(), false).unwrap();
        assert_eq!(outcome, Outcome::Unchanged);
        assert_eq!(once, twice);
    }

    #[test]
    fn a_different_entry_is_replaced() {
        let before = r#"{"mcpServers":{"oxispec":{"command":"old"}}}"#;
        let (text, outcome) = merge(Some(before), claude(), false).unwrap();
        assert_eq!(outcome, Outcome::Updated);
        assert!(!text.contains("old"));
    }

    #[test]
    fn copilot_uses_servers_and_a_type() {
        let t = target("copilot").unwrap();
        let (text, _) = merge(Some("  \n"), t, false).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["servers"]["oxispec"]["type"], "stdio");
        assert!(v.get("mcpServers").is_none());
    }

    #[test]
    fn a_file_with_comments_or_the_wrong_shape_is_refused() {
        for bad in ["{ // comment\n}", "[]", r#"{"mcpServers": 3}"#, "not json"] {
            assert!(merge(Some(bad), claude(), false).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_byte_order_mark_is_tolerated() {
        let before = "\u{feff}{\"a\":1}";
        let (text, _) = merge(Some(before), claude(), false).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["a"], 1);
    }
}
