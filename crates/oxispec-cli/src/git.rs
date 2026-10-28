//! Everything that asks git. The core crate never runs a process.

use std::collections::BTreeSet;
use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use anyhow::{Context, Result, bail};
use oxispec_core::Graph;
use oxispec_core::parse::parse_file_text;

/// A commit that a name such as `main`, `v1.0` or `date:2026-01-31` points at.
#[derive(Debug, Clone)]
pub struct Revision {
    pub oid: String,
    pub short: String,
    pub subject: String,
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.short, self.subject)
    }
}

/// One commit in the history of a file.
#[derive(Debug, Clone)]
pub struct Commit {
    pub oid: String,
    pub short: String,
    pub author: String,
    /// `YYYY-MM-DD`
    pub date: String,
    pub subject: String,
    /// Where the file was at this commit, relative to the project root.
    pub path: PathBuf,
}

/// The markdown files that differ from a base, relative to the project root.
#[derive(Debug, Default)]
pub struct Changes {
    pub paths: BTreeSet<PathBuf>,
    /// IDs the changed files had at the base. A deleted or renamed node is
    /// only known by its old ID.
    pub old_ids: BTreeSet<String>,
}

fn git(root: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .context("cannot run git; is it installed and on PATH?")
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = git(root, args)?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(out.stdout)
}

fn git_text(root: &Path, args: &[&str]) -> Result<String> {
    Ok(String::from_utf8_lossy(&git_stdout(root, args)?)
        .trim()
        .to_string())
}

/// Splits git's NUL separated output into names.
fn names(bytes: &[u8]) -> impl Iterator<Item = String> + '_ {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
}

pub fn require_repository(root: &Path, what: &str) -> Result<()> {
    let inside = git(root, &["rev-parse", "--is-inside-work-tree"])?;
    if !inside.status.success() {
        bail!(
            "`{what}` needs a git repository, and {} is not inside one",
            root.display()
        );
    }
    Ok(())
}

/// The path from the top of the repository to the project root, with a
/// trailing slash, or an empty string when they are the same folder.
fn prefix(root: &Path) -> Result<String> {
    git_text(root, &["rev-parse", "--show-prefix"])
}

/// The commit that `name` points at, if there is one.
fn commit_of(root: &Path, name: &str) -> Result<Option<String>> {
    let spec = format!("{name}^{{commit}}");
    let out = git(root, &["rev-parse", "--verify", "--quiet", &spec])?;
    Ok(out
        .status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()))
}

pub fn has_commits(root: &Path) -> Result<bool> {
    Ok(commit_of(root, "HEAD")?.is_some())
}

/// Turns what a person typed into a commit. Accepts anything git does (a
/// branch, a tag, `HEAD~3`, a hash), `date:YYYY-MM-DD` for the last commit
/// on or before that day, and a spec release tag without its `specs-` prefix.
pub fn resolve_rev(root: &Path, spec: &str) -> Result<Revision> {
    require_repository(root, "history commands")?;
    let oid = if let Some(date) = spec.strip_prefix("date:") {
        let looks_like_a_date = date.len() == 10
            && date.chars().enumerate().all(|(i, c)| {
                if i == 4 || i == 7 {
                    c == '-'
                } else {
                    c.is_ascii_digit()
                }
            });
        if !looks_like_a_date {
            bail!("a date looks like date:2026-01-31, not `{spec}`");
        }
        let before = format!("--before={date} 23:59:59");
        let found = git_text(root, &["rev-list", "-1", &before, "HEAD"])?;
        if found.is_empty() {
            bail!("no commit on or before {date}");
        }
        found
    } else {
        match commit_of(root, spec)? {
            Some(oid) => oid,
            None => commit_of(root, &format!("specs-{spec}"))?
                .with_context(|| format!("unknown revision `{spec}`"))?,
        }
    };
    let described = git_text(root, &["log", "-1", "--format=%h%x1f%s", &oid])?;
    let (short, subject) = described.split_once('\x1f').unwrap_or((&oid, ""));
    Ok(Revision {
        short: short.to_string(),
        subject: subject.to_string(),
        oid,
    })
}

/// Reads git objects, such as `<commit>:<path>`, with one `git cat-file`
/// process. An object that does not exist gives `None`.
pub fn read_objects(root: &Path, specs: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
    if specs.is_empty() {
        return Ok(Vec::new());
    }
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("cannot run git; is it installed and on PATH?")?;
    let mut stdin = child.stdin.take().context("git has no input pipe")?;
    // Written from another thread so a long list cannot fill the pipes and stall.
    let requests = specs.join("\n") + "\n";
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(requests.as_bytes());
    });

    let mut out = BufReader::new(child.stdout.take().context("git has no output pipe")?);
    let mut results = Vec::with_capacity(specs.len());
    for _ in specs {
        let mut header = String::new();
        if out.read_line(&mut header)? == 0 {
            bail!("git cat-file stopped early");
        }
        let header = header.trim_end();
        if header.ends_with(" missing") {
            results.push(None);
            continue;
        }
        let size: usize = header
            .rsplit(' ')
            .next()
            .and_then(|s| s.parse().ok())
            .with_context(|| format!("unexpected output from git cat-file: {header}"))?;
        let mut content = vec![0u8; size];
        out.read_exact(&mut content)?;
        let mut newline = [0u8; 1];
        out.read_exact(&mut newline)?;
        results.push(Some(content));
    }
    let _ = writer.join();
    let _ = child.wait();
    Ok(results)
}

