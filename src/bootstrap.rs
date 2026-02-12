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

/// Example skill template
const EXAMPLE_SKILL: &str = r"# Example Skill

This is an example Claude Code skill.

## Usage

Describe how and when to use this skill.

## Implementation

Replace this content with your actual skill definition.
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
# agent = "claude"  # Options: "claude", "claude-code", "codex", "gemini"
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
  "$schema": "https://raw.githubusercontent.com/google-gemini/gemini-cli/main/schemas/settings.schema.json",
  "general": {
    "preferredEditor": "code"
  },
  "ui": {
    "theme": "GitHub",
    "hideTips": false
  },
  "tools": {
    "autoAccept": false,
    "sandbox": false
  },
  "context": {
    "fileName": ["GEMINI.md"]
  },
  "privacy": {
    "usageStatisticsEnabled": true
  },
  "telemetry": {
    "enabled": false
  }
}
"#;

/// Default Codex TOML settings content
const DEFAULT_CODEX_SETTINGS: &str = r#"# Codex Settings
# Configure your Codex CLI settings here

# This file mirrors Codex CLI's `config.toml` schema.
# See the configuration reference for the latest options:
# https://developers.openai.com/codex/config-reference

# Model selection (examples: "gpt-5-codex", "openai/gpt-4.1", "anthropic/claude-3-5-sonnet")
# model = "gpt-5-codex"
# review_model = "gpt-5-codex"

# The model provider to use if not specified in the model name
# model_provider = "openai"

# Approval policy for running commands:
# - "untrusted" | "on-failure" | "on-request" | "never"
# approval_policy = "on-request"

# List of notification channels
# notify = ["desktop", "sound"]

# Model provider configurations
# [model_providers.openai]
# name = "OpenAI"
# base_url = "https://api.openai.com"
# env_key = "OPENAI_API_KEY"

# [model_providers.anthropic]
# name = "Anthropic"
# base_url = "https://api.anthropic.com"
# env_key = "ANTHROPIC_API_KEY"

# Example custom headers
# [model_providers.openai.http_headers]
# X-Custom-Header = "custom-value"

# Example header values sourced from environment variables
# [model_providers.openai.env_http_headers]
# Authorization = "OPENAI_AUTH_HEADER"

# Shell environment policy
# [shell_environment_policy]
# inherit = "all"  # Options: "core", "all", "none"
# ignore_default_excludes = false
# exclude = ["SECRET_*", "PASSWORD_*"]
# set = { TERM = "xterm-256color" }
# experimental_use_profile = false

# Sandbox configuration
# sandbox_mode = "workspace-write"  # "read-only" | "workspace-write" | "danger-full-access"
#
# [sandbox_workspace_write]
# writable_roots = ["/tmp", "/var/tmp"]
# network_access = false
# exclude_tmpdir_env_var = false
# exclude_slash_tmp = false

# History configuration
# [history]
# persistence = "save-all"  # "save-all" | "none"
# max_bytes = 10485760  # optional limit for ~/.codex/history.jsonl

# MCP servers will be merged from mcpServers.json
"#;

/// Default Codex requirements.toml content (admin-enforced constraints)
const DEFAULT_CODEX_REQUIREMENTS: &str = r#"# Codex requirements.toml
# Admin-enforced constraints that users cannot override.
#
# This file mirrors Codex CLI's `requirements.toml` schema.
# See the configuration reference for the latest options:
# https://developers.openai.com/codex/config-reference
#
# Example:
# allowed_approval_policies = ["untrusted", "on-request", "on-failure"]
# allowed_sandbox_modes = ["read-only", "workspace-write"]
"#;

/// Default Codex `managed_config.toml` content (admin-managed defaults)
const DEFAULT_CODEX_MANAGED_CONFIG: &str = r#"# Codex managed_config.toml
# Admin-managed defaults applied when Codex starts.
#
# Managed defaults are starting values that Codex reapplies when it launches. Users can still
# change settings during a session; the managed defaults are restored next time Codex starts.
#
# Managed config location (per Codex docs):
# - Linux/macOS (Unix): /etc/codex/managed_config.toml
# - Windows/non-Unix: ~/.codex/managed_config.toml
#
# See:
# https://developers.openai.com/codex/security#managed-defaults-managed_configtoml
#
# Example:
# approval_policy = "on-request"
# sandbox_mode = "workspace-write"
#
# [sandbox_workspace_write]
# network_access = false
"#;

