//! Format profiles: what a node of a type must contain.
//!
//! A profile names the sections (headings) and front matter fields a node must
//! have and the statuses it may take, and gives the template `oxispec new`
//! writes. A project chooses one profile per node type under `formats:` in
//! `.oxispec/config.yaml`, and can define its own under `profiles:`. A type with
//! no chosen profile is not checked beyond the structural checks.
//!
//! E028 a required section is missing
//! E029 a required front matter field is missing or empty
//! E030 the status is missing or not one of the profile's statuses
//! E031 the config names a profile that does not exist or is not valid
//! W011 a decision is marked superseded, but nothing supersedes it
//! W012 a decision is superseded by another, but is still marked live

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::{self, CONFIG_PATH, Config};
use crate::finding::Finding;
use crate::graph::Graph;
use crate::node::Node;
use crate::styles::Style;

/// The front matter fields a profile can require.
pub const FIELDS: &[&str] = &[
    "status",
    "owner",
    "approved_by",
    "approved_on",
    "version",
    "date",
    "decision-makers",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    pub name: String,
    pub required: bool,
    /// The heading level in the template: 2 (`##`) or 3 (`###`).
    pub level: u8,
    /// Text the template puts under the heading.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Profile {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub description: String,
    pub builtin: bool,
    pub sections: Vec<Section>,
    /// Front matter fields that must be set.
    pub fields: Vec<String>,
    /// Fields the template includes but `check` does not require.
    pub optional_fields: Vec<String>,
    /// How a requirement is worded, for requirement profiles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
    /// The statuses a node may have. Empty: any.
    pub statuses: Vec<String>,
}

/// A profile as written under `profiles:` in the config.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomProfile {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Required sections.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<String>,
    /// Sections that the template has but that are not required.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
    /// Fields the template includes but that are not required.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub statuses: Vec<String>,
    /// For a requirement profile: `ears`, `gherkin` or `user-story`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
}

fn sections(required: &[&str], optional: &[&str]) -> Vec<Section> {
    required
        .iter()
        .map(|n| (n, true))
        .chain(optional.iter().map(|n| (n, false)))
        .map(|(name, required)| Section {
            name: (*name).to_string(),
            required,
            level: 2,
            hint: String::new(),
        })
        .collect()
}

/// A section for a built-in profile: its heading level and the text under it.
fn section(name: &str, required: bool, level: u8, hint: &str) -> Section {
    Section {
        name: name.into(),
        required,
        level,
        hint: hint.into(),
    }
}

const WITH_REJECTED: &[&str] = &[
    "proposed",
    "accepted",
    "rejected",
    "deprecated",
    "superseded",
];

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

