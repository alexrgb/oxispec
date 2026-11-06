//! Agent skills (design section 15): `SKILL.md` folders for AI clients.
//!
//! The skills are written once, as markdown in `skills/`, and rendered into
//! the skills folder of each client. A hash in the frontmatter tells a file
//! that oxispec wrote from one that someone edited, so regenerating never
//! overwrites a hand edit.

use std::path::{Path, PathBuf};

/// An AI client and the folder it reads project skills from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Client {
    pub id: &'static str,
    pub name: &'static str,
    /// Relative to the project root. Each skill is a folder inside it.
    pub dir: &'static str,
}

pub const CLIENTS: &[Client] = &[
    Client {
        id: "claude",
        name: "Claude Code",
        dir: ".claude/skills",
    },
    Client {
        id: "cursor",
        name: "Cursor",
        dir: ".cursor/skills",
    },
    Client {
        id: "copilot",
        name: "GitHub Copilot",
        dir: ".github/skills",
    },
    Client {
        id: "gemini",
        name: "Gemini CLI",
        dir: ".gemini/skills",
    },
    // Read by Cursor, Copilot and Gemini CLI as well. One copy for all of
    // them, so it is not part of the default.
    Client {
        id: "agents",
        name: "Any client that reads .agents/skills",
        dir: ".agents/skills",
    },
];

/// The clients `oxispec init` writes for unless told otherwise.
pub const DEFAULT_CLIENTS: &[&str] = &["claude", "cursor", "copilot", "gemini"];

pub fn client(id: &str) -> Option<&'static Client> {
    CLIENTS.iter().find(|c| c.id == id)
}

/// Where overlays live, relative to the project root.
pub const OVERLAY_DIR: &str = ".oxispec/agents";

/// A skill in the Agent Skills format (agentskills.io).
#[derive(Debug, Clone, Copy)]
pub struct Skill {
    /// The folder name and the `name` in the frontmatter.
    pub name: &'static str,
    /// What the skill does and when to use it. One line, as agents choose by it.
    pub description: &'static str,
    /// Modules that have to be enabled. Nothing is written for a disabled module.
    pub requires: &'static [&'static str],
    body: &'static str,
}

