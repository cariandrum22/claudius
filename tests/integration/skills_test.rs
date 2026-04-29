use crate::fixtures::TestFixture;
use assert_cmd::Command;
use predicates::prelude::*;
use serial_test::serial;
use std::fs;

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

    fn run_codex_skill_sync(mode: &str) -> TestFixture {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_skill("codex-test", "# Codex Skill").unwrap();
        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fs::write(
            fixture.config.join("config.toml"),
            format!("[codex]\nskill-target = \"{mode}\"\n"),
        )
        .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["skills", "sync", "--agent", "codex", "--enable-codex-skills"])
            .assert()
            .success();

        fixture
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
        let hello_content = fixture.read_project_file(".claude/skills/hello/SKILL.md").unwrap();
        assert_eq!(hello_content, "# Hello Skill\nTest skill");

        let debug_content = fixture.read_project_file(".claude/skills/debug/SKILL.md").unwrap();
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
    fn test_skills_sync_gemini_combines_shared_and_agent_specific_skills() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_skill("shared-skill", "# Shared Skill").unwrap();
        fixture.with_agent_skill("gemini", "gemini-skill", "# Gemini Skill").unwrap();
        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();

        let mut initial_sync = Command::new(env!("CARGO_BIN_EXE_claudius"));
        initial_sync
            .current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["skills", "sync", "--agent", "gemini"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Legacy commands directory detected").not());

        assert!(fixture.project_file_exists(".gemini/skills/shared-skill/SKILL.md"));
        assert!(fixture.project_file_exists(".gemini/skills/gemini-skill/SKILL.md"));

        fs::remove_dir_all(fixture.config.join("skills").join("gemini").join("gemini-skill"))
            .unwrap();

        let mut prune_sync = Command::new(env!("CARGO_BIN_EXE_claudius"));
        prune_sync
            .current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["skills", "sync", "--agent", "gemini", "--prune"])
            .assert()
            .success();

        assert!(fixture.project_file_exists(".gemini/skills/shared-skill/SKILL.md"));
        assert!(!fixture.project_file_exists(".gemini/skills/gemini-skill/SKILL.md"));
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
    fn test_skills_sync_codex_auto_targets_both_project_local() {
        let _env_guard = EnvGuard::new();
        let fixture = run_codex_skill_sync("auto");

        assert!(fixture.project_file_exists(".codex/skills/codex-test/SKILL.md"));
        assert!(fixture.project_file_exists(".agents/skills/codex-test/SKILL.md"));
    }

    #[test]
    #[serial]
    fn test_skills_sync_codex_mode_targets_only_codex_project_local() {
        let _env_guard = EnvGuard::new();
        let fixture = run_codex_skill_sync("codex");

        assert!(fixture.project_file_exists(".codex/skills/codex-test/SKILL.md"));
        assert!(!fixture.project_file_exists(".agents/skills/codex-test/SKILL.md"));
    }

    #[test]
    #[serial]
    fn test_skills_sync_codex_mode_targets_only_agents_project_local() {
        let _env_guard = EnvGuard::new();
        let fixture = run_codex_skill_sync("agents");

        assert!(!fixture.project_file_exists(".codex/skills/codex-test/SKILL.md"));
        assert!(fixture.project_file_exists(".agents/skills/codex-test/SKILL.md"));
    }

    #[test]
    #[serial]
    fn test_skills_sync_codex_mode_targets_both_project_local() {
        let _env_guard = EnvGuard::new();
        let fixture = run_codex_skill_sync("both");

        assert!(fixture.project_file_exists(".codex/skills/codex-test/SKILL.md"));
        assert!(fixture.project_file_exists(".agents/skills/codex-test/SKILL.md"));
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

    #[test]
    #[serial]
    fn test_skills_sync_prune_updates_and_removes_only_managed_files() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_skill("keep", "# Keep Skill\nVersion 1").unwrap();
        fixture.with_skill("remove", "# Remove Skill").unwrap();
        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();

        let mut initial_sync = Command::new(env!("CARGO_BIN_EXE_claudius"));
        initial_sync
            .current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["skills", "sync"])
            .assert()
            .success();

        fs::write(
            fixture.config.join("skills").join("keep").join("SKILL.md"),
            "# Keep Skill\nVersion 2",
        )
        .unwrap();
        fs::remove_dir_all(fixture.config.join("skills").join("remove")).unwrap();
        fs::create_dir_all(fixture.project.join(".claude").join("skills").join("manual")).unwrap();
        fs::write(
            fixture.project.join(".claude").join("skills").join("manual").join("notes.txt"),
            "manual",
        )
        .unwrap();

        let mut prune_sync = Command::new(env!("CARGO_BIN_EXE_claudius"));
        prune_sync
            .current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["skills", "sync", "--prune"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Successfully synced 1 skill(s):"))
            .stdout(predicate::str::contains("Pruned 1 stale skill file(s)"));

        assert_eq!(
            fixture.read_project_file(".claude/skills/keep/SKILL.md").unwrap(),
            "# Keep Skill\nVersion 2"
        );
        assert!(!fixture.project_file_exists(".claude/skills/remove/SKILL.md"));
        assert!(fixture.project_file_exists(".claude/skills/manual/notes.txt"));
    }
}
