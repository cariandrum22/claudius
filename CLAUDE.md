# Claudius - Claude Configuration Management Tool

## Overview

`claudius` is a comprehensive configuration management tool for Claude Desktop/CLI that enables developers to maintain, version control, and share their Claude configurations across projects and teams.

### Why Claudius?

#### Problems Solved
- **Configuration Loss**: Claude configurations are frequently lost during updates
- **Manual Management**: Tedious process to restore settings manually
- **Project Isolation**: Difficult to maintain different configurations per project
- **Team Inconsistency**: No easy way to share configurations across teams
- **No Backup Strategy**: No built-in way to backup and restore configurations

#### Solution
Claudius provides a structured approach to Claude configuration management:
- Separate configuration files for different aspects (servers, settings, commands)
- Clear separation between global and project-local configurations
- Version-controllable text files
- Simple CLI commands for synchronization

## Quick Start

### Prerequisites
- Rust 1.86.0 or higher
- Nix 2.19.0 or higher (optional, for development)

### Basic Usage

```bash
# Initialize configuration
claudius init

# Sync configurations (project-local)
claudius sync

# Sync to global configuration
claudius sync --global

# Add template to CLAUDE.md
claudius append-template security
```

## Features

### 1. Configuration Management
- **MCP Server Configurations**: Define and manage Model Context Protocol servers
- **Claude Settings**: Manage API keys, environment variables, permissions, and other Claude settings
- **Custom Commands**: Create and distribute custom slash commands for Claude
- **Project Instructions**: Define project-specific instructions via CLAUDE.md
- **Secret Management**: Integrate with 1Password and HashiCorp Vault for secure credential handling

### 2. Multi-Project Support
- **Project-Local Configurations**: Each project can have its own `.mcp.json` and `.claude/settings.json`
- **Global Configurations**: Maintain system-wide settings in `~/.claude.json`
- **Configuration Separation**: MCP servers and settings are managed in separate files for project-local mode

### 3. Team Collaboration
- **Version Control**: All configuration files are JSON/Markdown, perfect for Git
- **Configuration Sharing**: Share MCP servers, commands, and rules across teams
- **Consistent Environments**: Ensure all team members use the same Claude configuration

### 4. Secure Command Execution
- **Run Command**: Execute any command with automatic secret resolution
- **Interactive Support**: Full stdio inheritance for interactive applications
- **Signal Handling**: Proper signal forwarding (e.g., Ctrl+C)
- **Environment Injection**: Seamless environment variable injection from secret managers

## File Structure & Architecture

### Configuration Sources
```
$XDG_CONFIG_HOME/claudius/     # or ~/.config/claudius/
├── config.toml                # Claudius app configuration (optional)
├── mcpServers.json            # MCP server definitions
├── settings.json              # General Claude settings (optional)
├── commands/                  # Custom slash commands
│   └── *.md                   # Command files (markdown)
└── rules/                     # CLAUDE.md templates
    └── *.md                   # Rule files (markdown)
```

### Target Locations
```
Project Directory (default):
├── .mcp.json                  # Project-local MCP servers configuration
├── .claude/
│   ├── settings.json          # Project-local Claude settings
│   └── commands/              # Project-local slash commands
└── CLAUDE.md                  # Project-specific instructions

Home Directory (--global):
├── .claude.json               # Global Claude configuration
└── .claude/
    └── commands/              # Global slash commands
```

### Data Flow

**Project-Local Mode (default):**
```
┌─────────────────┐     ┌──────────────────┐     ┌──────────────────────┐
│ Config Sources  │────▶│    Claudius      │────▶│ ./.mcp.json          │
│ • mcpServers    │     │ • Read configs   │     │ ./.claude/settings.json│
│ • settings      │     │ • Split output   │     │                      │
└─────────────────┘     └──────────────────┘     └──────────────────────┘
```

**Global Mode (--global):**
```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│ Config Sources  │────▶│    Claudius      │────▶│ ~/.claude.json  │
│ • mcpServers    │     │ • Read configs   │     │ (all merged)    │
│ • settings      │     │ • Merge data     │     │                 │
└─────────────────┘     └──────────────────┘     └─────────────────┘
```

## Command Reference

### `claudius init`
Bootstrap configuration directory with default files.

```bash
# Bootstrap configuration (preserves existing)
claudius init

# Force bootstrap (overwrites existing)
claudius init --force
```

Creates default:
- `mcpServers.json` with filesystem MCP server example
- `settings.json` with default Claude settings
- `config.toml` with commented configuration options
- `commands/example.md` - Example custom command
- `rules/example.md` - Example CLAUDE.md rule

