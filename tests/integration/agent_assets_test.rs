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
            match &self.xdg_config_home {
                Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
            match &self.home {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }
        }
    }

    #[test]
    #[serial]
    fn test_config_sync_gemini_syncs_custom_commands() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fixture
            .with_gemini_settings(r#"{"general":{"preferredEditor":"code"}}"#)
            .unwrap();
        fixture
            .with_gemini_command(
                "review",
                "description = \"Review the current diff\"\nprompt = \"Review this change set.\"",
            )
            .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync", "--agent", "gemini"])
            .assert()
            .success();

        assert!(fixture.project_file_exists(".gemini/commands/review.toml"));
        let command = fixture
            .read_project_file(".gemini/commands/review.toml")
            .expect("Gemini command should be readable");
        assert!(command.contains("Review the current diff"));
    }

    #[test]
    #[serial]
    fn test_config_sync_gemini_syncs_custom_agents() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fixture
            .with_gemini_settings(r#"{"general":{"preferredEditor":"code"}}"#)
            .unwrap();
        fixture
            .with_gemini_agent(
                "reviewer",
                "---\nname: reviewer\ndescription: Review Gemini workflows\n---\nFocus on Gemini-specific regressions.",
            )
            .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync", "--agent", "gemini"])
            .assert()
            .success();

        assert!(fixture.project_file_exists(".gemini/agents/reviewer.md"));
        let agent = fixture
            .read_project_file(".gemini/agents/reviewer.md")
            .expect("Gemini agent should be readable");
        assert!(agent.contains("Focus on Gemini-specific regressions."));
    }

    #[test]
    #[serial]
    fn test_config_sync_gemini_syncs_shared_and_agent_specific_skills() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fixture
            .with_gemini_settings(r#"{"general":{"preferredEditor":"code"}}"#)
            .unwrap();
        fixture.with_skill("shared-skill", "# Shared Skill").unwrap();
        fixture.with_agent_skill("gemini", "gemini-skill", "# Gemini Skill").unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync", "--agent", "gemini"])
            .assert()
            .success();

        assert!(fixture.project_file_exists(".gemini/skills/shared-skill/SKILL.md"));
        assert!(fixture.project_file_exists(".gemini/skills/gemini-skill/SKILL.md"));
    }

    #[test]
    #[serial]
    fn test_config_sync_gemini_warns_on_deprecated_agent_skill_overrides() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fixture
            .with_gemini_settings(r#"{"general":{"preferredEditor":"code"}}"#)
            .unwrap();
        fixture.with_agent_skill("gemini", "gemini-skill", "# Gemini Skill").unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync", "--agent", "gemini"])
            .assert()
            .success()
            .stderr(predicate::str::contains(
                "Deprecated full agent override directory detected for skill `gemini-skill` under skills/gemini/gemini-skill; prefer canonical target overlays in skill.yaml.",
            ));

        assert!(fixture.project_file_exists(".gemini/skills/gemini-skill/SKILL.md"));
    }

    #[test]
    #[serial]
    fn test_config_sync_claude_code_syncs_subagents() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fixture
            .with_claude_settings(r#"{"permissions":{"allow":[],"deny":[]}}"#)
            .unwrap();
        fixture
            .with_claude_code_agent(
                "reviewer",
                "---\nname: reviewer\ndescription: Review code changes\n---\nFocus on regressions.",
            )
            .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync", "--agent", "claude-code"])
            .assert()
            .success();

        assert!(fixture.project_file_exists(".claude/agents/reviewer.md"));
        let agent = fixture
            .read_project_file(".claude/agents/reviewer.md")
            .expect("Claude Code subagent should be readable");
        assert!(agent.contains("Focus on regressions."));
    }

    #[test]
    #[serial]
    fn test_config_sync_gemini_dry_run_prune_reports_stale_commands() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fixture
            .with_gemini_settings(r#"{"general":{"preferredEditor":"code"}}"#)
            .unwrap();
        fixture
            .with_gemini_command(
                "review",
                "description = \"Review the current diff\"\nprompt = \"Review this change set.\"",
            )
            .unwrap();

        let mut initial_sync = Command::new(env!("CARGO_BIN_EXE_claudius"));
        initial_sync
            .current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync", "--agent", "gemini"])
            .assert()
            .success();

        fs::remove_file(fixture.config.join("commands").join("gemini").join("review.toml"))
            .unwrap();

        let mut dry_run = Command::new(env!("CARGO_BIN_EXE_claudius"));
        dry_run
            .current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync", "--agent", "gemini", "--dry-run", "--prune"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Would prune 1 stale file(s):"))
            .stdout(predicate::str::contains("review.toml"));

        assert!(fixture.project_file_exists(".gemini/commands/review.toml"));
    }

    #[test]
    #[serial]
    fn test_config_sync_claude_code_prune_removes_only_managed_subagents() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fixture
            .with_claude_settings(r#"{"permissions":{"allow":[],"deny":[]}}"#)
            .unwrap();
        fixture
            .with_claude_code_agent(
                "reviewer",
                "---\nname: reviewer\ndescription: Review code changes\n---\nFocus on regressions.",
            )
            .unwrap();

        let mut initial_sync = Command::new(env!("CARGO_BIN_EXE_claudius"));
        initial_sync
            .current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync", "--agent", "claude-code"])
            .assert()
            .success();

        fs::remove_file(fixture.config.join("agents").join("claude-code").join("reviewer.md"))
            .unwrap();
        fs::create_dir_all(fixture.project.join(".claude").join("agents")).unwrap();
        fs::write(fixture.project.join(".claude").join("agents").join("manual.md"), "manual")
            .unwrap();

        let mut prune_sync = Command::new(env!("CARGO_BIN_EXE_claudius"));
        prune_sync
            .current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync", "--agent", "claude-code", "--prune"])
            .assert()
            .success();

        assert!(!fixture.project_file_exists(".claude/agents/reviewer.md"));
        assert!(fixture.project_file_exists(".claude/agents/manual.md"));
    }

    #[test]
    #[serial]
    fn test_config_sync_gemini_prune_removes_only_managed_agents() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fixture
            .with_gemini_settings(r#"{"general":{"preferredEditor":"code"}}"#)
            .unwrap();
        fixture
            .with_gemini_agent(
                "reviewer",
                "---\nname: reviewer\ndescription: Review Gemini workflows\n---\nFocus on Gemini-specific regressions.",
            )
            .unwrap();

        let mut initial_sync = Command::new(env!("CARGO_BIN_EXE_claudius"));
        initial_sync
            .current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync", "--agent", "gemini"])
            .assert()
            .success();

        fs::remove_file(fixture.config.join("agents").join("gemini").join("reviewer.md")).unwrap();
        fs::create_dir_all(fixture.project.join(".gemini").join("agents")).unwrap();
        fs::write(fixture.project.join(".gemini").join("agents").join("manual.md"), "manual")
            .unwrap();

        let mut prune_sync = Command::new(env!("CARGO_BIN_EXE_claudius"));
        prune_sync
            .current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync", "--agent", "gemini", "--prune"])
            .assert()
            .success();

        assert!(!fixture.project_file_exists(".gemini/agents/reviewer.md"));
        assert!(fixture.project_file_exists(".gemini/agents/manual.md"));
    }
}
