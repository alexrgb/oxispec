//! Findings reported by the parser and the checks.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub path: Option<PathBuf>,
    pub line: Option<usize>,
    pub id: Option<String>,
}

impl Finding {
    pub fn new(code: &'static str, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            code,
            severity,
            message: message.into(),
            path: None,
            line: None,
            id: None,
        }
    }

    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Error, message)
    }

    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Warning, message)
    }

    pub fn info(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Info, message)
    }

    pub fn at(mut self, path: &Path, line: Option<usize>) -> Self {
        self.path = Some(path.to_path_buf());
        self.line = line;
        self
    }

    pub fn node(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }
}
