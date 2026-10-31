//! `oxispec-stress`: makes big projects, times every command on them, and load-tests the
//! MCP server and concurrent command line use. For development and CI, not published.

mod generate;
mod load;
mod run;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use generate::Shape;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// The oxispec program to test. Default: the one next to this program
    #[arg(long, global = true)]
    bin: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Args, Clone)]
struct ShapeArgs {
    /// tiny, small, medium or large
    #[arg(long, default_value = "small")]
    preset: String,
    #[arg(long)]
    requirements: Option<usize>,
    #[arg(long)]
    adrs: Option<usize>,
    #[arg(long)]
    commits: Option<usize>,
    #[arg(long)]
    machines: Option<usize>,
    /// States in each generated state machine
    #[arg(long)]
    machine_states: Option<usize>,
    #[arg(long)]
    containers: Option<usize>,
    /// Pairs of contradicting claims to plant
    #[arg(long, default_value_t = 0)]
    conflicts: usize,
    /// Extra claims on one variable, which stresses the solver
    #[arg(long, default_value_t = 0)]
    hot_claims: usize,
    /// Extra kilobytes of text in every requirement
    #[arg(long, default_value_t = 0)]
    fat_kb: usize,
    /// A chain of this many requirements that refine each other
    #[arg(long, default_value_t = 0)]
    chain: usize,
    #[arg(long, default_value_t = 1)]
    seed: u64,
}

impl ShapeArgs {
    fn shape(&self) -> Result<Shape> {
        let mut s = Shape::preset(&self.preset)?;
        s.requirements = self.requirements.unwrap_or(s.requirements);
        s.adrs = self.adrs.unwrap_or(s.adrs);
        s.commits = self.commits.unwrap_or(s.commits);
        s.machines = self.machines.unwrap_or(s.machines);
        s.machine_states = self.machine_states.unwrap_or(s.machine_states);
        s.containers = self.containers.unwrap_or(s.containers);
        s.conflicts = self.conflicts;
        s.hot_claims = self.hot_claims;
        s.fat_kb = self.fat_kb;
        s.chain = self.chain;
        s.seed = self.seed;
        Ok(s)
    }
}

#[derive(Subcommand)]
enum Command {
    /// Write a synthetic project
    Generate {
        #[arg(long)]
        out: PathBuf,
        #[command(flatten)]
        shape: ShapeArgs,
    },
    /// Time every command on a project, and check each against its budget
    Run {
        /// A project made by `generate`. Without it one is made in a temporary folder
        #[arg(long)]
        dir: Option<PathBuf>,
        #[command(flatten)]
        shape: ShapeArgs,
        /// Runs of each command. The median is reported
        #[arg(long, default_value_t = 3)]
        repeat: usize,
        /// Do not fail when a command is over its budget
        #[arg(long)]
        no_budget: bool,
        /// Also write the results as JSON
        #[arg(long)]
        json: Option<PathBuf>,
    },
    /// Many MCP clients at once, or many command line processes sharing one project
    Load {
        #[arg(long)]
        dir: Option<PathBuf>,
        #[command(flatten)]
        shape: ShapeArgs,
        /// mcp, or cli for processes that share a project and its cache
        #[arg(long, default_value = "mcp")]
        mode: String,
        /// Clients (mcp) or reader processes (cli) at the same time
        #[arg(long, default_value_t = 8)]
        clients: usize,
        /// Requests per client (mcp) or commands per reader (cli)
        #[arg(long, default_value_t = 50)]
        requests: usize,
        /// Processes that add requirements while the readers read (cli only)
        #[arg(long, default_value_t = 0)]
        writers: usize,
        /// Fail when the 95th percentile is above this
        #[arg(long)]
        max_p95_ms: Option<f64>,
        /// Fail when more than this many requests fail
        #[arg(long, default_value_t = 0)]
        max_errors: usize,
        #[arg(long)]
        json: Option<PathBuf>,
    },
}

fn find_bin(asked: &Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = asked {
        return Ok(p.clone());
    }
    let me = std::env::current_exe()?;
    let name = if cfg!(windows) {
        "oxispec.exe"
    } else {
        "oxispec"
    };
    let next = me.with_file_name(name);
    if next.is_file() {
        return Ok(next);
    }
    bail!(
        "cannot find `{name}` next to {}; build it (`cargo build -p oxispec`) or pass --bin",
        me.display()
    )
}

