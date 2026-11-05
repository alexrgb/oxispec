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
    let status = profile.statuses.first().map_or("draft", String::as_str);
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

    let graph = load_graph(root, cache);
    let scheme = opts.scheme.unwrap_or(cfg.id_scheme);
    let mut counters: HashMap<String, u32> = HashMap::new();
    let mut taken: HashSet<String> = HashSet::new();
    let mut planned = Vec::new();
    for d in &drafts {
        let domain = d.domain();
        let prefix = format!("REQ-{domain}-");
        let id = match scheme {
            IdScheme::Sequential => {
                let n = counters
                    .entry(prefix.clone())
                    .or_insert_with(|| next_number(&graph, &prefix));
                let id = format!("{prefix}{n:04}");
                *n += 1;
                id
            }
            IdScheme::Random => loop {
                let id = allocate_id(&graph, &prefix, scheme)?;
                if taken.insert(id.clone()) {
                    break id;
                }
            },
        };
        let name = slug(&d.name).unwrap_or_else(|_| domain.to_ascii_lowercase());
        let rel = PathBuf::from("specs/requirements")
            .join(domain.to_ascii_lowercase())
            .join(format!("{name}.md"));
        if root.join(&rel).exists() {
            bail!(
                "{} already exists; move it or import from a smaller folder",
                rel.display()
            );
        }
        let text = d.render(
            &id,
            status,
            dated.then_some(today.as_str()),
            &shown(root, &d.path),
        );
        planned.push((id, rel, text));
    }
    if !opts.dry_run {
        for (_, rel, text) in &planned {
            write_new(root, rel, text)?;
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
            .map(|(d, (id, rel, _))| {
                json!({
                    "id": id,
                    "path": shown(root, rel),
                    "source": shown(root, &d.path),
                    "requirements": d.blocks.len(),
                    "notes": d.notes,
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
    for (d, (id, rel, _)) in drafts.iter().zip(&planned) {
        outln!(
            "{} {} -> {} ({}, {} requirement(s))",
            if opts.dry_run {
                "would import"
            } else {
                "imported"
            },
            shown(root, &d.path),
            shown(root, rel),
            id,
            d.blocks.len()
        );
        for note in &d.notes {
            outln!("    {note}");
        }
    }
    for (path, reason) in &skipped {
        outln!("not imported {path}: {reason}");
    }
    outln!(
        "{} node(s) in the `{}` format; run `oxispec check` to see what is left to fill in",
        planned.len(),
        profile.name
    );
    Ok(())
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

pub fn plan_start(
    root: &Path,
    from: &str,
    dir: Option<&Path>,
    force: bool,
    json: bool,
) -> Result<()> {
    let cfg = config::load(root)?;
    require_module(&cfg, "specs")?;
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

pub fn plan_set(root: &Path, n: usize, kind: Option<&str>, title: Option<&str>) -> Result<()> {
    let mut plan = read_plan(root)?;
    let Some(item) = plan.items.iter_mut().find(|i| i.n == n) else {
        bail!("the plan has no item {n}; it has 1 to {}", plan.items.len());
    };
    if let Some(k) = kind {
        let Some(k) = Kind::parse(k) else {
            bail!(
                "unknown kind `{k}`; the kinds are {}",
                Kind::NAMES.join(", ")
            );
        };
        item.kind = k;
    }
    if let Some(t) = title {
        if t.trim().is_empty() {
            bail!("the title cannot be empty");
        }
        item.title = t.trim().to_string();
    }
    let line = format!("{} {} {}", item.n, item.kind.name(), item.title);
    write_plan(root, &plan)?;
    outln!("item {line}");
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

fn build(root: &Path, plan: &Plan, cache: bool) -> Result<Vec<Out>> {
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
    let status = req_profile.statuses.first().map_or("draft", String::as_str);
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
    for item in live(Kind::Requirement) {
        let domain = import::domain_of(&item.title);
        let id = ids.next(&format!("REQ-{domain}-"))?;
        let made = import_plan::render_requirement(
            root,
            item,
            source,
            &id,
            status,
            dated.then_some(today.as_str()),
            normative,
        )?;
        known.push(Known {
            id: id.clone(),
            titles: made.titles.clone(),
        });
        let rel = PathBuf::from("specs/requirements")
            .join(domain.to_ascii_lowercase())
            .join(format!(
                "{}.md",
                slug(&item.title).unwrap_or_else(|_| domain.to_ascii_lowercase())
            ));
        outs.push(Out {
            n: item.n,
            kind: item.kind,
            id,
            rel,
            made,
        });
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
                "{} already exists; rename item {} with `oxispec import plan set {} --title`",
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
    let outs = build(root, &plan, cache)?;
    let out = outs
        .iter()
        .find(|o| o.n == n)
        .context("the item has no draft")?;
    if json {
        outln!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": OUTPUT_SCHEMA_VERSION, "n": n, "kind": out.kind.name(),
                "id": out.id, "path": shown(root, &out.rel), "notes": out.made.notes,
                "text": out.made.text,
            }))?
        );
    } else {
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
    Ok(())
}

pub fn apply(root: &Path, dry_run: bool, json: bool, cache: bool) -> Result<()> {
    let plan = read_plan(root)?;
    let outs = build(root, &plan, cache)?;
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