/// The profiles that come with oxispec. Choosing `default` or `shall` never
/// makes an existing project fail: they describe what `oxispec new` has always
/// written. The other ADR profiles are formats from outside, kept the same way
/// for every one of them: the status, date and deciders are front matter, and
/// who supersedes whom is a `supersedes` link, so a record can be converted
/// from one format to another without losing them.
pub fn builtin() -> Vec<Profile> {
    let adr = |name: &str, description: &str| Profile {
        name: name.into(),
        ty: "adr".into(),
        description: description.into(),
        builtin: true,
        sections: Vec::new(),
        fields: Vec::new(),
        optional_fields: Vec::new(),
        style: None,
        statuses: Vec::new(),
    };
    let requirement = |name: &str, description: &str, style: Option<Style>| Profile {
        name: name.into(),
        ty: "requirement".into(),
        description: description.into(),
        builtin: true,
        sections: Vec::new(),
        fields: Vec::new(),
        optional_fields: Vec::new(),
        style,
        statuses: Vec::new(),
    };
    vec![
        Profile {
            sections: sections(&[], &["Context", "Decision", "Consequences"]),
            ..adr(
                "default",
                "Context, Decision and Consequences, with nothing required",
            )
        },
        requirement(
            "shall",
            "A requirement with SHALL or MUST and scenarios (checks E006 to E008)",
            None,
        ),
        requirement(
            "ears",
            "A requirement in one of the EARS patterns: ubiquitous, state-driven, event-driven, optional feature, unwanted behaviour or complex",
            Some(Style::Ears),
        ),
        requirement(
            "gherkin",
            "A requirement whose scenarios are Gherkin: a WHEN and a THEN step, with GIVEN, AND and BUT",
            Some(Style::Gherkin),
        ),
        requirement(
            "user-story",
            "A user story, As a <role> I want <goal> so that <benefit>, with the scenarios as acceptance criteria",
            Some(Style::UserStory),
        ),
        Profile {
            sections: vec![
                section(
                    "Context",
                    true,
                    2,
                    "The forces at play, stated as neutral facts.",
                ),
                section("Decision", true, 2, "We will ..."),
                section(
                    "Consequences",
                    true,
                    2,
                    "What becomes easier or harder, and the risks to manage.",
                ),
            ],
            fields: strings(&["status"]),
            optional_fields: strings(&["date"]),
            statuses: strings(&["proposed", "accepted", "deprecated", "superseded"]),
            ..adr(
                "nygard",
                "Michael Nygard's ADR: Context, Decision, Consequences, with a status",
            )
        },
        Profile {
            sections: vec![
                section(
                    "Context and Problem Statement",
                    true,
                    2,
                    "{Describe the context and the problem, perhaps as a question.}",
                ),
                section("Decision Drivers", false, 2, "* {decision driver 1}"),
                section(
                    "Considered Options",
                    true,
                    2,
                    "* {title of option 1}\n* {title of option 2}",
                ),
                section(
                    "Decision Outcome",
                    true,
                    2,
                    "Chosen option: \"{title of option 1}\", because {justification}.",
                ),
                section(
                    "Consequences",
                    false,
                    3,
                    "* Good, because {positive consequence}\n* Bad, because {negative consequence}",
                ),
                section(
                    "Confirmation",
                    false,
                    3,
                    "{How the implementation of this decision is confirmed.}",
                ),
                section("Pros and Cons of the Options", false, 2, ""),
                section("More Information", false, 2, ""),
            ],
            fields: strings(&["status"]),
            optional_fields: strings(&["date", "decision-makers"]),
            statuses: strings(WITH_REJECTED),
            ..adr(
                "madr",
                "Markdown Architectural Decision Records 4.0, with considered options",
            )
        },
        Profile {
            sections: vec![section(
                "Statement",
                true,
                2,
                "In the context of {use case}, facing {concern}, we decided for {option} and neglected {other options}, to achieve {quality}, accepting {downside}, because {rationale}.",
            )],
            fields: strings(&["status"]),
            optional_fields: strings(&["date"]),
            statuses: strings(WITH_REJECTED),
            ..adr(
                "y-statement",
                "A decision in one sentence: context, concern, choice, neglected options, quality, downside",
            )
        },
    ]
}

/// The profiles a project can use: the built-in ones and its own valid ones.
/// A custom profile that is not valid is left out and reported as E031.
pub fn available(config: &Config) -> (Vec<Profile>, Vec<Finding>) {
    let mut profiles = builtin();
    let mut problems = Vec::new();
    let mut problem = |message: String| {
        problems.push(Finding::error("E031", message).at(Path::new(CONFIG_PATH), None));
    };
    for (name, custom) in &config.profiles {
        if let Some(reason) = invalid(name, custom, &profiles) {
            problem(format!("profile `{name}` {reason}"));
            continue;
        }
        let required: Vec<&str> = custom.sections.iter().map(String::as_str).collect();
        let optional: Vec<&str> = custom.optional.iter().map(String::as_str).collect();
        profiles.push(Profile {
            name: name.clone(),
            ty: custom.ty.clone(),
            description: custom.description.clone(),
            builtin: false,
            sections: sections(&required, &optional),
            fields: custom.fields.clone(),
            optional_fields: custom.optional_fields.clone(),
            style: custom.style.as_deref().and_then(Style::parse),
            statuses: custom.statuses.clone(),
        });
    }
    (profiles, problems)
}

