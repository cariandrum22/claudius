#![allow(missing_docs)]

use crate::agent_paths;
use crate::app_config::{Agent, AppConfig, ClaudeCodeScope};
use crate::codex_settings::{convert_mcp_to_toml, CodexSettings, ModelProvider};
use crate::commands;
use crate::config::{reader, writer, ClaudeConfig, Config, McpServersConfig, Settings};
use crate::json_merge::deep_merge_json_maps;
use crate::merge::{merge_configs, merge_settings, strategy::MergeStrategy};
use crate::validation::{pre_validate_settings, prompt_continue};
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use toml::Value as TomlValue;
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

/// Optional extra files for Codex in global mode
#[derive(Debug, Clone, Copy, Default)]
pub struct CodexGlobalSyncOptions {
    pub requirements: bool,
    pub managed_config: bool,
}

/// Agent context for sync operation
#[derive(Debug, Clone, Copy)]
pub struct AgentContext {
    pub agent: Option<Agent>,
    pub claude_code_scope: Option<ClaudeCodeScope>,
    pub is_codex: bool,
    pub is_gemini: bool,
    pub is_claude: bool,
    pub is_claude_desktop: bool,
    pub is_claude_code: bool,
}

impl AgentContext {
    #[must_use]
    pub fn new(agent: Option<Agent>, claude_code_scope: Option<ClaudeCodeScope>) -> Self {
        let is_codex = matches!(agent, Some(Agent::Codex));
        let is_gemini = matches!(agent, Some(Agent::Gemini));
        let is_claude_desktop = matches!(agent, Some(Agent::Claude)) || agent.is_none();
        let is_claude_code = matches!(agent, Some(Agent::ClaudeCode));
        let is_claude = is_claude_desktop || is_claude_code;

        let effective_claude_code_scope = is_claude_code.then_some(claude_code_scope).flatten();

        Self {
            agent,
            claude_code_scope: effective_claude_code_scope,
            is_codex,
            is_gemini,
            is_claude,
            is_claude_desktop,
            is_claude_code,
        }
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
    } else if agent_context.is_claude_desktop {
        (None, None)
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
pub fn handle_backup(
    config: &Config,
    target_config_path: &Path,
    read_result: &ReadConfigResult,
    agent_context: AgentContext,
    codex_global: CodexGlobalSyncOptions,
) -> Result<()> {
    let backup_paths =
        collect_backup_paths(config, target_config_path, read_result, agent_context, codex_global)?;

    for path in backup_paths {
        if !path.exists() {
            continue;
        }

        debug!("Creating backup of {}", path.display());
        match writer::backup_file(&path) {
            Ok(Some(backup_path)) => {
                debug!("Backup created: {backup_path}");
            },
            Ok(None) => {
                debug!("No backup needed (file doesn't exist)");
            },
            Err(e) => {
                warn!("Failed to create backup for {}: {e}", path.display());
                if !prompt_continue()? {
                    return Err(anyhow::anyhow!("Operation cancelled by user"));
                }
            },
        }
    }

    Ok(())
}

fn collect_backup_paths(
    config: &Config,
    target_config_path: &Path,
    read_result: &ReadConfigResult,
    agent_context: AgentContext,
    codex_global: CodexGlobalSyncOptions,
) -> Result<Vec<PathBuf>> {
    // Codex writes to a TOML file, not the JSON `target_config_path` in project-local mode.
    if agent_context.is_codex && !config.is_global {
        return Ok(config.project_settings_path.iter().cloned().collect());
    }

    let mut paths = vec![target_config_path.to_path_buf()];

    if codex_global.requirements && agent_context.is_codex && config.is_global {
        paths.push(agent_paths::codex_requirements_path());
    }

    if codex_global.managed_config && agent_context.is_codex && config.is_global {
        paths.push(agent_paths::codex_managed_config_path());
    }

    if agent_context.is_claude_code && config.is_global {
        paths.push(claude_code_settings_path(agent_context.claude_code_scope)?);
        return Ok(paths);
    }

    if agent_context.is_claude_code
        && !config.is_global
        && agent_context.claude_code_scope == Some(ClaudeCodeScope::Local)
        && should_write_claude_project_settings(read_result)
    {
        paths.push(claude_code_local_settings_path()?);
        return Ok(paths);
    }

    if agent_context.is_claude
        && !config.is_global
        && should_write_claude_project_settings(read_result)
    {
        if let Some(settings_path) = config.project_settings_path.as_ref() {
            paths.push(settings_path.clone());
        }
    }

    Ok(paths)
}

fn should_write_claude_project_settings(read_result: &ReadConfigResult) -> bool {
    let Some(settings) = read_result.settings.as_ref() else {
        return false;
    };

    let mut settings_to_write = settings.clone();
    settings_to_write.mcp_servers = None;

    settings_to_write.api_key_helper.is_some()
        || settings_to_write.cleanup_period_days.is_some()
        || settings_to_write.env.is_some()
        || settings_to_write.include_co_authored_by.is_some()
        || settings_to_write.permissions.is_some()
        || settings_to_write.preferred_notif_channel.is_some()
        || !settings_to_write.extra.is_empty()
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
    if agent_context.is_claude_code
        && !global
        && agent_context.claude_code_scope == Some(ClaudeCodeScope::Local)
    {
        merge_claude_code_local_mcp_servers(claude_config, &read_result.mcp_servers)?;
    } else {
        let original_count =
            claude_config.mcp_servers.as_ref().map_or(0, std::collections::HashMap::len);
        merge_configs(claude_config, &read_result.mcp_servers, MergeStrategy::default())?;
        let new_count =
            claude_config.mcp_servers.as_ref().map_or(0, std::collections::HashMap::len);
        debug!("Merged configuration: {} -> {} server(s)", original_count, new_count);
    }

    if agent_context.is_gemini {
        if let Some(settings) = read_result.settings.as_ref() {
            debug!("Merging Gemini settings");
            merge_gemini_settings_into_config(claude_config, settings)?;
            debug!("Gemini settings merged successfully");
        }
    }

    // For non-Claude agents in global mode (except Codex), merge settings into the target JSON.
    // Claude Desktop uses a dedicated config file containing MCP servers. Claude Code stores
    // settings separately in ~/.claude/settings.json. Codex stores settings in ~/.codex/config.toml.
    if global
        && !agent_context.is_codex
        && !agent_context.is_claude
        && !agent_context.is_claude_code
        && !agent_context.is_claude_desktop
    {
        if let Some(ref settings) = read_result.settings {
            debug!("Merging settings");
            merge_settings(claude_config, settings)?;
            debug!("Settings merged successfully");
        }
    }

    Ok(())
}

fn merge_claude_code_local_mcp_servers(
    claude_config: &mut ClaudeConfig,
    mcp_servers: &McpServersConfig,
) -> Result<()> {
    let project_dir = std::env::current_dir().context("Failed to determine current directory")?;
    let project_key = project_dir.to_string_lossy().to_string();

    let mut project_config = claude_config
        .other
        .get(&project_key)
        .cloned()
        .and_then(|value| serde_json::from_value::<ClaudeConfig>(value).ok())
        .unwrap_or(ClaudeConfig { mcp_servers: None, other: HashMap::new() });

    let original_count =
        project_config.mcp_servers.as_ref().map_or(0, std::collections::HashMap::len);
    merge_configs(&mut project_config, mcp_servers, MergeStrategy::default())?;
    let new_count = project_config.mcp_servers.as_ref().map_or(0, std::collections::HashMap::len);

    debug!(
        "Merged local MCP servers for {}: {} -> {} server(s)",
        project_key, original_count, new_count
    );

    claude_config.other.insert(
        project_key,
        serde_json::to_value(&project_config).context("Failed to serialize project MCP config")?,
    );

    Ok(())
}

fn merge_gemini_settings_into_config(
    claude_config: &mut ClaudeConfig,
    settings: &Settings,
) -> Result<()> {
    if let Some(mcp_servers) = settings.mcp_servers.as_ref() {
        let settings_mcp = McpServersConfig { mcp_servers: mcp_servers.clone() };
        merge_configs(claude_config, &settings_mcp, MergeStrategy::default())?;
    }

    let mut settings_value: Value = serde_json::to_value(settings)
        .context("Failed to serialize Gemini settings for merging")?;
    let Value::Object(map) = &mut settings_value else {
        return Ok(());
    };

    map.remove("mcpServers");
    migrate_legacy_gemini_settings(map);

    let overlay: HashMap<String, Value> =
        map.iter().map(|(key, value)| (key.clone(), value.clone())).collect();
    deep_merge_json_maps(&mut claude_config.other, &overlay);

    Ok(())
}

fn migrate_legacy_gemini_settings(map: &mut serde_json::Map<String, Value>) {
    // Migrate legacy (v1) Gemini CLI keys into the current category-based schema.
    //
    // This is best-effort and only runs when the legacy key exists. If the target category exists
    // but isn't an object, we leave the legacy key untouched to avoid clobbering user data.
    migrate_key_into_category(map, "contextFileName", "context", "fileName");
    migrate_key_into_category(map, "bugCommand", "advanced", "bugCommand");
    migrate_key_into_category(map, "fileFiltering", "context", "fileFiltering");
    migrate_key_into_category(map, "coreTools", "tools", "core");
    migrate_key_into_category(map, "excludeTools", "tools", "exclude");
    migrate_key_into_category(map, "autoAccept", "tools", "autoAccept");
    migrate_key_into_category(map, "theme", "ui", "theme");
    migrate_key_into_category(map, "hideTips", "ui", "hideTips");
    migrate_key_into_category(map, "sandbox", "tools", "sandbox");
    migrate_key_into_category(map, "toolDiscoveryCommand", "tools", "discoveryCommand");
    migrate_key_into_category(map, "toolCallCommand", "tools", "callCommand");
    migrate_key_into_category(map, "checkpointing", "general", "checkpointing");
    migrate_key_into_category(map, "preferredEditor", "general", "preferredEditor");
    migrate_key_into_category(map, "usageStatisticsEnabled", "privacy", "usageStatisticsEnabled");
}

fn migrate_key_into_category(
    map: &mut serde_json::Map<String, Value>,
    legacy_key: &str,
    category_key: &str,
    new_key: &str,
) {
    let Some(value) = map.get(legacy_key).cloned() else {
        return;
    };

    let should_remove_legacy_key = match map.get_mut(category_key) {
        Some(Value::Object(category_map)) => {
            category_map.entry(new_key.to_string()).or_insert(value);
            true
        },
        Some(_) => false,
        None => {
            let mut category_map = serde_json::Map::new();
            category_map.insert(new_key.to_string(), value);
            map.insert(category_key.to_string(), Value::Object(category_map));
            true
        },
    };

    if should_remove_legacy_key {
        map.remove(legacy_key);
    }
}

/// Handle dry run output
///
/// # Errors
///
/// Returns an error if serialization fails
pub fn handle_dry_run(
    config: &Config,
    target_config_path: &Path,
    claude_config: &ClaudeConfig,
    read_result: &ReadConfigResult,
    agent_context: AgentContext,
    codex_global: CodexGlobalSyncOptions,
) -> Result<()> {
    info!("Dry run mode - not writing changes");

    if config.is_global {
        print_global_dry_run(
            target_config_path,
            claude_config,
            read_result,
            agent_context,
            codex_global.requirements,
            codex_global.managed_config,
        )?;
    } else if agent_context.is_claude_code
        && agent_context.claude_code_scope == Some(ClaudeCodeScope::Local)
    {
        print_claude_code_local_dry_run(
            target_config_path,
            claude_config,
            read_result.settings.as_ref(),
        )?;
    } else {
        print_project_local_dry_run(claude_config, read_result, agent_context)?;
    }

    Ok(())
}

fn print_global_dry_run(
    target_config_path: &Path,
    claude_config: &ClaudeConfig,
    read_result: &ReadConfigResult,
    agent_context: AgentContext,
    include_codex_requirements: bool,
    include_codex_managed_config: bool,
) -> Result<()> {
    if agent_context.is_codex {
        print_codex_global_dry_run(
            target_config_path,
            claude_config,
            read_result.codex_settings.as_ref(),
            include_codex_requirements,
            include_codex_managed_config,
        )?;
        return Ok(());
    }

    if agent_context.is_claude_code {
        print_claude_code_global_dry_run(
            target_config_path,
            claude_config,
            read_result.settings.as_ref(),
            agent_context.claude_code_scope,
        )?;
        return Ok(());
    }

    println!("\n--- Result (dry run): {} ---", target_config_path.display());
    println!("{}", serde_json::to_string_pretty(&claude_config)?);
    Ok(())
}

/// Print dry run output for project-local mode
fn print_project_local_dry_run(
    claude_config: &ClaudeConfig,
    read_result: &ReadConfigResult,
    agent_context: AgentContext,
) -> Result<()> {
    if agent_context.is_codex {
        print_codex_dry_run(claude_config, read_result.codex_settings.as_ref())?;
    } else if agent_context.is_gemini {
        print_gemini_dry_run(claude_config)?;
    } else {
        print_other_agent_dry_run(claude_config, read_result.settings.as_ref())?;
    }
    Ok(())
}

fn print_gemini_dry_run(claude_config: &ClaudeConfig) -> Result<()> {
    println!("\n--- Gemini settings (.gemini/settings.json) ---");
    println!("{}", serde_json::to_string_pretty(&claude_config)?);
    Ok(())
}

fn print_claude_code_global_dry_run(
    target_config_path: &Path,
    claude_config: &ClaudeConfig,
    source_settings: Option<&Settings>,
    scope: Option<ClaudeCodeScope>,
) -> Result<()> {
    let claude_settings_path = claude_code_settings_path(scope)?;
    let settings_to_write =
        build_claude_code_settings_to_write(&claude_settings_path, source_settings)?;

    println!("\n--- MCP servers ({}) ---", target_config_path.display());
    println!("{}", serde_json::to_string_pretty(&claude_config)?);

    println!("\n--- Settings ({}) ---", claude_settings_path.display());
    println!("{}", serde_json::to_string_pretty(&settings_to_write)?);

    Ok(())
}

fn print_claude_code_local_dry_run(
    target_config_path: &Path,
    claude_config: &ClaudeConfig,
    source_settings: Option<&Settings>,
) -> Result<()> {
    let local_settings_path = claude_code_local_settings_path()?;
    let settings_to_write =
        build_claude_code_settings_to_write(&local_settings_path, source_settings)?;

    println!("\n--- MCP servers ({}) ---", target_config_path.display());
    println!("{}", serde_json::to_string_pretty(&claude_config)?);

    println!("\n--- Settings ({}) ---", local_settings_path.display());
    println!("{}", serde_json::to_string_pretty(&settings_to_write)?);

    Ok(())
}

/// Print Codex-specific dry run output
fn print_codex_dry_run(
    claude_config: &ClaudeConfig,
    codex_settings: Option<&CodexSettings>,
) -> Result<()> {
    let settings_location = ".codex/config.toml";
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

fn print_codex_global_dry_run(
    target_config_path: &Path,
    claude_config: &ClaudeConfig,
    codex_settings: Option<&CodexSettings>,
    include_codex_requirements: bool,
    include_codex_managed_config: bool,
) -> Result<()> {
    println!("\n--- Settings with MCP servers ({}) ---", target_config_path.display());

    let codex_to_write =
        build_codex_settings_for_global(target_config_path, claude_config, codex_settings)?;
    println!("{}", toml::to_string_pretty(&codex_to_write)?);

    if include_codex_requirements {
        let (_, requirements) = read_codex_requirements_from_config_dir()?;
        let target_path = agent_paths::codex_requirements_path();

        println!("\n--- requirements.toml ({}) ---", target_path.display());
        println!("{}", toml::to_string_pretty(&requirements)?);
    }

    if include_codex_managed_config {
        let (_, managed_config) = read_codex_managed_config_from_config_dir()?;
        let target_path = agent_paths::codex_managed_config_path();

        println!("\n--- managed_config.toml ({}) ---", target_path.display());
        println!("{}", toml::to_string_pretty(&managed_config)?);
    }
    Ok(())
}

fn read_codex_requirements_from_config_dir() -> Result<(PathBuf, TomlValue)> {
    let config_dir =
        Config::get_config_dir().context("Failed to determine Claudius config directory")?;

    let preferred_path = config_dir.join("codex.requirements.toml");
    let legacy_path = config_dir.join("requirements.toml");

    let source_path = if preferred_path.exists() {
        preferred_path.clone()
    } else if legacy_path.exists() {
        legacy_path.clone()
    } else {
        preferred_path.clone()
    };

    if !source_path.exists() {
        return Err(anyhow::anyhow!(
            "Codex requirements file not found. Create {} (preferred) or {}",
            preferred_path.display(),
            legacy_path.display(),
        ));
    }

    let content = std::fs::read_to_string(&source_path)
        .with_context(|| format!("Failed to read Codex requirements: {}", source_path.display()))?;
    let requirements: TomlValue = toml::from_str(&content).with_context(|| {
        format!("Failed to parse Codex requirements TOML: {}", source_path.display())
    })?;

    Ok((source_path, requirements))
}

fn write_codex_requirements() -> Result<()> {
    let (source_path, requirements) = read_codex_requirements_from_config_dir()?;
    let target_path = agent_paths::codex_requirements_path();

    info!("Writing Codex requirements from {} to {}", source_path.display(), target_path.display(),);

    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)
            .context("Failed to create requirements.toml parent directory")?;
    }

    std::fs::write(&target_path, toml::to_string_pretty(&requirements)?)
        .with_context(|| format!("Failed to write {}", target_path.display()))?;

    Ok(())
}

fn read_codex_managed_config_from_config_dir() -> Result<(PathBuf, TomlValue)> {
    let config_dir =
        Config::get_config_dir().context("Failed to determine Claudius config directory")?;

    let preferred_path = config_dir.join("codex.managed_config.toml");
    let legacy_path = config_dir.join("managed_config.toml");

    let source_path = if preferred_path.exists() {
        preferred_path.clone()
    } else if legacy_path.exists() {
        legacy_path.clone()
    } else {
        preferred_path.clone()
    };

    if !source_path.exists() {
        return Err(anyhow::anyhow!(
            "Codex managed_config file not found. Create {} (preferred) or {}",
            preferred_path.display(),
            legacy_path.display(),
        ));
    }

    let content = std::fs::read_to_string(&source_path).with_context(|| {
        format!("Failed to read Codex managed_config: {}", source_path.display())
    })?;
    let managed_config: TomlValue = toml::from_str(&content).with_context(|| {
        format!("Failed to parse Codex managed_config TOML: {}", source_path.display())
    })?;

    Ok((source_path, managed_config))
}

fn write_codex_managed_config() -> Result<()> {
    let (source_path, managed_config) = read_codex_managed_config_from_config_dir()?;
    let target_path = agent_paths::codex_managed_config_path();

    info!(
        "Writing Codex managed_config from {} to {}",
        source_path.display(),
        target_path.display(),
    );

    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)
            .context("Failed to create managed_config.toml parent directory")?;
    }

    std::fs::write(&target_path, toml::to_string_pretty(&managed_config)?)
        .with_context(|| format!("Failed to write {}", target_path.display()))?;

    Ok(())
}

