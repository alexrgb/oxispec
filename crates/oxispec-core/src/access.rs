//! Access to a served project: tokens, and what each token may see.
//!
//! A token has a name and, optionally, a list of node types it may read. The
//! server keeps only a hash of each token, in `.oxispec/server/tokens.json`,
//! which is kept out of git. A token with a list of types is *limited*: it sees
//! only nodes of those types, and only the tools that can be limited without
//! leaking what is hidden ([`LIMITED_TOOLS`]).
//!
//! Leak rules a limited token follows:
//! - search narrows the index query itself, so hidden nodes are neither ranked nor counted;
//! - a hidden node answers like a node that does not exist;
//! - a link to a hidden node shows as [`REDACTED`], so a graph stays readable
//!   without saying what is behind the link;
//! - counts over the whole project (index statistics, `spec_status`) are not offered.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::node::Node;

/// What a link to a node the token may not see shows instead of its ID.
pub const REDACTED: &str = "[restricted node]";

/// Every token starts with this, so a leaked one is easy to spot and to scan for.
pub const TOKEN_PREFIX: &str = "oxs_";

/// The tools a limited token may call.
pub const LIMITED_TOOLS: &[&str] = &["spec_search", "spec_get"];

const TOKENS_FILE: &str = ".oxispec/server/tokens.json";
const IGNORE_FILE: &str = ".oxispec/.gitignore";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("cannot use {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Token {
    pub name: String,
    /// BLAKE3 of the token, in hex.
    pub hash: String,
    /// Node types the token may read. `None`: all of them, and all tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<String>>,
    /// Seconds since the Unix epoch.
    pub created: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub tokens: Vec<Token>,
}

fn hash_of(secret: &str) -> String {
    blake3::hash(secret.as_bytes()).to_hex().to_string()
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

fn valid_type(ty: &str) -> bool {
    !ty.is_empty()
        && ty.len() <= 64
        && ty
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl Store {
    pub fn path(root: &Path) -> PathBuf {
        root.join(TOKENS_FILE)
    }

    /// The stored tokens, or none when the file does not exist.
    pub fn load(root: &Path) -> Result<Store, Error> {
        let path = Store::path(root);
        match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|e| Error::Invalid(format!("{} is not valid: {e}", path.display()))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Store::default()),
            Err(source) => Err(Error::Io { path, source }),
        }
    }

    /// Writes the file, readable by the owner only, and makes sure git ignores it.
    pub fn save(&self, root: &Path) -> Result<(), Error> {
        let path = Store::path(root);
        let io_error = |source| Error::Io {
            path: path.clone(),
            source,
        };
        fs::create_dir_all(path.parent().expect("a parent")).map_err(io_error)?;
        keep_out_of_git(root);
        let text = serde_json::to_string_pretty(self).map_err(|e| Error::Invalid(e.to_string()))?;
        let temporary = path.with_extension("json.tmp");
        write_private(&temporary, text.as_bytes()).map_err(io_error)?;
        fs::rename(&temporary, &path).map_err(io_error)
    }

    /// Adds a token for `secret`. The secret itself is not kept.
    pub fn add(
        &mut self,
        name: &str,
        types: Option<Vec<String>>,
        secret: &str,
        now: u64,
    ) -> Result<(), Error> {
        if !valid_name(name) {
            return Err(Error::Invalid(
                "a token name is letters, digits, `-`, `_` and `.`, up to 64 characters".into(),
            ));
        }
        if self.tokens.iter().any(|t| t.name == name) {
            return Err(Error::Invalid(format!("a token named `{name}` exists")));
        }
        if let Some(types) = &types
            && (types.is_empty() || !types.iter().all(|t| valid_type(t)))
        {
            return Err(Error::Invalid(
                "give node types such as `requirement,adr`".into(),
            ));
        }
        self.tokens.push(Token {
            name: name.to_string(),
            hash: hash_of(secret),
            types,
            created: now,
        });
        Ok(())
    }

    /// Removes a token. False when there was none of that name.
    pub fn revoke(&mut self, name: &str) -> bool {
        let before = self.tokens.len();
        self.tokens.retain(|t| t.name != name);
        self.tokens.len() != before
    }

    /// The token that `presented` is, if any. Every stored hash is compared,
    /// and each comparison takes the same time whatever the input.
    pub fn authenticate(&self, presented: &str) -> Option<&Token> {
        if !presented.starts_with(TOKEN_PREFIX) {
            return None;
        }
        let hash = blake3::hash(presented.as_bytes());
        let mut found = None;
        for token in &self.tokens {
            // `Hash` compares in constant time.
            if blake3::Hash::from_hex(&token.hash).is_ok_and(|h| h == hash) {
                found = Some(token);
            }
        }
        found
    }
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::write(path, bytes)
}

