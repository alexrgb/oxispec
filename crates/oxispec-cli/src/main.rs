use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand, ValueEnum};
use oxispec_core::check::{LinkResolution, resolve_link};
use oxispec_core::config::{self, Config, IdScheme};
use oxispec_core::edit::{LinkEdit, add_link};
use oxispec_core::impact::{impact, in_scope};
use oxispec_core::index;
use oxispec_core::node::KNOWN_LINK_KINDS;
use oxispec_core::{Finding, Graph, OUTPUT_SCHEMA_VERSION, Severity, check, id, modules};

#[macro_use]
mod output;

mod adr;
mod agents;
mod api;
mod architecture;
mod changes;
mod ci;
mod claims;
mod compliance;
mod connect;
mod context;
mod debt;
mod entities;
mod fetch;
mod formats;
mod git;
mod history;
mod importer;
mod machines;
mod mcp;
mod reference;
mod releases;
mod risks;
mod rollback;
mod search;
mod server;
mod site;
mod status;
mod testplan;
mod trace;

#[derive(Parser)]
#[command(
    name = "oxispec",
    version,
    about = "Linked specs, ADRs and checks for humans and agents"
)]
struct Cli {
    /// Project folder (default: the nearest parent with .oxispec/config.yaml)
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    /// Do not read or write the local index in .oxispec/cache
    #[arg(long, global = true)]
    no_cache: bool,
    #[command(subcommand)]
    command: Command,
}

/// How new IDs are numbered.
#[derive(Clone, Copy, ValueEnum)]
enum IdSchemeArg {
    /// REQ-AUTH-0001: the next free number
    Sequential,
    /// REQ-AUTH-7K3F: a random suffix, safe with parallel branches
    Random,
}