pub const SKILLS: &[Skill] = &[
    Skill {
        name: "oxispec-explore",
        description: "Read the project's specs before writing or changing anything. Use to find requirements, decisions and their links, see how a node changed, or answer what the project has decided about a topic.",
        requires: &["core"],
        body: include_str!("../skills/explore.md"),
    },
    Skill {
        name: "oxispec-propose",
        description: "Propose a change to the specs as a change set, with new or edited requirements and scenarios, linked and checked, ready for a person to review. Use when behavior should change or a new requirement is needed.",
        requires: &["specs"],
        body: include_str!("../skills/propose.md"),
    },
    Skill {
        name: "oxispec-extract-claims",
        description: "Write claims for a requirement: the lines a machine can check, such as for role = admin: session.idle_timeout <= 15m, plus the vocabulary they use. Use after writing a requirement with limits, or when oxispec check reports a conflict between claims.",
        requires: &["consistency"],
        body: include_str!("../skills/extract-claims.md"),
    },
    Skill {
        name: "oxispec-model-state-machines",
        description: "Model a lifecycle as a state machine: states, events and transitions, plus properties such as never cancel after shipped, checked with a counterexample path when they fail. Use when a requirement describes states or an order of events, or when oxispec check reports a state machine problem.",
        requires: &["statemachines"],
        body: include_str!("../skills/model-state-machines.md"),
    },
    Skill {
        name: "oxispec-model-architecture",
        description: "Describe the system's architecture once as a model of systems, containers, components and deployment environments, and draw views from it. Use when a requirement or decision changes how the system is built or deployed, or when oxispec check reports an architecture problem.",
        requires: &["architecture"],
        body: include_str!("../skills/model-architecture.md"),
    },
    Skill {
        name: "oxispec-apply",
        description: "Implement an approved change set in code and tests, and tie them to the specs with spec: and verifies: comments. Use when asked to build what a change set or requirement describes.",
        requires: &["specs"],
        body: include_str!("../skills/apply.md"),
    },
    Skill {
        name: "oxispec-verify",
        description: "Check that the specs are consistent and that code and tests cover them: check, coverage, test results, suspect links. Use before finishing work that touches specs or traced code.",
        requires: &["specs"],
        body: include_str!("../skills/verify.md"),
    },
    Skill {
        name: "oxispec-sync-code",
        description: "Review links between specs and code that changed since they were confirmed, decide which side is right, and confirm them. Use when oxispec suspect lists links.",
        requires: &["tests"],
        body: include_str!("../skills/sync-code.md"),
    },
    Skill {
        name: "oxispec-write-adr",
        description: "Write an architecture decision record with its options, decision and consequences, and link it to what it explains. Use for a decision that is hard to reverse.",
        requires: &["adrs"],
        body: include_str!("../skills/write-adr.md"),
    },
    Skill {
        name: "oxispec-manage-debt",
        description: "Record a tech debt item, keep the backlog honest, and pay items off: owner, severity, due date, what it arises from. Use when you take a shortcut on purpose, find one, or are asked what debt there is.",
        requires: &["debt"],
        body: include_str!("../skills/manage-debt.md"),
    },
    Skill {
        name: "oxispec-write-prd",
        description: "Write a product requirements document as nodes: interview the person, draft the purpose, goals and non-goals, then the requirements with their acceptance scenarios, and show the document before anything is written. Use when asked to write or start a PRD, or to turn an idea into specs.",
        requires: &["specs"],
        body: include_str!("../skills/write-prd.md"),
    },
    Skill {
        name: "oxispec-write-api",
        description: "Describe an HTTP API as requirements titled as calls, make the skeleton of its OpenAPI description from them, and compare a real openapi file with them. Use when asked to design or document an API, or to see whether the spec and the API file agree.",
        requires: &["specs"],
        body: include_str!("../skills/write-api.md"),
    },
    Skill {
        name: "oxispec-write-test-plan",
        description: "Write a test plan for requirements and risks: scope, approach, levels, environments and exit criteria, then read it against the tests to see what is not verified. Use when asked to plan testing, or what is not tested yet.",
        requires: &["tests"],
        body: include_str!("../skills/write-test-plan.md"),
    },
    Skill {
        name: "oxispec-review-security",
        description: "Review a feature, an API or a design for security with the OWASP API Security Top 10 or ASVS as the lens, and record what is found as risks for the person to rate and accept. Use when asked for a security review, a threat check, or whether an API is safe.",
        requires: &["risks"],
        body: include_str!("../skills/review-security.md"),
    },
    Skill {
        name: "oxispec-manage-risks",
        description: "Record a risk to the application, keep the register honest, and decide with the person what to do about each: owner, likelihood, impact, what mitigates it, who accepts it. Use when a threat or weakness is found, a design is reviewed for security, or you are asked what risks there are.",
        requires: &["risks"],
        body: include_str!("../skills/manage-risks.md"),
    },
    Skill {
        name: "oxispec-import-specs",
        description: "Import the specs, open changes and design notes of OpenSpec, Spec Kit, Kiro or plain markdown, deciding with the person what each file becomes, showing drafts and a dry run before writing. Use when asked to move a project to OxiSpec.",
        requires: &["specs"],
        body: include_str!("../skills/import-specs.md"),
    },
    Skill {
        name: "oxispec-review-change",
        description: "Review a branch or pull request that changes specs or traced code: new findings, graph differences, affected nodes, suspect links. Use when asked to review a change.",
        requires: &["core"],
        body: include_str!("../skills/review-change.md"),
    },
    Skill {
        name: "oxispec-rollback",
        description: "Restore specs to an earlier revision, checking that nothing breaks. Use when a spec change has to be undone.",
        requires: &["core"],
        body: include_str!("../skills/rollback.md"),
    },
    Skill {
        name: "oxispec-release",
        description: "Prepare, check and publish a release of the specs: manifest, gates and tag. Use when asked to cut a spec release.",
        requires: &["releases"],
        body: include_str!("../skills/release.md"),
    },
];