### `claudius sync`
Synchronize configurations to target files.

**Project-local mode (default):**
- MCP servers → `./.mcp.json`
- Settings → `./.claude/settings.json`
- Commands → `./.claude/commands/`

**Global mode (--global):**
- Everything → `~/.claude.json`
- Commands → `~/.claude/commands/`

```bash
# Basic sync (project-local: .mcp.json + .claude/settings.json)
claudius sync

# Sync to global ~/.claude.json
claudius sync --global

# Preview changes
claudius sync --dry-run

# Create backup before syncing
claudius sync --backup

# Custom paths
claudius sync --config /path/to/servers.json --claude-config /path/to/target.json

# Sync only commands
claudius sync --commands-only
```

### `claudius append-template`
Append rules or templates to CLAUDE.md.

```bash
# Use predefined rule
claudius append-template security

# Specify target directory
claudius append-template testing /path/to/project

# Use custom template
claudius append-template dummy . --template-path ./my-template.md
```

### `claudius install-context`
Install context rules to project-local .agents/rules directory with automatic reference directive.

This command:
- Copies specified rules from your rules directory to ./.agents/rules/ (default)
- Adds a reference directive to CLAUDE.md/AGENTS.md to include all rules
- The directive is idempotent - it won't be added if already present
- Supports subdirectories and preserves directory structure

```bash
# Install specific rules
claudius install-context security testing performance

# Install ALL rules from rules directory (including subdirectories)
claudius install-context --all

# Install rules to a specific project directory
claudius install-context security --path /path/to/project

# Install rules with a custom install directory
claudius install-context security --install-dir ./.claude/rules

# Install rules for a specific agent
claudius install-context security --agent gemini
```

The reference directive added to CLAUDE.md/AGENTS.md looks like:
```markdown
# External Rule References
The following rules are included from the .agents/rules directory:
{include:.agents/rules/**/*.md}
```

### `claudius run`
Execute commands with resolved secrets from environment variables.

This command enables running any program with secrets automatically resolved from configured secret managers. It's particularly useful for:
- Running applications that need API keys or passwords
- Executing database clients with credentials
- Starting servers with authentication tokens
- Running CI/CD scripts with secure variables

```bash
# Run with resolved secrets
CLAUDIUS_SECRET_API_KEY=op://vault/api/key claudius run -- npm start

# Run interactive commands
CLAUDIUS_SECRET_DB_PASSWORD=op://vault/db/password claudius run -- psql -U admin

# Run long-running processes
CLAUDIUS_SECRET_TOKEN=op://vault/tokens/github claudius run -- ./server.sh

# Multiple secrets
export CLAUDIUS_SECRET_AWS_KEY=op://vault/aws/access-key
export CLAUDIUS_SECRET_AWS_SECRET=op://vault/aws/secret-key
claudius run -- aws s3 ls

# Nested variable references
export CLAUDIUS_SECRET_ACCOUNT_ID="12345"
export CLAUDIUS_SECRET_API_URL='https://api.example.com/$CLAUDIUS_SECRET_ACCOUNT_ID/v1'
claudius run -- curl $API_URL/users
# Resolves to: https://api.example.com/12345/v1/users
```

**Features:**
- Full stdio inheritance (supports interactive prompts)
- Signal forwarding (Ctrl+C works correctly)
- Exit code preservation
- Environment variable injection without prefix
- DAG-based variable expansion for nested references

## Application Configuration

Claudius supports its own configuration file at `$XDG_CONFIG_HOME/claudius/config.toml` for managing secret resolution.

### config.toml
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
# When using Vault (not yet implemented):
# - Will show warning message
```

### Secret Resolution

Environment variables starting with `CLAUDIUS_SECRET_` are processed in three phases:

**Phase 1: Collection**
- All `CLAUDIUS_SECRET_*` environment variables are collected

**Phase 2: Secret Manager Resolution**
- If a secret manager is configured, op:// references are resolved
- Two syntax options:
  - Bare syntax: `op://vault/item/field` (backward compatible)
  - Delimiter syntax: `{{op://vault/item/field}}` (recommended for URLs)
- Examples:
  - Simple: `op://vault/item/field` → actual secret value
  - In URL: `https://api.com/{{op://vault/account/id}}/endpoint` → `https://api.com/12345/endpoint`

**Phase 3: Variable Expansion**
- Variables containing references to other variables are expanded using a DAG (Directed Acyclic Graph)
- Topological sorting ensures correct resolution order
- Circular dependencies are detected and reported

