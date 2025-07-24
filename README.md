# Claudius

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
- 🔧 **Cross-Platform** - Works on Linux, macOS, and Windows

## Installation

### Using Cargo

```bash
cargo install claudius
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

- **Rust**: 1.86.0 or higher
- **Nix**: 2.19.0 or higher (optional, for development)
- **1Password CLI**: For secret management features (optional)

## Build Status

✅ **Build**: Passing  
✅ **Tests**: All 299 tests passing  
✅ **Integration Tests**: Fixed all 34 integration test failures  
✅ **Security**: No vulnerabilities (`cargo audit` clean)  
⚠️ **Linting**: 117 clippy warnings (mostly style issues, not critical)  

### Recent Changes (Rust 1.86.0 Upgrade)
- Successfully upgraded from Rust 1.66.1 to 1.86.0
- Fixed 51 indexing panic warnings by using safe `.get()` methods
- Added missing error documentation to 31 functions
- Fixed 16 arithmetic side-effects using saturating arithmetic
- Resolved project-local sync issue where MCP servers are now correctly written to `.mcp.json`
- All integration tests now passing
- Updated pprof to 0.15.0 to fix RUSTSEC-2024-0408 security vulnerability

### Known Issues
- Clippy warnings remain for style issues (functions too long, missing error docs, etc.)
- These warnings do not affect functionality but should be addressed for code quality

## Quick Start

1. **Bootstrap configuration:**
   ```bash
   # Bootstrap with default configuration files
   claudius init
   
   # Force bootstrap (overwrites existing)
   claudius init --force
   ```

2. **Edit configuration files:**
   - Edit `~/.config/claudius/mcpServers.json` to define your MCP servers
   - Edit `~/.config/claudius/settings.json` to configure agent settings
   - Add custom commands to `~/.config/claudius/commands/`
   - Add context rules to `~/.config/claudius/rules/`

3. **Sync configuration:**
   ```bash
   # To project-local files (.mcp.json and .claude/settings.json)
   claudius sync
   
   # To global ~/.claude.json
   claudius sync --global
   ```

4. **Install context rules (optional):**
   ```bash
   # Install specific rules to project
   claudius install-context security testing
   
   # Or install all available rules
   claudius install-context --all
   ```

## Command Reference

### `claudius init`

Bootstrap Claudius configuration directory with default files.

```bash
# Bootstrap configuration (preserves existing files)
claudius init

# Force bootstrap (overwrites existing)
claudius init --force
```

This creates:
- `mcpServers.json` with example filesystem MCP server
- `settings.json` with default agent settings
- `config.toml` with Claudius application settings (optional)
- `commands/example.md` - Example custom slash command
- `rules/example.md` - Example context file rule template

### `claudius sync`

Synchronize all agent configurations to target files.

**Project-local mode (default):**
- MCP servers → `./.mcp.json`
- Settings → `./.claude/settings.json`
- Commands → `./.claude/commands/`

**Global mode (`--global`):**
- Everything → `~/.claude.json`
- Commands → `~/.claude/commands/`

**Agent-specific modes:**
- Claude (default): `.claude.json` or `.mcp.json`
- Codex: `.codex/config.toml`
- Gemini: `.gemini/settings.json`

```bash
# Basic sync to project-local files
claudius sync

# Sync to global configuration
claudius sync --global

# Preview changes without writing
claudius sync --dry-run

# Create backup before syncing
claudius sync --backup

# Use custom configuration paths
claudius sync --config /path/to/servers.json --claude-config /path/to/target.json

# Sync only commands
claudius sync --commands-only

# Use specific agent
claudius sync --agent codex
claudius sync --agent gemini
```

### `claudius append-context`

Append instructions or rules to project's context file (CLAUDE.md, CODEX.md, or GEMINI.md).

```bash
# Append a predefined rule
claudius append-context security

# Append to specific project
claudius append-context testing --path /path/to/project

# Use custom template file
claudius append-context --template-path ./my-template.md

