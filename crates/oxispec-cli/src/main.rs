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

mod agents;
mod architecture;
mod changes;
mod ci;
mod claims;
mod connect;
mod context;
mod formats;
mod git;
mod history;
mod machines;
mod mcp;
mod releases;
mod rollback;
mod search;
mod server;
mod site;
mod status;
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
        /// describes, overrides, conflicts-with, depends-on, changes or includes
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
    /// Show a node, its links, and the nodes that link to it
    Show {
        id: String,
        #[arg(long)]
        json: bool,
        /// Show the node as it was at a git revision (a branch, tag, hash or date:YYYY-MM-DD)
        #[arg(long)]
        at: Option<String>,
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
    Requirement { domain: String, title: String },
    /// An architecture decision record
    Adr {
        title: String,
        /// The format profile to write it in. Default: the one the config chooses for ADRs
        #[arg(long)]
        format: Option<String>,
    },
    /// A change set: a proposed change to the specs, with a lifecycle
    Change { title: String },
    /// A vocabulary: the typed variables that claims in requirements talk about
    Vocabulary { title: String },
    /// A state machine: states, events and transitions, with properties that are checked
    Machine { title: String },
    /// An element of the architecture model, or a deployment environment
    Architecture { kind: ElementKind, title: String },
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
    let cli = Cli::parse();
    // `run` is a large function and the MCP servers call it again for each tool,
    // so the main thread's stack (1 MiB on Windows) is not enough. A thread of our own has room.
    let program = std::thread::Builder::new()
        .name("oxispec".into())
        .stack_size(STACK_BYTES)
        .spawn(move || match run(cli) {
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
        } => {
            let dir = cli.root.unwrap_or_else(|| PathBuf::from("."));
            init(&dir, &preset, &with, &without, id_scheme.into(), &agents)?;
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
                NewKind::Requirement { domain, title } => {
                    require_module(&cfg, "specs")?;
                    new_requirement(&root, &domain, &title, scheme, cache)?
                }
                NewKind::Adr { title, format } => {
                    require_module(&cfg, "adrs")?;
                    new_adr(&root, &cfg, &title, format.as_deref(), scheme, cache)?
                }
                NewKind::Change { title } => {
                    require_module(&cfg, "specs")?;
                    new_change(&root, &title, scheme, cache)?
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
        Command::Show { id, json, at } => {
            show(
                &project_root(cli.root.as_deref())?,
                &id,
                json,
                cache,
                at.as_deref(),
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
    Ok(())
}

fn init(
    dir: &Path,
    preset: &str,
    with: &[String],
    without: &[String],
    id_scheme: IdScheme,
    agent_clients: &[String],
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

fn show(root: &Path, id: &str, json: bool, cache: bool, at: Option<&str>) -> Result<()> {
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

const REQUIREMENT_TEMPLATE: &str = "---
id: @ID@
type: requirement
status: draft
---
### Requirement: @TITLE@
The system SHALL <describe the behavior>.

#### Scenario: <name> {#S1}
- **WHEN** <condition>
- **THEN** <outcome>
";

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
    domain: &str,
    title: &str,
    scheme: IdScheme,
    cache: bool,
) -> Result<(String, PathBuf)> {
    let domain = domain.to_ascii_uppercase();
    if domain.is_empty() || !domain.chars().all(|c| c.is_ascii_alphanumeric()) {
        bail!("the domain must be letters and digits, for example AUTH");
    }
    let id = allocate_id(&load_graph(root, cache), &format!("REQ-{domain}-"), scheme)?;
    let rel = PathBuf::from("specs/requirements")
        .join(domain.to_ascii_lowercase())
        .join(format!("{}.md", slug(title)?));
    let body = REQUIREMENT_TEMPLATE
        .replace("@ID@", &id)
        .replace("@TITLE@", title);
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
