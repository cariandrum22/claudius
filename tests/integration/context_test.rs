use crate::fixtures::TestFixture;
use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

use anyhow::Result;

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to save and restore environment variables
    struct EnvGuard {
        xdg_config_home: Option<String>,
        home: Option<String>,
        current_dir: Option<std::path::PathBuf>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self {
                xdg_config_home: std::env::var("XDG_CONFIG_HOME").ok(),
                home: std::env::var("HOME").ok(),
                current_dir: std::env::current_dir().ok(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // Restore XDG_CONFIG_HOME
            match &self.xdg_config_home {
                Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
            // Restore HOME
            match &self.home {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }
            // Restore current directory
            if let Some(dir) = &self.current_dir {
                let _ = std::env::set_current_dir(dir);
            }
        }
    }

    // ========== context append command tests ==========

    #[test]
    #[serial]
    fn test_append_context_default_claude() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let project_dir = temp_dir.path().join("project");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&project_dir)?;

        // Set XDG_CONFIG_HOME to our temp directory
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create a rule file
        let rules_dir = temp_dir.path().join("claudius").join("rules");
        fs::create_dir_all(&rules_dir)?;
        fs::write(rules_dir.join("test-rule.md"), "# Test Rule\n\nThis is a test rule.")?;

        // Change to project directory
        std::env::set_current_dir(&project_dir)?;

        // Run context append command
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .env("XDG_CONFIG_HOME", temp_dir.path())
            .args(["context", "append", "test-rule"])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            return Err(anyhow::anyhow!("context append command failed"));
        }

        // Verify CLAUDE.md was created
        let claude_md = project_dir.join("CLAUDE.md");
        anyhow::ensure!(claude_md.exists(), "CLAUDE.md should exist");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_append_context_with_agent_gemini() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let project_dir = temp_dir.path().join("project");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&project_dir)?;

        // Set XDG_CONFIG_HOME to our temp directory
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create a rule file
        let rules_dir = temp_dir.path().join("claudius").join("rules");
        fs::create_dir_all(&rules_dir)?;
        fs::write(rules_dir.join("gemini-rule.md"), "# Gemini Rule\n\nFor Gemini agent.")?;

        // Change to project directory
        std::env::set_current_dir(&project_dir)?;

        // Run context append command with Gemini agent
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .env("XDG_CONFIG_HOME", temp_dir.path())
            .args(["context", "append", "gemini-rule", "--agent", "gemini"])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            return Err(anyhow::anyhow!("context append command failed"));
        }

        // Verify AGENTS.md was created (not CLAUDE.md)
        let agents_md = project_dir.join("AGENTS.md");
        anyhow::ensure!(agents_md.exists(), "AGENTS.md should exist for Gemini agent");
        anyhow::ensure!(!project_dir.join("CLAUDE.md").exists(), "CLAUDE.md should not exist");

        let content = fs::read_to_string(&agents_md)?;
        anyhow::ensure!(content.contains("# Gemini Rule"), "Content should contain Gemini Rule");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_append_context_with_agent_codex() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let project_dir = temp_dir.path().join("project");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&project_dir)?;

        // Set XDG_CONFIG_HOME to our temp directory
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create a rule file
        let rules_dir = temp_dir.path().join("claudius").join("rules");
        fs::create_dir_all(&rules_dir)?;
        fs::write(rules_dir.join("codex-rule.md"), "# Codex Rule\n\nFor Codex agent.")?;

        // Change to project directory
        std::env::set_current_dir(&project_dir)?;

        // Run context append command with Codex agent
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .env("XDG_CONFIG_HOME", temp_dir.path())
            .args(["context", "append", "codex-rule", "--agent", "codex"])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            return Err(anyhow::anyhow!("context append command failed"));
        }

        // Verify AGENTS.md was created (not CLAUDE.md)
        let agents_md = project_dir.join("AGENTS.md");
        anyhow::ensure!(agents_md.exists(), "AGENTS.md should exist for Codex agent");
        anyhow::ensure!(!project_dir.join("CLAUDE.md").exists(), "CLAUDE.md should not exist");

        let content = fs::read_to_string(&agents_md)?;
        anyhow::ensure!(content.contains("# Codex Rule"), "Content should contain Codex Rule");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_append_context_global_flag() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let home_dir = temp_dir.path().join("home");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&home_dir)?;

        // Set environment variables
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        std::env::set_var("HOME", &home_dir);

        // Create a rule file
        let rules_dir = temp_dir.path().join("claudius").join("rules");
        fs::create_dir_all(&rules_dir)?;
        fs::write(rules_dir.join("global-rule.md"), "# Global Rule\n\nGlobal context.")?;

        // Run context append command with --global flag
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .env("XDG_CONFIG_HOME", temp_dir.path())
            .env("HOME", &home_dir)
            .args(["context", "append", "global-rule", "--global"])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            return Err(anyhow::anyhow!("context append command failed"));
        }

        // Verify CLAUDE.md was created in home directory
        let claude_md = home_dir.join("CLAUDE.md");
        anyhow::ensure!(claude_md.exists(), "CLAUDE.md should exist in home directory");

        let content = fs::read_to_string(&claude_md)?;
        anyhow::ensure!(content.contains("# Global Rule"), "Content should contain Global Rule");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_append_context_with_custom_context_file() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();

        // Create a config file that specifies a custom context file
        let config_content = r#"[default]
