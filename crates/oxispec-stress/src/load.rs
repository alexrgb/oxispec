//! Load tests: many MCP clients at once, and many command line processes sharing a project.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::generate::req_id;

pub struct Latency {
    pub tool: String,
    pub micros: Vec<u128>,
    pub errors: usize,
}

pub struct Report {
    pub wall: Duration,
    pub per_tool: Vec<Latency>,
    pub failures: Vec<String>,
}

fn percentile(sorted: &[u128], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let at = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[at] as f64 / 1000.0
}

impl Report {
    pub fn errors(&self) -> usize {
        self.per_tool.iter().map(|l| l.errors).sum::<usize>()
    }

    pub fn all_sorted(&self) -> Vec<u128> {
        let mut all: Vec<u128> = self
            .per_tool
            .iter()
            .flat_map(|l| l.micros.clone())
            .collect();
        all.sort_unstable();
        all
    }

    pub fn p95_ms(&self) -> f64 {
        percentile(&self.all_sorted(), 0.95)
    }

    pub fn print(&self) {
        println!(
            "{:<22} {:>7} {:>7} {:>9} {:>9} {:>9} {:>9}",
            "tool", "calls", "errors", "p50 ms", "p95 ms", "p99 ms", "max ms"
        );
        for l in &self.per_tool {
            let mut v = l.micros.clone();
            v.sort_unstable();
            println!(
                "{:<22} {:>7} {:>7} {:>9.1} {:>9.1} {:>9.1} {:>9.1}",
                l.tool,
                v.len(),
                l.errors,
                percentile(&v, 0.5),
                percentile(&v, 0.95),
                percentile(&v, 0.99),
                percentile(&v, 1.0)
            );
        }
        let all = self.all_sorted();
        println!(
            "{:<22} {:>7} {:>7} {:>9.1} {:>9.1} {:>9.1} {:>9.1}",
            "all",
            all.len(),
            self.errors(),
            percentile(&all, 0.5),
            percentile(&all, 0.95),
            percentile(&all, 0.99),
            percentile(&all, 1.0)
        );
        println!(
            "{} requests in {:.1}s: {:.1} requests per second",
            all.len(),
            self.wall.as_secs_f64(),
            all.len() as f64 / self.wall.as_secs_f64().max(f64::EPSILON)
        );
        for f in self.failures.iter().take(10) {
            println!("failure: {f}");
        }
    }

    pub fn json(&self) -> Value {
        let all = self.all_sorted();
        json!({
            "requests": all.len(),
            "errors": self.errors(),
            "seconds": self.wall.as_secs_f64(),
            "p50_ms": percentile(&all, 0.5),
            "p95_ms": percentile(&all, 0.95),
            "p99_ms": percentile(&all, 0.99),
            "max_ms": percentile(&all, 1.0),
            "failures": self.failures,
        })
    }
}

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545F4914F6CDD1D) % n.max(1) as u64) as usize
    }
}

const QUERY_WORDS: &[&str] = &[
    "session", "token", "order", "payment", "retry", "limit", "audit",
];

/// One request of the mix an agent makes: mostly reads of one node, some searches,
/// a few whole-project questions.
fn pick(rng: &mut Rng, requirements: usize) -> (&'static str, Value) {
    let n = |rng: &mut Rng| req_id(rng.below(requirements.max(1)));
    match rng.below(100) {
        0..40 => (
            "spec_search",
            json!({"query": format!("{} {}", QUERY_WORDS[rng.below(7)], QUERY_WORDS[rng.below(7)]), "limit": 10}),
        ),
        40..65 => ("spec_get", json!({"id": n(rng)})),
        65..80 => (
            "spec_context",
            json!({"ids": [n(rng), n(rng)], "budget": 4000}),
        ),
        80..85 => ("spec_neighbors", json!({"id": n(rng)})),
        85..90 => (
            "spec_claims",
            json!({"variable": format!("limit.{}", rng.below(50))}),
        ),
        90..93 => ("spec_machine", json!({})),
        93..96 => ("spec_architecture", json!({})),
        _ => ("spec_check", json!({})),
    }
}

fn client(
    bin: &Path,
    dir: &Path,
    requests: usize,
    seed: u64,
    requirements: usize,
) -> Result<Vec<(&'static str, Duration, Option<String>)>> {
    let mut child = Command::new(bin)
        .arg("--root")
        .arg(dir)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("cannot start the MCP server")?;
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    let mut send = |value: Value, line: &mut String| -> Result<Value> {
        writeln!(stdin, "{value}")?;
        stdin.flush()?;
        line.clear();
        if stdout.read_line(line)? == 0 {
            bail!("the server closed the connection");
        }
        serde_json::from_str(line.trim()).with_context(|| format!("not JSON: {line}"))
    };
    send(
        json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}),
        &mut line,
    )?;
    let mut rng = Rng(seed.max(1));
    let mut out = Vec::new();
    for i in 1..=requests {
        let (tool, args) = pick(&mut rng, requirements);
        let started = Instant::now();
        let reply = send(
            json!({"jsonrpc": "2.0", "id": i, "method": "tools/call", "params": {"name": tool, "arguments": args}}),
            &mut line,
        )?;
        let failed = reply.get("error").is_some() || reply["result"]["isError"] == true;
        let why = failed.then(|| {
            let text = reply["result"]["content"][0]["text"]
                .as_str()
                .or_else(|| reply["error"]["message"].as_str())
                .unwrap_or("an error");
            format!("{tool}: {}", text.chars().take(200).collect::<String>())
        });
        out.push((tool, started.elapsed(), why));
    }
    drop(stdin);
    let _ = child.wait();
    Ok(out)
}