/// Why a custom profile cannot be used, if it cannot.
fn invalid(name: &str, custom: &CustomProfile, taken: &[Profile]) -> Option<String> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Some("needs a name of letters, digits, `-` and `_`".into());
    }
    if taken.iter().any(|p| p.name == name) {
        return Some("has the name of a built-in profile; pick another".into());
    }
    if custom.ty.trim().is_empty() {
        return Some("needs a `type`".into());
    }
    let mut seen = BTreeSet::new();
    for section in custom.sections.iter().chain(&custom.optional) {
        if section.trim().is_empty() {
            return Some("has an empty section name".into());
        }
        if !seen.insert(section.to_lowercase()) {
            return Some(format!("lists the section `{section}` twice"));
        }
    }
    if let Some(field) = custom
        .fields
        .iter()
        .chain(&custom.optional_fields)
        .find(|f| !FIELDS.contains(&f.as_str()))
    {
        return Some(format!(
            "requires the field `{field}`; the fields are {}",
            FIELDS.join(", ")
        ));
    }
    if let Some(style) = &custom.style {
        if custom.ty != "requirement" {
            return Some("has a `style`, which only requirement profiles have".into());
        }
        if Style::parse(style).is_none() {
            return Some(format!(
                "has the style `{style}`; the styles are {}",
                Style::NAMES.join(", ")
            ));
        }
    }
    if custom.statuses.iter().any(|s| s.trim().is_empty()) {
        return Some("has an empty status".into());
    }
    None
}

/// The profile chosen for each node type, and what was wrong in the config.
#[derive(Debug, Default)]
pub struct Formats {
    chosen: BTreeMap<String, Profile>,
    problems: Vec<Finding>,
}

impl Formats {
    pub fn from_config(config: &Config) -> Formats {
        let (profiles, mut problems) = available(config);
        let mut chosen = BTreeMap::new();
        for (ty, name) in &config.formats {
            match profiles.iter().find(|p| &p.name == name) {
                None => problems.push(
                    Finding::error(
                        "E031",
                        format!(
                            "`formats` gives `{ty}` the profile `{name}`, which does not exist"
                        ),
                    )
                    .at(Path::new(CONFIG_PATH), None),
                ),
                Some(p) if &p.ty != ty => problems.push(
                    Finding::error(
                        "E031",
                        format!(
                            "`formats` gives `{ty}` the profile `{name}`, which is for `{}`",
                            p.ty
                        ),
                    )
                    .at(Path::new(CONFIG_PATH), None),
                ),
                Some(p) => {
                    chosen.insert(ty.clone(), p.clone());
                }
            }
        }
        Formats { chosen, problems }
    }

    /// The profiles of the project at `root`. A config that cannot be read
    /// gives none; other commands report that.
    pub fn load(root: &Path) -> Formats {
        config::load(root)
            .map(|c| Formats::from_config(&c))
            .unwrap_or_default()
    }

    pub fn profile_for(&self, ty: &str) -> Option<&Profile> {
        self.chosen.get(ty)
    }
}

impl Profile {
    /// The text of a new node, with `@ID@`, `@TITLE@` and `@DATE@` still to fill in.
    pub fn template(&self) -> String {
        if self.ty == "requirement" {
            return self.requirement_template();
        }
        let status = self
            .statuses
            .first()
            .map(String::as_str)
            .unwrap_or(match self.ty.as_str() {
                "adr" => "proposed",
                _ => "draft",
            });
        let mut text = format!("---\nid: @ID@\ntype: {}\nstatus: {status}\n", self.ty);
        for field in self
            .fields
            .iter()
            .chain(&self.optional_fields)
            .filter(|f| f.as_str() != "status")
        {
            if field == "date" {
                text.push_str("date: @DATE@\n");
            } else {
                text.push_str(&format!("{field}: \n"));
            }
        }
        text.push_str("---\n# @ID@: @TITLE@\n");
        for section in &self.sections {
            let marks = "#".repeat(usize::from(section.level));
            text.push_str(&format!("\n{marks} {}\n", section.name));
            if !section.hint.is_empty() {
                text.push_str(&format!("\n{}\n", section.hint));
            }
        }
        text
    }
}

/// What the requirement templates of each style say.
const SHALL_BODY: &str = "### Requirement: @TITLE@
The system SHALL <describe the behavior>.

#### Scenario: <name> {#S1}
- **WHEN** <condition>
- **THEN** <outcome>
";

