use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use clap::{ArgAction, Args, CommandFactory, Parser, Subcommand};
use clap_complete::{
    generate,
    shells::{Bash, PowerShell, Zsh},
};

use crate::app::durable::{
    add_backlog, add_decision, add_intake, add_report, add_story, close_backlog, get_entity,
    record_decision_verification, record_story_verification, update_decision, update_intake,
    update_report, update_story, StoryUpdate,
};
use crate::app::index::{
    ensure_index, format_links_view, format_search_hits, links_for, search_index,
    write_project_index,
};
use crate::app::init::{run_init, run_migrate};
use crate::app::link::{link_project, list_projects, read_project_id, unlink_project};
use crate::app::local::{
    append_trace, append_worklog, git_commits, latest_trace, read_records, remove_tool,
    score_trace, upsert_tool,
};
use crate::app::project_link;
use crate::app::query::{query_view, query_view_json};
use crate::app::status::{
    doctor_json, format_doctor, format_handoff, format_status, next_items, status_json,
};
use crate::domain::frontmatter::as_string;
use crate::domain::paths::resolve_target_dir;
use crate::error::{Error, Result};
use crate::infra::entities::MutationLock;
use crate::VERSION;

#[derive(Parser, Debug)]
#[command(
    name = "harness",
    version = VERSION,
    disable_version_flag = true,
    about = "npm-native agent-ready repository harness — init, durable records, and queries",
    long_about = None
)]
struct Cli {
    /// print CLI version (also -V)
    #[arg(short = 'v', long = "version", action = ArgAction::SetTrue, global = true)]
    version: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Args, Debug, Clone)]
struct DirOpts {
    /// target project directory (default: cwd)
    #[arg(short = 'd', long = "dir")]
    dir: Option<String>,
    /// alias for --dir
    #[arg(long = "directory")]
    directory: Option<String>,
}

impl DirOpts {
    fn path(&self, positional: Option<&str>, cwd: &Path) -> PathBuf {
        let chosen = self
            .dir
            .as_deref()
            .or(self.directory.as_deref())
            .or(positional);
        resolve_target_dir(chosen, cwd)
    }
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Scaffold markdown operating files and register the project
    Init {
        /// target project directory (default: cwd)
        #[arg(value_name = "directory")]
        target: Option<String>,
        #[command(flatten)]
        dir: DirOpts,
        /// non-interactive (reserved)
        #[arg(short = 'y', long = "yes")]
        yes: bool,
        /// print planned operations without writing
        #[arg(long = "dry-run")]
        dry_run: bool,
        /// overwrite conflicting files after backup under .5harness-backup/
        #[arg(long = "force")]
        force: bool,
    },
    /// Legacy: migrate existing harness.db if present (markdown is SoT)
    Migrate {
        #[arg(value_name = "directory")]
        target: Option<String>,
        #[command(flatten)]
        dir: DirOpts,
    },
    /// Import legacy harness.db rows into markdown entities (non-clobbering)
    ImportSqlite {
        #[arg(value_name = "directory")]
        target: Option<String>,
        #[command(flatten)]
        dir: DirOpts,
    },
    /// Register a project path in the machine-local global registry
    Link {
        #[arg(value_name = "directory")]
        target: Option<String>,
        #[command(flatten)]
        dir: DirOpts,
    },
    /// Remove a project from the global registry (does not delete files)
    Unlink {
        #[arg(value_name = "directory")]
        target: Option<String>,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "id")]
        id: Option<String>,
        #[arg(long = "missing")]
        missing: bool,
    },
    /// Completely remove 5harness from a project (unlink + delete state + strip AGENTS.md)
    Remove {
        #[arg(value_name = "directory")]
        target: Option<String>,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "force")]
        force: bool,
        #[arg(long = "keep-entities")]
        keep_entities: bool,
    },
    /// Alias for `harness remove`
    Rm {
        #[arg(value_name = "directory")]
        target: Option<String>,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "force")]
        force: bool,
        #[arg(long = "keep-entities")]
        keep_entities: bool,
    },
    /// List projects linked in the global registry
    Projects,
    /// Inspect project-local Harness identity and Project Link
    Project {
        #[command(subcommand)]
        cmd: ProjectCmd,
    },
    /// Create and manage target-owned Project Link reports
    Report {
        #[command(subcommand)]
        cmd: ReportCmd,
    },
    /// Read bounded durable context from a configured project peer
    Peer {
        #[command(subcommand)]
        cmd: PeerCmd,
    },
    /// Start local multi-project dashboard (localhost) or manage settings
    Dashboard {
        #[arg(long = "port", default_value = "3927")]
        port: u16,
        #[arg(long = "host", default_value = "127.0.0.1")]
        host: String,
        #[arg(long = "public-url")]
        public_url: Option<String>,
        #[command(subcommand)]
        cmd: Option<DashboardCmd>,
    },
    /// Browse and search harness documentation
    Docs {
        #[command(subcommand)]
        cmd: DocsCmd,
    },
    /// Print shell completion script (bash | zsh | pwsh)
    Completion { shell: String },
    /// Update 5harness globally using the detected package manager
    Update,
    /// Upgrade harness block in AGENTS.md to match current CLI version
    Upgrade {
        #[command(flatten)]
        dir: DirOpts,
    },
    /// Rebuild derived agent index from markdown entities
    Reindex {
        #[command(flatten)]
        dir: DirOpts,
    },
    /// Print one durable entity by id or path
    Get {
        id_or_path: String,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "summary")]
        summary: bool,
        #[arg(long = "json")]
        json: bool,
    },
    /// Search entity catalog (path + snippet, not full dump)
    Search {
        query: String,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "limit", default_value = "20")]
        limit: usize,
        #[arg(long = "type")]
        ty: Option<String>,
        #[arg(long = "json")]
        json: bool,
    },
    /// Show outbound links and backlinks for an entity
    Links {
        id: String,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
        #[arg(long = "broken")]
        broken: bool,
    },
    /// Analyze prompt and suggest intake classification
    IntakeRun {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "prompt")]
        prompt: Option<String>,
        #[arg(long = "summary")]
        summary: Option<String>,
        #[arg(long = "json")]
        json: bool,
        #[arg(long = "commit")]
        commit: bool,
    },
    /// Record a feature intake classification
    Intake {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "type")]
        ty: Option<String>,
        #[arg(long = "summary")]
        summary: Option<String>,
        #[arg(long = "lane")]
        lane: Option<String>,
        #[arg(long = "flags")]
        flags: Option<String>,
        #[arg(long = "docs")]
        docs: Option<String>,
        #[arg(long = "story")]
        story: Option<String>,
        #[arg(long = "stories")]
        stories: Option<String>,
        #[arg(long = "notes")]
        notes: Option<String>,
        #[arg(long = "links")]
        links: Option<String>,
        #[command(subcommand)]
        cmd: Option<IntakeCmd>,
    },
    /// Add or update a story matrix row
    Story {
        #[command(subcommand)]
        cmd: StoryCmd,
    },
    /// Record a durable decision
    Decision {
        #[command(subcommand)]
        cmd: DecisionCmd,
    },
    /// Manage harness improvement backlog
    Backlog {
        #[command(subcommand)]
        cmd: BacklogCmd,
    },
    /// Query harness durable data
    Query {
        #[command(subcommand)]
        cmd: QueryCmd,
    },
    /// Record an agent execution trace
    Trace {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "summary")]
        summary: String,
        #[arg(long = "intake")]
        intake: Option<String>,
        #[arg(long = "story")]
        story: Option<String>,
        #[arg(long = "agent")]
        agent: Option<String>,
        #[arg(long = "outcome", default_value = "completed")]
        outcome: String,
        #[arg(long = "duration")]
        duration: Option<u64>,
        #[arg(long = "tokens")]
        tokens: Option<u64>,
        #[arg(long = "actions")]
        actions: Option<String>,
        #[arg(long = "read")]
        files_read: Option<String>,
        #[arg(long = "changed")]
        files_changed: Option<String>,
        #[arg(long = "decisions")]
        decisions: Option<String>,
        #[arg(long = "errors")]
        errors: Option<String>,
        #[arg(long = "friction")]
        friction: Option<String>,
        #[arg(long = "notes")]
        notes: Option<String>,
        #[arg(long = "json")]
        json: bool,
    },
    /// Score a trace against quality tiers
    ScoreTrace {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "id")]
        id: Option<String>,
        #[arg(long = "json")]
        json: bool,
    },
    /// Durable evidence trail linking implementation to stories
    Worklog {
        #[command(subcommand)]
        cmd: WorklogCmd,
    },
    /// Run workspace health checks for human and agent users
    Doctor {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
    },
    /// Project snapshot for agents: work, Project Link, version, index
    Status {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
    },
    /// Recommend next work item (active stories, backend reports, planned work)
    Next {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
        #[arg(long = "limit")]
        limit: Option<usize>,
    },
    /// Budgeted entity context pack (body + outbound/backlinks + proof)
    Context {
        id: String,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
        #[arg(long = "depth")]
        depth: Option<u32>,
        #[arg(long = "max-chars")]
        max_chars: Option<usize>,
    },
    /// Inbound tool registry: register, check, and remove external tools
    Tool {
        #[command(subcommand)]
        cmd: ToolCmd,
    },
    /// Run drift audit and entropy score
    Audit {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
    },
    /// Generate improvement proposals from audit findings
    Propose {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "commit")]
        commit: bool,
    },
    /// Export artifacts from durable history
    Export {
        #[command(subcommand)]
        cmd: ExportCmd,
    },
    /// Watch entity directories and auto-reindex on markdown changes
    Watch {
        #[command(flatten)]
        dir: DirOpts,
    },
    /// Emit concise session summary for the next agent (traces, worklog, status)
    Handoff {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "story")]
        story: Option<String>,
        #[arg(long = "json")]
        json: bool,
    },
    /// Start OAuth-protected MCP over HTTP (default port 3928)
    Mcp {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "port", default_value = "3928")]
        port: u16,
        #[arg(long = "host", default_value = "127.0.0.1")]
        host: String,
        #[arg(long = "public-url")]
        public_url: Option<String>,
        /// bearer token for MCP clients (generated when omitted)
        #[arg(long = "token")]
        token: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum ProjectCmd {
    /// Print the durable project id from AGENTS.md
    Id {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
        #[arg(long = "ensure")]
        ensure: bool,
    },
    Role {
        #[command(subcommand)]
        cmd: RoleCmd,
    },
    Peer {
        #[command(subcommand)]
        cmd: ProjectPeerCmd,
    },
}

