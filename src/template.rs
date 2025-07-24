use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::config::Config;

const DEFAULT_TEMPLATE: &str = r"
## MCP Servers Configuration

This project uses the following MCP servers:

### Available Servers

Check `~/.config/claudius/mcpServers.json` for the complete list of configured MCP servers.

### Sync Configuration

To sync MCP server configuration to this project:

```bash
claudius
```

To sync to global configuration:

```bash
claudius --global
```

### Custom Configuration

You can override the default MCP servers by creating a local `mcpServers.json` file:

```bash
claudius --config ./mcpServers.json
```
";

/// Appends a template to the CLAUDE.md file in the specified target directory.
///
/// # Errors
///
/// Returns an error if:
/// - Unable to determine current directory
/// - Unable to read template file
/// - Unable to read or write CLAUDE.md file
pub fn append_template_to_claude_md(
    template_path: Option<&Path>,
    target_dir: Option<&Path>,
) -> Result<()> {
    let base_dir = match target_dir {
        Some(dir) => dir.to_path_buf(),
        None => std::env::current_dir()?,
    };
    let claude_md_path = base_dir.join("CLAUDE.md");

    // Read template content
    let template_content = if let Some(path) = template_path {
        fs::read_to_string(path)
            .with_context(|| format!("Failed to read template file: {}", path.display()))?
    } else {
        DEFAULT_TEMPLATE.to_string()
    };

    // Check if CLAUDE.md exists
    if claude_md_path.exists() {
        // Read existing content
        let existing_content = fs::read_to_string(&claude_md_path)
            .with_context(|| format!("Failed to read CLAUDE.md: {}", claude_md_path.display()))?;

        // Check if template is already present (avoid duplicates)
        if existing_content.contains("## MCP Servers Configuration") {
            return Ok(());
        }

        // Append template
        let new_content = format!("{}\n{}", existing_content.trim_end(), template_content);
        fs::write(&claude_md_path, new_content)
            .with_context(|| format!("Failed to update CLAUDE.md: {}", claude_md_path.display()))?;
    } else {
        // Create new CLAUDE.md with template
        fs::write(&claude_md_path, &template_content)
            .with_context(|| format!("Failed to create CLAUDE.md: {}", claude_md_path.display()))?;
    }

    Ok(())
}

/// Appends rules to the CLAUDE.md file in the specified target directory.
///
/// # Errors
///
/// Returns an error if:
/// - Unable to determine current directory
/// - Unable to read rule files
/// - Unable to write to CLAUDE.md file
/// - No valid rules found
pub fn append_rules_to_claude_md(rule_names: &[String], target_dir: Option<&Path>) -> Result<()> {
    let base_dir = match target_dir {
        Some(dir) => dir.to_path_buf(),
        None => std::env::current_dir()?,
    };
    let claude_md_path = base_dir.join("CLAUDE.md");

    let (combined_content, found_rules) = collect_rule_contents(rule_names)?;

    if combined_content.is_empty() {
        return Err(anyhow::anyhow!("No valid rules found"));
    }

    write_rules_to_file(&claude_md_path, &combined_content, &found_rules)
}

/// Collect contents from rule files
fn collect_rule_contents(rule_names: &[String]) -> Result<(String, Vec<String>)> {
    let rules_dir = Config::get_config_dir()?.join("rules");
    let mut combined_content = String::new();
    let mut found_rules = Vec::new();

    for rule_name in rule_names {
        if let Some(content) = read_rule_file(&rules_dir, rule_name)? {
            if !combined_content.is_empty() {
                combined_content.push_str("\n\n");
            }
            combined_content.push_str(&content);
            found_rules.push(rule_name.clone());
        }
    }

    Ok((combined_content, found_rules))
}

