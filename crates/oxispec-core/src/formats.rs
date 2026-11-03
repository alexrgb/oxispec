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

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::{self, CONFIG_PATH, Config};
use crate::finding::Finding;
use crate::graph::Graph;
use crate::node::Node;

/// The front matter fields a profile can require.
pub const FIELDS: &[&str] = &["status", "owner", "approved_by", "approved_on", "version"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    pub name: String,
    pub required: bool,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub statuses: Vec<String>,
}

fn sections(required: &[&str], optional: &[&str]) -> Vec<Section> {
    required
        .iter()
        .map(|n| (n, true))
        .chain(optional.iter().map(|n| (n, false)))
        .map(|(name, required)| Section {
            name: (*name).to_string(),
            required,
        })
        .collect()
}

/// The profiles that come with oxispec. Choosing one never makes an existing
/// project fail: they describe what `oxispec new` has always written.
pub fn builtin() -> Vec<Profile> {
    vec![
        Profile {
            name: "default".into(),
            ty: "adr".into(),
            description: "Context, Decision and Consequences, with nothing required".into(),
            builtin: true,
            sections: sections(&[], &["Context", "Decision", "Consequences"]),
            fields: Vec::new(),
            statuses: Vec::new(),
        },
        Profile {
            name: "shall".into(),
            ty: "requirement".into(),
            description: "A requirement with SHALL or MUST and scenarios (checks E006 to E008)"
                .into(),
            builtin: true,
            sections: Vec::new(),
            fields: Vec::new(),
            statuses: Vec::new(),
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
    if let Some(field) = custom.fields.iter().find(|f| !FIELDS.contains(&f.as_str())) {
        return Some(format!(
            "requires the field `{field}`; the fields are {}",
            FIELDS.join(", ")
        ));
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
    /// The text of a new node, with `@ID@` and `@TITLE@` still to fill in.
    pub fn template(&self) -> String {
        let status = self
            .statuses
            .first()
            .map(String::as_str)
            .unwrap_or(match self.ty.as_str() {
                "adr" => "proposed",
                _ => "draft",
            });
        let mut text = format!("---\nid: @ID@\ntype: {}\nstatus: {status}\n", self.ty);
        for field in self.fields.iter().filter(|f| f.as_str() != "status") {
            text.push_str(&format!("{field}: \n"));
        }
        text.push_str("---\n# @ID@: @TITLE@\n");
        for section in &self.sections {
            text.push_str(&format!("\n## {}\n", section.name));
        }
        text
    }
}

fn field<'a>(node: &'a Node, name: &str) -> Option<&'a str> {
    match name {
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
    out
}

#[cfg(test)]
mod tests_of_formats {
    use super::*;
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
}