/// Print dry run output for non-Claude, non-Codex agents
fn print_other_agent_dry_run(
    claude_config: &ClaudeConfig,
    settings: Option<&Settings>,
) -> Result<()> {
    // Print MCP servers
    println!("\n--- MCP servers (.mcp.json) ---");
    let mcp_only =
        McpServersConfig { mcp_servers: claude_config.mcp_servers.clone().unwrap_or_default() };
    println!("{}", serde_json::to_string_pretty(&mcp_only)?);

    // Print settings if present
    if let Some(settings_ref) = settings {
        let mut settings_copy = settings_ref.clone();
        settings_copy.mcp_servers = None;

        let settings_location = ".claude/settings.json";
        println!("\n--- Settings ({settings_location}) ---");
        println!("{}", serde_json::to_string_pretty(&settings_copy)?);
    }

    Ok(())
}

/// Merge source Codex settings into existing settings
/// Only non-None fields from source override existing fields
fn merge_codex_settings(target: &mut CodexSettings, source: &CodexSettings) {
    merge_codex_simple_fields(target, source);
    merge_codex_model_providers(target, source);
    merge_codex_extra_fields(target, source);
}

fn merge_option<T: Clone>(target: &mut Option<T>, source: Option<&T>) {
    let Some(value) = source else {
        return;
    };

    match target.as_mut() {
        Some(existing) => existing.clone_from(value),
        None => {
            *target = Some(value.clone());
        },
    }
}

