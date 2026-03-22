#![allow(missing_docs)]

use crate::app_config::Agent;
use crate::asset_sync::{inspect_managed_tree, ManagedTreeInspection, SourceFileMapping};
use crate::config::Config;
use crate::skills;
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DoctorStatus {
    Supported,
    BestEffort,
    Legacy,
    Unmanaged,
    Experimental,
    Stale,
}

impl DoctorStatus {
    #[must_use]
    pub const fn heading(self) -> &'static str {
        match self {
            Self::Supported => "SUPPORTED",
            Self::BestEffort => "BEST-EFFORT",
            Self::Legacy => "LEGACY",
            Self::Unmanaged => "UNMANAGED",
            Self::Experimental => "EXPERIMENTAL",
            Self::Stale => "STALE",
        }
    }

    #[must_use]
    pub const fn ordered() -> [Self; 6] {
        [
            Self::Supported,
            Self::BestEffort,
            Self::Legacy,
            Self::Unmanaged,
            Self::Experimental,
            Self::Stale,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct DoctorFinding {
    pub status: DoctorStatus,
    pub summary: String,
    pub path: Option<PathBuf>,
    pub detail: Option<String>,
    pub recommendation: String,
}

#[derive(Debug, Clone)]
pub struct DoctorReport {
    pub global: bool,
    pub agent_filter: Option<Agent>,
    pub config_dir: PathBuf,
    pub deployment_base_dir: PathBuf,
    pub findings: Vec<DoctorFinding>,
}

#[derive(Debug, Clone, Copy)]
pub struct DoctorOptions {
    pub global: bool,
    pub agent_filter: Option<Agent>,
}

#[derive(Debug, Clone)]
struct SourceSurfaceState {
    shared_skills: Vec<SourceFileMapping>,
    legacy_commands: Vec<SourceFileMapping>,
    claude_skills: Vec<SourceFileMapping>,
    claude_code_skills: Vec<SourceFileMapping>,
    gemini_skills: Vec<SourceFileMapping>,
    codex_skills: Vec<SourceFileMapping>,
    gemini_commands: Vec<SourceFileMapping>,
    claude_code_agents: Vec<SourceFileMapping>,
}

/// Build a configuration health report for the selected deployment context.
///
/// # Errors
///
/// Returns an error if Claudius paths cannot be resolved or if source/manifest
/// inspection fails.
pub fn run_doctor(options: DoctorOptions) -> Result<DoctorReport> {
    let config_dir = Config::get_config_dir().context("Failed to determine Claudius config dir")?;
    let config = Config::new_with_agent(options.global, options.agent_filter)
        .context("Failed to resolve diagnostic context")?;
    let source_state = load_source_surface_state(&config_dir)?;
    let deployment_base_dir = config
        .deployment_base_dir()
        .context("Failed to determine deployment base dir")?;
    let findings = collect_findings(options, &config_dir, &deployment_base_dir, &source_state)?;

    Ok(DoctorReport {
        global: options.global,
        agent_filter: options.agent_filter,
        config_dir,
        deployment_base_dir,
        findings,
    })
}

pub fn render_report(report: &DoctorReport) -> String {
    let mut lines = vec![
        "Configuration doctor report".to_string(),
        format!("Context: {}", if report.global { "global" } else { "project-local" }),
        format!("Agent filter: {}", agent_filter_label(report.agent_filter)),
        format!("Config directory: {}", report.config_dir.display()),
        format!("Deployment base: {}", report.deployment_base_dir.display()),
    ];

    let mut groups = BTreeMap::<DoctorStatus, Vec<&DoctorFinding>>::new();
    for finding in &report.findings {
        groups.entry(finding.status).or_default().push(finding);
    }

    if report.findings.is_empty() {
        lines.push(String::new());
        lines.push("No managed surfaces or lifecycle risks were detected.".to_string());
        return lines.join("\n");
    }

    for status in DoctorStatus::ordered() {
        let Some(findings) = groups.get(&status) else {
            continue;
        };

        lines.push(String::new());
        lines.push(format!("{} ({})", status.heading(), findings.len()));
        for finding in findings {
            lines.push(format!("- {}", finding.summary));
            if let Some(path) = &finding.path {
                lines.push(format!("  Path: {}", path.display()));
            }
            if let Some(detail) = &finding.detail {
                lines.push(format!("  Detail: {detail}"));
            }
            lines.push(format!("  Next: {}", finding.recommendation));
        }
    }

    lines.join("\n")
}

fn load_source_surface_state(config_dir: &Path) -> Result<SourceSurfaceState> {
    Ok(SourceSurfaceState {
        shared_skills: collect_shared_skill_mappings(&config_dir.join("skills"))?,
        legacy_commands: collect_legacy_command_mappings(&config_dir.join("commands"))?,
        claude_skills: collect_agent_skill_mappings(&config_dir.join("skills").join("claude"))?,
        claude_code_skills: collect_agent_skill_mappings(
            &config_dir.join("skills").join("claude-code"),
        )?,
        gemini_skills: collect_agent_skill_mappings(&config_dir.join("skills").join("gemini"))?,
        codex_skills: collect_agent_skill_mappings(&config_dir.join("skills").join("codex"))?,
        gemini_commands: collect_tree_if_exists(&config_dir.join("commands").join("gemini"))?,
        claude_code_agents: collect_tree_if_exists(&config_dir.join("agents").join("claude-code"))?,
    })
}

fn collect_findings(
    options: DoctorOptions,
    config_dir: &Path,
    deployment_base_dir: &Path,
    source_state: &SourceSurfaceState,
) -> Result<Vec<DoctorFinding>> {
    let mut findings = Vec::new();

    inspect_claude_sources(config_dir, options.agent_filter, &mut findings);
    inspect_codex_sources(config_dir, options.agent_filter, &mut findings);
    inspect_gemini_sources(config_dir, options.agent_filter, &mut findings);
    inspect_skill_sources(
        config_dir,
        options.agent_filter,
        &source_state.shared_skills,
        &source_state.claude_skills,
        &source_state.claude_code_skills,
        &source_state.gemini_skills,
        &source_state.codex_skills,
        &source_state.legacy_commands,
        &mut findings,
    );
    inspect_auxiliary_sources(
        config_dir,
        options.agent_filter,
        &source_state.gemini_commands,
        &source_state.claude_code_agents,
        &mut findings,
    );
    inspect_target_surfaces(options, deployment_base_dir, source_state, &mut findings)?;

    Ok(findings)
}

fn inspect_claude_sources(
    config_dir: &Path,
    agent_filter: Option<Agent>,
    findings: &mut Vec<DoctorFinding>,
) {
    if !matches_filter(agent_filter, Agent::Claude)
        && !matches_filter(agent_filter, Agent::ClaudeCode)
    {
        return;
    }

    let preferred = config_dir.join("claude.settings.json");
    let legacy = config_dir.join("settings.json");

    if preferred.exists() {
        findings.push(DoctorFinding {
            status: DoctorStatus::Supported,
            summary: "Claude / Claude Code settings source is using the preferred layout."
                .to_string(),
            path: Some(preferred),
            detail: None,
            recommendation: "Keep using claude.settings.json for Claude and Claude Code settings."
                .to_string(),
        });
    } else if legacy.exists() {
        findings.push(DoctorFinding {
            status: DoctorStatus::Legacy,
            summary: "Legacy settings.json is still active for Claude / Claude Code settings."
                .to_string(),
            path: Some(legacy),
            detail: None,
            recommendation: "Rename or migrate settings.json to claude.settings.json.".to_string(),
        });
    }
}

fn inspect_codex_sources(
    config_dir: &Path,
    agent_filter: Option<Agent>,
    findings: &mut Vec<DoctorFinding>,
) {
    if !matches_filter(agent_filter, Agent::Codex) {
        return;
    }

    let settings = config_dir.join("codex.settings.toml");
    if settings.exists() {
        findings.push(DoctorFinding {
            status: DoctorStatus::Supported,
            summary: "Codex settings source is present.".to_string(),
            path: Some(settings),
            detail: None,
            recommendation: "Keep this in sync with `claudius config sync --agent codex`."
                .to_string(),
        });
    }

    let requirements = config_dir.join("codex.requirements.toml");
    if requirements.exists() {
        findings.push(DoctorFinding {
            status: DoctorStatus::Supported,
            summary: "Codex admin-enforced requirements source is present.".to_string(),
            path: Some(requirements),
            detail: None,
            recommendation:
                "Sync it with `claudius config sync --global --agent codex --codex-requirements` when admin requirements change."
                    .to_string(),
        });
    }

    let managed = config_dir.join("codex.managed_config.toml");
    let legacy_managed = config_dir.join("managed_config.toml");
    if managed.exists() {
        findings.push(DoctorFinding {
            status: DoctorStatus::Supported,
            summary: "Codex managed defaults source is present.".to_string(),
            path: Some(managed),
            detail: None,
            recommendation:
                "Sync it with `claudius config sync --global --agent codex --codex-managed-config` when managed defaults change."
                    .to_string(),
        });
    } else if legacy_managed.exists() {
        findings.push(DoctorFinding {
            status: DoctorStatus::Legacy,
            summary: "Legacy managed_config.toml is still active for Codex managed defaults."
                .to_string(),
            path: Some(legacy_managed),
            detail: None,
            recommendation: "Rename or migrate managed_config.toml to codex.managed_config.toml."
                .to_string(),
        });
    }
}

fn inspect_gemini_sources(
    config_dir: &Path,
    agent_filter: Option<Agent>,
    findings: &mut Vec<DoctorFinding>,
) {
    if !matches_filter(agent_filter, Agent::Gemini) {
        return;
    }

    let settings = config_dir.join("gemini.settings.json");
    if settings.exists() {
        findings.push(DoctorFinding {
            status: DoctorStatus::Supported,
            summary: "Gemini settings source is present.".to_string(),
            path: Some(settings),
            detail: None,
            recommendation: "Keep it in sync with `claudius config sync --agent gemini`."
                .to_string(),
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn inspect_skill_sources(
    config_dir: &Path,
    agent_filter: Option<Agent>,
    shared_skill_mappings: &[SourceFileMapping],
    claude_skill_mappings: &[SourceFileMapping],
    claude_code_skill_mappings: &[SourceFileMapping],
    gemini_skill_mappings: &[SourceFileMapping],
    codex_skill_mappings: &[SourceFileMapping],
    legacy_command_mappings: &[SourceFileMapping],
    findings: &mut Vec<DoctorFinding>,
) {
    let shared_skills_path = config_dir.join("skills");
    if !shared_skill_mappings.is_empty() && agent_filter != Some(Agent::Codex) {
        findings.push(DoctorFinding {
            status: DoctorStatus::Supported,
            summary: "Shared skills source is present.".to_string(),
            path: Some(shared_skills_path),
            detail: Some(format!(
                "{} shared skill file(s) are available for managed skill sync.",
                shared_skill_mappings.len()
            )),
            recommendation: "Sync them with `claudius skills sync` or the relevant `claudius config sync` command.".to_string(),
        });
    }

    push_agent_skill_finding(
        findings,
        agent_filter,
        Agent::Claude,
        config_dir.join("skills").join("claude"),
        claude_skill_mappings,
        "Claude-specific skills source is present.",
    );
    push_agent_skill_finding(
        findings,
        agent_filter,
        Agent::ClaudeCode,
        config_dir.join("skills").join("claude-code"),
        claude_code_skill_mappings,
        "Claude Code-specific skills source is present.",
    );
    push_agent_skill_finding(
        findings,
        agent_filter,
        Agent::Gemini,
        config_dir.join("skills").join("gemini"),
        gemini_skill_mappings,
        "Gemini-specific skills source is present.",
    );

    if !legacy_command_mappings.is_empty() && agent_filter != Some(Agent::Gemini) {
        findings.push(DoctorFinding {
            status: DoctorStatus::Legacy,
            summary: "Legacy commands/*.md skill fallback is still in use.".to_string(),
            path: Some(config_dir.join("commands")),
            detail: Some(format!(
                "{} legacy command file(s) still rely on commands/*.md fallback.",
                legacy_command_mappings.len()
            )),
            recommendation: "Move each commands/*.md file into skills/<name>/SKILL.md.".to_string(),
        });
    }

    if should_report_codex_experimental(
        agent_filter,
        shared_skill_mappings,
        codex_skill_mappings,
        legacy_command_mappings,
    ) {
        findings.push(DoctorFinding {
            status: DoctorStatus::Experimental,
            summary: "Codex skill sync remains in experimental compatibility mode.".to_string(),
            path: Some(config_dir.join("skills").join("codex")),
            detail: Some(
                "Codex skill sync still uses opt-in publishing and compatibility targets."
                    .to_string(),
            ),
            recommendation:
                "Use `claudius skills sync --agent codex --enable-codex-skills` only when you need Codex skills."
                    .to_string(),
        });
    }
}

fn inspect_auxiliary_sources(
    config_dir: &Path,
    agent_filter: Option<Agent>,
    gemini_command_mappings: &[SourceFileMapping],
    claude_code_agent_mappings: &[SourceFileMapping],
    findings: &mut Vec<DoctorFinding>,
) {
    if matches_filter(agent_filter, Agent::Gemini) && !gemini_command_mappings.is_empty() {
        findings.push(DoctorFinding {
            status: DoctorStatus::Supported,
            summary: "Gemini custom command source is present.".to_string(),
            path: Some(config_dir.join("commands").join("gemini")),
            detail: Some(format!(
                "{} Gemini command file(s) are ready to sync.",
                gemini_command_mappings.len()
            )),
            recommendation: "Deploy them with `claudius config sync --agent gemini`.".to_string(),
        });
    }

    if matches_filter(agent_filter, Agent::ClaudeCode) && !claude_code_agent_mappings.is_empty() {
        findings.push(DoctorFinding {
            status: DoctorStatus::Supported,
            summary: "Claude Code subagent source is present.".to_string(),
            path: Some(config_dir.join("agents").join("claude-code")),
            detail: Some(format!(
                "{} Claude Code subagent file(s) are ready to sync.",
                claude_code_agent_mappings.len()
            )),
            recommendation: "Deploy them with `claudius config sync --agent claude-code`."
                .to_string(),
        });
    }
}

fn inspect_target_surfaces(
    options: DoctorOptions,
    deployment_base_dir: &Path,
    source_state: &SourceSurfaceState,
    findings: &mut Vec<DoctorFinding>,
) -> Result<()> {
    inspect_best_effort_targets(options, findings)?;
    inspect_unmanaged_targets(options.agent_filter, deployment_base_dir, findings);
    inspect_claude_skill_targets(options, source_state, findings)?;
    inspect_gemini_targets(options, deployment_base_dir, source_state, findings)?;
    inspect_claude_code_targets(options, deployment_base_dir, source_state, findings)?;
    inspect_codex_targets(options, source_state, findings)?;

    Ok(())
}

fn inspect_best_effort_targets(
    options: DoctorOptions,
    findings: &mut Vec<DoctorFinding>,
) -> Result<()> {
    if !(options.global && matches_filter(options.agent_filter, Agent::Claude)) {
        return Ok(());
    }

    let claude_config = Config::new_with_agent(true, Some(Agent::Claude))?;
    if claude_config.target_config_path.exists() {
        findings.push(DoctorFinding {
            status: DoctorStatus::BestEffort,
            summary: "Claude Desktop JSON target is present as a legacy / best-effort surface."
                .to_string(),
            path: Some(claude_config.target_config_path),
            detail: Some(
                "Claudius can sync this JSON file, but it does not manage Claude Desktop Extensions or Connectors."
                    .to_string(),
            ),
            recommendation:
                "Prefer Claude Code, Codex, or Gemini when you need actively managed surfaces."
                    .to_string(),
        });
    }

    Ok(())
}

fn inspect_unmanaged_targets(
    agent_filter: Option<Agent>,
    deployment_base_dir: &Path,
    findings: &mut Vec<DoctorFinding>,
) {
    if matches_filter(agent_filter, Agent::Gemini) {
        let extensions_dir = deployment_base_dir.join(".gemini").join("extensions");
        if directory_has_entries(&extensions_dir) {
            findings.push(DoctorFinding {
                status: DoctorStatus::Unmanaged,
                summary: "Gemini extensions are present in an unmanaged target directory."
                    .to_string(),
                path: Some(extensions_dir),
                detail: None,
                recommendation:
                    "Install and update Gemini extensions through the Gemini CLI workflow; Claudius does not sync them."
                        .to_string(),
            });
        }
    }
}

fn inspect_claude_skill_targets(
    options: DoctorOptions,
    source_state: &SourceSurfaceState,
    findings: &mut Vec<DoctorFinding>,
) -> Result<()> {
    if !(matches_filter(options.agent_filter, Agent::Claude)
        || matches_filter(options.agent_filter, Agent::ClaudeCode))
    {
        return Ok(());
    }

    let source_mappings = combine_mappings(&[
        &source_state.shared_skills,
        &source_state.claude_skills,
        &source_state.claude_code_skills,
        &source_state.legacy_commands,
    ]);
    let target_dir =
        Config::new_with_agent(options.global, Some(Agent::ClaudeCode))?.skills_target_dir;
    push_stale_finding(
        findings,
        inspect_managed_tree(&target_dir, &source_mappings)?,
        "Claudius-managed Claude skills target has stale deployed files.",
        skill_prune_command(options.global, None),
    );

    Ok(())
}

fn inspect_gemini_targets(
    options: DoctorOptions,
    deployment_base_dir: &Path,
    source_state: &SourceSurfaceState,
    findings: &mut Vec<DoctorFinding>,
) -> Result<()> {
    if !matches_filter(options.agent_filter, Agent::Gemini) {
        return Ok(());
    }

    let gemini_skill_source_mappings = combine_mappings(&[
        &source_state.shared_skills,
        &source_state.gemini_skills,
        &source_state.legacy_commands,
    ]);
    let gemini_config = Config::new_with_agent(options.global, Some(Agent::Gemini))?;
    push_stale_finding(
        findings,
        inspect_managed_tree(&gemini_config.skills_target_dir, &gemini_skill_source_mappings)?,
        "Claudius-managed Gemini skills target has stale deployed files.",
        skill_prune_command(options.global, Some(Agent::Gemini)),
    );

    let gemini_command_target = gemini_config
        .gemini_commands_target_dir()?
        .unwrap_or_else(|| deployment_base_dir.join(".gemini").join("commands"));
    push_stale_finding(
        findings,
        inspect_managed_tree(&gemini_command_target, &source_state.gemini_commands)?,
        "Claudius-managed Gemini commands target has stale deployed files.",
        config_prune_command(options.global, Agent::Gemini),
    );

    Ok(())
}

fn inspect_claude_code_targets(
    options: DoctorOptions,
    deployment_base_dir: &Path,
    source_state: &SourceSurfaceState,
    findings: &mut Vec<DoctorFinding>,
) -> Result<()> {
    if !matches_filter(options.agent_filter, Agent::ClaudeCode) {
        return Ok(());
    }

    let claude_code_agent_target = Config::new_with_agent(options.global, Some(Agent::ClaudeCode))?
        .claude_code_agents_target_dir()?
        .unwrap_or_else(|| deployment_base_dir.join(".claude").join("agents"));
    push_stale_finding(
        findings,
        inspect_managed_tree(&claude_code_agent_target, &source_state.claude_code_agents)?,
        "Claudius-managed Claude Code subagents target has stale deployed files.",
        config_prune_command(options.global, Agent::ClaudeCode),
    );

    Ok(())
}

fn inspect_codex_targets(
    options: DoctorOptions,
    source_state: &SourceSurfaceState,
    findings: &mut Vec<DoctorFinding>,
) -> Result<()> {
    if !matches_filter(options.agent_filter, Agent::Codex) {
        return Ok(());
    }

    let codex_skill_source_mappings = combine_mappings(&[
        &source_state.shared_skills,
        &source_state.codex_skills,
        &source_state.legacy_commands,
    ]);
    let codex_config = Config::new_with_agent(options.global, Some(Agent::Codex))?;
    push_stale_finding(
        findings,
        inspect_managed_tree(&codex_config.skills_target_dir, &codex_skill_source_mappings)?,
        "Claudius-managed Codex skills target has stale deployed files.",
        skill_prune_command(options.global, Some(Agent::Codex)),
    );

    if let Some(compat_target) = codex_config.codex_compat_skills_target_dir()? {
        let inspection = inspect_managed_tree(&compat_target, &codex_skill_source_mappings)?;
        let had_managed_files = !inspection.managed_files.is_empty();
        push_stale_finding(
            findings,
            inspection,
            "Claudius-managed Codex compatibility skills target has stale deployed files.",
            skill_prune_command(options.global, Some(Agent::Codex)),
        );

        if had_managed_files || !source_state.codex_skills.is_empty() {
            findings.push(DoctorFinding {
                status: DoctorStatus::Experimental,
                summary:
                    "Codex compatibility skills target is present for experimental sync."
                        .to_string(),
                path: Some(compat_target),
                detail: Some(
                    "Claudius still publishes Codex skills to both .codex/skills and .agents/skills for compatibility."
                        .to_string(),
                ),
                recommendation:
                    "Keep Codex skills opt-in and expect compatibility targets to evolve."
                        .to_string(),
            });
        }
    }

    Ok(())
}

fn push_agent_skill_finding(
    findings: &mut Vec<DoctorFinding>,
    agent_filter: Option<Agent>,
    agent: Agent,
    path: PathBuf,
    mappings: &[SourceFileMapping],
    summary: &str,
) {
    if matches_filter(agent_filter, agent) && !mappings.is_empty() {
        findings.push(DoctorFinding {
            status: DoctorStatus::Supported,
            summary: summary.to_string(),
            path: Some(path),
            detail: Some(format!("{} skill file(s) are ready to sync.", mappings.len())),
            recommendation: "Publish them with `claudius skills sync` for the matching agent."
                .to_string(),
        });
    }
}

fn push_stale_finding(
    findings: &mut Vec<DoctorFinding>,
    inspection: ManagedTreeInspection,
    summary: &str,
    recommendation: String,
) {
    if inspection.stale_files.is_empty() {
        return;
    }

    findings.push(DoctorFinding {
        status: DoctorStatus::Stale,
        summary: summary.to_string(),
        path: Some(inspection.target_dir),
        detail: Some(format!(
            "{} stale file(s): {}",
            inspection.stale_files.len(),
            summarize_paths(&inspection.stale_files),
        )),
        recommendation,
    });
}

fn should_report_codex_experimental(
    agent_filter: Option<Agent>,
    shared_skill_mappings: &[SourceFileMapping],
    codex_skill_mappings: &[SourceFileMapping],
    legacy_command_mappings: &[SourceFileMapping],
) -> bool {
    if agent_filter == Some(Agent::Codex) {
        return !shared_skill_mappings.is_empty()
            || !codex_skill_mappings.is_empty()
            || !legacy_command_mappings.is_empty();
    }

    !codex_skill_mappings.is_empty()
}

fn collect_tree_if_exists(path: &Path) -> Result<Vec<SourceFileMapping>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    crate::asset_sync::collect_directory_tree_mappings(path)
}

fn collect_agent_skill_mappings(path: &Path) -> Result<Vec<SourceFileMapping>> {
    skills::collect_skill_mappings(path.exists().then_some(path))
}

fn collect_shared_skill_mappings(skills_root: &Path) -> Result<Vec<SourceFileMapping>> {
    if !skills_root.exists() {
        return Ok(Vec::new());
    }

    let mut mappings = Vec::new();
    let mut entries = fs::read_dir(skills_root)
        .with_context(|| format!("Failed to read directory: {}", skills_root.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        let entry_name = entry.file_name();
        let entry_name_lossy = entry_name.to_string_lossy();
        if is_agent_skill_subdir(&entry_name_lossy) {
            continue;
        }

        if !path.is_dir() {
            continue;
        }

        let skill_name = entry_name_lossy.to_string();
        let nested = crate::asset_sync::collect_directory_tree_mappings(&path)?;
        mappings.extend(nested.into_iter().map(|mapping| SourceFileMapping {
            source_path: mapping.source_path,
            relative_path: format!("{skill_name}/{}", mapping.relative_path),
        }));
    }

    mappings.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(mappings)
}

fn collect_legacy_command_mappings(commands_root: &Path) -> Result<Vec<SourceFileMapping>> {
    if !commands_root.exists() {
        return Ok(Vec::new());
    }

    let mut mappings = Vec::new();
    let mut entries = fs::read_dir(commands_root)
        .with_context(|| format!("Failed to read directory: {}", commands_root.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }

        let skill_name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid legacy command file name"))?
            .to_string();
        mappings.push(SourceFileMapping {
            source_path: path,
            relative_path: format!("{skill_name}/SKILL.md"),
        });
    }

    Ok(mappings)
}

fn combine_mappings(mapping_sets: &[&[SourceFileMapping]]) -> Vec<SourceFileMapping> {
    let mut combined = mapping_sets
        .iter()
        .flat_map(|mappings| mappings.iter().cloned())
        .collect::<Vec<_>>();
    combined.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    combined.dedup_by(|left, right| left.relative_path == right.relative_path);
    combined
}

fn summarize_paths(paths: &[String]) -> String {
    const MAX_ITEMS: usize = 5;
    if paths.len() <= MAX_ITEMS {
        return paths.join(", ");
    }

    let head = paths.iter().take(MAX_ITEMS).cloned().collect::<Vec<_>>().join(", ");
    format!("{head}, +{} more", paths.len().saturating_sub(MAX_ITEMS))
}

fn skill_prune_command(global: bool, selected_agent: Option<Agent>) -> String {
    let mut parts = vec!["claudius skills sync".to_string()];
    if global {
        parts.push("--global".to_string());
    }
    if let Some(agent_name) = selected_agent {
        parts.push(format!("--agent {}", agent_cli_name(agent_name)));
        if agent_name == Agent::Codex {
            parts.push("--enable-codex-skills".to_string());
        }
    }
    parts.push("--prune".to_string());
    parts.join(" ")
}

fn config_prune_command(global: bool, agent: Agent) -> String {
    let mut parts = vec!["claudius config sync".to_string()];
    if global {
        parts.push("--global".to_string());
    }
    parts.push(format!("--agent {}", agent_cli_name(agent)));
    parts.push("--prune".to_string());
    parts.join(" ")
}

fn directory_has_entries(path: &Path) -> bool {
    fs::read_dir(path).map(|mut entries| entries.next().is_some()).unwrap_or(false)
}

fn matches_filter(agent_filter: Option<Agent>, candidate: Agent) -> bool {
    agent_filter.is_none_or(|agent| agent == candidate)
}

fn agent_filter_label(agent_filter: Option<Agent>) -> String {
    agent_filter.map_or_else(|| "all".to_string(), |agent| agent_cli_name(agent).to_string())
}

fn agent_cli_name(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => "claude",
        Agent::ClaudeCode => "claude-code",
        Agent::Codex => "codex",
        Agent::Gemini => "gemini",
    }
}

fn is_agent_skill_subdir(name: &str) -> bool {
    matches!(name, "claude" | "claude-code" | "codex" | "gemini")
}