/// The skills whose modules are all enabled.
pub fn skills_for<S: AsRef<str>>(modules: &[S]) -> Vec<&'static Skill> {
    SKILLS
        .iter()
        .filter(|s| {
            s.requires
                .iter()
                .all(|r| *r == "core" || modules.iter().any(|m| m.as_ref() == *r))
        })
        .collect()
}

/// The project's own notes for a client: `_all.md` first, then `<client>.md`.
pub fn overlay(root: &Path, client: &str) -> String {
    let read = |name: &str| {
        std::fs::read_to_string(root.join(OVERLAY_DIR).join(name))
            .map(|t| t.trim().to_string())
            .unwrap_or_default()
    };
    let parts: Vec<String> = [read("_all.md"), read(&format!("{client}.md"))]
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect();
    parts.join("\n\n")
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn hash(text: &str) -> String {
    blake3::hash(normalize(text).as_bytes()).to_hex()[..16].to_string()
}

/// The text of a `SKILL.md`: frontmatter, the skill, then the project's notes.
pub fn render(skill: &Skill, overlay: &str) -> String {
    let mut body = String::from(skill.body.trim_end());
    body.push('\n');
    if !overlay.trim().is_empty() {
        body.push_str("\n## Project notes\n\n");
        body.push_str(overlay.trim());
        body.push('\n');
    }
    let body = format!("\n{body}");
    format!(
        "---\nname: {}\ndescription: \"{}\"\nmetadata:\n  generated-by: oxispec\n  content-hash: \"{}\"\n---\n{body}",
        skill.name,
        skill.description,
        hash(&body),
    )
}

/// What is on disk compared with what would be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Missing,
    /// Written by oxispec and the same as it would write now.
    Current,
    /// Written by oxispec, not edited, but different from what it would write now.
    Outdated,
    /// Edited by hand, or not written by oxispec at all.
    Edited,
}

/// Splits a file into its stored hash and the text after the frontmatter.
fn stored_hash(text: &str) -> Option<(String, String)> {
    let text = normalize(text);
    let rest = text.strip_prefix("---\n")?;
    let (front, body) = rest.split_once("\n---\n")?;
    let line = front
        .lines()
        .find_map(|l| l.trim().strip_prefix("content-hash:"))?;
    let stored = line.trim().trim_matches('"').to_string();
    Some((stored, format!("\n{}", body.trim_start_matches('\n'))))
}

pub fn state(existing: Option<&str>, expected: &str) -> State {
    let Some(existing) = existing else {
        return State::Missing;
    };
    if normalize(existing) == normalize(expected) {
        return State::Current;
    }
    match stored_hash(existing) {
        Some((stored, body)) if stored == hash(&body) => State::Outdated,
        _ => State::Edited,
    }
}

/// One file `agents update` would write.
#[derive(Debug, Clone)]
pub struct Planned {
    pub client: &'static str,
    pub skill: &'static str,
    /// Relative to the project root.
    pub path: PathBuf,
    pub content: String,
    pub state: State,
}

