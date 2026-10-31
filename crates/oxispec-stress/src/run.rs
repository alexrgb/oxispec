//! Times every command on a project, with the peak memory of each process.

use std::fs::{self, File};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

pub struct Scenario {
    pub name: &'static str,
    pub args: Vec<String>,
    /// Remove the local cache first.
    pub cold: bool,
}

fn s(name: &'static str, args: &[&str], cold: bool) -> Scenario {
    Scenario {
        name,
        args: args.iter().map(|a| a.to_string()).collect(),
        cold,
    }
}

/// The commands a project is measured with, for a project made by `generate`.
pub fn scenarios(site_out: &Path) -> Vec<Scenario> {
    vec![
        s("check (cold)", &["check", "--json"], true),
        s("check (warm)", &["check", "--json"], false),
        s("check --changed", &["check", "--changed"], false),
        s("index", &["index"], true),
        s(
            "search",
            &["search", "session", "token", "--limit", "20"],
            false,
        ),
        s("trace", &["trace", "--json"], false),
        s("coverage", &["coverage", "--json"], false),
        s("claims", &["claims", "--json"], false),
        s("machine (all)", &["machine", "--json"], false),
        s(
            "machine tests",
            &["machine", "SM-0001", "--tests", "--limit", "200", "--json"],
            false,
        ),
        s(
            "architecture view",
            &["architecture", "view", "container", "C4-0001"],
            false,
        ),
        s(
            "architecture deployment",
            &["architecture", "view", "deployment", "DEP-0001"],
            false,
        ),
        s(
            "context pack",
            &["context", "REQ-D00-0001", "--budget", "8000"],
            false,
        ),
        s("history", &["history", "REQ-D00-0001", "--json"], false),
        s("diff v1", &["diff", "v1", "--json"], false),
        s("ci --base v1", &["ci", "--base", "v1", "--json"], false),
        Scenario {
            name: "site",
            args: vec![
                "site".into(),
                "--out".into(),
                site_out.display().to_string(),
            ],
            cold: false,
        },
    ]
}

pub struct Sample {
    pub wall: Duration,
    /// Peak resident memory of the process, in megabytes. Not measured on Windows.
    pub rss_mb: Option<f64>,
    pub code: i32,
    pub stderr: String,
}

#[cfg(unix)]
fn wait(child: &mut std::process::Child) -> Result<(i32, Option<f64>)> {
    let mut status: libc::c_int = 0;
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: `status` and `usage` are valid for writes, and the pid is our child's.
    let pid = unsafe { libc::wait4(child.id() as libc::pid_t, &mut status, 0, &mut usage) };
    if pid < 0 {
        anyhow::bail!("wait4 failed: {}", std::io::Error::last_os_error());
    }
    let code = if libc::WIFEXITED(status) {
        libc::WEXITSTATUS(status)
    } else {
        128 + libc::WTERMSIG(status)
    };
    // Linux reports kilobytes, macOS bytes.
    let mb = if cfg!(target_os = "macos") {
        usage.ru_maxrss as f64 / (1024.0 * 1024.0)
    } else {
        usage.ru_maxrss as f64 / 1024.0
    };
    Ok((code, Some(mb)))
}

#[cfg(not(unix))]
fn wait(child: &mut std::process::Child) -> Result<(i32, Option<f64>)> {
    let status = child.wait()?;
    Ok((status.code().unwrap_or(-1), None))
}

pub fn timed(bin: &Path, dir: &Path, args: &[String]) -> Result<Sample> {
    let log = tempfile::NamedTempFile::new()?;
    let started = Instant::now();
    let mut child = Command::new(bin)
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::from(File::create(log.path())?))
        .spawn()
        .with_context(|| format!("cannot run {}", bin.display()))?;
    let (code, rss_mb) = wait(&mut child)?;
    let wall = started.elapsed();
    let mut stderr = fs::read_to_string(log.path()).unwrap_or_default();
    stderr.truncate(500);
    Ok(Sample {
        wall,
        rss_mb,
        code,
        stderr,
    })
}

pub struct Result_ {
    pub name: &'static str,
    pub best: Duration,
    pub median: Duration,
    pub max: Duration,
    pub rss_mb: Option<f64>,
    pub failed: Option<String>,
}

