//! Project configuration in `.oxispec/config.yaml`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const CONFIG_PATH: &str = ".oxispec/config.yaml";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("invalid config {0}: {1}")]
    Yaml(PathBuf, String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub modules: Vec<String>,
    #[serde(default = "default_topology")]
    pub topology: String,
}

fn default_topology() -> String {
    "single".to_string()
}

impl Config {
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

pub fn load(root: &Path) -> Result<Config, Error> {
    let path = root.join(CONFIG_PATH);
    let text = std::fs::read_to_string(&path).map_err(|e| Error::Io(path.clone(), e))?;
    serde_saphyr::from_str(&text).map_err(|e| Error::Yaml(path, e.to_string()))
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
