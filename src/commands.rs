use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Sync custom commands from source to target directory
///
/// # Errors
///
/// Returns an error if:
/// - Unable to create the target directory
/// - Unable to read the source directory
/// - Unable to copy command files
pub fn sync_commands(source_dir: &Path, target_dir: &Path) -> Result<Vec<String>> {
    // Ensure target directory exists
    fs::create_dir_all(target_dir)
        .with_context(|| format!("Failed to create target directory: {}", target_dir.display()))?;

    let mut synced_commands = Vec::new();

    // Check if source directory exists
    if !source_dir.exists() {
        return Ok(synced_commands);
    }

    // Read all markdown files from source directory
    let entries = fs::read_dir(source_dir)
        .with_context(|| format!("Failed to read commands directory: {}", source_dir.display()))?;

    for entry in entries {
        let dir_entry = entry?;
        let path = dir_entry.path();

        // Only process .md files
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
            let _file_name =
                path.file_name().ok_or_else(|| anyhow::anyhow!("Invalid file name"))?;

            // Get command name without extension
            let command_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid file stem"))?;

            // Target path without .md extension
            let target_path = target_dir.join(command_name);

            // Copy file without extension
            fs::copy(&path, &target_path).with_context(|| {
                format!(
                    "Failed to copy command from {} to {}",
                    path.display(),
                    target_path.display()
                )
            })?;

            synced_commands.push(command_name.to_string());
        }
    }

    Ok(synced_commands)
}

/// List all custom commands in a directory
///
/// # Errors
///
/// Returns an error if:
/// - Unable to read the commands directory
/// - Unable to access directory entries
pub fn list_commands(commands_dir: &Path) -> Result<Vec<String>> {
    let mut commands = Vec::new();

    if !commands_dir.exists() {
        return Ok(commands);
    }

    let entries = fs::read_dir(commands_dir).with_context(|| {
        format!("Failed to read commands directory: {}", commands_dir.display())
    })?;

    for entry in entries {
        let dir_entry = entry?;
        let path = dir_entry.path();

        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
            if let Some(command_name) = path.file_stem().and_then(|s| s.to_str()) {
                commands.push(command_name.to_string());
            }
        }
    }

    commands.sort();
    Ok(commands)
}

