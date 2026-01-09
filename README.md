# Claudius

[![CI Pipeline](https://github.com/cariandrum22/claudius/actions/workflows/ci.yml/badge.svg)](https://github.com/cariandrum22/claudius/actions/workflows/ci.yml)
[![Security Audit](https://github.com/cariandrum22/claudius/actions/workflows/security.yml/badge.svg)](https://github.com/cariandrum22/claudius/actions/workflows/security.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Multi-agent configuration management tool for AI assistants

## Overview

Claudius is a powerful configuration management tool that helps developers maintain, version control, and share configurations for multiple AI agents (Claude, Codex, Gemini) across projects and teams. It provides a structured approach to managing MCP (Model Context Protocol) servers, agent-specific settings, custom commands, and project-specific context instructions.

## Key Features

- 🔄 **Configuration Synchronization** - Sync MCP servers, settings, and commands
- 📁 **Multi-Project Support** - Project-local and global configurations
- 📝 **CLAUDE.md Templates** - Manage project-specific instructions
- 🛡️ **Safe Operations** - Dry-run mode and optional backups
- 🔐 **Secret Management** - Integration with 1Password for secure credentials
- 🔗 **Variable Expansion** - DAG-based nested environment variable resolution
- 🤖 **Multi-Agent Support** - Configure for Claude, Codex, or Gemini agents
- 🚀 **Fast & Reliable** - Written in Rust for performance and safety
- 🐧 **Linux and macOS** - Designed for Unix-like operating systems

## Installation

### Using Cargo (from Git)

```bash
cargo install --git https://github.com/cariandrum22/claudius
```

### Using Nix Flake

```bash
# Run directly
nix run github:cariandrum22/claudius

# Install to system
nix profile install github:cariandrum22/claudius
```

### Using Home Manager

Add claudius to your home-manager configuration:

```nix
# flake.nix
{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    home-manager.url = "github:nix-community/home-manager";
    claudius.url = "github:cariandrum22/claudius";
  };

  outputs = { nixpkgs, home-manager, claudius, ... }:
    let
      system = "x86_64-linux";  # or "aarch64-darwin" for ARM Mac
      pkgs = nixpkgs.legacyPackages.${system};
    in {
      homeConfigurations."your-username" = home-manager.lib.homeManagerConfiguration {
        inherit pkgs;
        
        extraSpecialArgs = {
          inherit claudius;
        };
        
        modules = [ ./home.nix ];
      };
    };
}
```

In your `home.nix`:

```nix
{ config, pkgs, claudius, ... }:

{
  home.packages = [ 
    claudius.packages.${pkgs.system}.default
  ];
}
```

Then apply the configuration:

```bash
home-manager switch --flake .#your-username
```

### Building from Source

```bash
git clone https://github.com/cariandrum22/claudius.git
cd claudius

# Using Nix development environment
nix develop
cargo build --release

# Install binary
cargo install --path .
```

## Prerequisites

- **Rust**: 1.92.0 or higher
- **Nix**: 2.19.0 or higher (optional, for development)
- **1Password CLI**: For secret management features (optional)

## Quick Start

Before running any subcommands, you can inspect the reorganized CLI surface:

```bash
claudius --list-commands
```

1. **Bootstrap configuration:**
   ```bash
   # Bootstrap with default configuration files
   claudius config init
   
   # Force bootstrap (overwrites existing)
   claudius config init --force
   ```

2. **Edit configuration files:**
   - Edit `~/.config/claudius/mcpServers.json` to define your MCP servers
   - Edit `~/.config/claudius/claude.settings.json` to configure Claude/Claude Code settings
   - Edit `~/.config/claudius/codex.settings.toml` to configure Codex settings
   - (Optional) Edit `~/.config/claudius/codex.requirements.toml` for Codex admin-enforced constraints
   - (Optional) Edit `~/.config/claudius/codex.managed_config.toml` for Codex admin-managed defaults
   - Edit `~/.config/claudius/gemini.settings.json` to configure Gemini settings
   - Add custom commands to `~/.config/claudius/commands/`
   - Add context rules to `~/.config/claudius/rules/`

3. **Sync configuration:**
   ```bash
   # To project-local files (.mcp.json and .claude/settings.json)
   claudius config sync
   
   # To Claude Desktop global config
   claudius config sync --global --agent claude

   # To Claude Code global config
   claudius config sync --global --agent claude-code

   # Sync only custom commands
   claudius commands sync
   ```

4. **Install context rules (optional):**
   ```bash
   # Install specific rules to project
   claudius context install security testing
   
   # Or install all available rules
   claudius context install --all
   ```

## Command Reference

### Migrating from legacy `claudius sync`

Version 0.1 reorganized the CLI into domain-focused verbs. If you previously ran
`claudius sync`, use the following replacements:

- Project/local sync: `claudius config sync`
- Global sync: `claudius config sync --global`
- Commands only: `claudius commands sync`

Tip: `claudius --list-commands` prints the new layout along with the available
subcommands.

### `claudius config init`

Bootstrap Claudius configuration directory with default files.

```bash
# Bootstrap configuration (preserves existing files)
claudius config init

# Force bootstrap (overwrites existing)
claudius config init --force
```

This creates:
- `mcpServers.json` with example filesystem MCP server
- `claude.settings.json` with default Claude/Claude Code settings
- `codex.settings.toml` with default Codex settings
- `codex.requirements.toml` with default Codex requirements (admin-enforced)
- `codex.managed_config.toml` with default Codex managed defaults (admin-managed)
- `gemini.settings.json` with default Gemini settings
- `config.toml` with Claudius application settings (optional)
- `commands/example.md` - Example custom slash command
- `rules/example.md` - Example context file rule template

### `claudius config sync`

Synchronize all agent configurations to target files.

**Project-local mode (default):**
- Claude Desktop (`--agent claude`): MCP servers → `./.mcp.json`
- Claude Code (`--agent claude-code`):
  - Project scope (default / `--scope project`): MCP servers → `./.mcp.json`, settings → `./.claude/settings.json`
  - Local scope (`--scope local`): MCP servers → `~/.claude.json` (per-project), settings → `./.claude/settings.local.json`
- Codex (`--agent codex`): settings + MCP servers → `./.codex/config.toml`
- Gemini (`--agent gemini`): settings + MCP servers → `./.gemini/settings.json`

**Global mode (`--global`):**
- Claude Desktop (`--agent claude`): `$XDG_CONFIG_HOME/Claude/claude_desktop_config.json` (macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`, Windows: `%APPDATA%\\Claude\\claude_desktop_config.json`)
- Claude Code (`--agent claude-code`):
  - User scope (default / `--scope user`): MCP servers → `~/.claude.json`, settings → `~/.claude/settings.json`
  - Managed scope (`--scope managed`): MCP servers → `managed-mcp.json`, settings → `managed-settings.json` (system directories)
- Codex (`--agent codex`):
  - User config: `~/.codex/config.toml`
  - (Optional) Admin-enforced: `/etc/codex/requirements.toml` (`--codex-requirements`)
  - (Optional) Managed defaults: `/etc/codex/managed_config.toml` (`--codex-managed-config`)
- Gemini (`--agent gemini`):
  - User settings: `~/.gemini/settings.json`
  - System settings: `/etc/gemini-cli/settings.json` (`--gemini-system`, path varies by OS)

```bash
# Basic sync to project-local files
claudius config sync

# Sync to global configuration
claudius config sync --global

# Preview changes without writing
claudius config sync --dry-run

# Create backup before syncing
claudius config sync --backup

# Use custom configuration paths
claudius config sync --config /path/to/servers.json --target-config /path/to/target.json

# Use specific agent
claudius config sync --agent claude-code
claudius config sync --agent codex
claudius config sync --agent gemini

# Claude Code scope selection
claudius config sync --agent claude-code --scope managed
claudius config sync --agent claude-code --scope local

# Codex admin-managed files (system-wide)
claudius config sync --global --agent codex --codex-requirements --codex-managed-config

# Gemini system settings (system-wide)
claudius config sync --global --agent gemini --gemini-system
```

### `claudius config validate`

Validate configuration source files without writing anything.

```bash
# Validate all available source files
claudius config validate

# Validate only a specific agent
claudius config validate --agent claude-code
claudius config validate --agent codex
claudius config validate --agent gemini

# Fail on warnings
claudius config validate --strict
```

### `claudius commands sync`

Synchronize custom slash command markdown files into Claude's command directories.

```bash
# Sync commands to project-local .claude/commands/
claudius commands sync

# Sync commands to global ~/.claude/commands/
claudius commands sync --global
```


### `claudius context append`

Append instructions or rules to the agent's context file (CLAUDE.md for Claude/Claude Code, AGENTS.md for Codex/Gemini).

```bash
# Append a predefined rule
claudius context append security

# Append to specific project
claudius context append testing --path /path/to/project

# Use custom template file
claudius context append --template-path ./my-template.md

# Use specific agent
claudius context append security --agent codex
claudius context append testing --agent gemini
```

### `claudius context install`

Install context rules to project-local .agents/rules directory.

This command copies rules from your global rules directory to a project-local directory and adds a reference directive to your context file (CLAUDE.md/AGENTS.md). The directive lists each installed rule explicitly with its file path.

**Key features:**
- Keeps context files compact while including many rules
- Reference directive is idempotent (updates existing section without duplication)
- Lists specific file paths for each rule
- Supports subdirectories and preserves directory structure

```bash
# Install specific rules
claudius context install security testing performance

# Install ALL rules from rules directory (including subdirectories)
claudius context install --all

# Install to specific project
claudius context install security --path /path/to/project

# Use custom install directory
claudius context install security --install-dir ./.claude/rules

# Use specific agent
claudius context install security --agent gemini
```

### `claudius secrets run`

Execute commands with automatic secret resolution from environment variables.

```bash
# Run with resolved secrets
CLAUDIUS_SECRET_API_KEY=op://vault/api/key claudius secrets run -- npm start

# Run interactive commands
CLAUDIUS_SECRET_DB_PASSWORD=op://vault/db/password claudius secrets run -- psql -U admin

# Multiple secrets
export CLAUDIUS_SECRET_AWS_KEY=op://vault/aws/access-key
export CLAUDIUS_SECRET_AWS_SECRET=op://vault/aws/secret-key
claudius secrets run -- aws s3 ls

# Nested variable references (NEW!)
export CLAUDIUS_SECRET_ACCOUNT_ID="12345"
export CLAUDIUS_SECRET_API_URL='https://api.example.com/$CLAUDIUS_SECRET_ACCOUNT_ID/v1'
claudius secrets run -- curl $API_URL/users
# Resolves to: https://api.example.com/12345/v1/users
```

Features:
- Automatic secret resolution from 1Password
- DAG-based variable expansion for nested references
- Full stdio inheritance for interactive commands
- Signal forwarding (Ctrl+C works correctly)
- Environment variable injection without prefix
- Circular dependency detection

## Configuration Files

### Directory Structure

```
~/.config/claudius/
├── config.toml        # Claudius app configuration (optional)
├── mcpServers.json    # MCP server definitions
├── claude.settings.json # Claude/Claude Code settings (optional)
├── codex.settings.toml  # Codex settings (optional)
├── codex.requirements.toml # Codex requirements (admin-enforced, optional)
├── codex.managed_config.toml # Codex managed defaults (admin-managed, optional)
├── gemini.settings.json # Gemini settings (optional)
├── settings.json      # Legacy alias for claude.settings.json (backward compatible)
├── commands/          # Custom slash commands
│   └── *.md          # Command files
└── rules/            # Context file templates
    └── *.md          # Rule files
```

### mcpServers.json

Define your MCP servers:

```json
{
  "mcpServers": {
    "server-name": {
      "command": "executable-command",
      "args": ["arg1", "arg2"],
      "env": {
        "API_KEY": "your-key"
      }
    }
  }
}
```

### claude.settings.json (Optional)

Configure Claude/Claude Code settings:

```json
{
  "apiKeyHelper": "/path/to/key-generator.sh",
  "cleanupPeriodDays": 20,
  "env": {"CUSTOM_VAR": "value"},
  "includeCoAuthoredBy": false,
  "permissions": {
    "allow": ["Bash(npm run lint)"],
    "deny": ["Write(/etc/*)"],
    "defaultMode": "allow"
  },
  "preferredNotifChannel": "chat"
}
```

### codex.settings.toml (Optional)

Configure Codex CLI settings (merged into `.codex/config.toml` or `~/.codex/config.toml`):

```toml
# model = "gpt-5-codex"
# approval_policy = "on-request"
```

### codex.requirements.toml (Optional)

Admin-enforced constraints for Codex (synced with `--codex-requirements`):

- Target (Unix): `/etc/codex/requirements.toml`
- Override target path: `CLAUDIUS_CODEX_REQUIREMENTS_PATH`

```toml
# allowed_approval_policies = ["untrusted", "on-request", "on-failure"]
# allowed_sandbox_modes = ["read-only", "workspace-write"]
```

### codex.managed_config.toml (Optional)

Admin-managed defaults for Codex (synced with `--codex-managed-config`):

- Target (Unix): `/etc/codex/managed_config.toml`
- Override target path: `CLAUDIUS_CODEX_MANAGED_CONFIG_PATH`

```toml
# approval_policy = "on-request"
# sandbox_mode = "workspace-write"
```

### gemini.settings.json (Optional)

Configure Gemini CLI settings (category-based schema):

```json
{
  "$schema": "https://raw.githubusercontent.com/google-gemini/gemini-cli/main/schemas/settings.schema.json",
  "general": {},
  "ui": {},
  "tools": {},
  "context": {},
  "privacy": {},
  "telemetry": {}
}
```

### config.toml (Optional)

Configure Claudius application settings:

```toml
# Default agent configuration (optional)
[default]
agent = "claude"  # or "claude-code" or "codex" or "gemini"
context-file = "CLAUDE.md"  # optional custom filename

# Secret Manager Configuration (optional)
[secret-manager]
type = "1password"  # or "vault"

# When using 1Password:
# - Install 1Password CLI (`op`)
# - Sign in with `op signin`
# - Use op:// references in CLAUDIUS_SECRET_* variables
#
# For op:// references in URLs, use {{op://...}} syntax:
# CLAUDIUS_SECRET_URL=https://api.example.com/{{op://vault/item/field}}/endpoint
```

### Custom Commands

Create custom slash commands in `~/.config/claudius/commands/`:

```bash
# Create a command
echo "# My Command\n\nCommand implementation..." > ~/.config/claudius/commands/mycommand.md

# Commands are synced automatically
claudius config sync
```

Commands are deployed to `~/.claude/commands/` without the `.md` extension.

### Context File Rules

Create reusable templates in `~/.config/claudius/rules/`:

```bash
# Create a rule
echo "# Security Rules\n\nAlways validate input..." > ~/.config/claudius/rules/security.md

# Apply the rule to CLAUDE.md (default)
claudius context append security

# Apply to agent-specific context files
claudius context append security --agent codex   # → AGENTS.md
claudius context append security --agent gemini  # → AGENTS.md
```

### Context Management Strategies

Claudius offers two ways to manage project context:

1. **context append**: Directly appends rules to CLAUDE.md/AGENTS.md
   - Best for: Small number of rules, simple projects
   - Result: All content in one file

2. **context install**: Copies rules to `.agents/rules/` with reference directive
   - Best for: Many rules, complex projects, team collaboration
   - Result: Compact context file + organized rule structure

## Environment Variables

- `CLAUDIUS_CONFIG` - Default path for MCP servers configuration source (`mcpServers.json`)
- `TARGET_CONFIG_PATH` - Override target config path for `claudius config sync`
- `CLAUDIUS_CLAUDE_CODE_MANAGED_DIR` - Override Claude Code managed config directory
- `CLAUDIUS_CODEX_REQUIREMENTS_PATH` - Override Codex `requirements.toml` target path
- `CLAUDIUS_CODEX_MANAGED_CONFIG_PATH` - Override Codex `managed_config.toml` target path
- `GEMINI_CLI_SYSTEM_SETTINGS_PATH` - Override Gemini CLI system settings path (used with `--gemini-system`)
- `XDG_CONFIG_HOME` - Base directory for configuration files
- `CLAUDIUS_SECRET_*` - Environment variables for secret injection (prefix is removed)
- `CLAUDIUS_TEST_MOCK_OP` - Enable mock mode for 1Password CLI (for testing)

### Variable Expansion

Claudius supports DAG-based variable expansion for nested environment variable references:

```bash
# Define variables with references
export CLAUDIUS_SECRET_HOST="api.example.com"
export CLAUDIUS_SECRET_PORT="8443"
export CLAUDIUS_SECRET_PROTOCOL="https"
export CLAUDIUS_SECRET_SERVER_URL='$CLAUDIUS_SECRET_PROTOCOL://$CLAUDIUS_SECRET_HOST:$CLAUDIUS_SECRET_PORT'
export CLAUDIUS_SECRET_API_ENDPOINT='$CLAUDIUS_SECRET_SERVER_URL/v2/production'

# Run command - variables are expanded automatically
claudius secrets run -- echo $API_ENDPOINT
# Output: https://api.example.com:8443/v2/production
```

Features:
- Supports both `$VAR` and `${VAR}` syntax
- Automatic topological sorting for correct resolution order
- Circular dependency detection
- Works with 1Password references (resolved first, then expanded)

## Merge Strategies

### MCP Servers
- New servers are added
- Existing servers with same name are replaced
- Other servers remain unchanged

### Settings
- Only specified fields are updated
- Unspecified fields remain unchanged
- All other configuration content is preserved

### Commands
- All .md files are synced
- Existing commands are overwritten
- Removed source files don't delete deployed commands

## Code Coverage

Claudius maintains high code quality standards with comprehensive test coverage:

### Requirements
- Line Coverage: ≥ 90%
- Branch Coverage: ≥ 85%
- Function Coverage: 100%

### Running Coverage
```bash
# Check test statistics
just test-stats

# Run coverage analysis (requires cargo-llvm-cov)
just coverage

# Generate HTML report
just coverage-html

# Run with detailed options
just coverage-detailed --min-coverage 90
```

For coverage setup instructions, see CLAUDE.md.

## Advanced Usage

### Multi-Project Setup

```bash
# Global configuration for all projects
claudius config sync --global

# Project-specific configuration
cd /path/to/project
claudius config sync  # Creates .mcp.json and .claude/settings.json

# Project instructions
claudius context append project-rules
```

### Multi-Agent Support

```bash
# Configure for different AI agents
claudius config sync --agent codex   # Creates .codex/config.toml
claudius config sync --agent gemini  # Creates .gemini/settings.json

# Set default agent in config.toml
echo '[agent]
type = "codex"' >> ~/.config/claudius/config.toml
```

### Secret Management

```bash
# Configure 1Password integration
echo '[secret-manager]
type = "1password"' >> ~/.config/claudius/config.toml

# Use secrets in environment
export CLAUDIUS_SECRET_API_KEY=op://vault/api/key
export CLAUDIUS_SECRET_DB_PASS=op://vault/db/password

# For op:// references in URLs, use {{op://...}} delimiter syntax
export CLAUDIUS_SECRET_BASE_URL="https://api.example.com/v1/{{op://vault/account/id}}/{{op://vault/region/code}}"
export CLAUDIUS_SECRET_AUTH="Bearer {{op://vault/tokens/api}}"

# Run command with resolved secrets
claudius secrets run -- ./my-app
# API_KEY, DB_PASS, BASE_URL, and AUTH are available to my-app with resolved values
```

### Team Collaboration

1. **Share configurations via Git:**
   ```bash
   cd ~/.config/claudius
   git init
   git add .
   git commit -m "Team agent configurations"
   ```

2. **Team members clone and sync:**
   ```bash
   git clone team-configs ~/.config/claudius
   claudius config sync --global --agent claude
   claudius config sync --global --agent claude-code
   ```

## Troubleshooting

### Configuration not found
```bash
# Check configuration directory
ls -la ~/.config/claudius/

# Use custom path
claudius config sync --config /custom/path/mcpServers.json
```

### Permission errors
```bash
# Check file permissions
ls -la ~/.claude.json                    # Claude Code MCP servers
ls -la ~/.claude/settings.json           # Claude Code settings
ls -la "$XDG_CONFIG_HOME/Claude/claude_desktop_config.json"  # Claude Desktop
ls -la ./.mcp.json

# Use sudo if needed (not recommended)
sudo claudius config sync --global
```

### JSON validation
```bash
# Validate JSON syntax
jq . ~/.config/claudius/mcpServers.json
```

### Nix build issues
If you're using claudius in a Nix flake and encounter test failures:
```bash
# The flake automatically sets CLAUDIUS_TEST_MOCK_OP=1
# This enables mock mode for 1Password CLI in sandboxed builds
```

### Test execution
When running tests, Claudius automatically uses mocks for external commands:
```bash
# Tests use mocks by default (CLAUDIUS_TEST_MOCK_OP=1 is set automatically)
cargo test
just test
just check

# The 1Password CLI is never called during tests, even if configured
# This ensures tests are reliable and reproducible
```

## Development

For development documentation including:
- Build instructions and testing
- Code style and linting guidelines
- Dependency management
- Contributing guidelines
- Architecture details

Please see [CLAUDE.md](./CLAUDE.md).

## Version History

See [CHANGELOG.md](./CHANGELOG.md) for detailed version history.

Current version: **v0.1.0** - Initial development release

Key features in v0.1.0:
- Multi-agent support (Claude, Codex, Gemini)
- Secret management with 1Password integration
- DAG-based variable expansion for nested environment variables
- Project-local and global configuration modes
- Context file templates (CLAUDE.md and AGENTS.md)
- Secure command execution with automatic secret resolution
- Comprehensive test coverage and Nix flake support

## Project Background

This is a personal utility I developed for multi-agent configuration management, but it also serves as an experimental project for AI-agent-driven development workflows. The codebase contains substantial contributions from generative AI, with the initial codebase largely written by Claude Code.

### AI-Generated Code Notice

This project represents an experiment in AI-assisted software development. A significant portion of the code, particularly the initial implementation, was generated by Claude Code and other AI assistants. This collaborative approach between human and AI demonstrates new possibilities in software development workflows.

### Copyright Statement

Unless explicitly stated otherwise, I do not claim copyright on the code in this repository. The extensive use of AI-generated content makes traditional copyright attribution complex and, in the spirit of open collaboration, unnecessary for this project.

## License

MIT License

## Contributing

Contributions are welcome! Please read our development documentation in [CLAUDE.md](./CLAUDE.md) before submitting pull requests.

## Links

- [Model Context Protocol (MCP)](https://modelcontextprotocol.io/)
- [Project Documentation](./CLAUDE.md)
- [Anthropic Claude](https://www.anthropic.com/claude)
- [OpenAI Codex](https://openai.com/blog/openai-codex)
- [Google Gemini](https://gemini.google.com/)
