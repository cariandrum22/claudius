use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "claudius",
    about = "Claude configuration management tool - Manage MCP servers, settings, commands, and project instructions",
    long_about = "Claudius is a comprehensive configuration management tool for Claude Desktop/CLI.

It helps you:
  • Manage MCP (Model Context Protocol) server configurations
  • Maintain Claude settings across projects
  • Organize custom slash commands
  • Define project-specific instructions via CLAUDE.md

Configuration files are stored in:
  • $XDG_CONFIG_HOME/claudius/ (or ~/.config/claudius/)
    - mcpServers.json: MCP server definitions
    - settings.json: General Claude settings
    - commands/: Custom slash commands (*.md)
    - rules/: CLAUDE.md templates (*.md)

Target files:
  • ./.mcp.json (MCP servers in project-local mode, default)
  • ./.claude/settings.json (Settings in project-local mode)
  • ~/.claude.json (everything in global mode with --global)
  • ./CLAUDE.md (project instructions)",
    version,
    author
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

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

    /// Manage custom command definitions
    #[command(subcommand, name = "commands")]
    Command(CommandCommands),

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
  • settings.json - Claude settings template
  • commands/ - Directory for custom slash commands
  • rules/ - Directory for CLAUDE.md rules

By default, existing files are preserved. Use --force to reinitialize.

Examples:
  # Initialize configuration (preserves existing files)
  claudius config init

  # Force reinitialize (resets to defaults)
  claudius config init --force")]
    Init(InitArgs),

    /// Synchronize configurations to project or global targets
    #[command(
        long_about = "Synchronize all Claude configurations to the target configuration files.

This command:
  1. Reads mcpServers.json for MCP server definitions
  2. Reads settings.json for Claude settings (if exists)
  3. Writes configurations to:
     - Project-local mode (default):
       • ./.mcp.json for MCP servers
       • ./.claude/settings.json for settings
     - Global mode (--global):
       • ~/claudius.json for everything
  4. Syncs custom commands from commands/ to ~/.claude/commands/

Examples:
  # Basic sync to project-local files
  claudius config sync

  # Sync to global configuration
  claudius config sync --global

  # Preview changes without writing
  claudius config sync --dry-run

  # Create backup before syncing
  claudius config sync --backup"
    )]
    Sync(ConfigSyncArgs),
}

#[derive(Subcommand, Debug, Clone, Copy)]
pub enum CommandCommands {
    /// Synchronize custom slash commands into Claude directories
    #[command(
        long_about = "Synchronize custom slash command definitions into Claude directories.

This command copies the markdown files from your commands/ directory into \
Claude's command directory, ensuring all commands are up to date."
    )]
    Sync(CommandSyncArgs),
}

#[derive(Subcommand, Debug)]
pub enum ContextCommands {
    /// Append rules or templates into agent-specific context files
    #[command(name = "append", long_about = "Append context to agent-specific context files.

Each agent uses a different context file:
  • Claude: CLAUDE.md
  • Others (Gemini, Codex): AGENTS.md

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
    #[command(name = "install", long_about = "Install context rules to project-local .agents/rules directory.

This command:
  • Copies specified rules from your rules directory to ./.agents/rules/ (default)
  • Adds a reference directive to CLAUDE.md/AGENTS.md to include all rules
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
    #[command(long_about = "List all available context rules and templates in the rules directory.")]
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

    /// Override target configuration file path
    #[arg(short = 'T', long, env = "TARGET_CONFIG_PATH", value_hint = clap::ValueHint::FilePath)]
    pub target_config: Option<PathBuf>,

    /// Target system-wide configuration (~/.claude.json) instead of project-local files
    #[arg(
        short,
        long,
        help = "Target system-wide configuration (~/.claude.json) instead of project-local files (.mcp.json, .claude/settings.json)"
    )]
    pub global: bool,

    /// Specify the agent to use (overrides config file)
    #[arg(short, long, value_enum, help = "Agent to use: claude, codex, or gemini")]
    pub agent: Option<crate::app_config::Agent>,
}

#[derive(Args, Debug, Clone, Copy)]
pub struct CommandSyncArgs {
    /// Target system-wide configuration (~/.claude/commands/) instead of project-local directory
    #[arg(
        short,
        long,
        help = "Target system-wide commands directory (~/.claude/commands/) instead of project-local commands directory"
    )]
    pub global: bool,
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