/// Ensure commands directory exists
///
/// # Errors
///
/// Returns an error if unable to create the commands directory
pub fn ensure_commands_directory(commands_dir: &Path) -> Result<()> {
    fs::create_dir_all(commands_dir).with_context(|| {
        format!("Failed to create commands directory: {}", commands_dir.display())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sync_commands_empty_source() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");

        // Source doesn't exist
        let result = sync_commands(&source_dir, &target_dir).expect("sync_commands should succeed");
        assert!(result.is_empty());
        assert!(target_dir.exists()); // Target should be created
    }

    #[test]
    fn test_sync_commands_with_md_files() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");

        // Create source directory and files
        fs::create_dir_all(&source_dir).expect("Failed to create source directory");
        fs::write(source_dir.join("command1.md"), "# Command 1")
            .expect("Failed to write command1.md");
        fs::write(source_dir.join("command2.md"), "# Command 2")
            .expect("Failed to write command2.md");
        fs::write(source_dir.join("not-a-command.txt"), "Text file")
            .expect("Failed to write not-a-command.txt");

        let result = sync_commands(&source_dir, &target_dir).expect("sync_commands should succeed");

        // Should sync only .md files
        assert_eq!(result.len(), 2);
        assert!(result.contains(&"command1".to_string()));
        assert!(result.contains(&"command2".to_string()));

        // Check target files exist without .md extension
        assert!(target_dir.join("command1").exists());
        assert!(target_dir.join("command2").exists());
        assert!(!target_dir.join("not-a-command.txt").exists());
        assert!(!target_dir.join("command1.md").exists()); // Should not have .md extension

        // Verify content
        let content1 =
            fs::read_to_string(target_dir.join("command1")).expect("Failed to read command1");
        assert_eq!(content1, "# Command 1");
    }

    #[test]
    fn test_sync_commands_nested_target() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("nested/deep/target");

        // Create source directory and file
        fs::create_dir_all(&source_dir).expect("Failed to create source directory");
        fs::write(source_dir.join("test.md"), "Test content").expect("Failed to write test.md");

        let result = sync_commands(&source_dir, &target_dir).expect("sync_commands should succeed");

        assert_eq!(result.len(), 1);
        assert_eq!(result.first(), Some(&"test".to_string()));
        assert!(target_dir.exists());
        assert!(target_dir.join("test").exists());
    }

    #[test]
    fn test_sync_commands_overwrite_existing() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");

        // Create directories
        fs::create_dir_all(&source_dir).expect("Failed to create source directory");
        fs::create_dir_all(&target_dir).expect("Failed to create target directory");

        // Create existing file in target
        fs::write(target_dir.join("command"), "Old content").expect("Failed to write old command");

        // Create source file
        fs::write(source_dir.join("command.md"), "New content")
            .expect("Failed to write new command.md");

        let result = sync_commands(&source_dir, &target_dir).expect("sync_commands should succeed");

        assert_eq!(result.len(), 1);

        // Verify content was overwritten
        let content =
            fs::read_to_string(target_dir.join("command")).expect("Failed to read command");
        assert_eq!(content, "New content");
    }

    #[test]
    fn test_list_commands_empty_directory() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let commands_dir = temp_dir.path().join("commands");

        // Directory doesn't exist
        let result = list_commands(&commands_dir).expect("list_commands should succeed");
        assert!(result.is_empty());

        // Create empty directory
        fs::create_dir_all(&commands_dir).expect("Failed to create commands directory");
        let result_after_create =
            list_commands(&commands_dir).expect("list_commands should succeed after create");
        assert!(result_after_create.is_empty());
    }

    #[test]
    fn test_list_commands_with_files() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let commands_dir = temp_dir.path().join("commands");

        fs::create_dir_all(&commands_dir).expect("Failed to create commands directory");
        fs::write(commands_dir.join("zebra.md"), "").expect("Failed to write zebra.md");
        fs::write(commands_dir.join("apple.md"), "").expect("Failed to write apple.md");
        fs::write(commands_dir.join("banana.md"), "").expect("Failed to write banana.md");
        fs::write(commands_dir.join("not-md.txt"), "").expect("Failed to write not-md.txt");

        let result = list_commands(&commands_dir).expect("list_commands should succeed");

        // Should list only .md files without extension, sorted
        assert_eq!(result.len(), 3);
        assert_eq!(result.first(), Some(&"apple".to_string()));
        assert_eq!(result.get(1), Some(&"banana".to_string()));
        assert_eq!(result.get(2), Some(&"zebra".to_string()));
    }

    #[test]
    fn test_list_commands_subdirectories() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let commands_dir = temp_dir.path().join("commands");

        fs::create_dir_all(&commands_dir).expect("Failed to create commands directory");
        fs::create_dir_all(commands_dir.join("subdir")).expect("Failed to create subdir");
        fs::write(commands_dir.join("command.md"), "").expect("Failed to write command.md");
        fs::write(commands_dir.join("subdir/nested.md"), "").expect("Failed to write nested.md");

        let result = list_commands(&commands_dir).expect("list_commands should succeed");

        // Should only list files in the root directory
        assert_eq!(result.len(), 1);
        assert_eq!(result.first(), Some(&"command".to_string()));
    }

    #[test]
    fn test_ensure_commands_directory_new() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let commands_dir = temp_dir.path().join("new/commands/dir");

        assert!(!commands_dir.exists());

        ensure_commands_directory(&commands_dir).expect("ensure_commands_directory should succeed");

        assert!(commands_dir.exists());
        assert!(commands_dir.is_dir());
    }

    #[test]
    fn test_ensure_commands_directory_existing() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let commands_dir = temp_dir.path().join("commands");

        // Create directory first
        fs::create_dir_all(&commands_dir).expect("Failed to create commands directory");

        // Should not fail if directory already exists
        ensure_commands_directory(&commands_dir).expect("ensure_commands_directory should succeed");

        assert!(commands_dir.exists());
    }

    #[test]
    fn test_ensure_commands_directory_file_exists() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let commands_path = temp_dir.path().join("commands");

        // Create a file at the path
        fs::write(&commands_path, "content").expect("Failed to write commands file");

        // Should fail if a file exists at the path
        let result = ensure_commands_directory(&commands_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_commands_special_characters() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&source_dir).expect("Failed to create source directory");
        fs::write(source_dir.join("test-command.md"), "Content 1")
            .expect("Failed to write test-command.md");
        fs::write(source_dir.join("test_command.md"), "Content 2")
            .expect("Failed to write test_command.md");
        fs::write(source_dir.join("test.command.md"), "Content 3")
            .expect("Failed to write test.command.md");

        let result = sync_commands(&source_dir, &target_dir).expect("sync_commands should succeed");

        assert_eq!(result.len(), 3);
        assert!(result.contains(&"test-command".to_string()));
        assert!(result.contains(&"test_command".to_string()));
        assert!(result.contains(&"test.command".to_string()));

        // Verify files exist with proper names
        assert!(target_dir.join("test-command").exists());
        assert!(target_dir.join("test_command").exists());
        assert!(target_dir.join("test.command").exists());
    }
}
