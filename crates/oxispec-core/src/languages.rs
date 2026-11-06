//! The languages the trace scan knows, and how it reads each.
//!
//! A language is read one of two ways. With a grammar (Rust, Go, TypeScript,
//! JavaScript) the scan finds the comments with a tree-sitter parser compiled
//! into the binary, and can tell which function a comment is about. Without
//! one, the scan reads the comment leaders of the language (`#`, `--`, `//`),
//! which the project can set in `.oxispec/config.yaml`:
//!
//! ```yaml
//! trace:
//!   languages: [rust, python, sql]   # scan only these; empty: every text file
//!   comments:                         # a language without a grammar
//!     python: { extensions: [py], leaders: ["#"] }
//! ```
//!
//! A grammar cannot be added by configuration: it is code, and goes in with
//! the binary.

use std::collections::BTreeMap;
use std::path::Path;

use crate::config::{CommentSyntax, TraceConfig};

/// One language and how its files are read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    pub name: String,
    pub extensions: Vec<String>,
    /// True when a tree-sitter grammar reads its comments.
    pub parsed: bool,
    /// The comment leaders, for a language without a grammar.
    pub leaders: Vec<String>,
}

fn lang(name: &str, extensions: &[&str], parsed: bool, leaders: &[&str]) -> Language {
    Language {
        name: name.into(),
        extensions: extensions.iter().map(|s| s.to_string()).collect(),
        parsed,
        leaders: leaders.iter().map(|s| s.to_string()).collect(),
    }
}

/// The languages known without any configuration.
pub fn builtin() -> Vec<Language> {
    let c_style: &[&str] = &["//", "/*"];
    vec![
        lang("rust", &["rs"], true, &[]),
        lang("go", &["go"], true, &[]),
        lang("typescript", &["ts", "tsx", "mts", "cts"], true, &[]),
        lang("javascript", &["js", "jsx", "mjs", "cjs"], true, &[]),
        lang("python", &["py"], false, &["#"]),
        lang("ruby", &["rb"], false, &["#"]),
        lang("shell", &["sh", "bash", "zsh"], false, &["#"]),
        lang("perl", &["pl", "pm"], false, &["#"]),
        lang("yaml", &["yml", "yaml"], false, &["#"]),
        lang("toml", &["toml"], false, &["#"]),
        lang("java", &["java"], false, c_style),
        lang("kotlin", &["kt", "kts"], false, c_style),
        lang("swift", &["swift"], false, c_style),
        lang("csharp", &["cs"], false, c_style),
        lang("c", &["c", "h"], false, c_style),
        lang("cpp", &["cc", "cpp", "cxx", "hpp", "hh"], false, c_style),
        lang("php", &["php"], false, &["//", "/*", "#"]),
        lang("scala", &["scala"], false, c_style),
        lang("dart", &["dart"], false, c_style),
        lang("sql", &["sql"], false, &["--", "/*"]),
        lang("lua", &["lua"], false, &["--"]),
        lang("haskell", &["hs"], false, &["--"]),
        lang("html", &["html", "htm", "xml"], false, &["<!--"]),
        lang("lisp", &["lisp", "clj", "el"], false, &[";"]),
    ]
}

/// The languages of a project: the built-in ones, changed by the config.
#[derive(Debug, Clone)]
pub struct Languages {
    list: Vec<Language>,
    /// When set, only files of these languages are scanned.
    only: Option<Vec<String>>,
}

impl Default for Languages {
    fn default() -> Self {
        Languages {
            list: builtin(),
            only: None,
        }
    }
}

impl Languages {
    /// The languages for a project's `trace` settings. A language named in
    /// `comments` is added, or, if it is built in and has no grammar,
    /// changed. An unknown name in `languages`, or a grammar language in
    /// `comments`, is an error with a message.
    // spec: REQ-TRACE-0004
    pub fn from_config(config: &TraceConfig) -> Result<Languages, String> {
        let mut list = builtin();
        for (name, syntax) in &config.comments {
            if let Some(existing) = list.iter_mut().find(|l| &l.name == name) {
                if existing.parsed {
                    return Err(format!(
                        "`{name}` is read with a grammar; `trace.comments` is for a language without one"
                    ));
                }
                if !syntax.extensions.is_empty() {
                    existing.extensions = syntax.extensions.clone();
                }
                if !syntax.leaders.is_empty() {
                    existing.leaders = syntax.leaders.clone();
                }
            } else {
                if syntax.extensions.is_empty() || syntax.leaders.is_empty() {
                    return Err(format!(
                        "`trace.comments.{name}` needs `extensions` and `leaders`"
                    ));
                }
                list.push(Language {
                    name: name.clone(),
                    extensions: syntax.extensions.clone(),
                    parsed: false,
                    leaders: syntax.leaders.clone(),
                });
            }
        }
        let only = if config.languages.is_empty() {
            None
        } else {
            for name in &config.languages {
                if !list.iter().any(|l| &l.name == name) {
                    let known: Vec<&str> = list.iter().map(|l| l.name.as_str()).collect();
                    return Err(format!(
                        "unknown language `{name}` in `trace.languages`; known: {}",
                        known.join(", ")
                    ));
                }
            }
            Some(config.languages.clone())
        };
        Ok(Languages { list, only })
    }

