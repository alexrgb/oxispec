//! Requirement styles: ways to word a requirement and its scenarios.
//!
//! A style belongs to a format profile for requirements (`formats: {requirement:
//! ears}` in the config). The graph does not change with the style: a
//! requirement is still a `### Requirement:` block with scenarios, so links and
//! coverage work the same. A style only adds a check on the wording.
//!
//! E033 a requirement is not worded the way its style asks
//! E034 a scenario does not have the steps its style asks
//!
//! - `shall`: the default. SHALL or MUST in the description, and a scenario.
//! - `ears`: the Easy Approach to Requirements Syntax. One of the six patterns
//!   below, ending in `shall <response>`, in any case.
//! - `gherkin`: every scenario has a WHEN and a THEN step, and may have GIVEN,
//!   AND and BUT. A `gherkin` fence with `Scenario:` lines counts as scenarios too.
//! - `user-story`: `As a <role>, I want <goal>, so that <benefit>`, with the
//!   scenarios as its acceptance criteria. It needs no SHALL.

use serde::{Deserialize, Serialize};

use crate::finding::Finding;
use crate::node::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Style {
    #[serde(rename = "ears")]
    Ears,
    #[serde(rename = "gherkin")]
    Gherkin,
    #[serde(rename = "user-story")]
    UserStory,
}

impl Style {
    pub const NAMES: &'static [&'static str] = &["ears", "gherkin", "user-story"];

    pub fn name(self) -> &'static str {
        match self {
            Style::Ears => "ears",
            Style::Gherkin => "gherkin",
            Style::UserStory => "user-story",
        }
    }

    pub fn parse(name: &str) -> Option<Style> {
        match name {
            "ears" => Some(Style::Ears),
            "gherkin" => Some(Style::Gherkin),
            "user-story" => Some(Style::UserStory),
            _ => None,
        }
    }

    /// True when `check` still asks for SHALL or MUST written in capitals
    /// (E007). EARS has its own test for `shall`, in any case, and a user story
    /// has none.
    pub fn needs_normative(self) -> bool {
        self == Style::Gherkin
    }
}

/// The six EARS patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ears {
    /// `The <system> shall <response>`
    Ubiquitous,
    /// `While <state>, the <system> shall <response>`
    StateDriven,
    /// `When <trigger>, the <system> shall <response>`
    EventDriven,
    /// `Where <feature>, the <system> shall <response>`
    OptionalFeature,
    /// `If <condition>, then the <system> shall <response>`
    UnwantedBehaviour,
    /// Two or more of the keywords, for example `While <state>, when <trigger>, ...`
    Complex,
}

impl Ears {
    pub fn name(self) -> &'static str {
        match self {
            Ears::Ubiquitous => "ubiquitous",
            Ears::StateDriven => "state-driven",
            Ears::EventDriven => "event-driven",
            Ears::OptionalFeature => "optional-feature",
            Ears::UnwantedBehaviour => "unwanted-behaviour",
            Ears::Complex => "complex",
        }
    }
}

const KEYWORDS: &[&str] = &["while", "when", "if", "where"];

/// The byte position of `word` as a whole word in `text`.
fn find_word(text: &str, word: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(found) = text[from..].find(word) {
        let at = from + found;
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        if before.is_none_or(|c| !c.is_alphanumeric()) && after.is_none_or(|c| !c.is_alphanumeric())
        {
            return Some(at);
        }
        from = at + word.len();
    }
    None
}

fn leading_keyword(segment: &str) -> Option<&'static str> {
    KEYWORDS.iter().copied().find(|k| {
        segment
            .strip_prefix(*k)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    })
}

/// Which EARS pattern a requirement follows, or why it follows none.
pub fn ears(statement: &str) -> Result<Ears, String> {
    let lower = statement.trim().to_lowercase();
    let found = [
        ("shall", find_word(&lower, "shall")),
        ("must", find_word(&lower, "must")),
    ]
    .into_iter()
    .filter_map(|(word, at)| at.map(|at| (word, at)))
    .min_by_key(|(_, at)| *at);
    let Some((word, at)) = found else {
        return Err("it has no `shall`".into());
    };
    if lower[at + word.len()..].trim().is_empty() {
        return Err(format!("nothing follows `{word}`"));
    }
    let head = lower[..at].trim();
    let segments: Vec<&str> = head.split(',').map(str::trim).collect();
    let Some(first) = leading_keyword(segments[0]) else {
        return if head.is_empty() {
            Err("no system before `shall`".into())
        } else {
            Ok(Ears::Ubiquitous)
        };
    };
    if segments.len() == 1 {
        return Err(format!(
            "the `{first}` clause needs a comma before the system, as in `{} <...>, the <system> shall <response>`",
            capitalised(first)
        ));
    }
    let last = segments[segments.len() - 1];
    let subject = last.strip_prefix("then ").unwrap_or(last).trim();
    if subject.is_empty() {
        return Err("no system between the condition and `shall`".into());
    }
    let mut keywords = Vec::new();
    for segment in &segments[..segments.len() - 1] {
        if let Some(k) = leading_keyword(segment) {
            if segment.len() == k.len() {
                return Err(format!("`{k}` has no condition after it"));
            }
            keywords.push(k);
        }
    }
    Ok(match keywords.as_slice() {
        ["while"] => Ears::StateDriven,
        ["when"] => Ears::EventDriven,
        ["where"] => Ears::OptionalFeature,
        ["if"] => Ears::UnwantedBehaviour,
        _ => Ears::Complex,
    })
}

fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

