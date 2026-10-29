//! `oxispec site`: a static site from the specs, for one revision or for
//! the working tree and every release.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use oxispec_core::release::Version;
use oxispec_core::site::{Page, Previous, STYLE, SiteOptions, VersionLink, render};
use oxispec_core::{Graph, Node, config, index};

use crate::git::{self, Revision};

/// Gives the text of a node's file.
type TextOf = Box<dyn Fn(&Node) -> Option<String>>;

pub struct SiteArgs {
    pub out: PathBuf,
    pub at: Option<String>,
    pub title: Option<String>,
    /// The working tree as `latest`, and a folder for each spec tag.
    pub releases: bool,
    pub cache: bool,
}

/// The text of every node file at a revision, by path.
fn texts_at(root: &Path, revision: &Revision, graph: &Graph) -> Result<HashMap<PathBuf, String>> {
    let prefix = git::prefix(root)?;
    let paths: Vec<&PathBuf> = graph.nodes.iter().map(|n| &n.path).collect();
    let specs: Vec<String> = paths
        .iter()
        .map(|p| {
            format!(
                "{}:{prefix}{}",
                revision.oid,
                p.to_string_lossy().replace('\\', "/")
            )
        })
        .collect();
    let contents = git::read_objects(root, &specs)?;
    Ok(paths
        .into_iter()
        .zip(contents)
        .filter_map(|(path, bytes)| {
            Some((path.clone(), String::from_utf8_lossy(&bytes?).into_owned()))
        })
        .collect())
}

/// Removes the pages of an earlier build, and nothing else.
fn clear_pages(dir: &Path) -> Result<()> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let generated =
                path.extension().is_some_and(|e| e == "html") || path.ends_with("style.css");
            if generated && path.is_file() {
                fs::remove_file(&path)
                    .with_context(|| format!("cannot remove {}", path.display()))?;
            }
        }
    }
    Ok(())
}

fn write_pages(dir: &Path, pages: &[Page]) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    clear_pages(dir)?;
    fs::write(dir.join("style.css"), STYLE)?;
    for page in pages {
        fs::write(dir.join(&page.path), &page.html)
            .with_context(|| format!("cannot write {}", dir.join(&page.path).display()))?;
    }
    Ok(())
}

/// A folder name for a tag: the same rule as for page names, without the extension.
fn dir_name(tag: &str) -> String {
    tag.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn title_of(root: &Path, given: Option<&str>) -> String {
    given.map(str::to_string).unwrap_or_else(|| {
        root.canonicalize()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "Specs".to_string())
    })
}

fn guard_out(root: &Path, out: &Path) -> Result<()> {
    let (root, out) = (
        root.canonicalize()?,
        fs::canonicalize(out).unwrap_or_else(|_| out.to_path_buf()),
    );
    if out == root {
        bail!("--out must be a folder of its own, not the project folder");
    }
    Ok(())
}

pub fn site_command(root: &Path, args: &SiteArgs) -> Result<()> {
    config::load(root)?;
    guard_out(root, &args.out)?;
    let title = title_of(root, args.title.as_deref());
    if args.releases {
        if args.at.is_some() {
            bail!("--at builds one revision, and --releases builds all of them; use one");
        }
        return releases(root, args, &title);
    }

    let (graph, revision, text): (Graph, String, TextOf) = match &args.at {
        Some(spec) => {
            git::require_repository(root, "site --at")?;
            let rev = git::resolve_rev(root, spec)?;
            let graph = git::graph_at(root, &rev)?;
            let texts = texts_at(root, &rev, &graph)?;
            (
                graph,
                rev.to_string(),
                Box::new(move |n: &Node| texts.get(&n.path).cloned()),
            )
        }
        None => {
            let graph = index::load(root, args.cache).0;
            let root = root.to_path_buf();
            (
                graph,
                "working tree".to_string(),
                Box::new(move |n: &Node| fs::read_to_string(root.join(&n.path)).ok()),
            )
        }
    };
    let options = SiteOptions {
        title,
        revision,
        current: None,
        versions: Vec::new(),
        previous: None,
    };
    let pages = render(&graph, &options, &*text);
    write_pages(&args.out, &pages)?;
    println!("wrote {} pages to {}", pages.len(), args.out.display());
    Ok(())
}