/// `clients` MCP servers at once, each answering `requests` calls one after another.
pub fn mcp(bin: &Path, dir: &Path, clients: usize, requests: usize, requirements: usize) -> Report {
    let started = Instant::now();
    let handles: Vec<_> = (0..clients)
        .map(|c| {
            let (bin, dir) = (bin.to_path_buf(), dir.to_path_buf());
            thread::spawn(move || client(&bin, &dir, requests, 17 + c as u64 * 7919, requirements))
        })
        .collect();
    let mut by_tool: std::collections::BTreeMap<&'static str, Latency> = Default::default();
    let mut failures = Vec::new();
    for h in handles {
        match h.join() {
            Ok(Ok(samples)) => {
                for (tool, d, why) in samples {
                    let l = by_tool.entry(tool).or_insert_with(|| Latency {
                        tool: tool.to_string(),
                        micros: Vec::new(),
                        errors: 0,
                    });
                    l.micros.push(d.as_micros());
                    if let Some(why) = why {
                        l.errors += 1;
                        if !failures.contains(&why) {
                            failures.push(why);
                        }
                    }
                }
            }
            Ok(Err(e)) => failures.push(format!("{e:#}")),
            Err(_) => failures.push("a client thread panicked".to_string()),
        }
    }
    Report {
        wall: started.elapsed(),
        per_tool: by_tool.into_values().collect(),
        failures,
    }
}

/// Many command line processes at once on one project: readers that must all succeed, and
/// optionally writers that add requirements while they read.
pub fn cli(bin: &Path, dir: &Path, readers: usize, rounds: usize, writers: usize) -> Report {
    let commands: Arc<Vec<Vec<&'static str>>> = Arc::new(vec![
        vec!["check", "--json"],
        vec!["search", "session", "token"],
        vec!["trace", "--json"],
        vec!["coverage", "--json"],
        vec!["claims", "--json"],
    ]);
    let bad = Arc::new(AtomicUsize::new(0));
    let started = Instant::now();
    let mut handles = Vec::new();
    for r in 0..readers {
        let (bin, dir, commands) = (bin.to_path_buf(), dir.to_path_buf(), commands.clone());
        handles.push(thread::spawn(move || {
            let mut out = Vec::new();
            for k in 0..rounds {
                let c = &commands[(r + k) % commands.len()];
                let args: Vec<String> = c.iter().map(|a| a.to_string()).collect();
                let name: &'static str = c[0];
                match crate::run::timed(&bin, &dir, &args) {
                    Ok(s) => out.push((name, s.wall, !matches!(s.code, 0 | 1), s.stderr)),
                    Err(e) => out.push((name, Duration::ZERO, true, e.to_string())),
                }
            }
            out
        }));
    }
    let mut writer_handles = Vec::new();
    for w in 0..writers {
        let (bin, dir, bad) = (bin.to_path_buf(), dir.to_path_buf(), bad.clone());
        writer_handles.push(thread::spawn(move || {
            let mut failures = Vec::new();
            for k in 0..rounds {
                let args = [
                    "new",
                    "requirement",
                    "race",
                    &format!("writer {w} number {k}"),
                ]
                .map(String::from);
                match crate::run::timed(&bin, &dir, &args) {
                    Ok(s) if s.code == 0 => {}
                    Ok(s) => {
                        bad.fetch_add(1, Ordering::Relaxed);
                        failures.push(format!(
                            "new requirement: exit {}: {}",
                            s.code,
                            s.stderr.trim()
                        ));
                    }
                    Err(e) => failures.push(e.to_string()),
                }
            }
            failures
        }));
    }
    let mut by_tool: std::collections::BTreeMap<&'static str, Latency> = Default::default();
    let mut failures = Vec::new();
    for h in handles {
        for (name, d, failed, stderr) in h.join().unwrap_or_default() {
            let l = by_tool.entry(name).or_insert_with(|| Latency {
                tool: name.to_string(),
                micros: Vec::new(),
                errors: 0,
            });
            l.micros.push(d.as_micros());
            if failed {
                l.errors += 1;
                failures.push(format!("{name}: {}", stderr.trim()));
            }
        }
    }
    for h in writer_handles {
        failures.extend(h.join().unwrap_or_default());
    }
    Report {
        wall: started.elapsed(),
        per_tool: by_tool.into_values().collect(),
        failures,
    }
}

/// After concurrent writers: how many requirements got the same ID.
pub fn duplicate_ids(bin: &Path, dir: &Path) -> Result<usize> {
    let out = Command::new(bin)
        .args(["check", "--json"])
        .current_dir(dir)
        .output()?;
    let v: Value = serde_json::from_slice(&out.stdout).context("check did not print JSON")?;
    Ok(v["findings"]
        .as_array()
        .map(|f| f.iter().filter(|f| f["code"] == "E001").count())
        .unwrap_or(0))
}
