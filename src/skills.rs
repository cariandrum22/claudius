use crate::{
    app_config::Agent,
    asset_sync::{self, ManagedTreeSyncReport, SourceFileMapping, SyncBehavior},
};
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_yaml::{Mapping as YamlMapping, Value as YamlValue};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;
use tempfile::TempDir;

const SKILL_FILE_NAME: &str = "SKILL.md";
const CANONICAL_SKILL_FILE_NAME: &str = "skill.yaml";
const DEFAULT_CANONICAL_INSTRUCTIONS_FILE: &str = "instructions.md";
const SKILL_RESOURCE_DIRS: &[&str] = &["scripts", "references", "assets"];
const CLAUDE_ONLY_FRONTMATTER_KEYS: &[&str] = &[
    "disable-model-invocation",
    "user-invocable",
    "allowed-tools",
    "arguments",
    "argument-hint",
    "agent",
    "context",
];

static YAML_FRONTMATTER_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?s)\A---\r?\n(.*?)\r?\n---\r?\n?(.*)\z")
        .expect("frontmatter regex should compile")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillSyncReport {
    pub target_dir: PathBuf,
    pub synced_skills: Vec<String>,
    pub synced_files: Vec<String>,
    pub pruned_files: Vec<String>,
}

#[derive(Debug)]
pub struct SkillSourceSet {
    pub mappings: Vec<SourceFileMapping>,
    pub includes_legacy_commands: bool,
    pub warnings: Vec<String>,
    _render_workspace: Option<TempDir>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillValidationReport {
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SkillSourceOrigin {
    LegacyCommand,
    Shared,
    AgentOverride,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillCandidateKind {
    CanonicalDirectory,
    LegacyDirectory,
    LegacyFile,
}

#[derive(Debug, Clone)]
struct SkillCandidate {
    name: String,
    path: PathBuf,
    kind: SkillCandidateKind,
    origin: SkillSourceOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SkillTargetName {
    Claude,
    ClaudeCode,
    Codex,
    Gemini,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SkillInvocationMode {
    Auto,
    Manual,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct CanonicalSkillDefinition {
    version: u8,
    name: String,
    description: String,
    #[serde(default = "default_instructions_file")]
    instructions_file: String,
    #[serde(default)]
    targets: BTreeMap<SkillTargetName, SkillTargetOverlay>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct SkillTargetOverlay {
    #[serde(default)]
    invocation: Option<SkillInvocationMode>,
    #[serde(default)]
    disable_model_invocation: Option<bool>,
    #[serde(default)]
    user_invocable: Option<bool>,
    #[serde(default)]
    allowed_tools: Option<Vec<String>>,
    #[serde(default)]
    arguments: Option<YamlValue>,
    #[serde(default)]
    argument_hint: Option<String>,
    #[serde(default)]
    context: Option<YamlValue>,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    interface: Option<YamlValue>,
    #[serde(default)]
    dependencies: Option<YamlValue>,
}

#[derive(Debug, Clone)]
struct LegacySkillDocument {
    raw_content: String,
    frontmatter: Option<YamlMapping>,
    body: String,
    parse_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RenderedTextFile {
    relative_path: String,
    content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RenderedSkillBundle {
    name: String,
    generated_files: Vec<RenderedTextFile>,
    resource_mappings: Vec<SourceFileMapping>,
}

impl SkillSyncReport {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.synced_files.is_empty() && self.pruned_files.is_empty()
    }
}

/// Sync skills from source to target directory.
///
/// Supports two source formats:
/// - skills/<name>/... (directory-based skills)
/// - legacy commands/*.md (files converted to skills/<name>/SKILL.md)
///
/// # Errors
///
/// Returns an error if:
/// - Unable to create the target directory
/// - Unable to read the source directory
/// - Unable to copy skill files
pub fn sync_skills(source_dir: &Path, target_dir: &Path) -> Result<Vec<String>> {
    fs::create_dir_all(target_dir)
        .with_context(|| format!("Failed to create target directory: {}", target_dir.display()))?;

    let report = sync_skills_with_options(
        Some(source_dir),
        target_dir,
        SyncBehavior { dry_run: false, prune: false },
    )?;

    Ok(report.synced_skills)
}

/// Sync skills with optional dry-run and pruning behavior.
///
/// # Errors
///
/// Returns an error if the source tree cannot be read, files cannot be copied,
/// or the managed-file manifest cannot be updated.
pub fn sync_skills_with_options(
    source_dir_opt: Option<&Path>,
    target_dir: &Path,
    behavior: SyncBehavior,
) -> Result<SkillSyncReport> {
    let mappings = collect_skill_mappings(source_dir_opt)?;
    sync_skill_mappings_with_options(&mappings, target_dir, behavior)
}

/// Sync already-collected skill mappings into a target directory.
///
/// # Errors
///
/// Returns an error if the managed-file manifest cannot be updated or the
/// target tree cannot be written.
pub fn sync_skill_mappings_with_options(
    mappings: &[SourceFileMapping],
    target_dir: &Path,
    behavior: SyncBehavior,
) -> Result<SkillSyncReport> {
    let synced_skills = skill_names_from_mappings(mappings);
    let ManagedTreeSyncReport { target_dir: synced_target_dir, synced_files, pruned_files } =
        asset_sync::sync_managed_tree(target_dir, mappings, behavior)?;

    Ok(SkillSyncReport { target_dir: synced_target_dir, synced_skills, synced_files, pruned_files })
}

fn default_instructions_file() -> String {
    DEFAULT_CANONICAL_INSTRUCTIONS_FILE.to_string()
}

/// Validate Claudius skills by loading and rendering them for the selected
/// agent(s) without writing deployment targets.
///
/// # Errors
///
/// Returns an error if canonical skills are invalid, required files are
/// missing, or rendering fails for the selected target agent.
pub fn validate_claudius_skill_sources(
    config_dir: &Path,
    agent_filter: Option<Agent>,
) -> Result<SkillValidationReport> {
    let mut warnings = BTreeSet::new();

    for agent in validation_agents(agent_filter) {
        let source_set = collect_claudius_skill_source_set(config_dir, Some(agent))
            .with_context(|| format!("Failed to validate skills for {}", agent_label(agent)))?;
        warnings.extend(source_set.warnings);
    }

    Ok(SkillValidationReport { warnings: warnings.into_iter().collect() })
}

fn validation_agents(agent_filter: Option<Agent>) -> Vec<Agent> {
    agent_filter.map_or_else(
        || vec![Agent::Claude, Agent::ClaudeCode, Agent::Codex, Agent::Gemini],
        |selected| vec![selected],
    )
}

fn agent_label(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => "claude",
        Agent::ClaudeCode => "claude-code",
        Agent::Codex => "codex",
        Agent::Gemini => "gemini",
    }
}

fn effective_render_agent(agent: Option<Agent>) -> Agent {
    agent.unwrap_or(Agent::Claude)
}

fn canonical_target_for_agent(agent: Agent) -> SkillTargetName {
    match agent {
        Agent::Claude => SkillTargetName::Claude,
        Agent::ClaudeCode => SkillTargetName::ClaudeCode,
        Agent::Codex => SkillTargetName::Codex,
        Agent::Gemini => SkillTargetName::Gemini,
    }
}

fn discover_skill_candidates(
    skills_root: &Path,
    commands_root: &Path,
    agent: Option<Agent>,
) -> Result<(Vec<SkillCandidate>, bool)> {
    let mut merged = BTreeMap::<String, SkillCandidate>::new();
    let legacy_commands = collect_legacy_command_candidates(commands_root)?;
    let includes_legacy_commands = !legacy_commands.is_empty();

    for candidate in legacy_commands {
        merged.insert(candidate.name.clone(), candidate);
    }

    for candidate in
        collect_skill_candidates_in_directory(skills_root, SkillSourceOrigin::Shared, true)?
    {
        merged.insert(candidate.name.clone(), candidate);
    }

    if let Some(selected_agent) = agent {
        let agent_root = skills_root.join(agent_skill_subdir(selected_agent));
        for candidate in collect_skill_candidates_in_directory(
            &agent_root,
            SkillSourceOrigin::AgentOverride,
            false,
        )? {
            merged.insert(candidate.name.clone(), candidate);
        }
    }

    Ok((merged.into_values().collect(), includes_legacy_commands))
}

fn collect_skill_candidates_in_directory(
    root: &Path,
    origin: SkillSourceOrigin,
    skip_agent_dirs: bool,
) -> Result<Vec<SkillCandidate>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut entries = fs::read_dir(root)
        .with_context(|| format!("Failed to read skills directory: {}", root.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    let mut candidates = Vec::new();
    for entry in entries {
        let path = entry.path();
        let entry_name = entry
            .file_name()
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid skill entry name"))?
            .to_string();

        if path.is_dir() {
            if skip_agent_dirs && is_agent_skill_subdir(&entry_name) {
                continue;
            }

            let canonical_path = path.join(CANONICAL_SKILL_FILE_NAME);
            let legacy_skill_path = path.join(SKILL_FILE_NAME);

            if canonical_path.exists() && legacy_skill_path.exists() {
                anyhow::bail!(
                    "Skill directory {} contains both {} and {}; choose exactly one source format",
                    path.display(),
                    CANONICAL_SKILL_FILE_NAME,
                    SKILL_FILE_NAME,
                );
            }

            let kind = if canonical_path.exists() {
                SkillCandidateKind::CanonicalDirectory
            } else if legacy_skill_path.exists() {
                SkillCandidateKind::LegacyDirectory
            } else {
                anyhow::bail!(
                    "Skill directory {} must contain either {} or {}",
                    path.display(),
                    CANONICAL_SKILL_FILE_NAME,
                    SKILL_FILE_NAME,
                );
            };

            candidates.push(SkillCandidate { name: entry_name, path, kind, origin });
            continue;
        }

        if path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("md") {
            let skill_name = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid skill file stem"))?
                .to_string();
            candidates.push(SkillCandidate {
                name: skill_name,
                path,
                kind: SkillCandidateKind::LegacyFile,
                origin,
            });
        }
    }

    Ok(candidates)
}

fn collect_legacy_command_candidates(commands_root: &Path) -> Result<Vec<SkillCandidate>> {
    if !commands_root.exists() {
        return Ok(Vec::new());
    }

    let mut entries = fs::read_dir(commands_root)
        .with_context(|| format!("Failed to read commands directory: {}", commands_root.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    let mut candidates = Vec::new();
    for entry in entries {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }

        let skill_name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid skill file stem"))?
            .to_string();
        candidates.push(SkillCandidate {
            name: skill_name,
            path,
            kind: SkillCandidateKind::LegacyFile,
            origin: SkillSourceOrigin::LegacyCommand,
        });
    }

    Ok(candidates)
}

fn render_candidate_to_mappings(
    candidate: &SkillCandidate,
    render_agent: Agent,
    render_workspace: &Path,
    warnings: &mut BTreeSet<String>,
) -> Result<Vec<SourceFileMapping>> {
    let bundle = match candidate.kind {
        SkillCandidateKind::CanonicalDirectory => {
            render_canonical_skill_bundle(candidate, render_agent, warnings)?
        },
        SkillCandidateKind::LegacyDirectory | SkillCandidateKind::LegacyFile => {
            render_legacy_skill_bundle(candidate, render_agent, warnings)?
        },
    };

    materialize_rendered_bundle(&bundle, render_workspace)
}

fn render_canonical_skill_bundle(
    candidate: &SkillCandidate,
    render_agent: Agent,
    warnings: &mut BTreeSet<String>,
) -> Result<RenderedSkillBundle> {
    let definition = load_canonical_skill_definition(candidate)?;
    warnings.extend(collect_canonical_layout_warnings(&candidate.path, &definition)?);
    let target_name = canonical_target_for_agent(render_agent);
    let target_overlay = definition.targets.get(&target_name).cloned().unwrap_or_default();
    let instructions = load_canonical_instructions(&candidate.path, &definition, target_name)?;

    match target_name {
        SkillTargetName::Codex => {
            if target_overlay.disable_model_invocation.is_some()
                || target_overlay.user_invocable.is_some()
                || target_overlay.allowed_tools.is_some()
                || target_overlay.arguments.is_some()
                || target_overlay.argument_hint.is_some()
                || target_overlay.context.is_some()
                || target_overlay.agent.is_some()
            {
                warnings.insert(format!(
                    "Codex target overlay for skill `{}` contains Claude-specific fields that will be ignored during rendering.",
                    definition.name
                ));
            }
        },
        SkillTargetName::Claude | SkillTargetName::ClaudeCode | SkillTargetName::Gemini => {
            if target_overlay.interface.is_some() || target_overlay.dependencies.is_some() {
                warnings.insert(format!(
                    "{} target overlay for skill `{}` contains Codex-only fields that will be ignored during rendering.",
                    target_name_label(target_name),
                    definition.name
                ));
            }
        },
    }

    let generated_files = match target_name {
        SkillTargetName::Codex => render_codex_generated_files(
            &definition.name,
            &definition.description,
            &instructions,
            &target_overlay,
        )?,
        SkillTargetName::Claude | SkillTargetName::ClaudeCode | SkillTargetName::Gemini => {
            vec![RenderedTextFile {
                relative_path: SKILL_FILE_NAME.to_string(),
                content: render_claude_family_skill_markdown(
                    &definition.name,
                    &definition.description,
                    &instructions,
                    &target_overlay,
                )?,
            }]
        },
    };

    Ok(RenderedSkillBundle {
        name: definition.name.clone(),
        generated_files,
        resource_mappings: collect_canonical_resource_mappings(&candidate.path, &definition.name)?,
    })
}

fn load_canonical_skill_definition(candidate: &SkillCandidate) -> Result<CanonicalSkillDefinition> {
    let definition_path = candidate.path.join(CANONICAL_SKILL_FILE_NAME);
    let content = fs::read_to_string(&definition_path)
        .with_context(|| format!("Failed to read {}", definition_path.display()))?;
    let definition: CanonicalSkillDefinition = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", definition_path.display()))?;

    if definition.version != 1 {
        anyhow::bail!(
            "Canonical skill {} uses unsupported version {}; only version 1 is supported",
            definition_path.display(),
            definition.version,
        );
    }

    if definition.name.trim().is_empty() {
        anyhow::bail!("Canonical skill {} must define a non-empty name", definition_path.display());
    }

    if definition.description.trim().is_empty() {
        anyhow::bail!(
            "Canonical skill {} must define a non-empty description",
            definition_path.display()
        );
    }

    if definition.name != candidate.name {
        anyhow::bail!(
            "Canonical skill directory name `{}` must match skill name `{}`",
            candidate.name,
            definition.name,
        );
    }

    validate_canonical_instructions_file(&candidate.path, &definition)?;
    validate_canonical_reserved_directories(&candidate.path, &definition.name)?;

    Ok(definition)
}

fn validate_canonical_instructions_file(
    skill_root: &Path,
    definition: &CanonicalSkillDefinition,
) -> Result<()> {
    let definition_path = skill_root.join(CANONICAL_SKILL_FILE_NAME);
    let instructions_file = Path::new(&definition.instructions_file);
    let mut components = instructions_file.components();

    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => {},
        _ => anyhow::bail!(
            "Canonical skill {} must define instructions_file as a single file name within the skill directory",
            definition_path.display(),
        ),
    }

    let instructions_path = skill_root.join(&definition.instructions_file);
    if !instructions_path.is_file() {
        anyhow::bail!(
            "Canonical skill {} declares instructions_file `{}` but {} is not a regular file",
            definition_path.display(),
            definition.instructions_file,
            instructions_path.display(),
        );
    }

    Ok(())
}

fn validate_canonical_reserved_directories(skill_root: &Path, skill_name: &str) -> Result<()> {
    for dir_name in SKILL_RESOURCE_DIRS.iter().copied().chain(std::iter::once("targets")) {
        let path = skill_root.join(dir_name);
        if path.exists() && !path.is_dir() {
            anyhow::bail!(
                "Canonical skill `{skill_name}` expects `{dir_name}` to be a directory when present: {}",
                path.display(),
            );
        }
    }

    Ok(())
}

fn collect_canonical_layout_warnings(
    skill_root: &Path,
    definition: &CanonicalSkillDefinition,
) -> Result<Vec<String>> {
    let allowed_entries = allowed_canonical_top_level_entries(definition);
    let mut warnings = Vec::new();
    let mut entries = fs::read_dir(skill_root)
        .with_context(|| {
            format!("Failed to read canonical skill directory: {}", skill_root.display())
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let entry_name = entry
            .file_name()
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid canonical skill entry name"))?
            .to_string();

        if entry_name.starts_with('.') {
            continue;
        }

        if allowed_entries.contains(&entry_name) {
            if entry_name == "targets" {
                warnings.extend(collect_canonical_target_entry_warnings(
                    &skill_root.join("targets"),
                    &definition.name,
                )?);
            }
            continue;
        }

        warnings.push(format!(
            "Canonical skill `{}` contains unsupported top-level entry `{entry_name}`; only {} are rendered, so this entry will be ignored.",
            definition.name,
            format_canonical_top_level_entries(&allowed_entries),
        ));
    }

    Ok(warnings)
}

fn collect_canonical_target_entry_warnings(
    targets_dir: &Path,
    skill_name: &str,
) -> Result<Vec<String>> {
    if !targets_dir.exists() {
        return Ok(Vec::new());
    }

    let allowed_entries = allowed_canonical_target_entries();
    let mut entries = fs::read_dir(targets_dir)
        .with_context(|| format!("Failed to read targets directory: {}", targets_dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    Ok(entries
        .into_iter()
        .filter_map(|entry| {
            let entry_name = entry.file_name().to_str()?.to_string();
            if entry_name.starts_with('.') || allowed_entries.contains(&entry_name) {
                return None;
            }

            Some(format!(
                "Canonical skill `{skill_name}` contains unsupported targets entry `targets/{entry_name}`; keep per-agent metadata in skill.yaml and limit Markdown fragments to {}.",
                format_canonical_target_entries(&allowed_entries),
            ))
        })
        .collect())
}

fn allowed_canonical_top_level_entries(definition: &CanonicalSkillDefinition) -> BTreeSet<String> {
    let mut entries = BTreeSet::from([
        CANONICAL_SKILL_FILE_NAME.to_string(),
        definition.instructions_file.clone(),
        "targets".to_string(),
    ]);
    entries.extend(SKILL_RESOURCE_DIRS.iter().map(|dir_name| (*dir_name).to_string()));
    entries
}

fn allowed_canonical_target_entries() -> BTreeSet<String> {
    [
        SkillTargetName::Claude,
        SkillTargetName::ClaudeCode,
        SkillTargetName::Codex,
        SkillTargetName::Gemini,
    ]
    .into_iter()
    .flat_map(|target| {
        let label = target_name_label(target);
        [format!("{label}.prepend.md"), format!("{label}.append.md")]
    })
    .collect()
}

fn format_canonical_top_level_entries(entries: &BTreeSet<String>) -> String {
    entries
        .iter()
        .map(|entry| {
            if entry == CANONICAL_SKILL_FILE_NAME || is_markdown_file_name(entry) {
                format!("`{entry}`")
            } else {
                format!("`{entry}/`")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_canonical_target_entries(entries: &BTreeSet<String>) -> String {
    entries
        .iter()
        .map(|entry| format!("`targets/{entry}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn is_markdown_file_name(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

fn load_canonical_instructions(
    skill_root: &Path,
    definition: &CanonicalSkillDefinition,
    target_name: SkillTargetName,
) -> Result<String> {
    let instructions_path = skill_root.join(&definition.instructions_file);
    let mut body = fs::read_to_string(&instructions_path)
        .with_context(|| format!("Failed to read {}", instructions_path.display()))?;

    let prepend_path = skill_root
        .join("targets")
        .join(format!("{}.prepend.md", target_name_label(target_name)));
    if prepend_path.exists() {
        let prepend = fs::read_to_string(&prepend_path)
            .with_context(|| format!("Failed to read {}", prepend_path.display()))?;
        body = format!("{}\n\n{}", prepend.trim_end(), body.trim_start());
    }

    let append_path = skill_root
        .join("targets")
        .join(format!("{}.append.md", target_name_label(target_name)));
    if append_path.exists() {
        let append = fs::read_to_string(&append_path)
            .with_context(|| format!("Failed to read {}", append_path.display()))?;
        body = format!("{}\n\n{}", body.trim_end(), append.trim_start());
    }

    Ok(body)
}

fn render_codex_generated_files(
    name: &str,
    description: &str,
    instructions: &str,
    target_overlay: &SkillTargetOverlay,
) -> Result<Vec<RenderedTextFile>> {
    let mut files = vec![RenderedTextFile {
        relative_path: SKILL_FILE_NAME.to_string(),
        content: render_codex_skill_markdown(name, description, instructions)?,
    }];

    if let Some(openai_yaml) = render_codex_openai_yaml(target_overlay)? {
        files.push(RenderedTextFile {
            relative_path: "agents/openai.yaml".to_string(),
            content: openai_yaml,
        });
    }

    Ok(files)
}

fn render_legacy_skill_bundle(
    candidate: &SkillCandidate,
    render_agent: Agent,
    warnings: &mut BTreeSet<String>,
) -> Result<RenderedSkillBundle> {
    let skill_path = legacy_skill_markdown_path(candidate);
    let document = parse_legacy_skill_document(&skill_path)?;

    if let Some(parse_warning) = &document.parse_warning {
        warnings.insert(parse_warning.clone());
    }

    let mut generated_files = Vec::new();
    let mut resource_mappings = if candidate.kind == SkillCandidateKind::LegacyDirectory {
        collect_legacy_directory_resource_mappings(
            &candidate.path,
            &candidate.name,
            &BTreeSet::from([SKILL_FILE_NAME.to_string()]),
        )?
    } else {
        Vec::new()
    };

    if render_agent == Agent::Codex {
        if let Some(frontmatter) = &document.frontmatter {
            let Some((name, description)) = extract_name_and_description(frontmatter) else {
                warnings.insert(format!(
                    "Legacy skill `{}` contains YAML frontmatter without valid `name` and `description`; preserving the original SKILL.md for Codex.",
                    candidate.name
                ));
                generated_files.push(RenderedTextFile {
                    relative_path: SKILL_FILE_NAME.to_string(),
                    content: document.raw_content.clone(),
                });
                return Ok(RenderedSkillBundle {
                    name: candidate.name.clone(),
                    generated_files,
                    resource_mappings,
                });
            };

            if frontmatter_contains_any(frontmatter, CLAUDE_ONLY_FRONTMATTER_KEYS)
                && candidate.origin != SkillSourceOrigin::AgentOverride
            {
                warnings.insert(format!(
                    "Legacy shared skill `{}` contains Claude-specific metadata that will be dropped when rendering for Codex.",
                    candidate.name
                ));
            }

            generated_files.push(RenderedTextFile {
                relative_path: SKILL_FILE_NAME.to_string(),
                content: render_codex_skill_markdown(&name, &description, &document.body)?,
            });

            if let Some(openai_yaml) = render_codex_openai_yaml_from_legacy(frontmatter)? {
                if candidate.kind == SkillCandidateKind::LegacyDirectory {
                    resource_mappings = collect_legacy_directory_resource_mappings(
                        &candidate.path,
                        &candidate.name,
                        &BTreeSet::from([
                            SKILL_FILE_NAME.to_string(),
                            "agents/openai.yaml".to_string(),
                        ]),
                    )?;
                }
                generated_files.push(RenderedTextFile {
                    relative_path: "agents/openai.yaml".to_string(),
                    content: openai_yaml,
                });
            }
        } else {
            generated_files.push(RenderedTextFile {
                relative_path: SKILL_FILE_NAME.to_string(),
                content: document.raw_content.clone(),
            });
        }
    } else {
        generated_files.push(RenderedTextFile {
            relative_path: SKILL_FILE_NAME.to_string(),
            content: document.raw_content.clone(),
        });
    }

    Ok(RenderedSkillBundle { name: candidate.name.clone(), generated_files, resource_mappings })
}

fn legacy_skill_markdown_path(candidate: &SkillCandidate) -> PathBuf {
    match candidate.kind {
        SkillCandidateKind::LegacyDirectory => candidate.path.join(SKILL_FILE_NAME),
        SkillCandidateKind::CanonicalDirectory | SkillCandidateKind::LegacyFile => {
            candidate.path.clone()
        },
    }
}

fn parse_legacy_skill_document(path: &Path) -> Result<LegacySkillDocument> {
    let raw_content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;

    let Some(captures) = YAML_FRONTMATTER_RE.captures(&raw_content) else {
        return Ok(LegacySkillDocument {
            body: raw_content.clone(),
            frontmatter: None,
            parse_warning: None,
            raw_content,
        });
    };

    let frontmatter_text = captures.get(1).map(|capture| capture.as_str()).ok_or_else(|| {
        anyhow::anyhow!("Failed to extract YAML frontmatter from {}", path.display())
    })?;
    let body = captures.get(2).map(|capture| capture.as_str().to_string()).ok_or_else(|| {
        anyhow::anyhow!("Failed to extract Markdown body from {}", path.display())
    })?;

    let frontmatter = match serde_yaml::from_str::<YamlMapping>(frontmatter_text) {
        Ok(parsed) => Some(parsed),
        Err(error) => {
            return Ok(LegacySkillDocument {
                raw_content: raw_content.clone(),
                frontmatter: None,
                body: raw_content,
                parse_warning: Some(format!(
                    "Legacy skill {} has invalid YAML frontmatter and will be treated as raw Markdown: {}",
                    path.display(),
                    error
                )),
            });
        },
    };

    Ok(LegacySkillDocument { raw_content, frontmatter, body, parse_warning: None })
}

fn extract_name_and_description(frontmatter: &YamlMapping) -> Option<(String, String)> {
    let name = frontmatter
        .get(YamlValue::String("name".to_string()))
        .and_then(YamlValue::as_str)?;
    let description = frontmatter
        .get(YamlValue::String("description".to_string()))
        .and_then(YamlValue::as_str)?;

    Some((name.to_string(), description.to_string()))
}

fn frontmatter_contains_any(frontmatter: &YamlMapping, keys: &[&str]) -> bool {
    keys.iter()
        .any(|key| frontmatter.contains_key(YamlValue::String((*key).to_string())))
}

fn render_claude_family_skill_markdown(
    name: &str,
    description: &str,
    instructions: &str,
    target_overlay: &SkillTargetOverlay,
) -> Result<String> {
    let mut frontmatter = YamlMapping::new();
    frontmatter.insert(yaml_key("name"), YamlValue::String(name.to_string()));
    frontmatter.insert(yaml_key("description"), YamlValue::String(description.to_string()));

    let disable_model_invocation = target_overlay.disable_model_invocation.or_else(|| {
        target_overlay
            .invocation
            .map(|mode| matches!(mode, SkillInvocationMode::Manual))
    });
    insert_optional_yaml_value(
        &mut frontmatter,
        "disable-model-invocation",
        disable_model_invocation.map(YamlValue::Bool),
    );
    insert_optional_yaml_value(
        &mut frontmatter,
        "user-invocable",
        target_overlay.user_invocable.map(YamlValue::Bool),
    );
    insert_optional_yaml_value(
        &mut frontmatter,
        "allowed-tools",
        target_overlay
            .allowed_tools
            .as_ref()
            .map(|value| serde_yaml::to_value(value).expect("allowed tools should serialize")),
    );
    insert_optional_yaml_value(&mut frontmatter, "arguments", target_overlay.arguments.clone());
    insert_optional_yaml_value(
        &mut frontmatter,
        "argument-hint",
        target_overlay.argument_hint.clone().map(YamlValue::String),
    );
    insert_optional_yaml_value(&mut frontmatter, "context", target_overlay.context.clone());
    insert_optional_yaml_value(
        &mut frontmatter,
        "agent",
        target_overlay.agent.clone().map(YamlValue::String),
    );

    render_markdown_with_frontmatter(&frontmatter, instructions)
}

fn render_codex_skill_markdown(
    name: &str,
    description: &str,
    instructions: &str,
) -> Result<String> {
    let mut frontmatter = YamlMapping::new();
    frontmatter.insert(yaml_key("name"), YamlValue::String(name.to_string()));
    frontmatter.insert(yaml_key("description"), YamlValue::String(description.to_string()));
    render_markdown_with_frontmatter(&frontmatter, instructions)
}

fn render_codex_openai_yaml(target_overlay: &SkillTargetOverlay) -> Result<Option<String>> {
    let mut document = YamlMapping::new();

    if let Some(interface) = &target_overlay.interface {
        document.insert(yaml_key("interface"), interface.clone());
    }
    if let Some(dependencies) = &target_overlay.dependencies {
        document.insert(yaml_key("dependencies"), dependencies.clone());
    }

    if let Some(invocation) = target_overlay.invocation {
        let mut policy = YamlMapping::new();
        policy.insert(
            yaml_key("allow_implicit_invocation"),
            YamlValue::Bool(matches!(invocation, SkillInvocationMode::Auto)),
        );
        document.insert(yaml_key("policy"), YamlValue::Mapping(policy));
    }

    if document.is_empty() {
        return Ok(None);
    }

    Ok(Some(serialize_yaml_document(&YamlValue::Mapping(document))?))
}

fn render_codex_openai_yaml_from_legacy(frontmatter: &YamlMapping) -> Result<Option<String>> {
    let disable_model_invocation = frontmatter
        .get(YamlValue::String("disable-model-invocation".to_string()))
        .and_then(YamlValue::as_bool);

    if disable_model_invocation != Some(true) {
        return Ok(None);
    }

    let mut policy = YamlMapping::new();
    policy.insert(yaml_key("allow_implicit_invocation"), YamlValue::Bool(false));

    let mut document = YamlMapping::new();
    document.insert(yaml_key("policy"), YamlValue::Mapping(policy));
    Ok(Some(serialize_yaml_document(&YamlValue::Mapping(document))?))
}

fn render_markdown_with_frontmatter(frontmatter: &YamlMapping, body: &str) -> Result<String> {
    let serialized = serialize_yaml_document(&YamlValue::Mapping(frontmatter.clone()))?;
    let trimmed_body = body.trim_end();

    if trimmed_body.is_empty() {
        return Ok(format!("---\n{serialized}---\n"));
    }

    Ok(format!("---\n{serialized}---\n\n{trimmed_body}\n"))
}

fn serialize_yaml_document(value: &YamlValue) -> Result<String> {
    let serialized = serde_yaml::to_string(value).context("Failed to serialize YAML")?;
    Ok(serialized.strip_prefix("---\n").unwrap_or(&serialized).to_string())
}

fn yaml_key(name: &str) -> YamlValue {
    YamlValue::String(name.to_string())
}

fn insert_optional_yaml_value(mapping: &mut YamlMapping, key: &str, value: Option<YamlValue>) {
    if let Some(value) = value {
        mapping.insert(yaml_key(key), value);
    }
}

fn collect_canonical_resource_mappings(
    skill_root: &Path,
    skill_name: &str,
) -> Result<Vec<SourceFileMapping>> {
    let mut mappings = Vec::new();

    for dir_name in SKILL_RESOURCE_DIRS {
        let path = skill_root.join(dir_name);
        if !path.exists() {
            continue;
        }

        let nested = asset_sync::collect_directory_tree_mappings(&path)?;
        mappings.extend(nested.into_iter().map(|mapping| SourceFileMapping {
            source_path: mapping.source_path,
            relative_path: normalize_skill_relative_path(
                skill_name,
                &format!("{dir_name}/{}", mapping.relative_path),
            ),
        }));
    }

    mappings.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(mappings)
}

fn collect_legacy_directory_resource_mappings(
    skill_root: &Path,
    skill_name: &str,
    excluded_relative_paths: &BTreeSet<String>,
) -> Result<Vec<SourceFileMapping>> {
    let mappings = asset_sync::collect_directory_tree_mappings(skill_root)?
        .into_iter()
        .filter(|mapping| !excluded_relative_paths.contains(&mapping.relative_path))
        .map(|mapping| SourceFileMapping {
            source_path: mapping.source_path,
            relative_path: normalize_skill_relative_path(skill_name, &mapping.relative_path),
        })
        .collect::<Vec<_>>();

    Ok(mappings)
}

fn materialize_rendered_bundle(
    bundle: &RenderedSkillBundle,
    render_workspace: &Path,
) -> Result<Vec<SourceFileMapping>> {
    let skill_workspace = render_workspace.join(&bundle.name);
    fs::create_dir_all(&skill_workspace).with_context(|| {
        format!("Failed to create render workspace {}", skill_workspace.display())
    })?;

    let mut mappings = bundle.resource_mappings.clone();
    for generated_file in &bundle.generated_files {
        let generated_path = skill_workspace.join(&generated_file.relative_path);
        if let Some(parent) = generated_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        fs::write(&generated_path, &generated_file.content)
            .with_context(|| format!("Failed to write {}", generated_path.display()))?;
        mappings.push(SourceFileMapping {
            source_path: generated_path,
            relative_path: normalize_skill_relative_path(
                &bundle.name,
                &generated_file.relative_path,
            ),
        });
    }

    mappings.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(mappings)
}

fn target_name_label(target: SkillTargetName) -> &'static str {
    match target {
        SkillTargetName::Claude => "claude",
        SkillTargetName::ClaudeCode => "claude-code",
        SkillTargetName::Codex => "codex",
        SkillTargetName::Gemini => "gemini",
    }
}

/// List all skills in a directory.
///
/// # Errors
///
/// Returns an error if:
/// - Unable to read the skills directory
/// - Unable to access directory entries
pub fn list_skills(skills_dir: &Path) -> Result<Vec<String>> {
    let mut skills = Vec::new();

    if !skills_dir.exists() {
        return Ok(skills);
    }

    let entries = fs::read_dir(skills_dir)
        .with_context(|| format!("Failed to read skills directory: {}", skills_dir.display()))?;

    for entry in entries {
        let dir_entry = entry?;
        let path = dir_entry.path();

        if path.is_dir() {
            if let Some(skill_name) = dir_entry.file_name().to_str() {
                skills.push(skill_name.to_string());
            }
            continue;
        }

        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
            if let Some(skill_name) = path.file_stem().and_then(|s| s.to_str()) {
                skills.push(skill_name.to_string());
            }
        }
    }

    skills.sort();
    skills.dedup();
    Ok(skills)
}

/// Ensure skills directory exists.
///
/// # Errors
///
/// Returns an error if unable to create the skills directory.
pub fn ensure_skills_directory(skills_dir: &Path) -> Result<()> {
    fs::create_dir_all(skills_dir)
        .with_context(|| format!("Failed to create skills directory: {}", skills_dir.display()))?;
    Ok(())
}

/// Collect source-to-target mappings for skill deployment.
///
/// # Errors
///
/// Returns an error if the source tree cannot be read or if a skill name cannot
/// be derived from the source path.
pub fn collect_skill_mappings(source_dir_opt: Option<&Path>) -> Result<Vec<SourceFileMapping>> {
    let Some(source_dir) = source_dir_opt else {
        return Ok(Vec::new());
    };

    if !source_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = fs::read_dir(source_dir)
        .with_context(|| format!("Failed to read skills directory: {}", source_dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    let mut mappings = Vec::new();

    for entry in entries {
        let path = entry.path();

        if path.is_dir() {
            let skill_name = entry
                .file_name()
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid skill directory name"))?
                .to_string();
            let skill_files = asset_sync::collect_directory_tree_mappings(&path)?;
            mappings.extend(skill_files.into_iter().map(|mapping| SourceFileMapping {
                source_path: mapping.source_path,
                relative_path: normalize_skill_relative_path(&skill_name, &mapping.relative_path),
            }));
            continue;
        }

        if path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("md") {
            let skill_name = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid skill file stem"))?;
            let relative_path = normalize_skill_relative_path(skill_name, SKILL_FILE_NAME);

            mappings.push(SourceFileMapping { source_path: path, relative_path });
        }
    }

    mappings.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(mappings)
}

/// Collect shared, legacy, and agent-specific skill mappings from the Claudius
/// config tree with override precedence applied per skill name.
///
/// Precedence order:
/// 1. legacy `commands/*.md`
/// 2. shared `skills/<skill>/...`
/// 3. agent-specific `skills/<agent>/<skill>/...`
///
/// A higher-precedence source replaces the full skill directory from a
/// lower-precedence source.
///
/// # Errors
///
/// Returns an error if the Claudius skill or command trees cannot be read.
pub fn collect_claudius_skill_source_set(
    config_dir: &Path,
    agent: Option<Agent>,
) -> Result<SkillSourceSet> {
    let skills_root = config_dir.join("skills");
    let commands_root = config_dir.join("commands");
    let render_agent = effective_render_agent(agent);
    let (candidates, includes_legacy_commands) =
        discover_skill_candidates(&skills_root, &commands_root, agent)?;
    let render_workspace = TempDir::new().context("Failed to create temporary skill render dir")?;
    let mut warnings = BTreeSet::new();

    let mut mappings = Vec::new();
    for candidate in &candidates {
        if candidate.origin == SkillSourceOrigin::AgentOverride {
            warnings.insert(format!(
                "Deprecated full agent override directory detected for skill `{}` under skills/{}/{}; prefer canonical target overlays in skill.yaml.",
                candidate.name,
                agent_skill_subdir(render_agent),
                candidate.name,
            ));
        }

        mappings.extend(render_candidate_to_mappings(
            candidate,
            render_agent,
            render_workspace.path(),
            &mut warnings,
        )?);
    }

    mappings.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(SkillSourceSet {
        mappings,
        includes_legacy_commands,
        warnings: warnings.into_iter().collect(),
        _render_workspace: Some(render_workspace),
    })
}

/// Collect mappings for shared skills in `skills/`, excluding agent-specific
/// subdirectories such as `skills/gemini/`.
///
/// # Errors
///
/// Returns an error if the shared skills tree cannot be read.
pub fn collect_shared_skill_mappings(skills_root: &Path) -> Result<Vec<SourceFileMapping>> {
    if !skills_root.exists() {
        return Ok(Vec::new());
    }

    let mut entries = fs::read_dir(skills_root)
        .with_context(|| format!("Failed to read skills directory: {}", skills_root.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    let mut mappings = Vec::new();
    for entry in entries {
        let path = entry.path();
        let entry_name = entry
            .file_name()
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid skill entry name"))?
            .to_string();

        if path.is_dir() {
            if is_agent_skill_subdir(&entry_name) {
                continue;
            }

            let skill_files = asset_sync::collect_directory_tree_mappings(&path)?;
            mappings.extend(skill_files.into_iter().map(|mapping| SourceFileMapping {
                source_path: mapping.source_path,
                relative_path: normalize_skill_relative_path(&entry_name, &mapping.relative_path),
            }));
            continue;
        }

        if path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("md") {
            let skill_name_str = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid skill file stem"))?;
            let skill_name = skill_name_str.to_string();
            mappings.push(SourceFileMapping {
                source_path: path,
                relative_path: normalize_skill_relative_path(&skill_name, SKILL_FILE_NAME),
            });
        }
    }

    mappings.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(mappings)
}

/// Collect mappings for agent-specific skills under `skills/<agent>/`.
///
/// # Errors
///
/// Returns an error if the agent-specific skill tree cannot be read.
pub fn collect_agent_skill_mappings(
    skills_root: &Path,
    agent: Agent,
) -> Result<Vec<SourceFileMapping>> {
    let candidate = skills_root.join(agent_skill_subdir(agent));
    collect_skill_mappings(candidate.exists().then_some(candidate.as_path()))
}

/// Collect mappings for legacy `commands/*.md` fallback skills.
///
/// # Errors
///
/// Returns an error if the commands directory cannot be read.
pub fn collect_legacy_command_mappings(commands_root: &Path) -> Result<Vec<SourceFileMapping>> {
    if !commands_root.exists() {
        return Ok(Vec::new());
    }

    let mut entries = fs::read_dir(commands_root)
        .with_context(|| format!("Failed to read commands directory: {}", commands_root.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    let mut mappings = Vec::new();
    for entry in entries {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }

        let skill_name_str = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid skill file stem"))?;
        let skill_name = skill_name_str.to_string();
        mappings.push(SourceFileMapping {
            source_path: path,
            relative_path: normalize_skill_relative_path(&skill_name, SKILL_FILE_NAME),
        });
    }

    Ok(mappings)
}

fn skill_names_from_mappings(mappings: &[SourceFileMapping]) -> Vec<String> {
    mappings
        .iter()
        .filter_map(|mapping| mapping.relative_path.split('/').next().map(str::to_string))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn agent_skill_subdir(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => "claude",
        Agent::ClaudeCode => "claude-code",
        Agent::Codex => "codex",
        Agent::Gemini => "gemini",
    }
}

pub fn is_agent_skill_subdir(name: &str) -> bool {
    matches!(name, "claude" | "claude-code" | "codex" | "gemini")
}

fn normalize_skill_relative_path(skill_name: &str, suffix: &str) -> String {
    format!("{skill_name}/{}", suffix.replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sync_skills_empty_source() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");

        let result = sync_skills(&source_dir, &target_dir).expect("sync_skills should succeed");
        assert!(result.is_empty());
        assert!(target_dir.exists());
    }

    #[test]
    fn test_sync_skills_with_directories() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");

        let skill_dir = source_dir.join("review");
        fs::create_dir_all(&skill_dir).expect("Failed to create skill directory");
        fs::write(skill_dir.join(SKILL_FILE_NAME), "# Review Skill")
            .expect("Failed to write SKILL.md");
        fs::write(skill_dir.join("prompt.txt"), "Extra content")
            .expect("Failed to write prompt.txt");

        let result = sync_skills(&source_dir, &target_dir).expect("sync_skills should succeed");
        assert_eq!(result, vec!["review".to_string()]);

        assert!(target_dir.join("review").exists());
        assert!(target_dir.join("review").join(SKILL_FILE_NAME).exists());
        assert!(target_dir.join("review").join("prompt.txt").exists());
    }

    #[test]
    fn test_sync_skills_with_legacy_md_files() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&source_dir).expect("Failed to create source directory");
        fs::write(source_dir.join("command1.md"), "# Command 1")
            .expect("Failed to write command1.md");
        fs::write(source_dir.join("ignore.txt"), "Ignore me").expect("Failed to write ignore.txt");

        let result = sync_skills(&source_dir, &target_dir).expect("sync_skills should succeed");
        assert_eq!(result, vec!["command1".to_string()]);

        let target_skill = target_dir.join("command1");
        assert!(target_skill.exists());
        assert!(target_skill.join(SKILL_FILE_NAME).exists());

        let content = fs::read_to_string(target_skill.join(SKILL_FILE_NAME))
            .expect("Failed to read SKILL.md");
        assert_eq!(content, "# Command 1");
    }

    #[test]
    fn test_collect_skill_mappings_supports_directory_and_legacy_sources() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let source_dir = temp_dir.path().join("source");
        let skill_dir = source_dir.join("review");
        fs::create_dir_all(skill_dir.join("prompts")).expect("Failed to create prompts dir");
        fs::write(skill_dir.join(SKILL_FILE_NAME), "# Review Skill")
            .expect("Failed to write SKILL.md");
        fs::write(skill_dir.join("prompts").join("summary.txt"), "Summarize")
            .expect("Failed to write summary.txt");
        fs::write(source_dir.join("legacy.md"), "# Legacy Skill").expect("Failed to write legacy");

        let mappings = collect_skill_mappings(Some(&source_dir)).expect("collect mappings");
        let relative_paths =
            mappings.iter().map(|mapping| mapping.relative_path.clone()).collect::<Vec<_>>();

        assert_eq!(
            relative_paths,
            vec![
                "legacy/SKILL.md".to_string(),
                "review/SKILL.md".to_string(),
                "review/prompts/summary.txt".to_string(),
            ]
        );
    }

    #[test]
    fn test_sync_skills_with_options_prunes_removed_skill_files() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");
        let review_dir = source_dir.join("review");
        fs::create_dir_all(&review_dir).expect("Failed to create review dir");
        fs::write(review_dir.join(SKILL_FILE_NAME), "# Review Skill")
            .expect("Failed to write SKILL.md");

        sync_skills_with_options(
            Some(&source_dir),
            &target_dir,
            SyncBehavior { dry_run: false, prune: false },
        )
        .expect("initial sync should succeed");

        fs::remove_dir_all(&review_dir).expect("Failed to remove review dir");
        let report = sync_skills_with_options(
            Some(&source_dir),
            &target_dir,
            SyncBehavior { dry_run: false, prune: true },
        )
        .expect("prune sync should succeed");

        assert_eq!(report.pruned_files, vec!["review/SKILL.md".to_string()]);
        assert!(!target_dir.join("review").exists());
    }

    #[test]
    fn test_collect_shared_skill_mappings_skips_agent_subdirs() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let skills_dir = temp_dir.path().join("skills");
        let shared_dir = skills_dir.join("shared");
        let gemini_dir = skills_dir.join("gemini").join("agent-only");

        fs::create_dir_all(&shared_dir).expect("Failed to create shared skill");
        fs::create_dir_all(&gemini_dir).expect("Failed to create gemini skill");
        fs::write(shared_dir.join(SKILL_FILE_NAME), "# Shared").expect("Failed to write shared");
        fs::write(gemini_dir.join(SKILL_FILE_NAME), "# Gemini").expect("Failed to write gemini");

        let mappings =
            collect_shared_skill_mappings(&skills_dir).expect("shared skill mappings should load");
        let relative_paths =
            mappings.iter().map(|mapping| mapping.relative_path.clone()).collect::<Vec<_>>();

        assert_eq!(relative_paths, vec!["shared/SKILL.md".to_string()]);
    }

    #[test]
    fn test_collect_claudius_skill_source_set_prefers_agent_specific_over_shared_and_legacy() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_dir = temp_dir.path().join("claudius");
        let shared_dir = config_dir.join("skills").join("review");
        let agent_dir = config_dir.join("skills").join("gemini").join("review");
        let commands_dir = config_dir.join("commands");

        fs::create_dir_all(&shared_dir).expect("Failed to create shared dir");
        fs::create_dir_all(&agent_dir).expect("Failed to create agent dir");
        fs::create_dir_all(&commands_dir).expect("Failed to create commands dir");

        fs::write(commands_dir.join("review.md"), "# Legacy").expect("Failed to write legacy");
        fs::write(shared_dir.join(SKILL_FILE_NAME), "# Shared").expect("Failed to write shared");
        fs::write(agent_dir.join(SKILL_FILE_NAME), "# Agent").expect("Failed to write agent");

        let source_set = collect_claudius_skill_source_set(&config_dir, Some(Agent::Gemini))
            .expect("skill source set should load");
        let first_mapping = source_set.mappings.first().expect("expected a mapping");

        assert!(source_set.includes_legacy_commands);
        assert_eq!(source_set.mappings.len(), 1);
        assert_eq!(first_mapping.relative_path, "review/SKILL.md");
        assert_eq!(
            fs::read_to_string(&first_mapping.source_path).expect("source should read"),
            "# Agent"
        );
    }

    #[test]
    fn test_collect_claudius_skill_source_set_renders_canonical_codex_bundle() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_dir = temp_dir.path().join("claudius");
        let skill_dir = config_dir.join("skills").join("setup-commitlint");

        fs::create_dir_all(skill_dir.join("scripts")).expect("Failed to create skill scripts dir");
        fs::write(
            skill_dir.join(CANONICAL_SKILL_FILE_NAME),
            "version: 1\nname: setup-commitlint\ndescription: Set up commitlint.\ntargets:\n  codex:\n    invocation: manual\n    interface:\n      display_name: Commitlint Setup\n",
        )
        .expect("Failed to write skill.yaml");
        fs::write(
            skill_dir.join(DEFAULT_CANONICAL_INSTRUCTIONS_FILE),
            "Set up commitlint in the current repository.\n",
        )
        .expect("Failed to write instructions");
        fs::write(skill_dir.join("scripts").join("setup.sh"), "#!/usr/bin/env bash\necho setup\n")
            .expect("Failed to write script");

        let source_set = collect_claudius_skill_source_set(&config_dir, Some(Agent::Codex))
            .expect("canonical codex source set should render");
        let relative_paths = source_set
            .mappings
            .iter()
            .map(|mapping| mapping.relative_path.clone())
            .collect::<Vec<_>>();

        assert!(relative_paths.contains(&"setup-commitlint/SKILL.md".to_string()));
        assert!(relative_paths.contains(&"setup-commitlint/agents/openai.yaml".to_string()));
        assert!(relative_paths.contains(&"setup-commitlint/scripts/setup.sh".to_string()));

        let skill_mapping = source_set
            .mappings
            .iter()
            .find(|mapping| mapping.relative_path == "setup-commitlint/SKILL.md")
            .expect("SKILL.md mapping should exist");
        let skill_content =
            fs::read_to_string(&skill_mapping.source_path).expect("Rendered SKILL.md should read");
        assert!(skill_content.contains("name: setup-commitlint"));
        assert!(skill_content.contains("description: Set up commitlint."));
        assert!(!skill_content.contains("display_name"));

        let openai_mapping = source_set
            .mappings
            .iter()
            .find(|mapping| mapping.relative_path == "setup-commitlint/agents/openai.yaml")
            .expect("openai.yaml mapping should exist");
        let openai_content = fs::read_to_string(&openai_mapping.source_path)
            .expect("Rendered openai.yaml should read");
        assert!(openai_content.contains("display_name: Commitlint Setup"));
        assert!(openai_content.contains("allow_implicit_invocation: false"));
    }

    #[test]
    fn test_collect_claudius_skill_source_set_rejects_mixed_canonical_and_legacy_dir() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_dir = temp_dir.path().join("claudius");
        let skill_dir = config_dir.join("skills").join("mixed-skill");

        fs::create_dir_all(&skill_dir).expect("Failed to create skill dir");
        fs::write(skill_dir.join(SKILL_FILE_NAME), "# Legacy").expect("Failed to write legacy");
        fs::write(
            skill_dir.join(CANONICAL_SKILL_FILE_NAME),
            "version: 1\nname: mixed-skill\ndescription: Mixed\n",
        )
        .expect("Failed to write canonical");

        let error = collect_claudius_skill_source_set(&config_dir, Some(Agent::ClaudeCode))
            .expect_err("mixed skill dir should be rejected");
        assert!(error.to_string().contains("contains both skill.yaml and SKILL.md"));
    }

    #[test]
    fn test_list_skills_empty_directory() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let skills_dir = temp_dir.path().join("skills");

        let result = list_skills(&skills_dir).expect("list_skills should succeed");
        assert!(result.is_empty());

        fs::create_dir_all(&skills_dir).expect("Failed to create skills directory");
        let result_after_create =
            list_skills(&skills_dir).expect("list_skills should succeed after create");
        assert!(result_after_create.is_empty());
    }

    #[test]
    fn test_list_skills_with_directories() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let skills_dir = temp_dir.path().join("skills");

        fs::create_dir_all(skills_dir.join("alpha")).expect("Failed to create alpha skill");
        fs::create_dir_all(skills_dir.join("bravo")).expect("Failed to create bravo skill");
        fs::write(skills_dir.join("legacy.md"), "Legacy").expect("Failed to write legacy.md");

        let result = list_skills(&skills_dir).expect("list_skills should succeed");
        assert_eq!(result, vec!["alpha", "bravo", "legacy"]);
    }

    #[test]
    fn test_ensure_skills_directory_new() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let skills_dir = temp_dir.path().join("new/skills/dir");

        assert!(!skills_dir.exists());
        ensure_skills_directory(&skills_dir).expect("ensure_skills_directory should succeed");
        assert!(skills_dir.exists());
        assert!(skills_dir.is_dir());
    }
}