#[derive(Subcommand, Debug)]
enum RoleCmd {
    Set {
        role: String,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "stack")]
        stack: Option<String>,
        #[arg(long = "json")]
        json: bool,
    },
    Show {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ProjectPeerCmd {
    Add {
        id_or_path: String,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "role")]
        role: Option<String>,
    },
    Remove {
        project_id: String,
        #[command(flatten)]
        dir: DirOpts,
    },
    List {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ReportCmd {
    Add {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "to")]
        to: String,
        #[arg(long = "summary")]
        summary: String,
    },
    List {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
        #[arg(long = "status")]
        status: Option<String>,
    },
    Get {
        id: String,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "from")]
        from: Option<String>,
    },
    Update {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "id")]
        id: String,
        #[arg(long = "status")]
        status: String,
        #[arg(long = "resolution")]
        resolution: Option<String>,
        #[arg(long = "related")]
        related: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum PeerCmd {
    Search {
        query: String,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "peer")]
        peer: Option<String>,
        #[arg(long = "role")]
        role: Option<String>,
        #[arg(long = "limit", default_value = "20")]
        limit: usize,
        #[arg(long = "json")]
        json: bool,
    },
    Get {
        id_or_path: String,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "peer")]
        peer: Option<String>,
        #[arg(long = "role")]
        role: Option<String>,
        #[arg(long = "summary")]
        summary: bool,
        #[arg(long = "json")]
        json: bool,
    },
    Context {
        id: String,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "peer")]
        peer: Option<String>,
        #[arg(long = "role")]
        role: Option<String>,
        #[arg(long = "depth")]
        depth: Option<u32>,
        #[arg(long = "max-chars")]
        max_chars: Option<usize>,
        #[arg(long = "json")]
        json: bool,
    },
    Links {
        id: String,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "peer")]
        peer: Option<String>,
        #[arg(long = "role")]
        role: Option<String>,
        #[arg(long = "json")]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum DashboardCmd {
    /// Change the dashboard authentication password
    SetPassword {
        #[arg(long = "password")]
        password: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum DocsCmd {
    Search {
        query: String,
        #[arg(long = "json")]
        json: bool,
    },
    List {
        #[arg(long = "json")]
        json: bool,
    },
    Read {
        path: String,
        #[arg(long = "json")]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum IntakeCmd {
    Update {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "id")]
        id: String,
        #[arg(long = "status")]
        status: Option<String>,
        #[arg(long = "stories")]
        stories: Option<String>,
        #[arg(long = "notes")]
        notes: Option<String>,
    },
    Close {
        id: Option<String>,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "id")]
        id_flag: Option<String>,
        #[arg(long = "notes")]
        notes: Option<String>,
    },
    Dismiss {
        id: Option<String>,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "id")]
        id_flag: Option<String>,
        #[arg(long = "notes")]
        notes: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum StoryCmd {
    Add {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "id")]
        id: String,
        #[arg(long = "title")]
        title: String,
        #[arg(long = "lane")]
        lane: String,
        #[arg(long = "contract")]
        contract: Option<String>,
        #[arg(long = "verify")]
        verify: Option<String>,
        #[arg(long = "notes")]
        notes: Option<String>,
        #[arg(long = "links")]
        links: Option<String>,
    },
    Update {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "id")]
        id: String,
        #[arg(long = "status")]
        status: Option<String>,
        #[arg(long = "evidence")]
        evidence: Option<String>,
        #[arg(long = "unit")]
        unit: Option<String>,
        #[arg(long = "integration")]
        integration: Option<String>,
        #[arg(long = "e2e")]
        e2e: Option<String>,
        #[arg(long = "platform")]
        platform: Option<String>,
        #[arg(long = "verify")]
        verify: Option<String>,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "contract")]
        contract: Option<String>,
        #[arg(long = "notes")]
        notes: Option<String>,
        #[arg(long = "links")]
        links: Option<String>,
    },
    Start {
        id: Option<String>,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "id")]
        id_flag: Option<String>,
        #[arg(long = "evidence")]
        evidence: Option<String>,
    },
    Done {
        id: Option<String>,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "id")]
        id_flag: Option<String>,
        #[arg(long = "evidence")]
        evidence: Option<String>,
    },
    Block {
        id: Option<String>,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "id")]
        id_flag: Option<String>,
        #[arg(long = "reason")]
        reason: Option<String>,
    },
    Verify {
        id: Option<String>,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "id")]
        id_flag: Option<String>,
        /// Explicitly approve execution of the project-authored shell command.
        #[arg(long = "allow-project-command")]
        allow_project_command: bool,
    },
    VerifyAll {
        #[command(flatten)]
        dir: DirOpts,
        /// Explicitly approve execution of all project-authored shell commands.
        #[arg(long = "allow-project-command")]
        allow_project_command: bool,
    },
}

#[derive(Subcommand, Debug)]
enum DecisionCmd {
    Add {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "id")]
        id: String,
        #[arg(long = "title")]
        title: String,
        #[arg(long = "status")]
        status: Option<String>,
        #[arg(long = "doc")]
        doc: Option<String>,
        #[arg(long = "verify")]
        verify: Option<String>,
        #[arg(long = "notes")]
        notes: Option<String>,
        #[arg(long = "links")]
        links: Option<String>,
        #[arg(long = "force")]
        force: bool,
    },
    Update {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "id")]
        id: String,
        #[arg(long = "title")]
        title: Option<String>,
        #[arg(long = "status")]
        status: Option<String>,
        #[arg(long = "doc")]
        doc: Option<String>,
        #[arg(long = "verify")]
        verify: Option<String>,
        #[arg(long = "notes")]
        notes: Option<String>,
        #[arg(long = "links")]
        links: Option<String>,
    },
    Verify {
        id: Option<String>,
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "id")]
        id_flag: Option<String>,
        /// Explicitly approve execution of the project-authored shell command.
        #[arg(long = "allow-project-command")]
        allow_project_command: bool,
    },
}

