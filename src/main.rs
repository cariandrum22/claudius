#![allow(missing_docs)]

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
#[cfg(feature = "profiling")]
use claudius::profiling::profile_flamegraph;
use claudius::{
    agent_paths,
    app_config::AppConfig,
    bootstrap,
    cli::{self, Cli},
    commands,
    config::{reader, Config},
    secrets::SecretResolver,
    sync_operations::{
        determine_agent, handle_backup, handle_dry_run, merge_all_configs, read_configurations,
        sync_commands_if_exists, write_configurations, AgentContext, CodexGlobalSyncOptions,
    },
    template::{
        append_rules_to_context_file, append_template_to_context_file, ensure_rules_directory,
    },
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

fn main() -> Result<()> {
    let cli = Cli::parse();

    initialize_tracing(cli.debug, cli.trace);

    let app_config = load_and_log_config()?;
    resolve_and_inject_secrets(app_config.as_ref());

    if cli.list_commands {
        print_available_commands();
        return Ok(());
    }

    let Some(command) = cli.command else {
        Cli::command().print_help().context("failed to print top-level help")?;
        println!();
        return Ok(());
    };

    dispatch_command(command, app_config.as_ref())
}

/// Initialize tracing with the specified debug/trace flags
fn initialize_tracing(debug: bool, trace: bool) {
    let log_level = if trace {
        Level::TRACE
    } else if debug {
        Level::DEBUG
    } else {
        Level::WARN
    };

    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(EnvFilter::builder().with_default_directive(log_level.into()).from_env_lossy())
        .init();

    // Only log profiling mode if debug is enabled
    if debug && std::env::var("CLAUDIUS_PROFILE").is_ok() {
        info!("Profiling mode enabled (CLAUDIUS_PROFILE is set)");
    }
}

/// Load application configuration and log its status
fn load_and_log_config() -> Result<Option<AppConfig>> {
    let app_config = AppConfig::load().context("Failed to load app configuration")?;

    if let Some(ref config) = app_config {
        debug!("Loaded app configuration from: {}", AppConfig::config_path()?.display());

        if let Some(ref secret_manager) = config.secret_manager {
            debug!("Secret manager configured: {:?}", secret_manager.manager_type);
        }
    } else {
        debug!("No app configuration file found at: {}", AppConfig::config_path()?.display());
    }

    Ok(app_config)
}

/// Resolve and inject secrets from environment variables
fn resolve_and_inject_secrets(app_config: Option<&AppConfig>) {
    let secret_manager_config = app_config.and_then(|c| c.secret_manager);
    let resolver = SecretResolver::new(secret_manager_config);

    match resolver.resolve_env_vars() {
        Ok(resolved_vars) => {
            if !resolved_vars.is_empty() {
                debug!("Resolved {} secret(s) from environment variables", resolved_vars.len());
                for key in resolved_vars.keys() {
                    debug!("  - {} (from CLAUDIUS_SECRET_{})", key, key);
                }
                SecretResolver::inject_env_vars(resolved_vars);
            }
        },
        Err(e) => {
            error!("Failed to resolve secrets: {}", e);
            std::process::exit(1);
        },
    }
}

/// Dispatch to the appropriate command handler
fn dispatch_command(command: cli::Commands, app_config: Option<&AppConfig>) -> Result<()> {
    match command {
        cli::Commands::Config(subcommand) => match subcommand {
            cli::ConfigCommands::Init(args) => run_init(args.force, app_config),
            cli::ConfigCommands::Sync(args) => run_config_sync(args, app_config),
            cli::ConfigCommands::Validate(args) => run_config_validate(args, app_config),
        },
        cli::Commands::Command(subcommand) => match subcommand {
            cli::CommandCommands::Sync(args) => run_sync_commands(args.global),
        },
        cli::Commands::Context(subcommand) => match subcommand {
            cli::ContextCommands::Append(args) => run_append_context(
                args.rule,
                args.path,
                args.template_path,
                args.global,
                args.agent,
                app_config,
            ),
            cli::ContextCommands::Install(args) => run_install_context(
                args.rules,
                args.all,
                args.path,
                args.agent,
                args.install_dir,
                app_config,
            ),
            cli::ContextCommands::List(args) => run_list_context(args, app_config),
        },
        cli::Commands::Secrets(subcommand) => match subcommand {
            cli::SecretsCommands::Run(args) => run_command(&args.command, app_config),
        },
    }
}

fn print_available_commands() {
    let root = Cli::command();
    println!("Available commands:");
    for subcommand in root.get_subcommands() {
        let name = subcommand.get_name();
        let about = subcommand
            .get_about()
            .or_else(|| subcommand.get_long_about())
            .map_or_else(|| String::from("(no description)"), std::string::ToString::to_string);
        println!("  {:<10} {}", name, about.trim());

        let nested: Vec<String> =
            subcommand.get_subcommands().map(|child| child.get_name().to_string()).collect();
        if !nested.is_empty() {
            println!("    subcommands: {}", nested.join(", "));
        }
    }
    println!();
    println!("Use `claudius <command> --help` for detailed usage.");
}

fn run_init(force: bool, app_config: Option<&AppConfig>) -> Result<()> {
    // Always use the default config directory (not project-local)
    let config = Config::new(true)?;
    let config_dir = config
        .mcp_servers_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Failed to determine config directory"))?;

    println!("Bootstrapping Claudius configuration at: {}", config_dir.display());

    // Get default context file from config if available
    let default_context = app_config
        .and_then(|c| c.default.as_ref())
        .and_then(|d| d.context_file.as_deref());

    // Get current working directory for context file creation
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;

    match bootstrap::bootstrap_config_with_context(config_dir, &current_dir, force, default_context)
    {
        Ok(()) => {
            println!("Claudius configuration bootstrapped successfully!");
            println!();
            println!("Next steps:");
            println!("  1. Edit configuration files in: {}", config_dir.display());
            println!("  2. Run 'claudius config sync' to apply your configuration");
            println!("  3. Run 'claudius commands sync' to publish custom commands");
            Ok(())
        },
        Err(e) => {
            error!("Failed to bootstrap configuration: {e:#}");
            std::process::exit(1);
        },
    }
}

fn run_sync_commands(global: bool) -> Result<()> {
    let config = Config::new(global)?;
    match commands::sync_commands(&config.commands_dir, &config.claude_commands_dir) {
        Ok(synced) => {
            if synced.is_empty() {
                println!("No commands to sync");
            } else {
                println!("Successfully synced {} command(s):", synced.len());
                for cmd in &synced {
                    println!("  - {cmd}");
                }
            }
            Ok(())
        },
        Err(e) => {
            error!("Failed to sync commands: {e:#}");
            std::process::exit(1);
        },
    }
}

fn run_config_sync(args: cli::ConfigSyncArgs, app_config: Option<&AppConfig>) -> Result<()> {
    let options = build_sync_options(args, app_config)?;
    run_sync(&options, app_config)
}

fn build_sync_options(
    args: cli::ConfigSyncArgs,
    app_config: Option<&AppConfig>,
) -> Result<SyncOptions> {
    let cli::ConfigSyncArgs {
        config,
        dry_run,
        backup,
        target_config,
        global,
        agent,
        scope,
        codex_requirements,
        codex_managed_config,
        gemini_system,
    } = args;

    let effective_agent = determine_agent(agent, app_config);
    validate_sync_agent_flags(
        effective_agent,
        scope,
        codex_requirements,
        codex_managed_config,
        gemini_system,
    )?;

    let effective_global = compute_effective_global(global, scope);
    if codex_requirements && !effective_global {
        anyhow::bail!(
            "--codex-requirements requires --global (Codex requirements are system-wide)"
        );
    }

    if codex_managed_config && !effective_global {
        anyhow::bail!(
            "--codex-managed-config requires --global (Codex managed_config.toml is system-wide)",
        );
    }

    if gemini_system && !effective_global {
        anyhow::bail!("--gemini-system requires --global (Gemini system settings are system-wide)");
    }

    let effective_target_config = target_config
        .or_else(|| gemini_system.then_some(agent_paths::gemini_cli_system_settings_path()));

    Ok(SyncOptions {
        config_path: config,
        target_config_path: effective_target_config,
        dry_run,
        backup,
        global: effective_global,
        agent_override: agent,
        claude_code_scope: scope,
        codex_requirements,
        codex_managed_config,
        gemini_system,
    })
}

fn validate_sync_agent_flags(
    effective_agent: Option<claudius::app_config::Agent>,
    scope: Option<claudius::app_config::ClaudeCodeScope>,
    codex_requirements: bool,
    codex_managed_config: bool,
    gemini_system: bool,
) -> Result<()> {
    if scope.is_some() && effective_agent != Some(claudius::app_config::Agent::ClaudeCode) {
        anyhow::bail!("--scope is only supported with --agent claude-code");
    }

    if codex_requirements && effective_agent != Some(claudius::app_config::Agent::Codex) {
        anyhow::bail!("--codex-requirements is only supported with --agent codex");
    }

    if codex_managed_config && effective_agent != Some(claudius::app_config::Agent::Codex) {
        anyhow::bail!("--codex-managed-config is only supported with --agent codex");
    }

    if gemini_system && effective_agent != Some(claudius::app_config::Agent::Gemini) {
        anyhow::bail!("--gemini-system is only supported with --agent gemini");
    }

    Ok(())
}

fn compute_effective_global(
    global: bool,
    scope: Option<claudius::app_config::ClaudeCodeScope>,
) -> bool {
    scope.map_or(global, |claude_scope| match claude_scope {
        claudius::app_config::ClaudeCodeScope::Managed
        | claudius::app_config::ClaudeCodeScope::User => true,
        claudius::app_config::ClaudeCodeScope::Project
        | claudius::app_config::ClaudeCodeScope::Local => false,
    })
}

fn run_config_validate(
    args: cli::ConfigValidateArgs,
    app_config: Option<&AppConfig>,
) -> Result<()> {
    use claudius::app_config::Agent;

    let cli::ConfigValidateArgs { agent, strict } = args;
    let effective_agent =
        agent.or_else(|| app_config.and_then(|cfg| cfg.default.as_ref()).map(|d| d.agent));

    let config_dir =
        Config::get_config_dir().context("Failed to determine Claudius config directory")?;

    let mut warnings = Vec::new();

    // MCP servers (required)
    let mcp_servers_path = config_dir.join("mcpServers.json");
    let mcp_servers = reader::read_mcp_servers_config(&mcp_servers_path).with_context(|| {
        format!("Failed to read MCP servers config: {}", mcp_servers_path.display())
    })?;

    for (name, server) in &mcp_servers.mcp_servers {
        if server.command.is_none() && server.url.is_none() {
            warnings.push(format!(
                "{}: mcpServers.{name} must define either command or url",
                mcp_servers_path.display(),
            ));
        }
    }

    match effective_agent {
        Some(Agent::Claude | Agent::ClaudeCode) => {
            warnings.extend(validate_claude_settings_sources(&config_dir)?);
        },
        Some(Agent::Codex) => {
            warnings.extend(validate_codex_sources(&config_dir)?);
        },
        Some(Agent::Gemini) => {
            warnings.extend(validate_gemini_sources(&config_dir)?);
        },
        None => {
            warnings.extend(validate_claude_settings_sources(&config_dir)?);
            warnings.extend(validate_codex_sources(&config_dir)?);
            warnings.extend(validate_gemini_sources(&config_dir)?);
        },
    }

    if warnings.is_empty() {
        println!("Configuration validation passed");
        return Ok(());
    }

    println!("Configuration validation warnings ({}):", warnings.len());
    for warning in &warnings {
        println!("  - {warning}");
    }

    if strict {
        anyhow::bail!("Validation failed due to warnings (--strict)");
    }

    Ok(())
}

fn validate_claude_settings_sources(config_dir: &std::path::Path) -> Result<Vec<String>> {
    let claude_settings_path = config_dir.join("claude.settings.json");
    let legacy_settings_path = config_dir.join("settings.json");

    let settings_candidate = if claude_settings_path.exists() {
        Some(claude_settings_path)
    } else if legacy_settings_path.exists() {
        Some(legacy_settings_path)
    } else {
        None
    };

    let Some(settings_path) = settings_candidate else {
        return Ok(Vec::new());
    };

    let content = std::fs::read_to_string(&settings_path)
        .with_context(|| format!("Failed to read {}", settings_path.display()))?;
    let json_value: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON from {}", settings_path.display()))?;

    let mut warnings = claudius::validation::validate_claude_settings(&json_value)
        .into_iter()
        .map(|w| format!("{}: {w}", settings_path.display()))
        .collect::<Vec<_>>();

    // Ensure Settings struct deserialization succeeds (we preserve unknown fields via `extra`).
    let _: claudius::config::Settings = serde_json::from_value(json_value).with_context(|| {
        format!("Failed to deserialize Settings from {}", settings_path.display())
    })?;

    // Legacy settings.json is not agent-specific, so annotate which file was used.
    if settings_path.file_name().and_then(|n| n.to_str()) == Some("settings.json") {
        warnings.push(format!(
            "{}: Using legacy settings.json (consider migrating to claude.settings.json)",
            settings_path.display(),
        ));
    }

    Ok(warnings)
}

fn validate_codex_sources(config_dir: &std::path::Path) -> Result<Vec<String>> {
    let mut warnings = Vec::new();

    let codex_settings_path = config_dir.join("codex.settings.toml");
    if codex_settings_path.exists() {
        warnings.extend(validate_codex_settings_like_file(&codex_settings_path)?);
    }

    let codex_requirements_path = config_dir.join("codex.requirements.toml");
    if codex_requirements_path.exists() {
        validate_toml_parse_file(&codex_requirements_path)?;
    }

    if let Some((managed_config_path, is_legacy)) = select_codex_managed_config_source(config_dir) {
        warnings.extend(validate_codex_settings_like_file(&managed_config_path)?);

        if is_legacy {
            warnings.push(format!(
                "{}: Using legacy managed_config.toml (consider migrating to codex.managed_config.toml)",
                managed_config_path.display(),
            ));
        }
    }

    Ok(warnings)
}

fn validate_toml_parse_file(path: &std::path::Path) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let _: toml::Value =
        toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(())
}

