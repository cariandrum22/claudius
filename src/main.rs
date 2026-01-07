#![allow(missing_docs)]

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
#[cfg(feature = "profiling")]
use claudius::profiling::profile_flamegraph;
use claudius::{
    app_config::AppConfig,
    bootstrap,
    cli::{self, Cli},
    commands,
    config::{reader, Config},
    secrets::SecretResolver,
    sync_operations::{
        determine_agent, handle_backup, handle_dry_run, merge_all_configs, read_configurations,
        sync_commands_if_exists, write_configurations, AgentContext,
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
        Cli::command().print_help().expect("failed to print top-level help");
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
            .map(|s| s.to_string())
            .unwrap_or_else(|| String::from("(no description)"));
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
    let cli::ConfigSyncArgs { config, dry_run, backup, target_config, global, agent } = args;

    run_sync(
        &SyncOptions {
            config_path: config,
            target_config_path: target_config,
            dry_run,
            backup,
            global,
            agent_override: agent,
        },
        app_config,
    )
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
    directories: BTreeMap<String, RulesTree>,
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
    let total = node.directories.len() + node.files.len();
    if total == 0 {
        return;
    }

    let mut index = 0_usize;

    for (dir, child) in &node.directories {
        index += 1;
        let is_last = index == total;
        let connector = if is_last { "└── " } else { "├── " };
        println!("{prefix}{connector}{dir}/");
        let next_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });
        print_rules_tree(child, &next_prefix);
    }

    for file in &node.files {
        index += 1;
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
}

fn run_sync(options: &SyncOptions, app_config: Option<&AppConfig>) -> Result<()> {
    // If global mode, no agent specified, and no custom paths provided, sync all available agents
    if options.global
        && options.agent_override.is_none()
        && options.config_path.is_none()
        && options.target_config_path.is_none()
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
        )?;

        // Log configuration paths
        log_sync_paths(&paths, options.global, &config);

        // Execute sync operation
        execute_sync_operation(
            &config,
            &paths,
            agent_context,
            options.backup,
            options.dry_run,
            options.global,
        )
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
        )?;

        // Log configuration paths
        log_sync_paths(&paths, true, &config);

        // Execute sync operation for this agent
        execute_sync_operation(
            &config,
            &paths,
            agent_context,
            options.backup,
            options.dry_run,
            true,
        )?;
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
) -> Result<(AgentContext, Config, SyncPaths)> {
    let agent = determine_agent(agent_override, app_config);
    if let Some(a) = agent {
        debug!("Using agent: {:?}", a);
    }

    let agent_context = AgentContext::new(agent);
    let config = Config::new_with_agent(global, agent)?;

    let paths = SyncPaths {
        mcp_servers: config_opt.unwrap_or_else(|| config.mcp_servers_path.clone()),
        target_config: target_config_opt.unwrap_or_else(|| config.target_config_path.clone()),
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

/// Execute the main sync operation
fn execute_sync_operation(
    config: &Config,
    paths: &SyncPaths,
    agent_context: AgentContext,
    backup: bool,
    dry_run: bool,
    global: bool,
) -> Result<()> {
    // Read configurations
    let read_result = read_configurations(config, &paths.mcp_servers, agent_context)?;

    debug!("Reading target configuration");
    let mut claude_config = if global && (agent_context.is_codex || agent_context.is_gemini) {
        // For Codex/Gemini in global mode, don't read from ~/.claude.json
        // Start with empty config - the actual existing config will be read in write_*_global functions
        claudius::config::ClaudeConfig { mcp_servers: None, other: HashMap::new() }
    } else {
        reader::read_claude_config(&paths.target_config)
            .context("Failed to read target configuration")?
    };

    // Process configurations
    handle_backup(backup, &paths.target_config)?;
    merge_all_configs(&mut claude_config, &read_result, agent_context, global)?;

    // Output results
    if dry_run {
        handle_dry_run(&claude_config, &read_result, agent_context, global)
    } else {
        write_configurations(
            config,
            &claude_config,
            &paths.target_config,
            &read_result,
            agent_context,
            global,
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

    // Define section markers
    const SECTION_START: &str = "<!-- CLAUDIUS_RULES_START -->";
    const SECTION_END: &str = "<!-- CLAUDIUS_RULES_END -->";

    // Check if section already exists
    let section_exists = existing_content.contains(SECTION_START);

    // Build the list of rule files with descriptions
    let mut rule_list = String::new();
    for rule_name in copied_rules {
        let rule_path = relative_rules_path.join(format!("{rule_name}.md"));
        let rule_path_str = rule_path.to_string_lossy().replace('\\', "/");
        rule_list.push_str(&format!("- `{}`: {}\n", rule_path_str, rule_name.replace('/', " / ")));
    }

    // Build the reference directive
    let reference_directive = format!(
        "\n{}\n\
# External Rule References\n\
\n\
The following rules from `{}` are installed:\n\
\n\
{}\
\n\
Read these files to understand the project conventions and guidelines.\n\
{}\n",
        SECTION_START,
        relative_rules_path.display(),
        rule_list,
        SECTION_END
    );

    if section_exists {
        // Section exists - update it
        if let Some(start_pos) = existing_content.find(SECTION_START) {
            if let Some(end_pos) = existing_content.find(SECTION_END) {
                // Calculate end position including the marker
                let end_with_marker = end_pos + SECTION_END.len();

                // Build new content
                let new_content = format!(
                    "{}{}{}",
                    &existing_content[..start_pos],
                    reference_directive.trim_start(),
                    &existing_content[end_with_marker..]
                );

                // Write the updated content
                fs::write(&context_file_path, new_content)
                    .with_context(|| format!("Failed to update {}", context_file_path.display()))?;
                println!("Updated reference directive in {context_filename}");
            } else {
                return Err(anyhow::anyhow!(
                    "Found section start marker but no end marker in {}",
                    context_filename
                ));
            }
        }
    } else {
        // Section doesn't exist - append it
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&context_file_path)
            .with_context(|| format!("Failed to open {}", context_file_path.display()))?;

        file.write_all(reference_directive.as_bytes())?;
        println!("Added reference directive to {context_filename}");
    }

    Ok(())
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
