//! IDs that a project writes in its own shape, and IDs of things outside the project.
//!
//! `ids:` in the config names patterns (`TC-[A-Z]+-[0-9]+`): an ID that matches is a known
//! reference even when no node has it, so a link to it is not dangling and `trace` lists where it
//! is named. `external:` does the same for the keys of another system, such as Jira, and can say how
//! to build a link to one. A range such as `TC-CAT-1200..1212` stands for the IDs between its ends.

use std::collections::BTreeMap;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// The most IDs one range may stand for, so that a typo does not make a million of them.
pub const MAX_RANGE: u64 = 1000;

/// A kind of ID that belongs to another system.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalConfig {
    /// A regular expression the whole ID matches, for example `KAN-[0-9]+`.
    pub pattern: String,
    /// Where the item lives, with `{id}` for the ID: `https://example.atlassian.net/browse/{id}`.
    /// Without it, an ID is shown as text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid pattern in `ids.{0}`: {1}")]
    Ids(String, String),
    #[error("invalid pattern in `external.{0}`: {1}")]
    External(String, String),
    #[error("`external.{0}.url` has no `{{id}}` to put the ID in")]
    NoPlaceholder(String),
    #[error("`external.{0}.url` must start with http:// or https://")]
    NotHttp(String),
}

/// The patterns of a project, compiled.
#[derive(Debug, Default)]
pub struct Refs {
    ids: Vec<(String, Regex)>,
    external: Vec<(String, Regex, Option<String>)>,
}

fn whole(pattern: &str) -> Result<Regex, regex::Error> {
    Regex::new(&format!("^(?:{pattern})$"))
}

impl Refs {
    // spec: REQ-IDS-0001
    pub fn new(
        ids: &BTreeMap<String, String>,
        external: &BTreeMap<String, ExternalConfig>,
    ) -> Result<Refs, Error> {
        let mut refs = Refs::default();
        for (name, pattern) in ids {
            let re = whole(pattern).map_err(|e| Error::Ids(name.clone(), e.to_string()))?;
            refs.ids.push((name.clone(), re));
        }
        for (name, cfg) in external {
            let re =
                whole(&cfg.pattern).map_err(|e| Error::External(name.clone(), e.to_string()))?;
            if let Some(url) = &cfg.url {
                if !url.contains("{id}") {
                    return Err(Error::NoPlaceholder(name.clone()));
                }
                if !(url.starts_with("https://") || url.starts_with("http://")) {
                    return Err(Error::NotHttp(name.clone()));
                }
            }
            refs.external.push((name.clone(), re, cfg.url.clone()));
        }
        Ok(refs)
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty() && self.external.is_empty()
    }

    /// The name of the pattern the ID matches, if any.
    pub fn kind(&self, id: &str) -> Option<&str> {
        self.ids
            .iter()
            .map(|(n, re)| (n, re))
            .chain(self.external.iter().map(|(n, re, _)| (n, re)))
            .find(|(_, re)| re.is_match(id))
            .map(|(n, _)| n.as_str())
    }

    pub fn matches(&self, id: &str) -> bool {
        self.kind(id).is_some()
    }

    /// Whether the ID is one of another system.
    pub fn is_external(&self, id: &str) -> bool {
        self.external.iter().any(|(_, re, _)| re.is_match(id))
    }

    /// The link for an external ID, when its kind has a URL.
    // spec: REQ-IDS-0003
    pub fn url(&self, id: &str) -> Option<String> {
        self.external
            .iter()
            .find(|(_, re, _)| re.is_match(id))
            .and_then(|(_, _, url)| url.as_ref())
            .map(|u| u.replace("{id}", id))
    }
}

/// The IDs a range stands for: `TC-CAT-1200..1212`, or `TC-CAT-1200..TC-CAT-1212`. Numbers keep
/// their leading zeros (`TC-CAT-008..010`). `None` when `text` is not a range, runs backwards, or is
/// longer than [`MAX_RANGE`].
// spec: REQ-IDS-0002
pub fn expand_range(text: &str) -> Option<Vec<String>> {
    let (from, to) = text.split_once("..")?;
    let (prefix, start) = split_number(from)?;
    let end_digits = match split_number(to) {
        Some((p, n)) if p == prefix => n,
        Some(("", n)) => n,
        _ => return None,
    };
    let (first, last): (u64, u64) = (start.parse().ok()?, end_digits.parse().ok()?);
    if last < first || last - first >= MAX_RANGE {
        return None;
    }
    let width = if start.starts_with('0') {
        start.len()
    } else {
        0
    };
    Some(
        (first..=last)
            .map(|n| format!("{prefix}{n:0width$}"))
            .collect(),
    )
}