fn merge_codex_simple_fields(target: &mut CodexSettings, source: &CodexSettings) {
    merge_option(&mut target.model, source.model.as_ref());
    merge_option(&mut target.review_model, source.review_model.as_ref());
    merge_option(&mut target.model_provider, source.model_provider.as_ref());
    merge_option(&mut target.model_context_window, source.model_context_window.as_ref());
    merge_option(&mut target.approval_policy, source.approval_policy.as_ref());
    merge_option(&mut target.disable_response_storage, source.disable_response_storage.as_ref());
    merge_option(&mut target.notify, source.notify.as_ref());
    merge_option(&mut target.shell_environment_policy, source.shell_environment_policy.as_ref());
    merge_option(&mut target.sandbox_mode, source.sandbox_mode.as_ref());
    merge_option(&mut target.sandbox_workspace_write, source.sandbox_workspace_write.as_ref());
    merge_option(&mut target.sandbox, source.sandbox.as_ref());
    merge_option(&mut target.history, source.history.as_ref());
}

fn merge_codex_model_providers(target: &mut CodexSettings, source: &CodexSettings) {
    let Some(source_providers) = source.model_providers.as_ref() else {
        return;
    };

    let mut merged_providers = target.model_providers.take().unwrap_or_default();

    for (name, provider) in source_providers {
        merged_providers
            .entry(name.clone())
            .and_modify(|existing| merge_model_provider(existing, provider))
            .or_insert_with(|| provider.clone());
    }

    target.model_providers = Some(merged_providers);
}