impl From<IdSchemeArg> for IdScheme {
    fn from(arg: IdSchemeArg) -> Self {
        match arg {
            IdSchemeArg::Sequential => IdScheme::Sequential,
            IdSchemeArg::Random => IdScheme::Random,
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Create .oxispec/config.yaml and the folders for the enabled modules
    Init {
        /// minimal, specs-tests, docs, standard or full
        #[arg(long, default_value = "minimal")]
        preset: String,
        /// Extra modules to enable, comma separated
        #[arg(long, value_delimiter = ',')]
        with: Vec<String>,
        /// Modules to leave out of the preset, comma separated
        #[arg(long, value_delimiter = ',')]
        without: Vec<String>,
        /// How new IDs are numbered. Use random when several branches add specs at once.
        #[arg(long, value_enum, default_value_t = IdSchemeArg::Sequential)]
        id_scheme: IdSchemeArg,
        /// AI clients that get skills when the agents module is on, comma separated: claude,
        /// cursor, copilot, gemini, agents, or none (default: claude,cursor,copilot,gemini)
        #[arg(long, value_delimiter = ',')]
        agents: Vec<String>,
        /// The languages of the project, comma separated (see `oxispec languages`). The trace
        /// scan then reads only their files. Default: every text file
        #[arg(long, value_delimiter = ',')]
        languages: Vec<String>,
    },
    /// The languages the trace scan knows: which have a grammar, which are read by their
    /// comment leaders, and which the project chose in `trace.languages`
    Languages {
        #[arg(long)]
        json: bool,
    },
    /// List, add or remove modules
    Modules {
        #[command(subcommand)]
        action: Option<ModulesAction>,
    },
    /// Create a node from a template
    New {
        /// Override the project's ID scheme for this node
        #[arg(long, global = true, value_enum)]
        id_scheme: Option<IdSchemeArg>,
        #[command(subcommand)]
        kind: NewKind,
    },
    /// Work with change sets
    Change {
        #[command(subcommand)]
        action: ChangeAction,
    },
    /// Prepare, check and publish releases of the specs
    Release {
        #[command(subcommand)]
        action: ReleaseAction,
    },
    /// Skills for AI clients: write them, check them for drift, or export them for other tools
    Agents {
        #[command(subcommand)]
        action: AgentsAction,
    },
    /// A summary of the project: what is in it and what needs attention
    Status {
        /// Read the specs as they were at a git revision
        #[arg(long)]
        at: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Add the MCP server to the config files of AI clients (claude, cursor, gemini, copilot)
    Connect {
        /// The clients to write for (default: the project's `agents`, else all four)
        clients: Vec<String>,
        /// Start the server with `--read-only`
        #[arg(long, conflicts_with = "url")]
        read_only: bool,
        /// Point the clients at a running `oxispec serve`, for example `https://specs.example.com/mcp`.
        /// The token is read from the OXISPEC_TOKEN environment variable, and is never written to a file
        #[arg(long, value_name = "URL")]
        url: Option<String>,
        /// Show what would be written, and write nothing
        #[arg(long)]
        dry_run: bool,
    },
    /// The text of some nodes and what is near them, sized to a token budget, for an agent
    Context {
        /// The nodes to start from
        #[arg(required = true)]
        ids: Vec<String>,
        /// How many link hops from them to look
        #[arg(long, default_value_t = 2)]
        depth: u32,
        /// Tokens to fill, counted as one per four characters
        #[arg(long, default_value_t = 8000)]
        budget: usize,
        /// Read the specs as they were at a git revision
        #[arg(long)]
        at: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// List the vocabulary and the claims written against it. `check` reports conflicts
    Claims {
        /// Only this variable
        #[arg(long = "var", value_name = "NAME")]
        variable: Option<String>,
        /// Read the specs as they were at a git revision
        #[arg(long)]
        at: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// The architecture model: list its elements, or draw a view of it as Mermaid
    Architecture {
        /// Read the specs as they were at a git revision
        #[arg(long, global = true)]
        at: Option<String>,
        #[arg(long, global = true)]
        json: bool,
        #[command(subcommand)]
        action: Option<ArchitectureAction>,
    },
    /// List state machines, or show one with its properties, a diagram or generated tests
    Machine {
        /// A state machine ID, such as SM-ORDER. Without it, every machine is listed
        id: Option<String>,
        /// Print a Mermaid state diagram
        #[arg(long, conflicts_with = "tests")]
        diagram: bool,
        /// Print one test for each transition: the path from the initial state to it
        #[arg(long)]
        tests: bool,
        /// With --tests, print only this many
        #[arg(long, requires = "tests")]
        limit: Option<usize>,
        /// Read the specs as they were at a git revision
        #[arg(long)]
        at: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Serve the specs to AI clients over the Model Context Protocol, on stdin and stdout
    Mcp {
        /// Do not offer the tool that writes a draft change set
        #[arg(long)]
        read_only: bool,
    },
    /// Serve the specs to many AI clients over HTTP, read-only, with one bearer token per client
    Serve {
        /// Address to listen on
        #[arg(long, default_value = "127.0.0.1:7878", value_name = "ADDRESS")]
        listen: String,
        /// Allow a non-local address because a TLS proxy in front encrypts the traffic
        #[arg(long)]
        behind_tls_proxy: bool,
        /// Where to append one JSON line per request. Default: .oxispec/server/audit.jsonl
        #[arg(long, value_name = "FILE")]
        audit_log: Option<PathBuf>,
    },
    /// An API as the requirements state it
    Api {
        #[command(subcommand)]
        action: ApiAction,
    },
    /// Test plans read against the tests: what each covers, which scenarios no test verifies or
    /// that fail, and whether what mitigates each risk is tested
    TestPlan {
        /// Only this plan
        id: Option<String>,
        /// Exit 1 when an approved plan has a gap
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        json: bool,
    },
    /// The risk register, the most serious and the oldest first
    #[command(args_conflicts_with_subcommands = true)]
    Risks {
        #[command(subcommand)]
        action: Option<RisksAction>,
        /// Only risks with this status: open, mitigating, mitigated, accepted or closed
        #[arg(long)]
        status: Option<String>,
        /// Only risks at this level: low, medium, high or critical
        #[arg(long)]
        level: Option<String>,
        /// Include risks that are not live (mitigated, accepted, closed)
        #[arg(long)]
        all: bool,
        #[arg(long)]
        json: bool,
    },
    /// The entities the specs are about, and what depends on each (needs the consistency module)
    Entities {
        /// Show this entity with everything that depends on it, as ENT-0001
        id: Option<String>,
        /// How many links away a dependency is followed
        #[arg(long, default_value_t = 2)]
        depth: u32,
        #[arg(long)]
        json: bool,
    },
    /// The tech debt backlog, the most serious and the oldest first
    #[command(args_conflicts_with_subcommands = true)]
    Debt {
        #[command(subcommand)]
        action: Option<DebtAction>,
        /// Only items with this status: open, planned, in-progress, resolved or accepted
        #[arg(long)]
        status: Option<String>,
        /// Only items with this severity: low, medium, high or critical
        #[arg(long)]
        severity: Option<String>,
        /// Include items that are resolved or accepted
        #[arg(long)]
        all: bool,
        #[arg(long)]
        json: bool,
    },
    /// Traceability and sign-offs the project's compliance rules ask for. Exits 1 when one is missing
    #[command(args_conflicts_with_subcommands = true)]
    Compliance {
        #[command(subcommand)]
        action: Option<ComplianceAction>,
        #[arg(long)]
        json: bool,
    },
    /// Documentation written from the program: the command reference and the check codes
    Docs {
        #[command(subcommand)]
        action: DocsAction,
    },
    /// Decision records: convert them between formats
    Adr {
        #[command(subcommand)]
        action: AdrAction,
    },
    /// Bring in records written for another tool
    Import {
        #[command(subcommand)]
        kind: ImportKind,
    },
    /// The format profiles a project can use: what a node of a type must contain
    Formats {
        #[command(subcommand)]
        action: FormatsAction,
    },
    /// Create, list and revoke the tokens that `serve` accepts
    Token {
        #[command(subcommand)]
        action: TokenAction,
    },
    /// Add a typed link from one node to another, for example: link REQ-AUTH-0001 decided-by ADR-0001
    Link {
        /// The node that holds the link
        from: String,
        /// satisfies, verifies, implements, decided-by, supersedes, refines, governed-by,
        /// describes, overrides, conflicts-with, depends-on, changes, includes, amends or clarifies
        kind: String,
        /// A node, or a scenario of a node such as REQ-AUTH-0001.S1
        to: String,
    },
    /// Run structural checks. Exit code 1 when there are errors.
    Check {
        #[arg(long)]
        json: bool,
        /// Treat warnings as errors
        #[arg(long)]
        strict: bool,
        /// Also print informational findings
        #[arg(long, short)]
        verbose: bool,
        /// Only report findings for spec files that changed since BASE, and the nodes that link to them
        #[arg(long)]
        changed: bool,
        /// The git ref to compare with, for --changed
        #[arg(long, default_value = "HEAD", requires = "changed")]
        base: String,
        /// Check the specs as they were at a git revision (a branch, tag, hash or date:YYYY-MM-DD)
        #[arg(long, conflicts_with = "changed")]
        at: Option<String>,
    },
    /// Check a change for CI: only the findings it introduces, with reports for pull requests
    Ci {
        /// What to compare with (the branch point of HEAD and BASE is used)
        #[arg(long, default_value = "origin/main")]
        base: String,
        /// Warnings that the change introduces fail the run too
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        json: bool,
        /// Write SARIF (for code scanning) to this file
        #[arg(long, value_name = "FILE")]
        sarif: Option<PathBuf>,
        /// Write JUnit XML to this file
        #[arg(long, value_name = "FILE")]
        junit: Option<PathBuf>,
        /// Write the pull request comment (markdown) to this file
        #[arg(long, value_name = "FILE")]
        markdown: Option<PathBuf>,
    },
    /// Build a static site: a page per node, linked by ID
    Site {
        /// The folder to write to
        #[arg(long, default_value = "site")]
        out: PathBuf,
        /// Build the specs as they were at a git revision (a branch, tag, hash or date:YYYY-MM-DD)
        #[arg(long)]
        at: Option<String>,
        /// The site's name (default: the project folder's name)
        #[arg(long)]
        title: Option<String>,
        /// Build the working tree as `latest` and a folder for every spec tag, with a version picker
        #[arg(long, conflicts_with = "at")]
        releases: bool,
    },
    /// Show where a node is implemented and tested, from `spec:` and `verifies:` comments
    Trace {
        /// A node (default: list every annotation found)
        id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// List links between specs and code that changed since they were confirmed
    Suspect {
        /// Also list links that are fine
        #[arg(long)]
        all: bool,
        /// Links that are not confirmed yet, or whose code is gone, fail the run too
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        json: bool,
    },
    /// Record that the code and the spec still agree, so their links stop being suspect
    Confirm {
        /// Nodes whose links to confirm
        ids: Vec<String>,
        /// Confirm every link
        #[arg(long, conflicts_with = "ids")]
        all: bool,
        /// Show what would be confirmed and write nothing
        #[arg(long)]
        dry_run: bool,
    },
    /// Import test results (JUnit XML or `go test -json`), so coverage can say what passes
    Results {
        #[command(subcommand)]
        action: ResultsAction,
    },
    /// Report requirements nothing implements and scenarios no test verifies
    Coverage {
        /// Only requirements in this status, since a draft is not built yet
        #[arg(long)]
        status: Option<String>,
        /// Exit with 1 when either percentage is below this
        #[arg(long, value_name = "PERCENT")]
        fail_under: Option<f64>,
        /// Count a scenario only when its tests passed in the imported results, and fail on a failing one
        #[arg(long)]
        require_pass: bool,
        /// List everything, not the first 20
        #[arg(long)]
        all: bool,
        #[arg(long)]
        json: bool,
    },
    /// Search the text of all nodes, for example: search "session timeout" --type requirement
    Search {
        /// The words to look for. All of them have to match. Quotes make a phrase
        #[arg(required = true)]
        query: Vec<String>,
        /// Only nodes of this type. Repeat for several types
        #[arg(long = "type", value_name = "TYPE")]
        ty: Vec<String>,
        /// Only nodes with this status
        #[arg(long)]
        status: Option<String>,
        /// How many results to show
        #[arg(long, short = 'n', default_value_t = 10)]
        limit: usize,
        /// Delete the stored search index and build it again
        #[arg(long)]
        rebuild: bool,
        #[arg(long)]
        json: bool,
    },
    /// Show how a node changed over time, from git history
    History {
        id: String,
        /// Look at history up to this revision, and find nodes that no longer exist
        #[arg(long)]
        at: Option<String>,
        /// Number of commits to show
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Put nodes back to how they were at a git revision, after checking nothing breaks.
    /// Nothing is committed: review the result and commit it.
    Rollback {
        /// A branch, tag, hash, or date:YYYY-MM-DD
        revision: String,
        /// The nodes to restore
        ids: Vec<String>,
        /// Restore every node that differs from that revision
        #[arg(long)]
        all: bool,
        /// Go ahead although it would introduce errors or lose uncommitted work
        #[arg(long)]
        force: bool,
        /// Show what would happen and change nothing
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Name a revision with a tag (specs-NAME), or list the tags when no name is given
    Tag {
        name: Option<String>,
        /// The revision to tag (default: HEAD)
        #[arg(long)]
        at: Option<String>,
        /// The tag's message
        #[arg(short, long)]
        message: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show how the specs differ between two revisions (default for the second: the working tree)
    Diff {
        /// A branch, tag, hash, or date:YYYY-MM-DD
        from: String,
        to: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Build or refresh the local index in .oxispec/cache
    Index {
        /// Delete the cache first, so every file is parsed again
        #[arg(long)]
        rebuild: bool,
        #[arg(long)]
        json: bool,
    },
    /// List nodes, for example: list --type requirement --domain carousels
    List {
        /// Only nodes of this type. Repeat for several types
        #[arg(long = "type", value_name = "TYPE")]
        ty: Vec<String>,
        /// Only nodes with this status
        #[arg(long)]
        status: Option<String>,
        /// Only nodes whose ID is of this domain, such as `carousels` for REQ-CAROUSELS-0001
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show a node, its links, and the nodes that link to it
    Show {
        id: String,
        /// Print the text of the node as well
        #[arg(long, conflicts_with = "at")]
        text: bool,
        #[arg(long)]
        json: bool,
        /// Show the node as it was at a git revision (a branch, tag, hash or date:YYYY-MM-DD)
        #[arg(long)]
        at: Option<String>,
    },
}

#[derive(Subcommand)]
enum DebtAction {
    /// Move an item to a status, and record today as the day it was reviewed
    Status { id: String, status: String },
}

#[derive(Subcommand)]
enum AdrAction {
    /// Rewrite decisions in another format. The front matter stays; sections that mean the
    /// same are matched, the others are kept as they are. Example: adr convert --to madr ADR-0001
    Convert {
        /// The decisions to convert. Default: every ADR in the project
        ids: Vec<String>,
        /// The format profile to convert to: nygard, madr, y-statement, or one of your own
        #[arg(long)]
        to: String,
        /// Show what would change and write nothing
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ImportKind {
    /// Records of adr-tools, MADR or log4brains. Links such as "Superceded by" and "Amended by"
    /// become `supersedes`, `amends` and `clarifies` links
    Adr {
        /// adr-tools, madr or log4brains
        #[arg(long)]
        from: String,
        /// The folder of records. Default: where the tool keeps them (doc/adr, docs/decisions,
        /// docs/adr)
        #[arg(long)]
        dir: Option<PathBuf>,
        /// The format profile to write them in. Default: the one the config chooses for ADRs
        #[arg(long)]
        format: Option<String>,
        /// Add this to every number, when the project has decisions already
        #[arg(long, default_value_t = 0)]
        offset: u32,
        /// Show what would be written and write nothing
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Decide what to make of each file of another tool, look at drafts, then apply
    Plan {
        #[command(subcommand)]
        action: PlanAction,
    },
    /// Write the nodes of the import plan. With --dry-run: show them and write nothing
    Apply {
        /// The status to give the requirements. Default: the first status of the profile
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Specs of OpenSpec, Spec Kit, Kiro or plain markdown. Each requirement becomes a node;
    /// what has no place in one (design notes, tasks, open changes) is listed.
    /// Example: import specs --from openspec --dir openspec/specs --status accepted
    Specs {
        /// The name of the tool, not a path: openspec, speckit, kiro, markdown, prd (a product
        /// requirements document) or openapi (OpenAPI or Swagger, JSON or YAML)
        #[arg(long)]
        from: String,
        /// The folder of specs (for markdown, a file or folder). Default: where the tool keeps
        /// them, such as openspec/specs
        #[arg(long)]
        dir: Option<PathBuf>,
        /// The status to give the requirements, for example `accepted` for specs that are in
        /// force. Default: the first status of the profile
        #[arg(long)]
        status: Option<String>,
        /// The requirement profile to write them in. Default: the one the config chooses
        #[arg(long)]
        format: Option<String>,
        /// sequential or random; overrides the config
        #[arg(long, value_enum)]
        id_scheme: Option<IdSchemeArg>,
        /// Show what would be written and write nothing
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum DocsAction {
    /// Write commands.md and check-codes.md. With --check, write nothing and exit 1 when they differ
    Reference {
        /// Where to write them
        #[arg(long, default_value = "docs/reference")]
        out: PathBuf,
        #[arg(long)]
        check: bool,
    },
    /// An OpenAPI skeleton made from the requirements that are titled as calls, such as
    /// `GET /orders/{id}`. It has no schemas or security. With --against, compare a file with
    /// the requirements instead and exit 1 when they differ
    Openapi {
        /// Write the skeleton to this file instead of printing it
        #[arg(long)]
        out: Option<PathBuf>,
        /// Compare this OpenAPI or Swagger file (JSON or YAML), or the description a running
        /// service serves at an http:// address, with the requirements
        #[arg(long)]
        against: Option<String>,
        #[arg(long, default_value = "API")]
        title: String,
        #[arg(long, default_value = "0.1.0")]
        version: String,
    },
    /// The product requirements document a `prd` node and the requirements that refine it make.
    /// Printed, or written with --out. It is a view of the nodes: change them, not the file
    Prd {
        id: String,
        /// Write the document to this file instead of printing it
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum PlanAction {
    /// Look at the files and propose what to make of each. Writes .oxispec/import-plan.json
    Start {
        /// openspec, speckit, kiro, markdown, prd (a product requirements document) or openapi
        /// (OpenAPI or Swagger, JSON or YAML)
        #[arg(long)]
        from: String,
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Start again, replacing a plan that exists
        #[arg(long)]
        force: bool,
        /// Plan only these kinds (comma separated: requirement, change); the other items are
        /// set to skip
        #[arg(long, value_delimiter = ',', value_name = "KINDS")]
        only: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// The items of the plan, or one item. With --draft: the file it would write
    Show {
        n: Option<usize>,
        #[arg(long)]
        draft: bool,
        #[arg(long)]
        json: bool,
    },
    /// Change what an item becomes: requirement, change, adr or skip
    Set {
        /// The item. Leave it out with --where to change several items
        n: Option<usize>,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        title: Option<String>,
        /// Change every item of this kind, for example `kind=requirement`
        #[arg(long = "where", value_name = "kind=KIND")]
        where_: Option<String>,
    },
}

#[derive(Subcommand)]
enum FormatsAction {
    /// List the profiles, built in and the project's own
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show what a profile requires and the template `new` writes
    Show {
        name: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum TokenAction {
    /// Create a token and print it once. The project keeps only a hash
    Add {
        /// A name for the client, such as `ci` or `docs-bot`
        name: String,
        /// Limit the token to these node types, for example `requirement,adr`. It then
        /// reads only those nodes, with spec_search and spec_get. Without it, it reads everything
        #[arg(long, value_delimiter = ',', value_name = "TYPES")]
        types: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// List token names and what each may read. Secrets are never shown
    List {
        #[arg(long)]
        json: bool,
    },
    /// Stop accepting a token
    Revoke { name: String },
}

#[derive(Subcommand)]
enum ModulesAction {
    /// List all modules (default)
    List,
    /// Enable modules, and the modules they need
    Add { names: Vec<String> },
    /// Disable modules. Existing files are never deleted.
    Remove { names: Vec<String> },
}

#[derive(Subcommand)]
enum ArchitectureAction {
    /// List the elements, environments and rules (the default)
    List,
    /// Draw one view as a Mermaid flowchart
    View {
        kind: architecture::ViewArg,
        /// The system, container or environment the view is of
        id: String,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ElementKind {
    System,
    Person,
    Container,
    Component,
    Environment,
}

#[derive(Subcommand)]
enum NewKind {
    /// A requirement with one scenario, for example: new requirement auth "Admin session timeout"
    Requirement {
        domain: String,
        title: String,
        /// The format profile to write it in: shall, ears, gherkin, user-story or your own.
        /// Default: the one the config chooses for requirements, else shall
        #[arg(long)]
        format: Option<String>,
    },
    /// An architecture decision record
    Adr {
        title: String,
        /// The format profile to write it in. Default: the one the config chooses for ADRs
        #[arg(long)]
        format: Option<String>,
    },
    /// A test plan: what is tested, how, where, and when it is done. Link what it covers with
    /// `covers`
    TestPlan { title: String },
    /// A risk to the application, for example: new risk "Session tokens never expire"
    /// --likelihood medium --impact high --from REQ-AUTH-0001
    Risk {
        title: String,
        /// low, medium or high
        #[arg(long, default_value = "medium")]
        likelihood: String,
        /// low, medium, high or critical
        #[arg(long, default_value = "medium")]
        impact: String,
        /// The node it arises from; becomes an `arises-from` link
        #[arg(long)]
        from: Option<String>,
    },
    /// A tech debt item, for example: new debt "Retry loop has no backoff" --from REQ-API-0003
    Debt {
        title: String,
        /// low, medium, high or critical
        #[arg(long, default_value = "medium")]
        severity: String,
        /// The node it arises from; becomes an `arises-from` link
        #[arg(long)]
        from: Option<String>,
    },
    /// A product requirements document. Requirements that `refine` it make up the document
    /// `docs prd` writes
    Prd { title: String },
    /// A change set: a proposed change to the specs, with a lifecycle
    Change { title: String },
    /// A vocabulary: the typed variables that claims in requirements talk about
    Vocabulary { title: String },
    /// An entity: a thing the specs are about, with typed attributes that claims can use
    Entity { title: String },
    /// A state machine: states, events and transitions, with properties that are checked
    Machine { title: String },
    /// An element of the architecture model, or a deployment environment
    Architecture { kind: ElementKind, title: String },
}

#[derive(Subcommand)]
enum ApiAction {
    /// Everything the project says about an operation: its requirement, PRD, decisions, debt,
    /// risks, test plans, responses and how each scenario is tested. Name it by its call
    /// (`"GET /orders/{id}"`) or by its requirement ID
    Show {
        target: String,
        #[arg(long)]
        json: bool,
    },
    /// Build or update the search index of the operations: one chunk of text for each, with
    /// its PRD, decisions, risks, debt and test plans. Kept in .oxispec/cache. Embeds with the
    /// built-in embedder (words and word pairs), or the service in `api_index.embedder`
    Index {
        /// An OpenAPI or Swagger file, or an http:// address, whose text is added to each chunk
        #[arg(long)]
        openapi: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// The operations nearest to a query, from the index
    Search {
        query: String,
        /// How many to show
        #[arg(long, default_value_t = 5)]
        top: usize,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum RisksAction {
    /// Move a risk to open, mitigating, mitigated, accepted or closed, and record the review.
    /// Accepting a risk records who accepts it
    Status {
        id: String,
        status: String,
        /// Who accepts the risk (default: git user.name). Only for `accepted`
        #[arg(long)]
        by: Option<String>,
    },
}

#[derive(Subcommand)]
enum ComplianceAction {
    /// The built-in standards, their levels and the rules each level adds
    Standards {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ChangeAction {
    /// Move a change set along draft, in-review, approved, merged, reverted
    Status {
        id: String,
        status: String,
        /// Who approves, when moving to approved (default: git user.name)
        #[arg(long)]
        by: Option<String>,
        /// Skip a step, or set a status that is not the next one
        #[arg(long)]
        force: bool,
    },
    /// List change sets
    List {
        /// Only change sets with this status
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ResultsAction {
    /// Read results from files and keep them in the cache
    Import {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Forget earlier imports first
        #[arg(long)]
        replace: bool,
    },
    /// Show the imported results
    Show {
        #[arg(long)]
        json: bool,
    },
    /// Forget the imported results
    Clear,
}

#[derive(Subcommand)]
enum AgentsAction {
    /// List the clients and the skills, and which ones this project gets
    List {
        #[arg(long)]
        json: bool,
    },
    /// Write the skills into each client's folder. A file edited by hand is left alone
    Update {
        /// Only these clients (default: the project's, set by `init --agents`)
        #[arg(long = "client", value_delimiter = ',')]
        clients: Vec<String>,
        /// Replace files that were edited by hand
        #[arg(long)]
        force: bool,
        /// Show what would be written and write nothing
        #[arg(long)]
        dry_run: bool,
    },
    /// Fail when a skill file is missing, out of date or edited by hand
    Check {
        #[arg(long = "client", value_delimiter = ',')]
        clients: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// Write the skills in the layout another tool reads
    Export {
        /// skills-repo (`npx skills add`) or ruler
        #[arg(long, value_enum)]
        to: agents::ExportTarget,
        /// The folder to write into
        #[arg(long, default_value = ".")]
        out: PathBuf,
        /// Every skill and no project notes, without needing a project
        #[arg(long)]
        all: bool,
    },
}

#[derive(Subcommand)]
enum ReleaseAction {
    /// Work out the version and notes since the last release, and write the manifest
    Prepare {
        /// The version, for example 0.2.0 (default: the suggested one)
        version: Option<String>,
        /// Write the notes again for a version that already has a manifest
        #[arg(long)]
        force: bool,
        /// Show the manifest and write nothing
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Run the gates for a release. Exits with 1 when it is not ready
    Check {
        version: String,
        #[arg(long)]
        json: bool,
    },
    /// Run the gates on the last commit and tag it specs-vVERSION
    Publish {
        version: String,
        #[arg(long)]
        json: bool,
    },
    /// List releases and whether they are published
    List {
        #[arg(long)]
        json: bool,
    },
}

// spec: REQ-PROJECT-0003
// spec: REQ-ROBUSTNESS-0001
fn main() -> ExitCode {
    // `oxispec ... | head` closes our output early. `outln!` panics on that, and the
    // panic message is noise: stop quietly, as other command line tools do.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let text = info.to_string();
        // Unix says "Broken pipe" (32). Windows says "The pipe is being closed" (232)
        // or "The pipe has been ended" (109).
        let closed = ["Broken pipe", "os error 32", "os error 232", "os error 109"];
        if closed.iter().any(|c| text.contains(c)) {
            std::process::exit(0);
        }
        default_hook(info);
    }));
    // `run` is a large function and the MCP servers call it again for each tool,
    // and parsing the arguments builds every subcommand, so the main thread's stack
    // (1 MiB on Windows) is not enough. A thread of our own has room.
    let program = std::thread::Builder::new()
        .name("oxispec".into())
        .stack_size(STACK_BYTES)
        .spawn(|| match run(Cli::parse()) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("error: {e:#}");
                ExitCode::from(2)
            }
        })
        .expect("a thread can be started");
    // A panic has already been reported by the hook above.
    program.join().unwrap_or(ExitCode::from(101))
}

/// The stack of the thread that runs a command.
const STACK_BYTES: usize = 32 * 1024 * 1024;

fn run(cli: Cli) -> Result<ExitCode> {
    let cache = !cli.no_cache;
    match cli.command {
        Command::Init {
            preset,
            with,
            without,
            id_scheme,
            agents,
            languages,
        } => {
            let dir = cli.root.unwrap_or_else(|| PathBuf::from("."));
            init(
                &dir,
                &preset,
                &with,
                &without,
                id_scheme.into(),
                &agents,
                &languages,
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Languages { json } => {
            let root = project_root(cli.root.as_deref())?;
            languages_command(&root, json)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Claims { variable, at, json } => {
            let root = project_root(cli.root.as_deref())?;
            claims::claims_command(&root, variable.as_deref(), at.as_deref(), json, cache)
        }
        Command::Architecture { at, json, action } => {
            let root = project_root(cli.root.as_deref())?;
            match action.unwrap_or(ArchitectureAction::List) {
                ArchitectureAction::List => architecture::list(&root, at.as_deref(), json, cache),
                ArchitectureAction::View { kind, id } => {
                    architecture::view(&root, kind, &id, at.as_deref(), json, cache)
                }
            }
        }
        Command::Machine {
            id,
            diagram,
            tests,
            limit,
            at,
            json,
        } => {
            let root = project_root(cli.root.as_deref())?;
            let output = machines::Output {
                diagram,
                tests,
                limit,
                json,
            };
            machines::machine_command(&root, id.as_deref(), output, at.as_deref(), cache)
        }
        Command::Context {
            ids,
            depth,
            budget,
            at,
            json,
        } => {
            let root = project_root(cli.root.as_deref())?;
            context::context_command(&root, &ids, depth, budget, at.as_deref(), json, cache)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Mcp { read_only } => {
            let root = project_root(cli.root.as_deref())?;
            let cfg = config::load(&root)?;
            require_module(&cfg, "mcp")?;
            mcp::serve(&root, read_only)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Serve {
            listen,
            behind_tls_proxy,
            audit_log,
        } => {
            let root = project_root(cli.root.as_deref())?;
            let cfg = config::load(&root)?;
            require_module(&cfg, "mcp")?;
            server::serve(
                &root,
                &server::ServeOptions {
                    listen,
                    behind_tls_proxy,
                    audit_log,
                },
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Debt {
            action,
            status,
            severity,
            all,
            json,
        } => {
            let root = project_root(cli.root.as_deref())?;
            match action {
                Some(DebtAction::Status { id, status }) => {
                    let _lock = lock_project(&root)?;
                    debt::set_status(&root, &id, &status, cache)?
                }
                None => debt::list(
                    &root,
                    status.as_deref(),
                    severity.as_deref(),
                    all,
                    json,
                    cache,
                )?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Api { action } => {
            let root = project_root(cli.root.as_deref())?;
            match action {
                ApiAction::Show { target, json } => api::show(&root, &target, json, cache)?,
                ApiAction::Index { openapi, json } => {
                    let _lock = lock_project(&root)?;
                    api::index(&root, openapi.as_deref(), json, cache)?
                }
                ApiAction::Search { query, top, json } => api::search(&root, &query, top, json)?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::TestPlan { id, strict, json } => {
            let root = project_root(cli.root.as_deref())?;
            testplan::run(&root, id.as_deref(), strict, json, cache)
        }
        Command::Entities { id, depth, json } => {
            let root = project_root(cli.root.as_deref())?;
            entities::run(&root, id.as_deref(), depth, json, cache)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Risks {
            action,
            status,
            level,
            all,
            json,
        } => {
            let root = project_root(cli.root.as_deref())?;
            match action {
                Some(RisksAction::Status { id, status, by }) => {
                    let _lock = lock_project(&root)?;
                    risks::set_status(&root, &id, &status, by.as_deref(), cache)?
                }
                None => risks::list(&root, status.as_deref(), level.as_deref(), all, json, cache)?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Compliance { action, json } => match action {
            Some(ComplianceAction::Standards { json }) => {
                compliance::standards(json)?;
                Ok(ExitCode::SUCCESS)
            }
            None => {
                let root = project_root(cli.root.as_deref())?;
                compliance::report(&root, json, cache)
            }
        },
        Command::Docs { action } => {
            match action {
                DocsAction::Reference { out, check } => {
                    reference::run(&<Cli as clap::CommandFactory>::command(), &out, check)?
                }
                DocsAction::Openapi {
                    out,
                    against,
                    title,
                    version,
                } => {
                    let root = project_root(cli.root.as_deref())?;
                    let cfg = config::load(&root)?;
                    require_module(&cfg, "specs")?;
                    let graph = load_graph(&root, cache);
                    let ops = oxispec_core::api::operations(&graph, &root)?;
                    if let Some(file) = against {
                        let doc = if file.starts_with("http://") || file.starts_with("https://") {
                            oxispec_core::api::parse(&fetch::get(&file)?, &file)?
                        } else {
                            oxispec_core::api::load(Path::new(&file))?
                        };
                        let d = oxispec_core::api::drift(&ops, &doc);
                        for c in &d.only_in_requirements {
                            outln!("only in the requirements: {c}");
                        }
                        for c in &d.only_in_description {
                            outln!("only in {file}: {c}");
                        }
                        for (c, missing, extra) in &d.codes {
                            outln!(
                                "{c}: requirements state {} that the file lacks; the file has {} that they do not",
                                if missing.is_empty() {
                                    "nothing".into()
                                } else {
                                    missing.join(", ")
                                },
                                if extra.is_empty() {
                                    "nothing".into()
                                } else {
                                    extra.join(", ")
                                }
                            );
                        }
                        if d.is_empty() {
                            outln!(
                                "{} and the requirements agree on {} operation(s)",
                                file,
                                ops.len()
                            );
                            return Ok(ExitCode::SUCCESS);
                        }
                        return Ok(ExitCode::FAILURE);
                    }
                    let text = oxispec_core::api::render(&ops, &title, &version);
                    match out {
                        Some(path) => {
                            std::fs::write(&path, &text)
                                .with_context(|| format!("cannot write {}", path.display()))?;
                            outln!("wrote {} ({} operation(s))", path.display(), ops.len());
                        }
                        None => outln!("{text}"),
                    }
                }
                DocsAction::Prd { id, out } => {
                    let root = project_root(cli.root.as_deref())?;
                    let cfg = config::load(&root)?;
                    require_module(&cfg, "specs")?;
                    let graph = load_graph(&root, cache);
                    let text = oxispec_core::prd::render(&graph, &root, &id)?;
                    match out {
                        Some(path) => {
                            std::fs::write(&path, &text)
                                .with_context(|| format!("cannot write {}", path.display()))?;
                            outln!("wrote {}", path.display());
                        }
                        None => outln!("{text}"),
                    }
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Adr { action } => {
            let root = project_root(cli.root.as_deref())?;
            match action {
                AdrAction::Convert {
                    ids,
                    to,
                    dry_run,
                    json,
                } => {
                    let _lock = lock_project(&root)?;
                    adr::convert(&root, &ids, &to, dry_run, json, cache)?
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Import { kind } => {
            let root = project_root(cli.root.as_deref())?;
            match kind {
                ImportKind::Adr {
                    from,
                    dir,
                    format,
                    offset,
                    dry_run,
                    json,
                } => {
                    let _lock = lock_project(&root)?;
                    adr::import(
                        &root,
                        &adr::ImportOptions {
                            from: &from,
                            dir: dir.as_deref(),
                            format: format.as_deref(),
                            offset,
                            dry_run,
                            json,
                        },
                        cache,
                    )?
                }
                ImportKind::Plan { action } => match action {
                    PlanAction::Start {
                        from,
                        dir,
                        force,
                        only,
                        json,
                    } => {
                        let _lock = lock_project(&root)?;
                        importer::plan_start(&root, &from, dir.as_deref(), force, &only, json)?
                    }
                    PlanAction::Show { n, draft, json } => {
                        importer::plan_show(&root, n, draft, json, cache)?
                    }
                    PlanAction::Set {
                        n,
                        kind,
                        title,
                        where_,
                    } => {
                        let _lock = lock_project(&root)?;
                        importer::plan_set(
                            &root,
                            n,
                            kind.as_deref(),
                            title.as_deref(),
                            where_.as_deref(),
                        )?
                    }
                },
                ImportKind::Apply {
                    status,
                    dry_run,
                    json,
                } => {
                    let _lock = lock_project(&root)?;
                    importer::apply(&root, status.as_deref(), dry_run, json, cache)?
                }
                ImportKind::Specs {
                    from,
                    dir,
                    status,
                    format,
                    id_scheme,
                    dry_run,
                    json,
                } => {
                    let _lock = lock_project(&root)?;
                    importer::import_specs(
                        &root,
                        &importer::Options {
                            from: &from,
                            dir: dir.as_deref(),
                            format: format.as_deref(),
                            status: status.as_deref(),
                            scheme: id_scheme.map(IdScheme::from),
                            dry_run,
                            json,
                        },
                        cache,
                    )?
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Formats { action } => {
            let root = project_root(cli.root.as_deref())?;
            match action {
                FormatsAction::List { json } => formats::list(&root, json)?,
                FormatsAction::Show { name, json } => formats::show(&root, &name, json)?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Token { action } => {
            let root = project_root(cli.root.as_deref())?;
            match action {
                TokenAction::Add { name, types, json } => {
                    server::token_add(&root, &name, types, json)?
                }
                TokenAction::List { json } => server::token_list(&root, json)?,
                TokenAction::Revoke { name } => server::token_revoke(&root, &name)?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Status { at, json } => {
            let root = project_root(cli.root.as_deref())?;
            status::status_command(&root, at.as_deref(), json, cache)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Connect {
            clients,
            read_only,
            url,
            dry_run,
        } => {
            let root = project_root(cli.root.as_deref())?;
            let transport = match &url {
                Some(url) => oxispec_core::connect::Transport::Http { url },
                None => oxispec_core::connect::Transport::Stdio { read_only },
            };
            connect::connect_command(&root, &clients, transport, dry_run)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Agents { action } => match action {
            AgentsAction::Export { to, out, all } => {
                let root = project_root(cli.root.as_deref()).ok();
                agents::export(root.as_deref(), to, &out, all)?;
                Ok(ExitCode::SUCCESS)
            }
            AgentsAction::List { json } => {
                let root = project_root(cli.root.as_deref())?;
                agents::list(&root, json)?;
                Ok(ExitCode::SUCCESS)
            }
            AgentsAction::Update {
                clients,
                force,
                dry_run,
            } => {
                let root = project_root(cli.root.as_deref())?;
                agents::update(&root, &clients, force, dry_run)
            }
            AgentsAction::Check { clients, json } => {
                let root = project_root(cli.root.as_deref())?;
                agents::check(&root, &clients, json)
            }
        },
        Command::Modules { action } => {
            modules_command(cli.root.as_deref(), action.unwrap_or(ModulesAction::List))?;
            Ok(ExitCode::SUCCESS)
        }
        Command::New { id_scheme, kind } => {
            let root = project_root(cli.root.as_deref())?;
            let cfg = config::load(&root)?;
            let scheme = id_scheme.map_or(cfg.id_scheme, IdScheme::from);
            // Allocating an ID and writing its file must not interleave with another
            // command that writes in the same project, or both would pick the same number.
            let _lock = lock_project(&root)?;
            let created = match kind {
                NewKind::Requirement {
                    domain,
                    title,
                    format,
                } => {
                    require_module(&cfg, "specs")?;
                    new_requirement(
                        &root,
                        &cfg,
                        &domain,
                        &title,
                        format.as_deref(),
                        scheme,
                        cache,
                    )?
                }
                NewKind::Adr { title, format } => {
                    require_module(&cfg, "adrs")?;
                    new_adr(&root, &cfg, &title, format.as_deref(), scheme, cache)?
                }
                NewKind::Debt {
                    title,
                    severity,
                    from,
                } => {
                    require_module(&cfg, "debt")?;
                    new_debt(&root, &title, &severity, from.as_deref(), scheme, cache)?
                }
                NewKind::Prd { title } => {
                    require_module(&cfg, "specs")?;
                    new_prd(&root, &title, scheme, cache)?
                }
                NewKind::TestPlan { title } => {
                    require_module(&cfg, "tests")?;
                    new_test_plan(&root, &title, scheme, cache)?
                }
                NewKind::Risk {
                    title,
                    likelihood,
                    impact,
                    from,
                } => {
                    require_module(&cfg, "risks")?;
                    new_risk(
                        &root,
                        &title,
                        &likelihood,
                        &impact,
                        from.as_deref(),
                        scheme,
                        cache,
                    )?
                }
                NewKind::Change { title } => {
                    require_module(&cfg, "specs")?;
                    new_change(&root, &title, scheme, cache)?
                }
                NewKind::Entity { title } => {
                    require_module(&cfg, "consistency")?;
                    new_entity(&root, &title, scheme, cache)?
                }
                NewKind::Vocabulary { title } => {
                    require_module(&cfg, "consistency")?;
                    new_vocabulary(&root, &title, scheme, cache)?
                }
                NewKind::Architecture { kind, title } => {
                    require_module(&cfg, "architecture")?;
                    new_element(&root, kind, &title, scheme, cache)?
                }
                NewKind::Machine { title } => {
                    require_module(&cfg, "statemachines")?;
                    new_machine(&root, &title, scheme, cache)?
                }
            };
            outln!("created {} ({})", created.0, created.1.display());
            Ok(ExitCode::SUCCESS)
        }
        Command::Change { action } => {
            let root = project_root(cli.root.as_deref())?;
            let _lock = lock_project(&root)?;
            match action {
                ChangeAction::Status {
                    id,
                    status,
                    by,
                    force,
                } => changes::change_status(&root, &id, &status, by.as_deref(), force, cache)?,
                ChangeAction::List { status, json } => {
                    changes::change_list(&root, status.as_deref(), json, cache)?
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Ci {
            base,
            strict,
            json,
            sarif,
            junit,
            markdown,
        } => {
            let root = project_root(cli.root.as_deref())?;
            let options = ci::CiOptions {
                base,
                strict,
                json,
                sarif,
                junit,
                markdown,
                cache,
            };
            ci::ci_command(&root, &options)
        }
        Command::Search {
            query,
            ty,
            status,
            limit,
            rebuild,
            json,
        } => {
            let root = project_root(cli.root.as_deref())?;
            let options = search::SearchOptions {
                query: query.join(" "),
                ty,
                status,
                limit,
                rebuild,
                json,
                cache,
            };
            search::search_command(&root, &options)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Site {
            out,
            at,
            title,
            releases,
        } => {
            let root = project_root(cli.root.as_deref())?;
            site::site_command(
                &root,
                &site::SiteArgs {
                    out,
                    at,
                    title,
                    releases,
                    cache,
                },
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Trace { id, json } => {
            let root = project_root(cli.root.as_deref())?;
            trace::trace_command(&root, id.as_deref(), json, cache)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Suspect { all, strict, json } => {
            let root = project_root(cli.root.as_deref())?;
            trace::suspect_command(&root, all, strict, json, cache)
        }
        Command::Confirm { ids, all, dry_run } => {
            let root = project_root(cli.root.as_deref())?;
            let _lock = lock_project(&root)?;
            trace::confirm_command(&root, &ids, all, dry_run, cache)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Results { action } => {
            let root = project_root(cli.root.as_deref())?;
            let _lock = lock_project(&root)?;
            match action {
                ResultsAction::Import { files, replace } => {
                    trace::results_import(&root, &files, replace)?
                }
                ResultsAction::Show { json } => trace::results_show(&root, json)?,
                ResultsAction::Clear => trace::results_clear(&root)?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Coverage {
            status,
            fail_under,
            require_pass,
            all,
            json,
        } => {
            let root = project_root(cli.root.as_deref())?;
            let options = trace::CoverageOptions {
                status,
                fail_under,
                require_pass,
                all,
                json,
                cache,
            };
            trace::coverage_command(&root, &options)
        }
        Command::Release { action } => {
            let root = project_root(cli.root.as_deref())?;
            let _lock = lock_project(&root)?;
            let ready = match action {
                ReleaseAction::Prepare {
                    version,
                    force,
                    dry_run,
                    json,
                } => {
                    releases::prepare(&root, version.as_deref(), force, dry_run, json, cache)?;
                    true
                }
                ReleaseAction::Check { version, json } => {
                    releases::check(&root, &version, json, cache)?
                }
                ReleaseAction::Publish { version, json } => {
                    releases::publish(&root, &version, json)?
                }
                ReleaseAction::List { json } => {
                    releases::list(&root, json, cache)?;
                    true
                }
            };
            Ok(if ready {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        Command::Link { from, kind, to } => {
            let root = project_root(cli.root.as_deref())?;
            let _lock = lock_project(&root)?;
            config::load(&root)?;
            link_command(&root, &from, &kind, &to, cache)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Check {
            json,
            strict,
            verbose,
            changed,
            base,
            at,
        } => {
            let options = CheckOptions {
                json,
                strict,
                verbose,
                changed_since: changed.then_some(base),
                at,
            };
            check_command(&project_root(cli.root.as_deref())?, &options, cache)
        }
        Command::Index { rebuild, json } => {
            if !cache {
                bail!("`index` builds the local index, so it cannot be used with --no-cache");
            }
            index_command(&project_root(cli.root.as_deref())?, rebuild, json)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Show { id, text, json, at } => {
            show(
                &project_root(cli.root.as_deref())?,
                &id,
                json,
                cache,
                at.as_deref(),
                text,
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Command::List {
            ty,
            status,
            domain,
            json,
        } => {
            list_nodes(
                &project_root(cli.root.as_deref())?,
                &ty,
                status.as_deref(),
                domain.as_deref(),
                json,
                cache,
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Command::History {
            id,
            at,
            limit,
            json,
        } => {
            let root = project_root(cli.root.as_deref())?;
            history::history_command(&root, &id, at.as_deref(), limit, json, cache)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Rollback {
            revision,
            ids,
            all,
            force,
            dry_run,
            json,
        } => {
            let root = project_root(cli.root.as_deref())?;
            let _lock = lock_project(&root)?;
            let options = rollback::Options {
                revision: &revision,
                ids,
                all,
                force,
                dry_run,
                json,
                cache,
            };
            rollback::rollback_command(&root, &options)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Tag {
            name,
            at,
            message,
            json,
        } => {
            let root = project_root(cli.root.as_deref())?;
            history::tag_command(
                &root,
                name.as_deref(),
                at.as_deref(),
                message.as_deref(),
                json,
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Diff { from, to, json } => {
            let root = project_root(cli.root.as_deref())?;
            history::diff_command(&root, &from, to.as_deref(), json, cache)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn project_root(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(dir) = explicit {
        if dir.join(config::CONFIG_PATH).is_file() {
            return Ok(dir.to_path_buf());
        }
        bail!(
            "{} is not an OxiSpec project; run `oxispec init` there",
            dir.display()
        );
    }
    let cwd = std::env::current_dir()?;
    config::find_root(&cwd)
        .context("not an OxiSpec project here or in any parent folder; run `oxispec init`")
}

fn require_module(cfg: &Config, module: &str) -> Result<()> {
    if !cfg.has(module) {
        bail!("module `{module}` is not enabled; run `oxispec modules add {module}`");
    }
    Ok(())
}

/// Folders created for modules that keep files on disk.
fn ensure_module_dirs(root: &Path, cfg: &Config) -> Result<()> {
    if cfg.has("specs") {
        fs::create_dir_all(root.join("specs/requirements"))?;
        fs::create_dir_all(root.join("specs/changes"))?;
    }
    if cfg.has("adrs") {
        fs::create_dir_all(root.join("specs/adr"))?;
    }
    if cfg.has("releases") {
        fs::create_dir_all(root.join("specs/releases"))?;
    }
    if cfg.has("consistency") {
        fs::create_dir_all(root.join("specs/vocabulary"))?;
    }
    if cfg.has("architecture") {
        fs::create_dir_all(root.join("architecture/model"))?;
    }
    if cfg.has("statemachines") {
        fs::create_dir_all(root.join("specs/machines"))?;
    }
    if cfg.has("debt") {
        fs::create_dir_all(root.join("specs/debt"))?;
    }
    if cfg.has("tests") {
        fs::create_dir_all(root.join("specs/test-plans"))?;
    }
    if cfg.has("risks") {
        fs::create_dir_all(root.join("specs/risks"))?;
    }
    Ok(())
}

// spec: REQ-PROJECT-0001
fn init(
    dir: &Path,
    preset: &str,
    with: &[String],
    without: &[String],
    id_scheme: IdScheme,
    agent_clients: &[String],
    languages: &[String],
) -> Result<()> {
    if dir.join(config::CONFIG_PATH).exists() {
        bail!("{} is already initialized", dir.display());
    }
    let base = modules::preset(preset).with_context(|| {
        format!(
            "unknown preset `{preset}` (known: {})",
            modules::PRESETS.join(", ")
        )
    })?;
    let mut wanted: Vec<String> = base.iter().map(|s| s.to_string()).collect();
    wanted.extend(with.iter().cloned());
    for name in without {
        if name == "core" {
            bail!("`core` cannot be left out");
        }
        wanted.retain(|m| m != name);
    }
    let resolved = modules::resolve(&wanted).map_err(anyhow::Error::msg)?;
    for name in without {
        if let Some(needed_by) = resolved
            .iter()
            .find(|m| modules::info(m).is_some_and(|i| i.needs.contains(&name.as_str())))
        {
            bail!("cannot leave out `{name}`: module `{needed_by}` needs it");
        }
    }
    let mut cfg = Config::new(resolved, id_scheme);
    cfg.trace = oxispec_core::languages::config_for(languages).map_err(anyhow::Error::msg)?;
    if cfg.has("agents") {
        cfg.agents = if agent_clients.is_empty() {
            oxispec_core::agents::DEFAULT_CLIENTS
                .iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            agents::parse_clients(agent_clients)?
        };
    } else if !agent_clients.is_empty() {
        bail!("--agents needs the `agents` module; add it with `--with agents`");
    }
    fs::create_dir_all(dir)?;
    config::save(dir, &cfg)?;
    fs::write(dir.join(".oxispec/.gitignore"), "cache/\n")?;
    ensure_module_dirs(dir, &cfg)?;
    let scheme = match cfg.id_scheme {
        IdScheme::Sequential => "sequential",
        IdScheme::Random => "random",
    };
    outln!(
        "initialized {} with modules: {} (ids: {scheme})",
        dir.display(),
        cfg.modules.join(", ")
    );
    warn_planned(&cfg.modules);
    agents::write_after_init(dir, &cfg)?;
    Ok(())
}

fn languages_command(root: &Path, json: bool) -> Result<()> {
    use serde_json::json as obj;
    let cfg = config::load(root)?;
    let langs = oxispec_core::languages::Languages::from_config(&cfg.trace)
        .map_err(|e| anyhow::anyhow!("invalid trace settings: {e}"))?;
    let chosen = langs.chosen();
    let scanned = |name: &str| chosen.is_none_or(|c| c.iter().any(|n| n == name));
    if json {
        let rows: Vec<_> = langs
            .all()
            .iter()
            .map(|l| {
                obj!({
                    "name": l.name,
                    "extensions": l.extensions,
                    "read_with": if l.parsed { "grammar" } else { "comment leaders" },
                    "leaders": l.leaders,
                    "scanned": scanned(&l.name),
                })
            })
            .collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&obj!({
                "schema_version": oxispec_core::OUTPUT_SCHEMA_VERSION,
                "limited": langs.is_limited(),
                "languages": rows,
            }))?
        );
        return Ok(());
    }
    outln!(
        "{:<12} {:<9} {:<16} {:<18} EXTENSIONS",
        "LANGUAGE",
        "SCANNED",
        "READ WITH",
        "LEADERS"
    );
    for l in langs.all() {
        outln!(
            "{:<12} {:<9} {:<16} {:<18} {}",
            l.name,
            if scanned(&l.name) { "yes" } else { "no" },
            if l.parsed {
                "grammar"
            } else {
                "comment leaders"
            },
            l.leaders.join(" "),
            l.extensions.join(", ")
        );
    }
    outln!(
        "{}",
        if langs.is_limited() {
            "only the languages marked yes are scanned (trace.languages)"
        } else {
            "every text file is scanned; limit it with `trace.languages` in .oxispec/config.yaml or `init --languages`"
        }
    );
    Ok(())
}

fn warn_planned(names: &[String]) {
    let planned: Vec<&str> = names
        .iter()
        .filter(|n| modules::info(n).is_some_and(|i| !i.implemented))
        .map(String::as_str)
        .collect();
    if !planned.is_empty() {
        outln!(
            "note: not implemented yet, recorded only: {}",
            planned.join(", ")
        );
    }
}

// spec: REQ-PROJECT-0002
fn modules_command(explicit: Option<&Path>, action: ModulesAction) -> Result<()> {
    match action {
        ModulesAction::List => {
            let enabled = project_root(explicit)
                .ok()
                .and_then(|root| config::load(&root).ok())
                .map(|c| c.modules);
            for m in modules::MODULES {
                let state = match &enabled {
                    Some(list) if list.iter().any(|e| e == m.name) => "enabled",
                    Some(_) => "-",
                    None => "",
                };
                let status = if m.implemented { "" } else { " (planned)" };
                outln!("{:<14} {:<8} {}{}", m.name, state, m.description, status);
            }
            Ok(())
        }
        ModulesAction::Add { names } => {
            let root = project_root(explicit)?;
            let mut cfg = config::load(&root)?;
            let before = cfg.modules.clone();
            let mut all = before.clone();
            all.extend(names);
            cfg.modules = modules::resolve(&all).map_err(anyhow::Error::msg)?;
            let added: Vec<String> = cfg
                .modules
                .iter()
                .filter(|m| !before.contains(m))
                .cloned()
                .collect();
            config::save(&root, &cfg)?;
            ensure_module_dirs(&root, &cfg)?;
            if added.is_empty() {
                outln!("nothing to add");
            } else {
                outln!("enabled: {}", added.join(", "));
                warn_planned(&added);
                if added.iter().any(|m| m == "agents") {
                    outln!("run `oxispec agents update` to write the skills");
                }
            }
            Ok(())
        }
        ModulesAction::Remove { names } => {
            let root = project_root(explicit)?;
            let mut cfg = config::load(&root)?;
            for name in &names {
                if name == "core" {
                    bail!("`core` cannot be removed");
                }
                if modules::info(name).is_none() {
                    bail!("unknown module `{name}`");
                }
                let remaining: Vec<String> = cfg
                    .modules
                    .iter()
                    .filter(|m| !names.contains(m))
                    .cloned()
                    .collect();
                let blockers = modules::dependents_of(name, &remaining);
                if !blockers.is_empty() {
                    bail!("cannot remove `{name}`: needed by {}", blockers.join(", "));
                }
            }
            cfg.modules.retain(|m| !names.contains(m));
            config::save(&root, &cfg)?;
            outln!("disabled: {} (files are kept)", names.join(", "));
            Ok(())
        }
    }
}

// spec: REQ-LINK-0001
// spec: REQ-LINK-0002
fn link_command(root: &Path, from: &str, kind: &str, to: &str, cache: bool) -> Result<()> {
    let graph = load_graph(root, cache);
    let parsed = id::parse(from).with_context(|| format!("{from:?} is not a valid id"))?;
    if parsed.namespace.is_some() || parsed.child.is_some() {
        bail!("a link starts at a node of this project, not at {from}");
    }
    let node = graph
        .get(parsed.base)
        .with_context(|| format!("no node with id {from}"))?;
    if !KNOWN_LINK_KINDS.contains(&kind) {
        bail!(
            "unknown link kind `{kind}` (known: {})",
            KNOWN_LINK_KINDS.join(", ")
        );
    }
    match resolve_link(&graph, to) {
        LinkResolution::Ok | LinkResolution::External => {}
        LinkResolution::Invalid => bail!("{to:?} is not a valid id"),
        LinkResolution::MissingNode => bail!("{to} does not exist"),
        LinkResolution::MissingScenario(anchor) => bail!(
            "{} has no scenario with anchor {{#{anchor}}}; add it to the scenario heading first",
            id::base_of(to)
        ),
    }
    if id::base_of(to) == node.id {
        bail!("a node cannot link to itself");
    }

    let path = root.join(&node.path);
    let text =
        fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
    match add_link(&text, kind, to)
        .with_context(|| format!("cannot edit {}", node.path.display()))?
    {
        LinkEdit::Added(updated) => {
            fs::write(&path, updated)
                .with_context(|| format!("cannot write {}", path.display()))?;
            outln!("linked {} -[{kind}]-> {to}", node.id);
        }
        LinkEdit::AlreadyLinked => outln!("{} already has {kind} -> {to}", node.id),
    }
    Ok(())
}

struct CheckOptions {
    json: bool,
    strict: bool,
    verbose: bool,
    /// `Some(base)` limits the report to files changed since that git ref.
    changed_since: Option<String>,
    /// Check the specs as they were at this git revision.
    at: Option<String>,
}

/// `error[E002]: message`, the first line of a finding.
pub(crate) fn finding_line(f: &Finding) -> String {
    let level = match f.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    };
    format!("{level}[{}]: {}", f.code, f.message)
}

fn revision_json(revision: &git::Revision) -> serde_json::Value {
    serde_json::json!({"commit": revision.oid, "short": revision.short, "subject": revision.subject})
}

fn load_graph(root: &Path, cache: bool) -> Graph {
    index::load(root, cache).0
}

// spec: REQ-INDEX-0001
fn index_command(root: &Path, rebuild: bool, json: bool) -> Result<()> {
    config::load(root)?;
    if rebuild {
        index::clear(root).context("cannot delete the old cache")?;
    }
    let (graph, stats) = index::load(root, true);
    if json {
        let out = serde_json::json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "nodes": graph.nodes.len(),
            "files": stats.files,
            "read": stats.read,
            "parsed": stats.parsed,
            "reused": stats.reused,
            "removed": stats.removed,
            "cache_written": stats.cache_written,
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        outln!(
            "indexed {} markdown files ({} nodes): {} parsed, {} reused, {} removed",
            stats.files,
            graph.nodes.len(),
            stats.parsed,
            stats.reused,
            stats.removed
        );
        if stats.parsed > 0 && !stats.cache_written {
            outln!("note: the cache could not be written, so the next run parses everything again");
        }
    }
    Ok(())
}

// spec: REQ-CHECKING-0001
// spec: REQ-CHECKING-0002
fn check_command(root: &Path, options: &CheckOptions, cache: bool) -> Result<ExitCode> {
    config::load(root)?;
    let revision = options
        .at
        .as_deref()
        .map(|spec| git::resolve_rev(root, spec))
        .transpose()?;
    let graph = match &revision {
        Some(rev) => git::graph_at(root, rev)?,
        None => load_graph(root, cache),
    };
    let mut findings = check(&graph);
    let total = graph.nodes.len();
    let mut checked = total;
    let mut scope = None;
    if let Some(base) = &options.changed_since {
        let changes = git::changes(root, base)?;
        let touched = impact(&graph, &changes.paths, &changes.old_ids);
        findings.retain(|f| in_scope(f, &touched));
        checked = touched.nodes;
        scope = Some((base.as_str(), changes.paths.len()));
    }
    let count = |s: Severity| findings.iter().filter(|f| f.severity == s).count();
    let (errors, warnings) = (count(Severity::Error), count(Severity::Warning));
    if options.json {
        let out = serde_json::json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "at": revision.as_ref().map(revision_json),
            "nodes": total,
            "checked_nodes": checked,
            "scope": scope.map(|(base, files)| serde_json::json!({"base": base, "changed_files": files})),
            "errors": errors,
            "warnings": warnings,
            "findings": findings,
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        if let Some(rev) = &revision {
            outln!("checking {rev}");
        }
        for f in &findings {
            if f.severity == Severity::Info && !options.verbose {
                continue;
            }
            outln!("{}", finding_line(f));
            if let Some(path) = &f.path {
                match f.line {
                    Some(line) => outln!("  --> {}:{line}", path.display()),
                    None => outln!("  --> {}", path.display()),
                }
            }
        }
        match scope {
            Some((base, files)) => outln!(
                "{checked} of {total} nodes checked ({files} changed files since {base}, and the nodes that link to them): {errors} errors, {warnings} warnings"
            ),
            None => outln!("{total} nodes checked: {errors} errors, {warnings} warnings"),
        }
    }
    let failed = errors > 0 || (options.strict && warnings > 0);
    Ok(if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

// spec: REQ-SEARCH-0003
fn list_nodes(
    root: &Path,
    types: &[String],
    status: Option<&str>,
    domain: Option<&str>,
    json: bool,
    cache: bool,
) -> Result<()> {
    config::load(root)?;
    let graph = load_graph(root, cache);
    let domain = domain.map(|d| d.to_ascii_uppercase().replace(['-', '_', ' '], ""));
    let rows: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| types.is_empty() || types.iter().any(|t| t == &n.ty))
        .filter(|n| status.is_none_or(|s| n.status.as_deref() == Some(s)))
        .filter(|n| {
            domain.as_ref().is_none_or(|d| {
                // REQ-CAROUSELS-0001: the part between the first dash and the number.
                let mut parts = n.id.split('-');
                parts.next();
                let tail: Vec<&str> = parts.collect();
                tail.len() >= 2 && tail[..tail.len() - 1].join("") == *d
            })
        })
        .collect();
    let title_of = |n: &oxispec_core::node::Node| {
        n.title
            .clone()
            .or_else(|| n.requirements.first().map(|r| r.name.clone()))
            .unwrap_or_default()
    };
    if json {
        let items: Vec<_> = rows
            .iter()
            .map(|n| {
                serde_json::json!({
                    "id": n.id, "type": n.ty, "status": n.status,
                    "title": title_of(n), "path": n.path,
                })
            })
            .collect();
        outln!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": OUTPUT_SCHEMA_VERSION,
                "nodes": items,
            }))?
        );
        return Ok(());
    }
    for n in &rows {
        outln!(
            "{}  {}  [{}]  {}",
            n.id,
            n.ty,
            n.status.as_deref().unwrap_or("-"),
            title_of(n)
        );
    }
    outln!("{} node(s)", rows.len());
    Ok(())
}

fn show(
    root: &Path,
    id: &str,
    json: bool,
    cache: bool,
    at: Option<&str>,
    text: bool,
) -> Result<()> {
    config::load(root)?;
    let revision = at.map(|spec| git::resolve_rev(root, spec)).transpose()?;
    let graph = match &revision {
        Some(rev) => git::graph_at(root, rev)?,
        None => load_graph(root, cache),
    };
    let node = graph
        .get(id)
        .with_context(|| format!("no node with id {id}"))?;
    let incoming = graph.incoming(&node.id);
    if json {
        let incoming: Vec<_> = incoming
            .iter()
            .map(|(n, kind)| serde_json::json!({"id": n.id, "kind": kind, "path": n.path}))
            .collect();
        let out = serde_json::json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "at": revision.as_ref().map(revision_json),
            "node": node,
            "incoming": incoming,
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    if let Some(rev) = &revision {
        outln!("at {rev}");
    }
    outln!(
        "{}  {}  [{}]",
        node.id,
        node.ty,
        node.status.as_deref().unwrap_or("no status")
    );
    outln!("path:  {}", node.path.display());
    if let Some(owner) = &node.owner {
        outln!("owner: {owner}");
    }
    if text {
        let raw = std::fs::read_to_string(root.join(&node.path))
            .with_context(|| format!("cannot read {}", node.path.display()))?;
        let raw = raw.replace("\r\n", "\n");
        let body = oxispec_core::parse::frontmatter_span(&raw)
            .map_or(raw.as_str(), |span| &raw[span.body_start..]);
        outln!("\n{}\n", body.trim());
    }
    for r in &node.requirements {
        let anchors: Vec<&str> = r
            .scenarios
            .iter()
            .filter_map(|s| s.anchor.as_deref())
            .collect();
        let count = r.scenarios.len();
        let noun = if count == 1 { "scenario" } else { "scenarios" };
        if anchors.is_empty() {
            outln!("requirement: {} ({count} {noun})", r.name);
        } else {
            outln!(
                "requirement: {} ({count} {noun}, anchors {})",
                r.name,
                anchors.join(", ")
            );
        }
    }
    if !node.links.is_empty() {
        outln!("links:");
        for (kind, targets) in &node.links {
            for t in targets {
                outln!("  {kind} -> {t}");
            }
        }
    }
    if !incoming.is_empty() {
        outln!("linked from:");
        for (n, kind) in incoming {
            outln!("  {} ({kind})  {}", n.id, n.path.display());
        }
    }
    Ok(())
}

fn slug(title: &str) -> Result<String> {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        bail!("the title needs at least one letter or digit");
    }
    Ok(out)
}

fn next_number(graph: &Graph, prefix: &str) -> u32 {
    graph
        .nodes
        .iter()
        .filter_map(|n| n.id.strip_prefix(prefix))
        .filter_map(|s| s.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

/// Picks a free ID starting with `prefix`. Random IDs are checked against
/// the nodes on disk, and another branch can still pick the same one, but
/// with about a million values that is rare and `oxispec check` reports it.
fn allocate_id(graph: &Graph, prefix: &str, scheme: IdScheme) -> Result<String> {
    match scheme {
        IdScheme::Sequential => Ok(format!("{prefix}{:04}", next_number(graph, prefix))),
        IdScheme::Random => {
            for _ in 0..50 {
                let mut bytes = [0u8; 4];
                getrandom::fill(&mut bytes).map_err(|e| anyhow!("cannot read randomness: {e}"))?;
                let candidate = format!("{prefix}{}", id::random_suffix(u32::from_le_bytes(bytes)));
                if graph.get(&candidate).is_none() {
                    return Ok(candidate);
                }
            }
            bail!("could not find a free random id; try again")
        }
    }
}

/// Writes a new file, failing if it already exists.
fn write_new(root: &Path, rel: &Path, content: &str) -> Result<()> {
    let path = root.join(rel);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .with_context(|| format!("cannot create {}", rel.display()))?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

const ADR_TEMPLATE: &str = "---
id: @ID@
type: adr
status: proposed
---
# @ID@: @TITLE@

## Context

## Decision

## Consequences
";

const PRD_TEMPLATE: &str = "---
id: @ID@
type: prd
status: draft
---
# @ID@: @TITLE@

## Overview

## Users

## Goals

## Non-goals

## Success metrics

## Open questions
";

fn new_prd(root: &Path, title: &str, scheme: IdScheme, cache: bool) -> Result<(String, PathBuf)> {
    let id = allocate_id(&load_graph(root, cache), "PRD-", scheme)?;
    let suffix = id.strip_prefix("PRD-").unwrap_or(&id);
    let rel = PathBuf::from("specs/prd").join(format!("{suffix}-{}.md", slug(title)?));
    let body = PRD_TEMPLATE.replace("@ID@", &id).replace("@TITLE@", title);
    write_new(root, &rel, &body)?;
    Ok((id, rel))
}

const CHANGE_TEMPLATE: &str = "---
id: @ID@
type: change
status: draft
---
# @ID@: @TITLE@

## Why

## What changes

## Impact
";

const VOCABULARY_TEMPLATE: &str = "---
id: @ID@
type: vocabulary
status: accepted
---
# @ID@: @TITLE@

The variables that claims talk about. A claim is a line in a `claims` block of
a requirement, for example `for role = admin: session.idle_timeout <= 15m`.

```vocabulary
# session.idle_timeout: {type: duration}
# password.min_length: {type: int}
# role: {type: enum, values: [admin, user, guest]}
# mfa: {type: bool}
```
";

const MACHINE_TEMPLATE: &str = "---
id: @ID@
type: state-machine
status: draft
---
# @ID@: @TITLE@

What this machine describes, and the requirements it implements.

```statemachine
initial: start
states: [start, done]
final: [done]
transitions:
  - {from: start, event: finish, to: done}
properties:
  - start leads to done
```
";

fn new_requirement(
    root: &Path,
    cfg: &Config,
    domain: &str,
    title: &str,
    format: Option<&str>,
    scheme: IdScheme,
    cache: bool,
) -> Result<(String, PathBuf)> {
    let (profiles, _) = oxispec_core::formats::available(cfg);
    let asked = format
        .or_else(|| cfg.formats.get("requirement").map(String::as_str))
        .unwrap_or("shall");
    let template = match profiles.iter().find(|p| p.name == asked) {
        Some(p) if p.ty == "requirement" => p.template(),
        Some(p) => bail!(
            "the profile `{asked}` is for {} nodes, not requirements",
            p.ty
        ),
        None => bail!("no profile named `{asked}`; see `oxispec formats list`"),
    };
    let domain = domain.to_ascii_uppercase();
    if domain.is_empty() || !domain.chars().all(|c| c.is_ascii_alphanumeric()) {
        bail!("the domain must be letters and digits, for example AUTH");
    }
    let id = allocate_id(&load_graph(root, cache), &format!("REQ-{domain}-"), scheme)?;
    let rel = PathBuf::from("specs/requirements")
        .join(domain.to_ascii_lowercase())
        .join(format!("{}.md", slug(title)?));
    let today = oxispec_core::debt::today();
    let body = template
        .replace("@ID@", &id)
        .replace("@TITLE@", title)
        .replace("@DATE@", &today);
    write_new(root, &rel, &body)?;
    Ok((id, rel))
}

fn new_adr(
    root: &Path,
    cfg: &Config,
    title: &str,
    format: Option<&str>,
    scheme: IdScheme,
    cache: bool,
) -> Result<(String, PathBuf)> {
    let (profiles, _) = oxispec_core::formats::available(cfg);
    let asked = format.or_else(|| cfg.formats.get("adr").map(String::as_str));
    let template = match asked {
        None => ADR_TEMPLATE.to_string(),
        Some(name) => match profiles.iter().find(|p| p.name == name) {
            Some(p) if p.ty == "adr" => p.template(),
            Some(p) => bail!("the profile `{name}` is for {} nodes, not ADRs", p.ty),
            None => bail!("no profile named `{name}`; see `oxispec formats list`"),
        },
    };
    let id = allocate_id(&load_graph(root, cache), "ADR-", scheme)?;
    let suffix = id.strip_prefix("ADR-").unwrap_or(&id);
    let rel = PathBuf::from("specs/adr").join(format!("{suffix}-{}.md", slug(title)?));
    let today = oxispec_core::change::utc_date(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    );
    let body = template
        .replace("@ID@", &id)
        .replace("@TITLE@", title)
        .replace("@DATE@", &today);
    write_new(root, &rel, &body)?;
    Ok((id, rel))
}

fn new_debt(
    root: &Path,
    title: &str,
    severity: &str,
    from: Option<&str>,
    scheme: IdScheme,
    cache: bool,
) -> Result<(String, PathBuf)> {
    if !oxispec_core::debt::SEVERITIES.contains(&severity) {
        bail!(
            "severity `{severity}` is not one of {}",
            oxispec_core::debt::SEVERITIES.join(", ")
        );
    }
    let graph = load_graph(root, cache);
    if let Some(from) = from
        && graph.get(from).is_none()
    {
        bail!("no node {from} to link `arises-from` to");
    }
    let id = allocate_id(&graph, "DEBT-", scheme)?;
    let suffix = id.strip_prefix("DEBT-").unwrap_or(&id);
    let rel = PathBuf::from("specs/debt").join(format!("{suffix}-{}.md", slug(title)?));
    let today = oxispec_core::debt::today();
    let links = from.map_or(String::new(), |f| format!("links:\n  arises-from: {f}\n"));
    let body = debt::TEMPLATE
        .replace("@ID@", &id)
        .replace("@TITLE@", title)
        .replace("@SEVERITY@", severity)
        .replace("@DATE@", &today)
        .replace("@LINKS@", &links);
    write_new(root, &rel, &body)?;
    Ok((id, rel))
}

const TEST_PLAN_TEMPLATE: &str = "---
id: @ID@
type: test-plan
status: draft
owner:
date: @DATE@
links:
  covers: []
---
# @ID@: @TITLE@

## Scope

What is tested, and what is not.

## Approach

## Levels

Unit, integration, end to end, security, performance: which apply, and where each runs.

## Environments

## Exit criteria

When testing is done: which scenarios pass, and what may be left.
";

fn new_test_plan(
    root: &Path,
    title: &str,
    scheme: IdScheme,
    cache: bool,
) -> Result<(String, PathBuf)> {
    let id = allocate_id(&load_graph(root, cache), "TP-", scheme)?;
    let suffix = id.strip_prefix("TP-").unwrap_or(&id);
    let rel = PathBuf::from("specs/test-plans").join(format!("{suffix}-{}.md", slug(title)?));
    let body = TEST_PLAN_TEMPLATE
        .replace("@ID@", &id)
        .replace("@TITLE@", title)
        .replace("@DATE@", &oxispec_core::debt::today());
    write_new(root, &rel, &body)?;
    Ok((id, rel))
}

fn new_risk(
    root: &Path,
    title: &str,
    likelihood: &str,
    impact: &str,
    from: Option<&str>,
    scheme: IdScheme,
    cache: bool,
) -> Result<(String, PathBuf)> {
    if !oxispec_core::risks::LIKELIHOODS.contains(&likelihood) {
        bail!(
            "likelihood `{likelihood}` is not one of {}",
            oxispec_core::risks::LIKELIHOODS.join(", ")
        );
    }
    if !oxispec_core::risks::IMPACTS.contains(&impact) {
        bail!(
            "impact `{impact}` is not one of {}",
            oxispec_core::risks::IMPACTS.join(", ")
        );
    }
    let graph = load_graph(root, cache);
    if let Some(from) = from
        && graph.get(from).is_none()
    {
        bail!("no node {from} to link `arises-from` to");
    }
    let id = allocate_id(&graph, "RISK-", scheme)?;
    let suffix = id.strip_prefix("RISK-").unwrap_or(&id);
    let rel = PathBuf::from("specs/risks").join(format!("{suffix}-{}.md", slug(title)?));
    let links = from.map_or(String::new(), |f| format!("links:\n  arises-from: {f}\n"));
    let body = risks::TEMPLATE
        .replace("@ID@", &id)
        .replace("@TITLE@", title)
        .replace("@LIKELIHOOD@", likelihood)
        .replace("@IMPACT@", impact)
        .replace("@DATE@", &oxispec_core::debt::today())
        .replace("@LINKS@", &links);
    write_new(root, &rel, &body)?;
    Ok((id, rel))
}

fn new_vocabulary(
    root: &Path,
    title: &str,
    scheme: IdScheme,
    cache: bool,
) -> Result<(String, PathBuf)> {
    let id = allocate_id(&load_graph(root, cache), "VOC-", scheme)?;
    let suffix = id.strip_prefix("VOC-").unwrap_or(&id);
    let rel = PathBuf::from("specs/vocabulary").join(format!("{suffix}-{}.md", slug(title)?));
    let body = VOCABULARY_TEMPLATE
        .replace("@ID@", &id)
        .replace("@TITLE@", title);
    write_new(root, &rel, &body)?;
    Ok((id, rel))
}

const ENTITY_TEMPLATE: &str = "---
id: @ID@
type: entity
status: draft
---
# @ID@: @TITLE@

What it is, who or what creates it, and what ends it.

```entity
name: @NAME@
attributes:
#  total: {type: int}
#  status: {type: enum, values: [open, paid, shipped]}
```

Requirements, tests and decisions that are about it say so with a link:
`oxispec link REQ-ORDERS-0001 about @ID@`. The attributes are variables named
`@NAME@.ATTRIBUTE`, so claims can use them.
";

// spec: REQ-ENTITY-0004
fn new_entity(
    root: &Path,
    title: &str,
    scheme: IdScheme,
    cache: bool,
) -> Result<(String, PathBuf)> {
    let id = allocate_id(&load_graph(root, cache), "ENT-", scheme)?;
    let suffix = id.strip_prefix("ENT-").unwrap_or(&id);
    let name = slug(title)?.replace('-', "_");
    let rel = PathBuf::from("specs/entities").join(format!("{suffix}-{}.md", slug(title)?));
    let body = ENTITY_TEMPLATE
        .replace("@ID@", &id)
        .replace("@TITLE@", title)
        .replace("@NAME@", &name);
    write_new(root, &rel, &body)?;
    Ok((id, rel))
}

const ELEMENT_TEMPLATE: &str = "---
id: @ID@
type: @TYPE@
status: draft
---
# @ID@: @TITLE@

What it is and why it exists.

```architecture
name: @TITLE@
@FIELDS@```
";

/// The starter fields for each kind of element.
fn element_fields(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::System | ElementKind::Person => {
            "# uses:\n#   - {to: C4-0002, description: sends orders}\n"
        }
        ElementKind::Container => {
            "technology: \nowner: \nparent: # the ID of its system\n# uses:\n#   - {to: C4-0002, description: reads and writes, technology: SQL}\n"
        }
        ElementKind::Component => {
            "parent: # the ID of its container\n# uses:\n#   - {to: C4-0002, description: calls}\n"
        }
        ElementKind::Environment => {
            "nodes:\n#  - {id: cluster, kind: kubernetes-cluster, region: eu-west-1, hosts: [C4-0002]}\n"
        }
    }
}

fn new_element(
    root: &Path,
    kind: ElementKind,
    title: &str,
    scheme: IdScheme,
    cache: bool,
) -> Result<(String, PathBuf)> {
    let (prefix, ty) = match kind {
        ElementKind::System => ("C4-", "system"),
        ElementKind::Person => ("C4-", "person"),
        ElementKind::Container => ("C4-", "container"),
        ElementKind::Component => ("C4-", "component"),
        ElementKind::Environment => ("DEP-", "deployment-environment"),
    };
    let id = allocate_id(&load_graph(root, cache), prefix, scheme)?;
    let suffix = id.strip_prefix(prefix).unwrap_or(&id);
    let rel = PathBuf::from("architecture/model").join(format!("{suffix}-{}.md", slug(title)?));
    let body = ELEMENT_TEMPLATE
        .replace("@ID@", &id)
        .replace("@TYPE@", ty)
        .replace("@TITLE@", title)
        .replace("@FIELDS@", element_fields(kind));
    write_new(root, &rel, &body)?;
    Ok((id, rel))
}

/// An exclusive lock on the project for a command that writes files, held until the returned
/// file is dropped. Two such commands at once would otherwise read the same file, change it
/// differently, and the second write would erase the first.
/// Holds the project lock. Dropping it tells a long-running process (the MCP
/// server) that files may have changed, without waiting for the file watcher.
struct ProjectLock(#[allow(dead_code)] fs::File);

impl Drop for ProjectLock {
    fn drop(&mut self) {
        oxispec_core::index::graph_may_have_changed();
    }
}

// spec: REQ-ROBUSTNESS-0002
fn lock_project(root: &Path) -> Result<ProjectLock> {
    let dir = root.join(oxispec_core::index::CACHE_DIR);
    fs::create_dir_all(&dir)?;
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(dir.join("new.lock"))
        .context("cannot create the lock for `oxispec new`")?;
    file.lock()
        .context("cannot lock the project for `oxispec new`")?;
    Ok(ProjectLock(file))
}

fn new_machine(
    root: &Path,
    title: &str,
    scheme: IdScheme,
    cache: bool,
) -> Result<(String, PathBuf)> {
    let id = allocate_id(&load_graph(root, cache), "SM-", scheme)?;
    let suffix = id.strip_prefix("SM-").unwrap_or(&id);
    let rel = PathBuf::from("specs/machines").join(format!("{suffix}-{}.md", slug(title)?));
    let body = MACHINE_TEMPLATE
        .replace("@ID@", &id)
        .replace("@TITLE@", title);
    write_new(root, &rel, &body)?;
    Ok((id, rel))
}

fn new_change(
    root: &Path,
    title: &str,
    scheme: IdScheme,
    cache: bool,
) -> Result<(String, PathBuf)> {
    let id = allocate_id(&load_graph(root, cache), "CHG-", scheme)?;
    let suffix = id.strip_prefix("CHG-").unwrap_or(&id);
    let rel = PathBuf::from("specs/changes").join(format!("{suffix}-{}.md", slug(title)?));
    let body = CHANGE_TEMPLATE
        .replace("@ID@", &id)
        .replace("@TITLE@", title);
    write_new(root, &rel, &body)?;
    Ok((id, rel))
}