/// `latest` from the working tree, and one folder for each spec tag, with a
/// version picker on every page and each page compared with the release before.
fn releases(root: &Path, args: &SiteArgs, title: &str) -> Result<()> {
    git::require_repository(root, "site --releases")?;
    let mut tags = git::list_tags(root)?;
    // Newest first. Tags that are versions go by version. The rest go by how
    // much history leads up to them, since tags made in the same second have
    // no reliable date order.
    let mut keyed = Vec::new();
    for tag in tags.drain(..) {
        let count = git::commit_count(root, &format!("{}^{{commit}}", tag.tag))?;
        keyed.push(((Version::parse(&tag.name), count), tag));
    }
    keyed.sort_by(|a, b| b.0.cmp(&a.0));
    let tags: Vec<_> = keyed.into_iter().map(|(_, tag)| tag).collect();
    let mut versions = vec![VersionLink {
        dir: "latest".into(),
        label: "latest (working tree)".into(),
    }];
    versions.extend(tags.iter().map(|t| VersionLink {
        dir: dir_name(&t.name),
        label: t.name.clone(),
    }));
    let mut seen = std::collections::HashSet::new();
    if let Some(dup) = versions.iter().find(|v| !seen.insert(v.dir.clone())) {
        bail!("two versions would share the folder `{}`", dup.dir);
    }

    // Tags come newest first, so the release before tag `i` is tag `i + 1`.
    let mut graphs = Vec::new();
    for tag in &tags {
        let rev = git::resolve_rev(root, &tag.tag)?;
        let graph = git::graph_at(root, &rev)?;
        graphs.push((rev, graph));
    }
    let mut total = 0;
    for (i, tag) in tags.iter().enumerate() {
        let (rev, graph) = &graphs[i];
        let texts = texts_at(root, rev, graph)?;
        let previous = graphs.get(i + 1).map(|(_, g)| Previous {
            label: tags[i + 1].name.clone(),
            graph: g,
        });
        let options = SiteOptions {
            title: title.to_string(),
            revision: format!("{} ({})", tag.name, tag.commit),
            current: Some(dir_name(&tag.name)),
            versions: versions.clone(),
            previous,
        };
        let pages = render(graph, &options, &|n: &Node| texts.get(&n.path).cloned());
        total += pages.len();
        write_pages(&args.out.join(dir_name(&tag.name)), &pages)?;
    }

    let latest = index::load(root, args.cache).0;
    let previous = graphs.first().map(|(_, g)| Previous {
        label: tags[0].name.clone(),
        graph: g,
    });
    let options = SiteOptions {
        title: title.to_string(),
        revision: "working tree".into(),
        current: Some("latest".into()),
        versions: versions.clone(),
        previous,
    };
    let base = root.to_path_buf();
    let pages = render(&latest, &options, &|n: &Node| {
        fs::read_to_string(base.join(&n.path)).ok()
    });
    total += pages.len();
    write_pages(&args.out.join("latest"), &pages)?;

    fs::create_dir_all(&args.out)?;
    fs::write(args.out.join("index.html"), root_page(title, &versions))?;
    println!(
        "wrote {total} pages for {} versions to {}",
        versions.len(),
        args.out.display()
    );
    Ok(())
}

/// A page at the top that sends readers to the latest version and lists the others.
fn root_page(title: &str, versions: &[VersionLink]) -> String {
    use oxispec_core::site::escape;
    let mut list = String::new();
    for v in versions {
        list.push_str(&format!(
            "<li><a href=\"{}/index.html\">{}</a></li>",
            escape(&v.dir),
            escape(&v.label)
        ));
    }
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{t}</title><meta http-equiv=\"refresh\" content=\"0; url=latest/index.html\"></head><body><h1>{t}</h1><p>Versions:</p><ul>{list}</ul></body></html>\n",
        t = escape(title)
    )
}