fn merge_codex_extra_fields(target: &mut CodexSettings, source: &CodexSettings) {
    // Note: `mcp_servers` are handled separately in the calling function.
    // Deep-merge tables to avoid dropping unknown nested keys.
    source.extra.iter().for_each(|(key, value)| {
        target
            .extra
            .entry(key.clone())
            .and_modify(|existing| deep_merge_toml_value(existing, value))
            .or_insert_with(|| value.clone());
    });
}

fn merge_model_provider(target: &mut ModelProvider, source: &ModelProvider) {
    if source.name.is_some() {
        target.name.clone_from(&source.name);
    }

    if source.base_url.is_some() {
        target.base_url.clone_from(&source.base_url);
    }

    if source.env_key.is_some() {
        target.env_key.clone_from(&source.env_key);
    }

    if let Some(source_headers) = source.http_headers.as_ref() {
        let mut merged_headers = target.http_headers.take().unwrap_or_default();

        for (key, value) in source_headers {
            merged_headers.insert(key.clone(), value.clone());
        }

        target.http_headers = Some(merged_headers);
    }

    if let Some(source_headers) = source.env_http_headers.as_ref() {
        let mut merged_headers = target.env_http_headers.take().unwrap_or_default();

        for (key, value) in source_headers {
            merged_headers.insert(key.clone(), value.clone());
        }

        target.env_http_headers = Some(merged_headers);
    }

    if let Some(source_query_params) = source.query_params.as_ref() {
        let mut merged_query_params = target.query_params.take().unwrap_or_default();

        for (key, value) in source_query_params {
            merged_query_params.insert(key.clone(), value.clone());
        }

        target.query_params = Some(merged_query_params);
    }

    if source.wire_api.is_some() {
        target.wire_api.clone_from(&source.wire_api);
    }

    if source.requires_openai_auth.is_some() {
        target.requires_openai_auth = source.requires_openai_auth;
    }

    deep_merge_toml_maps(&mut target.extra, &source.extra);
}