fn validate_codex_settings_like_file(path: &std::path::Path) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let value: toml::Value =
        toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;

    let warnings = claudius::codex_settings::validate_codex_settings(&value)
        .into_iter()
        .map(|w| format!("{}: {w}", path.display()))
        .collect::<Vec<_>>();

    let _: claudius::codex_settings::CodexSettings = toml::from_str(&content)
        .with_context(|| format!("Failed to deserialize {}", path.display()))?;

    Ok(warnings)
}

fn select_codex_managed_config_source(
    config_dir: &std::path::Path,
) -> Option<(std::path::PathBuf, bool)> {
    let codex_managed_config_path = config_dir.join("codex.managed_config.toml");
    if codex_managed_config_path.exists() {
        return Some((codex_managed_config_path, false));
    }

    let legacy_managed_config_path = config_dir.join("managed_config.toml");
    legacy_managed_config_path
        .exists()
        .then_some((legacy_managed_config_path, true))
}

fn validate_gemini_sources(config_dir: &std::path::Path) -> Result<Vec<String>> {
    let mut warnings = Vec::new();

    let gemini_settings_path = config_dir.join("gemini.settings.json");
    if !gemini_settings_path.exists() {
        return Ok(warnings);
    }

    let content = std::fs::read_to_string(&gemini_settings_path)
        .with_context(|| format!("Failed to read {}", gemini_settings_path.display()))?;
    let json_value: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON from {}", gemini_settings_path.display()))?;

    warnings.extend(
        claudius::gemini_settings::validate_gemini_settings(&json_value)
            .into_iter()
            .map(|w| format!("{}: {w}", gemini_settings_path.display())),
    );

    let _: claudius::gemini_settings::GeminiSettings = serde_json::from_value(json_value)
        .with_context(|| format!("Failed to deserialize {}", gemini_settings_path.display()))?;

    Ok(warnings)
}

