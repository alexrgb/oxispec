//! `oxispec import specs`: specs written for OpenSpec, Spec Kit, Kiro or plain
//! markdown, brought in as requirement nodes.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use oxispec_core::OUTPUT_SCHEMA_VERSION;
use oxispec_core::config::{self, IdScheme};
use oxispec_core::formats;
use oxispec_core::import::{self, Source};
use oxispec_core::import_plan::{self, Kind, Known, Plan};
use oxispec_core::styles::Style;
use serde_json::json;

use crate::{allocate_id, load_graph, next_number, require_module, slug, write_new};

/// What `oxispec import specs` was asked to do.
pub struct Options<'a> {
    pub from: &'a str,
    pub dir: Option<&'a Path>,
    pub format: Option<&'a str>,
    pub status: Option<&'a str>,
    pub scheme: Option<IdScheme>,
    pub dry_run: bool,
    pub json: bool,
}

fn shown(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

// spec: REQ-IMPORT-0003
pub fn import_specs(root: &Path, opts: &Options, cache: bool) -> Result<()> {
    let cfg = config::load(root)?;
    require_module(&cfg, "specs")?;
    let Some(source) = Source::parse(opts.from) else {
        bail!(
            "unknown source `{}`; the sources are {}",
            opts.from,
            import::NAMES.join(", ")
        );
    };
    let dir: PathBuf = match (opts.dir, source.default_dir()) {
        (Some(d), _) => d.to_path_buf(),
        (None, Some(d)) => root.join(d),
        (None, None) => {
            bail!(
                "`{}` has no default place; pass `--dir` with a file or folder",
                opts.from
            )
        }
    };
    if !dir.exists() {
        bail!("{} does not exist; pass `--dir`", dir.display());
    }
    let (profiles, _) = formats::available(&cfg);
    let asked = opts
        .format
        .or_else(|| cfg.formats.get("requirement").map(String::as_str))
        .unwrap_or("shall");
    let profile = match profiles.iter().find(|p| p.name == asked) {
        Some(p) if p.ty == "requirement" => p,
        Some(p) => bail!(
            "the profile `{asked}` is for {} nodes, not requirements",
            p.ty
        ),
        None => bail!("no profile named `{asked}`; see `oxispec formats list`"),
    };
    let normative = profile.style.is_none_or(Style::needs_normative);
    let status = chosen_status(profile, opts.status)?;
    let dated = profile
        .fields
        .iter()
        .chain(&profile.optional_fields)
        .any(|f| f == "date");
    let today = oxispec_core::debt::today();

    let found =
        import::find(&dir, source).with_context(|| format!("cannot read {}", dir.display()))?;
    let drafts: Vec<_> = found
        .inputs
        .iter()
        .map(|i| import::draft(i, source, normative))
        .collect();

    let mut ids = Ids {
        graph: load_graph(root, cache),
        scheme: opts.scheme.unwrap_or(cfg.id_scheme),
        counters: HashMap::new(),
        taken: HashSet::new(),
    };
    let mut paths: HashSet<PathBuf> = HashSet::new();
    let mut planned: Vec<Vec<Node>> = Vec::new();
    for d in &drafts {
        let nodes = nodes_of(
            d,
            &shown(root, &d.path),
            &mut ids,
            status,
            dated.then_some(today.as_str()),
            &mut paths,
        )?;
        for n in &nodes {
            if root.join(&n.rel).exists() {
                bail!(
                    "{} already exists; move it or import from a smaller folder",
                    n.rel.display()
                );
            }
        }
        planned.push(nodes);
    }
    if !opts.dry_run {
        for n in planned.iter().flatten() {
            write_new(root, &n.rel, &n.text)?;
        }
    }

    let skipped: Vec<_> = found
        .unmapped
        .iter()
        .map(|u| (shown(root, &u.path), u.reason.as_str()))
        .collect();
    if opts.json {
        let items: Vec<_> = drafts
            .iter()
            .zip(&planned)
            .flat_map(|(d, nodes)| {
                nodes.iter().map(move |n| {
                    json!({
                        "id": n.id,
                        "path": shown(root, &n.rel),
                        "source": shown(root, &d.path),
                        "requirements": usize::from(!d.blocks.is_empty()),
                        "notes": n.notes,
                    })
                })
            })
            .collect();
        let skipped: Vec<_> = skipped
            .iter()
            .map(|(p, r)| json!({"path": p, "reason": r}))
            .collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "from": opts.from,
                "format": profile.name,
                "dry_run": opts.dry_run,
                "nodes": items,
                "not_imported": skipped,
            }))?
        );
        return Ok(());
    }
    for (d, nodes) in drafts.iter().zip(&planned) {
        let (first, last) = (nodes.first(), nodes.last());
        outln!(
            "{} {} -> {} node(s), {} to {}",
            if opts.dry_run {
                "would import"
            } else {
                "imported"
            },
            shown(root, &d.path),
            nodes.len(),
            first.map_or("", |n| n.id.as_str()),
            last.map_or("", |n| n.id.as_str()),
        );
        for n in nodes {
            for note in &n.notes {
                outln!("    {note}");
            }
        }
    }
    for (path, reason) in &skipped {
        outln!("not imported {path}: {reason}");
    }
    outln!(
        "{} node(s) in the `{}` format; run `oxispec check` to see what is left to fill in",
        planned.iter().map(Vec::len).sum::<usize>(),
        profile.name
    );
    Ok(())
}

