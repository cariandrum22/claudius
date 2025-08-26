#![allow(missing_docs)]

use crate::app_config::{Agent, AppConfig};
use crate::codex_settings::{convert_mcp_to_toml, CodexSettings};
use crate::commands;
use crate::config::{reader, writer, ClaudeConfig, Config, McpServersConfig, Settings};
use crate::merge::{merge_configs, merge_settings, strategy::MergeStrategy};
use crate::validation::{pre_validate_settings, prompt_continue};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Configuration for sync operation
#[derive(Debug)]
pub struct SyncConfig {
    pub config_opt: Option<PathBuf>,
    pub dry_run: bool,
    pub backup: bool,
    pub target_config_opt: Option<PathBuf>,
    pub global: bool,
    pub agent_override: Option<Agent>,
    pub app_config: Option<AppConfig>,
}

/// Result of reading configurations
#[derive(Debug)]
pub struct ReadConfigResult {
    pub mcp_servers: McpServersConfig,
    pub settings: Option<Settings>,
    pub codex_settings: Option<CodexSettings>,
}

/// Agent context for sync operation
#[derive(Debug, Clone, Copy)]
pub struct AgentContext {
    pub agent: Option<Agent>,
    pub is_codex: bool,
    pub is_gemini: bool,
    pub is_claude: bool,
}

impl AgentContext {
    #[must_use]
    pub const fn new(agent: Option<Agent>) -> Self {
        let is_codex = matches!(agent, Some(Agent::Codex));
        let is_gemini = matches!(agent, Some(Agent::Gemini));
        let is_claude = matches!(agent, Some(Agent::Claude)) || agent.is_none();

        Self { agent, is_codex, is_gemini, is_claude }
    }
}

/// Determine the agent to use based on override and app config
#[must_use]
pub fn determine_agent(
    agent_override: Option<Agent>,
    app_config: Option<&AppConfig>,
) -> Option<Agent> {
    agent_override
        .or_else(|| app_config.and_then(|c| c.default.as_ref()).map(|d| d.agent))
        .or(Some(Agent::Claude))
}

/// Read all necessary configurations
///
/// # Errors
///
/// Returns an error if:
/// - Unable to read MCP servers configuration
/// - Unable to read settings configuration
/// - Settings validation fails
pub fn read_configurations(
    config: &Config,
    mcp_servers_path: &Path,
    agent_context: AgentContext,
) -> Result<ReadConfigResult> {
    // Read MCP servers
    debug!("Reading MCP servers configuration");
    let mcp_servers = reader::read_mcp_servers_config(mcp_servers_path)
        .context("Failed to read MCP servers configuration")?;

    debug!("Found {} MCP server(s) to sync", mcp_servers.mcp_servers.len());
    for name in mcp_servers.mcp_servers.keys() {
        debug!("  - {name}");
    }

    // Read settings based on agent type
    let (settings, codex_settings) = if agent_context.is_codex {
        read_codex_settings(&config.settings_path)?
    } else {
        read_regular_settings(&config.settings_path)?
    };

    Ok(ReadConfigResult { mcp_servers, settings, codex_settings })
}

/// Read Codex-specific settings
fn read_codex_settings(settings_path: &Path) -> Result<(Option<Settings>, Option<CodexSettings>)> {
    let codex_settings =
        reader::read_codex_settings(settings_path).context("Failed to read Codex TOML settings")?;

    if let Some(ref cs) = codex_settings {
        debug!("Found codex.settings.toml to sync");
        log_codex_settings_fields(cs);
    }

    Ok((None, codex_settings))
}

/// Read regular settings with validation
fn read_regular_settings(
    settings_path: &Path,
) -> Result<(Option<Settings>, Option<CodexSettings>)> {
    // Validate first
    let validation_result =
        pre_validate_settings(settings_path).context("Failed to validate settings file")?;

    // Display warnings
    if !validation_result.warnings.is_empty() {
        warn!("Configuration validation warnings:");
        for warning in &validation_result.warnings {
            warn!("  - {}", warning);
        }
    }

    // Read settings
    let settings =
        reader::read_settings(settings_path).context("Failed to read settings configuration")?;

    if let Some(ref s) = settings {
        let filename = settings_path
            .file_name()
            .map_or_else(|| "settings file".into(), |n| n.to_string_lossy());
        debug!("Found {} to sync", filename);
        log_settings_fields(s);
    }

    Ok((settings, None))
}

