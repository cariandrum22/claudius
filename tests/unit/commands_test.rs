use anyhow::Result;
use claudius::commands;
use std::fs;
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_commands_empty_source() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        // Source doesn't exist
        let synced = commands::sync_commands(&source, &target)?;
        anyhow::ensure!(synced.is_empty());

        // Target directory should be created
        anyhow::ensure!(target.exists());

        Ok(())
    }

    #[test]
    fn test_sync_commands_with_markdown_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        // Create source directory and files
        fs::create_dir_all(&source)?;
        fs::write(source.join("test-command.md"), "# Test Command\nThis is a test")?;
        fs::write(source.join("another.md"), "# Another\nAnother command")?;
        fs::write(source.join("not-markdown.txt"), "This should be ignored")?;

        // Sync commands
        let synced = commands::sync_commands(&source, &target)?;

        // Check results
        anyhow::ensure!(synced.len() == 2, "Expected 2 synced commands");
        anyhow::ensure!(synced.contains(&"test-command".to_string()));
        anyhow::ensure!(synced.contains(&"another".to_string()));

        // Check target files exist without extension
        anyhow::ensure!(target.join("test-command").exists());
        anyhow::ensure!(target.join("another").exists());
        anyhow::ensure!(!target.join("not-markdown").exists());
        anyhow::ensure!(!target.join("not-markdown.txt").exists());

        // Check content
        let content = fs::read_to_string(target.join("test-command"))?;
        anyhow::ensure!(
            content == "# Test Command\nThis is a test",
            "Content mismatch for test-command"
        );

        Ok(())
    }

    #[test]
    fn test_list_commands() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let commands_dir = temp_dir.path().join("commands");

        // Non-existent directory
        let commands = commands::list_commands(&commands_dir)?;
        anyhow::ensure!(commands.is_empty());

        // Create directory and files
        fs::create_dir_all(&commands_dir)?;
        fs::write(commands_dir.join("cmd1.md"), "Command 1")?;
        fs::write(commands_dir.join("cmd2.md"), "Command 2")?;
        fs::write(commands_dir.join("ignored.txt"), "Ignored")?;

        // List commands
        let listed_commands = commands::list_commands(&commands_dir)?;
        anyhow::ensure!(listed_commands.len() == 2, "Expected 2 commands");
        anyhow::ensure!(
            listed_commands.first() == Some(&"cmd1".to_string()),
            "First command should be cmd1"
        );
        anyhow::ensure!(
            listed_commands.get(1) == Some(&"cmd2".to_string()),
            "Second command should be cmd2"
        );

        Ok(())
    }

    #[test]
    fn test_ensure_commands_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let commands_dir = temp_dir.path().join("deep").join("nested").join("commands");

        // Directory doesn't exist
        anyhow::ensure!(!commands_dir.exists());

        // Ensure directory
        commands::ensure_commands_directory(&commands_dir)?;

        // Directory should exist
        anyhow::ensure!(commands_dir.exists());
        anyhow::ensure!(commands_dir.is_dir());

        Ok(())
    }

    #[test]
    fn test_sync_commands_overwrite_existing() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source = temp_dir.path().join("source");
        let target = temp_dir.path().join("target");

        // Create source and target directories
        fs::create_dir_all(&source)?;
        fs::create_dir_all(&target)?;

        // Create initial files
        fs::write(source.join("command.md"), "New content")?;
        fs::write(target.join("command"), "Old content")?;

        // Sync commands
        let synced = commands::sync_commands(&source, &target)?;

        // Check that file was overwritten
        anyhow::ensure!(synced.len() == 1, "Expected 1 synced command");
        let content = fs::read_to_string(target.join("command"))?;
        anyhow::ensure!(content == "New content", "Content should be overwritten");

        Ok(())
    }
}