/// The status for imported requirements: the one asked for, which the profile has to allow,
/// or the first the profile lists.
fn chosen_status<'a>(profile: &'a formats::Profile, asked: Option<&'a str>) -> Result<&'a str> {
    match asked {
        Some(s) if profile.statuses.is_empty() || profile.statuses.iter().any(|t| t == s) => Ok(s),
        Some(s) => bail!(
            "the status `{s}` is not one `{}` allows ({})",
            profile.name,
            profile.statuses.join(", ")
        ),
        None => Ok(profile.statuses.first().map_or("draft", String::as_str)),
    }
}

/// A requirement node that is about to be written.
struct Node {
    id: String,
    rel: PathBuf,
    text: String,
    notes: Vec<String>,
    title: String,
}

/// A short file name for a requirement: the words of its title.
fn file_stem(title: &str, fallback: &str) -> String {
    let stem = slug(title).unwrap_or_else(|_| fallback.to_string());
    if stem.len() <= 60 {
        return stem;
    }
    let cut = &stem[..60];
    cut.rsplit_once('-')
        .map_or(cut, |(words, _)| words)
        .to_string()
}

/// The nodes of one spec file, a node for each requirement in it, with their
/// IDs and files. A name that is taken in the run gets a number after it.
fn nodes_of(
    d: &import::Draft,
    source: &str,
    ids: &mut Ids,
    status: &str,
    date: Option<&str>,
    paths: &mut HashSet<PathBuf>,
) -> Result<Vec<Node>> {
    let domain = d.domain();
    let prefix = format!("REQ-{domain}-");
    let dir = PathBuf::from("specs/requirements").join(domain.to_ascii_lowercase());
    let mut out = Vec::new();
    for i in 0..d.node_count() {
        let id = ids.next(&prefix)?;
        let title = d
            .blocks
            .get(i)
            .map_or(d.name.as_str(), |b| b.title.as_str());
        let stem = file_stem(title, &domain.to_ascii_lowercase());
        let mut rel = dir.join(format!("{stem}.md"));
        let mut n = 2;
        while !paths.insert(rel.clone()) {
            rel = dir.join(format!("{stem}-{n}.md"));
            n += 1;
        }
        out.push(Node {
            text: d.render_block(i, &id, status, date, source),
            notes: d.notes_for(i),
            title: title.to_string(),
            id,
            rel,
        });
    }
    Ok(out)
}

