use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "claudius",
    about = "AI agent configuration management tool - Manage MCP servers, settings, skills, and project instructions",
    long_about = "Claudius is a configuration management tool for Claude Code, Codex, Gemini, and legacy Claude Desktop targets.

It helps you:
  • Manage MCP (Model Context Protocol) server configurations
  • Maintain agent settings across projects
  • Manage agent skills and Claude Code subagents
  • Define project-specific instructions via agent-specific context files

Claude Desktop support is retained as a legacy / best-effort MCP target.
Prefer Claude Code, Codex, or Gemini when you need actively managed surfaces.

    Configuration files are stored in:
      • $XDG_CONFIG_HOME/claudius/ (or ~/.config/claudius/)
        - mcpServers.json: MCP server definitions
        - claude.settings.json: Claude/Claude Code settings (optional)
        - codex.settings.toml: Codex settings (optional)
        - codex.requirements.toml: Codex requirements (admin-enforced, optional)
        - codex.managed_config.toml: Codex managed defaults (admin-managed, optional)
        - gemini.settings.json: Gemini settings (optional)
        - gemini.system_defaults.json: Gemini CLI system defaults (optional)
        - settings.json: Legacy Claude settings (backward compatible)
        - skills/: Shared and agent-specific skills (directories with SKILL.md)
        - commands/gemini/: Gemini custom commands (*.toml)
        - agents/gemini/: Gemini custom agents (*.md)
        - agents/claude-code/: Claude Code subagents (*.md)
        - rules/: Agent context templates (*.md)

Target files:
  • ./.mcp.json (MCP servers in project-local mode, default)
  • ./.claude/settings.json (Claude Code settings in project-local mode)
  • ./.gemini/settings.json (Gemini project-local config)
  • $XDG_CONFIG_HOME/Claude/claude_desktop_config.json (Claude Desktop legacy/best-effort global MCP target)
  • ~/.claude.json + ~/.claude/settings.json (Claude Code global config)
  • System-level managed-settings.json / managed-mcp.json (Claude Code managed scope)
  • ~/.codex/config.toml (Codex global config)
      • /etc/codex/requirements.toml (Codex requirements, admin-enforced)
      • /etc/codex/managed_config.toml (Codex managed defaults)
      • ~/.gemini/settings.json (Gemini global config)
      • /etc/gemini-cli/settings.json (Gemini CLI system settings)
      • /etc/gemini-cli/system-defaults.json (Gemini CLI system defaults)
      • ./CLAUDE.md / ./GEMINI.md / ./AGENTS.md (project instructions)",
    version,
    author
)]
pub struct Cli {
    /// List all available subcommands and exit
    #[arg(
        long,
        global = true,
        help = "List all available top-level commands and their subcommands"
    )]
    pub list_commands: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Enable debug output (shows INFO and DEBUG messages)
    #[arg(long, global = true)]
    pub debug: bool,

    /// Enable trace output (shows all log messages including TRACE)
    #[arg(short = 't', long, global = true)]
    pub trace: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Manage configuration files (initialization and synchronization)
    #[command(subcommand)]
    Config(ConfigCommands),

    /// Manage agent skills
    #[command(subcommand, name = "skills")]
    Skills(SkillsCommands),

    /// Manage project context rules and templates
    #[command(subcommand)]
    Context(ContextCommands),

    /// Execute processes with automatic secret resolution
    #[command(subcommand)]
    Secrets(SecretsCommands),
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Bootstrap Claudius configuration directory with default files
    #[command(long_about = "Bootstrap Claudius configuration directory with default files.

    This command creates the following structure in $XDG_CONFIG_HOME/claudius/:
      • mcpServers.json - MCP server configuration template
      • claude.settings.json - Claude/Claude Code settings template
      • codex.settings.toml - Codex settings template
      • codex.requirements.toml - Codex requirements template (admin-enforced)
      • codex.managed_config.toml - Codex managed defaults template (admin-managed)
      • gemini.settings.json - Gemini settings template
      • gemini.system_defaults.json - Gemini CLI system defaults template
      • settings.json - Legacy Claude settings (backward compatible)
      • skills/ - Directory for shared and agent-specific skills
      • commands/gemini/ - Gemini custom commands (*.toml)
      • agents/gemini/ - Gemini custom agents (*.md)
      • agents/claude-code/ - Claude Code subagents (*.md)
      • rules/ - Directory for agent context rules