fn run_list_context(args: cli::ContextListArgs, _app_config: Option<&AppConfig>) -> Result<()> {
    let rules_dir = ensure_rules_directory()?;
    let mut rules = Vec::new();
    collect_md_files(&rules_dir, &rules_dir, &mut rules)?;
    rules.sort();

    if rules.is_empty() {
        println!("No rules found in {}", rules_dir.display());
        return Ok(());
    }

    println!("Rules directory: {}", rules_dir.display());

    if args.tree {
        let mut tree = RulesTree::default();
        for rule in &rules {
            let components: Vec<&str> =
                rule.split('/').filter(|segment| !segment.is_empty()).collect();
            if components.is_empty() {
                continue;
            }
            insert_rule_path(&mut tree, &components);
        }
        print_rules_tree(&tree, "");
    } else {
        println!("Available rules ({}):", rules.len());
        for rule in &rules {
            println!("  - {rule}");
        }
    }

    Ok(())
}

#[derive(Default)]
struct RulesTree {
    directories: BTreeMap<String, Self>,
    files: BTreeSet<String>,
}

fn insert_rule_path(node: &mut RulesTree, components: &[&str]) {
    if let Some((head, tail)) = components.split_first() {
        if tail.is_empty() {
            node.files.insert(format!("{head}.md"));
        } else {
            insert_rule_path(node.directories.entry((*head).to_owned()).or_default(), tail);
        }
    }
}