/// Checks `As a <role>, I want <goal>, so that <benefit>`.
pub fn user_story(statement: &str) -> Result<(), String> {
    let lower = statement.trim().to_lowercase();
    let rest = lower
        .strip_prefix("as an ")
        .or_else(|| lower.strip_prefix("as a "))
        .ok_or("it does not start with `As a <role>`")?;
    let want = [" i want", " i need", " i can"]
        .iter()
        .filter_map(|w| rest.find(w).map(|at| (at, w.len())))
        .min_by_key(|(at, _)| *at)
        .ok_or("it has no `I want <goal>`")?;
    if rest[..want.0].trim_matches([',', ' ']).is_empty() {
        return Err("it names no role after `As a`".into());
    }
    let after = &rest[want.0 + want.1..];
    let so = after
        .find("so that")
        .ok_or("it has no `so that <benefit>`")?;
    if after[..so].trim_matches([',', ' ']).is_empty() {
        return Err("it names no goal after `I want`".into());
    }
    if after[so + "so that".len()..]
        .trim_matches([',', '.', ' '])
        .is_empty()
    {
        return Err("it names no benefit after `so that`".into());
    }
    Ok(())
}

/// Checks the step keywords of one scenario: it needs a WHEN and a THEN.
pub fn gherkin_steps(steps: &[String]) -> Result<(), String> {
    let missing: Vec<&str> = ["when", "then"]
        .into_iter()
        .filter(|k| !steps.iter().any(|s| s == k))
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "it has no {} step",
            missing
                .iter()
                .map(|k| k.to_uppercase())
                .collect::<Vec<_>>()
                .join(" or ")
        ))
    }
}

/// The findings for one requirement node under a style.
pub fn check(node: &Node, style: Style) -> Vec<Finding> {
    let mut out = Vec::new();
    for r in &node.requirements {
        let wording = match style {
            Style::Ears => ears(&r.statement).map(|_| ()),
            Style::UserStory => user_story(&r.statement),
            Style::Gherkin => Ok(()),
        };
        if let Err(why) = wording {
            let ask = match style {
                Style::Ears => "an EARS pattern",
                _ => "the user story form `As a <role>, I want <goal>, so that <benefit>`",
            };
            out.push(
                Finding::error(
                    "E033",
                    format!("requirement {:?} is not worded as {ask}: {why}", r.name),
                )
                .at(&node.path, Some(r.line))
                .node(&node.id),
            );
        }
        if style == Style::Gherkin {
            for s in &r.scenarios {
                if let Err(why) = gherkin_steps(&s.steps) {
                    out.push(
                        Finding::error(
                            "E034",
                            format!("scenario {:?} is not a Gherkin scenario: {why}", s.name),
                        )
                        .at(&node.path, Some(s.line))
                        .node(&node.id),
                    );
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests_of_styles {
    use super::*;

    #[test]
    fn the_six_ears_patterns_are_told_apart() {
        for (text, want) in [
            ("The system shall log every login.", Ears::Ubiquitous),
            (
                "While the door is open, the alarm shall stay silent.",
                Ears::StateDriven,
            ),
            (
                "When the user presses stop, the player shall pause.",
                Ears::EventDriven,
            ),
            (
                "Where a camera is fitted, the app shall offer a scan.",
                Ears::OptionalFeature,
            ),
            (
                "If the card is declined, then the shop shall ask for another.",
                Ears::UnwantedBehaviour,
            ),
            (
                "While in standby, when the lid opens, the laptop shall wake.",
                Ears::Complex,
            ),
        ] {
            assert_eq!(ears(text), Ok(want), "{text}");
        }
    }

    #[test]
    fn ears_accepts_must_a_missing_then_and_any_case() {
        assert_eq!(
            ears("IF the link drops, the client MUST retry."),
            Ok(Ears::UnwantedBehaviour)
        );
        assert_eq!(
            ears("When A, B and C happen, the system shall do D."),
            Ok(Ears::EventDriven)
        );
    }

    #[test]
    fn ears_says_what_is_wrong() {
        for (text, part) in [
            ("The system does it.", "no `shall`"),
            ("The system shall", "nothing follows"),
            ("Shall do it.", "no system"),
            (
                "When the user presses stop the player shall pause.",
                "needs a comma",
            ),
            ("When , the player shall pause.", "no condition"),
            ("When the user presses stop, shall pause.", "no system"),
        ] {
            let why = ears(text).unwrap_err();
            assert!(why.contains(part), "{text}: {why}");
        }
    }

    #[test]
    fn shall_inside_a_word_is_not_shall() {
        assert!(ears("The marshall command prints.").is_err());
    }

    #[test]
    fn a_user_story_needs_a_role_a_goal_and_a_benefit() {
        assert_eq!(
            user_story("As an admin, I want to lock accounts, so that leaks stay small."),
            Ok(())
        );
        assert_eq!(
            user_story("As a user I can export my data so that I can leave."),
            Ok(())
        );
        for (text, part) in [
            ("I want to lock accounts.", "As a <role>"),
            ("As a user, lock accounts.", "I want"),
            ("As a , I want x, so that y.", "no role"),
            ("As a user, I want x.", "so that"),
            ("As a user, I want , so that y.", "no goal"),
            ("As a user, I want x, so that .", "no benefit"),
        ] {
            let why = user_story(text).unwrap_err();
            assert!(why.contains(part), "{text}: {why}");
        }
    }

    #[test]
    fn a_gherkin_scenario_needs_when_and_then() {
        let steps = |s: &[&str]| s.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            gherkin_steps(&steps(&["given", "when", "then", "and"])),
            Ok(())
        );
        assert_eq!(gherkin_steps(&steps(&["when", "then"])), Ok(()));
        assert!(
            gherkin_steps(&steps(&["given", "then"]))
                .unwrap_err()
                .contains("WHEN")
        );
        assert!(gherkin_steps(&[]).unwrap_err().contains("WHEN or THEN"));
    }
}