#[derive(Subcommand, Debug)]
enum BacklogCmd {
    Add {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "title")]
        title: String,
        #[arg(long = "while")]
        while_text: Option<String>,
        #[arg(long = "pain")]
        pain: Option<String>,
        #[arg(long = "suggestion")]
        suggestion: Option<String>,
        #[arg(long = "risk")]
        risk: Option<String>,
        #[arg(long = "predicted")]
        predicted: Option<String>,
        #[arg(long = "notes")]
        notes: Option<String>,
        #[arg(long = "links")]
        links: Option<String>,
    },
    Close {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "id")]
        id: String,
        #[arg(long = "status")]
        status: Option<String>,
        #[arg(long = "outcome")]
        outcome: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum QueryCmd {
    /// Story test matrix
    Matrix {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "numeric")]
        numeric: bool,
        #[arg(long = "json")]
        json: bool,
    },
    /// Summary counts
    Stats {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
    },
    Intakes {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
    },
    Decisions {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
    },
    Stories {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
    },
    Backlog {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "open")]
        open: bool,
        #[arg(long = "closed")]
        closed: bool,
        #[arg(long = "json")]
        json: bool,
    },
    Traces {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
    },
    Reports {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
    },
    Tools {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum WorklogCmd {
    Add {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "story")]
        story: String,
        #[arg(long = "summary")]
        summary: String,
        #[arg(long = "pr")]
        pr: Option<String>,
        #[arg(long = "commit")]
        commit: Option<String>,
    },
    List {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "json")]
        json: bool,
    },
    FromGit {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "story")]
        story: String,
        #[arg(long = "since")]
        since: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum ToolCmd {
    Register {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "name")]
        name: String,
        #[arg(long = "command")]
        command: String,
        #[arg(long = "description")]
        description: String,
        #[arg(long = "responsibility")]
        responsibility: String,
        #[arg(long = "kind", default_value = "external")]
        kind: String,
        #[arg(long = "capability")]
        capability: Option<String>,
        #[arg(long = "json")]
        json: bool,
    },
    Check {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "name")]
        name: Option<String>,
        #[arg(long = "json")]
        json: bool,
    },
    Remove {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "name")]
        name: String,
        #[arg(long = "json")]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ExportCmd {
    Changelog {
        #[command(flatten)]
        dir: DirOpts,
        #[arg(long = "since")]
        since: Option<String>,
        #[arg(long = "json")]
        json: bool,
    },
}

pub fn run() -> Result<()> {
    let mut argv: Vec<String> = env::args().collect();
    for a in argv.iter_mut() {
        if a == "-V" {
            *a = "--version".into();
        }
    }
    let cli = Cli::parse_from(argv);
    if cli.version {
        println!("{VERSION}");
        return Ok(());
    }
    let cwd = env::current_dir()?;
    match cli.command {
        None => run_dashboard("127.0.0.1", 3927, true, None),
        Some(cmd) => dispatch(cmd, &cwd),
    }
}