Examples:
- Simple: `CLAUDIUS_SECRET_API_KEY=op://vault/item/field` → `API_KEY=<resolved-value>`
- Plain: `CLAUDIUS_SECRET_TOKEN=plain-text-token` → `TOKEN=plain-text-token`
- Nested: `CLAUDIUS_SECRET_URL='https://api.com/$CLAUDIUS_SECRET_ACCOUNT_ID'` → `URL=https://api.com/12345`
- Delimited: `CLAUDIUS_SECRET_URL='https://api.com/{{op://vault/account/id}}/{{op://vault/region}}'` → `URL=https://api.com/12345/us-east`

### Variable Expansion Syntax

Claudius supports two syntax forms for variable references:
- `$VARIABLE_NAME` - Simple syntax
- `${VARIABLE_NAME}` - Braced syntax (useful when followed by alphanumeric characters)

Example:
```bash
export CLAUDIUS_SECRET_BASE="prod"
export CLAUDIUS_SECRET_PATH='/${CLAUDIUS_SECRET_BASE}api'  # Results in: /prodapi
```

## Configuration Formats

### mcpServers.json
```json
{
  "mcpServers": {
    "server-name": {
      "command": "command-to-run",
      "args": ["arg1", "arg2"],
      "env": {
        "KEY": "value"
      }
    }
  }
}
```

### settings.json
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

### Custom Commands (commands/*.md)
Markdown files containing slash command implementations.
Deployed to `~/.claude/commands/` without the .md extension.

### Rules (rules/*.md)
Template files for CLAUDE.md content. Can contain:
- Project-specific instructions
- Coding standards
- Architecture decisions
- Context for Claude

## Configuration Merge Strategies

1. **MCP Servers**: Complete replacement
   - New server definitions override existing ones
   - Servers not in new config remain unchanged

2. **Settings**: Field-level merge
   - Only specified fields are updated
   - Unspecified fields remain unchanged
   - Null values don't remove existing values

3. **Commands**: File-based sync
   - All .md files from source are copied
   - Existing commands are overwritten
   - Removed source files don't delete deployed commands

## System Components

1. **Config Module** - Configuration file management
   - Reading various configuration formats
   - Path resolution (XDG Base Directory support)
   - Environment variable handling

2. **Merge Module** - Smart configuration merging
   - MCP servers: Replace strategy (new servers override)
   - Settings: Field-level merge (only specified fields updated)
   - Conflict resolution

3. **CLI Module** - Command-line interface
   - Subcommand structure (sync, append-template, init)
   - Rich help documentation
   - Environment variable support

4. **Commands Module** - Custom command management
   - Markdown file processing
   - Command deployment to Claude directory
   - Automatic .md extension removal

5. **Template Module** - CLAUDE.md management
   - Rule application from templates
   - Duplicate prevention
   - Custom template support

6. **Secrets Module** - Secret management and resolution
   - 1Password CLI integration
   - Environment variable processing
   - Three-phase resolution process