/// Read a single rule file if it exists
fn read_rule_file(rules_dir: &Path, rule_name: &str) -> Result<Option<String>> {
    let rule_file = rules_dir.join(format!("{rule_name}.md"));

    if rule_file.exists() {
        let content = fs::read_to_string(&rule_file)
            .with_context(|| format!("Failed to read rule file: {}", rule_file.display()))?;
        Ok(Some(content))
    } else {
        warn!("Rule '{}' not found at {}", rule_name, rule_file.display());
        Ok(None)
    }
}

/// Write rules to the target file
fn write_rules_to_file(file_path: &Path, content: &str, _found_rules: &[String]) -> Result<()> {
    if file_path.exists() {
        let existing_content = fs::read_to_string(file_path).with_context(|| {
            format!("Failed to read {}: {}", file_path.display(), file_path.display())
        })?;

        let new_content = format!("{}\n\n{}", existing_content.trim_end(), content);
        fs::write(file_path, new_content).with_context(|| {
            format!("Failed to update {}: {}", file_path.display(), file_path.display())
        })?;
    } else {
        fs::write(file_path, content).with_context(|| {
            format!("Failed to create {}: {}", file_path.display(), file_path.display())
        })?;
    }

    Ok(())
}

/// Ensures the rules directory exists and returns its path.
///
/// # Errors
///
/// Returns an error if:
/// - Unable to determine user config directory
/// - Unable to create rules directory
pub fn ensure_rules_directory() -> Result<PathBuf> {
    let rules_dir = Config::get_config_dir()?.join("rules");
    fs::create_dir_all(&rules_dir)
        .with_context(|| format!("Failed to create rules directory: {}", rules_dir.display()))?;
    Ok(rules_dir)
}

/// Collect all rule names from the rules directory
///
/// # Errors
///
/// Returns an error if:
/// - Unable to determine user config directory
/// - Unable to read rules directory
/// - Unable to read directory entries
pub fn collect_all_rule_names() -> Result<Vec<String>> {
    let rules_dir = Config::get_config_dir()?.join("rules");
    let mut rule_names = Vec::new();

    if rules_dir.exists() {
        let entries = fs::read_dir(&rules_dir)
            .with_context(|| format!("Failed to read rules directory: {}", rules_dir.display()))?;

        for entry in entries {
            let dir_entry = entry.context("Failed to read directory entry")?;
            let path = dir_entry.path();

            if !path.is_file() {
                continue;
            }

            if path.extension().is_some_and(|ext| ext == "md") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    rule_names.push(name.to_string());
                }
            }
        }
    }

    rule_names.sort();
    Ok(rule_names)
}

/// Appends a template to a context file (CLAUDE.md or AGENTS.md).
///
/// # Errors
///
/// Returns an error if:
/// - Unable to read template file
/// - Unable to create parent directory
/// - Unable to read or write context file
pub fn append_template_to_context_file(
    template_path: Option<&Path>,
    context_file_path: &Path,
) -> Result<()> {
    let template_content = read_template_content(template_path)?;
    ensure_parent_directory(context_file_path)?;

    if context_file_path.exists() {
        append_to_existing_file(context_file_path, &template_content, template_path.is_none())
    } else {
        create_new_file_with_content(context_file_path, &template_content)
    }
}

/// Read template content from file or use default
fn read_template_content(template_path: Option<&Path>) -> Result<String> {
    template_path.map_or_else(
        || Ok(DEFAULT_TEMPLATE.to_string()),
        |path| {
            fs::read_to_string(path)
                .with_context(|| format!("Failed to read template file: {}", path.display()))
        },
    )
}

/// Ensure parent directory exists for the target file
fn ensure_parent_directory(file_path: &Path) -> Result<()> {
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent directory: {}", parent.display()))?;
    }
    Ok(())
}

