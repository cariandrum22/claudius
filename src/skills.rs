use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const SKILL_FILE_NAME: &str = "SKILL.md";

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
    // Ensure target directory exists
    fs::create_dir_all(target_dir)
        .with_context(|| format!("Failed to create target directory: {}", target_dir.display()))?;

    if !source_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(source_dir)
        .with_context(|| format!("Failed to read skills directory: {}", source_dir.display()))?;

    let mut synced_skills = BTreeSet::new();

    for entry in entries {
        let dir_entry = entry?;
        let path = dir_entry.path();

        if path.is_dir() {
            let skill_name = dir_entry
                .file_name()
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid skill directory name"))?
                .to_string();

            let target_skill_dir = target_dir.join(&skill_name);
            copy_dir_recursive(&path, &target_skill_dir)?;
            synced_skills.insert(skill_name);
            continue;
        }

        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
            let skill_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid skill file stem"))?;

            let target_skill_dir = target_dir.join(skill_name);
            fs::create_dir_all(&target_skill_dir).with_context(|| {
                format!(
                    "Failed to create skill directory: {}",
                    target_skill_dir.display()
                )
            })?;

            let target_path = target_skill_dir.join(SKILL_FILE_NAME);
            fs::copy(&path, &target_path).with_context(|| {
                format!(
                    "Failed to copy skill from {} to {}",
                    path.display(),
                    target_path.display()
                )
            })?;

            synced_skills.insert(skill_name.to_string());
        }
    }

    Ok(synced_skills.into_iter().collect())
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

    let entries = fs::read_dir(skills_dir).with_context(|| {
        format!("Failed to read skills directory: {}", skills_dir.display())
    })?;

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
    fs::create_dir_all(skills_dir).with_context(|| {
        format!("Failed to create skills directory: {}", skills_dir.display())
    })?;
    Ok(())
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)
        .with_context(|| format!("Failed to create directory: {}", target.display()))?;

    for entry in fs::read_dir(source)
        .with_context(|| format!("Failed to read directory: {}", source.display()))?
    {
        let dir_entry = entry?;
        let path = dir_entry.path();
        let target_path = target.join(dir_entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &target_path)?;
        } else if path.is_file() {
            fs::copy(&path, &target_path).with_context(|| {
                format!(
                    "Failed to copy file from {} to {}",
                    path.display(),
                    target_path.display()
                )
            })?;
        }
    }

    Ok(())
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
        fs::write(source_dir.join("ignore.txt"), "Ignore me")
            .expect("Failed to write ignore.txt");

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
        fs::write(skills_dir.join("legacy.md"), "Legacy")
            .expect("Failed to write legacy.md");

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
