use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use tracing::info;

/// Default mcpServers.json template
const DEFAULT_MCP_SERVERS: &str = r#"{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem"],
      "env": {}
    }
  }
}
"#;

/// Default settings.json template
const DEFAULT_SETTINGS: &str = r#"{
  "apiKeyHelper": null,
  "cleanupPeriodDays": 30,
  "env": {},
  "includeCoAuthoredBy": true,
  "permissions": {
    "allow": [],
    "deny": [],
    "defaultMode": null
  },
  "preferredNotifChannel": null
}
"#;

/// Example command template
const EXAMPLE_COMMAND: &str = r"# Example Command

This is an example custom slash command for Claude.

## Usage

Type `/example` in Claude to use this command.

## Implementation

Replace this content with your actual command implementation.
";

/// Example rule template
const EXAMPLE_RULE: &str = r"# Example Rule

This is an example rule template for CLAUDE.md.

## Guidelines

- Always follow project conventions
- Write clear, maintainable code
- Document your changes

## Project-Specific Instructions

Add your project-specific instructions here.
";

/// Example config.toml template
const EXAMPLE_CONFIG: &str = r#"# Claudius Configuration File
# This file configures the Claudius application itself

# [default]
# Default settings that can be overridden by command-line arguments
# agent = "claude"  # Options: "claude", "codex", "gemini"
# context-file = "CONTEXT.md"  # Custom context file name (overrides agent defaults)

# [secret-manager]
# Configure a secret manager to resolve environment variables
# Supported types: "vault", "1password"
#
# Example for 1Password:
# type = "1password"
#
# When using 1Password, environment variables starting with CLAUDIUS_SECRET_*
# that contain values starting with op:// will be resolved using 1Password CLI.
# For example:
#   CLAUDIUS_SECRET_API_KEY=op://vault/item/field
# Will be resolved and made available as API_KEY environment variable.
#
# Example for HashiCorp Vault (not yet implemented):
# type = "vault"
"#;

/// Bootstrap Claudius configuration directory with default files
/// Default Gemini settings content
const DEFAULT_GEMINI_SETTINGS: &str = r#"{
  "contextFileName": "GEMINI.md",
  "autoAccept": false,
  "theme": "Default",
  "sandbox": false,
  "checkpointing": {
    "enabled": false
  },
  "telemetry": {
    "enabled": false
  },
  "usageStatisticsEnabled": true,
  "hideTips": false
}
"#;

/// Default Codex TOML settings content
const DEFAULT_CODEX_SETTINGS: &str = r#"# Codex Settings
# Configure your Codex CLI settings here

# The default model to use (e.g., "openai/gpt-4", "anthropic/claude-3-5-sonnet-20241022")
# model = "openai/gpt-4"

# The model provider to use if not specified in the model name
# model_provider = "openai"

# Approval policy for commands: "none", "required", or custom script path
# approval_policy = "none"

# Whether to disable response storage
# disable_response_storage = false

# List of notification channels
# notify = ["desktop", "sound"]

# Model provider configurations
# [model_providers.openai]
# base_url = "https://api.openai.com"
# api_key_env = "OPENAI_API_KEY"

# [model_providers.anthropic]
# base_url = "https://api.anthropic.com"
# api_key_env = "ANTHROPIC_API_KEY"

# Shell environment policy
# [shell_environment_policy]
# inherit = "all"  # Options: "all", "none", "login"
# ignore_default_excludes = false
# exclude = ["SECRET_*", "PASSWORD_*"]
# set = { TERM = "xterm-256color" }

# Sandbox configuration
# [sandbox]
# mode = "none"  # Options: "none", "docker", "firejail"
# writable_roots = ["/tmp", "/var/tmp"]
# network_access = true

# History configuration
# [history]
# persistence = "disk"  # Options: "disk", "memory", "none"

# MCP servers will be merged from mcpServers.json
"#;

/// Create a file with content if it doesn't exist or force is true
fn create_file_if_needed(path: &Path, content: &str, force: bool, description: &str) -> Result<()> {
    if force || !path.exists() {
        fs::write(path, content)
            .with_context(|| format!("Failed to create {}: {}", description, path.display()))?;
        info!("Created {}", description);
    } else {
        info!("{} already exists, skipping", description);
    }
    Ok(())
}

/// Create a directory, optionally removing it first if force is true
fn create_directory(path: &Path, force: bool) -> Result<()> {
    if force && path.exists() {
        fs::remove_dir_all(path)
            .with_context(|| format!("Failed to remove directory: {}", path.display()))?;
    }
    fs::create_dir_all(path)
        .with_context(|| format!("Failed to create directory: {}", path.display()))?;
    Ok(())
}

/// Initialize MCP servers configuration
fn init_mcp_servers(config_dir: &Path, force: bool) -> Result<()> {
    let mcp_servers_path = config_dir.join("mcpServers.json");
    create_file_if_needed(&mcp_servers_path, DEFAULT_MCP_SERVERS, force, "mcpServers.json")
}

/// Initialize agent-specific settings files
fn init_agent_settings(config_dir: &Path, force: bool) -> Result<()> {
    let agent_settings = vec![
        ("claude.settings.json", DEFAULT_SETTINGS),
        ("codex.settings.toml", DEFAULT_CODEX_SETTINGS),
        ("gemini.settings.json", DEFAULT_GEMINI_SETTINGS),
    ];

    for (filename, content) in agent_settings {
        let settings_path = config_dir.join(filename);
        create_file_if_needed(&settings_path, content, force, filename)?;
    }
    Ok(())
}