/// Hands out IDs for a run that writes several nodes: sequential numbers
/// continue from the project's, and random ones do not repeat.
struct Ids {
    graph: oxispec_core::Graph,
    scheme: IdScheme,
    counters: HashMap<String, u32>,
    taken: HashSet<String>,
}

impl Ids {
    fn next(&mut self, prefix: &str) -> Result<String> {
        match self.scheme {
            IdScheme::Sequential => {
                let n = self
                    .counters
                    .entry(prefix.to_string())
                    .or_insert_with(|| next_number(&self.graph, prefix));
                let id = format!("{prefix}{n:04}");
                *n += 1;
                Ok(id)
            }
            IdScheme::Random => loop {
                let id = allocate_id(&self.graph, prefix, self.scheme)?;
                if self.taken.insert(id.clone()) {
                    return Ok(id);
                }
            },
        }
    }
}

const PLAN_PATH: &str = ".oxispec/import-plan.json";

fn read_plan(root: &Path) -> Result<Plan> {
    let path = root.join(PLAN_PATH);
    let text = std::fs::read_to_string(&path)
        .with_context(|| "there is no import plan; start one with `oxispec import plan start`")?;
    serde_json::from_str(&text).with_context(|| format!("cannot read {}", path.display()))
}

fn write_plan(root: &Path, plan: &Plan) -> Result<()> {
    let path = root.join(PLAN_PATH);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(plan)? + "\n")?;
    Ok(())
}

fn item_json(i: &import_plan::Item) -> serde_json::Value {
    json!({"n": i.n, "kind": i.kind.name(), "role": i.role, "title": i.title,
           "sources": i.sources, "why": i.why})
}

fn print_plan(plan: &Plan) {
    for i in &plan.items {
        let more = if i.sources.len() > 1 {
            format!(" (+{} more)", i.sources.len() - 1)
        } else {
            String::new()
        };
        outln!(
            "{:>2}  {:<11} {:<24} {}{more}",
            i.n,
            i.kind.name(),
            i.title,
            i.sources.first().map_or("", String::as_str)
        );
        outln!("                {}", i.why);
    }
}

/// The source files that nodes under `specs/` say they were imported from.
fn already_imported(dir: &Path) -> HashSet<String> {
    let mut found = HashSet::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "md")
                && let Ok(text) = std::fs::read_to_string(&p)
            {
                for line in text.lines() {
                    if let Some(rest) = line
                        .strip_prefix("<!-- imported from ")
                        .and_then(|r| r.strip_suffix(" -->"))
                    {
                        found.extend(rest.split(", ").map(str::to_string));
                    }
                }
            }
        }
    }
    found
}