fn dispatch(cmd: Commands, cwd: &Path) -> Result<()> {
    match cmd {
        Commands::Init {
            target,
            dir,
            dry_run,
            force,
            yes: _,
        } => {
            let result = run_init(
                target.as_deref().or(dir.dir.as_deref()).or(dir.directory.as_deref()),
                force,
                dry_run,
                cwd,
                false,
            )?;
            for line in &result.logs {
                println!("{line}");
            }
            println!();
            if result.dry_run {
                println!("Dry run complete for {}", result.target_dir.display());
            } else {
                println!("Harness initialized in {}", result.target_dir.display());
                println!(
                    "Files created: {}, overwritten: {}, skipped: {}",
                    result.created.len(),
                    result.overwritten.len(),
                    result.skipped.len()
                );
                if result.registered {
                    if let Some(p) = result.registry_path {
                        println!("Registered in global registry: {}", p.display());
                    }
                }
                println!("Entity dirs: docs/stories|decisions|intakes|backlog|reports");
            }
            Ok(())
        }
        Commands::Migrate { target, dir } => {
            let target = dir.path(target.as_deref(), cwd);
            println!("{}", run_migrate(&target));
            Ok(())
        }
        Commands::ImportSqlite { .. } => Err(Error::new("import-sqlite is not implemented in the markdown-only release; use migration tooling before init")),
        Commands::Link { target, dir } => {
            let chosen = target.as_deref().or(dir.dir.as_deref()).or(dir.directory.as_deref());
            let link = link_project(chosen, cwd)?;
            println!(
                "Linked {} ({}) → {}",
                link.entry.name,
                link.entry.id,
                link.registry_path.display()
            );
            Ok(())
        }
        Commands::Unlink { target, dir, .. } => {
            let chosen = target.as_deref().or(dir.dir.as_deref()).or(dir.directory.as_deref());
            let (removed, path) = unlink_project(chosen, cwd)?;
            match removed {
                Some(p) => println!("Unlinked {} from {}", p.name, path.display()),
                None => println!("No registry entry for that path."),
            }
            Ok(())
        }
        Commands::Remove { target, dir, force, keep_entities }
        | Commands::Rm { target, dir, force, keep_entities } => {
            let target = dir.path(target.as_deref(), cwd);
            if !force {
                return Err(Error::new("remove is destructive; rerun with --force"));
            }
            unlink_project(Some(&target.to_string_lossy()), cwd)?;
            let agents = target.join("AGENTS.md");
            if agents.exists() {
                let text = fs::read_to_string(&agents)?;
                let stripped = crate::domain::upgrade::remove_harness_block(&text);
                crate::infra::entities::atomic_write(&agents, &stripped)?;
            }
            let state = target.join(".5harness");
            if state.exists() {
                fs::remove_dir_all(&state)?;
            }
            if !keep_entities {
                for ty in crate::domain::entities::ENTITY_TYPES {
                    let dir = target.join(crate::domain::entities::entity_dir(ty)?);
                    if dir.exists() { fs::remove_dir_all(dir)?; }
                }
            }
            if !keep_entities {
                println!("Removed harness state from {}", target.display());
            } else {
                println!("Removed harness state (kept entity dirs) from {}", target.display());
            }
            Ok(())
        }
        Commands::Projects => {
            for (p, missing) in list_projects() {
                let flag = if missing { " missing" } else { "" };
                println!("{}  {}  {}{flag}", p.id, p.name, p.path);
            }
            Ok(())
        }
        Commands::Project { cmd } => match cmd {
            ProjectCmd::Id { dir, json, ensure } => {
                let target = dir.path(None, cwd);
                let id = if ensure {
                    crate::app::init::ensure_project_id(&target, None)?
                } else {
                    read_project_id(&target)?
                };
                if json {
                    println!(
                        "{}",
                        serde_json::json!({"id": id, "path": target, "name": target.file_name().and_then(|s| s.to_str())})
                    );
                } else {
                    println!("{id}");
                }
                Ok(())
            }
            ProjectCmd::Role { cmd } => {
                match cmd {
                    RoleCmd::Set { role, dir, stack, json } => {
                        let target = dir.path(None, cwd);
                        let value = project_link::set_role(&target, &role, stack.as_deref())?;
                        if json { println!("{}", serde_json::to_string_pretty(&value)?); }
                        else { println!("Project role set to {}", role); }
                        Ok(())
                    }
                    RoleCmd::Show { dir, json } => {
                        let value = project_link::role(&dir.path(None, cwd))?;
                        if json { println!("{}", serde_json::to_string_pretty(&value)?); }
                        else { println!("role: {}\nstack: {}", value["role"].as_str().unwrap_or("unset"), value["stack"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(",")).unwrap_or_default()); }
                        Ok(())
                    }
                }
            }
            ProjectCmd::Peer { cmd } => {
                match cmd {
                    ProjectPeerCmd::Add { id_or_path, dir, role } => {
                        let value = project_link::add_peer(&dir.path(None, cwd), &id_or_path, role.as_deref())?;
                        println!("Peer {} configured{}.", value["id"].as_str().unwrap_or(""), if value["reverse_written"] == true { " (reverse marker written)" } else { "" });
                        Ok(())
                    }
                    ProjectPeerCmd::Remove { project_id, dir } => {
                        if project_link::remove_peer(&dir.path(None, cwd), &project_id)? { println!("Peer {project_id} removed."); } else { println!("Peer {project_id} was not configured."); }
                        Ok(())
                    }
                    ProjectPeerCmd::List { dir, json } => {
                        let values = project_link::peers(&dir.path(None, cwd))?;
                        if json { println!("{}", serde_json::to_string_pretty(&values)?); }
                        else { for value in values { println!("{}  {}  {}", value["id"].as_str().unwrap_or(""), value["role"].as_str().unwrap_or(""), value["path"].as_str().unwrap_or("unresolved")); } }
                        Ok(())
                    }
                }
            }
        },
        Commands::Report { cmd } => report_cmd(cmd, cwd),
        Commands::Peer { cmd } => peer_cmd(cmd, cwd),
        Commands::Dashboard { port, host, public_url, cmd } => match cmd {
            Some(DashboardCmd::SetPassword { password }) => {
                let password = password.ok_or_else(|| Error::new("dashboard set-password requires --password"))?;
                let path = crate::app::dashboard::set_dashboard_password(&password)?;
                println!("Dashboard password updated successfully (stored at {}).", path.display());
                Ok(())
            }
            None => run_dashboard(&host, port, true, public_url.as_deref()),
        },
        Commands::Docs { cmd } => docs_cmd(cmd),
        Commands::Completion { shell } => {
            let mut command = Cli::command();
            let mut out = std::io::stdout();
            match shell.to_ascii_lowercase().as_str() {
                "bash" => generate(Bash, &mut command, "harness", &mut out),
                "zsh" => generate(Zsh, &mut command, "harness", &mut out),
                "pwsh" | "powershell" => generate(PowerShell, &mut command, "harness", &mut out),
                _ => return Err(Error::new("completion shell must be bash, zsh, or pwsh")),
            }
            Ok(())
        }
        Commands::Update => {
            let status = std::process::Command::new("npm").args(["install", "--global", "5harness@latest"]).status()?;
            if status.success() { Ok(()) } else { Err(Error::new(format!("npm update failed with {status}"))) }
        }
        Commands::Upgrade { dir } => {
            let target = dir.path(None, cwd);
            let agents = target.join("AGENTS.md");
            let text = fs::read_to_string(&agents)?;
            let re = regex::Regex::new(r"(?m)^(<!--\s*harness-version:)\s*[^>]+(-->\s*)$").unwrap();
            if !re.is_match(&text) { return Err(Error::new("AGENTS.md has no harness-managed version marker")); }
            let updated = re.replace(&text, format!("$1 {VERSION} $2")).into_owned();
            crate::infra::entities::atomic_write(&agents, &updated)?;
            println!("Upgraded harness block in {}", agents.display());
            Ok(())
        }
        Commands::Reindex { dir } => {
            let target = dir.path(None, cwd);
            let _lock = MutationLock::acquire(&target)?;
            let (path, entities, edges) = write_project_index(&target)?;
            println!("Reindexed {entities} entities, {edges} edges");
            println!("Index: {}", path.display());
            Ok(())
        }
        Commands::Get {
            id_or_path,
            dir,
            summary,
            json,
        } => {
            let target = dir.path(None, cwd);
            let file = get_entity(&target, &id_or_path)?
                .ok_or_else(|| Error::new(format!("Entity not found: {id_or_path}")))?;
            let id = as_string(&file.data, "id").unwrap_or(id_or_path.clone());
            let ty = as_string(&file.data, "type").unwrap_or_default();
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "id": id,
                        "type": ty,
                        "path": file.relative_path,
                        "title": as_string(&file.data, "title"),
                        "status": as_string(&file.data, "status"),
                        "frontmatter": crate::app::durable::fm_json(&file.data),
                        "body": if summary { serde_json::Value::Null } else { serde_json::Value::String(file.body) },
                    })
                );
            } else {
                println!("# {id} ({ty})");
                println!("path: {}", file.relative_path);
                println!("---");
                println!("{}", crate::app::durable::fm_to_yaml(&file.data).trim_end());
                if !summary && !file.body.trim().is_empty() {
                    println!("---");
                    println!("{}", file.body.trim_end());
                }
            }
            Ok(())
        }
        Commands::Search {
            query,
            dir,
            limit,
            ty,
            json,
        } => {
            if query.trim().is_empty() { return Err(Error::new("search query must not be empty")); }
            let target = dir.path(None, cwd);
            let index = ensure_index(&target)?;
            let hits = search_index(&index, &query, limit, ty.as_deref());
            if json {
                println!("{}", serde_json::to_string_pretty(&hits)?);
            } else {
                println!("{}", format_search_hits(&hits));
            }
            Ok(())
        }
        Commands::Links { id, dir, json, broken } => {
            let target = dir.path(None, cwd);
            let index = ensure_index(&target)?;
            let canonical_id = get_entity(&target, &id)?.and_then(|file| as_string(&file.data, "id")).unwrap_or(id.clone());
            let mut view = links_for(&index, &canonical_id);
            if broken {
                if let Some(arr) = view.get("outbound").and_then(|v| v.as_array()).cloned() {
                    let filtered: Vec<_> = arr
                        .into_iter()
                        .filter(|o| o.get("resolved") == Some(&serde_json::Value::Bool(false)))
                        .collect();
                    view["outbound"] = serde_json::Value::Array(filtered);
                }
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&view)?);
            } else {
                println!("{}", format_links_view(&view));
            }
            Ok(())
        }
        Commands::IntakeRun { prompt, summary, json, commit, dir } => {
            let text = prompt.or(summary).unwrap_or_default();
            let plan = serde_json::json!({
                "type": "spec_slice",
                "summary": text,
                "lane": "normal",
            });
            if commit {
                let target = dir.path(None, cwd);
                let (file, id) = add_intake(&target, "spec_slice", &text, "normal", None, None, None, None, None, None)?;
                if json {
                    println!("{}", serde_json::json!({"committed": true, "id": id, "path": file.relative_path, "plan": plan}));
                } else {
                    println!("Intake {id} recorded.\n  file: {}", file.relative_path);
                }
                return Ok(());
            }
            if json {
                println!("{}", serde_json::json!({"committed": false, "plan": plan}));
            } else {
                println!("Suggested intake: spec_slice / normal\n{text}");
            }
            Ok(())
        }
        Commands::Intake {
            dir,
            ty,
            summary,
            lane,
            flags,
            docs,
            story,
            stories,
            notes,
            links,
            cmd,
        } => match cmd {
            Some(IntakeCmd::Update {
                dir,
                id,
                status,
                stories,
                notes,
            }) => {
                let target = dir.path(None, cwd);
                let file = update_intake(
                    &target,
                    &id,
                    status.as_deref(),
                    stories.as_deref(),
                    notes.as_deref(),
                )?;
                println!("Intake {id} updated.");
                println!(
                    "  status: {}",
                    as_string(&file.data, "status").unwrap_or_else(|| "pending".into())
                );
                println!("  file: {}", file.relative_path);
                Ok(())
            }
            Some(IntakeCmd::Close { id, dir, id_flag, notes }) => {
                let id = id.or(id_flag).ok_or_else(|| Error::new("intake close requires an entity id"))?;
                let target = dir.path(None, cwd);
                let file = update_intake(&target, &id, Some("completed"), None, notes.as_deref())?;
                println!("Intake {id} updated.");
                println!("  status: {}", as_string(&file.data, "status").unwrap_or_default());
                println!("  file: {}", file.relative_path);
                Ok(())
            }
            Some(IntakeCmd::Dismiss { id, dir, id_flag, notes }) => {
                let id = id.or(id_flag).ok_or_else(|| Error::new("intake dismiss requires an entity id"))?;
                let target = dir.path(None, cwd);
                let file = update_intake(&target, &id, Some("dismissed"), None, notes.as_deref())?;
                println!("Intake {id} updated.");
                println!("  status: {}", as_string(&file.data, "status").unwrap_or_default());
                println!("  file: {}", file.relative_path);
                Ok(())
            }
            None => {
                let ty = ty.ok_or_else(|| Error::new("intake requires --type, --summary, and --lane"))?;
                let summary = summary.ok_or_else(|| Error::new("intake requires --type, --summary, and --lane"))?;
                let lane = lane.ok_or_else(|| Error::new("intake requires --type, --summary, and --lane"))?;
                let target = dir.path(None, cwd);
                let (file, id) = add_intake(
                    &target,
                    &ty,
                    &summary,
                    &lane,
                    flags.as_deref(),
                    docs.as_deref(),
                    story.as_deref(),
                    stories.as_deref(),
                    notes.as_deref(),
                    links.as_deref(),
                )?;
                println!("Intake {id} recorded.");
                println!("  file: {}", file.relative_path);
                Ok(())
            }
        },
        Commands::Story { cmd } => story_cmd(cmd, cwd),
        Commands::Decision { cmd } => decision_cmd(cmd, cwd),
        Commands::Backlog { cmd } => match cmd {
            BacklogCmd::Add {
                dir,
                title,
                while_text,
                pain,
                suggestion,
                risk,
                predicted,
                notes,
                links,
            } => {
                let target = dir.path(None, cwd);
                let (file, id) = add_backlog(
                    &target,
                    &title,
                    while_text.as_deref(),
                    pain.as_deref(),
                    suggestion.as_deref(),
                    risk.as_deref(),
                    predicted.as_deref(),
                    notes.as_deref(),
                    links.as_deref(),
                )?;
                println!("Backlog {id} added.");
                println!("  file: {}", file.relative_path);
                Ok(())
            }
            BacklogCmd::Close { dir, id, status, outcome } => {
                let target = dir.path(None, cwd);
                let file = close_backlog(&target, &id, status.as_deref(), outcome.as_deref())?;
                println!("Backlog {id} closed.");
                println!("  file: {}", file.relative_path);
                Ok(())
            }
        },
        Commands::Query { cmd } => query_cmd(cmd, cwd),
        Commands::Trace { dir, summary, intake, story, agent, outcome, duration, tokens, actions, files_read, files_changed, decisions, errors, friction, notes, json } => {
            if summary.trim().chars().count() < 10 { return Err(Error::new("trace --summary must be at least 10 characters")); }
            if !["completed", "blocked", "partial", "failed"].contains(&outcome.as_str()) { return Err(Error::new("trace --outcome must be completed, blocked, partial, or failed")); }
            let target = dir.path(None, cwd);
            let record = append_trace(&target, serde_json::json!({
                "task_summary": summary,
                "intake_id": intake,
                "story_id": story,
                "agent": agent.unwrap_or_else(|| "unknown".into()),
                "actions_taken": csv_json(actions),
                "files_read": csv_json(files_read),
                "files_changed": csv_json(files_changed),
                "decisions_made": csv_json(decisions),
                "errors": csv_json(errors),
                "outcome": outcome,
                "duration_seconds": duration,
                "token_estimate": tokens,
                "harness_friction": friction.unwrap_or_else(|| "none".into()),
                "notes": notes,
            }))?;
            let scored = score_trace(&record);
            if json { println!("{}", serde_json::json!({"trace":record,"score":scored})); }
            else { println!("Trace {} recorded ({} tier, score {}).", record["id"].as_str().unwrap_or(""), scored["tier"].as_str().unwrap_or(""), scored["score"].as_u64().unwrap_or(0)); }
            Ok(())
        }
        Commands::ScoreTrace { dir, id, json } => {
            let target = dir.path(None, cwd);
            let record = latest_trace(&target, id.as_deref())?.ok_or_else(|| Error::new("no trace found"))?;
            let scored = score_trace(&record);
            if json { println!("{}", serde_json::to_string_pretty(&serde_json::json!({"trace":record,"score":scored}))?); }
            else { println!("Trace {}: {} (score {})", record["id"].as_str().unwrap_or(""), scored["tier"].as_str().unwrap_or(""), scored["score"].as_u64().unwrap_or(0)); }
            Ok(())
        }
        Commands::Worklog { cmd } => match cmd {
            WorklogCmd::Add { dir, story, summary, pr, commit } => {
                let target = dir.path(None, cwd);
                let record = append_worklog(&target, serde_json::json!({"story":story,"summary":summary,"pr":pr,"commit":commit}))?;
                println!("Worklog {} recorded.", record["id"].as_str().unwrap_or(""));
                Ok(())
            }
            WorklogCmd::List { dir, json } => {
                let records = read_records(&dir.path(None, cwd), "worklog")?;
                if json { println!("{}", serde_json::to_string_pretty(&records)?); }
                else { for record in records { println!("{}  {}  {}", record["id"].as_str().unwrap_or(""), record["story"].as_str().unwrap_or(""), record["summary"].as_str().unwrap_or("")); } }
                Ok(())
            }
            WorklogCmd::FromGit { dir, story, since: _ } => {
                let target = dir.path(None, cwd);
                let commits = git_commits(&target, 20)?;
                for commit in commits {
                    append_worklog(&target, serde_json::json!({"story":story,"summary":commit["summary"],"commit":commit["commit"],"source":"git"}))?;
                }
                println!("Imported git commits into worklog.");
                Ok(())
            }
        },
        Commands::Doctor { dir, json } => {
            let target = dir.path(None, cwd);
            if json {
                println!("{}", serde_json::to_string_pretty(&doctor_json(&target)?)?);
            } else {
                println!("{}", format_doctor(&target)?);
            }
            Ok(())
        }
        Commands::Status { dir, json } => {
            let target = dir.path(None, cwd);
            if json {
                println!("{}", serde_json::to_string_pretty(&status_json(&target)?)?);
            } else {
                println!("{}", format_status(&target)?);
            }
            Ok(())
        }
        Commands::Next { dir, json, limit } => {
            let target = dir.path(None, cwd);
            if json {
                println!("{}", serde_json::to_string_pretty(&next_items(&target, limit)?)?);
            } else {
                let items = next_items(&target, limit)?;
                if items.is_empty() {
                    println!("Next work\n  (no active stories, pending intakes, backlog, or reports)");
                } else {
                    println!("Next work\n{}", items.iter().map(|item| format!("  [{}] {}  {}", item["kind"].as_str().unwrap_or("work"), item["id"].as_str().unwrap_or(""), item["title"].as_str().unwrap_or(""))).collect::<Vec<_>>().join("\n"));
                }
            }
            Ok(())
        }
        Commands::Context { id, dir, json, depth, max_chars } => {
            let target = dir.path(None, cwd);
            let file = get_entity(&target, &id)?
                .ok_or_else(|| Error::new(format!("Entity not found: {id}")))?;
            let depth = depth.unwrap_or(1);
            if depth > 1 { return Err(Error::new("context --depth must be 0 or 1")); }
            let max_chars = max_chars.unwrap_or(12_000).min(100_000);
            let entity_id = as_string(&file.data, "id").unwrap_or(id.clone());
            let index = ensure_index(&target)?;
            let links = crate::app::index::links_for(&index, &entity_id);
            let mut related = Vec::new();
            if depth > 0 {
                for key in ["outbound", "backlinks"] {
                    if let Some(items) = links.get(key).and_then(|v| v.as_array()) {
                        for item in items {
                            if let Some(value) = item.get(if key == "outbound" { "to" } else { "from" }).and_then(|v| v.as_str()) {
                                related.push(value.to_string());
                            }
                        }
                    }
                }
            }
            related.sort();
            related.dedup();
            let body = truncate_chars(&file.body, max_chars);
            if json {
                println!("{}", serde_json::json!({"id": entity_id, "path": file.relative_path, "frontmatter": crate::app::durable::fm_json(&file.data), "body": body, "depth": depth, "max_chars": max_chars, "links": links, "related": related}));
            } else {
                println!("# {entity_id}\npath: {}\n{}\n\nlinks: {}", file.relative_path, body, related.join(", "));
            }
            Ok(())
        }
        Commands::Tool { cmd } => match cmd {
            ToolCmd::Register { dir, name, command, description, responsibility, kind, capability, json } => {
                let target = dir.path(None, cwd);
                let value = upsert_tool(&target, serde_json::json!({"name":name,"command":command,"description":description,"responsibility":responsibility,"kind":kind,"capability":capability,"status":"registered","source":"project"}))?;
                if json { println!("{}", serde_json::to_string_pretty(&value)?); } else { println!("Tool {} registered.", value["name"].as_str().unwrap_or("")); }
                Ok(())
            }
            ToolCmd::Check { dir, name, json } => {
                let target = dir.path(None, cwd);
                let records = read_records(&target, "tools")?;
                let mut checked = Vec::new();
                for mut value in records {
                    if name.as_deref().is_some_and(|filter| value["name"].as_str() != Some(filter)) { continue; }
                    let command = value["command"].as_str().unwrap_or("");
                    let ok = if command.is_empty() { false } else {
                        #[cfg(unix)]
                        { std::process::Command::new("sh").arg("-c").arg(command).status().map(|s| s.success()).unwrap_or(false) }
                        #[cfg(windows)]
                        { std::process::Command::new("cmd").args(["/C", command]).status().map(|s| s.success()).unwrap_or(false) }
                    };
                    if let Some(map) = value.as_object_mut() { map.insert("status".into(), serde_json::json!(if ok { "ok" } else { "failed" })); }
                    let _ = upsert_tool(&target, value.clone())?;
                    checked.push(value);
                }
                if json { println!("{}", serde_json::to_string_pretty(&checked)?); } else { for value in checked { println!("{}: {}", value["name"].as_str().unwrap_or(""), value["status"].as_str().unwrap_or("")); } }
                Ok(())
            }
            ToolCmd::Remove { dir, name, json } => {
                let removed = remove_tool(&dir.path(None, cwd), &name)?;
                if json { println!("{}", serde_json::json!({"name":name,"removed":removed})); } else { println!("{}", if removed { "Tool removed." } else { "Tool not found." }); }
                Ok(())
            }
        },
        Commands::Audit { dir, json } => {
            let target = dir.path(None, cwd);
            let index = ensure_index(&target)?;
            let broken = index.edges.iter().filter(|e| !e.resolved).count();
            let records = read_records(&target, "traces")?;
            let entropy = ((broken * 10) + if index.catalog.is_empty() { 10 } else { 0 }).min(100);
            let value = serde_json::json!({"project":target,"findings": if broken > 0 { vec![serde_json::json!({"kind":"broken_link","count":broken})] } else { Vec::<serde_json::Value>::new() },"entropy":entropy,"traces":records.len()});
            if json { println!("{}", serde_json::to_string_pretty(&value)?); } else { println!("Audit entropy: {entropy}/100\nBroken links: {broken}\nTraces: {}", records.len()); }
            Ok(())
        }
        Commands::Propose { dir, commit } => {
            let target = dir.path(None, cwd);
            let index = ensure_index(&target)?;
            let broken = index.edges.iter().filter(|e| !e.resolved).count();
            if broken == 0 { println!("No new proposals."); return Ok(()); }
            let title = format!("Resolve {broken} broken entity links");
            if commit {
                let (file, id) = add_backlog(&target, &title, Some("audit"), Some("Broken durable links reduce agent context reliability"), Some("Resolve or remove broken links"), Some("normal"), None, None, None)?;
                println!("Proposal {id} recorded in {}.", file.relative_path);
            } else { println!("Proposal: {title}\nRun harness propose --commit to record it."); }
            Ok(())
        }
        Commands::Export { cmd: ExportCmd::Changelog { dir, since: _, json } } => {
            let target = dir.path(None, cwd);
            let cat = crate::app::catalog::build_catalog(&target)?;
            let entries: Vec<_> = crate::app::catalog::by_type(&cat, "story").into_iter().filter(|e| e.status == "implemented").map(|e| format!("- {}: {}", e.id, e.title)).collect();
            let text = if entries.is_empty() { "No implemented stories.".into() } else { entries.join("\n") };
            if json {
                println!("{}", serde_json::json!({"changelog": text, "root": target}));
            } else {
                println!("{text}");
            }
            Ok(())
        }
        Commands::Watch { dir } => {
            let target = dir.path(None, cwd);
            println!("Watching entity directories under {} (Ctrl+C to stop).", target.display());
            let mut last = entity_mtime_fingerprint(&target);
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let current = entity_mtime_fingerprint(&target);
                if current != last {
                    let _lock = MutationLock::acquire(&target)?;
                    write_project_index(&target)?;
                    println!("Reindexed after markdown change.");
                    last = current;
                }
            }
        }
        Commands::Handoff { dir, json, .. } => {
            let target = dir.path(None, cwd);
            if json {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"status":status_json(&target)?,"next":next_items(&target, Some(10))?,"traces":read_records(&target,"traces")?.into_iter().rev().take(5).collect::<Vec<_>>(),"worklog":read_records(&target,"worklog")?.into_iter().rev().take(5).collect::<Vec<_>>()}))?);
            } else {
                println!("{}", format_handoff(&target)?);
            }
            Ok(())
        }
        Commands::Mcp { dir, port, host, public_url, token } => {
            let target = dir.path(None, cwd);
            let dash = crate::app::mcp::start_mcp(&host, port, target, false, public_url.as_deref(), token)?;
            println!("Harness MCP");
            println!("  {}", dash.url);
            println!("  MCP: {}mcp", dash.url);
            if let Some(token) = &dash.auth_token {
                println!("  Bearer token: {token}");
            }
            println!("Press Ctrl+C to stop.");
            loop {
                std::thread::sleep(std::time::Duration::from_secs(60));
            }
        }
    }
}