agent = "claude"
context-file = "CUSTOM.md"
"#;
        std::fs::write(fixture.config.join("config.toml"), config_content).unwrap();

        // Create a rule file
        fixture
            .with_rule("custom-rule", "# Custom Rule\n\nFor custom context file.")
            .unwrap();

        // Create a project directory
        let project_dir = assert_fs::TempDir::new().unwrap();

        // Set up environment
        fixture.setup_env();

        // Run context append command
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(project_dir.path())
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["context", "append"])
            .arg("custom-rule")
            .assert()
            .success();

        // Verify CUSTOM.md was created (not CLAUDE.md)
        project_dir.child("CUSTOM.md").assert(predicate::path::exists());
        project_dir.child("CLAUDE.md").assert(predicate::path::missing());

        // Verify content
        project_dir
            .child("CUSTOM.md")
            .assert(predicate::str::contains("# Custom Rule"))
            .assert(predicate::str::contains("For custom context file"));
    }

    #[test]
    #[serial]
    fn test_append_context_with_specific_path() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let project_dir = temp_dir.path().join("project");
        let subdir = project_dir.join("subdir");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&subdir)?;

        // Set XDG_CONFIG_HOME to our temp directory
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create a rule file
        let rules_dir = temp_dir.path().join("claudius").join("rules");
        fs::create_dir_all(&rules_dir)?;
        fs::write(rules_dir.join("path-rule.md"), "# Path Rule\n\nWith specific path.")?;

        // Run context append command with specific path
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .env("XDG_CONFIG_HOME", temp_dir.path())
            .args(["context", "append", "path-rule", "--path", subdir.to_str().unwrap()])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            return Err(anyhow::anyhow!("context append command failed"));
        }

        // Verify CLAUDE.md was created in subdir
        let claude_md = subdir.join("CLAUDE.md");
        anyhow::ensure!(claude_md.exists(), "CLAUDE.md should exist in subdir");

        let content = fs::read_to_string(&claude_md)?;
        anyhow::ensure!(content.contains("# Path Rule"), "Content should contain Path Rule");
        anyhow::ensure!(
            content.contains("With specific path."),
            "Content should contain specific path text"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_append_context_with_template_path() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let project_dir = temp_dir.path().join("project");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&project_dir)?;

        // Set XDG_CONFIG_HOME to our temp directory
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create a custom template file
        let template_path = temp_dir.path().join("custom-template.md");
        fs::write(&template_path, "# Custom Template\n\nThis is from a custom template file.")?;

        // Change to project directory
        std::env::set_current_dir(&project_dir)?;

        // Run context append command with --template-path
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .env("XDG_CONFIG_HOME", temp_dir.path())
            .args(["context", "append", "--template-path", template_path.to_str().unwrap()])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            return Err(anyhow::anyhow!("context append command failed"));
        }

        // Verify CLAUDE.md was created with template content
        let claude_md = project_dir.join("CLAUDE.md");
        anyhow::ensure!(claude_md.exists(), "CLAUDE.md should exist");

        let content = fs::read_to_string(&claude_md)?;
        anyhow::ensure!(
            content.contains("# Custom Template"),
            "Content should contain Custom Template"
        );
        anyhow::ensure!(
            content.contains("This is from a custom template file."),
            "Content should contain custom template text"
        );

        Ok(())
    }

    // ========== Project configuration tests ==========

    #[test]
    #[serial]
    fn test_project_local_default() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let servers_file = temp_dir.child("mcpServers.json");

        // Create mcpServers.json
        servers_file
            .write_str(
                r#"{
        "mcpServers": {
            "local-test": {
                "command": "test",
                "args": [],
                "env": {}
            }
        }
    }"#,
            )
            .unwrap();

        // Run in temp directory (should use project-local .mcp.json)
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .args(["config", "sync"])
            .arg("--config")
            .arg(servers_file.path())
            .arg("--dry-run")
            .assert()
            .success()
            .success();

        // The .mcp.json path should be in the current directory
        let local_mcp = temp_dir.path().join(".mcp.json");
        assert!(!local_mcp.exists(), "Dry-run shouldn't create the file");
    }

    #[test]
    #[serial]
    fn test_global_flag() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let servers_file = temp_dir.child("mcpServers.json");

        // Create mcpServers.json
        servers_file
            .write_str(
                r#"{
        "mcpServers": {
            "global-test": {
                "command": "test",
                "args": [],
                "env": {}
            }
        }
    }"#,
            )
            .unwrap();

        // Run with --global flag
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.args(["config", "sync"])
            .arg("--config")
            .arg(servers_file.path())
            .arg("--global")
            .arg("--dry-run")
            .assert()
            .success()
            .success();
    }

    #[test]
    #[serial]
    fn test_project_local_sync_claude_code() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create MCP servers config
        fixture
            .with_mcp_servers(r#"{"mcpServers": {"test": {"command": "test-cmd", "args": []}}}"#)
            .unwrap();

        // Create claude.settings.json
        fixture.with_claude_settings(r#"{"env": {"TEST_VAR": "test_value"}}"#).unwrap();

        // Use the fixture's project directory instead of creating a new one
        let project_dir = &fixture.project;

        // Run sync in project-local mode (default)
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .current_dir(project_dir)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .arg("--debug")
            .args(["config", "sync"])
            .arg("--agent")
            .arg("claude-code")
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("stdout: '{stdout}'");
        eprintln!("stderr: '{stderr}'");
        eprintln!("exit code: {}", output.status.code().unwrap_or(-1));

        // List files in project directory
        eprintln!("Files in project directory:");
        for entry in std::fs::read_dir(project_dir).unwrap() {
            let dir_entry = entry.unwrap();
            eprintln!("  {}", dir_entry.path().display());
        }

        assert!(output.status.success(), "sync command failed");

        // Verify .mcp.json was created
        let mcp_json = project_dir.join(".mcp.json");
        assert!(mcp_json.exists(), ".mcp.json should exist in project directory");

        // Verify .claude/settings.json was created
        let settings_json = project_dir.join(".claude").join("settings.json");
        assert!(settings_json.exists(), ".claude/settings.json should exist");

        // Verify global claude.json was NOT created
        let global_claude = fixture.home_dir().join(".claude.json");
        assert!(!global_claude.exists(), "Global claude.json should not exist");
    }

    #[test]
    #[serial]
    fn test_project_local_sync_claude_desktop() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create MCP servers config
        fixture
            .with_mcp_servers(r#"{"mcpServers": {"test": {"command": "test-cmd", "args": []}}}"#)
            .unwrap();

        // Create claude.settings.json (ignored by Claude Desktop)
        fixture.with_claude_settings(r#"{"env": {"TEST_VAR": "test_value"}}"#).unwrap();

        let project_dir = &fixture.project;

        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .current_dir(project_dir)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "sync"])
            .output()
            .unwrap();

        assert!(output.status.success(), "sync command failed");

        // Verify .mcp.json was created
        let mcp_json = project_dir.join(".mcp.json");
        assert!(mcp_json.exists(), ".mcp.json should exist in project directory");

        // Claude Desktop does not use project-local settings.json
        let settings_json = project_dir.join(".claude").join("settings.json");
        assert!(
            !settings_json.exists(),
            ".claude/settings.json should not exist for Claude Desktop"
        );

        // Verify global claude.json was NOT created
        let global_claude = fixture.home_dir().join(".claude.json");
        assert!(!global_claude.exists(), "Global claude.json should not exist");
    }

    // ========== Template and rules tests ==========

    #[test]
    #[serial]
    fn test_append_template_command() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create a rule file
        fixture.with_rule("basic", "# Basic Rule\n\nThis is a basic rule.").unwrap();

        // Create a temp directory for the target CLAUDE.md
        let target_dir = assert_fs::TempDir::new().unwrap();

        // Run with rule
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(target_dir.path())
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["context", "append"])
            .arg("basic")
            .assert()
            .success();

        // Verify CLAUDE.md was created with the rule
        let claude_md = target_dir.child("CLAUDE.md");
        claude_md.assert(predicate::path::exists());
        claude_md.assert(predicate::str::contains("# Basic Rule"));
        claude_md.assert(predicate::str::contains("This is a basic rule"));
    }

    #[test]
    #[serial]
    fn test_append_to_existing_claude_md() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create rule file
        fixture
            .with_rule("append", "# Append Rule\n\nThis should be appended.")
            .unwrap();

        // Create a target directory with existing CLAUDE.md
        let target_dir = assert_fs::TempDir::new().unwrap();
        let claude_md = target_dir.child("CLAUDE.md");
        claude_md.write_str("# Existing Content\n\nOriginal content here.").unwrap();

        // Run with rule
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(target_dir.path())
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["context", "append"])
            .arg("append")
            .assert()
            .success();

        // Verify both contents exist
        let content = fs::read_to_string(claude_md.path()).unwrap();
        assert!(content.contains("# Existing Content"), "Content should contain Existing Content");
        assert!(
            content.contains("Original content here"),
            "Content should contain original content"
        );
        assert!(content.contains("# Append Rule"), "Content should contain Append Rule");
        assert!(
            content.contains("This should be appended"),
            "Content should contain appended text"
        );
    }

    #[test]
    #[serial]
    fn test_append_custom_template() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create a custom template file outside of rules directory
        let custom_template = assert_fs::NamedTempFile::new("custom.md").unwrap();
        custom_template
            .write_str("# Custom Template\n\nThis is a custom template file.")
            .unwrap();

        // Create a target directory
        let target_dir = assert_fs::TempDir::new().unwrap();

        // Run with custom template
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(target_dir.path())
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["context", "append"])
            .arg("--template-path")
            .arg(custom_template.path())
            .assert()
            .success();

        // Verify CLAUDE.md was created with custom template
        let claude_md = target_dir.child("CLAUDE.md");
        claude_md.assert(predicate::path::exists());
        claude_md.assert(predicate::str::contains("# Custom Template"));
        claude_md.assert(predicate::str::contains("This is a custom template file"));
    }

    #[test]
    #[serial]
    fn test_rules_command() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create rule files
        fixture
            .with_rule("security", "# Security Rules\n\nAlways validate input.")
            .unwrap();
        fixture
            .with_rule("performance", "# Performance Rules\n\nOptimize database queries.")
            .unwrap();

        // Create another temp dir for the target CLAUDE.md
        let target_dir = assert_fs::TempDir::new().unwrap();

        // Run with rules
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(target_dir.path())
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["context", "append"])
            .arg("security")
            .assert()
            .success();

        // Verify CLAUDE.md was created with the rule
        let claude_md = target_dir.child("CLAUDE.md");
        claude_md.assert(predicate::path::exists());
        claude_md.assert(predicate::str::contains("# Security Rules"));
        claude_md.assert(predicate::str::contains("Always validate input"));
    }

    #[test]
    #[serial]
    fn test_rules_command_with_existing_claude_md() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create rule file
        fixture
            .with_rule("testing", "# Testing Rules\n\nWrite comprehensive tests.")
            .unwrap();

        // Create a target directory with existing CLAUDE.md
        let target_dir = assert_fs::TempDir::new().unwrap();
        let claude_md = target_dir.child("CLAUDE.md");
        claude_md.write_str("# Existing Project Rules\n\nOriginal content.").unwrap();

        // Run with rule
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(target_dir.path())
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["context", "append"])
            .arg("testing")
            .assert()
            .success();

        // Verify both contents exist
        let content = fs::read_to_string(claude_md.path()).unwrap();
        assert!(
            content.contains("# Existing Project Rules"),
            "Content should contain Existing Project Rules"
        );
        assert!(content.contains("Original content"), "Content should contain original content");
        assert!(content.contains("# Testing Rules"), "Content should contain Testing Rules");
        assert!(
            content.contains("Write comprehensive tests"),
            "Content should contain testing rules text"
        );
    }

    #[test]
    #[serial]
    fn test_context_list_outputs_rules() -> Result<()> {
        let _env_guard = EnvGuard::new();
        let temp_dir = TempDir::new()?;

        let rules_dir = temp_dir.path().join("claudius").join("rules");
        fs::create_dir_all(rules_dir.join("nested"))?;
        fs::write(rules_dir.join("alpha.md"), "# Alpha Rule\n\nDetails.")?;
        fs::write(rules_dir.join("nested").join("omega.md"), "# Omega Rule\n\nDetails.")?;

        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .env("XDG_CONFIG_HOME", temp_dir.path())
            .args(["context", "list"])
            .output()?;

        anyhow::ensure!(output.status.success(), "context list command failed");
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::ensure!(stdout.contains("alpha"), "List output should mention alpha rule");
        anyhow::ensure!(
            stdout.contains("nested/omega"),
            "List output should mention nested omega rule"
        );
        Ok(())
    }

    #[test]
    #[serial]
    fn test_context_list_tree_view() -> Result<()> {
        let _env_guard = EnvGuard::new();
        let temp_dir = TempDir::new()?;

        let rules_dir = temp_dir.path().join("claudius").join("rules");
        fs::create_dir_all(rules_dir.join("security").join("sub"))?;
        fs::write(rules_dir.join("security").join("base.md"), "# Base\n")?;
        fs::write(rules_dir.join("security").join("sub").join("extended.md"), "# Extended\n")?;

        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .env("XDG_CONFIG_HOME", temp_dir.path())
            .args(["context", "list", "--tree"])
            .output()?;

        anyhow::ensure!(output.status.success(), "context list --tree command failed");
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::ensure!(stdout.contains("security/"), "Tree output should show directory names");
        anyhow::ensure!(
            stdout.contains("└── base.md"),
            "Tree output should include leaf files with connectors"
        );
        Ok(())
    }
}