/// Create a new `CodexSettings` struct with MCP servers
fn create_codex_settings_with_mcp_servers(claude_config: &ClaudeConfig) -> CodexSettings {
    let mut codex_settings = CodexSettings {
        model: None,
        review_model: None,
        model_provider: None,
        model_context_window: None,
        approval_policy: None,
        disable_response_storage: None,
        notify: None,
        model_providers: None,
        shell_environment_policy: None,
        sandbox_mode: None,
        sandbox_workspace_write: None,
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
    codex_global: CodexGlobalSyncOptions,
) -> Result<()> {
    if config.is_global {
        write_global_configurations(
            claude_config,
            target_config_path,
            read_result,
            agent_context,
            codex_global.requirements,
            codex_global.managed_config,
        )?;
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
    include_codex_requirements: bool,
    include_codex_managed_config: bool,
) -> Result<()> {
    if agent_context.is_claude_code {
        write_claude_code_global(
            claude_config,
            target_config_path,
            read_result.settings.as_ref(),
            agent_context.claude_code_scope,
        )?;
    } else if agent_context.is_codex {
        write_codex_global(claude_config, target_config_path, read_result.codex_settings.as_ref())?;
        if include_codex_requirements {
            write_codex_requirements()?;
        }
        if include_codex_managed_config {
            write_codex_managed_config()?;
        }
    } else {
        info!("Writing updated configuration");
        writer::write_claude_config(target_config_path, claude_config)
            .context("Failed to write Claude configuration")?;
    }
    Ok(())
}

/// Write Claude Code configuration in global mode
fn write_claude_code_global(
    claude_config: &ClaudeConfig,
    target_config_path: &Path,
    settings: Option<&Settings>,
    scope: Option<ClaudeCodeScope>,
) -> Result<()> {
    let claude_settings_path = claude_code_settings_path(scope)?;
    let settings_to_write = build_claude_code_settings_to_write(&claude_settings_path, settings)?;

    info!("Writing MCP servers to {}", target_config_path.display());
    writer::write_claude_config(target_config_path, claude_config)
        .context("Failed to write ~/.claude.json")?;

    info!("Writing settings to {}", claude_settings_path.display());
    if let Some(parent) = claude_settings_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create ~/.claude directory")?;
    }
    writer::write_settings(&claude_settings_path, &settings_to_write)
        .context("Failed to write ~/.claude/settings.json")?;

    Ok(())
}

fn claude_code_settings_path(scope: Option<ClaudeCodeScope>) -> Result<PathBuf> {
    if matches!(scope, Some(ClaudeCodeScope::Managed)) {
        return Ok(agent_paths::claude_code_managed_settings_path());
    }

    let base_dirs = directories::BaseDirs::new()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    Ok(base_dirs.home_dir().join(".claude").join("settings.json"))
}

fn claude_code_local_settings_path() -> Result<PathBuf> {
    let project_dir = std::env::current_dir().context("Failed to determine current directory")?;
    Ok(project_dir.join(".claude").join("settings.local.json"))
}

fn build_claude_code_settings_to_write(
    settings_path: &Path,
    source_settings: Option<&Settings>,
) -> Result<Settings> {
    let existing_settings = reader::read_settings(settings_path)
        .context("Failed to read existing Claude Code settings")?;

    let mut settings_to_write = match (existing_settings, source_settings) {
        (Some(mut existing), Some(source)) => {
            merge_claude_code_settings(&mut existing, source);
            existing
        },
        (Some(existing), None) => existing,
        (None, Some(source)) => source.clone(),
        (None, None) => Settings {
            api_key_helper: None,
            cleanup_period_days: None,
            env: None,
            include_co_authored_by: None,
            permissions: None,
            preferred_notif_channel: None,
            mcp_servers: None,
            extra: HashMap::new(),
        },
    };

    settings_to_write.mcp_servers = None;
    Ok(settings_to_write)
}

/// Merge Claude Code settings (field by field merge)
fn merge_claude_code_settings(target: &mut Settings, source: &Settings) {
    if source.api_key_helper.is_some() {
        target.api_key_helper.clone_from(&source.api_key_helper);
    }
    if source.cleanup_period_days.is_some() {
        target.cleanup_period_days = source.cleanup_period_days;
    }
    if source.env.is_some() {
        target.env.clone_from(&source.env);
    }
    if source.include_co_authored_by.is_some() {
        target.include_co_authored_by = source.include_co_authored_by;
    }
    if source.permissions.is_some() {
        target.permissions.clone_from(&source.permissions);
    }
    if source.preferred_notif_channel.is_some() {
        target.preferred_notif_channel.clone_from(&source.preferred_notif_channel);
    }

    deep_merge_json_maps(&mut target.extra, &source.extra);
}

/// Write Codex configuration in global mode
fn write_codex_global(
    claude_config: &ClaudeConfig,
    target_config_path: &Path,
    codex_settings: Option<&CodexSettings>,
) -> Result<()> {
    let codex_to_write =
        build_codex_settings_for_global(target_config_path, claude_config, codex_settings)?;
    info!("Writing settings to {}", target_config_path.display());

    if let Some(parent) = target_config_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create .codex directory")?;
    }

    writer::write_codex_settings(target_config_path, &codex_to_write)
        .context("Failed to write Codex settings")?;

    Ok(())
}

