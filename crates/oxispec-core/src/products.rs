//! Products of a monorepo: projects of their own in folders below the project.
//!
//! A folder with its own `.oxispec/config.yaml` is a product. Its documents, formats and IDs are
//! its own, so the project above leaves its files out of its graph, and `check` and `site` of
//! that project go through each product with the product's config.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::graph::Graph;
use crate::id;

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

/// The project this one is a product of: the nearest folder above it with a project of its own.
pub fn workspace_of(root: &Path) -> Option<PathBuf> {
    let abs = root.canonicalize().ok()?;
    abs.ancestors()
        .skip(1)
        .find(|d| is_project(d))
        .map(Path::to_path_buf)
}

/// The folder a namespace names, when it is a project other than `root`. A namespace is a
/// folder below the workspace (the project above `root`, or `root` itself when there is none),
/// such as `platform` or `libs/core`.
pub fn target_dir(root: &Path, namespace: &str) -> Option<PathBuf> {
    let namespace = namespace.trim_end_matches('/');
    let plain = !namespace.is_empty()
        && namespace
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
    if !plain {
        return None;
    }
    let own = root.canonicalize().ok()?;
    let base = workspace_of(root).unwrap_or_else(|| own.clone());
    let dir = base.join(namespace);
    (is_project(&dir) && dir.canonicalize().ok()? != own).then_some(dir)
}

/// The namespaces that the links of a graph use.
pub fn namespaces(graph: &Graph) -> BTreeSet<String> {
    graph
        .nodes
        .iter()
        .flat_map(|n| n.links.values().flatten())
        .filter_map(|t| id::parse(t).and_then(|p| p.namespace))
        .map(|ns| ns.trim_end_matches('/').to_string())
        .collect()
}

/// A node of another project that links to a node of this one.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Dependent {
    /// The project of the linking node, relative to the workspace (`.` for the workspace itself).
    pub project: String,
    pub id: String,
    /// The file of the linking node, relative to the workspace.
    pub path: String,
    pub kind: String,
    /// What it links to, as written: `platform:REQ-AUTH-0001.S1`.
    pub target: String,
}

/// The nodes of the other projects of the workspace that link to the nodes of `root` that
/// `wanted` accepts (by ID). Empty when `root` has no workspace.
// spec: REQ-PROJECT-0008
pub fn dependents(root: &Path, wanted: &dyn Fn(&str) -> bool, use_cache: bool) -> Vec<Dependent> {
    let (Some(ws), Ok(own)) = (workspace_of(root), root.canonicalize()) else {
        return Vec::new();
    };
    let Ok(own_rel) = own.strip_prefix(&ws).map(slash_path) else {
        return Vec::new();
    };
    let mut projects: Vec<PathBuf> = vec![PathBuf::from(".")];
    projects.extend(find(&ws));
    let mut out = Vec::new();
    for rel in projects {
        let name = slash(&rel);
        if name == own_rel {
            continue;
        }
        let dir = ws.join(&rel);
        let graph = crate::index::load_plain(&dir, use_cache);
        for node in &graph.nodes {
            for (kind, targets) in &node.links {
                for target in targets {
                    let Some(parsed) = id::parse(target) else {
                        continue;
                    };
                    if parsed.namespace.map(|n| n.trim_end_matches('/')) == Some(own_rel.as_str())
                        && wanted(parsed.base)
                    {
                        out.push(Dependent {
                            project: name.clone(),
                            id: node.id.clone(),
                            path: if name == "." {
                                slash(&node.path)
                            } else {
                                slash(&under(&rel, &node.path))
                            },
                            kind: kind.clone(),
                            target: target.clone(),
                        });
                    }
                }
            }
        }
    }
    out
}

fn slash_path(path: &Path) -> String {
    slash(path)
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