/// `TC-CAT-1200` split into `TC-CAT-` and `1200`.
fn split_number(s: &str) -> Option<(&str, &str)> {
    let at = s.rfind(|c: char| !c.is_ascii_digit()).map_or(0, |i| i + 1);
    let (prefix, digits) = s.split_at(at);
    if digits.is_empty() || digits.len() > 12 {
        None
    } else {
        Some((prefix, digits))
    }
}

/// The length of a range at the start of `rest`, which follows an ID: `..1212` or `..TC-CAT-1212`.
// spec: REQ-IDS-0002
pub fn range_tail(rest: &str) -> usize {
    let Some(after) = rest.strip_prefix("..") else {
        return 0;
    };
    let end = after
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .unwrap_or(after.len());
    if end == 0 { 0 } else { 2 + end }
}

#[cfg(test)]
mod tests {
    use super::*;

    // verifies: REQ-IDS-0002.S1
    #[test]
    fn a_range_stands_for_the_ids_between_its_ends() {
        assert_eq!(
            expand_range("TC-CAT-1200..1203").unwrap(),
            ["TC-CAT-1200", "TC-CAT-1201", "TC-CAT-1202", "TC-CAT-1203"]
        );
        assert_eq!(
            expand_range("TC-CAT-1200..TC-CAT-1201").unwrap(),
            ["TC-CAT-1200", "TC-CAT-1201"]
        );
        assert_eq!(
            expand_range("TC-A-008..010").unwrap(),
            ["TC-A-008", "TC-A-009", "TC-A-010"]
        );
    }

    // verifies: REQ-IDS-0002.S2
    #[test]
    fn a_range_that_runs_backwards_or_is_too_long_is_not_one() {
        assert_eq!(expand_range("TC-CAT-1212..1200"), None);
        assert_eq!(expand_range("TC-CAT-1..99999"), None);
        assert_eq!(expand_range("TC-CAT-1200..TC-DOG-1202"), None);
        assert_eq!(expand_range("TC-CAT-1200"), None);
        assert_eq!(expand_range("REQ-AUTH-0001.S1"), None);
    }

    #[test]
    fn the_tail_of_a_range_is_measured() {
        assert_eq!(range_tail("..1212 lists"), 6);
        assert_eq!(range_tail("..TC-CAT-1212'"), 13);
        assert_eq!(range_tail(" lists"), 0);
        assert_eq!(range_tail(".."), 0);
    }

    // verifies: REQ-IDS-0001.S1
    #[test]
    fn an_id_is_known_when_a_pattern_matches_all_of_it() {
        let ids = BTreeMap::from([("TC".to_string(), "TC-[A-Z]+-[0-9]+".to_string())]);
        let refs = Refs::new(&ids, &BTreeMap::new()).unwrap();
        assert!(refs.matches("TC-CAT-1200"));
        assert!(!refs.matches("TC-CAT-1200x"));
        assert!(!refs.matches("xTC-CAT-1200"));
        assert_eq!(refs.kind("TC-CAT-1"), Some("TC"));
    }

    // verifies: REQ-IDS-0003.S1
    #[test]
    fn a_link_to_an_external_id_needs_a_url_with_the_id_in_it() {
        let ext = |url: Option<&str>| {
            BTreeMap::from([(
                "KAN".to_string(),
                ExternalConfig {
                    pattern: "KAN-[0-9]+".into(),
                    url: url.map(str::to_string),
                },
            )])
        };
        let none = BTreeMap::new();
        let refs = Refs::new(&none, &ext(None)).unwrap();
        assert!(refs.is_external("KAN-7"));
        assert_eq!(refs.url("KAN-7"), None);
        let refs = Refs::new(&none, &ext(Some("https://x.example/browse/{id}"))).unwrap();
        assert_eq!(refs.url("KAN-7").unwrap(), "https://x.example/browse/KAN-7");
        assert!(Refs::new(&none, &ext(Some("https://x.example/browse"))).is_err());
        assert!(Refs::new(&none, &ext(Some("javascript:{id}"))).is_err());
    }
}