/// Append template to existing file with duplicate check
fn append_to_existing_file(
    file_path: &Path,
    template_content: &str,
    is_default: bool,
) -> Result<()> {
    let existing_content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read context file: {}", file_path.display()))?;

    // Check for duplicates only for default template
    if is_default && existing_content.contains("## MCP Servers Configuration") {
        info!(
            "MCP Servers Configuration section already exists in {}",
            get_filename_display(file_path)
        );
        return Ok(());
    }

    let new_content = format!("{}\n{}", existing_content.trim_end(), template_content);
    fs::write(file_path, new_content)
        .with_context(|| format!("Failed to update context file: {}", file_path.display()))?;

    Ok(())
}

/// Create new file with template content
fn create_new_file_with_content(file_path: &Path, content: &str) -> Result<()> {
    fs::write(file_path, content)
        .with_context(|| format!("Failed to create context file: {}", file_path.display()))?;

    Ok(())
}

/// Get filename for display
fn get_filename_display(path: &Path) -> &str {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("context file")
}

/// Appends rules to a context file (CLAUDE.md or AGENTS.md).
///
/// # Errors
///
/// Returns an error if:
/// - Unable to read rule files
/// - Unable to create parent directory
/// - Unable to write to context file
/// - No valid rules found
pub fn append_rules_to_context_file(rule_names: &[String], context_file_path: &Path) -> Result<()> {
    let (combined_content, found_rules) = collect_rule_contents(rule_names)?;

    if combined_content.is_empty() {
        return Err(anyhow::anyhow!("No valid rules found"));
    }

    ensure_parent_directory(context_file_path)?;
    write_rules_to_file(context_file_path, &combined_content, &found_rules)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::TempDir;

    fn setup_test_rules_dir(temp_dir: &Path) -> PathBuf {
        let rules_dir = temp_dir.join("claudius").join("rules");
        fs::create_dir_all(&rules_dir).expect("Failed to create rules directory");

        // Create test rule files
        fs::write(rules_dir.join("security.md"), "# Security Rules\nAlways validate input")
            .expect("Failed to write security.md");
        fs::write(rules_dir.join("testing.md"), "# Testing Rules\nWrite tests first")
            .expect("Failed to write testing.md");
        fs::write(rules_dir.join("docs.md"), "# Documentation\nDocument everything")
            .expect("Failed to write docs.md");

        rules_dir
    }

    #[test]
    fn test_append_template_to_claude_md_new_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let result = append_template_to_claude_md(None, Some(temp_dir.path()));
        assert!(result.is_ok());

        let claude_md_path = temp_dir.path().join("CLAUDE.md");
        assert!(claude_md_path.exists());

        let content = fs::read_to_string(&claude_md_path).expect("Failed to read CLAUDE.md");
        assert!(content.contains("## MCP Servers Configuration"));
        assert!(content.contains("claudius"));
    }

    #[test]
    fn test_append_template_to_claude_md_existing_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let claude_md_path = temp_dir.path().join("CLAUDE.md");

        // Create existing file
        fs::write(&claude_md_path, "# Existing Content\n\nSome text")
            .expect("Failed to write existing content");

        let result = append_template_to_claude_md(None, Some(temp_dir.path()));
        assert!(result.is_ok());

        let content = fs::read_to_string(&claude_md_path).expect("Failed to read CLAUDE.md");
        assert!(content.contains("# Existing Content"));
        assert!(content.contains("## MCP Servers Configuration"));
    }

    #[test]
    fn test_append_template_to_claude_md_already_exists() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let claude_md_path = temp_dir.path().join("CLAUDE.md");

        // Create file with existing MCP section
        fs::write(&claude_md_path, "# Content\n\n## MCP Servers Configuration\n\nExisting")
            .expect("Failed to write file with existing MCP section");

        let result = append_template_to_claude_md(None, Some(temp_dir.path()));
        assert!(result.is_ok());

        // Content should not be duplicated
        let content = fs::read_to_string(&claude_md_path).expect("Failed to read CLAUDE.md");
        let mcp_count = content.matches("## MCP Servers Configuration").count();
        assert_eq!(mcp_count, 1);
    }

    #[test]
    fn test_append_template_to_claude_md_custom_template() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let template_path = temp_dir.path().join("custom.md");

        fs::write(&template_path, "# Custom Template\n\nCustom content")
            .expect("Failed to write custom template");

        let result = append_template_to_claude_md(Some(&template_path), Some(temp_dir.path()));
        assert!(result.is_ok());

        let claude_md_path = temp_dir.path().join("CLAUDE.md");
        let content = fs::read_to_string(&claude_md_path).expect("Failed to read CLAUDE.md");
        assert!(content.contains("# Custom Template"));
        assert!(content.contains("Custom content"));
    }

    #[test]
    #[serial]
    fn test_append_rules_to_claude_md_new_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        setup_test_rules_dir(temp_dir.path());

        let project_dir = temp_dir.path().join("project");
        fs::create_dir_all(&project_dir).expect("Failed to create project directory");

        let rules = vec!["security".to_string(), "testing".to_string()];
        let result = append_rules_to_claude_md(&rules, Some(&project_dir));
        assert!(result.is_ok());

        let claude_md_path = project_dir.join("CLAUDE.md");
        assert!(claude_md_path.exists());

        let content = fs::read_to_string(&claude_md_path).expect("Failed to read CLAUDE.md");
        assert!(content.contains("# Security Rules"));
        assert!(content.contains("# Testing Rules"));
        assert!(!content.contains("# Documentation"));

        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    #[serial]
    fn test_append_rules_to_claude_md_existing_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        let rules_dir = setup_test_rules_dir(temp_dir.path());

        // Verify rules were created
        assert!(rules_dir.join("docs.md").exists());

        let project_dir = temp_dir.path().join("project");
        fs::create_dir_all(&project_dir).expect("Failed to create project directory");

        let claude_md_path = project_dir.join("CLAUDE.md");
        fs::write(&claude_md_path, "# Existing Content\n")
            .expect("Failed to write existing content");

        let rules = vec!["docs".to_string()];
        let result = append_rules_to_claude_md(&rules, Some(&project_dir));

        // Since we're testing with a rule that exists in setup, it should succeed
        assert!(result.is_ok());

        let content = fs::read_to_string(&claude_md_path).expect("Failed to read CLAUDE.md");
        assert!(content.contains("# Existing Content"));
        assert!(content.contains("# Documentation"));

        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    #[serial]
    fn test_append_rules_to_claude_md_missing_rule() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        setup_test_rules_dir(temp_dir.path());

        let project_dir = temp_dir.path().join("project");
        fs::create_dir_all(&project_dir).expect("Failed to create project directory");

        let rules = vec!["nonexistent".to_string()];
        let result = append_rules_to_claude_md(&rules, Some(&project_dir));
        assert!(result.is_err());

        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    #[serial]
    fn test_append_rules_to_claude_md_mixed_rules() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        setup_test_rules_dir(temp_dir.path());

        let project_dir = temp_dir.path().join("project");
        fs::create_dir_all(&project_dir).expect("Failed to create project directory");

        let rules = vec!["security".to_string(), "nonexistent".to_string(), "testing".to_string()];
        let result = append_rules_to_claude_md(&rules, Some(&project_dir));
        assert!(result.is_ok());

        let claude_md_path = project_dir.join("CLAUDE.md");
        let content = fs::read_to_string(&claude_md_path).expect("Failed to read CLAUDE.md");
        assert!(content.contains("# Security Rules"));
        assert!(content.contains("# Testing Rules"));

        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    #[serial]
    fn test_ensure_rules_directory_new() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        let result = ensure_rules_directory();
        assert!(result.is_ok());

        let rules_dir = result.expect("ensure_rules_directory should succeed");
        assert!(rules_dir.exists());
        assert!(rules_dir.is_dir());
        assert!(rules_dir.ends_with("rules"));

        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    #[serial]
    fn test_ensure_rules_directory_existing() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create directory first
        let expected_dir = temp_dir.path().join("claudius").join("rules");
        fs::create_dir_all(&expected_dir).expect("Failed to create expected directory");

        let result = ensure_rules_directory();
        assert!(result.is_ok());

        let rules_dir = result.expect("ensure_rules_directory should succeed");
        assert_eq!(rules_dir, expected_dir);

        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    fn test_append_template_to_context_file_new() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let context_file = temp_dir.path().join("GEMINI.md");

        let result = append_template_to_context_file(None, &context_file);
        assert!(result.is_ok());

        assert!(context_file.exists());
        let content = fs::read_to_string(&context_file).expect("Failed to read context file");
        assert!(content.contains("## MCP Servers Configuration"));
    }

    #[test]
    fn test_append_template_to_context_file_nested() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let context_file = temp_dir.path().join("nested/dir/AGENTS.md");

        let result = append_template_to_context_file(None, &context_file);
        assert!(result.is_ok());

        assert!(context_file.exists());
        assert!(context_file.parent().expect("Context file should have parent").exists());
    }

    #[test]
    fn test_append_template_to_context_file_custom() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let template_path = temp_dir.path().join("custom.md");
        let context_file = temp_dir.path().join("CONTEXT.md");

        fs::write(&template_path, "# Custom Content").expect("Failed to write custom content");

        let result = append_template_to_context_file(Some(&template_path), &context_file);
        assert!(result.is_ok());

        let content = fs::read_to_string(&context_file).expect("Failed to read context file");
        assert_eq!(content, "# Custom Content");
    }

    #[test]
    fn test_append_template_to_context_file_duplicate_check() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let context_file = temp_dir.path().join("CONTEXT.md");

        // First append
        append_template_to_context_file(None, &context_file)
            .expect("append_template_to_context_file should succeed");

        // Second append should detect duplicate
        let result = append_template_to_context_file(None, &context_file);
        assert!(result.is_ok());

        // Verify no duplicate
        let content = fs::read_to_string(&context_file).expect("Failed to read context file");
        let mcp_count = content.matches("## MCP Servers Configuration").count();
        assert_eq!(mcp_count, 1);
    }

    #[test]
    #[serial]
    fn test_append_rules_to_context_file_new() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        setup_test_rules_dir(temp_dir.path());

        let context_file = temp_dir.path().join("project/AGENTS.md");
        let rules = vec!["security".to_string(), "docs".to_string()];

        let result = append_rules_to_context_file(&rules, &context_file);
        assert!(result.is_ok());

        assert!(context_file.exists());
        let content = fs::read_to_string(&context_file).expect("Failed to read context file");
        assert!(content.contains("# Security Rules"));
        assert!(content.contains("# Documentation"));

        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    #[serial]
    fn test_append_rules_to_context_file_existing() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        setup_test_rules_dir(temp_dir.path());

        let context_file = temp_dir.path().join("CONTEXT.md");
        fs::write(&context_file, "# Existing\n").expect("Failed to write existing content");

        let rules = vec!["testing".to_string()];
        let result = append_rules_to_context_file(&rules, &context_file);
        assert!(result.is_ok());

        let content = fs::read_to_string(&context_file).expect("Failed to read context file");
        assert!(content.contains("# Existing"));
        assert!(content.contains("# Testing Rules"));

        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    #[serial]
    fn test_append_rules_to_context_file_no_valid_rules() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        setup_test_rules_dir(temp_dir.path());

        let context_file = temp_dir.path().join("CONTEXT.md");
        let rules = vec!["invalid1".to_string(), "invalid2".to_string()];

        let result = append_rules_to_context_file(&rules, &context_file);
        assert!(result.is_err());
        assert!(!context_file.exists());

        std::env::remove_var("XDG_CONFIG_HOME");
    }
}