/// Log fields present in Codex settings
fn log_codex_settings_fields(cs: &CodexSettings) {
    if cs.model.is_some() {
        debug!("  - model");
    }
    if cs.model_provider.is_some() {
        debug!("  - model_provider");
    }
    if cs.approval_policy.is_some() {
        debug!("  - approval_policy");
    }
    if cs.model_providers.is_some() {
        debug!("  - model_providers");
    }
    if cs.sandbox.is_some() {
        debug!("  - sandbox");
    }
    if cs.shell_environment_policy.is_some() {
        debug!("  - shell_environment_policy");
    }
}

/// Log fields present in regular settings
fn log_settings_fields(s: &Settings) {
    if s.api_key_helper.is_some() {
        debug!("  - apiKeyHelper");
    }
    if s.cleanup_period_days.is_some() {
        debug!("  - cleanupPeriodDays");
    }
    if s.env.is_some() {
        debug!("  - env");
    }
    if s.include_co_authored_by.is_some() {
        debug!("  - includeCoAuthoredBy");
    }
    if s.permissions.is_some() {
        debug!("  - permissions");
    }
}

/// Create backup if requested and file exists
///
/// # Errors
///
/// Returns an error if:
/// - Backup creation fails
/// - User cancels operation after backup failure
pub fn handle_backup(backup: bool, target_config_path: &Path) -> Result<()> {
    if backup && target_config_path.exists() {
        debug!("Creating backup of configuration file");
        match writer::backup_file(target_config_path) {
            Ok(Some(backup_path)) => {
                debug!("Backup created: {backup_path}");
            },
            Ok(None) => {
                debug!("No backup needed (file doesn't exist)");
            },
            Err(e) => {
                warn!("Failed to create backup: {e}");
                if !prompt_continue()? {
                    return Err(anyhow::anyhow!("Operation cancelled by user"));
                }
            },
        }
    }
    Ok(())
}

/// Merge configurations and settings
///
/// # Errors
///
/// Returns an error if merging configurations fails
pub fn merge_all_configs(
    claude_config: &mut ClaudeConfig,
    read_result: &ReadConfigResult,
    agent_context: AgentContext,
    global: bool,
) -> Result<()> {
    // Merge MCP servers
    debug!("Merging configurations");
    let original_count =
        claude_config.mcp_servers.as_ref().map_or(0, std::collections::HashMap::len);
    merge_configs(claude_config, &read_result.mcp_servers, MergeStrategy::default())?;
    let new_count = claude_config.mcp_servers.as_ref().map_or(0, std::collections::HashMap::len);
    debug!("Merged configuration: {} -> {} server(s)", original_count, new_count);

    // Merge settings for Claude in global mode
    if global && agent_context.is_claude {
        if let Some(ref settings) = read_result.settings {
            debug!("Merging settings");
            merge_settings(claude_config, settings)?;
            debug!("Settings merged successfully");
        }
    }
    // For non-Claude in global mode (except Gemini), merge settings only
    else if global && !agent_context.is_gemini && !agent_context.is_claude {
        if let Some(ref settings) = read_result.settings {
            debug!("Merging settings");
            merge_settings(claude_config, settings)?;
            debug!("Settings merged successfully");
        }
    }

    Ok(())
}

/// Handle dry run output
///
/// # Errors
///
/// Returns an error if serialization fails
pub fn handle_dry_run(
    claude_config: &ClaudeConfig,
    read_result: &ReadConfigResult,
    agent_context: AgentContext,
    global: bool,
) -> Result<()> {
    info!("Dry run mode - not writing changes");

    if global {
        println!("\n--- Result (dry run) ---");
        println!("{}", serde_json::to_string_pretty(&claude_config)?);
    } else {
        print_project_local_dry_run(claude_config, read_result, agent_context)?;
    }

    Ok(())
}

/// Print dry run output for project-local mode
fn print_project_local_dry_run(
    claude_config: &ClaudeConfig,
    read_result: &ReadConfigResult,
    agent_context: AgentContext,
) -> Result<()> {
    if agent_context.is_claude {
        print_claude_dry_run(claude_config, read_result.settings.as_ref())?;
    } else if agent_context.is_codex {
        print_codex_dry_run(claude_config, read_result.codex_settings.as_ref())?;
    } else {
        print_other_agent_dry_run(
            claude_config,
            read_result.settings.as_ref(),
            agent_context.is_gemini,
        )?;
    }
    Ok(())
}

