use anyhow::Result;
use claudius::skills;
use std::fs;
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_skills_empty_source() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        // Source doesn't exist
        let synced = skills::sync_skills(&source, &target)?;
        anyhow::ensure!(synced.is_empty());

        // Target directory should be created
        anyhow::ensure!(target.exists());

        Ok(())
    }

    #[test]
    fn test_sync_skills_with_directories() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        // Create source directory and skill folders
        let alpha_dir = source.join("alpha");
        let bravo_dir = source.join("bravo");
        fs::create_dir_all(&alpha_dir)?;
        fs::create_dir_all(&bravo_dir)?;
        fs::write(alpha_dir.join("SKILL.md"), "# Alpha Skill\nThis is a test")?;
        fs::write(bravo_dir.join("SKILL.md"), "# Bravo Skill\nAnother skill")?;
        fs::write(source.join("not-a-skill.txt"), "This should be ignored")?;

        // Sync skills
        let synced = skills::sync_skills(&source, &target)?;

        // Check results
        anyhow::ensure!(synced.len() == 2, "Expected 2 synced skills");
        anyhow::ensure!(synced.contains(&"alpha".to_string()));
        anyhow::ensure!(synced.contains(&"bravo".to_string()));

        // Check target files exist with SKILL.md
        anyhow::ensure!(target.join("alpha/SKILL.md").exists());
        anyhow::ensure!(target.join("bravo/SKILL.md").exists());
        anyhow::ensure!(!target.join("not-a-skill.txt").exists());

        // Check content
        let content = fs::read_to_string(target.join("alpha/SKILL.md"))?;
        anyhow::ensure!(
            content == "# Alpha Skill\nThis is a test",
            "Content mismatch for alpha skill"
        );

        Ok(())
    }

    #[test]
    fn test_list_skills() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let skills_dir = temp_dir.path().join("skills");

        // Non-existent directory
        let skills_list = skills::list_skills(&skills_dir)?;
        anyhow::ensure!(skills_list.is_empty());

        // Create directory and skills
        fs::create_dir_all(skills_dir.join("alpha"))?;
        fs::create_dir_all(skills_dir.join("bravo"))?;
        fs::write(skills_dir.join("legacy.md"), "Legacy")?;

        // List skills
        let listed_skills = skills::list_skills(&skills_dir)?;
        anyhow::ensure!(listed_skills.len() == 3, "Expected 3 skills");
        anyhow::ensure!(
            listed_skills.first() == Some(&"alpha".to_string()),
            "First skill should be alpha"
        );
        anyhow::ensure!(
            listed_skills.get(1) == Some(&"bravo".to_string()),
            "Second skill should be bravo"
        );

        Ok(())
    }

    #[test]
    fn test_ensure_skills_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let skills_dir = temp_dir.path().join("deep").join("nested").join("skills");

        // Directory doesn't exist
        anyhow::ensure!(!skills_dir.exists());

        // Ensure directory
        skills::ensure_skills_directory(&skills_dir)?;

        // Directory should exist
        anyhow::ensure!(skills_dir.exists());
        anyhow::ensure!(skills_dir.is_dir());

        Ok(())
    }

    #[test]
    fn test_sync_skills_overwrite_existing() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        // Create source and target directories
        let source_skill_dir = source.join("command");
        let target_skill_dir = target.join("command");
        fs::create_dir_all(&source_skill_dir)?;
        fs::create_dir_all(&target_skill_dir)?;

        // Create initial files
        fs::write(source_skill_dir.join("SKILL.md"), "New content")?;
        fs::write(target_skill_dir.join("SKILL.md"), "Old content")?;

        // Sync skills
        let synced = skills::sync_skills(&source, &target)?;

        // Check that file was overwritten
        anyhow::ensure!(synced.len() == 1, "Expected 1 synced skill");
        let content = fs::read_to_string(target.join("command/SKILL.md"))?;
        anyhow::ensure!(content == "New content", "Content should be overwritten");

        Ok(())
    }
}