/// Hidden files and folders are not specs, as when reading the working tree.
fn is_hidden(path: &str) -> bool {
    path.split('/').any(|part| part.starts_with('.'))
}

/// The specs as they were at a revision.
pub fn graph_at(root: &Path, revision: &Revision) -> Result<Graph> {
    let prefix = prefix(root)?;
    let listing = git_stdout(root, &["ls-tree", "-r", "-z", "--name-only", &revision.oid])?;
    let files: Vec<String> = names(&listing)
        .filter(|n| n.ends_with(".md") && !is_hidden(n) && !n.contains('\n'))
        .collect();
    let specs: Vec<String> = files
        .iter()
        .map(|n| format!("{}:{prefix}{n}", revision.oid))
        .collect();
    let contents = read_objects(root, &specs)?;
    Ok(Graph::from_sources(
        files
            .into_iter()
            .zip(contents)
            .filter_map(|(name, content)| content.map(|bytes| (PathBuf::from(name), bytes))),
    ))
}

/// The newest `limit` commits that changed a file, following it across renames.
pub fn file_history(root: &Path, start: &str, path: &Path, limit: usize) -> Result<Vec<Commit>> {
    if start == "HEAD" && !has_commits(root)? {
        return Ok(Vec::new());
    }
    let path = path.to_string_lossy().replace('\\', "/");
    let max = format!("-n{limit}");
    let out = git_stdout(
        root,
        &[
            "-c",
            "core.quotepath=off",
            "log",
            "--follow",
            &max,
            "--format=%x1e%H%x1f%h%x1f%an%x1f%as%x1f%s",
            "--name-only",
            start,
            "--",
            &path,
        ],
    )?;
    let text = String::from_utf8_lossy(&out);
    let mut commits = Vec::new();
    for record in text.split('\x1e').filter(|r| !r.trim().is_empty()) {
        let mut lines = record.lines();
        let fields: Vec<&str> = lines.next().unwrap_or("").split('\x1f').collect();
        let [oid, short, author, date, subject] = fields[..] else {
            continue;
        };
        let name = lines
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or(path.as_str());
        commits.push(Commit {
            oid: oid.to_string(),
            short: short.to_string(),
            author: author.to_string(),
            date: date.to_string(),
            subject: subject.to_string(),
            path: PathBuf::from(name),
        });
    }
    Ok(commits)
}

/// The text of a file at a commit, or `None` when it did not exist there.
pub fn file_at(root: &Path, oid: &str, path: &Path) -> Result<Option<Vec<u8>>> {
    let spec = format!(
        "{oid}:{}{}",
        prefix(root)?,
        path.to_string_lossy().replace('\\', "/")
    );
    Ok(read_objects(root, &[spec])?.remove(0))
}

/// Markdown files that differ from `base` in the working tree (staged,
/// unstaged and untracked), and the IDs those files had at `base`.
///
/// In a repository with no commits yet, `HEAD` does not exist, so every
/// tracked or untracked file counts as changed.
pub fn changes(root: &Path, base: &str) -> Result<Changes> {
    require_repository(root, "check --changed")?;
    let base_commit = commit_of(root, base)?;

    let mut all: BTreeSet<String> = BTreeSet::new();
    if base_commit.is_some() {
        let diff = git_stdout(
            root,
            &["diff", "--name-only", "-z", "--relative", base, "--"],
        )?;
        all.extend(names(&diff));
    } else if base == "HEAD" {
        all.extend(names(&git_stdout(root, &["ls-files", "-z", "--cached"])?));
    } else {
        bail!("unknown git ref `{base}`");
    }
    let untracked = git_stdout(root, &["ls-files", "-z", "--others", "--exclude-standard"])?;
    all.extend(names(&untracked));
    let files: Vec<String> = all.into_iter().filter(|n| n.ends_with(".md")).collect();

    let mut changes = Changes::default();
    if let Some(oid) = base_commit {
        let prefix = prefix(root)?;
        let specs: Vec<String> = files.iter().map(|n| format!("{oid}:{prefix}{n}")).collect();
        for (name, content) in files.iter().zip(read_objects(root, &specs)?) {
            let text = content.map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
            if let Some(text) = text
                && let (Some(node), _) = parse_file_text(Path::new(name), &text)
            {
                changes.old_ids.insert(node.id);
            }
        }
    }
    changes.paths.extend(files.into_iter().map(PathBuf::from));
    Ok(changes)
}
