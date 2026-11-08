//! Opt-in modules and presets (design section 3).

pub struct ModuleInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub needs: &'static [&'static str],
    /// False means the module can be enabled and recorded, but does nothing yet.
    pub implemented: bool,
}

pub const MODULES: &[ModuleInfo] = &[
    ModuleInfo {
        name: "core",
        description: "Nodes, links, IDs, structural checks, CLI",
        needs: &[],
        implemented: true,
    },
    ModuleInfo {
        name: "specs",
        description: "Requirements and scenarios",
        needs: &["core"],
        implemented: true,
    },
    ModuleInfo {
        name: "tests",
        description: "Test cases, scanners, coverage, suspect links",
        needs: &["specs"],
        implemented: true,
    },
    ModuleInfo {
        name: "adrs",
        description: "Architecture decision records",
        needs: &["core"],
        implemented: true,
    },
    ModuleInfo {
        name: "decisions",
        description: "Lightweight decision log",
        needs: &["core"],
        implemented: false,
    },
    ModuleInfo {
        name: "standards",
        description: "Code standards with linter bindings",
        needs: &["core"],
        implemented: false,
    },
    ModuleInfo {
        name: "api",
        description: "AsyncAPI import and operation nodes of their own. OpenAPI import, generation and `oxispec api` need only `specs`",
        needs: &["core"],
        implemented: false,
    },
    ModuleInfo {
        name: "diagrams",
        description: "Mermaid, PlantUML and D2 diagrams",
        needs: &["core"],
        implemented: false,
    },
    ModuleInfo {
        name: "architecture",
        description: "C4 and deployment model, views, rules",
        needs: &["core"],
        implemented: true,
    },
    ModuleInfo {
        name: "statemachines",
        description: "State machines, property checks, diagrams, generated tests",
        needs: &["specs"],
        implemented: true,
    },
    ModuleInfo {
        name: "consistency",
        description: "Claims, vocabulary, solver",
        needs: &["specs"],
        implemented: true,
    },
    ModuleInfo {
        name: "releases",
        description: "Release manifests and gates",
        needs: &["core"],
        implemented: true,
    },
    ModuleInfo {
        name: "debt",
        description: "Tech debt backlog with owners, due dates and release limits",
        needs: &["core"],
        implemented: true,
    },
    ModuleInfo {
        name: "risks",
        description: "Risk register with owners, review dates and a release gate",
        needs: &["core"],
        implemented: true,
    },
    ModuleInfo {
        name: "ci",
        description: "`oxispec ci`: graph diff, impact analysis, reports",
        needs: &["core"],
        implemented: true,
    },
    ModuleInfo {
        name: "agents",
        description: "Skills and rules for AI clients",
        needs: &["core"],
        implemented: true,
    },
    ModuleInfo {
        name: "mcp",
        description: "MCP server",
        needs: &["core"],
        implemented: true,
    },
    ModuleInfo {
        name: "server",
        description: "Hosted web app, OIDC and policies. `oxispec serve` needs only `mcp`",
        needs: &["core"],
        implemented: false,
    },
];

pub fn info(name: &str) -> Option<&'static ModuleInfo> {
    MODULES.iter().find(|m| m.name == name)
}

pub const PRESETS: &[&str] = &["minimal", "specs-tests", "docs", "standard", "full"];

pub fn preset(name: &str) -> Option<Vec<&'static str>> {
    let list: Vec<&'static str> = match name {
        "minimal" => vec!["core", "specs"],
        "specs-tests" => vec!["core", "specs", "tests"],
        "docs" => vec![
            "core",
            "specs",
            "adrs",
            "decisions",
            "diagrams",
            "architecture",
        ],
        "standard" => vec![
            "core",
            "specs",
            "tests",
            "adrs",
            "decisions",
            "standards",
            "architecture",
            "consistency",
            "statemachines",
            "debt",
            "risks",
            "ci",
            "agents",
            "mcp",
        ],
        "full" => MODULES.iter().map(|m| m.name).collect(),
        _ => return None,
    };
    Some(list)
}

/// Adds everything the given modules need, rejects unknown names, and
/// returns the result in registry order without duplicates.
pub fn resolve<S: AsRef<str>>(names: &[S]) -> Result<Vec<String>, String> {
    let mut wanted: Vec<&'static str> = vec!["core"];
    let mut stack: Vec<&str> = names.iter().map(|s| s.as_ref()).collect();
    while let Some(name) = stack.pop() {
        let m = info(name).ok_or_else(|| {
            let known: Vec<&str> = MODULES.iter().map(|m| m.name).collect();
            format!("unknown module `{name}` (known: {})", known.join(", "))
        })?;
        if !wanted.contains(&m.name) {
            wanted.push(m.name);
        }
        stack.extend(m.needs.iter().copied());
    }
    Ok(MODULES
        .iter()
        .filter(|m| wanted.contains(&m.name))
        .map(|m| m.name.to_string())
        .collect())
}

/// Enabled modules (other than `name`) that need `name`.
pub fn dependents_of(name: &str, enabled: &[String]) -> Vec<&'static str> {
    MODULES
        .iter()
        .filter(|m| m.needs.contains(&name) && enabled.iter().any(|e| e == m.name))
        .map(|m| m.name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_tests_pulls_in_specs_and_core() {
        assert_eq!(resolve(&["tests"]).unwrap(), vec!["core", "specs", "tests"]);
    }

    #[test]
    fn unknown_module_is_rejected() {
        assert!(resolve(&["nope"]).unwrap_err().contains("unknown module"));
    }

    #[test]
    fn presets_are_valid_and_closed() {
        for p in PRESETS {
            let list = preset(p).unwrap();
            assert_eq!(
                resolve(&list).unwrap().len(),
                list.len(),
                "preset {p} is not closed"
            );
        }
    }

    #[test]
    fn dependents() {
        let enabled = resolve(&["tests"]).unwrap();
        assert_eq!(dependents_of("specs", &enabled), vec!["tests"]);
        assert!(dependents_of("adrs", &enabled).is_empty());
    }
}