pub fn plan_start(
    root: &Path,
    from: &str,
    dir: Option<&Path>,
    force: bool,
    only: &[String],
    json: bool,
) -> Result<()> {
    let cfg = config::load(root)?;
    require_module(&cfg, "specs")?;
    let mut only_kinds = Vec::new();
    for k in only {
        match Kind::parse(k) {
            Some(kind) => only_kinds.push(kind),
            None => bail!(
                "unknown kind `{k}`; the kinds are {}",
                Kind::NAMES.join(", ")
            ),
        }
    }
    let Some(source) = Source::parse(from) else {
        bail!(
            "unknown source `{from}`; the sources are {}",
            import::NAMES.join(", ")
        );
    };
    let dir: PathBuf = match (dir, source.default_dir()) {
        (Some(d), _) => d.to_path_buf(),
        (None, Some(d)) => root.join(d),
        (None, None) => bail!("`{from}` has no default place; pass `--dir`"),
    };
    if source == Source::OpenApi {
        bail!(
            "an API description is read whole, so it has no plan; use `oxispec import specs --from openapi`"
        );
    }
    if !dir.exists() {
        bail!("{} does not exist; pass `--dir`", dir.display());
    }
    if root.join(PLAN_PATH).exists() && !force {
        bail!(
            "there is an import plan already; look at it with `oxispec import plan show`, or pass `--force` to start again"
        );
    }
    let plan = import_plan::propose(&dir, source, from, &|p| shown(root, p))
        .with_context(|| format!("cannot read {}", dir.display()))?;
    if plan.items.is_empty() {
        bail!("nothing to import was found in {}", dir.display());
    }
    let mut plan = plan;
    let done = already_imported(&root.join("specs"));
    for item in &mut plan.items {
        if item.role == import_plan::Role::Spec
            && item.sources.iter().all(|s| done.contains(s))
            && !item.sources.is_empty()
        {
            item.kind = import_plan::Kind::Skip;
            item.why = "already imported by `oxispec import specs`".to_string();
        }
    }
    if !only_kinds.is_empty() {
        for item in &mut plan.items {
            if item.kind != Kind::Skip && !only_kinds.contains(&item.kind) {
                item.kind = Kind::Skip;
                item.why = "left out by `--only`".to_string();
            }
        }
    }
    write_plan(root, &plan)?;
    if json {
        let items: Vec<_> = plan.items.iter().map(item_json).collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION, "from": from,
                "plan": PLAN_PATH, "items": items,
                "kinds": Kind::NAMES,
            }))?
        );
        return Ok(());
    }
    outln!("import plan for {from}, written to {PLAN_PATH}");
    print_plan(&plan);
    outln!(
        "change an item with `oxispec import plan set N --kind {}`",
        Kind::NAMES.join("|")
    );
    outln!(
        "look at a draft with `oxispec import plan show N --draft`, then `oxispec import apply --dry-run`"
    );
    Ok(())
}

pub fn plan_set(
    root: &Path,
    n: Option<usize>,
    kind: Option<&str>,
    title: Option<&str>,
    where_: Option<&str>,
) -> Result<()> {
    let mut plan = read_plan(root)?;
    let kind = match kind {
        Some(k) => match Kind::parse(k) {
            Some(k) => Some(k),
            None => bail!(
                "unknown kind `{k}`; the kinds are {}",
                Kind::NAMES.join(", ")
            ),
        },
        None => None,
    };
    let title = match title {
        Some(t) if t.trim().is_empty() => bail!("the title cannot be empty"),
        Some(t) => Some(t.trim().to_string()),
        None => None,
    };
    let numbers: Vec<usize> = match (n, where_) {
        (Some(_), Some(_)) => bail!("give an item number or `--where`, not both"),
        (None, None) => bail!("give an item number, or `--where kind=KIND` for several items"),
        (Some(n), None) => {
            if !plan.items.iter().any(|i| i.n == n) {
                bail!("the plan has no item {n}; it has 1 to {}", plan.items.len());
            }
            vec![n]
        }
        (None, Some(w)) => {
            let Some(name) = w.strip_prefix("kind=") else {
                bail!("`--where` reads `kind=KIND`, for example `kind=requirement`");
            };
            let Some(of) = Kind::parse(name) else {
                bail!(
                    "unknown kind `{name}`; the kinds are {}",
                    Kind::NAMES.join(", ")
                );
            };
            if title.is_some() {
                bail!("a title is for one item; give its number");
            }
            plan.items
                .iter()
                .filter(|i| i.kind == of)
                .map(|i| i.n)
                .collect()
        }
    };
    for item in plan.items.iter_mut().filter(|i| numbers.contains(&i.n)) {
        if let Some(k) = kind {
            item.kind = k;
        }
        if let Some(t) = &title {
            item.title = t.clone();
        }
        outln!("item {} {} {}", item.n, item.kind.name(), item.title);
    }
    if numbers.is_empty() {
        outln!("no item matches");
    }
    write_plan(root, &plan)?;
    Ok(())
}

/// What an item would write.
struct Out {
    n: usize,
    kind: Kind,
    id: String,
    rel: PathBuf,
    made: import_plan::Made,
}

