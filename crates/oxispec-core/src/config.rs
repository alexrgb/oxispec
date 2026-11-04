//! Project configuration in `.oxispec/config.yaml`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::formats::CustomProfile;

pub const CONFIG_PATH: &str = ".oxispec/config.yaml";

/// The project format this version reads and writes. See docs/versioning.md.
pub const FORMAT: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("invalid config {0}: {1}")]
    Yaml(PathBuf, String),
    #[error("{0} uses format {1}, but this oxispec supports up to format {2}; upgrade oxispec")]
    FormatTooNew(PathBuf, u32, u32),
}

/// How new node IDs are numbered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IdScheme {
    /// `REQ-AUTH-0001`, the next free number. Simple, and fine for one branch at a time.
    #[default]
    Sequential,
    /// `REQ-AUTH-7K3F`, a random suffix, so parallel branches do not collide.
    Random,
}

/// Where trace annotations are looked for. By default, every text file that
/// git does not ignore, except markdown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceConfig {
    /// Globs of files to leave out, for example `vendor/**`.
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl TraceConfig {
    fn is_default(&self) -> bool {
        self.exclude.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_format")]
    pub format: u32,
    pub modules: Vec<String>,
    #[serde(default = "default_topology")]
    pub topology: String,
    #[serde(default)]
    pub id_scheme: IdScheme,
    #[serde(default, skip_serializing_if = "TraceConfig::is_default")]
    pub trace: TraceConfig,
    /// The AI clients that get skills, by id. See `agents::CLIENTS`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<String>,
    /// The format profile for each node type, by profile name. See `formats`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub formats: BTreeMap<String, String>,
    /// Profiles this project defines, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profiles: BTreeMap<String, CustomProfile>,
    /// How tech debt is judged. See `debt`.
    #[serde(default, skip_serializing_if = "DebtConfig::is_default")]
    pub debt: DebtConfig,
}

/// Settings for tech debt items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebtConfig {
    /// A live item that nobody has reviewed for this many days is stale.
    #[serde(default = "default_stale_days")]
    pub stale_days: u32,
    /// The most live items a release may have, by severity. A severity that is
    /// not listed has no limit.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub limits: BTreeMap<String, u32>,
}

fn default_stale_days() -> u32 {
    180
}

impl Default for DebtConfig {
    fn default() -> Self {
        DebtConfig {
            stale_days: default_stale_days(),
            limits: BTreeMap::new(),
        }
    }
}

impl DebtConfig {
    fn is_default(&self) -> bool {
        *self == DebtConfig::default()
    }
}

fn default_format() -> u32 {
    FORMAT
}

fn default_topology() -> String {
    "single".to_string()
}

impl Config {
    pub fn new(modules: Vec<String>, id_scheme: IdScheme) -> Self {
        Self {
            format: FORMAT,
            modules,
            topology: default_topology(),
            id_scheme,
            trace: TraceConfig::default(),
            agents: Vec::new(),
            formats: BTreeMap::new(),
            profiles: BTreeMap::new(),
            debt: DebtConfig::default(),
        }
    }

    pub fn has(&self, module: &str) -> bool {
        self.modules.iter().any(|m| m == module)
    }
}

/// Walks up from `start` to the folder that contains `.oxispec/config.yaml`.
pub fn find_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(CONFIG_PATH).is_file())
        .map(Path::to_path_buf)
}

fn parse_text(path: &Path, text: &str) -> Result<Config, Error> {
    let config: Config =
        serde_saphyr::from_str(text).map_err(|e| Error::Yaml(path.to_path_buf(), e.to_string()))?;
    if config.format > FORMAT {
        return Err(Error::FormatTooNew(
            path.to_path_buf(),
            config.format,
            FORMAT,
        ));
    }
    Ok(config)
}

pub fn load(root: &Path) -> Result<Config, Error> {
    let path = root.join(CONFIG_PATH);
    let text = std::fs::read_to_string(&path).map_err(|e| Error::Io(path.clone(), e))?;
    parse_text(&path, &text)
}

pub fn save(root: &Path, config: &Config) -> Result<(), Error> {
    let path = root.join(CONFIG_PATH);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::Io(dir.to_path_buf(), e))?;
    }
    let text =
        serde_saphyr::to_string(config).map_err(|e| Error::Yaml(path.clone(), e.to_string()))?;
    std::fs::write(&path, text).map_err(|e| Error::Io(path, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> &'static Path {
        Path::new(CONFIG_PATH)
    }

    #[test]
    fn config_from_iteration_one_still_loads() {
        // No `format` and no `id_scheme`.
        let c = parse_text(path(), "modules:\n- core\n- specs\ntopology: single\n").unwrap();
        assert_eq!(c.format, FORMAT);
        assert_eq!(c.id_scheme, IdScheme::Sequential);
        assert!(c.has("specs"));
    }

    #[test]
    fn a_newer_format_is_refused() {
        let e = parse_text(path(), "format: 99\nmodules: [core]\n").unwrap_err();
        let message = e.to_string();
        assert!(message.contains("format 99"), "{message}");
        assert!(message.contains("upgrade oxispec"), "{message}");
    }

    #[test]
    fn the_current_format_and_scheme_survive_a_round_trip() {
        let config = Config::new(vec!["core".into(), "specs".into()], IdScheme::Random);
        let text = serde_saphyr::to_string(&config).unwrap();
        assert!(text.contains("format: 1"), "{text}");
        assert!(text.contains("id_scheme: random"), "{text}");
        assert_eq!(parse_text(path(), &text).unwrap(), config);
    }

    #[test]
    fn an_unknown_scheme_is_an_error() {
        let e = parse_text(path(), "modules: [core]\nid_scheme: lucky\n").unwrap_err();
        assert!(matches!(e, Error::Yaml(..)), "{e}");
    }
}