By default, existing files are preserved. Use --force to reinitialize.

Examples:
  # Initialize configuration (preserves existing files)
  claudius config init

  # Force reinitialize (resets to defaults)
  claudius config init --force")]
    Init(InitArgs),

    /// Synchronize configurations to project or global targets
    #[command(long_about = "Synchronize agent configurations to the target files.

This command:
  1. Reads mcpServers.json for MCP server definitions
     2. Reads agent settings (if present):
        - claude.settings.json (or legacy settings.json)
        - codex.settings.toml
        - codex.requirements.toml (optional; used with --codex-requirements)
        - codex.managed_config.toml (optional; used with --codex-managed-config)
        - gemini.settings.json
        - gemini.system_defaults.json (optional; used with --gemini-system-defaults)
  3. Writes configurations to:
     - Project-local mode (default):
       • Claude (`--agent claude`): ./.mcp.json (legacy / best-effort Desktop-compatible MCP target)
       • Claude Code (`--agent claude-code`): ./.mcp.json (MCP servers) + ./.claude/settings.json (settings)
       • Claude Code local scope (--scope local): ~/.claude.json (per-project MCP) + ./.claude/settings.local.json
       • Codex: ./.codex/config.toml
       • Gemini: ./.gemini/settings.json
     - Global mode (--global):
       • Claude Desktop (`--agent claude`, legacy / best-effort): $XDG_CONFIG_HOME/Claude/claude_desktop_config.json
       • Claude Code: ~/.claude.json + ~/.claude/settings.json
       • Claude Code managed scope (--scope managed): managed-settings.json + managed-mcp.json (system dirs)
       • Codex: ~/.codex/config.toml
       • Gemini: ~/.gemini/settings.json
       • Gemini system settings (--gemini-system): /etc/gemini-cli/settings.json
       • Gemini system defaults (--gemini-system-defaults): /etc/gemini-cli/system-defaults.json
     4. Syncs auxiliary agent content when present:
       - skills/ -> agent skills directories
       - commands/gemini/ -> .gemini/commands
       - agents/gemini/ -> .gemini/agents
       - agents/claude-code/ -> .claude/agents
       - Codex skills stay explicit via `claudius skills sync --agent codex`

Note: `--agent claude` is retained for legacy Claude Desktop JSON workflows.
For actively managed CLI surfaces, prefer `claude-code`, `codex`, or `gemini`.