fn print_rules_tree(node: &RulesTree, prefix: &str) {
    let total = node.directories.len().saturating_add(node.files.len());
    if total == 0 {
        return;
    }

    let mut index = 0_usize;

    for (dir, child) in &node.directories {
        index = index.saturating_add(1);
        let is_last = index == total;
        let connector = if is_last { "└── " } else { "├── " };
        println!("{prefix}{connector}{dir}/");
        let next_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });
        print_rules_tree(child, &next_prefix);
    }

    for file in &node.files {
        index = index.saturating_add(1);
        let is_last = index == total;
        let connector = if is_last { "└── " } else { "├── " };
        println!("{prefix}{connector}{file}");
    }
}

/// Determine the context filename based on agent and configuration
fn determine_context_filename(
    agent_override: Option<claudius::app_config::Agent>,
    app_config: Option<&AppConfig>,
    agent: claudius::app_config::Agent,
) -> String {
    // If agent was explicitly overridden, always use agent-specific file
    if agent_override.is_some() {
        debug!("Agent explicitly overridden, using agent-specific file");
        return get_agent_context_filename(agent);
    }

    // Check for custom context file in configuration
    if let Some(config) = app_config {
        debug!("App config found: {:?}", config);
        if let Some(ref default_config) = config.default {
            debug!("Default config found: {:?}", default_config);
            if let Some(ref context_file) = default_config.context_file {
                debug!("Using custom context file from config: {}", context_file);
                return context_file.clone();
            }
        }
    }

    // Fall back to agent-based default
    debug!("Using agent default context file");
    get_agent_context_filename(agent)
}

/// Determine the target directory for context files
fn determine_target_directory(
    global: bool,
    path: Option<std::path::PathBuf>,
) -> Result<std::path::PathBuf> {
    if global {
        Ok(directories::BaseDirs::new()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
            .home_dir()
            .to_path_buf())
    } else if let Some(p) = path {
        Ok(if p.is_absolute() { p } else { std::env::current_dir()?.join(p) })
    } else {
        std::env::current_dir().map_err(Into::into)
    }
}