/// Every file for the given clients and the enabled modules, with its state on disk.
pub fn plan<S: AsRef<str>>(root: &Path, clients: &[&Client], modules: &[S]) -> Vec<Planned> {
    let skills = skills_for(modules);
    let mut out = Vec::new();
    for client in clients {
        let notes = overlay(root, client.id);
        for skill in &skills {
            let path = Path::new(client.dir).join(skill.name).join("SKILL.md");
            let content = render(skill, &notes);
            let existing = std::fs::read_to_string(root.join(&path)).ok();
            out.push(Planned {
                client: client.id,
                skill: skill.name,
                state: state(existing.as_deref(), &content),
                path,
                content,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_names_and_descriptions_follow_the_standard() {
        for s in SKILLS {
            assert!(s.name.len() <= 64, "{}", s.name);
            assert!(
                s.name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{}",
                s.name
            );
            assert!(s.description.len() <= 1024, "{}", s.name);
            assert!(!s.description.contains(['"', '\n', '\\']), "{}", s.name);
            assert!(!s.body.trim().is_empty(), "{}", s.name);
        }
        let mut names: Vec<_> = SKILLS.iter().map(|s| s.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), SKILLS.len(), "duplicate skill name");
    }

    #[test]
    fn every_required_module_exists() {
        for s in SKILLS {
            for r in s.requires {
                assert!(crate::modules::info(r).is_some(), "{} needs {r}", s.name);
            }
        }
    }

    #[test]
    fn a_disabled_module_gets_no_skill() {
        let minimal = skills_for(&["core", "specs"]);
        assert!(minimal.iter().any(|s| s.name == "oxispec-propose"));
        assert!(!minimal.iter().any(|s| s.name == "oxispec-write-adr"));
        assert!(!minimal.iter().any(|s| s.name == "oxispec-release"));
        assert!(!minimal.iter().any(|s| s.name == "oxispec-sync-code"));
        assert!(
            !minimal
                .iter()
                .any(|s| s.name == "oxispec-model-state-machines")
        );
        let all = skills_for(&[
            "core",
            "specs",
            "tests",
            "adrs",
            "releases",
            "consistency",
            "statemachines",
            "architecture",
            "debt",
            "risks",
        ]);
        assert_eq!(all.len(), SKILLS.len());
    }

    #[test]
    fn rendering_has_the_frontmatter_clients_need() {
        let text = render(&SKILLS[0], "");
        assert!(text.starts_with("---\nname: oxispec-explore\ndescription: \""));
        assert!(text.contains("\nmetadata:\n  generated-by: oxispec\n  content-hash: \""));
        assert!(text.contains("\n---\n\n# Explore the specs"));
        assert!(!text.contains("Project notes"));
    }

    #[test]
    fn project_notes_are_appended() {
        let text = render(&SKILLS[0], "Use the `billing` domain for money.\n");
        assert!(text.contains("\n## Project notes\n\nUse the `billing` domain for money.\n"));
    }

    #[test]
    fn states() {
        let skill = &SKILLS[0];
        let text = render(skill, "");
        assert_eq!(state(None, &text), State::Missing);
        assert_eq!(state(Some(&text), &text), State::Current);
        // A newer version of oxispec writes other text: still safe to replace.
        let newer = render(skill, "A new note.");
        assert_eq!(state(Some(&text), &newer), State::Outdated);
        // An edit by hand is never replaced silently.
        let edited = text.replace("Read what the project", "Read what our project");
        assert_ne!(edited, text);
        assert_eq!(state(Some(&edited), &text), State::Edited);
        // A file that oxispec did not write.
        assert_eq!(
            state(Some("---\nname: oxispec-explore\n---\nmine\n"), &text),
            State::Edited
        );
        assert_eq!(state(Some("just text"), &text), State::Edited);
    }

    #[test]
    fn line_endings_do_not_count_as_edits() {
        let text = render(&SKILLS[0], "");
        let crlf = text.replace('\n', "\r\n");
        assert_eq!(state(Some(&crlf), &text), State::Current);
        let newer = render(&SKILLS[0], "note");
        assert_eq!(state(Some(&crlf), &newer), State::Outdated);
    }

    #[test]
    fn default_clients_are_known() {
        for id in DEFAULT_CLIENTS {
            assert!(client(id).is_some(), "{id}");
        }
        assert!(client("vim").is_none());
    }
}