/// Print Claude-specific dry run output
fn print_claude_dry_run(claude_config: &ClaudeConfig, settings: Option<&Settings>) -> Result<()> {
    let settings_location = ".claude/settings.json";
    println!("\n--- Settings with MCP servers ({settings_location}) ---");

    if let Some(mut settings_copy) = settings.cloned() {
        settings_copy.mcp_servers.clone_from(&claude_config.mcp_servers);
        println!("{}", serde_json::to_string_pretty(&settings_copy)?);
    } else {
        let new_settings = create_settings_with_mcp_servers(claude_config);
        println!("{}", serde_json::to_string_pretty(&new_settings)?);
    }
    Ok(())
}

/// Print Codex-specific dry run output
fn print_codex_dry_run(
    claude_config: &ClaudeConfig,
    codex_settings: Option<&CodexSettings>,
) -> Result<()> {
    let settings_location = ".claude/settings.toml";
    println!("\n--- Settings with MCP servers ({settings_location}) ---");

    if let Some(mut codex_settings_copy) = codex_settings.cloned() {
        if let Some(ref mcp_servers) = claude_config.mcp_servers {
            codex_settings_copy.mcp_servers = Some(convert_mcp_to_toml(mcp_servers));
        }
        println!("{}", toml::to_string_pretty(&codex_settings_copy)?);
    } else {
        let new_codex_settings = create_codex_settings_with_mcp_servers(claude_config);
        println!("{}", toml::to_string_pretty(&new_codex_settings)?);
    }
    Ok(())
}

/// Print dry run output for non-Claude, non-Codex agents
fn print_other_agent_dry_run(
    claude_config: &ClaudeConfig,
    settings: Option<&Settings>,
    is_gemini: bool,
) -> Result<()> {
    // Print MCP servers
    println!("\n--- MCP servers (.mcp.json) ---");
    let mcp_only =
        McpServersConfig { mcp_servers: claude_config.mcp_servers.clone().unwrap_or_default() };
    println!("{}", serde_json::to_string_pretty(&mcp_only)?);

    // Print settings if present
    if let Some(settings_ref) = settings {
        let settings_location =
            if is_gemini { "./gemini/settings.json" } else { ".claude/settings.json" };
        println!("\n--- Settings ({settings_location}) ---");
        println!("{}", serde_json::to_string_pretty(&settings_ref)?);
    }

    Ok(())
}

/// Create a new Settings struct with MCP servers
fn create_settings_with_mcp_servers(claude_config: &ClaudeConfig) -> Settings {
    Settings {
        api_key_helper: None,
        cleanup_period_days: None,
        env: None,
        include_co_authored_by: None,
        permissions: None,
        preferred_notif_channel: None,
        mcp_servers: claude_config.mcp_servers.clone(),
        extra: HashMap::new(),
    }
}

/// Merge source Codex settings into existing settings
/// Only non-None fields from source override existing fields
fn merge_codex_settings(target: &mut CodexSettings, source: &CodexSettings) {
    if source.model.is_some() {
        target.model.clone_from(&source.model);
    }
    if source.model_provider.is_some() {
        target.model_provider.clone_from(&source.model_provider);
    }
    if source.approval_policy.is_some() {
        target.approval_policy.clone_from(&source.approval_policy);
    }
    if source.disable_response_storage.is_some() {
        target.disable_response_storage = source.disable_response_storage;
    }
    if source.notify.is_some() {
        target.notify.clone_from(&source.notify);
    }
    if source.model_providers.is_some() {
        target.model_providers.clone_from(&source.model_providers);
    }
    if source.shell_environment_policy.is_some() {
        target.shell_environment_policy.clone_from(&source.shell_environment_policy);
    }
    if source.sandbox.is_some() {
        target.sandbox.clone_from(&source.sandbox);
    }
    if source.history.is_some() {
        target.history.clone_from(&source.history);
    }
    // Note: mcp_servers are handled separately in the calling function
    // Merge extra fields
    for (key, value) in &source.extra {
        target.extra.insert(key.clone(), value.clone());
    }
}

/// Create a new `CodexSettings` struct with MCP servers
fn create_codex_settings_with_mcp_servers(claude_config: &ClaudeConfig) -> CodexSettings {
    let mut codex_settings = CodexSettings {
        model: None,
        model_provider: None,
        approval_policy: None,
        disable_response_storage: None,
        notify: None,
        model_providers: None,
        shell_environment_policy: None,
        sandbox: None,
        history: None,
        mcp_servers: None,
        extra: HashMap::new(),
    };

    if let Some(ref mcp_servers) = claude_config.mcp_servers {
        codex_settings.mcp_servers = Some(convert_mcp_to_toml(mcp_servers));
    }

    codex_settings
}