fn run_append_context(
    rule: Option<String>,
    path: Option<std::path::PathBuf>,
    template_path: Option<std::path::PathBuf>,
    global: bool,
    agent_override: Option<claudius::app_config::Agent>,
    app_config: Option<&AppConfig>,
) -> Result<()> {
    // Determine agent
    let agent = agent_override
        .or_else(|| app_config.as_ref().and_then(|c| c.default.as_ref()).map(|d| d.agent))
        .unwrap_or(claudius::app_config::Agent::Claude);

    debug!("Using agent: {:?}", agent);

    // Determine context filename
    let context_filename = determine_context_filename(agent_override, app_config, agent);

    // Determine target directory
    let target_dir = determine_target_directory(global, path)?;

    let context_file_path = target_dir.join(&context_filename);
    debug!("Target context file: {}", context_file_path.display());

    if let Some(tmpl_path) = template_path {
        // Handle custom template
        append_template_to_context_file(Some(&tmpl_path), &context_file_path)?;
        println!("Template appended successfully to {context_filename}");
        Ok(())
    } else if let Some(rule_name) = rule {
        // Handle single rule
        append_rules_to_context_file(&[rule_name], &context_file_path)?;
        println!("Rule appended successfully to {context_filename}");
        Ok(())
    } else {
        // This should not happen due to CLI validation
        Err(anyhow::anyhow!("No rule or template specified"))
    }
}

fn get_agent_context_filename(agent: claudius::app_config::Agent) -> String {
    match agent {
        claudius::app_config::Agent::Claude | claudius::app_config::Agent::ClaudeCode => {
            "CLAUDE.md".to_string()
        },
        _ => "AGENTS.md".to_string(), // Codex and Gemini both use AGENTS.md
    }
}

/// Configuration for sync operation
struct SyncOptions {
    config_path: Option<std::path::PathBuf>,
    target_config_path: Option<std::path::PathBuf>,
    dry_run: bool,
    backup: bool,
    global: bool,
    agent_override: Option<claudius::app_config::Agent>,
    claude_code_scope: Option<claudius::app_config::ClaudeCodeScope>,
    codex_requirements: bool,
    codex_managed_config: bool,
    gemini_system: bool,
}

fn run_sync(options: &SyncOptions, app_config: Option<&AppConfig>) -> Result<()> {
    // If global mode, no agent specified, and no custom paths provided, sync all available agents
    if options.global
        && options.agent_override.is_none()
        && options.config_path.is_none()
        && options.target_config_path.is_none()
        && options.claude_code_scope.is_none()
        && !options.codex_requirements
        && !options.codex_managed_config
        && !options.gemini_system
        && app_config.is_none_or(|cfg| cfg.default.is_none())
    {
        sync_all_available_agents(options, app_config)
    } else {
        // Single agent sync (current behavior)
        let (agent_context, config, paths) = setup_sync_context(
            options.agent_override,
            app_config,
            options.global,
            options.config_path.clone(),
            options.target_config_path.clone(),
            options.claude_code_scope,
        )?;

        // Log configuration paths
        log_sync_paths(&paths, options.global, &config);

        // Execute sync operation
        let flags = SyncExecutionFlags {
            backup: options.backup,
            dry_run: options.dry_run,
            codex_global: CodexGlobalSyncOptions {
                requirements: options.codex_requirements,
                managed_config: options.codex_managed_config,
            },
        };

        execute_sync_operation(&config, &paths, agent_context, flags)
    }
}

/// Sync all available agents in global mode
fn sync_all_available_agents(options: &SyncOptions, app_config: Option<&AppConfig>) -> Result<()> {
    // Detect available agents
    let available_agents = Config::detect_available_agents()?;

    if available_agents.is_empty() {
        warn!("No agent configuration files found in config directory");
        // Still sync commands if they exist
        let config = Config::new_with_agent(true, None)?;
        sync_commands_if_exists(&config);
        return Ok(());
    }

    println!(
        "Found configurations for {} agent(s): {}",
        available_agents.len(),
        available_agents.iter().map(|a| format!("{a:?}")).collect::<Vec<_>>().join(", ")
    );

    // Sync each available agent
    for agent in &available_agents {
        let agent_name = match agent {
            claudius::app_config::Agent::Claude => "Claude",
            claudius::app_config::Agent::ClaudeCode => "Claude Code",
            claudius::app_config::Agent::Codex => "Codex",
            claudius::app_config::Agent::Gemini => "Gemini",
        };
        println!("\nSyncing agent: {agent_name}");
        println!("===============================================");

        let (agent_context, config, paths) = setup_sync_context(
            Some(*agent),
            app_config,
            true,
            options.config_path.clone(),
            options.target_config_path.clone(),
            None,
        )?;

        // Log configuration paths
        log_sync_paths(&paths, true, &config);

        // Execute sync operation for this agent
        let flags = SyncExecutionFlags {
            backup: options.backup,
            dry_run: options.dry_run,
            codex_global: CodexGlobalSyncOptions::default(),
        };

        execute_sync_operation(&config, &paths, agent_context, flags)?;
    }

    // Sync commands once after all agents
    if !available_agents.is_empty() {
        let config = Config::new_with_agent(true, None)?;
        sync_commands_if_exists(&config);
    }

    println!("\nAll agent configurations synced successfully");
    Ok(())
}

