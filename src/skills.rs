use crate::asset_sync::{self, ManagedTreeSyncReport, SourceFileMapping, SyncBehavior};
use anyhow::{Context, Result};
use std::collections::BTreeSet;
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
    let synced_skills = skill_names_from_mappings(&mappings);
    let ManagedTreeSyncReport { target_dir: synced_target_dir, synced_files, pruned_files } =
        asset_sync::sync_managed_tree(target_dir, &mappings, behavior)?;

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

fn skill_names_from_mappings(mappings: &[SourceFileMapping]) -> Vec<String> {
    mappings
        .iter()
        .filter_map(|mapping| mapping.relative_path.split('/').next().map(str::to_string))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
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