/// Write configurations to disk
///
/// # Errors
///
/// Returns an error if:
/// - Unable to write configuration files
/// - Unable to create parent directories
/// - Serialization fails
pub fn write_configurations(
    config: &Config,
    claude_config: &ClaudeConfig,
    target_config_path: &Path,
    read_result: &ReadConfigResult,
    agent_context: AgentContext,
    global: bool,
) -> Result<()> {
    if global {
        write_global_configurations(claude_config, target_config_path, read_result, agent_context)?;
    } else {
        write_project_local_configurations(
            config,
            claude_config,
            target_config_path,
            read_result,
            agent_context,
        )?;
    }

    info!("Configuration updated successfully");
    Ok(())
}

/// Write configurations in global mode
fn write_global_configurations(
    claude_config: &ClaudeConfig,
    target_config_path: &Path,
    read_result: &ReadConfigResult,
    agent_context: AgentContext,
) -> Result<()> {
    if agent_context.is_gemini {
        write_gemini_global(claude_config, target_config_path, read_result.settings.as_ref())?;
    } else if agent_context.is_codex {
        write_codex_global(claude_config, target_config_path, read_result.codex_settings.as_ref())?;
    } else {
        info!("Writing updated configuration");
        writer::write_claude_config(target_config_path, claude_config)
            .context("Failed to write Claude configuration")?;
    }
    Ok(())
}

/// Write Codex configuration in global mode
fn write_codex_global(
    claude_config: &ClaudeConfig,
    _target_config_path: &Path,
    codex_settings: Option<&CodexSettings>,
) -> Result<()> {
    // For Codex, we write everything to ~/.codex/config.toml
    let codex_config_path = directories::BaseDirs::new()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .home_dir()
        .join(".codex")
        .join("config.toml");

    // Read existing Codex configuration from target location
    let existing_codex = reader::read_codex_settings(&codex_config_path)
        .context("Failed to read existing Codex settings")?;

    // Start with existing settings if they exist, otherwise use source settings or create new
    let mut codex_to_write = existing_codex
        .or_else(|| codex_settings.cloned())
        .unwrap_or_else(|| create_codex_settings_with_mcp_servers(claude_config));

    // Merge source settings into existing settings (if source settings exist)
    if let Some(source_settings) = codex_settings {
        merge_codex_settings(&mut codex_to_write, source_settings);
    }

    // Merge MCP servers - combine existing and new servers
    if let Some(ref new_mcp_servers) = claude_config.mcp_servers {
        let new_toml_servers = convert_mcp_to_toml(new_mcp_servers);

        if let Some(existing_mcp) = codex_to_write.mcp_servers.as_mut() {
            // Merge new servers into existing ones
            for (name, server) in new_toml_servers {
                existing_mcp.insert(name, server);
            }
        } else {
            // No existing servers, just use new ones
            codex_to_write.mcp_servers = Some(new_toml_servers);
        }
    }

    info!("Writing settings to ~/.codex/config.toml");

    if let Some(parent) = codex_config_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create .codex directory")?;
    }

    writer::write_codex_settings(&codex_config_path, &codex_to_write)
        .context("Failed to write Codex settings")?;

    Ok(())
}

/// Write Gemini configuration in global mode
fn write_gemini_global(
    claude_config: &ClaudeConfig,
    target_config_path: &Path,
    settings: Option<&Settings>,
) -> Result<()> {
    info!("Writing MCP servers to target configuration");
    writer::write_claude_config(target_config_path, claude_config)
        .context("Failed to write configuration")?;

    if let Some(settings_ref) = settings {
        let gemini_settings_path = directories::BaseDirs::new()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
            .home_dir()
            .join(".gemini")
            .join("settings.json");

        info!("Writing settings to ~/.gemini/settings.json");

        if let Some(parent) = gemini_settings_path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create .gemini directory")?;
        }

        writer::write_settings(&gemini_settings_path, settings_ref)
            .context("Failed to write ~/.gemini/settings.json")?;
    }
    Ok(())
}

/// Write configurations in project-local mode
fn write_project_local_configurations(
    config: &Config,
    claude_config: &ClaudeConfig,
    target_config_path: &Path,
    read_result: &ReadConfigResult,
    agent_context: AgentContext,
) -> Result<()> {
    if agent_context.is_claude {
        write_claude_project_local(config, claude_config, read_result.settings.as_ref())?;
    } else if agent_context.is_codex {
        write_codex_project_local(config, claude_config, read_result.codex_settings.as_ref())?;
    } else {
        write_other_agent_project_local(
            config,
            claude_config,
            target_config_path,
            read_result.settings.as_ref(),
        )?;
    }
    Ok(())
}

