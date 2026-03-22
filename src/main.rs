#![allow(missing_docs)]

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
#[cfg(feature = "profiling")]
use claudius::profiling::profile_flamegraph;
use claudius::{
    agent_paths,
    app_config::AppConfig,
    asset_sync::SyncBehavior,
    bootstrap,
    cli::{self, Cli},
    config::{reader, Config},
    doctor::{render_report, run_doctor, DoctorOptions},
    secrets::SecretResolver,
    skills,
    sync_operations::{
        determine_agent, handle_backup, handle_dry_run, merge_all_configs,
        print_supporting_assets_dry_run, read_configurations, sync_supporting_assets,
        write_configurations, AgentContext, CodexGlobalSyncOptions,
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
            cli::ConfigCommands::Doctor(args) => run_config_doctor(args),
        },
        cli::Commands::Skills(subcommand) => match subcommand {
            cli::SkillsCommands::Sync(args) => run_sync_skills(args, app_config),
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
        .and_then(|d| d.context_file.clone().or_else(|| Some(get_agent_context_filename(d.agent))));

    // Get current working directory for context file creation
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;

    match bootstrap::bootstrap_config_with_context(
        config_dir,
        &current_dir,
        force,
        default_context.as_deref(),
    ) {
        Ok(()) => {
            println!("Claudius configuration bootstrapped successfully!");
            println!();
            println!("Next steps:");
            println!("  1. Edit configuration files in: {}", config_dir.display());
            println!("  2. Run 'claudius config sync' to apply your configuration");
            println!("  3. Run 'claudius skills sync' to publish skills when needed");
            Ok(())
        },
        Err(e) => {
            error!("Failed to bootstrap configuration: {e:#}");
            std::process::exit(1);
        },
    }
}

fn run_sync_skills(args: cli::SkillsSyncArgs, app_config: Option<&AppConfig>) -> Result<()> {
    let cli::SkillsSyncArgs { global, agent, enable_codex_skills, dry_run, prune } = args;

    let effective_agent = determine_agent(agent, app_config);

    if effective_agent == Some(claudius::app_config::Agent::Codex) && !enable_codex_skills {
        println!("Codex skills sync is experimental. Re-run with --enable-codex-skills.");
        return Ok(());
    }

    let config = Config::new_with_agent(global, effective_agent)?;
    let Some(source_dir) = config.resolve_skills_source_dir() else {
        let skill_targets = determine_skill_sync_targets(&config)?;
        let reports = skill_targets
            .iter()
            .map(|target_dir| {
                skills::sync_skills_with_options(None, target_dir, SyncBehavior { dry_run, prune })
            })
            .collect::<Result<Vec<_>>>()?;
        print_skill_sync_result(&reports, dry_run);
        return Ok(());
    };

    if source_dir != config.skills_dir {
        println!(
            "Legacy commands directory detected; syncing skills from {}",
            source_dir.display()
        );
    }

    let skill_targets = determine_skill_sync_targets(&config)?;
    let reports = skill_targets
        .iter()
        .map(|target_dir| {
            skills::sync_skills_with_options(
                Some(&source_dir),
                target_dir,
                SyncBehavior { dry_run, prune },
            )
        })
        .collect::<Result<Vec<_>>>()?;

    print_skill_sync_result(&reports, dry_run);
    Ok(())
}

fn determine_skill_sync_targets(config: &Config) -> Result<Vec<std::path::PathBuf>> {
    let mut targets = vec![config.skills_target_dir.clone()];

    if let Some(compat_target) = config.codex_compat_skills_target_dir()? {
        if !targets.contains(&compat_target) {
            targets.push(compat_target);
        }
    }

    Ok(targets)
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
        prune,
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
        prune,
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
        Some(Agent::Claude) => {
            warnings.extend(validate_claude_settings_sources(&config_dir)?);
        },
        Some(Agent::ClaudeCode) => {
            warnings.extend(validate_claude_code_sources(&config_dir)?);
        },
        Some(Agent::Codex) => {
            warnings.extend(validate_codex_sources(&config_dir)?);
        },
        Some(Agent::Gemini) => {
            warnings.extend(validate_gemini_sources(&config_dir)?);
        },
        None => {
            warnings.extend(validate_claude_code_sources(&config_dir)?);
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

fn run_config_doctor(args: cli::ConfigDoctorArgs) -> Result<()> {
    let report = run_doctor(DoctorOptions { global: args.global, agent_filter: args.agent })?;
    println!("{}", render_report(&report));
    Ok(())
}

fn validate_claude_code_sources(config_dir: &std::path::Path) -> Result<Vec<String>> {
    let mut warnings = validate_claude_settings_sources(config_dir)?;
    warnings.extend(validate_claude_code_subagent_sources(config_dir)?);
    Ok(warnings)
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

    warnings.extend(validate_codex_skill_compatibility(config_dir));

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
    if gemini_settings_path.exists() {
        let content = std::fs::read_to_string(&gemini_settings_path)
            .with_context(|| format!("Failed to read {}", gemini_settings_path.display()))?;
        let json_value: serde_json::Value = serde_json::from_str(&content).with_context(|| {
            format!("Failed to parse JSON from {}", gemini_settings_path.display())
        })?;

        warnings.extend(
            claudius::gemini_settings::validate_gemini_settings(&json_value)
                .into_iter()
                .map(|w| format!("{}: {w}", gemini_settings_path.display())),
        );

        let _: claudius::gemini_settings::GeminiSettings = serde_json::from_value(json_value)
            .with_context(|| format!("Failed to deserialize {}", gemini_settings_path.display()))?;
    }

    warnings.extend(validate_gemini_command_sources(config_dir)?);

    Ok(warnings)
}

fn validate_gemini_command_sources(config_dir: &std::path::Path) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    let commands_dir = config_dir.join("commands").join("gemini");
    let mut command_files = Vec::new();
    collect_files_with_extension(&commands_dir, "toml", &mut command_files)?;

    for command_file in command_files {
        let result = claudius::validation::validate_gemini_command_file(&command_file)?;
        warnings.extend(
            result
                .warnings
                .into_iter()
                .map(|warning| format!("{}: {warning}", command_file.display())),
        );
    }

    Ok(warnings)
}

fn validate_claude_code_subagent_sources(config_dir: &std::path::Path) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    let agents_dir = config_dir.join("agents").join("claude-code");
    let mut agent_files = Vec::new();
    collect_files_with_extension(&agents_dir, "md", &mut agent_files)?;

    for agent_file in agent_files {
        let result = claudius::validation::validate_claude_code_subagent_file(&agent_file)?;
        warnings.extend(
            result
                .warnings
                .into_iter()
                .map(|warning| format!("{}: {warning}", agent_file.display())),
        );
    }

    Ok(warnings)
}

fn validate_codex_skill_compatibility(config_dir: &std::path::Path) -> Vec<String> {
    let skills_dir = config_dir.join("skills");
    let legacy_commands_dir = config_dir.join("commands");

    [
        skills_dir.join("codex"),
        skills_dir,
        legacy_commands_dir,
    ]
    .iter()
    .find(|path| path.exists() && directory_has_entries(path))
    .map_or_else(Vec::new, |path| {
        vec![format!(
            "{}: Codex skills sync remains experimental and publishes to both .codex/skills and .agents/skills for compatibility",
            path.display()
        )]
    })
}

fn directory_has_entries(path: &std::path::Path) -> bool {
    std::fs::read_dir(path)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

fn collect_files_with_extension(
    dir: &std::path::Path,
    extension: &str,
    files: &mut Vec<std::path::PathBuf>,
) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read directory: {}", dir.display()))?
    {
        let entry = entry.with_context(|| format!("Failed to read entry in {}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_extension(&path, extension, files)?;
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
            files.push(path);
        }
    }

    files.sort();
    Ok(())
}

fn run_list_context(args: cli::ContextListArgs, _app_config: Option<&AppConfig>) -> Result<()> {
    let rules_dir = ensure_rules_directory()?;

    if args.tree {
        let tree = build_directory_tree(&rules_dir, 0)?;
        if tree.is_empty() {
            println!("No rules or templates found in: {}", rules_dir.display());
        } else {
            println!("Available rules and templates in {}:", rules_dir.display());
            print!("{tree}");
        }
        return Ok(());
    }

    let mut rules = Vec::new();
    collect_rule_names(&rules_dir, &rules_dir, &mut rules)?;
    rules.sort();

    if rules.is_empty() {
        println!("No rules or templates found in: {}", rules_dir.display());
    } else {
        println!("Available rules and templates in {}:", rules_dir.display());
        for rule in rules {
            println!("  - {rule}");
        }
    }

    Ok(())
}

fn collect_rule_names(
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
            collect_rule_names(&path, base_dir, rules)?;
            continue;
        }

        let Some(filename) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        if !filename.to_ascii_lowercase().ends_with(".md") {
            continue;
        }

        let Ok(relative_path) = path.strip_prefix(base_dir) else {
            continue;
        };
        let Some(rule_path) = relative_path.to_str() else {
            continue;
        };

        rules.push(rule_path.trim_end_matches(".md").replace('\\', "/"));
    }

    Ok(())
}

