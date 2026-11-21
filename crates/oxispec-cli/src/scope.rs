//! Which project of a monorepo a command reads: the top one, one product named with
//! `--product`, or all of them.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use oxispec_core::{config, products};

/// The top project as `.`, then each product, with its folder.
pub fn projects(root: &Path) -> Vec<(String, PathBuf)> {
    let mut all = vec![(".".to_string(), root.to_path_buf())];
    all.extend(
        products::find(root)
            .into_iter()
            .map(|rel| (products::slash(&rel), root.join(rel))),
    );
    all
}

/// The folder of the product `name`, or `root` itself when none is named.
// spec: REQ-PROJECT-0009
pub fn choose(root: &Path, name: Option<&str>) -> Result<PathBuf> {
    let Some(name) = name else {
        return Ok(root.to_path_buf());
    };
    let wanted = name.trim_end_matches('/').replace('\\', "/");
    let found = projects(root);
    match found.iter().skip(1).find(|(p, _)| *p == wanted) {
        Some((_, dir)) => Ok(dir.clone()),
        None => {
            let known: Vec<&str> = found.iter().skip(1).map(|(p, _)| p.as_str()).collect();
            if known.is_empty() {
                bail!(
                    "`{name}` is not a product: no folder below this project has a project of its own"
                );
            }
            bail!("`{name}` is not a product (known: {})", known.join(", "))
        }
    }
}

/// The one product that `has` something, when the top project has not. Several products with
/// it are an error that names them.
// spec: REQ-PROJECT-0009
pub fn holder(
    root: &Path,
    what: &str,
    has: impl Fn(&Path) -> bool,
) -> Result<Option<(String, PathBuf)>> {
    let mut found: Vec<(String, PathBuf)> = projects(root)
        .into_iter()
        .skip(1)
        .filter(|(_, dir)| config::load(dir).is_ok() && has(dir))
        .collect();
    match found.len() {
        0 => Ok(None),
        1 => Ok(found.pop()),
        _ => {
            let names: Vec<String> = found.into_iter().map(|(p, _)| p).collect();
            bail!(
                "{what} is in several products: {}; name one with `--product`",
                names.join(", ")
            )
        }
    }
}