/// Write Claude configuration in project-local mode
fn write_claude_project_local(
    config: &Config,
    claude_config: &ClaudeConfig,
    settings: Option<&Settings>,
) -> Result<()> {
    // Write MCP servers to .mcp.json
    if let Some(ref mcp_servers) = claude_config.mcp_servers {
        info!("Writing MCP servers to .mcp.json");
        let mcp_config = McpServersConfig { mcp_servers: mcp_servers.clone() };
        writer::write_mcp_servers_config(&config.target_config_path, &mcp_config)
            .context("Failed to write .mcp.json")?;
    }

    // Write settings (without MCP servers) to .claude/settings.json
    if let Some(ref settings_path) = config.project_settings_path {
        if let Some(mut settings_to_write) = settings.cloned() {
            // Remove MCP servers from settings - they go in .mcp.json
            settings_to_write.mcp_servers = None;

            // Check if we have any actual settings to write
            let has_settings = settings_to_write.api_key_helper.is_some()
                || settings_to_write.cleanup_period_days.is_some()
                || settings_to_write.env.is_some()
                || settings_to_write.include_co_authored_by.is_some()
                || settings_to_write.permissions.is_some()
                || settings_to_write.preferred_notif_channel.is_some();

            if has_settings {
                info!("Writing settings to .claude/settings.json");
                ensure_parent_directory_exists(settings_path)?;
                writer::write_settings(settings_path, &settings_to_write)
                    .context("Failed to write .claude/settings.json")?;
            }
        }
    }
    Ok(())
}

/// Write Codex configuration in project-local mode
fn write_codex_project_local(
    config: &Config,
    claude_config: &ClaudeConfig,
    codex_settings: Option<&CodexSettings>,
) -> Result<()> {
    let mut codex_to_write = codex_settings
        .map_or_else(|| create_codex_settings_with_mcp_servers(claude_config), Clone::clone);

    if let Some(ref mcp_servers) = claude_config.mcp_servers {
        codex_to_write.mcp_servers = Some(convert_mcp_to_toml(mcp_servers));
    }

    if let Some(ref settings_path) = config.project_settings_path {
        info!("Writing merged settings and MCP servers to {}", settings_path.display());
        ensure_parent_directory_exists(settings_path)?;
        writer::write_codex_settings(settings_path, &codex_to_write)
            .with_context(|| format!("Failed to write {}", settings_path.display()))?;
    }
    Ok(())
}

/// Write configuration for non-Claude, non-Codex agents in project-local mode
fn write_other_agent_project_local(
    config: &Config,
    claude_config: &ClaudeConfig,
    target_config_path: &Path,
    settings: Option<&Settings>,
) -> Result<()> {
    // Write MCP servers
    info!("Writing MCP servers to .mcp.json");
    let mcp_only =
        McpServersConfig { mcp_servers: claude_config.mcp_servers.clone().unwrap_or_default() };
    writer::write_mcp_servers_config(target_config_path, &mcp_only)
        .context("Failed to write .mcp.json")?;

    // Write settings if present
    if let Some(settings_ref) = settings {
        if let Some(ref settings_path) = config.project_settings_path {
            info!("Writing settings to {}", settings_path.display());
            ensure_parent_directory_exists(settings_path)?;
            writer::write_settings(settings_path, settings_ref)
                .context("Failed to write settings")?;
        }
    }
    Ok(())
}

/// Ensure parent directory exists
fn ensure_parent_directory_exists(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create parent directory")?;
    }
    Ok(())
}

/// Sync custom commands
pub fn sync_commands_if_exists(config: &Config) {
    if config.commands_dir.exists() {
        debug!("Syncing custom slash commands");
        debug!("Source: {}", config.commands_dir.display());
        debug!("Target: {}", config.claude_commands_dir.display());

        match commands::sync_commands(&config.commands_dir, &config.claude_commands_dir) {
            Ok(synced) => {
                if !synced.is_empty() {
                    info!("Synced {} custom command(s)", synced.len());
                    for cmd in &synced {
                        debug!("  - {}", cmd);
                    }
                }
            },
            Err(e) => {
                warn!("Failed to sync commands: {}", e);
            },
        }
    }
}