fn build_directory_tree(dir: &std::path::Path, depth: usize) -> Result<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(String::new());
    };

    let mut entries = entries.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    let mut output = String::new();
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let indent = "  ".repeat(depth);

        if path.is_dir() {
            output.push_str(&format!("{indent}{name}/\n"));
            output.push_str(&build_directory_tree(&path, depth + 1)?);
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            output.push_str(&format!("{indent}{name}\n"));
        }
    }

    Ok(output)
}

fn determine_context_filename(
    agent_override: Option<claudius::app_config::Agent>,
    app_config: Option<&AppConfig>,
    fallback_agent: claudius::app_config::Agent,
) -> String {
    app_config
        .and_then(|cfg| cfg.default.as_ref())
        .and_then(|defaults| {
            let effective_agent = agent_override.unwrap_or(defaults.agent);
            if effective_agent == fallback_agent {
                defaults.context_file.clone()
            } else {
                None
            }
        })
        .unwrap_or_else(|| get_agent_context_filename(fallback_agent))
}

fn determine_target_directory(global: bool, path: Option<std::path::PathBuf>) -> Result<std::path::PathBuf> {
    if let Some(custom_path) = path {
        if custom_path.is_absolute() {
            return Ok(custom_path);
        }

        return Ok(std::env::current_dir()?.join(custom_path));
    }

    if global {
        let base_dirs = directories::BaseDirs::new()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        return Ok(base_dirs.home_dir().to_path_buf());
    }

    std::env::current_dir().context("Failed to get current directory")
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
        claudius::app_config::Agent::Gemini => "GEMINI.md".to_string(),
        claudius::app_config::Agent::Codex => "AGENTS.md".to_string(),
    }
}

/// Configuration for sync operation
struct SyncOptions {
    config_path: Option<std::path::PathBuf>,
    target_config_path: Option<std::path::PathBuf>,
    dry_run: bool,
    backup: bool,
    prune: bool,
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
            prune: options.prune,
            codex_global: CodexGlobalSyncOptions {
                requirements: options.codex_requirements,
                managed_config: options.codex_managed_config,
            },
        };

        execute_sync_operation(&config, &paths, agent_context, flags)
    }
}