/// Setup sync context with agent and paths
fn setup_sync_context(
    agent_override: Option<claudius::app_config::Agent>,
    app_config: Option<&AppConfig>,
    global: bool,
    config_opt: Option<std::path::PathBuf>,
    target_config_opt: Option<std::path::PathBuf>,
    claude_code_scope: Option<claudius::app_config::ClaudeCodeScope>,
) -> Result<(AgentContext, Config, SyncPaths)> {
    let agent = determine_agent(agent_override, app_config);
    if let Some(a) = agent {
        debug!("Using agent: {:?}", a);
    }

    if claude_code_scope.is_some() && agent != Some(claudius::app_config::Agent::ClaudeCode) {
        anyhow::bail!("--scope is only supported with --agent claude-code");
    }

    let agent_context = AgentContext::new(agent, claude_code_scope);
    let config = Config::new_with_agent(global, agent)?;

    let default_target_config = match agent_context.claude_code_scope {
        Some(claudius::app_config::ClaudeCodeScope::Managed)
            if agent_context.is_claude_code && global =>
        {
            agent_paths::claude_code_managed_mcp_path()
        },
        Some(claudius::app_config::ClaudeCodeScope::Local)
            if agent_context.is_claude_code && !global =>
        {
            let base_dirs = directories::BaseDirs::new()
                .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
            base_dirs.home_dir().join(".claude.json")
        },
        _ => config.target_config_path.clone(),
    };

    let paths = SyncPaths {
        mcp_servers: config_opt.unwrap_or_else(|| config.mcp_servers_path.clone()),
        target_config: target_config_opt.unwrap_or(default_target_config),
    };

    Ok((agent_context, config, paths))
}

/// Container for sync paths
struct SyncPaths {
    mcp_servers: std::path::PathBuf,
    target_config: std::path::PathBuf,
}

/// Log sync configuration paths
fn log_sync_paths(paths: &SyncPaths, global: bool, config: &Config) {
    debug!("MCP servers config: {}", paths.mcp_servers.display());
    debug!(
        "Target config: {} ({}):",
        paths.target_config.display(),
        if global { "global" } else { "project-local" }
    );
    debug!("Settings file: {}", config.settings_path.display());
}

#[derive(Clone, Copy, Debug)]
struct SyncExecutionFlags {
    backup: bool,
    dry_run: bool,
    codex_global: CodexGlobalSyncOptions,
}

/// Execute the main sync operation
fn execute_sync_operation(
    config: &Config,
    paths: &SyncPaths,
    agent_context: AgentContext,
    flags: SyncExecutionFlags,
) -> Result<()> {
    // Read configurations
    let read_result = read_configurations(config, &paths.mcp_servers, agent_context)?;

    debug!("Reading target configuration");
    let mut claude_config = if config.is_global && agent_context.is_codex {
        // For Codex in global mode, don't read from paths.target_config (Codex uses ~/.codex/config.toml).
        claudius::config::ClaudeConfig { mcp_servers: None, other: HashMap::new() }
    } else {
        reader::read_claude_config(&paths.target_config)
            .context("Failed to read target configuration")?
    };

    // Process configurations
    if flags.backup {
        handle_backup(
            config,
            &paths.target_config,
            &read_result,
            agent_context,
            flags.codex_global,
        )?;
    }
    merge_all_configs(&mut claude_config, &read_result, agent_context, config.is_global)?;

    // Output results
    if flags.dry_run {
        handle_dry_run(
            config,
            &paths.target_config,
            &claude_config,
            &read_result,
            agent_context,
            flags.codex_global,
        )
    } else {
        write_configurations(
            config,
            &claude_config,
            &paths.target_config,
            &read_result,
            agent_context,
            flags.codex_global,
        )?;
        sync_commands_if_exists(config);
        Ok(())
    }
}

/// Execute command with inherited stdio
fn execute_command(program: &str, args: &[String]) -> Result<std::process::ExitStatus> {
    use std::process::{Command, Stdio};

    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("Failed to execute command: {program}"))?;

    child.wait().with_context(|| format!("Failed to wait for command: {program}"))
}

/// Handle the exit status of a child process
fn handle_exit_status(status: std::process::ExitStatus) -> ! {
    if !status.success() {
        if let Some(code) = status.code() {
            debug!("Command exited with code: {}", code);
            std::process::exit(code);
        } else {
            // Terminated by signal (Unix)
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                if let Some(signal) = status.signal() {
                    error!("Command terminated by signal: {}", signal);
                    // Exit with 128 + signal number (standard Unix convention)
                    std::process::exit(128_i32.saturating_add(signal));
                }
            }
            error!("Command terminated abnormally");
            std::process::exit(1);
        }
    }
    std::process::exit(0);
}