fn story_cmd(cmd: StoryCmd, cwd: &Path) -> Result<()> {
    match cmd {
        StoryCmd::Add {
            dir,
            id,
            title,
            lane,
            contract,
            verify,
            notes,
            links,
        } => {
            let target = dir.path(None, cwd);
            let file = add_story(
                &target,
                &id,
                &title,
                &lane,
                contract.as_deref(),
                verify.as_deref(),
                notes.as_deref(),
                links.as_deref(),
            )?;
            println!("Story {id} added.");
            println!("  file: {}", file.relative_path);
            Ok(())
        }
        StoryCmd::Update {
            dir,
            id,
            status,
            evidence,
            unit,
            integration,
            e2e,
            platform,
            verify,
            title,
            contract,
            notes,
            links,
        } => {
            let target = dir.path(None, cwd);
            let file = update_story(
                &target,
                StoryUpdate {
                    id: id.clone(),
                    status,
                    evidence,
                    unit,
                    integration,
                    e2e,
                    platform,
                    verify,
                    title,
                    notes,
                    contract,
                    links,
                },
            )?;
            println!("Story {id} updated.");
            println!("  file: {}", file.relative_path);
            Ok(())
        }
        StoryCmd::Start {
            id,
            dir,
            id_flag,
            evidence,
        } => lifecycle(
            cwd,
            &dir,
            id.or(id_flag),
            "in_progress",
            "started",
            evidence,
            None,
        ),
        StoryCmd::Done {
            id,
            dir,
            id_flag,
            evidence,
        } => lifecycle(
            cwd,
            &dir,
            id.or(id_flag),
            "implemented",
            "done",
            evidence,
            None,
        ),
        StoryCmd::Block {
            id,
            dir,
            id_flag,
            reason,
        } => lifecycle(
            cwd,
            &dir,
            id.or(id_flag),
            "blocked",
            "blocked",
            None,
            reason,
        ),
        StoryCmd::Verify {
            id,
            dir,
            id_flag,
            allow_project_command,
        } => {
            let id = id
                .or(id_flag)
                .ok_or_else(|| Error::new("story verify requires an entity id"))?;
            let target = dir.path(None, cwd);
            let file = get_entity(&target, &id)?
                .ok_or_else(|| Error::new(format!("Story {id} not found")))?;
            let command = as_string(&file.data, "verify")
                .ok_or_else(|| Error::new(format!("Story {id} has no verify command")))?;
            require_project_command_trust(
                &id,
                &file.relative_path,
                &command,
                allow_project_command,
            )?;
            let (passed, output) = run_verify_command(&command, &target);
            record_story_verification(&target, &id, passed, &output)?;
            println!(
                "Story {id} verification: {}",
                if passed { "passed" } else { "failed" }
            );
            if passed {
                Ok(())
            } else {
                Err(Error::new(format!("story verification failed: {output}")))
            }
        }
        StoryCmd::VerifyAll {
            dir,
            allow_project_command,
        } => {
            let target = dir.path(None, cwd);
            let files = crate::infra::entities::list_entity_files(&target, "story")?;
            if !allow_project_command {
                if let Some(file) = files
                    .iter()
                    .find(|file| as_string(&file.data, "verify").is_some())
                {
                    let id = as_string(&file.data, "id").unwrap_or_else(|| "<unknown>".into());
                    let command = as_string(&file.data, "verify").unwrap_or_default();
                    require_project_command_trust(&id, &file.relative_path, &command, false)?;
                }
            }
            let mut failures = 0;
            for file in files {
                let Some(id) = as_string(&file.data, "id") else {
                    continue;
                };
                let Some(command) = as_string(&file.data, "verify") else {
                    continue;
                };
                let (passed, output) = run_verify_command(&command, &target);
                record_story_verification(&target, &id, passed, &output)?;
                println!("Story {id}: {}", if passed { "passed" } else { "failed" });
                if !passed {
                    failures += 1;
                }
            }
            if failures == 0 {
                Ok(())
            } else {
                Err(Error::new(format!("{failures} story verifications failed")))
            }
        }
    }
}