const EARS_BODY: &str = "### Requirement: @TITLE@
When <trigger>, the <system> shall <response>.

#### Scenario: <name> {#S1}
- **WHEN** <trigger>
- **THEN** <response>
";

const GHERKIN_BODY: &str = "### Requirement: @TITLE@
The system SHALL <describe the behavior>.

#### Scenario: <name> {#S1}
- **GIVEN** <context>
- **WHEN** <action>
- **THEN** <outcome>
";

const USER_STORY_BODY: &str = "### Requirement: @TITLE@
As a <role>, I want <goal>, so that <benefit>.

#### Scenario: <acceptance criterion> {#S1}
- **GIVEN** <context>
- **WHEN** <action>
- **THEN** <outcome>
";

impl Profile {
    /// The text of a new requirement: front matter, the requirement in the
    /// style, and the profile's own sections after it.
    fn requirement_template(&self) -> String {
        let mut text = String::from("---\nid: @ID@\ntype: requirement\nstatus: ");
        text.push_str(self.statuses.first().map_or("draft", String::as_str));
        text.push('\n');
        for field in self
            .fields
            .iter()
            .chain(&self.optional_fields)
            .filter(|f| f.as_str() != "status")
        {
            if field == "date" {
                text.push_str("date: @DATE@\n");
            } else {
                text.push_str(&format!("{field}: \n"));
            }
        }
        text.push_str("---\n");
        text.push_str(match self.style {
            None => SHALL_BODY,
            Some(Style::Ears) => EARS_BODY,
            Some(Style::Gherkin) => GHERKIN_BODY,
            Some(Style::UserStory) => USER_STORY_BODY,
        });
        for section in &self.sections {
            let marks = "#".repeat(usize::from(section.level));
            text.push_str(&format!("\n{marks} {}\n", section.name));
            if !section.hint.is_empty() {
                text.push_str(&format!("\n{}\n", section.hint));
            }
        }
        text
    }
}

fn field<'a>(node: &'a Node, name: &str) -> Option<&'a str> {
    match name {
        "date" => node.date.as_deref(),
        "decision-makers" => node
            .decision_makers
            .iter()
            .map(String::as_str)
            .find(|s| !s.trim().is_empty()),
        "status" => node.status.as_deref(),
        "owner" => node.owner.as_deref(),
        "approved_by" => node.approved_by.as_deref(),
        "approved_on" => node.approved_on.as_deref(),
        "version" => node.version.as_deref(),
        _ => None,
    }
}

pub fn check(graph: &Graph) -> Vec<Finding> {
    let formats = &graph.formats;
    let mut out = formats.problems.clone();
    if formats.chosen.is_empty() {
        return out;
    }
    for node in graph.nodes.iter() {
        let Some(profile) = formats.profile_for(&node.ty) else {
            continue;
        };
        let at = |f: Finding| f.at(&node.path, Some(1)).node(&node.id);
        if let Some(style) = profile.style {
            out.extend(crate::styles::check(node, style));
        }
        for section in profile.sections.iter().filter(|s| s.required) {
            if !node
                .headings
                .iter()
                .any(|h| h.text.eq_ignore_ascii_case(&section.name))
            {
                out.push(at(Finding::error(
                    "E028",
                    format!(
                        "{} has no `{}` section, which the profile `{}` requires",
                        node.ty, section.name, profile.name
                    ),
                )));
            }
        }
        for name in &profile.fields {
            if field(node, name).is_none_or(|v| v.trim().is_empty()) {
                out.push(at(Finding::error(
                    "E029",
                    format!(
                        "{} has no `{name}`, which the profile `{}` requires",
                        node.ty, profile.name
                    ),
                )));
            }
        }
        if !profile.statuses.is_empty() {
            let known = profile.statuses.join(", ");
            match node.status.as_deref() {
                Some(s) if profile.statuses.iter().any(|k| k == s) => {}
                Some(s) => out.push(at(Finding::error(
                    "E030",
                    format!(
                        "{} status {s:?} is not one of {known} (profile `{}`)",
                        node.ty, profile.name
                    ),
                ))),
                None => out.push(at(Finding::error(
                    "E030",
                    format!(
                        "{} has no `status`; the profile `{}` allows {known}",
                        node.ty, profile.name
                    ),
                ))),
            }
        }
    }
    superseded(graph, &mut out);
    out
}