    pub fn all(&self) -> &[Language] {
        &self.list
    }

    /// The language of a file, by its extension.
    pub fn for_path(&self, path: &Path) -> Option<&Language> {
        let ext = path.extension()?.to_str()?;
        self.list
            .iter()
            .find(|l| l.extensions.iter().any(|e| e == ext))
    }

    /// Whether a file is scanned. With no `trace.languages`, every file is.
    pub fn wants(&self, path: &Path) -> bool {
        match &self.only {
            None => true,
            Some(only) => self
                .for_path(path)
                .is_some_and(|l| only.iter().any(|n| n == &l.name)),
        }
    }

    /// Whether `trace.languages` limits the scan.
    pub fn is_limited(&self) -> bool {
        self.only.is_some()
    }

    /// The names a project chose, if any.
    pub fn chosen(&self) -> Option<&[String]> {
        self.only.as_deref()
    }
}

/// The `trace.comments` block for the languages a project names, for `init`.
pub fn config_for(names: &[String]) -> Result<TraceConfig, String> {
    let known = builtin();
    let mut config = TraceConfig::default();
    for name in names {
        if !known.iter().any(|l| &l.name == name) {
            let all: Vec<&str> = known.iter().map(|l| l.name.as_str()).collect();
            return Err(format!(
                "unknown language `{name}`; known: {}",
                all.join(", ")
            ));
        }
        config.languages.push(name.clone());
    }
    config.comments = BTreeMap::<String, CommentSyntax>::new();
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comments(name: &str, extensions: &[&str], leaders: &[&str]) -> TraceConfig {
        let mut c = TraceConfig::default();
        c.comments.insert(
            name.into(),
            CommentSyntax {
                extensions: extensions.iter().map(|s| s.to_string()).collect(),
                leaders: leaders.iter().map(|s| s.to_string()).collect(),
            },
        );
        c
    }

    // verifies: REQ-TRACE-0004.S1
    #[test]
    fn a_project_adds_a_language_and_limits_the_scan_to_some() {
        let langs = Languages::default();
        assert!(langs.wants(Path::new("a.anything")));
        assert_eq!(langs.for_path(Path::new("a.py")).unwrap().name, "python");

        let mut config = comments("zig", &["zig"], &["//"]);
        config.languages = vec!["rust".into(), "zig".into()];
        let langs = Languages::from_config(&config).unwrap();
        assert!(langs.wants(Path::new("src/a.rs")));
        assert!(langs.wants(Path::new("src/a.zig")));
        assert!(!langs.wants(Path::new("src/a.py")));
        assert!(!langs.wants(Path::new("Makefile")));
        assert_eq!(langs.for_path(Path::new("a.zig")).unwrap().leaders, ["//"]);
    }

    // verifies: REQ-TRACE-0004.S2
    #[test]
    fn a_bad_setting_is_an_error_with_the_reason() {
        let config = TraceConfig {
            languages: vec!["cobol".into()],
            ..TraceConfig::default()
        };
        assert!(
            Languages::from_config(&config)
                .unwrap_err()
                .contains("unknown language `cobol`")
        );
        let err = Languages::from_config(&comments("rust", &["rs"], &["//"])).unwrap_err();
        assert!(err.contains("grammar"), "{err}");
        let err = Languages::from_config(&comments("zig", &[], &["//"])).unwrap_err();
        assert!(err.contains("needs"), "{err}");
        // A built-in language without a grammar can have its leaders changed.
        let langs = Languages::from_config(&comments("python", &["py", "pyi"], &["#"])).unwrap();
        assert_eq!(langs.for_path(Path::new("a.pyi")).unwrap().name, "python");
    }
}
