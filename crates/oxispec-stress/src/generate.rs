//! Writes a synthetic project: requirements with scenarios and links, ADRs, a vocabulary
//! with claims, state machines, an architecture model, annotated code, and git history.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct Shape {
    pub requirements: usize,
    pub adrs: usize,
    pub variables: usize,
    pub claims_per_requirement: usize,
    pub machines: usize,
    pub machine_states: usize,
    pub containers: usize,
    pub commits: usize,
    /// Pairs of claims that contradict each other, planted on purpose.
    pub conflicts: usize,
    /// Extra claims on one variable, to stress the solver.
    pub hot_claims: usize,
    /// Extra kilobytes of text in each requirement.
    pub fat_kb: usize,
    /// A chain of requirements that each refine the previous one.
    pub chain: usize,
    pub seed: u64,
}

impl Shape {
    pub fn preset(name: &str) -> Result<Shape> {
        let (requirements, adrs, machines, machine_states, containers, commits) = match name {
            "tiny" => (30, 5, 2, 10, 8, 5),
            "small" => (200, 20, 5, 20, 30, 50),
            "medium" => (2_000, 200, 20, 50, 300, 300),
            "large" => (10_000, 1_000, 50, 100, 1_500, 1_000),
            other => bail!("unknown preset `{other}` (tiny, small, medium, large)"),
        };
        Ok(Shape {
            requirements,
            adrs,
            variables: 50,
            claims_per_requirement: 2,
            machines,
            machine_states,
            containers,
            commits,
            conflicts: 0,
            hot_claims: 0,
            fat_kb: 0,
            chain: 0,
            seed: 1,
        })
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const WORDS: &[&str] = &[
    "session", "token", "order", "invoice", "payment", "account", "profile", "audit", "export",
    "import", "report", "queue", "retry", "limit", "quota", "policy", "role", "tenant", "billing",
    "refund", "shipment", "catalog", "price", "discount", "search", "index", "cache", "backup",
    "restore", "upload", "download", "notify", "email", "webhook", "schedule", "archive", "delete",
    "approve", "review", "comment", "tag", "label", "filter", "sort", "paginate", "validate",
    "encrypt", "rotate", "revoke", "login", "logout", "password", "device", "region", "currency",
];

fn sentence(rng: &mut Rng, words: usize) -> String {
    (0..words)
        .map(|_| WORDS[rng.below(WORDS.len())])
        .collect::<Vec<_>>()
        .join(" ")
}

pub const DOMAINS: usize = 20;

pub fn req_id(i: usize) -> String {
    format!("REQ-D{:02}-{:04}", i % DOMAINS, i / DOMAINS + 1)
}

fn req_path(i: usize) -> String {
    format!("specs/requirements/d{:02}/r{i}.md", i % DOMAINS)
}

fn write(root: &Path, rel: &str, text: &str) -> Result<()> {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().context("a path with a folder")?)?;
    fs::write(&path, text).with_context(|| format!("cannot write {}", path.display()))
}

const TIERS: [&str; 4] = ["free", "pro", "enterprise", "admin"];

fn requirement(i: usize, shape: &Shape, rng: &mut Rng) -> String {
    let mut links = String::new();
    let mut targets = Vec::new();
    if shape.chain > 0 && i > 0 && i < shape.chain {
        targets.push(format!("refines: [{}]", req_id(i - 1)));
    } else if i >= DOMAINS && rng.below(3) == 0 {
        targets.push(format!("refines: [{}]", req_id(rng.below(i))));
    }
    if shape.adrs > 0 && rng.below(2) == 0 {
        targets.push(format!(
            "decided-by: [ADR-{:04}]",
            rng.below(shape.adrs) + 1
        ));
    }
    if !targets.is_empty() {
        links.push_str("links:\n");
        for t in &targets {
            let (kind, rest) = t.split_once(": ").unwrap();
            let _ = writeln!(links, "  {kind}: {rest}");
        }
    }
    let mut body = String::new();
    let _ = write!(
        body,
        "---\nid: {}\ntype: requirement\nstatus: accepted\n{links}---\n### Requirement: {}\nThe system SHALL {} when {}.\n",
        req_id(i),
        sentence(rng, 3),
        sentence(rng, 8),
        sentence(rng, 6),
    );
    for k in 1..=2 {
        let _ = write!(
            body,
            "\n#### Scenario: {} {{#S{k}}}\n- **WHEN** {}\n- **THEN** {}\n",
            sentence(rng, 3),
            sentence(rng, 6),
            sentence(rng, 6)
        );
    }
    if shape.fat_kb > 0 {
        body.push('\n');
        while body.len() < shape.fat_kb * 1024 {
            let _ = writeln!(body, "{}.", sentence(rng, 18));
        }
    }
    if shape.variables > 0 && shape.claims_per_requirement > 0 {
        body.push_str("\n```claims\n");
        for k in 0..shape.claims_per_requirement {
            let v = (i * 7 + k) % shape.variables;
            let tier = TIERS[(i + k) % TIERS.len()];
            let _ = writeln!(body, "for tier = {tier}: limit.{v} <= {}", 100 + i);
        }
        body.push_str("```\n");
    }
    body
}

fn machine(n: usize, states: usize) -> String {
    let states = states.max(3);
    let names: Vec<String> = (0..states).map(|i| format!("s{i}")).collect();
    let mut t = String::new();
    for i in 0..states - 1 {
        let _ = writeln!(t, "  - {{from: s{i}, event: next, to: s{}}}", i + 1);
        let _ = writeln!(
            t,
            "  - {{from: s{i}, event: skip, to: s{}}}",
            (i + 3).min(states - 1)
        );
    }
    format!(
        "---\nid: SM-{n:04}\ntype: state-machine\nstatus: accepted\n---\n# SM-{n:04}: Generated machine\n\n```statemachine\ninitial: s0\nstates: [{}]\nfinal: [s{}]\ntransitions:\n{t}properties:\n  - s0 leads to s{}\n  - next requires s0\n  - never skip after s{}\n```\n",
        names.join(", "),
        states - 1,
        states - 1,
        states - 1,
    )
}

fn git(root: &Path, args: &[&str]) -> Result<()> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_COMMITTER_NAME", "stress")
        .env("GIT_COMMITTER_EMAIL", "stress@example.com")
        .env("GIT_AUTHOR_NAME", "stress")
        .env("GIT_AUTHOR_EMAIL", "stress@example.com")
        .output()
        .context("cannot run git")?;
    if !out.status.success() {
        bail!("git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(())
}

/// Writes the project into `root`, which must be empty or missing.
pub fn generate(bin: &Path, root: &Path, shape: &Shape) -> Result<()> {
    if root.exists() && fs::read_dir(root)?.next().is_some() {
        bail!("{} is not empty", root.display());
    }
    fs::create_dir_all(root)?;
    git(root, &["init", "-q", "-b", "main"])?;
    let out = Command::new(bin)
        .args(["init", "--preset", "standard", "--agents", "none"])
        .current_dir(root)
        .output()
        .context("cannot run oxispec")?;
    if !out.status.success() {
        bail!("oxispec init: {}", String::from_utf8_lossy(&out.stderr));
    }
    let mut rng = Rng(shape.seed.max(1));

    for i in 0..shape.requirements {
        write(root, &req_path(i), &requirement(i, shape, &mut rng))?;
    }
    for i in 0..shape.adrs {
        write(
            root,
            &format!("specs/adr/{:04}-decision.md", i + 1),
            &format!(
                "---\nid: ADR-{:04}\ntype: adr\nstatus: accepted\n---\n# ADR-{:04}: {}\n\n## Context\n{}.\n\n## Decision\n{}.\n",
                i + 1,
                i + 1,
                sentence(&mut rng, 4),
                sentence(&mut rng, 20),
                sentence(&mut rng, 12)
            ),
        )?;
    }

    // Each planted conflict is two requirements that disagree about a variable of their own.
    for c in 0..shape.conflicts {
        for (side, bound) in [("A", "<= 10"), ("B", ">= 20")] {
            write(
                root,
                &format!("specs/requirements/cf/{}{c}.md", side.to_lowercase()),
                &format!(
                    "---\nid: REQ-CF{side}-{:04}\ntype: requirement\nstatus: accepted\n---\n### Requirement: Planted {side}\nThe system SHALL keep conflict {c} {bound}.\n\n#### Scenario: S {{#S1}}\n- **WHEN** a\n- **THEN** b\n\n```claims\nfor tier = free: conflict.{c} {bound}\n```\n",
                    c + 1
                ),
            )?;
        }
    }

    let mut vocab = String::from(
        "tier: {type: enum, values: [free, pro, enterprise, admin]}\nhot: {type: int}\n",
    );
    for v in 0..shape.variables {
        let _ = writeln!(vocab, "limit.{v}: {{type: int}}");
    }
    for c in 0..shape.conflicts {
        let _ = writeln!(vocab, "conflict.{c}: {{type: int}}");
    }
    write(
        root,
        "specs/vocabulary/0001-generated.md",
        &format!(
            "---\nid: VOC-0001\ntype: vocabulary\nstatus: accepted\n---\n# VOC-0001: Generated\n\n```vocabulary\n{vocab}```\n"
        ),
    )?;
    if shape.hot_claims > 0 {
        let mut text = String::from(
            "---\nid: REQ-HOT-0001\ntype: requirement\nstatus: accepted\n---\n### Requirement: Hot variable\nThe system SHALL bound hot.\n\n#### Scenario: S {#S1}\n- **WHEN** a\n- **THEN** b\n\n```claims\n",
        );
        for k in 0..shape.hot_claims {
            let _ = writeln!(
                text,
                "for tier = {}: hot <= {}",
                TIERS[k % 4],
                1_000_000 - k
            );
        }
        text.push_str("```\n");
        write(root, "specs/requirements/hot/hot.md", &text)?;
    }

    for m in 0..shape.machines {
        write(
            root,
            &format!("specs/machines/{:04}-generated.md", m + 1),
            &machine(m + 1, shape.machine_states),
        )?;
    }

    if shape.containers > 0 {
        let a = |id: &str, ty: &str, block: &str| {
            format!(
                "---\nid: {id}\ntype: {ty}\nstatus: accepted\n---\n# {id}\n\n```architecture\n{block}```\n"
            )
        };
        write(
            root,
            "architecture/model/0001-system.md",
            &a("C4-0001", "system", "name: Generated system\n"),
        )?;
        let mut hosts = Vec::new();
        for c in 0..shape.containers {
            let id = format!("C4-{:04}", c + 2);
            let mut uses = String::new();
            if c > 0 {
                for _ in 0..3 {
                    let to = format!("C4-{:04}", rng.below(c) + 2);
                    let _ = writeln!(
                        uses,
                        "  - {{to: {to}, description: calls, technology: HTTPS}}"
                    );
                }
            }
            let block = format!(
                "name: Service {c}\ntechnology: Rust\nowner: team-{}\nparent: C4-0001\n{}{uses}",
                c % 10,
                if uses.is_empty() { "" } else { "uses:\n" }
            );
            write(
                root,
                &format!("architecture/model/{:04}-container.md", c + 2),
                &a(&id, "container", &block),
            )?;
            hosts.push(id);
        }
        let mut nodes = String::new();
        for (n, chunk) in hosts.chunks(50).enumerate() {
            let _ = writeln!(
                nodes,
                "  - {{id: node{n}, kind: kubernetes, region: eu-west-1, hosts: [{}]}}",
                chunk.join(", ")
            );
        }
        write(
            root,
            "architecture/model/9000-prod.md",
            &a(
                "DEP-0001",
                "deployment-environment",
                &format!("name: Production\nnodes:\n{nodes}"),
            ),
        )?;
    }

    // Code: one file per 20 requirements. 70% of requirements are implemented, 40% tested.
    for (file, chunk) in (0..shape.requirements)
        .collect::<Vec<_>>()
        .chunks(20)
        .enumerate()
    {
        let mut src = String::new();
        for &i in chunk {
            if rng.below(10) < 7 {
                let _ = writeln!(
                    src,
                    "// spec: {}\npub fn f{i}() -> usize {{\n    {i}\n}}\n",
                    req_id(i)
                );
            }
            if rng.below(10) < 4 {
                let _ = writeln!(
                    src,
                    "// verifies: {}.S1\n#[test]\nfn t{i}() {{\n    assert_eq!({i}, {i});\n}}\n",
                    req_id(i)
                );
            }
        }
        write(root, &format!("src/generated/m{file}.rs"), &src)?;
    }

    git(root, &["add", "-A"])?;
    git(root, &["commit", "-q", "-m", "generated project"])?;
    git(root, &["tag", "v0"])?;
    for c in 0..shape.commits {
        for _ in 0..3 {
            let i = rng.below(shape.requirements.max(1));
            let path = root.join(req_path(i));
            if let Ok(mut text) = fs::read_to_string(&path) {
                let _ = writeln!(text, "\nNote {c}: {}.", sentence(&mut rng, 6));
                fs::write(&path, text)?;
            }
        }
        git(root, &["commit", "-q", "-a", "-m", &format!("change {c}")])?;
        if c == shape.commits / 2 {
            git(root, &["tag", "v1"])?;
        }
    }
    Ok(())
}