/// Create a file with content if it doesn't exist or force is true
fn create_file_if_needed(path: &Path, content: &str, force: bool, description: &str) -> Result<()> {
    if force || !path.exists() {
        fs::write(path, content)
            .with_context(|| format!("Failed to create {description}: {}", path.display()))?;
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

fn prompt_yes_no(prompt: &str) -> Result<bool> {
    use std::io::{self, Write};

    eprint!("{prompt}");
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().eq_ignore_ascii_case("y"))
}

fn maybe_backup_context_file(path: &Path, label: &str) -> Result<()> {
    let metadata = fs::metadata(path)?;
    let file_size = metadata.len();

    // Only offer backup if file has substantial content (>100 bytes).
    if file_size <= 100 {
        return Ok(());
    }

    let should_backup = prompt_yes_no(&format!(
        "\nThe {label} file contains {file_size} bytes of data. Create a backup? [y/N]: ",
    ))?;

    if !should_backup {
        return Ok(());
    }

    let backup_path = path.with_extension("md.bak");
    let real_path = metadata
        .is_symlink()
        .then(|| fs::read_link(path))
        .transpose()?
        .unwrap_or_else(|| path.to_path_buf());

    fs::copy(&real_path, &backup_path)
        .with_context(|| format!("Failed to create backup: {}", backup_path.display()))?;
    info!("Created backup: {}", backup_path.display());
    Ok(())
}

/// Initialize MCP servers configuration
fn init_mcp_servers(config_dir: &Path, force: bool) -> Result<()> {
    let mcp_servers_path = config_dir.join("mcpServers.json");
    create_file_if_needed(&mcp_servers_path, DEFAULT_MCP_SERVERS, force, "mcpServers.json")
}

/// Initialize agent-specific settings files
fn init_agent_settings(config_dir: &Path, force: bool) -> Result<()> {
    // Migrate legacy settings.json to claude.settings.json if needed.
    let legacy_settings_path = config_dir.join("settings.json");
    let claude_settings_path = config_dir.join("claude.settings.json");

    if !force && !claude_settings_path.exists() && legacy_settings_path.exists() {
        fs::copy(&legacy_settings_path, &claude_settings_path).with_context(|| {
            format!("Failed to migrate legacy settings.json to {}", claude_settings_path.display())
        })?;
        info!("Migrated legacy settings.json to claude.settings.json");
    } else {
        create_file_if_needed(
            &claude_settings_path,
            DEFAULT_SETTINGS,
            force,
            "claude.settings.json",
        )?;
    }

    let agent_settings = [
        ("codex.settings.toml", DEFAULT_CODEX_SETTINGS),
        ("codex.requirements.toml", DEFAULT_CODEX_REQUIREMENTS),
        ("codex.managed_config.toml", DEFAULT_CODEX_MANAGED_CONFIG),
        ("gemini.settings.json", DEFAULT_GEMINI_SETTINGS),
    ];

    agent_settings.into_iter().try_for_each(|(filename, content)| {
        let settings_path = config_dir.join(filename);
        create_file_if_needed(&settings_path, content, force, filename)
    })?;
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

/// Initialize skills directory with example
fn init_skills_directory(config_dir: &Path, force: bool) -> Result<()> {
    let skills_dir = config_dir.join("skills");
    create_directory(&skills_dir, force)?;

    let example_skill_dir = skills_dir.join("example");
    create_directory(&example_skill_dir, force)?;

    let example_skill_path = example_skill_dir.join("SKILL.md");
    create_file_if_needed(&example_skill_path, EXAMPLE_SKILL, force, "example skill")
}

/// Initialize rules directory with example
fn init_rules_directory(config_dir: &Path, force: bool) -> Result<()> {
    let rules_dir = config_dir.join("rules");
    create_directory(&rules_dir, force)?;

    let example_rule_path = rules_dir.join("example.md");
    create_file_if_needed(&example_rule_path, EXAMPLE_RULE, force, "example rule")
}

/// Initialize context files in project directory based on config
fn init_context_files(target_dir: &Path, default_context: Option<&str>, force: bool) -> Result<()> {
    use std::os::unix::fs as unix_fs;

    // Determine which file is the default context
    let (primary_file, secondary_file) = match default_context {
        Some("AGENTS.md") => ("AGENTS.md", "CLAUDE.md"),
        _ => ("CLAUDE.md", "AGENTS.md"), // Default to CLAUDE.md
    };

    let primary_path = target_dir.join(primary_file);
    let secondary_path = target_dir.join(secondary_file);

    // Check if files exist
    let primary_exists = primary_path.exists();
    let secondary_exists = secondary_path.exists();

    if primary_exists || secondary_exists {
        if !force {
            eprintln!("\nWarning: Context files already exist:");
            if primary_exists {
                eprintln!("  - {primary_file} (primary)");
            }
            if secondary_exists {
                eprintln!("  - {secondary_file} (secondary)");
            }

            if !prompt_yes_no("\nDo you want to overwrite them? [y/N]: ")? {
                info!("Skipping context file initialization");
                return Ok(());
            }
        }

        if primary_exists && !force {
            maybe_backup_context_file(&primary_path, primary_file)?;
        }

        // Remove existing files
        if primary_exists {
            fs::remove_file(&primary_path)
                .with_context(|| format!("Failed to remove {}", primary_path.display()))?;
        }
        if secondary_exists {
            fs::remove_file(&secondary_path)
                .with_context(|| format!("Failed to remove {}", secondary_path.display()))?;
        }
    }

    // Create empty primary context file
    fs::write(&primary_path, "")
        .with_context(|| format!("Failed to create {}", primary_path.display()))?;
    info!("Created {}", primary_file);

    // Create symlink from secondary to primary
    unix_fs::symlink(&primary_path, &secondary_path).with_context(|| {
        format!("Failed to create symlink from {secondary_file} to {primary_file}")
    })?;
    info!("Created symlink: {} -> {}", secondary_file, primary_file);

    Ok(())
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
    init_skills_directory(config_dir, force)?;
    init_rules_directory(config_dir, force)?;

    info!("Bootstrap complete at: {}", config_dir.display());
    Ok(())
}

/// Bootstrap with context file initialization
///
/// # Errors
///
/// Returns an error if:
/// - Unable to create the configuration directory
/// - Unable to create any of the required subdirectories or files
/// - I/O operations fail during initialization
/// - Context file operations fail
pub fn bootstrap_config_with_context(
    config_dir: &Path,
    target_dir: &Path,
    force: bool,
    default_context: Option<&str>,
) -> Result<()> {
    // First do regular bootstrap
    bootstrap_config(config_dir, force)?;

    // Then initialize context files in target directory
    init_context_files(target_dir, default_context, force)?;

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
        assert!(config_dir.join("claude.settings.json").exists());
        assert!(config_dir.join("codex.settings.toml").exists());
        assert!(config_dir.join("codex.requirements.toml").exists());
        assert!(config_dir.join("codex.managed_config.toml").exists());
        assert!(config_dir.join("gemini.settings.json").exists());
        assert!(config_dir.join("settings.json").exists());
        assert!(config_dir.join("config.toml").exists());
        assert!(config_dir.join("skills").exists());
        assert!(config_dir.join("skills/example/SKILL.md").exists());
        assert!(config_dir.join("rules").exists());
        assert!(config_dir.join("rules/example.md").exists());

        // Verify content
        let mcp_content = fs::read_to_string(config_dir.join("mcpServers.json"))
            .expect("mcpServers.json should be readable");
        assert!(mcp_content.contains("filesystem"));
    }

    #[test]
    fn test_bootstrap_migrates_legacy_settings_json() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_dir = temp_dir.path().join("claudius");

        fs::create_dir_all(&config_dir).expect("Failed to create config directory");
        let legacy_content = r#"{"apiKeyHelper":"/bin/legacy-helper","custom":{"nested":true}}"#;
        fs::write(config_dir.join("settings.json"), legacy_content)
            .expect("Failed to write legacy settings.json");

        bootstrap_config(&config_dir, false).expect("bootstrap_config should succeed");

        let migrated = fs::read_to_string(config_dir.join("claude.settings.json"))
            .expect("claude.settings.json should be readable");
        assert_eq!(migrated, legacy_content);

        let legacy = fs::read_to_string(config_dir.join("settings.json"))
            .expect("settings.json should be readable");
        assert_eq!(legacy, legacy_content);
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
        let skills_dir = config_dir.join("skills");
        let custom_skill_dir = skills_dir.join("custom");
        fs::create_dir_all(&custom_skill_dir).expect("Failed to create skills directory");
        fs::write(custom_skill_dir.join("SKILL.md"), "custom skill")
            .expect("Failed to write custom skill");

        let rules_dir = config_dir.join("rules");
        fs::create_dir_all(&rules_dir).expect("Failed to create rules directory");
        fs::write(rules_dir.join("custom.md"), "custom rule").expect("Failed to write custom rule");

        bootstrap_config(&config_dir, true).expect("bootstrap_config with force should succeed");

        // Verify custom files were removed
        assert!(!custom_skill_dir.join("SKILL.md").exists());
        assert!(!rules_dir.join("custom.md").exists());

        // Verify example files exist
        assert!(skills_dir.join("example/SKILL.md").exists());
        assert!(rules_dir.join("example.md").exists());
    }

    #[test]
    fn test_bootstrap_with_context_creates_claude_md() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_dir = temp_dir.path().join("claudius");
        let target_dir = temp_dir.path().join("project");
        fs::create_dir_all(&target_dir).expect("Failed to create target directory");

        // Bootstrap with CLAUDE.md as default (None means CLAUDE.md)
        bootstrap_config_with_context(&config_dir, &target_dir, false, None)
            .expect("bootstrap_config_with_context should succeed");

        // Verify CLAUDE.md exists in target directory
        let claude_md = target_dir.join("CLAUDE.md");
        assert!(claude_md.exists());

        // Verify AGENTS.md is a symlink
        let agents_md = target_dir.join("AGENTS.md");
        assert!(agents_md.exists());
        let metadata = fs::symlink_metadata(&agents_md).expect("Failed to get metadata");
        assert!(metadata.is_symlink());
    }

    #[test]
    fn test_bootstrap_with_context_creates_agents_md() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_dir = temp_dir.path().join("claudius");
        let target_dir = temp_dir.path().join("project");
        fs::create_dir_all(&target_dir).expect("Failed to create target directory");

        // Bootstrap with AGENTS.md as default
        bootstrap_config_with_context(&config_dir, &target_dir, false, Some("AGENTS.md"))
            .expect("bootstrap_config_with_context should succeed");

        // Verify AGENTS.md exists in target directory
        let agents_md = target_dir.join("AGENTS.md");
        assert!(agents_md.exists());

        // Verify CLAUDE.md is a symlink
        let claude_md = target_dir.join("CLAUDE.md");
        assert!(claude_md.exists());
        let metadata = fs::symlink_metadata(&claude_md).expect("Failed to get metadata");
        assert!(metadata.is_symlink());
    }

    #[test]
    fn test_init_context_files_respects_symlinks() {
        use std::os::unix::fs as unix_fs;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let target_dir = temp_dir.path().join("project");
        fs::create_dir_all(&target_dir).expect("Failed to create target directory");

        // Create CLAUDE.md with content
        let claude_md = target_dir.join("CLAUDE.md");
        let content = "# Original Content\n\nThis is original content with more than 100 bytes to trigger backup prompt. ".repeat(3);
        fs::write(&claude_md, content).expect("Failed to write CLAUDE.md");

        // Create AGENTS.md as symlink to CLAUDE.md
        let agents_md = target_dir.join("AGENTS.md");
        unix_fs::symlink(&claude_md, &agents_md).expect("Failed to create symlink");

        // Run init_context_files with force=true (to bypass prompts)
        init_context_files(&target_dir, None, true).expect("init_context_files should succeed");

        // Verify both files exist
        assert!(claude_md.exists());
        assert!(agents_md.exists());

        // Verify AGENTS.md is still a symlink
        let metadata = fs::symlink_metadata(&agents_md).expect("Failed to get metadata");
        assert!(metadata.is_symlink());
    }
}