Examples:
  # Basic sync to project-local files
  claudius config sync

  # Sync Claude Code global configuration
  claudius config sync --global --agent claude-code

  # Sync legacy Claude Desktop global MCP configuration
  claudius config sync --global --agent claude

  # Preview changes without writing
  claudius config sync --dry-run

  # Create backup before syncing
  claudius config sync --backup")]
    Sync(ConfigSyncArgs),

    /// Validate configuration source files without writing anything
    #[command(
        long_about = "Validate Claudius configuration source files without writing anything.

    This command checks:
      • mcpServers.json (required) - MCP server definitions
      • claude.settings.json / settings.json (optional) - Claude/Claude Code settings
      • codex.settings.toml (optional) - Codex settings
      • codex.requirements.toml (optional) - Codex admin-enforced requirements
      • codex.managed_config.toml (optional) - Codex admin-managed defaults
      • gemini.settings.json (optional) - Gemini settings
      • gemini.system_defaults.json (optional) - Gemini CLI system defaults
      • commands/gemini/*.toml (optional) - Gemini custom commands
      • agents/gemini/*.md (optional) - Gemini custom agents
      • agents/claude-code/*.md (optional) - Claude Code subagent definitions

Use --agent to validate a specific agent's settings.
Use --strict to fail on warnings."
    )]
    Validate(ConfigValidateArgs),

    /// Inspect configuration health, lifecycle risks, and unmanaged surfaces
    #[command(
        long_about = "Inspect Claudius configuration health, lifecycle risks, and unmanaged surfaces.

This command scans the Claudius source tree and the current deployment context
to report situations such as:
  • legacy settings.json still in use
  • legacy commands/*.md fallback still present
  • Claude Desktop JSON targets being used as legacy / best-effort surfaces
  • unmanaged Claude Code slash commands in .claude/commands
  • unmanaged Gemini extensions in the selected deployment context
  • legacy Codex compatibility skill targets in .codex/skills
  • stale deployed assets that no longer exist in the source tree

Use --global to inspect global deployment targets under $HOME.
Use --agent to focus on a single agent surface."
    )]
    Doctor(ConfigDoctorArgs),
}

#[derive(Subcommand, Debug)]
pub enum SkillsCommands {
    /// Synchronize skills into agent directories
    #[command(long_about = "Synchronize skills into agent directories.

This command copies skills from your skills/ directory into \
the agent's skills directory, ensuring all skills are up to date.

Choose the Codex target behavior in $XDG_CONFIG_HOME/claudius/config.toml:

  [codex]
  skill-target = \"auto\"   # auto | agents | both | codex

`auto` publishes to the official .agents/skills path.
Use `both` only if you still need compatibility copies in .codex/skills.")]
    Sync(SkillsSyncArgs),

    /// Validate canonical and legacy skills without deploying them
    #[command(long_about = "Validate Claudius skills without writing deployment targets.

This command:
  • loads shared, legacy, and agent-specific skill sources
  • validates canonical skill.yaml definitions and required files
  • renders the selected agent view to catch schema/rendering failures early
  • warns about deprecated full override directories and metadata that will be dropped

Examples:
  claudius skills validate
  claudius skills validate --agent codex
  claudius skills validate --strict")]
    Validate(SkillsValidateArgs),

    /// Render skills for an agent into a directory for inspection or tests
    #[command(
        long_about = "Render skills for an agent into a directory without touching native agent locations.

This command is useful for debugging render output, golden tests, and schema review.

Examples:
  claudius skills render --agent claude-code --output /tmp/claude-skills
  claudius skills render --agent codex --output /tmp/codex-skills --prune"
    )]
    Render(SkillsRenderArgs),
}

#[derive(Subcommand, Debug)]
pub enum ContextCommands {
    /// Append rules or templates into agent-specific context files
    #[command(
        name = "append",
        long_about = "Append context to agent-specific context files.

	Each agent uses a different context file:
	  • Claude / Claude Code: CLAUDE.md
	  • Gemini: GEMINI.md
	  • Codex: AGENTS.md

This command can:
  • Append predefined rules from your rules directory
  • Append custom templates from any file
  • Automatically select the right file based on agent

Rules are stored in: $XDG_CONFIG_HOME/claudius/rules/*.md

Examples:
  # Append a predefined rule to current agent's context file
  claudius context append security

  # Append to a specific directory
  claudius context append testing --path /path/to/project

  # Use a custom template file
  claudius context append --template-path ./my-template.md

  # Append to global context file in home directory
  claudius context append security --global

  # Append a rule to a specific agent's global file
  claudius context append security --global --agent gemini"
    )]
    Append(AppendContextArgs),

    /// Install rules into project-local directories with include directives
    #[command(
        name = "install",
        long_about = "Install context rules to project-local .agents/rules directory.

	This command:
	  • Copies specified rules from your rules directory to ./.agents/rules/ (default)
	  • Adds a reference directive to the current agent context file to include all rules
	  • The directive is idempotent - it won't be added if already present

This approach keeps context files compact while allowing you to include many rules.

Rules are stored in: $XDG_CONFIG_HOME/claudius/rules/*.md

Examples:
  # Install specific rules (to ./.agents/rules/)
  claudius context install security testing performance

  # Install ALL rules from rules directory
  claudius context install --all

  # Install rules to a specific project directory
  claudius context install security --path /path/to/project

  # Install rules with a custom install directory
  claudius context install security --install-dir ./.claude/rules

  # Install rules with a custom agent
  claudius context install security --agent gemini"
    )]
    Install(InstallContextArgs),

    /// List available rules and templates
    #[command(
        long_about = "List all available context rules and templates in the rules directory."
    )]
    List(ContextListArgs),
}

#[derive(Subcommand, Debug)]
pub enum SecretsCommands {
    /// Run a process with secrets resolved from environment variables
    #[command(
        name = "run",
        long_about = "Run a command with resolved secrets from environment variables.

This command:
  1. Loads the Claudius configuration (if present)
  2. Resolves CLAUDIUS_SECRET_* environment variables using the configured secret manager
  3. Injects resolved values as environment variables (without CLAUDIUS_SECRET_ prefix)
  4. Executes the specified command with full stdio binding

The command inherits all stdio streams, allowing for:
  • Interactive prompts and user input
  • Real-time output streaming
  • Proper signal handling (e.g., Ctrl+C)

Examples:
  # Run a command with resolved secrets
  CLAUDIUS_SECRET_API_KEY=op://vault/item/field claudius secrets run -- npm start

  # Run an interactive command
  CLAUDIUS_SECRET_DB_PASSWORD=op://vault/db/password claudius secrets run -- psql -U admin

  # Run a long-running process
  CLAUDIUS_SECRET_TOKEN=op://vault/tokens/github claudius secrets run -- ./server.sh

Note: Everything after '--' is treated as the command and its arguments."
    )]
    Run(RunArgs),
}

#[derive(Args, Debug, Clone, Copy)]
pub struct InitArgs {
    /// Force reinitialization (removes existing configurations)
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ConfigSyncArgs {
    /// Path to the MCP servers configuration file
    #[arg(short, long, env = "CLAUDIUS_CONFIG", value_hint = clap::ValueHint::FilePath)]
    pub config: Option<PathBuf>,

    /// Preview changes without writing them
    #[arg(short, long, help = "Preview changes without writing them")]
    pub dry_run: bool,

    /// Create timestamped backup before making changes
    #[arg(
        short,
        long,
        help = "Create timestamped backup of target configuration before making changes"
    )]
    pub backup: bool,

    /// Remove stale auxiliary files that Claudius previously deployed
    #[arg(
        long,
        help = "Remove stale skills, commands, and subagents previously deployed by Claudius"
    )]
    pub prune: bool,

    /// Override target configuration file path
    #[arg(short = 'T', long, env = "TARGET_CONFIG_PATH", value_hint = clap::ValueHint::FilePath)]
    pub target_config: Option<PathBuf>,

    /// Target agent global configuration instead of project-local files
    #[arg(
        short,
        long,
        help = "Target agent global configuration instead of project-local files (.mcp.json, .claude/settings.json)"
    )]
    pub global: bool,

    /// Specify the agent to use (overrides config file)
    #[arg(
        short,
        long,
        value_enum,
        help = "Agent to use: claude (legacy/best-effort Desktop target), claude-code, codex, or gemini"
    )]
    pub agent: Option<crate::app_config::Agent>,

    /// Claude Code configuration scope (only valid with --agent claude-code)
    #[arg(long, value_enum, help = "Claude Code scope: managed, user, project, or local")]
    pub scope: Option<crate::app_config::ClaudeCodeScope>,

    /// Also sync Codex admin-enforced requirements.toml (global Codex only)
    #[arg(
        long,
        help = "Also sync /etc/codex/requirements.toml (admin-enforced; global Codex only)"
    )]
    pub codex_requirements: bool,

    /// Also sync Codex managed defaults (global Codex only)
    #[arg(
        long,
        help = "Also sync /etc/codex/managed_config.toml (managed defaults; global Codex only)"
    )]
    pub codex_managed_config: bool,

    /// Target Gemini CLI system settings file (admin-managed; global Gemini only)
    #[arg(
        long,
        help = "Target Gemini CLI system settings file (e.g. /etc/gemini-cli/settings.json; global Gemini only)"
    )]
    pub gemini_system: bool,

    /// Target Gemini CLI system-defaults.json file (global Gemini only)
    #[arg(
        long,
        help = "Target Gemini CLI system-defaults.json (e.g. /etc/gemini-cli/system-defaults.json; global Gemini only)"
    )]
    pub gemini_system_defaults: bool,
}

#[derive(Args, Debug, Clone, Copy)]
pub struct ConfigValidateArgs {
    /// Validate a specific agent (defaults to all available source files)
    #[arg(short, long, value_enum)]
    pub agent: Option<crate::app_config::Agent>,

    /// Treat warnings as errors (exit non-zero)
    #[arg(long)]
    pub strict: bool,
}

#[derive(Args, Debug, Clone, Copy)]
pub struct ConfigDoctorArgs {
    /// Inspect global deployment targets under $HOME instead of the current project
    #[arg(short, long)]
    pub global: bool,

    /// Focus diagnostics on a specific agent
    #[arg(short, long, value_enum)]
    pub agent: Option<crate::app_config::Agent>,
}

#[derive(Args, Debug, Clone, Copy)]
pub struct SkillsSyncArgs {
    /// Target system-wide configuration (~/.claude/skills/) instead of project-local directory
    #[arg(
        short,
        long,
        help = "Target system-wide skills directory instead of project-local skills directory"
    )]
    pub global: bool,

    /// Preview skill sync changes without writing them
    #[arg(short, long, help = "Preview skill sync changes without writing them")]
    pub dry_run: bool,

    /// Remove stale deployed skill files that Claudius previously published
    #[arg(long, help = "Remove stale deployed skill files previously published by Claudius")]
    pub prune: bool,

    /// Specify the agent (defaults to Claude)
    #[arg(short, long, value_enum, help = "Agent to use: claude, claude-code, codex, or gemini")]
    pub agent: Option<crate::app_config::Agent>,

    /// Deprecated no-op kept for backward compatibility
    #[arg(long, help = "Deprecated: Codex skills sync is enabled by default")]
    pub enable_codex_skills: bool,
}

#[derive(Args, Debug, Clone, Copy)]
pub struct SkillsValidateArgs {
    /// Validate a specific agent view instead of all supported skill render targets
    #[arg(short, long, value_enum)]
    pub agent: Option<crate::app_config::Agent>,

    /// Treat warnings as errors
    #[arg(long)]
    pub strict: bool,
}

#[derive(Args, Debug, Clone)]
pub struct SkillsRenderArgs {
    /// Specify the agent (defaults to the configured default agent or Claude)
    #[arg(short, long, value_enum)]
    pub agent: Option<crate::app_config::Agent>,

    /// Output directory for rendered skills
    #[arg(short, long, value_hint = clap::ValueHint::DirPath)]
    pub output: PathBuf,

    /// Remove stale rendered files that Claudius previously generated in the output directory
    #[arg(long)]
    pub prune: bool,
}

#[derive(Args, Debug, Clone)]
pub struct AppendContextArgs {
    /// Rule name from rules directory (e.g., 'security' for security.md)
    #[arg(value_name = "RULE", required_unless_present = "template_path")]
    pub rule: Option<String>,

    /// Target directory (optional, defaults to current directory or $HOME with --global)
    #[arg(
        short,
        long,
        value_name = "PATH",
        value_hint = clap::ValueHint::DirPath,
        env = "CLAUDIUS_PROJECT_PATH"
    )]
    pub path: Option<PathBuf>,

    /// Use custom template file instead of predefined rule
    #[arg(
        short = 'T',
        long,
        value_hint = clap::ValueHint::FilePath,
        conflicts_with = "rule",
        env = "CLAUDIUS_TEMPLATE_PATH"
    )]
    pub template_path: Option<PathBuf>,

    /// Target context file in home directory instead of project directory
    #[arg(
        short,
        long,
        help = "Target context file in home directory ($HOME) instead of project directory"
    )]
    pub global: bool,

    /// Specify the agent (overrides config file)
    #[arg(short, long, value_enum, help = "Agent to use: claude, codex, or gemini")]
    pub agent: Option<crate::app_config::Agent>,
}

#[derive(Args, Debug, Clone)]
pub struct InstallContextArgs {
    /// Rule names to install (e.g., 'security', 'testing')
    #[arg(value_name = "RULES", required_unless_present = "all", num_args = 1..)]
    pub rules: Vec<String>,

    /// Install ALL rules from the rules directory
    #[arg(short = 'A', long, help = "Install all available rules from the rules directory")]
    pub all: bool,

    /// Target directory (optional, defaults to current directory)
    #[arg(short, long, value_name = "PATH", value_hint = clap::ValueHint::DirPath)]
    pub path: Option<PathBuf>,

    /// Specify the agent (overrides config file)
    #[arg(short, long, value_enum, help = "Agent to use: claude, codex, or gemini")]
    pub agent: Option<crate::app_config::Agent>,

    /// Custom install directory (defaults to .agents/rules)
    #[arg(
        short = 'i',
        long,
        value_name = "DIR",
        value_hint = clap::ValueHint::DirPath,
        env = "CLAUDIUS_INSTALL_DIR"
    )]
    pub install_dir: Option<PathBuf>,
}

#[derive(Args, Debug, Clone, Copy)]
pub struct ContextListArgs {
    /// Show detailed tree structure of rule directories
    #[arg(
        short = 'T',
        long,
        help = "Display rule directory tree with nested files instead of simple list"
    )]
    pub tree: bool,
}

#[derive(Args, Debug, Clone)]
pub struct RunArgs {
    /// Command and arguments to execute
    #[arg(
        required = true,
        num_args = 1..,
        value_name = "COMMAND",
        trailing_var_arg = true,
        help = "Command and arguments to execute (use -- before the command)"
    )]
    pub command: Vec<String>,
}