fn report_cmd(cmd: ReportCmd, cwd: &Path) -> Result<()> {
    match cmd {
        ReportCmd::Add { dir, to, summary } => {
            let target = dir.path(None, cwd);
            let peer_root = project_link::resolve_peer(&target, Some(&to), None)?;
            project_link::ensure_peer_write_allowed(&peer_root)?;
            let from = read_project_id(&target).ok();
            let (file, id) = add_report(&peer_root, &summary, None, from.as_deref(), None)?;
            println!(
                "Report {id} added to {}.\n  file: {}",
                peer_root.display(),
                file.relative_path
            );
            Ok(())
        }
        ReportCmd::List { dir, json, status } => {
            let target = dir.path(None, cwd);
            let mut values = Vec::new();
            for file in crate::infra::entities::list_entity_files(&target, "report")? {
                let current = as_string(&file.data, "status").unwrap_or_default();
                if status.as_deref().is_some_and(|filter| filter != current) {
                    continue;
                }
                values.push(serde_json::json!({
                    "id": as_string(&file.data, "id"),
                    "status": current,
                    "severity": as_string(&file.data, "severity"),
                    "summary": as_string(&file.data, "summary"),
                    "updated_at": as_string(&file.data, "updated_at"),
                    "path": file.relative_path,
                }));
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&values)?);
            } else {
                for value in values {
                    println!(
                        "{}  {}  {}",
                        value["id"].as_str().unwrap_or(""),
                        value["status"].as_str().unwrap_or(""),
                        value["summary"].as_str().unwrap_or("")
                    );
                }
            }
            Ok(())
        }
        ReportCmd::Get { id, dir, from } => {
            let target = dir.path(None, cwd);
            let root = if let Some(selector) = from {
                project_link::resolve_peer(&target, Some(&selector), None)?
            } else {
                target
            };
            let file = get_entity(&root, &id)?
                .ok_or_else(|| Error::new(format!("Report {id} not found")))?;
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({"id":id,"path":file.relative_path,"frontmatter":crate::app::durable::fm_json(&file.data),"body":file.body})
                )?
            );
            Ok(())
        }
        ReportCmd::Update {
            dir,
            id,
            status,
            resolution,
            related,
        } => {
            let target = dir.path(None, cwd);
            let file = update_report(
                &target,
                &id,
                &status,
                resolution.as_deref(),
                related.as_deref(),
            )?;
            println!(
                "Report {id} updated.\n  status: {}",
                as_string(&file.data, "status").unwrap_or_default()
            );
            Ok(())
        }
    }
}

