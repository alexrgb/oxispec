//! Load tests: many MCP clients at once (one server process each, or one shared HTTP
//! server), and many command line processes sharing a project.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
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

    /// p50, p95 and p99 in milliseconds, and requests per second.
    pub fn summary(&self) -> [f64; 4] {
        let all = self.all_sorted();
        [
            percentile(&all, 0.5),
            percentile(&all, 0.95),
            percentile(&all, 0.99),
            all.len() as f64 / self.wall.as_secs_f64().max(f64::EPSILON),
        ]
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

type Sample = (&'static str, Duration, Option<String>);

/// One client's session: an `initialize`, then `requests` calls from the usual mix.
/// `send` carries one JSON-RPC message to the server and returns its answer.
fn drive(
    requests: usize,
    seed: u64,
    requirements: usize,
    send: &mut dyn FnMut(Value) -> Result<Value>,
) -> Result<Vec<Sample>> {
    send(
        json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}),
    )?;
    let mut rng = Rng(seed.max(1));
    let mut out = Vec::new();
    for i in 1..=requests {
        let (tool, args) = pick(&mut rng, requirements);
        let started = Instant::now();
        let reply = send(
            json!({"jsonrpc": "2.0", "id": i, "method": "tools/call", "params": {"name": tool, "arguments": args}}),
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
    Ok(out)
}

/// A client with a server process of its own, over stdin and stdout.
fn client(
    bin: &Path,
    dir: &Path,
    requests: usize,
    seed: u64,
    requirements: usize,
) -> Result<Vec<Sample>> {
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
    let mut send = |value: Value| -> Result<Value> {
        writeln!(stdin, "{value}")?;
        stdin.flush()?;
        line.clear();
        if stdout.read_line(&mut line)? == 0 {
            bail!("the server closed the connection");
        }
        serde_json::from_str(line.trim()).with_context(|| format!("not JSON: {line}"))
    };
    let out = drive(requests, seed, requirements, &mut send);
    drop(stdin);
    let _ = child.wait();
    out
}

/// A client of a shared HTTP server: one connection, kept open, with a token.
fn http_client(
    addr: &str,
    token: &str,
    requests: usize,
    seed: u64,
    requirements: usize,
) -> Result<Vec<Sample>> {
    let mut stream = TcpStream::connect(addr).with_context(|| format!("cannot reach {addr}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(120)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut send = |value: Value| -> Result<Value> {
        let body = value.to_string();
        // One write per request, so the server sees it whole.
        let request = format!(
            "POST /mcp HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes())?;
        stream.flush()?;
        let mut status = String::new();
        reader.read_line(&mut status)?;
        let code: u16 = status
            .split(' ')
            .nth(1)
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        let (mut length, mut chunked) = (0usize, false);
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header)? == 0 {
                bail!("the server closed the connection");
            }
            if header.trim().is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':') {
                if name.eq_ignore_ascii_case("content-length") {
                    length = value.trim().parse().unwrap_or(0);
                } else if name.eq_ignore_ascii_case("transfer-encoding")
                    && value.to_ascii_lowercase().contains("chunked")
                {
                    chunked = true;
                }
            }
        }
        let mut answer = Vec::new();
        if chunked {
            // Each chunk is its size in hex, a line break, that many bytes, and a line break.
            loop {
                let mut size = String::new();
                reader.read_line(&mut size)?;
                let size = usize::from_str_radix(size.trim().split(';').next().unwrap_or(""), 16)
                    .context("a bad chunk size")?;
                let mut chunk = vec![0u8; size + 2];
                reader.read_exact(&mut chunk)?;
                if size == 0 {
                    break;
                }
                answer.extend_from_slice(&chunk[..size]);
            }
        } else {
            answer = vec![0u8; length];
            reader.read_exact(&mut answer)?;
        }
        if code != 200 {
            bail!("HTTP {code}: {}", String::from_utf8_lossy(&answer));
        }
        serde_json::from_slice(&answer).context("the answer is not JSON")
    };
    drive(requests, seed, requirements, &mut send)
}

/// Runs one session per client at once, and gathers their latencies.
fn gather(
    clients: usize,
    session: impl Fn(usize) -> Result<Vec<Sample>> + Send + Sync + 'static,
) -> Report {
    let started = Instant::now();
    let session = Arc::new(session);
    let handles: Vec<_> = (0..clients)
        .map(|c| {
            let session = session.clone();
            thread::spawn(move || session(c))
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

/// `clients` MCP servers at once, each answering `requests` calls one after another.
pub fn mcp(bin: &Path, dir: &Path, clients: usize, requests: usize, requirements: usize) -> Report {
    let (bin, dir) = (bin.to_path_buf(), dir.to_path_buf());
    gather(clients, move |c| {
        client(&bin, &dir, requests, 17 + c as u64 * 7919, requirements)
    })
}

const TOKEN_NAME: &str = "stress";

/// A server started by [`serve`]. It stops when this is dropped.
pub struct HttpServer {
    child: std::process::Child,
    pub addr: String,
    pub token: String,
    bin: PathBuf,
    dir: PathBuf,
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // The project may be one the caller keeps, so do not leave the token in it.
        let _ = Command::new(&self.bin)
            .arg("--root")
            .arg(&self.dir)
            .args(["token", "revoke", TOKEN_NAME])
            .output();
    }
}

/// Makes a token for the project and starts `oxispec serve` on a free local port.
pub fn serve(bin: &Path, dir: &Path) -> Result<HttpServer> {
    let made = Command::new(bin)
        .arg("--root")
        .arg(dir)
        .args(["token", "add", TOKEN_NAME])
        .output()
        .context("cannot run `token add`")?;
    if !made.status.success() {
        bail!(
            "`token add` failed: {}",
            String::from_utf8_lossy(&made.stderr)
        );
    }
    let token = String::from_utf8_lossy(&made.stdout).trim().to_string();
    let mut child = Command::new(bin)
        .arg("--root")
        .arg(dir)
        .args(["serve", "--listen", "127.0.0.1:0"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("cannot start `serve`")?;
    let mut line = String::new();
    BufReader::new(child.stderr.take().unwrap()).read_line(&mut line)?;
    let Some(addr) = line
        .split("http://")
        .nth(1)
        .and_then(|rest| rest.split("/mcp").next())
        .map(str::to_string)
    else {
        let _ = child.kill();
        bail!("`serve` did not say where it listens: {line:?}");
    };
    Ok(HttpServer {
        child,
        addr,
        token,
        bin: bin.to_path_buf(),
        dir: dir.to_path_buf(),
    })
}

/// `clients` clients of one HTTP server, each with its own connection.
pub fn http(server: &HttpServer, clients: usize, requests: usize, requirements: usize) -> Report {
    let (addr, token) = (server.addr.clone(), server.token.clone());
    gather(clients, move |c| {
        http_client(&addr, &token, requests, 17 + c as u64 * 7919, requirements)
    })
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

/// The median of each figure in [`Report::summary`] over several runs.
pub fn median_summary(runs: &[[f64; 4]]) -> [f64; 4] {
    let mut medians = [0.0; 4];
    for (i, slot) in medians.iter_mut().enumerate() {
        let mut column: Vec<f64> = runs.iter().map(|r| r[i]).collect();
        column.sort_by(f64::total_cmp);
        *slot = column[column.len() / 2];
    }
    medians
}

#[cfg(test)]
mod tests_of_load {
    use super::*;

    #[test]
    fn the_median_of_each_figure_is_taken_on_its_own() {
        let runs = [
            [5.0, 50.0, 90.0, 100.0],
            [60.0, 40.0, 80.0, 300.0],
            [20.0, 60.0, 70.0, 200.0],
        ];
        assert_eq!(median_summary(&runs), [20.0, 50.0, 80.0, 200.0]);
    }
}