fn run_command(command: &[String], _app_config: Option<&AppConfig>) -> Result<()> {
    if command.is_empty() {
        error!("No command specified");
        std::process::exit(1);
    }

    // Check if profiling is enabled
    #[allow(unused_variables)]
    let profiling_enabled = std::env::var("CLAUDIUS_PROFILE").is_ok();

    #[cfg(feature = "profiling")]
    let status = if profiling_enabled {
        profile_flamegraph("run-command", || run_command_inner(command))??
    } else {
        run_command_inner(command)?
    };

    #[cfg(not(feature = "profiling"))]
    let status = {
        let _ = profiling_enabled; // Suppress unused variable warning
        run_command_inner(command)?
    };

    handle_exit_status(status);
}

fn run_command_inner(command: &[String]) -> Result<std::process::ExitStatus> {
    // Secrets are already resolved in main(), no need to resolve again

    // Extract command and arguments
    let (program, args) =
        command.split_first().ok_or_else(|| anyhow::anyhow!("Command is empty"))?;

    debug!("Running command: {}", command.join(" "));

    // Execute command and handle exit status
    execute_command(program, args)
}

// Helper functions for run_install_context
fn collect_md_files(
    dir: &std::path::Path,
    base_dir: &std::path::Path,
    rules: &mut Vec<String>,
) -> Result<()> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Recurse into subdirectories
            collect_md_files(&path, base_dir, rules)?;
        } else {
            process_file_entry(&path, base_dir, rules);
        }
    }
    Ok(())
}

fn process_file_entry(path: &std::path::Path, base_dir: &std::path::Path, rules: &mut Vec<String>) {
    let Some(filename) = path.file_name().and_then(|f| f.to_str()) else { return };
    if !filename.to_ascii_lowercase().ends_with(".md") {
        return;
    }

    // Get relative path from base_dir without .md extension
    let Ok(relative_path) = path.strip_prefix(base_dir) else { return };
    let Some(rule_path) = relative_path.to_str() else { return };

    // Remove .md extension and use forward slashes
    let rule_name = rule_path.trim_end_matches(".md").replace('\\', "/");
    rules.push(rule_name);
}

fn copy_rules(
    rules_to_copy: &[String],
    source_rules_dir: &std::path::Path,
    rules_dir: &std::path::Path,
) -> Result<Vec<String>> {
    use std::fs;

    let mut copied_rules = Vec::new();
    for rule_name in rules_to_copy {
        let source_path = source_rules_dir.join(format!("{rule_name}.md"));
        let dest_path = rules_dir.join(format!("{rule_name}.md"));

        debug!("Checking for rule at: {}", source_path.display());

        if !source_path.exists() {
            warn!("Rule '{}' not found at {}", rule_name, source_path.display());
            continue;
        }

        // Create subdirectories if needed
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        fs::copy(&source_path, &dest_path).with_context(|| {
            format!("Failed to copy {} to {}", source_path.display(), dest_path.display())
        })?;

        copied_rules.push(rule_name.clone());
        println!("Installed rule: {rule_name}");
    }

    if copied_rules.is_empty() {
        error!("No rules were installed");
        return Err(anyhow::anyhow!("No valid rules found"));
    }

    Ok(copied_rules)
}

const CLAUDIUS_RULES_SECTION_START: &str = "<!-- CLAUDIUS_RULES_START -->";
const CLAUDIUS_RULES_SECTION_END: &str = "<!-- CLAUDIUS_RULES_END -->";

fn add_reference_directive(
    target_dir: &std::path::Path,
    rules_dir: &std::path::Path,
    context_filename: &str,
    copied_rules: &[String],
) -> Result<()> {
    use std::fs;
    use std::io::Write;

    let context_file_path = target_dir.join(context_filename);

    // Calculate relative path from target_dir to rules_dir
    let relative_rules_path = rules_dir
        .strip_prefix(target_dir)
        .map_or_else(|_| rules_dir.to_path_buf(), std::path::Path::to_path_buf);

    // Read existing content
    let existing_content = if context_file_path.exists() {
        fs::read_to_string(&context_file_path)?
    } else {
        String::new()
    };

    let reference_directive = build_reference_directive(&relative_rules_path, copied_rules)?;

    if existing_content.contains(CLAUDIUS_RULES_SECTION_START) {
        let new_content = replace_existing_reference_section(
            &existing_content,
            &reference_directive,
            context_filename,
        )?;

        fs::write(&context_file_path, new_content)
            .with_context(|| format!("Failed to update {}", context_file_path.display()))?;
        println!("Updated reference directive in {context_filename}");
        return Ok(());
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&context_file_path)
        .with_context(|| format!("Failed to open {}", context_file_path.display()))?;

    file.write_all(reference_directive.as_bytes())?;
    println!("Added reference directive to {context_filename}");
    Ok(())
}

