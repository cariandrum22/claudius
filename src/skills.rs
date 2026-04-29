use crate::{
    app_config::Agent,
    asset_sync::{self, ManagedTreeSyncReport, SourceFileMapping, SyncBehavior},
};
use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const SKILL_FILE_NAME: &str = "SKILL.md";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillSyncReport {
    pub target_dir: PathBuf,
    pub synced_skills: Vec<String>,
    pub synced_files: Vec<String>,
    pub pruned_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillSourceSet {
    pub mappings: Vec<SourceFileMapping>,
    pub includes_legacy_commands: bool,
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
    let legacy_mappings = collect_legacy_command_mappings(&commands_root)?;
    let includes_legacy_commands = !legacy_mappings.is_empty();
    let shared_mappings = collect_shared_skill_mappings(&skills_root)?;
    let agent_mappings = agent.map_or_else(
        || Ok(Vec::new()),
        |selected| collect_agent_skill_mappings(&skills_root, selected),
    )?;

    let mut merged = BTreeMap::<String, Vec<SourceFileMapping>>::new();
    extend_skill_groups(&mut merged, legacy_mappings);
    extend_skill_groups(&mut merged, shared_mappings);
    extend_skill_groups(&mut merged, agent_mappings);

    let mappings = merged
        .into_values()
        .flat_map(std::iter::IntoIterator::into_iter)
        .collect::<Vec<_>>();

    Ok(SkillSourceSet { mappings, includes_legacy_commands })
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

fn extend_skill_groups(
    groups: &mut BTreeMap<String, Vec<SourceFileMapping>>,
    mappings: Vec<SourceFileMapping>,
) {
    let mut grouped = BTreeMap::<String, Vec<SourceFileMapping>>::new();
    for mapping in mappings {
        let skill_name = mapping
            .relative_path
            .split('/')
            .next()
            .expect("skill mappings always contain a top-level directory")
            .to_string();
        grouped.entry(skill_name).or_default().push(mapping);
    }

    for (skill_name, mut group_mappings) in grouped {
        group_mappings.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        groups.insert(skill_name, group_mappings);
    }
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