fn peer_cmd(cmd: PeerCmd, cwd: &Path) -> Result<()> {
    match cmd {
        PeerCmd::Search {
            query,
            dir,
            peer,
            role,
            limit,
            json,
        } => {
            let local = dir.path(None, cwd);
            let root = project_link::resolve_peer(&local, peer.as_deref(), role.as_deref())?;
            let index = ensure_index(&root)?;
            let hits = search_index(&index, &query, limit, None);
            if json {
                println!("{}", serde_json::to_string_pretty(&hits)?);
            } else {
                println!("{}", format_search_hits(&hits));
            }
            Ok(())
        }
        PeerCmd::Get {
            id_or_path,
            dir,
            peer,
            role,
            summary,
            json,
        } => {
            let local = dir.path(None, cwd);
            let root = project_link::resolve_peer(&local, peer.as_deref(), role.as_deref())?;
            let file = get_entity(&root, &id_or_path)?
                .ok_or_else(|| Error::new(format!("Peer entity not found: {id_or_path}")))?;
            let value = serde_json::json!({"id":as_string(&file.data,"id").unwrap_or(id_or_path),"path":file.relative_path,"frontmatter":crate::app::durable::fm_json(&file.data),"body":if summary { serde_json::Value::Null } else { serde_json::Value::String(file.body) }});
            if json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", value["body"].as_str().unwrap_or(""));
            }
            Ok(())
        }
        PeerCmd::Context {
            id,
            dir,
            peer,
            role,
            depth,
            max_chars,
            json,
        } => {
            if depth.unwrap_or(1) > 1 {
                return Err(Error::new("peer context --depth must be 0 or 1"));
            }
            let local = dir.path(None, cwd);
            let root = project_link::resolve_peer(&local, peer.as_deref(), role.as_deref())?;
            let file = get_entity(&root, &id)?
                .ok_or_else(|| Error::new(format!("Peer entity not found: {id}")))?;
            let index = ensure_index(&root)?;
            let entity_id = as_string(&file.data, "id").unwrap_or(id);
            let body = truncate_chars(&file.body, max_chars.unwrap_or(12_000).min(100_000));
            let value = serde_json::json!({"id":entity_id,"path":file.relative_path,"frontmatter":crate::app::durable::fm_json(&file.data),"body":body,"links":crate::app::index::links_for(&index,&entity_id)});
            if json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", body);
            }
            Ok(())
        }
        PeerCmd::Links {
            id,
            dir,
            peer,
            role,
            json,
        } => {
            let local = dir.path(None, cwd);
            let root = project_link::resolve_peer(&local, peer.as_deref(), role.as_deref())?;
            let index = ensure_index(&root)?;
            let value = crate::app::index::links_for(&index, &id);
            if json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("{}", crate::app::index::format_links_view(&value));
            }
            Ok(())
        }
    }
}

fn lifecycle(
    cwd: &Path,
    dir: &DirOpts,
    id: Option<String>,
    status: &str,
    verb: &str,
    evidence: Option<String>,
    reason: Option<String>,
) -> Result<()> {
    let id = id.ok_or_else(|| {
        Error::new(format!(
            "story {verb} requires an entity id (positional <id> or --id <id>)"
        ))
    })?;
    let target = dir.path(None, cwd);
    let file = update_story(
        &target,
        StoryUpdate {
            id: id.clone(),
            status: Some(status.into()),
            evidence: evidence.clone(),
            unit: None,
            integration: None,
            e2e: None,
            platform: None,
            verify: None,
            title: None,
            notes: reason,
            contract: None,
            links: None,
        },
    )?;
    println!("Story {id} {verb}.");
    println!("  status: {status}");
    println!("  file: {}", file.relative_path);
    Ok(())
}