fn build(root: &Path, plan: &Plan, status: Option<&str>, cache: bool) -> Result<Vec<Out>> {
    let cfg = config::load(root)?;
    require_module(&cfg, "specs")?;
    let Some(source) = Source::parse(&plan.from) else {
        bail!("the plan names an unknown source `{}`", plan.from);
    };
    let (profiles, _) = formats::available(&cfg);
    let wanted = |ty: &str, default: &str| -> Result<oxispec_core::formats::Profile> {
        let name = cfg.formats.get(ty).map_or(default, String::as_str);
        match profiles.iter().find(|p| p.name == name && p.ty == ty) {
            Some(p) => Ok(p.clone()),
            None => bail!("no {ty} profile named `{name}`; see `oxispec formats list`"),
        }
    };
    let req_profile = wanted("requirement", "shall")?;
    let normative = req_profile.style.is_none_or(Style::needs_normative);
    let status = chosen_status(&req_profile, status)?;
    let dated = req_profile
        .fields
        .iter()
        .chain(&req_profile.optional_fields)
        .any(|f| f == "date");
    let today = oxispec_core::debt::today();
    let graph = load_graph(root, cache);
    let mut known: Vec<Known> = graph
        .nodes
        .iter()
        .filter(|n| n.ty == "requirement")
        .map(|n| Known {
            id: n.id.clone(),
            titles: n.requirements.iter().map(|r| r.name.clone()).collect(),
        })
        .collect();
    let mut ids = Ids {
        graph,
        scheme: cfg.id_scheme,
        counters: HashMap::new(),
        taken: HashSet::new(),
    };
    let mut outs: Vec<Out> = Vec::new();
    let live = |k: Kind| plan.items.iter().filter(move |i| i.kind == k);
    let mut paths: HashSet<PathBuf> = HashSet::new();
    for item in live(Kind::Requirement) {
        let draft = import_plan::draft_of(root, item, source, normative)?;
        let nodes = nodes_of(
            &draft,
            &item.sources.join(", "),
            &mut ids,
            status,
            dated.then_some(today.as_str()),
            &mut paths,
        )?;
        for n in nodes {
            known.push(Known {
                id: n.id.clone(),
                titles: vec![n.title.clone()],
            });
            outs.push(Out {
                n: item.n,
                kind: item.kind,
                id: n.id,
                rel: n.rel,
                made: import_plan::Made {
                    text: n.text,
                    notes: n.notes,
                    titles: vec![n.title],
                },
            });
        }
    }
    for item in live(Kind::Change) {
        let id = ids.next("CHG-")?;
        let made = import_plan::render_change(root, item, &id, &known)?;
        let suffix = id.strip_prefix("CHG-").unwrap_or(&id);
        let rel =
            PathBuf::from("specs/changes").join(format!("{suffix}-{}.md", slug(&item.title)?));
        outs.push(Out {
            n: item.n,
            kind: item.kind,
            id,
            rel,
            made,
        });
    }
    let adrs = live(Kind::Adr).count();
    if adrs > 0 {
        require_module(&cfg, "adrs")?;
        let target = crate::adr::adr_profile(&cfg, None)?;
        let adr_status = ["proposed", "draft"]
            .into_iter()
            .find(|s| target.statuses.iter().any(|t| t == s))
            .or(target.statuses.first().map(String::as_str))
            .unwrap_or("proposed");
        for item in live(Kind::Adr) {
            let id = ids.next("ADR-")?;
            let made = import_plan::render_adr(root, item, &id, &target, adr_status, &today)?;
            let suffix = id.strip_prefix("ADR-").unwrap_or(&id);
            let rel =
                PathBuf::from("specs/adr").join(format!("{suffix}-{}.md", slug(&item.title)?));
            outs.push(Out {
                n: item.n,
                kind: item.kind,
                id,
                rel,
                made,
            });
        }
    }
    outs.sort_by_key(|o| o.n);
    for o in &outs {
        if root.join(&o.rel).exists() {
            bail!(
                "{} already exists; item {} may be imported already (skip it with `oxispec import plan set {} --kind skip`) or needs another name (`--title`)",
                o.rel.display(),
                o.n,
                o.n
            );
        }
    }
    Ok(outs)
}