/// Adds `server/` to `.oxispec/.gitignore`, where the token hashes live.
fn keep_out_of_git(root: &Path) {
    let path = root.join(IGNORE_FILE);
    let current = fs::read_to_string(&path).unwrap_or_default();
    if current.lines().any(|l| l.trim() == "server/") {
        return;
    }
    let mut text = current;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("server/\n");
    let _ = fs::write(path, text);
}

/// What one caller may see. Built from a [`Token`] by [`Policy::for_token`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Policy {
    types: Option<Vec<String>>,
}

impl Policy {
    /// No limit: every node and every tool.
    pub fn full() -> Policy {
        Policy::default()
    }

    pub fn for_token(token: &Token) -> Policy {
        Policy {
            types: token.types.clone(),
        }
    }

    pub fn is_limited(&self) -> bool {
        self.types.is_some()
    }

    /// The types to hand to the search, or none for any type.
    pub fn types(&self) -> &[String] {
        self.types.as_deref().unwrap_or(&[])
    }

    pub fn allows_tool(&self, name: &str) -> bool {
        !self.is_limited() || LIMITED_TOOLS.contains(&name)
    }

    pub fn allows(&self, node: &Node) -> bool {
        self.allows_type(&node.ty)
    }

    pub fn allows_type(&self, ty: &str) -> bool {
        match &self.types {
            None => true,
            Some(types) => types.iter().any(|t| t == ty),
        }
    }
}

#[cfg(test)]
mod tests_of_access {
    use super::*;

    fn store_with(name: &str, types: Option<Vec<String>>, secret: &str) -> Store {
        let mut store = Store::default();
        store.add(name, types, secret, 1).unwrap();
        store
    }

    #[test]
    fn a_token_is_known_by_its_hash_and_nothing_else_matches() {
        let store = store_with("ci", None, "oxs_abc");
        assert_eq!(store.authenticate("oxs_abc").unwrap().name, "ci");
        assert!(store.authenticate("oxs_abd").is_none());
        assert!(store.authenticate("abc").is_none());
        assert!(store.authenticate("").is_none());
        let stored = serde_json::to_string(&store).unwrap();
        assert!(!stored.contains("oxs_abc"), "the secret is not stored");
    }

    #[test]
    fn names_and_types_are_checked() {
        let mut store = Store::default();
        for bad in ["", "a b", "x/y", &"n".repeat(65)] {
            assert!(store.add(bad, None, "oxs_1", 1).is_err(), "{bad:?}");
        }
        assert!(store.add("ok", Some(vec![]), "oxs_1", 1).is_err());
        assert!(
            store
                .add("ok", Some(vec!["a b".into()]), "oxs_1", 1)
                .is_err()
        );
        store.add("ok", None, "oxs_1", 1).unwrap();
        assert!(
            store.add("ok", None, "oxs_2", 1).is_err(),
            "names are unique"
        );
    }

    #[test]
    fn revoking_removes_the_token() {
        let mut store = store_with("ci", None, "oxs_abc");
        assert!(store.revoke("ci"));
        assert!(!store.revoke("ci"));
        assert!(store.authenticate("oxs_abc").is_none());
    }

    #[test]
    fn a_limited_policy_allows_only_its_types_and_tools() {
        let token = store_with("docs", Some(vec!["adr".into()]), "oxs_1").tokens[0].clone();
        let policy = Policy::for_token(&token);
        assert!(policy.is_limited());
        assert!(policy.allows_type("adr"));
        assert!(!policy.allows_type("requirement"));
        assert!(policy.allows_tool("spec_search"));
        assert!(!policy.allows_tool("spec_check"));
        assert!(Policy::full().allows_tool("spec_check"));
        assert!(Policy::full().allows_type("anything"));
    }

    #[test]
    fn the_file_is_private_and_git_ignores_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".oxispec")).unwrap();
        fs::write(root.join(IGNORE_FILE), "cache/\n").unwrap();
        let store = store_with("ci", None, "oxs_abc");
        store.save(root).unwrap();
        store.save(root).unwrap();
        let ignore = fs::read_to_string(root.join(IGNORE_FILE)).unwrap();
        assert_eq!(ignore, "cache/\nserver/\n");
        let loaded = Store::load(root).unwrap();
        assert_eq!(loaded.tokens, store.tokens);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(Store::path(root))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0);
        }
    }

    #[test]
    fn a_missing_file_is_no_tokens_and_a_broken_one_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Store::load(dir.path()).unwrap().tokens.is_empty());
        fs::create_dir_all(dir.path().join(".oxispec/server")).unwrap();
        fs::write(Store::path(dir.path()), "nope").unwrap();
        assert!(Store::load(dir.path()).is_err());
    }
}