fn decision_cmd(cmd: DecisionCmd, cwd: &Path) -> Result<()> {
    match cmd {
        DecisionCmd::Add {
            dir,
            id,
            title,
            status,
            doc,
            verify,
            notes,
            links,
            force,
        } => {
            let target = dir.path(None, cwd);
            let file = add_decision(
                &target,
                &id,
                &title,
                status.as_deref(),
                doc.as_deref(),
                verify.as_deref(),
                notes.as_deref(),
                links.as_deref(),
                force,
            )?;
            println!("Decision {id} added.");
            println!("  file: {}", file.relative_path);
            Ok(())
        }
        DecisionCmd::Update {
            dir,
            id,
            title,
            status,
            doc,
            verify,
            notes,
            links,
        } => {
            let target = dir.path(None, cwd);
            let file = update_decision(
                &target,
                &id,
                title.as_deref(),
                status.as_deref(),
                doc.as_deref(),
                verify.as_deref(),
                notes.as_deref(),
                links.as_deref(),
            )?;
            println!("Decision {id} updated.");
            println!("  file: {}", file.relative_path);
            Ok(())
        }
        DecisionCmd::Verify {
            id,
            dir,
            id_flag,
            allow_project_command,
        } => {
            let id = id
                .or(id_flag)
                .ok_or_else(|| Error::new("decision verify requires an entity id"))?;
            let target = dir.path(None, cwd);
            let file = get_entity(&target, &id)?
                .ok_or_else(|| Error::new(format!("Decision {id} not found")))?;
            let command = as_string(&file.data, "verify")
                .ok_or_else(|| Error::new(format!("Decision {id} has no verify command")))?;
            require_project_command_trust(
                &id,
                &file.relative_path,
                &command,
                allow_project_command,
            )?;
            let (passed, output) = run_verify_command(&command, &target);
            record_decision_verification(&target, &id, passed, &output)?;
            println!(
                "Decision {id} verification: {}",
                if passed { "passed" } else { "failed" }
            );
            if passed {
                Ok(())
            } else {
                Err(Error::new(format!(
                    "decision verification failed: {output}"
                )))
            }
        }
    }
}

fn query_cmd(cmd: QueryCmd, cwd: &Path) -> Result<()> {
    let (view, dir, json, numeric, open, closed) = match cmd {
        QueryCmd::Matrix { dir, numeric, json } => ("matrix", dir, json, numeric, false, false),
        QueryCmd::Stats { dir, json } => ("stats", dir, json, false, false, false),
        QueryCmd::Intakes { dir, json } => ("intakes", dir, json, false, false, false),
        QueryCmd::Decisions { dir, json } => ("decisions", dir, json, false, false, false),
        QueryCmd::Stories { dir, json } => ("stories", dir, json, false, false, false),
        QueryCmd::Backlog {
            dir,
            open,
            closed,
            json,
        } => ("backlog", dir, json, false, open, closed),
        QueryCmd::Traces { dir, json } => ("traces", dir, json, false, false, false),
        QueryCmd::Reports { dir, json } => ("reports", dir, json, false, false, false),
        QueryCmd::Tools { dir, json } => ("tools", dir, json, false, false, false),
    };
    let target = dir.path(None, cwd);
    if json {
        let mut value = query_view_json(&target, view)?;
        if view == "backlog" && (open || closed) {
            if let Some(items) = value.as_array_mut() {
                let wanted_open = open && !closed;
                items.retain(|item| {
                    let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("");
                    if wanted_open {
                        status == "proposed" || status == "accepted"
                    } else {
                        status == "implemented" || status == "rejected"
                    }
                });
            }
        }
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("{}", query_view(&target, view, numeric, open, closed)?);
    }
    Ok(())
}

fn docs_cmd(cmd: DocsCmd) -> Result<()> {
    let root = crate::infra::package_root::resolve_package_root()?;
    let docs = root.join("docs");
    match cmd {
        DocsCmd::List { json } => {
            let mut files = Vec::new();
            if docs.is_dir() {
                collect_md(&docs, &docs, &mut files);
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&files)?);
            } else {
                for f in files {
                    println!("{f}");
                }
            }
        }
        DocsCmd::Search { query, json } => {
            let mut hits = Vec::new();
            let mut files = Vec::new();
            collect_md(&docs, &docs, &mut files);
            let q = query.to_ascii_lowercase();
            for f in files {
                let path = docs.join(&f);
                if let Ok(text) = fs::read_to_string(&path) {
                    if text.to_ascii_lowercase().contains(&q) {
                        hits.push(f);
                    }
                }
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&hits)?);
            } else {
                for h in hits {
                    println!("{h}");
                }
            }
        }
        DocsCmd::Read { path, json } => {
            let root = docs
                .canonicalize()
                .map_err(|e| Error::new(format!("docs directory unavailable: {e}")))?;
            let relative = crate::infra::entities::safe_relative_path(&path)?;
            let full = root.join(&relative);
            let canonical = full
                .canonicalize()
                .map_err(|e| Error::new(format!("documentation file not found: {path}: {e}")))?;
            if !canonical.starts_with(&root) {
                return Err(Error::new(format!(
                    "Documentation path escapes package docs: {path}"
                )));
            }
            let text = fs::read_to_string(&full)?;
            if json {
                println!("{}", serde_json::json!({"path": relative, "body": text}));
            } else {
                print!("{text}");
            }
        }
    }
    Ok(())
}

fn collect_md(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let Ok(file_type) = e.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_md(root, &p, out);
        } else if p.extension().and_then(|s| s.to_str()) == Some("md") {
            if let Ok(rel) = p.strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let all: Vec<char> = text.chars().collect();
    if all.len() <= max_chars {
        return text.to_string();
    }
    let marker: Vec<char> = "… [truncated]".chars().collect();
    if max_chars <= marker.len() {
        return marker.into_iter().take(max_chars).collect();
    }
    let mut output: String = all.into_iter().take(max_chars - marker.len()).collect();
    output.extend(marker);
    output
}

fn csv_json(value: Option<String>) -> serde_json::Value {
    serde_json::Value::Array(
        value
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| serde_json::Value::String(v.to_string()))
            .collect(),
    )
}

fn require_project_command_trust(
    id: &str,
    relative_path: &str,
    command: &str,
    allow_project_command: bool,
) -> Result<()> {
    crate::app::durable::validate_verify_command_for_cli(command)?;
    if allow_project_command {
        return Ok(());
    }
    let command = crate::error::redact_sensitive(command);
    Err(Error::new(format!(
        "refusing to execute project-authored verify command for {id} ({relative_path}): {command}\nrerun with --allow-project-command after reviewing the command"
    )))
}

fn run_verify_command(command: &str, project_root: &Path) -> (bool, String) {
    #[cfg(unix)]
    let result = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(project_root)
        .output();
    #[cfg(windows)]
    let result = std::process::Command::new("cmd")
        .args(["/C", command])
        .current_dir(project_root)
        .output();
    match result {
        Ok(output) => {
            let mut text = String::from_utf8_lossy(&output.stdout).to_string();
            text.push_str(&String::from_utf8_lossy(&output.stderr));
            let text = crate::error::redact_sensitive(&text);
            (
                output.status.success(),
                text.trim().chars().take(2_000).collect(),
            )
        }
        Err(err) => (false, crate::error::redact_sensitive(&err.to_string())),
    }
}

fn entity_mtime_fingerprint(project_root: &Path) -> u128 {
    let mut latest = 0u128;
    for ty in crate::domain::entities::ENTITY_TYPES {
        let dir = project_root.join(crate::domain::entities::entity_dir(ty).unwrap_or(""));
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if let Ok(modified) = entry.metadata().and_then(|m| m.modified()) {
                if let Ok(age) = modified.duration_since(std::time::UNIX_EPOCH) {
                    latest = latest.max(age.as_nanos());
                }
            }
        }
    }
    latest
}

fn run_dashboard(host: &str, port: u16, forever: bool, public_url: Option<&str>) -> Result<()> {
    let dash = crate::app::dashboard::start_dashboard(host, port, false, public_url)?;
    println!("Harness dashboard");
    println!("  {}", dash.url);
    println!("  MCP: {}mcp", dash.url);
    println!("  API: {}api/projects", dash.url);
    println!("Press Ctrl+C to stop.");
    if forever {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
    }
    let _ = dash;
    Ok(())
}