pub fn plan_show(
    root: &Path,
    n: Option<usize>,
    draft: bool,
    json: bool,
    cache: bool,
) -> Result<()> {
    let plan = read_plan(root)?;
    let Some(n) = n else {
        if json {
            let items: Vec<_> = plan.items.iter().map(item_json).collect();
            outln!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "schema_version": OUTPUT_SCHEMA_VERSION, "from": plan.from,
                    "items": items, "kinds": Kind::NAMES,
                }))?
            );
        } else {
            print_plan(&plan);
        }
        return Ok(());
    };
    let Some(item) = plan.items.iter().find(|i| i.n == n) else {
        bail!("the plan has no item {n}; it has 1 to {}", plan.items.len());
    };
    if !draft {
        if json {
            outln!("{}", serde_json::to_string_pretty(&item_json(item))?);
        } else {
            print_plan(&Plan {
                from: plan.from.clone(),
                items: vec![item.clone()],
            });
        }
        return Ok(());
    }
    if item.kind == Kind::Skip {
        bail!("item {n} is skipped; set a kind to see its draft");
    }
    let outs = build(root, &plan, None, cache)?;
    let mine: Vec<&Out> = outs.iter().filter(|o| o.n == n).collect();
    let first = mine.first().context("the item has no draft")?;
    if json {
        let nodes: Vec<_> = mine
            .iter()
            .map(|o| {
                json!({"id": o.id, "path": shown(root, &o.rel),
                       "notes": o.made.notes, "text": o.made.text})
            })
            .collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION, "n": n, "kind": first.kind.name(),
                "id": first.id, "path": shown(root, &first.rel), "notes": first.made.notes,
                "text": first.made.text, "nodes": nodes,
            }))?
        );
    } else {
        for out in mine {
            outln!(
                "# item {n}: {} {} -> {}",
                out.kind.name(),
                out.id,
                shown(root, &out.rel)
            );
            for note in &out.made.notes {
                outln!("# note: {note}");
            }
            outln!("{}", out.made.text);
        }
    }
    Ok(())
}

// spec: REQ-IMPORT-0004
pub fn apply(
    root: &Path,
    status: Option<&str>,
    dry_run: bool,
    json: bool,
    cache: bool,
) -> Result<()> {
    let plan = read_plan(root)?;
    let outs = build(root, &plan, status, cache)?;
    if outs.is_empty() {
        bail!("every item is skipped; nothing to write");
    }
    if !dry_run {
        for o in &outs {
            write_new(root, &o.rel, &o.made.text)?;
        }
        std::fs::remove_file(root.join(PLAN_PATH))?;
    }
    let skipped: Vec<_> = plan.items.iter().filter(|i| i.kind == Kind::Skip).collect();
    if json {
        let nodes: Vec<_> = outs
            .iter()
            .map(|o| {
                json!({"n": o.n, "kind": o.kind.name(), "id": o.id,
                            "path": shown(root, &o.rel), "notes": o.made.notes})
            })
            .collect();
        let skipped: Vec<_> = skipped.iter().map(|i| item_json(i)).collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION, "dry_run": dry_run,
                "nodes": nodes, "skipped": skipped,
            }))?
        );
        return Ok(());
    }
    for o in &outs {
        outln!(
            "{} item {} -> {} ({}, {})",
            if dry_run { "would write" } else { "wrote" },
            o.n,
            shown(root, &o.rel),
            o.id,
            o.kind.name()
        );
        for note in &o.made.notes {
            outln!("    {note}");
        }
    }
    for i in skipped {
        outln!("skipped item {}: {}", i.n, i.title);
    }
    if dry_run {
        outln!("nothing was written; run `oxispec import apply` to write it");
    } else {
        outln!(
            "{} node(s) written and the plan removed; run `oxispec check`",
            outs.len()
        );
    }
    Ok(())
}