# Use specific agent
claudius append-context security --agent codex
claudius append-context testing --agent gemini
```

### `claudius install-context`

Install context rules to project-local .agents/rules directory.

This command copies rules from your global rules directory to a project-local directory and adds a reference directive to your context file (CLAUDE.md/AGENTS.md). The directive ensures all installed rules are automatically included.

**Key features:**
- Keeps context files compact while including many rules
- Reference directive is idempotent (won't duplicate)
- Supports subdirectories with `**/*.md` pattern

```bash
# Install specific rules
claudius install-context security testing performance

# Install ALL rules from rules directory (including subdirectories)
claudius install-context --all

# Install to specific project
claudius install-context security --path /path/to/project

# Use custom install directory
claudius install-context security --install-dir ./.claude/rules

# Use specific agent
claudius install-context security --agent gemini
```

### `claudius run`

Execute commands with automatic secret resolution from environment variables.

```bash
# Run with resolved secrets
CLAUDIUS_SECRET_API_KEY=op://vault/api/key claudius run -- npm start

# Run interactive commands
CLAUDIUS_SECRET_DB_PASSWORD=op://vault/db/password claudius run -- psql -U admin

# Multiple secrets
export CLAUDIUS_SECRET_AWS_KEY=op://vault/aws/access-key
export CLAUDIUS_SECRET_AWS_SECRET=op://vault/aws/secret-key
claudius run -- aws s3 ls

# Nested variable references (NEW!)
export CLAUDIUS_SECRET_ACCOUNT_ID="12345"
export CLAUDIUS_SECRET_API_URL='https://api.example.com/$CLAUDIUS_SECRET_ACCOUNT_ID/v1'
claudius run -- curl $API_URL/users
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
├── settings.json      # General agent settings (optional)
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

### settings.json (Optional)

Configure general agent settings:

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

### config.toml (Optional)

Configure Claudius application settings:

```toml
# Default agent configuration (optional)
[default]
agent = "claude"  # or "codex" or "gemini"
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
claudius sync
```

Commands are deployed to `~/.claude/commands/` without the `.md` extension.

### Context File Rules

Create reusable templates in `~/.config/claudius/rules/`:

```bash
# Create a rule
echo "# Security Rules\n\nAlways validate input..." > ~/.config/claudius/rules/security.md

# Apply the rule to CLAUDE.md (default)
claudius append-context security

# Apply to agent-specific context files
claudius append-context security --agent codex   # → CODEX.md
claudius append-context security --agent gemini  # → GEMINI.md
```

### Context Management Strategies

Claudius offers two ways to manage project context:

1. **append-context**: Directly appends rules to CLAUDE.md/AGENTS.md
   - Best for: Small number of rules, simple projects
   - Result: All content in one file

2. **install-context**: Copies rules to `.agents/rules/` with reference directive
   - Best for: Many rules, complex projects, team collaboration
   - Result: Compact context file + organized rule structure

## Environment Variables

- `CLAUDIUS_CONFIG` - Default path for MCP servers configuration
- `CLAUDE_CONFIG_PATH` - Default path for claude.json
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
claudius run -- echo $API_ENDPOINT
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
claudius sync --global

# Project-specific configuration
cd /path/to/project
claudius sync  # Creates .mcp.json and .claude/settings.json

# Project instructions
claudius append-context project-rules
```

### Multi-Agent Support

```bash
# Configure for different AI agents
claudius sync --agent codex   # Creates .codex/config.toml
claudius sync --agent gemini  # Creates .gemini/settings.json

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
claudius run -- ./my-app
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
   claudius sync --global
   ```

## Troubleshooting

### Configuration not found
```bash
# Check configuration directory
ls -la ~/.config/claudius/

# Use custom path
claudius sync --config /custom/path/mcpServers.json
```

### Permission errors
```bash
# Check file permissions (example for Claude agent)
ls -la ~/.claude.json
ls -la ./.mcp.json

# Use sudo if needed (not recommended)
sudo claudius sync --global
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
- Context file templates (CLAUDE.md, CODEX.md, GEMINI.md)
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