/// A project to measure: the one given, or a new one that lives as long as the returned guard.
fn project(
    bin: &Path,
    dir: &Option<PathBuf>,
    shape: &ShapeArgs,
) -> Result<(PathBuf, Option<tempfile::TempDir>, usize)> {
    let s = shape.shape()?;
    match dir {
        Some(d) => {
            let n = std::fs::read_dir(d.join("specs/requirements"))
                .map(|r| r.count() * 100)
                .unwrap_or(s.requirements);
            Ok((d.clone(), None, n.max(s.requirements)))
        }
        None => {
            let tmp = tempfile::tempdir()?;
            let root = tmp.path().join("project");
            let started = std::time::Instant::now();
            generate::generate(bin, &root, &s)?;
            eprintln!(
                "generated {} requirements, {} commits in {:.1}s",
                s.requirements,
                s.commits,
                started.elapsed().as_secs_f64()
            );
            run::oracle(bin, &root, s.conflicts)
                .context("the generated project is not what was planned")?;
            eprintln!(
                "oracle: check reports exactly the {} planted conflicts",
                s.conflicts
            );
            Ok((root, Some(tmp), s.requirements))
        }
    }
}

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let bin = find_bin(&cli.bin)?;
    match cli.command {
        Command::Generate { out, shape } => {
            let s = shape.shape()?;
            generate::generate(&bin, &out, &s)?;
            println!("wrote {} requirements to {}", s.requirements, out.display());
            Ok(ExitCode::SUCCESS)
        }
        Command::Run {
            dir,
            shape,
            repeat,
            no_budget,
            json,
        } => {
            let (root, _guard, _) = project(&bin, &dir, &shape)?;
            let site = tempfile::tempdir()?;
            let results = run::run_all(&bin, &root, repeat, &site.path().join("site"))?;
            let preset = shape.preset.clone();
            let mut over = 0;
            println!(
                "{:<26} {:>9} {:>9} {:>9}  budget",
                "command", "median s", "max s", "peak MB"
            );
            let mut rows = Vec::new();
            for r in &results {
                let budget = run::budget(r.name, &preset);
                let mut note = String::new();
                if let Some(why) = &r.failed {
                    note = format!("FAILED {why}");
                    over += 1;
                } else if let Some((secs, mb)) = budget {
                    // The best run: a noisy machine makes runs slower, never faster.
                    let slow = r.best.as_secs_f64() > secs;
                    let big = r.rss_mb.is_some_and(|m| m > mb);
                    note = format!(
                        "{secs:.1}s, {mb:.0} MB{}",
                        match (slow || big, no_budget) {
                            (false, _) => "",
                            (true, false) => "  OVER",
                            (true, true) => "  over (not enforced)",
                        }
                    );
                    over += usize::from((slow || big) && !no_budget);
                }
                println!(
                    "{:<26} {:>9.3} {:>9.3} {:>9}  {note}",
                    r.name,
                    r.median.as_secs_f64(),
                    r.max.as_secs_f64(),
                    r.rss_mb.map_or("-".to_string(), |m| format!("{m:.0}"))
                );
                rows.push(serde_json::json!({
                    "command": r.name,
                    "median_s": r.median.as_secs_f64(),
                    "max_s": r.max.as_secs_f64(),
                    "peak_mb": r.rss_mb,
                    "failed": r.failed,
                }));
            }
            if let Some(path) = json {
                std::fs::write(
                    &path,
                    serde_json::to_string_pretty(
                        &serde_json::json!({"preset": preset, "commands": rows}),
                    )?,
                )?;
            }
            if over > 0 {
                eprintln!("{over} commands failed or are over budget");
                return Ok(ExitCode::from(1));
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Load {
            dir,
            shape,
            mode,
            clients,
            requests,
            writers,
            max_p95_ms,
            max_errors,
            json,
        } => {
            let (root, _guard, requirements) = project(&bin, &dir, &shape)?;
            let report = match mode.as_str() {
                "mcp" => load::mcp(&bin, &root, clients, requests, requirements),
                "cli" => load::cli(&bin, &root, clients, requests, writers),
                other => bail!("unknown mode `{other}` (mcp, cli)"),
            };
            report.print();
            let mut failed = report.errors() > max_errors;
            if writers > 0 {
                let dup = load::duplicate_ids(&bin, &root).context("cannot check afterwards")?;
                println!("{dup} requirements share an ID after {writers} concurrent writers");
            }
            if let Some(max) = max_p95_ms
                && report.p95_ms() > max
            {
                println!("p95 {:.1} ms is over {max} ms", report.p95_ms());
                failed = true;
            }
            if let Some(path) = json {
                std::fs::write(&path, serde_json::to_string_pretty(&report.json())?)?;
            }
            Ok(if failed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            })
        }
    }
}