fn build_codex_settings_for_global(
    target_config_path: &Path,
    claude_config: &ClaudeConfig,
    codex_settings: Option<&CodexSettings>,
) -> Result<CodexSettings> {
    // Read existing Codex configuration from target location.
    let existing_codex = reader::read_codex_settings(target_config_path)
        .context("Failed to read existing Codex settings")?;

    // Start with existing settings if they exist, otherwise use source settings or create new.
    let mut codex_to_write = existing_codex
        .or_else(|| codex_settings.cloned())
        .unwrap_or_else(|| create_codex_settings_with_mcp_servers(claude_config));

    // Merge source settings into existing settings (if source settings exist).
    if let Some(source_settings) = codex_settings {
        merge_codex_settings(&mut codex_to_write, source_settings);
    }

    // Merge MCP servers from (existing target) + (source settings) + (mcpServers.json).
    let mut merged_mcp_servers = codex_to_write.mcp_servers.take().unwrap_or_default();

    if let Some(source_settings) = codex_settings {
        if let Some(source_mcp_servers) = source_settings.mcp_servers.as_ref() {
            deep_merge_toml_maps(&mut merged_mcp_servers, source_mcp_servers);
        }
    }

    if let Some(ref new_mcp_servers) = claude_config.mcp_servers {
        let new_toml_servers = convert_mcp_to_toml(new_mcp_servers);
        deep_merge_toml_maps(&mut merged_mcp_servers, &new_toml_servers);
    }

    codex_to_write.mcp_servers = (!merged_mcp_servers.is_empty()).then_some(merged_mcp_servers);
    Ok(codex_to_write)
}

