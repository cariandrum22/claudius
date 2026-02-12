use crate::fixtures::TestFixture;
use assert_cmd::Command;
use predicates::prelude::*;
use serial_test::serial;

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to save and restore environment variables
    struct EnvGuard {
        xdg_config_home: Option<String>,
        home: Option<String>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self {
                xdg_config_home: std::env::var("XDG_CONFIG_HOME").ok(),
                home: std::env::var("HOME").ok(),
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
        }
    }

    #[test]
    #[serial]
    fn test_skills_sync_project_local() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create test skills
        fixture.with_skill("hello", "# Hello Skill\nTest skill").unwrap();
        fixture.with_skill("debug", "# Debug Skill\nDebug info").unwrap();

        // Create minimal config
        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();

        // Run sync in project-local mode (default)
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["skills", "sync"])
            .assert()
            .success();

        // Verify skills were synced to project-local .claude/skills/
        assert!(fixture.project_file_exists(".claude/skills"));

        // Check skills exist with SKILL.md
        assert!(fixture.project_file_exists(".claude/skills/hello/SKILL.md"));
        assert!(fixture.project_file_exists(".claude/skills/debug/SKILL.md"));

        // Verify content
        let hello_content = fixture
            .read_project_file(".claude/skills/hello/SKILL.md")
            .unwrap();
        assert_eq!(hello_content, "# Hello Skill\nTest skill");

        let debug_content = fixture
            .read_project_file(".claude/skills/debug/SKILL.md")
            .unwrap();
        assert_eq!(debug_content, "# Debug Skill\nDebug info");
    }

    #[test]
    #[serial]
    fn test_skills_sync_gemini_project_local() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_skill("gemini-skill", "# Gemini Skill").unwrap();
        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["skills", "sync"])
            .args(["--agent", "gemini"])
            .assert()
            .success();

        assert!(fixture.project_file_exists(".gemini/skills"));
        assert!(fixture.project_file_exists(".gemini/skills/gemini-skill/SKILL.md"));
    }

    #[test]
    #[serial]
    fn test_skills_sync_global() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create test skills
        fixture.with_skill("test", "# Test Skill").unwrap();

        // Create minimal config
        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();

        // Run sync in global mode
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["skills", "sync"])
            .arg("--global")
            .assert()
            .success();

        // Verify skills were synced to ~/.claude/skills/
        assert!(fixture.home_file_exists(".claude/skills"));

        // Check skill exists
        assert!(fixture.home_file_exists(".claude/skills/test/SKILL.md"));

        let test_content = fixture.read_home_file(".claude/skills/test/SKILL.md").unwrap();
        assert_eq!(test_content, "# Test Skill");
    }

    #[test]
    #[serial]
    fn test_skills_only_mode_project_local() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create test skill
        fixture.with_skill("cmd", "Skill content").unwrap();

        // Create config files
        fixture
            .with_mcp_servers(r#"{"mcpServers": {"test": {"command": "test"}}}"#)
            .unwrap();
        fixture.with_settings(r#"{"apiKeyHelper": "test"}"#).unwrap();

        // Run sync using dedicated skills subcommand
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["skills", "sync"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Successfully synced"));

        // Verify only skills were synced (no .mcp.json or settings.json)
        assert!(!fixture.project_file_exists(".mcp.json"));
        assert!(!fixture.project_file_exists(".claude/settings.json"));

        // But skills should exist
        assert!(fixture.project_file_exists(".claude/skills/cmd/SKILL.md"));
    }
}