fn build_reference_directive(
    relative_rules_path: &std::path::Path,
    copied_rules: &[String],
) -> Result<String> {
    let rule_list = build_installed_rules_list(relative_rules_path, copied_rules)?;

    Ok(format!(
        "\n{section_start}\n\
# External Rule References\n\
\n\
The following rules from `{rules_dir}` are installed:\n\
\n\
{rule_list}\
\n\
Read these files to understand the project conventions and guidelines.\n\
{section_end}\n",
        section_start = CLAUDIUS_RULES_SECTION_START,
        section_end = CLAUDIUS_RULES_SECTION_END,
        rules_dir = relative_rules_path.display(),
        rule_list = rule_list,
    ))
}

fn build_installed_rules_list(
    relative_rules_path: &std::path::Path,
    copied_rules: &[String],
) -> Result<String> {
    use std::fmt::Write as _;

    let mut rule_list = String::new();
    for rule_name in copied_rules {
        let rule_path = relative_rules_path.join(format!("{rule_name}.md"));
        let rule_path_str = rule_path.to_string_lossy().replace('\\', "/");
        writeln!(&mut rule_list, "- `{rule_path_str}`: {}", rule_name.replace('/', " / "),)
            .map_err(|e| anyhow::anyhow!("Failed to format rules list: {e}"))?;
    }

    Ok(rule_list)
}

fn replace_existing_reference_section(
    existing_content: &str,
    reference_directive: &str,
    context_filename: &str,
) -> Result<String> {
    let Some(start_pos) = existing_content.find(CLAUDIUS_RULES_SECTION_START) else {
        return Ok(existing_content.to_string());
    };

    let remaining = existing_content
        .get(start_pos..)
        .ok_or_else(|| anyhow::anyhow!("Invalid section start boundary in {context_filename}"))?;
    let Some(end_rel) = remaining.find(CLAUDIUS_RULES_SECTION_END) else {
        anyhow::bail!("Found section start marker but no end marker in {context_filename}");
    };

    let end_pos = start_pos
        .checked_add(end_rel)
        .ok_or_else(|| anyhow::anyhow!("Section end marker position overflow"))?;
    let end_with_marker = end_pos
        .checked_add(CLAUDIUS_RULES_SECTION_END.len())
        .ok_or_else(|| anyhow::anyhow!("Section end marker position overflow"))?;

    let prefix = existing_content
        .get(..start_pos)
        .ok_or_else(|| anyhow::anyhow!("Invalid section start boundary in {context_filename}"))?;
    let suffix = existing_content
        .get(end_with_marker..)
        .ok_or_else(|| anyhow::anyhow!("Invalid section end boundary in {context_filename}"))?;

    Ok(format!("{prefix}{}{suffix}", reference_directive.trim_start()))
}

fn run_install_context(
    rules: Vec<String>,
    all: bool,
    path: Option<std::path::PathBuf>,
    agent_override: Option<claudius::app_config::Agent>,
    install_dir: Option<std::path::PathBuf>,
    app_config: Option<&AppConfig>,
) -> Result<()> {
    use std::fs;

    // Determine agent
    let agent = agent_override
        .or_else(|| app_config.as_ref().and_then(|c| c.default.as_ref()).map(|d| d.agent))
        .unwrap_or(claudius::app_config::Agent::Claude);

    debug!("Using agent: {:?}", agent);

    // Determine context filename
    let context_filename = determine_context_filename(agent_override, app_config, agent);

    // Determine target directory
    let target_dir = if let Some(p) = path {
        if p.is_absolute() {
            p
        } else {
            std::env::current_dir()?.join(p)
        }
    } else {
        std::env::current_dir()?
    };

    // Create rules directory (default: .agents/rules, or custom install_dir)
    let rules_base = install_dir.unwrap_or_else(|| std::path::PathBuf::from(".agents/rules"));
    let rules_dir = if rules_base.is_absolute() { rules_base } else { target_dir.join(rules_base) };
    fs::create_dir_all(&rules_dir)
        .with_context(|| format!("Failed to create directory: {}", rules_dir.display()))?;

    // Get config directory for source rules
    let source_rules_dir = Config::get_config_dir()?.join("rules");
    debug!("Looking for rules in: {}", source_rules_dir.display());

    // Determine which rules to copy
    let rules_to_copy = if all {
        // Get all .md files from the rules directory recursively
        println!(
            "Installing ALL rules from {} (including subdirectories)",
            source_rules_dir.display()
        );
        let mut all_rules = Vec::new();
        collect_md_files(&source_rules_dir, &source_rules_dir, &mut all_rules)?;

        if all_rules.is_empty() {
            return Err(anyhow::anyhow!("No rules found in {}", source_rules_dir.display()));
        }
        all_rules.sort(); // Sort for consistent ordering
        all_rules
    } else {
        rules
    };

    // Copy specified rules
    let copied_rules = copy_rules(&rules_to_copy, &source_rules_dir, &rules_dir)?;

    // Add reference directive to context file with the list of copied rules
    add_reference_directive(&target_dir, &rules_dir, &context_filename, &copied_rules)?;

    println!("Successfully installed {} rule(s) to {}", copied_rules.len(), rules_dir.display());

    Ok(())
}