/// Create legacy settings.json for backward compatibility
fn create_legacy_settings(config_dir: &Path, force: bool) -> Result<()> {
    let legacy_settings_path = config_dir.join("settings.json");
    let claude_settings_path = config_dir.join("claude.settings.json");

    if (force || !legacy_settings_path.exists()) && claude_settings_path.exists() {
        fs::copy(&claude_settings_path, &legacy_settings_path).with_context(|| {
            format!("Failed to create legacy settings.json: {}", legacy_settings_path.display())
        })?;
        info!("Created legacy settings.json for backward compatibility");
    }
    Ok(())
}

/// Initialize app configuration
fn init_app_config(config_dir: &Path, force: bool) -> Result<()> {
    let app_config_path = config_dir.join("config.toml");
    create_file_if_needed(&app_config_path, EXAMPLE_CONFIG, force, "config.toml")
}

/// Initialize commands directory with example
fn init_commands_directory(config_dir: &Path, force: bool) -> Result<()> {
    let commands_dir = config_dir.join("commands");
    create_directory(&commands_dir, force)?;

    let example_command_path = commands_dir.join("example.md");
    create_file_if_needed(&example_command_path, EXAMPLE_COMMAND, force, "example command")
}

/// Initialize rules directory with example
fn init_rules_directory(config_dir: &Path, force: bool) -> Result<()> {
    let rules_dir = config_dir.join("rules");
    create_directory(&rules_dir, force)?;

    let example_rule_path = rules_dir.join("example.md");
    create_file_if_needed(&example_rule_path, EXAMPLE_RULE, force, "example rule")
}

/// Bootstrap Claudius configuration directory with all necessary files
///
/// # Errors
///
/// Returns an error if:
/// - Unable to create the configuration directory
/// - Unable to create any of the required subdirectories or files
/// - I/O operations fail during initialization
pub fn bootstrap_config(config_dir: &Path, force: bool) -> Result<()> {
    // Create main config directory
    fs::create_dir_all(config_dir)
        .with_context(|| format!("Failed to create config directory: {}", config_dir.display()))?;

    // Initialize all components
    init_mcp_servers(config_dir, force)?;
    init_agent_settings(config_dir, force)?;
    create_legacy_settings(config_dir, force)?;
    init_app_config(config_dir, force)?;
    init_commands_directory(config_dir, force)?;
    init_rules_directory(config_dir, force)?;

    info!("Bootstrap complete at: {}", config_dir.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_bootstrap_creates_structure() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_dir = temp_dir.path().join("claudius");

        bootstrap_config(&config_dir, false).expect("bootstrap_config should succeed");

        // Check all files and directories exist
        assert!(config_dir.exists());
        assert!(config_dir.join("mcpServers.json").exists());
        assert!(config_dir.join("settings.json").exists());
        assert!(config_dir.join("config.toml").exists());
        assert!(config_dir.join("commands").exists());
        assert!(config_dir.join("commands/example.md").exists());
        assert!(config_dir.join("rules").exists());
        assert!(config_dir.join("rules/example.md").exists());

        // Verify content
        let mcp_content = fs::read_to_string(config_dir.join("mcpServers.json"))
            .expect("mcpServers.json should be readable");
        assert!(mcp_content.contains("filesystem"));
    }

    #[test]
    fn test_bootstrap_preserves_existing() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_dir = temp_dir.path().join("claudius");

        // Create existing file with custom content
        fs::create_dir_all(&config_dir).expect("Failed to create config directory");
        let custom_content = r#"{"custom": "content"}"#;
        fs::write(config_dir.join("mcpServers.json"), custom_content)
            .expect("Failed to write custom content");

        bootstrap_config(&config_dir, false).expect("bootstrap_config should succeed");

        // Verify existing file was preserved
        let content = fs::read_to_string(config_dir.join("mcpServers.json"))
            .expect("mcpServers.json should be readable");
        assert_eq!(content, custom_content);
    }

    #[test]
    fn test_bootstrap_force_overwrites() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_dir = temp_dir.path().join("claudius");

        // Create existing file with custom content
        fs::create_dir_all(&config_dir).expect("Failed to create config directory");
        let custom_content = r#"{"custom": "content"}"#;
        fs::write(config_dir.join("mcpServers.json"), custom_content)
            .expect("Failed to write custom content");

        bootstrap_config(&config_dir, true).expect("bootstrap_config with force should succeed");

        // Verify file was overwritten
        let content = fs::read_to_string(config_dir.join("mcpServers.json"))
            .expect("mcpServers.json should be readable");
        assert!(content.contains("filesystem"));
        assert!(!content.contains("custom"));
    }

    #[test]
    fn test_bootstrap_force_cleans_directories() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_dir = temp_dir.path().join("claudius");

        // Create existing structure with custom files
        let commands_dir = config_dir.join("commands");
        fs::create_dir_all(&commands_dir).expect("Failed to create commands directory");
        fs::write(commands_dir.join("custom.md"), "custom command")
            .expect("Failed to write custom command");

        let rules_dir = config_dir.join("rules");
        fs::create_dir_all(&rules_dir).expect("Failed to create rules directory");
        fs::write(rules_dir.join("custom.md"), "custom rule").expect("Failed to write custom rule");

        bootstrap_config(&config_dir, true).expect("bootstrap_config with force should succeed");

        // Verify custom files were removed
        assert!(!commands_dir.join("custom.md").exists());
        assert!(!rules_dir.join("custom.md").exists());

        // Verify example files exist
        assert!(commands_dir.join("example.md").exists());
        assert!(rules_dir.join("example.md").exists());
    }
}
