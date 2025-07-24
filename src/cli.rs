use clap::{Parser, Subcommand};
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
  claudius init

  # Force reinitialize (resets to defaults)
  claudius init --force")]
    Init {
        /// Force reinitialization (removes existing configurations)
        #[arg(short, long)]
        force: bool,
    },

    /// Synchronize configurations to .mcp.json/.claude/settings.json or claude.json
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
       • ~/claude.json for everything
  4. Syncs custom commands from commands/ to ~/.claude/commands/

Examples:
  # Basic sync to project-local files
  claudius sync

  # Sync to global configuration
  claudius sync --global

  # Preview changes without writing
  claudius sync --dry-run

  # Create backup before syncing
  claudius sync --backup"
    )]
    Sync {
        /// Path to the MCP servers configuration file
        #[arg(short, long, env = "CLAUDIUS_CONFIG", value_hint = clap::ValueHint::FilePath)]
        config: Option<PathBuf>,

        /// Preview changes without writing them
        #[arg(short, long, help = "Preview changes without writing them")]
        dry_run: bool,

        /// Create timestamped backup before making changes
        #[arg(
            short,
            long,
            help = "Create timestamped backup of target configuration before making changes"
        )]
        backup: bool,

        /// Override target configuration file path
        #[arg(short = 'T', long, env = "TARGET_CONFIG_PATH", value_hint = clap::ValueHint::FilePath)]
        target_config: Option<PathBuf>,

        /// Target system-wide configuration (~/.claude.json) instead of project-local files
        #[arg(
            short,
            long,
            help = "Target system-wide configuration (~/.claude.json) instead of project-local files (.mcp.json, .claude/settings.json)"
        )]
        global: bool,

        /// Only sync custom slash commands (skip config merge)
        #[arg(short = 'C', long)]
        commands_only: bool,

        /// Specify the agent to use (overrides config file)
        #[arg(short, long, value_enum, help = "Agent to use: claude, codex, or gemini")]
        agent: Option<crate::app_config::Agent>,
    },

    /// Append context to agent-specific context files
    #[command(
        name = "append-context",
        long_about = "Append context to agent-specific context files.

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
  claudius append-context security

  # Append to a specific directory
  claudius append-context testing --path /path/to/project

  # Use a custom template file
  claudius append-context --template-path ./my-template.md

  # Append to global context file in home directory
  claudius append-context security --global

  # Append a rule to a specific agent's global file
  claudius append-context security --global --agent gemini"
    )]
    AppendContext {
        /// Rule name from rules directory (e.g., 'security' for security.md)
        #[arg(value_name = "RULE", required_unless_present = "template_path")]
        rule: Option<String>,

        /// Target directory (optional, defaults to current directory or $HOME with --global)
        #[arg(short, long, value_name = "PATH", value_hint = clap::ValueHint::DirPath, env = "CLAUDIUS_PROJECT_PATH")]
        path: Option<PathBuf>,

        /// Use custom template file instead of predefined rule
        #[arg(short = 'T', long, value_hint = clap::ValueHint::FilePath, conflicts_with = "rule", env = "CLAUDIUS_TEMPLATE_PATH")]
        template_path: Option<PathBuf>,

        /// Target context file in home directory instead of project directory
        #[arg(
            short,
            long,
            help = "Target context file in home directory ($HOME) instead of project directory"
        )]
        global: bool,

        /// Specify the agent (overrides config file)
        #[arg(short, long, value_enum, help = "Agent to use: claude, codex, or gemini")]
        agent: Option<crate::app_config::Agent>,
    },

    /// Install context rules to project-local .agents/rules directory
    #[command(
        name = "install-context",
        long_about = "Install context rules to project-local .agents/rules directory.

This command:
  • Copies specified rules from your rules directory to ./.agents/rules/ (default)
  • Adds a reference directive to CLAUDE.md/AGENTS.md to include all rules
  • The directive is idempotent - it won't be added if already present

This approach keeps context files compact while allowing you to include many rules.

Rules are stored in: $XDG_CONFIG_HOME/claudius/rules/*.md

Examples:
  # Install specific rules (to ./.agents/rules/)
  claudius install-context security testing performance

  # Install ALL rules from rules directory
  claudius install-context --all

  # Install rules to a specific project directory
  claudius install-context security --path /path/to/project

  # Install rules with a custom install directory
  claudius install-context security --install-dir ./.claude/rules

  # Install rules with a custom agent
  claudius install-context security --agent gemini"
    )]
    InstallContext {
        /// Rule names to install (e.g., 'security', 'testing')
        #[arg(value_name = "RULES", required_unless_present = "all", num_args = 1..)]
        rules: Vec<String>,

        /// Install ALL rules from the rules directory
        #[arg(short = 'A', long, help = "Install all available rules from the rules directory")]
        all: bool,

        /// Target directory (optional, defaults to current directory)
        #[arg(short, long, value_name = "PATH", value_hint = clap::ValueHint::DirPath)]
        path: Option<PathBuf>,

        /// Specify the agent (overrides config file)
        #[arg(short, long, value_enum, help = "Agent to use: claude, codex, or gemini")]
        agent: Option<crate::app_config::Agent>,

        /// Custom install directory (defaults to .agents/rules)
        #[arg(short = 'i', long, value_name = "DIR", value_hint = clap::ValueHint::DirPath, env = "CLAUDIUS_INSTALL_DIR")]
        install_dir: Option<PathBuf>,
    },

    /// Run a command with resolved secrets from environment variables
    #[command(long_about = "Run a command with resolved secrets from environment variables.

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
  CLAUDIUS_SECRET_API_KEY=op://vault/item/field claudius run -- npm start

  # Run an interactive command
  CLAUDIUS_SECRET_DB_PASSWORD=op://vault/db/password claudius run -- psql -U admin

  # Run a long-running process
  CLAUDIUS_SECRET_TOKEN=op://vault/tokens/github claudius run -- ./server.sh

Note: Everything after '--' is treated as the command and its arguments.")]
    Run {
        /// Command and arguments to execute
        #[arg(
            required = true,
            num_args = 1..,
            value_name = "COMMAND",
            trailing_var_arg = true,
            help = "Command and arguments to execute (use -- before the command)"
        )]
        command: Vec<String>,
    },
}

// Helper methods to maintain backward compatibility
impl Commands {
    #[must_use]
    pub const fn is_sync(&self) -> bool {
        matches!(self, Self::Sync { .. })
    }

    #[must_use]
    pub const fn is_append_context(&self) -> bool {
        matches!(self, Self::AppendContext { .. })
    }
}