pub fn run_all(bin: &Path, dir: &Path, repeat: usize, site_out: &Path) -> Result<Vec<Result_>> {
    let mut out = Vec::new();
    for sc in scenarios(site_out) {
        // Printed before the run, so a log shows which command was last when something dies.
        eprintln!("running {}: oxispec {}", sc.name, sc.args.join(" "));
        let mut walls = Vec::new();
        let mut rss: Option<f64> = None;
        let mut failed = None;
        for _ in 0..repeat.max(1) {
            if sc.cold {
                let _ = fs::remove_dir_all(dir.join(".oxispec/cache"));
            }
            let sample = timed(bin, dir, &sc.args)?;
            // 1 means findings, which is an answer. Anything else is a failure.
            if !matches!(sample.code, 0 | 1) {
                failed = Some(format!("exit {}: {}", sample.code, sample.stderr.trim()));
            }
            walls.push(sample.wall);
            rss = match (rss, sample.rss_mb) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            };
        }
        walls.sort();
        out.push(Result_ {
            name: sc.name,
            best: walls[0],
            median: walls[walls.len() / 2],
            max: *walls.last().unwrap(),
            rss_mb: rss,
            failed,
        });
    }
    Ok(out)
}

/// The most seconds a command may take on a project of this size, with the memory it may use.
/// These are about four times what was measured (the laptop for small and medium, a CI
/// runner for large), so a slower runner passes and a change that makes something several
/// times slower does not.
pub fn budget(name: &str, preset: &str) -> Option<(f64, f64)> {
    let table: &[(&str, [(f64, f64); 3])] = BUDGETS;
    let at = match preset {
        "small" => 0,
        "medium" => 1,
        "large" => 2,
        _ => return None,
    };
    table.iter().find(|(n, _)| *n == name).map(|(_, b)| b[at])
}

/// (seconds, megabytes) for small, medium and large, per scenario.
const BUDGETS: &[(&str, [(f64, f64); 3])] = &[
    ("check (cold)", [(1.0, 100.0), (1.0, 100.0), (3.0, 230.0)]),
    ("check (warm)", [(1.0, 100.0), (1.0, 100.0), (1.1, 270.0)]),
    (
        "check --changed",
        [(1.0, 100.0), (1.0, 100.0), (1.6, 270.0)],
    ),
    ("index", [(1.0, 100.0), (1.0, 100.0), (3.0, 200.0)]),
    ("search", [(1.0, 100.0), (1.0, 130.0), (1.4, 290.0)]),
    ("trace", [(1.0, 100.0), (1.0, 100.0), (2.6, 300.0)]),
    ("coverage", [(1.0, 100.0), (1.0, 100.0), (1.3, 290.0)]),
    ("claims", [(1.0, 100.0), (1.0, 100.0), (1.5, 370.0)]),
    ("machine (all)", [(1.0, 100.0), (1.0, 100.0), (1.0, 270.0)]),
    ("machine tests", [(1.0, 100.0), (1.0, 100.0), (1.0, 270.0)]),
    (
        "architecture view",
        [(1.0, 100.0), (1.0, 100.0), (1.0, 270.0)],
    ),
    (
        "architecture deployment",
        [(1.0, 100.0), (1.0, 100.0), (1.0, 270.0)],
    ),
    ("context pack", [(1.0, 100.0), (1.0, 100.0), (1.0, 270.0)]),
    ("history", [(1.0, 100.0), (1.0, 100.0), (1.3, 270.0)]),
    ("diff v1", [(1.0, 100.0), (2.8, 100.0), (6.1, 380.0)]),
    ("ci --base v1", [(1.2, 100.0), (4.1, 110.0), (7.5, 380.0)]),
    ("site", [(1.0, 100.0), (2.0, 100.0), (5.0, 270.0)]),
];

/// What `check` must say about a project made by `generate`: the planted conflicts and nothing
/// else that is an error or a warning. This is how a large run proves the answers stay right.
pub fn oracle(bin: &Path, dir: &Path, conflicts: usize) -> Result<()> {
    let out = Command::new(bin)
        .args(["check", "--json"])
        .current_dir(dir)
        .output()
        .context("cannot run oxispec")?;
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).context("check did not print JSON")?;
    let mut found = 0;
    let mut other = Vec::new();
    for f in v["findings"].as_array().into_iter().flatten() {
        match (f["severity"].as_str(), f["code"].as_str()) {
            (Some("error"), Some("E019")) => found += 1,
            (Some("error" | "warning"), Some(code)) => other.push(format!(
                "{code} {}",
                f["message"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(120)
                    .collect::<String>()
            )),
            _ => {}
        }
    }
    if found != conflicts || !other.is_empty() {
        anyhow::bail!(
            "check found {found} conflicts, expected {conflicts}; other findings: {:?}",
            &other[..other.len().min(5)]
        );
    }
    Ok(())
}