/// Decisions whose profile has the `superseded` status are held to it: a
/// decision is superseded when a decision that is in force links `supersedes`
/// to it, and not otherwise. A successor that is still proposed or was rejected
/// has not replaced anything yet.
fn superseded(graph: &Graph, out: &mut Vec<Finding>) {
    let uses_it = |node: &Node| {
        graph
            .formats
            .profile_for(&node.ty)
            .is_some_and(|p| p.statuses.iter().any(|s| s == "superseded"))
    };
    let mut replaced_by: BTreeMap<&str, &str> = BTreeMap::new();
    for node in graph.nodes.iter() {
        if matches!(node.status.as_deref(), Some("proposed" | "rejected")) {
            continue;
        }
        for target in node.links.get("supersedes").into_iter().flatten() {
            replaced_by.entry(target.as_str()).or_insert(&node.id);
        }
    }
    for node in graph.nodes.iter().filter(|n| uses_it(n)) {
        let at = |f: Finding| f.at(&node.path, Some(1)).node(&node.id);
        let status = node.status.as_deref();
        match (status, replaced_by.get(node.id.as_str())) {
            (Some("superseded"), None) => out.push(at(Finding::warning(
                "W011",
                format!(
                    "{} is marked superseded, but no decision in force links `supersedes` to it",
                    node.ty
                ),
            ))),
            (Some("proposed" | "accepted"), Some(by)) => out.push(at(Finding::warning(
                "W012",
                format!(
                    "{} is superseded by {by}, but its status is {}; set it to superseded",
                    node.ty,
                    status.unwrap_or_default()
                ),
            ))),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests_of_formats {
    use super::*;

    #[test]
    fn the_requirement_profiles_have_a_style_and_a_template_that_passes_its_own_check() {
        let all = builtin();
        for (name, style) in [
            ("shall", None),
            ("ears", Some(Style::Ears)),
            ("gherkin", Some(Style::Gherkin)),
            ("user-story", Some(Style::UserStory)),
        ] {
            let p = all.iter().find(|p| p.name == name).unwrap();
            assert_eq!(p.style, style, "{name}");
            let text = p
                .template()
                .replace("@ID@", "REQ-A-0001")
                .replace("@TITLE@", "Thing");
            let (node, _) = crate::parse::parse_file_text(Path::new("r.md"), &text);
            let node = node.unwrap();
            if let Some(style) = p.style {
                assert!(crate::styles::check(&node, style).is_empty(), "{name}");
            }
            assert_eq!(node.requirements.len(), 1, "{name}");
            assert_eq!(node.requirements[0].scenarios.len(), 1, "{name}");
        }
    }

    #[test]
    fn a_custom_requirement_profile_can_name_a_style_and_nothing_else_can() {
        let mut config = Config::new(vec!["core".into()], config::IdScheme::Sequential);
        config.profiles.insert(
            "strict".into(),
            CustomProfile {
                ty: "requirement".into(),
                style: Some("ears".into()),
                sections: vec!["Rationale".into()],
                ..CustomProfile::default()
            },
        );
        config.profiles.insert(
            "odd".into(),
            CustomProfile {
                ty: "requirement".into(),
                style: Some("haiku".into()),
                ..CustomProfile::default()
            },
        );
        config.profiles.insert(
            "wrong".into(),
            CustomProfile {
                ty: "adr".into(),
                style: Some("ears".into()),
                ..CustomProfile::default()
            },
        );
        let (profiles, problems) = available(&config);
        let strict = profiles.iter().find(|p| p.name == "strict").unwrap();
        assert_eq!(strict.style, Some(Style::Ears));
        assert!(strict.template().contains("## Rationale"));
        assert!(
            !profiles
                .iter()
                .any(|p| p.name == "odd" || p.name == "wrong")
        );
        assert_eq!(problems.len(), 2);
    }
    use crate::config::IdScheme;
    use std::path::PathBuf;

    fn config(yaml: &str) -> Config {
        let base = "format: 1\nmodules: [core, specs, adrs]\n";
        serde_saphyr::from_str(&format!("{base}{yaml}")).expect("a config")
    }

    fn graph(config: &Config, files: &[(&str, &str)]) -> Graph {
        Graph::from_sources(
            files
                .iter()
                .map(|(path, text)| (PathBuf::from(path), text.as_bytes().to_vec())),
        )
        .with_formats(Formats::from_config(config))
    }

    fn codes(graph: &Graph) -> Vec<&'static str> {
        check(graph).iter().map(|f| f.code).collect()
    }

    const LIGHT: &str = "formats:\n  adr: light\nprofiles:\n  light:\n    type: adr\n    sections: [Context, Decision]\n    optional: [Consequences]\n    fields: [owner]\n    statuses: [proposed, accepted]\n";

    #[test]
    fn a_node_that_meets_its_profile_has_no_findings() {
        let cfg = config(LIGHT);
        let g = graph(
            &cfg,
            &[(
                "specs/adr/a.md",
                "---\nid: ADR-0001\ntype: adr\nstatus: accepted\nowner: ana\n---\n# ADR-0001: A\n\n## context\n\n## Decision\n",
            )],
        );
        assert!(codes(&g).is_empty(), "{:?}", check(&g));
    }

    #[test]
    fn a_missing_section_field_and_status_are_reported_with_their_own_codes() {
        let cfg = config(LIGHT);
        let g = graph(
            &cfg,
            &[(
                "specs/adr/a.md",
                "---\nid: ADR-0001\ntype: adr\nstatus: maybe\n---\n# ADR-0001: A\n\n## Context\n",
            )],
        );
        assert_eq!(codes(&g), ["E028", "E029", "E030"]);
        let g = graph(
            &cfg,
            &[(
                "specs/adr/a.md",
                "---\nid: ADR-0001\ntype: adr\nowner: ana\n---\n# ADR-0001: A\n\n## Context\n\n## Decision\n",
            )],
        );
        assert_eq!(codes(&g), ["E030"], "a missing status is E030");
    }

    #[test]
    fn a_heading_in_a_code_block_is_not_a_section() {
        let cfg = config(LIGHT);
        let g = graph(
            &cfg,
            &[(
                "specs/adr/a.md",
                "---\nid: ADR-0001\ntype: adr\nstatus: proposed\nowner: a\n---\n# ADR-0001: A\n\n## Context\n\n```\n## Decision\n```\n",
            )],
        );
        assert_eq!(codes(&g), ["E028"]);
    }

    #[test]
    fn types_without_a_chosen_profile_are_not_checked() {
        let cfg = config(LIGHT);
        let g = graph(
            &cfg,
            &[(
                "specs/x.md",
                "---\nid: STD-0001\ntype: standard\n---\n# STD-0001: S\n",
            )],
        );
        assert!(codes(&g).is_empty());
    }

    #[test]
    fn choosing_a_built_in_profile_never_fails_an_existing_project() {
        let cfg = config("formats:\n  adr: default\n  requirement: shall\n");
        let g = graph(
            &cfg,
            &[(
                "specs/adr/a.md",
                "---\nid: ADR-0001\ntype: adr\n---\n# ADR-0001: A\n",
            )],
        );
        assert!(codes(&g).is_empty());
    }

    #[test]
    fn a_config_that_names_a_missing_or_wrong_profile_is_reported() {
        let cfg = config("formats:\n  adr: nope\n  requirement: default\n");
        let g = graph(&cfg, &[]);
        let found = check(&g);
        assert_eq!(
            found.iter().map(|f| f.code).collect::<Vec<_>>(),
            ["E031", "E031"]
        );
        assert!(found[0].message.contains("does not exist"));
        assert!(found[1].message.contains("is for `adr`"));
        assert_eq!(found[0].path.as_deref(), Some(Path::new(CONFIG_PATH)));
    }

    #[test]
    fn a_custom_profile_that_is_not_valid_is_left_out_and_reported() {
        let cases = [
            ("a", "type: adr\n    sections: [X, x]\n", "twice"),
            ("b", "type: adr\n    fields: [color]\n", "color"),
            ("default", "type: adr\n", "built-in"),
            ("c", "sections: [X]\n    type: ''\n", "needs a `type`"),
            ("d e", "type: adr\n", "name"),
        ];
        for (name, body, reason) in cases {
            let cfg = config(&format!("profiles:\n  '{name}':\n    {body}"));
            let (profiles, problems) = available(&cfg);
            assert_eq!(profiles.len(), builtin().len(), "{name}");
            assert_eq!(problems.len(), 1, "{name}");
            assert!(
                problems[0].message.contains(reason),
                "{name}: {:?}",
                problems[0]
            );
        }
    }

    #[test]
    fn a_template_has_the_status_the_fields_and_every_section() {
        let cfg = config(LIGHT);
        let (profiles, _) = available(&cfg);
        let light = profiles.iter().find(|p| p.name == "light").unwrap();
        assert_eq!(
            light.template(),
            "---\nid: @ID@\ntype: adr\nstatus: proposed\nowner: \n---\n# @ID@: @TITLE@\n\n## Context\n\n## Decision\n\n## Consequences\n"
        );
        // The built-in ADR profile writes what `new adr` always wrote.
        let default = profiles.iter().find(|p| p.name == "default").unwrap();
        assert_eq!(
            default.template(),
            "---\nid: @ID@\ntype: adr\nstatus: proposed\n---\n# @ID@: @TITLE@\n\n## Context\n\n## Decision\n\n## Consequences\n"
        );
    }

    #[test]
    fn profile_names_are_unique_and_a_config_survives_a_round_trip() {
        let names: BTreeSet<_> = builtin().into_iter().map(|p| p.name).collect();
        assert_eq!(names.len(), builtin().len());
        let cfg = config(LIGHT);
        let text = serde_saphyr::to_string(&cfg).unwrap();
        let back: Config = serde_saphyr::from_str(&text).unwrap();
        assert_eq!(back, cfg);
        let _ = IdScheme::Sequential;
    }

    fn adr(id: &str, status: &str, links: &str) -> (String, String) {
        (
            format!("specs/adr/{id}.md"),
            format!(
                "---\nid: {id}\ntype: adr\nstatus: {status}\n{links}---\n# {id}: T\n\n## Context\n\n## Decision\n\n## Consequences\n\n## Context and Problem Statement\n\n## Considered Options\n\n## Decision Outcome\n"
            ),
        )
    }

    fn superseded_codes(chosen: &str, adrs: &[(String, String)]) -> Vec<(&'static str, String)> {
        let cfg = config(&format!("formats:\n  adr: {chosen}\n"));
        let files: Vec<(&str, &str)> = adrs.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
        let g = graph(&cfg, &files);
        check(&g)
            .into_iter()
            .map(|f| (f.code, f.id.unwrap_or_default()))
            .collect()
    }

    #[test]
    fn every_built_in_template_meets_its_own_profile() {
        for profile in builtin().iter().filter(|p| p.ty == "adr") {
            let text = profile
                .template()
                .replace("@ID@", "ADR-0001")
                .replace("@TITLE@", "T")
                .replace("@DATE@", "2026-11-03");
            let cfg = config(&format!("formats:\n  adr: {}\n", profile.name));
            let g = graph(&cfg, &[("specs/adr/a.md", text.as_str())]);
            assert!(
                check(&g).is_empty(),
                "{}: {:?}\n{text}",
                profile.name,
                check(&g)
            );
            assert_eq!(
                g.nodes[0].date.as_deref().is_some(),
                profile.optional_fields.contains(&"date".into())
            );
        }
    }

    #[test]
    fn the_three_adr_formats_share_one_status_vocabulary() {
        let profiles = builtin();
        let statuses = |name: &str| {
            profiles
                .iter()
                .find(|p| p.name == name)
                .unwrap()
                .statuses
                .clone()
        };
        for name in ["nygard", "madr", "y-statement"] {
            let s = statuses(name);
            for core in ["proposed", "accepted", "deprecated", "superseded"] {
                assert!(s.iter().any(|x| x == core), "{name} lacks {core}");
            }
        }
        assert!(statuses("madr").contains(&"rejected".to_string()));
        assert!(!statuses("nygard").contains(&"rejected".to_string()));
    }

    #[test]
    fn madr_needs_its_options_and_outcome_and_nygard_does_not() {
        let thin = "---\nid: ADR-0001\ntype: adr\nstatus: accepted\n---\n# ADR-0001: T\n\n## Context\n\n## Decision\n\n## Consequences\n";
        let nygard = config("formats:\n  adr: nygard\n");
        assert!(check(&graph(&nygard, &[("specs/adr/a.md", thin)])).is_empty());
        let madr = config("formats:\n  adr: madr\n");
        let found = check(&graph(&madr, &[("specs/adr/a.md", thin)]));
        assert_eq!(
            found.iter().map(|f| f.code).collect::<Vec<_>>(),
            ["E028"; 3]
        );
        assert!(
            found
                .iter()
                .any(|f| f.message.contains("Considered Options"))
        );
    }

    #[test]
    fn a_decision_marked_superseded_needs_a_successor_in_force() {
        let old = adr("ADR-0001", "superseded", "");
        assert_eq!(
            superseded_codes("nygard", std::slice::from_ref(&old)),
            [("W011", "ADR-0001".into())]
        );
        let new = adr("ADR-0002", "accepted", "links:\n  supersedes: ADR-0001\n");
        assert!(superseded_codes("nygard", &[old, new]).is_empty());
    }

    #[test]
    fn a_decision_with_a_successor_in_force_must_say_it_is_superseded() {
        let old = adr("ADR-0001", "accepted", "");
        let new = adr("ADR-0002", "accepted", "links:\n  supersedes: ADR-0001\n");
        assert_eq!(
            superseded_codes("madr", &[old.clone(), new]),
            [("W012", "ADR-0001".into())]
        );
        // A proposed or rejected successor has replaced nothing yet.
        for status in ["proposed", "rejected"] {
            let pending = adr("ADR-0002", status, "links:\n  supersedes: ADR-0001\n");
            assert!(
                superseded_codes("madr", &[old.clone(), pending]).is_empty(),
                "{status}"
            );
        }
        // A decision that is already deprecated is left alone.
        let deprecated = adr("ADR-0001", "deprecated", "");
        let new = adr("ADR-0002", "accepted", "links:\n  supersedes: ADR-0001\n");
        assert!(superseded_codes("madr", &[deprecated, new]).is_empty());
    }

    #[test]
    fn projects_without_the_superseded_status_see_no_such_warnings() {
        let old = adr("ADR-0001", "accepted", "");
        let new = adr("ADR-0002", "accepted", "links:\n  supersedes: ADR-0001\n");
        // The built-in `default` profile has no statuses; so does having no profile at all.
        assert!(superseded_codes("default", &[old.clone(), new.clone()]).is_empty());
        let none = graph(
            &config(""),
            &[("a.md", old.1.as_str()), ("b.md", new.1.as_str())],
        );
        assert!(check(&none).is_empty());
    }

    #[test]
    fn date_and_decision_makers_are_read_and_can_be_required() {
        let cfg = config(
            "formats:\n  adr: strict\nprofiles:\n  strict:\n    type: adr\n    fields: [date, decision-makers]\n",
        );
        let ok = "---\nid: ADR-0001\ntype: adr\ndate: 2026-11-03\ndecision-makers: [ana, bo]\n---\n# ADR-0001: T\n";
        let g = graph(&cfg, &[("a.md", ok)]);
        assert!(check(&g).is_empty(), "{:?}", check(&g));
        assert_eq!(g.nodes[0].decision_makers, ["ana", "bo"]);
        assert_eq!(g.nodes[0].date.as_deref(), Some("2026-11-03"));
        let one = "---\nid: ADR-0001\ntype: adr\ndate: 2026-11-03\ndecision-makers: ana\n---\n# ADR-0001: T\n";
        assert_eq!(
            graph(&cfg, &[("a.md", one)]).nodes[0].decision_makers,
            ["ana"]
        );
        let none = "---\nid: ADR-0001\ntype: adr\ndecision-makers:\n---\n# ADR-0001: T\n";
        let codes: Vec<_> = check(&graph(&cfg, &[("a.md", none)]))
            .iter()
            .map(|f| f.code)
            .collect();
        assert_eq!(codes, ["E029", "E029"]);
    }
}
