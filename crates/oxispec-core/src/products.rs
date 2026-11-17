//! Products of a monorepo: projects of their own in folders below the project.
//!
//! A folder with its own `.oxispec/config.yaml` is a product. Its documents, formats and IDs are
//! its own, so the project above leaves its files out of its graph, and `check` and `site` of
//! that project go through each product with the product's config.

use std::path::{Path, PathBuf};

use crate::config::CONFIG_PATH;

/// Whether `dir` holds a project of its own.
pub fn is_project(dir: &Path) -> bool {
    dir.join(CONFIG_PATH).is_file()
}

/// The products below `root`, as folders relative to it, in name order. A product inside a
/// product belongs to the product, not to `root`.
// spec: REQ-PROJECT-0007
pub fn find(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = ignore::WalkBuilder::new(root)
        .require_git(false)
        .build()
        .flatten()
        .filter(|e| e.path() != root && e.file_type().is_some_and(|t| t.is_dir()))
        .filter(|e| is_project(e.path()))
        .filter_map(|e| e.path().strip_prefix(root).ok().map(Path::to_path_buf))
        .collect();
    found.sort();
    // Keep the outermost ones: a product inside another is part of that product.
    let all = found.clone();
    found.retain(|p| !all.iter().any(|q| q != p && p.starts_with(q)));
    found
}

/// A path with `/` separators on every platform, as paths are kept elsewhere.
pub fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// `inner`, a path inside a product, as seen from the project above.
pub fn under(product: &Path, inner: &Path) -> PathBuf {
    PathBuf::from(format!("{}/{}", slash(product), slash(inner)))
}

/// The folder name of a product in a built site: its path with `/` turned into `_`.
pub fn site_dir(product: &Path) -> String {
    product
        .to_string_lossy()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // verifies: REQ-PROJECT-0007.S1
    #[test]
    fn products_are_the_outermost_projects_below_the_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for p in ["apps/a", "apps/b", "apps/b/inner", "tools"] {
            fs::create_dir_all(root.join(p).join(".oxispec")).unwrap();
        }
        for p in ["", "apps/a", "apps/b", "apps/b/inner"] {
            fs::create_dir_all(root.join(p).join(".oxispec")).unwrap();
            fs::write(
                root.join(p).join(CONFIG_PATH),
                "format: 1\nmodules: [core]\n",
            )
            .unwrap();
        }
        let found = find(root);
        assert_eq!(
            found,
            vec![PathBuf::from("apps/a"), PathBuf::from("apps/b")]
        );
        assert_eq!(site_dir(Path::new("apps/a")), "apps_a");
    }
}