/// Write configurations in project-local mode
fn write_project_local_configurations(
    config: &Config,
    claude_config: &ClaudeConfig,
    target_config_path: &Path,
    read_result: &ReadConfigResult,
    agent_context: AgentContext,
) -> Result<()> {
    if agent_context.is_claude_code
        && agent_context.claude_code_scope == Some(ClaudeCodeScope::Local)
    {
        write_claude_code_local(target_config_path, claude_config, read_result.settings.as_ref())?;
    } else if agent_context.is_claude {
        write_claude_project_local(
            config,
            target_config_path,
            claude_config,
            read_result.settings.as_ref(),
        )?;
    } else if agent_context.is_gemini {
        write_gemini_project_local(target_config_path, claude_config)?;
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

fn write_gemini_project_local(
    target_config_path: &Path,
    claude_config: &ClaudeConfig,
) -> Result<()> {
    info!("Writing Gemini settings to {}", target_config_path.display());
    writer::write_claude_config(target_config_path, claude_config)
        .context("Failed to write .gemini/settings.json")?;
    Ok(())
}

fn write_claude_code_local(
    target_config_path: &Path,
    claude_config: &ClaudeConfig,
    settings: Option<&Settings>,
) -> Result<()> {
    info!("Writing local-scoped MCP servers to {}", target_config_path.display());
    writer::write_claude_config(target_config_path, claude_config)
        .context("Failed to write ~/.claude.json")?;

    let Some(settings_ref) = settings else {
        return Ok(());
    };

    let mut settings_candidate = settings_ref.clone();
    settings_candidate.mcp_servers = None;

    let has_settings = settings_candidate.api_key_helper.is_some()
        || settings_candidate.cleanup_period_days.is_some()
        || settings_candidate.env.is_some()
        || settings_candidate.include_co_authored_by.is_some()
        || settings_candidate.permissions.is_some()
        || settings_candidate.preferred_notif_channel.is_some()
        || !settings_candidate.extra.is_empty();

    if !has_settings {
        return Ok(());
    }

    let local_settings_path = claude_code_local_settings_path()?;
    let settings_to_write =
        build_claude_code_settings_to_write(&local_settings_path, Some(&settings_candidate))?;

    info!("Writing settings to {}", local_settings_path.display());
    ensure_parent_directory_exists(&local_settings_path)?;
    writer::write_settings(&local_settings_path, &settings_to_write)
        .context("Failed to write .claude/settings.local.json")?;

    Ok(())
}

/// Write Claude configuration in project-local mode
fn write_claude_project_local(
    config: &Config,
    target_config_path: &Path,
    claude_config: &ClaudeConfig,
    settings: Option<&Settings>,
) -> Result<()> {
    // Write MCP servers to .mcp.json
    if let Some(ref mcp_servers) = claude_config.mcp_servers {
        info!("Writing MCP servers to {}", target_config_path.display());
        let mcp_config = McpServersConfig { mcp_servers: mcp_servers.clone() };
        writer::write_mcp_servers_config(target_config_path, &mcp_config)
            .with_context(|| format!("Failed to write {}", target_config_path.display()))?;
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
                || settings_to_write.preferred_notif_channel.is_some()
                || !settings_to_write.extra.is_empty();

            if has_settings {
                let existing_settings = reader::read_settings(settings_path)
                    .context("Failed to read existing .claude/settings.json")?;

                let mut merged_settings = existing_settings.unwrap_or_else(|| Settings {
                    api_key_helper: None,
                    cleanup_period_days: None,
                    env: None,
                    include_co_authored_by: None,
                    permissions: None,
                    preferred_notif_channel: None,
                    mcp_servers: None,
                    extra: HashMap::new(),
                });

                merge_claude_code_settings(&mut merged_settings, &settings_to_write);
                merged_settings.mcp_servers = None;

                info!("Writing settings to .claude/settings.json");
                ensure_parent_directory_exists(settings_path)?;
                writer::write_settings(settings_path, &merged_settings)
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
        let new_toml_servers = convert_mcp_to_toml(mcp_servers);
        let mut merged_mcp_servers = codex_to_write.mcp_servers.take().unwrap_or_default();
        deep_merge_toml_maps(&mut merged_mcp_servers, &new_toml_servers);
        codex_to_write.mcp_servers = (!merged_mcp_servers.is_empty()).then_some(merged_mcp_servers);
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
            let mut settings_to_write = settings_ref.clone();
            settings_to_write.mcp_servers = None;

            info!("Writing settings to {}", settings_path.display());
            ensure_parent_directory_exists(settings_path)?;
            writer::write_settings(settings_path, &settings_to_write)
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

fn deep_merge_toml_maps(
    target: &mut HashMap<String, TomlValue>,
    overlay: &HashMap<String, TomlValue>,
) {
    for (key, value) in overlay {
        match target.get_mut(key) {
            Some(existing) => deep_merge_toml_value(existing, value),
            None => {
                target.insert(key.clone(), value.clone());
            },
        }
    }
}

fn deep_merge_toml_value(target: &mut TomlValue, overlay: &TomlValue) {
    match (target, overlay) {
        (TomlValue::Table(target_table), TomlValue::Table(overlay_table)) => {
            for (key, overlay_value) in overlay_table {
                match target_table.get_mut(key) {
                    Some(existing_value) => deep_merge_toml_value(existing_value, overlay_value),
                    None => {
                        target_table.insert(key.clone(), overlay_value.clone());
                    },
                }
            }
        },
        (target_value, overlay_value) => {
            *target_value = overlay_value.clone();
        },
    }
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