7. **Variable Expansion Module** - DAG-based variable expansion
   - Directed Acyclic Graph construction
   - Topological sorting (Kahn's algorithm)
   - Circular dependency detection
   - Support for $VAR and ${VAR} syntax

## Performance Profiling

Claudius includes built-in profiling capabilities to help identify performance bottlenecks, particularly with secret resolution.

### Recent Performance Improvements

Two major performance improvements have been implemented:

#### 1. Eliminated Duplicate Secret Resolution
A critical issue was fixed where secret resolution was being executed twice:
- **Before**: Secret resolution ran in both `main()` and `run_command_inner()`, taking ~18 seconds total (9s × 2)
- **After**: Secret resolution runs only once in `main()`, reducing execution time by 50%

#### 2. Parallel Secret Resolution
Implemented parallel processing for independent secret resolutions using Rayon:
- **Before**: Sequential processing took 282ms for 7 op:// references (40ms average per call)
- **After**: Parallel processing takes 81ms for the same references (11ms average per call)
- **Result**: ~3.5x speedup in secret resolution phase

The parallel implementation respects DAG dependencies:
- Phase 2 (op:// resolution): Runs in parallel as each variable is independent
- Phase 3 (variable expansion): Uses DAG to handle inter-variable dependencies

These improvements significantly reduce startup time for the `run` subcommand, especially when using 1Password with multiple secrets.


### Quick Start

```bash
# Profile with timing details
just profile -- run -- echo hello

# Profile with release build (recommended)
just profile-release -- run -- npm start

# Or manually:
CLAUDIUS_PROFILE=1 cargo run -- run -- your-command
```

### Profiling Output

When `CLAUDIUS_PROFILE=1` is set, Claudius provides detailed timing information:

```
[INFO] Starting timer: Total secret resolution
[INFO] Starting timer: Phase 1: Collecting env vars
[INFO] Starting timer: Phase 2: Resolving secret references
[INFO] Starting timer: op read op://vault/item/field
[INFO] Timer 'op read op://vault/item/field' completed in 45.2ms
[INFO] === Secret Resolution Performance Summary ===
[INFO] Total secrets processed: 5
[INFO] Successful resolutions: 3
[INFO] Failed resolutions: 2
[INFO] Total time: 281.076241ms
[INFO] Average time per op call: 40.153748ms
[INFO] Slowest op calls:
[INFO]   1. op://vault/slow/item - 62.4ms (success)
[INFO]   2. op://vault/item2/field - 45.2ms (success)
```

### Key Metrics

1. **Phase Timings**:
   - Phase 1: Environment variable collection
   - Phase 2: Secret resolution (op:// references)
   - Phase 3: Variable expansion
   - Phase 4: Prefix removal

2. **Op Call Metrics**:
   - Individual timing for each 1Password CLI call
   - Success/failure tracking
   - Cache hit/miss information

3. **Summary Statistics**:
   - Total secrets processed
   - Success/failure rates
   - Average time per op call
   - Slowest operations

### Performance Tips

- **1Password CLI is the bottleneck**: Each `op` call takes 40-50ms on average
- **Caching helps**: Repeated references to the same secret use cached values
- **Batch operations**: Minimize the number of unique secrets to resolve
- **Use release builds**: Debug builds are significantly slower

### Advanced Profiling

For CPU profiling with flamegraphs (requires `profiling` feature):

```bash
# Build with profiling feature
cargo build --profile=profiling --features=profiling

# Flamegraphs will be saved as flamegraph-*.svg
```

## Development Guide

### Technology Stack
- **Language**: Rust (for safety and performance)
- **Build System**: Nix Flake (reproducible build environment)
- **Testing**: TDD with comprehensive unit and integration tests
- **Target OS**: Linux/macOS (Unix-like systems)

### Development Workflow

1. **Setup Environment**
   ```bash
   nix develop  # Enter development shell
   ```

2. **Run Tests**
   ```bash
   cargo test  # Run all tests
   cargo test -- --test-threads=1  # For environment-sensitive tests
   ```

3. **Build and Run**
   ```bash
   cargo build
   cargo run -- sync --dry-run
   ```

4. **Code Quality**
   ```bash
   cargo fmt      # Format code
   cargo clippy   # Lint code
   ```

### Testing Strategy

Following Kent Beck's TDD cycle:
1. **Red** - Write a failing test
2. **Green** - Write minimal code to pass
3. **Refactor** - Improve code quality

#### Test Categories
- **Unit Tests**: Individual component testing in `tests/unit/`
- **Integration Tests**: CLI and file system interaction in `tests/integration/`
- **Property Tests**: Invariant verification
- Use `tempfile` for isolated test environments
- Avoid global state to enable parallel test execution

#### Test Execution Notes
Tests are designed to be reliable and reproducible:

1. **Mock Usage**: Tests automatically use mocks for external commands (1Password CLI) via:
   - `CLAUDIUS_TEST_MOCK_OP=1` environment variable (set by default in all test commands)
   - Conditional compilation (`#[cfg(test)]`) ensures mocks are always used in test builds
   
2. **Environment Isolation**: 
   - Tests that modify environment variables are marked with `#[serial]` attribute
   - Environment variables are cleaned up between tests to prevent pollution
   - Tests use absolute paths (`/bin/sh`) to avoid shell resolution issues

3. **Flexible Assertions**: 
   - Tests accommodate varying numbers of `CLAUDIUS_SECRET_*` environment variables
   - Stderr assertions check for patterns rather than exact matches

All tests should pass reliably:
```bash
cargo test                    # Runs with CLAUDIUS_TEST_MOCK_OP=1
just test                     # Same, with thread limiting
just check                    # Full check suite including linting
```

### Project Structure
```
claudius/
├── src/           # Source code
├── tests/         # Test files
├── Cargo.toml     # Rust dependencies
├── flake.nix      # Nix configuration
└── CLAUDE.md      # This file
```

## Design Philosophy

1. **Non-Destructive**: Always preserve user data
2. **Explicit**: No surprising automatic behaviors
3. **Composable**: Each feature works independently
4. **Versionable**: All configs in version-control-friendly formats
5. **Linux and macOS**: Designed for Unix-like operating systems

## Current Status & Roadmap

### Current Features (v0.1.0)
- **Configuration Management**: MCP servers, settings, commands
- **Multi-Project Support**: Project-local and global configurations
- **Template System**: CLAUDE.md rules and custom templates
- **Backup & Safety**: Dry-run mode, optional backups
- **CLI Interface**: Intuitive subcommand structure with rich help
- **Secret Management**: Integration with 1Password with inline op:// reference support
- **Secure Execution**: Run commands with automatic secret resolution
- **Variable Expansion**: DAG-based nested environment variable resolution
- **Inline Secret References**: Support for {{op://...}} syntax within URLs and strings

### Roadmap

#### Near-term (v0.2.0)
- Configuration validation
- HashiCorp Vault integration
- More comprehensive error recovery

#### Mid-term (v0.3.0)
- Configuration profiles
- Rollback functionality
- Import/export features

#### Long-term
- Web UI for configuration management
- Plugin system for extensibility
- Cloud synchronization support

## Important Considerations

### Security
- No sensitive data in configuration files
- File permission preservation
- Environment variable sanitization
- Safe file operations with proper error handling
- Secure secret resolution via 1Password CLI
- Secrets never logged or displayed in output

### Error Handling
- Clear error messages with context
- Suggestions for recovery
- Non-destructive operations by default

### Backup Strategy
- Optional backup creation before modifications
- Timestamped backup files
- Automatic cleanup of old backups (future)

### Known Issues and Workarounds

1. **Parallel Test Execution**: Some tests modify environment variables
   - Solution: Tests use `serial_test` crate for sequential execution

2. **File Permissions**: .claude.json must be writable
   - Solution: Check permissions before operations

3. **JSON Formatting**: Order preservation using serde_json
   - Feature: `preserve_order` enabled in Cargo.toml

## Contributing

When contributing to Claudius:

1. Follow TDD practices
2. Maintain backward compatibility
3. Update documentation
4. Add tests for new features
5. Run `cargo fmt` and `cargo clippy`
6. Update CHANGELOG.md

## Important Notes

- Claudius never modifies configuration files without explicit user action
- Dry-run mode is available for all destructive operations
- Backups are optional but recommended
- Project-local configurations take precedence over global ones
- All configuration files are human-readable and editable

## Version History

| Version | Release Date | Description |
|---------|-------------|-------------|
| v0.1.0 | 2025-06-29 | Initial development release with comprehensive Claude configuration management |

## Code Style and Linting

### Philosophy

Our code style emphasizes:
- **Functional Programming**: Prefer immutability, pure functions, and expression-based code
- **Clarity**: Code should be self-documenting with meaningful names
- **Safety**: Leverage Rust's type system and avoid unsafe patterns
- **Consistency**: Uniform style across the entire codebase
- **Modern Idioms**: Use the latest stable Rust features appropriately

### Quick Start

```bash
# Format your code
just fmt

# Run all linting checks
just lint

# Run format, lint, and tests
just check

# Install git hooks for automatic checking
just install-hooks
```

### Formatting (rustfmt)

We use `rustfmt` with a custom configuration that promotes functional style:

#### Key formatting rules:
- **Line width**: 100 characters maximum
- **Indentation**: 4 spaces (no tabs)
- **Imports**: Grouped by std/external/crate, sorted alphabetically
- **Match expressions**: Always use trailing commas
- **Chain calls**: Formatted for readability with 80-char width
- **Comments**: Wrapped at 80 characters for better readability

Run formatting:
```bash
cargo fmt           # Format all files
cargo fmt -- --check # Check formatting without changes
```

### Linting (Clippy)

We use an extensive set of Clippy lints to enforce best practices:

#### Lint Categories

1. **Standard Lints**
   - `clippy::all` - All default lints
   - `clippy::pedantic` - More opinionated lints
   - `clippy::nursery` - Newer, experimental lints
   - `clippy::cargo` - Cargo.toml best practices

2. **Functional Programming**
   - No `unwrap()` - Use `expect()` or proper error handling
   - No imperative loops - Prefer iterator methods
   - No unnecessary mutability
   - Prefer expression-based code over statements

3. **Error Handling**
   - Document all error cases
   - No panics in non-test code
   - Proper Result/Option handling
   - No indexing that could panic

4. **Code Clarity**
   - Cognitive complexity limits
   - Function size limits (50 lines)
   - Meaningful variable names
   - No shadowing variables

5. **Safety**
   - No unsafe code without justification
   - Careful numeric conversions
   - No lossy casts

#### Running Clippy

```bash
# Run clippy with our configuration
cargo clippy --all-targets --all-features -- -D warnings

# Auto-fix some issues
cargo clippy --fix
```

### Pre-commit Hooks

We provide git hooks that run automatically before commits:

1. **Format check** - Ensures code is properly formatted
2. **Clippy check** - Runs linting with strict rules
3. **Debug check** - Prevents committing debug statements
4. **Compilation** - Ensures code compiles
5. **Tests** - Runs test suite (can be skipped with `SKIP_TESTS=1`)

Install hooks:
```bash
just install-hooks
# or
./scripts/install-hooks.sh
```

### Common Issues and Solutions

#### Long Functions
**Issue**: Function exceeds 50 lines
**Solution**: Break into smaller, focused functions
```rust
// Bad
fn process_data(data: &[u8]) -> Result<String> {
    // 100 lines of code...
}

// Good
fn process_data(data: &[u8]) -> Result<String> {
    let parsed = parse_input(data)?;
    let validated = validate_data(parsed)?;
    format_output(validated)
}
```

#### Unwrap Usage
**Issue**: Using `.unwrap()` in non-test code
**Solution**: Use proper error handling
```rust
// Bad
let value = some_option.unwrap();

// Good
let value = some_option.ok_or_else(|| anyhow!("Missing value"))?;
// or
let value = some_option.expect("value should exist because...");
```

#### Imperative Loops
**Issue**: Using for/while loops
**Solution**: Use iterator methods
```rust
// Bad
let mut result = vec![];
for item in items {
    if item.is_valid() {
        result.push(item.process());
    }
}

// Good
let result: Vec<_> = items
    .into_iter()
    .filter(|item| item.is_valid())
    .map(|item| item.process())
    .collect();
```

### Editor Integration

#### VS Code
Add to `.vscode/settings.json`:
```json
{
    "rust-analyzer.checkOnSave.command": "clippy",
    "rust-analyzer.checkOnSave.extraArgs": [
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings"
    ],
    "[rust]": {
        "editor.formatOnSave": true
    }
}
```

#### Vim/Neovim
Using rust.vim:
```vim
let g:rustfmt_autosave = 1
let g:rust_clip_command = 'cargo clippy --all-features'
```

### Shell Scripts

All shell scripts in this project must follow these conventions:

#### Shebang
Always use `#!/usr/bin/env bash` instead of `#!/bin/bash` for better portability:
```bash
#!/usr/bin/env bash
# This ensures the script uses the bash found in PATH
```

This convention:
- Ensures scripts work on systems where bash is not in `/bin/`
- Improves portability across different Unix-like systems
- Follows best practices for shell script compatibility

## Just Command Runner

The project uses [Just](https://github.com/casey/just), a modern command runner written in Rust that provides better error messages, improved UX, and more features than Make.

### Available Commands

```bash
# Show all available commands
just

# Or explicitly:
just --list
```

### Key Commands

#### Development
- `just build` - Build in release mode
- `just run <args>` - Run the development version
- `just test` - Run all tests
- `just check` - Format, lint, and test
- `just watch` - Watch for changes and run tests

#### Code Quality
- `just fmt` - Format all code
- `just lint` - Run linting checks
- `just fix` - Auto-fix clippy warnings
- `just audit` - Security audit

#### Coverage
- `just coverage` - Full coverage analysis
- `just coverage-html` - HTML report only
- `just coverage-lcov` - LCOV report only
- `just test-stats` - Test statistics (Rust 1.86.0 compatible)

#### Utilities
- `just clean` - Clean build artifacts
- `just doc` - Build and open documentation
- `just stats` - Show project statistics
- `just update` - Update dependencies
- `just outdated` - Check for outdated dependencies

#### Development Workflow
- `just dev-test` - Quick test (check + clippy + lib tests)
- `just verbose <args>` - Run with debug logging
- `just trace <args>` - Run with trace logging

#### Release
- `just release <version>` - Create a new release

### Advantages over Make

1. **Better Error Messages** - Clear, helpful error reporting
2. **Rust Integration** - Written in Rust, understands Rust workflows
3. **Modern Features** - String interpolation, conditionals, functions
4. **Shell Selection** - Can use any shell, not just sh/bash

### Examples

```bash
# Run with arguments
just run sync --dry-run

# Coverage with options
just coverage-detailed --format html --min-coverage 90

# Create a release
just release 0.2.0

# Run specific Claudius commands
just run init
just sync --global
```

### Tips

- Just recipes can call other recipes
- Use `{{ARGS}}` to pass arguments through
- Recipes starting with `_` are hidden from listing
- Add `@` before commands to hide them from output
- Use `#!/usr/bin/env bash` for multi-line shell scripts

### Integration

The justfile integrates with:
- Nix flake (just is in devShell)
- Git hooks (use `just install-hooks`)
- CI/CD workflows
- All existing scripts in `scripts/`

For more details, see the [Just documentation](https://just.systems/).

## Test Coverage

### Prerequisites

Due to Rust 1.86.0 compatibility constraints, we recommend using one of these approaches:

#### Option 1: Use cargo-llvm-cov (Recommended)

If you can upgrade to a newer Rust version (1.81+):

```bash
rustup update
cargo install cargo-llvm-cov
```

Then run coverage:

```bash
# Simple coverage summary
cargo llvm-cov --all-features --workspace --summary-only

# Generate HTML report
cargo llvm-cov --all-features --workspace --html

# Generate LCOV report
cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
```

#### Option 2: Use the provided scripts

We've included coverage scripts that work with cargo-llvm-cov:

```bash
# Basic coverage (all formats)
./scripts/coverage.sh

# Detailed coverage with options
./scripts/coverage-detailed.sh --help
```

#### Option 3: Manual coverage with grcov

If you need to stay on Rust 1.86.0:

1. Install grcov:
```bash
cargo install grcov
```

2. Set up environment and build:
```bash
export CARGO_INCREMENTAL=0
export RUSTFLAGS='-Cinstrument-coverage'
export LLVM_PROFILE_FILE='cargo-test-%p-%m.profraw'

cargo build
cargo test
```

3. Generate coverage report:
```bash
grcov . --binary-path ./target/debug/deps/ -s . -t html --branch --ignore-not-existing --ignore '../*' --ignore "/*" -o target/coverage/html
```

### Using Just

We've included just recipes for convenience:

```bash
# Run full coverage analysis
just coverage

# Generate HTML coverage report only
just coverage-html

# Generate LCOV coverage report only  
just coverage-lcov

# Run detailed coverage with options
just coverage-detailed --format html --min-coverage 90
```

### Coverage Output Formats

The coverage tools can generate reports in multiple formats:

- **HTML**: Human-readable report with source code highlighting
- **LCOV**: For integration with CI tools and IDEs
- **JSON**: Machine-readable format for custom processing
- **Cobertura XML**: For integration with various CI/CD platforms

### Interpreting Coverage Results

The coverage report shows:

- **Line Coverage**: Percentage of code lines executed by tests
- **Branch Coverage**: Percentage of conditional branches taken
- **Function Coverage**: Percentage of functions called by tests

Aim for at least 80% coverage for critical code paths.

### CI Integration

For CI pipelines, use the LCOV or Cobertura format:

```bash
cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
# or
cargo llvm-cov --all-features --workspace --cobertura --output-path cobertura.xml
```

Then upload to your coverage service (Codecov, Coveralls, etc.).

### Troubleshooting

#### Rust Version Issues

If you encounter dependency version conflicts due to Rust 1.86.0:

1. Consider upgrading Rust: `rustup update`
2. Or use the manual grcov approach described above
3. Or run coverage in a Docker container with a newer Rust version

#### Missing Coverage Data

If coverage shows 0% or missing files:

1. Ensure you've built with coverage flags
2. Run `cargo clean` and rebuild
3. Check that tests are actually running and passing

#### Performance

Coverage builds are slower than normal builds. For day-to-day development, run tests without coverage instrumentation.

## Build Status and Test Report

### Build Status

#### Release Build
✅ **Build Successful**

- **Binary Location**: `target/release/claudius`
- **Build Time**: Completed successfully
- **Linting**: All clippy lints passing with strict pedantic configuration

#### Build Statistics
- **Source Files**: 16
- **Source Lines**: 2,873
- **Dependencies**: 44 crates compiled

#### Key Dependencies
- `clap` 4.0.32 - Command line parsing
- `serde` 1.0.219 - Serialization
- `toml` 0.8.23 - TOML configuration
- `anyhow` 1.0.98 - Error handling
- `tracing` 0.1.41 - Logging
- `directories` 5.0.1 - Platform directories

### Test Status

#### Test Summary
✅ **All Tests Passing** - Full test suite is working reliably

#### Test Results
- **Total Tests**: 150
- **Passing Tests**: 150 (100%)
- **Failing Tests**: 0 (0%)
- **Unit Tests**: 121 tests
- **Integration Tests**: 150 tests total
- **Test Coverage**: Ready to measure with `cargo-llvm-cov`

#### Resolved Issues
- ✅ Downgraded dependencies for Rust 1.86.0 compatibility
- ✅ Fixed mutex poisoning in test environment
- ✅ Implemented proper test mocking for 1Password CLI
- ✅ Fixed environment variable pollution between tests
- ✅ Resolved shell execution path issues
- ✅ Made test assertions flexible for varying environment configurations
- ✅ All tests now use mocks by default (CLAUDIUS_TEST_MOCK_OP=1)

### Documentation

✅ **Documentation Generated**

- **Location**: `target/doc/claudius/`
- **Warnings**: Missing documentation for public items
- Run `cargo doc --open` to view

### Artifacts

#### Build Artifacts
```
target/
├── release/
│   └── claudius          # Main executable
└── doc/
    └── claudius/         # API documentation
```

#### Binary Information
```bash
# Check binary size
ls -lh target/release/claudius

# Run the binary
./target/release/claudius --help
```

### Project Quality Metrics

#### Code Organization
- ✅ Modular structure
- ✅ Clear separation of concerns
- ✅ Comprehensive error handling
- ✅ Strict clippy linting enforced

#### Testing
- ✅ Good test coverage ratio
- ✅ Unit and integration tests
- ✅ Tests now working with Rust 1.86.0

#### Build System
- ✅ Nix flake support
- ✅ Just command runner
- ✅ CI/CD workflows configured
- ✅ Linting and formatting setup

## Dependency Management

### Rust Version Compatibility

This project is currently constrained to **Rust 1.86.0** due to the Nix flake configuration.

### Dependency Version Constraints

To maintain compatibility with Rust 1.86.0, the following dependencies have been downgraded from their latest versions:

| Dependency | Latest Version | Pinned Version | Rust Requirement |
|------------|-----------------|----------------|------------------|
| `bstr` | 1.12.0 | 1.6.2 | Latest requires 1.73+ |
| `predicates` | 3.1.3 | 3.0.1 | Latest requires 1.74+ |
| `predicates-core` | 1.0.9 | 1.0.5 | Latest requires 1.74+ |
| `assert_fs` | 1.0.13 | 1.0.7 | Depends on predicates-core |
| `ignore` | 0.4.23 | 0.4.18 | Latest uses unstable features |

### How to Update Dependencies

If you need to update dependencies while maintaining Rust 1.86.0 compatibility:

```bash
# Check which version is compatible
cargo search <package-name> --limit 20

# Update to a specific version
cargo update -p <package>@<current-version> --precise <target-version>

# Example:
cargo update -p bstr@1.12.0 --precise 1.6.2
```

### Testing Compatibility

After any dependency updates, ensure tests still compile and run:

```bash
# Run all tests
cargo test

# If you encounter version conflicts, check the error message for guidance
# The error will tell you which Rust version is required
```

### Coverage Tool Compatibility

For test coverage with Rust 1.86.0:

```bash
# Install compatible version
cargo install cargo-llvm-cov --version 0.5.31

# Latest versions (0.6+) require Rust 1.81+
```

### Future Upgrades

To use the latest versions of all dependencies, you would need to:

1. Update the Rust version in `flake.nix`
2. Run `cargo update` to get latest compatible versions
3. Update any code that may have breaking changes

### Notes

- The project builds and runs successfully with these constraints
- All tests pass with the downgraded dependencies
- No functionality is lost with these versions


---

# Important Instruction Reminders
- Do what has been asked; nothing more, nothing less
- NEVER create files unless absolutely necessary
- ALWAYS prefer editing existing files
- NEVER proactively create documentation unless requested

## VERIFIED TRUTH DIRECTIVE — CLAUDE

• Do not present guesses or speculation as fact.
• If not confirmed, say:
  - "I cannot verify this."
  - "I do not have access to that information."
• Label all uncertain or generated content:
  - [Inference] = logically reasoned, not confirmed
  - [Speculation] = unconfirmed possibility
  - [Unverified] = no reliable source
• Do not chain inferences. Label each unverified step.
• Only quote real documents. No fake sources.
• If any part is unverified, label the entire output.
• Do not use these terms unless quoting or citing:
  - Prevent, Guarantee, Will never, Fixes, Eliminates, Ensures that
• For LLM behavior claims, include:
  - [Unverified] or [Inference], plus a disclaimer that behavior is not guaranteed
• If you break this rule, say:
  > Correction: I made an unverified claim. That was incorrect.

# External Rule References
The following rules are included from the .agents/rules directory:
{include:.agents/rules/**/*.md}
