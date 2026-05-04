use crate::fixtures::TestFixture;
use assert_cmd::Command;
use predicates::prelude::*;
use serial_test::serial;
use std::fs;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial]
    fn test_config_doctor_reports_legacy_and_supported_sources() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fixture.with_settings(r#"{"apiKeyHelper":"legacy-helper"}"#).unwrap();
        fixture.with_skill("shared-skill", "# Shared Skill").unwrap();
        fixture
            .with_gemini_system_defaults(r#"{"billing":{"project":"shared-project"}}"#)
            .unwrap();
        fixture
            .with_gemini_command(
                "review",
                "description = \"Review the current diff\"\nprompt = \"Review this change set.\"",
            )
            .unwrap();
        fixture
            .with_gemini_agent(
                "triage",
                "---\nname: triage\ndescription: Triage Gemini issues\n---\nFocus on Gemini-specific issues.",
            )
            .unwrap();
        fixture
            .with_claude_code_agent(
                "reviewer",
                "---\nname: reviewer\ndescription: Review code changes\n---\nFocus on regressions.",
            )
            .unwrap();
        fs::write(fixture.config.join("commands").join("legacy-review.md"), "# Legacy command")
            .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "doctor"])
            .assert()
            .success()
            .stdout(predicate::str::contains("SUPPORTED"))
            .stdout(predicate::str::contains("Shared skills source is present."))
            .stdout(predicate::str::contains("Gemini system defaults source is present."))
            .stdout(predicate::str::contains("Gemini custom command source is present."))
            .stdout(predicate::str::contains("Gemini agent source is present."))
            .stdout(predicate::str::contains("Claude Code subagent source is present."))
            .stdout(predicate::str::contains("LEGACY"))
            .stdout(predicate::str::contains(
                "Legacy settings.json is still active for Claude / Claude Code settings.",
            ))
            .stdout(predicate::str::contains(
                "Legacy commands/*.md skill fallback is still in use.",
            ));
    }

    #[test]
    #[serial]
    fn test_config_doctor_reports_stale_and_unmanaged_gemini_surfaces() {
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
        fixture
            .with_gemini_agent(
                "triage",
                "---\nname: triage\ndescription: Triage Gemini issues\n---\nFocus on Gemini-specific issues.",
            )
            .unwrap();

        let mut sync = Command::new(env!("CARGO_BIN_EXE_claudius"));
        sync.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "sync", "--agent", "gemini"])
            .assert()
            .success();

        fs::remove_file(fixture.config.join("commands").join("gemini").join("review.toml"))
            .unwrap();
        fs::remove_file(fixture.config.join("agents").join("gemini").join("triage.md")).unwrap();
        fs::create_dir_all(fixture.project.join(".gemini").join("extensions").join("sample"))
            .unwrap();
        fs::write(
            fixture
                .project
                .join(".gemini")
                .join("extensions")
                .join("sample")
                .join("extension.json"),
            "{}",
        )
        .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "doctor", "--agent", "gemini"])
            .assert()
            .success()
            .stdout(predicate::str::contains("UNMANAGED"))
            .stdout(predicate::str::contains(
                "Gemini extensions are present in an unmanaged target directory.",
            ))
            .stdout(predicate::str::contains(".gemini/extensions"))
            .stdout(predicate::str::contains("STALE"))
            .stdout(predicate::str::contains(
                "Claudius-managed Gemini commands target has stale deployed files.",
            ))
            .stdout(predicate::str::contains(
                "Claudius-managed Gemini agents target has stale deployed files.",
            ))
            .stdout(predicate::str::contains("claudius config sync --agent gemini --prune"));
    }

    #[test]
    #[serial]
    fn test_config_doctor_reports_unmanaged_claude_code_slash_commands() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fs::create_dir_all(fixture.project.join(".claude").join("commands")).unwrap();
        fs::write(
            fixture.project.join(".claude").join("commands").join("review.md"),
            "# Review command",
        )
        .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "doctor", "--agent", "claude-code"])
            .assert()
            .success()
            .stdout(predicate::str::contains("UNMANAGED"))
            .stdout(predicate::str::contains(
                "Claude Code slash commands are present in an unmanaged target directory.",
            ))
            .stdout(predicate::str::contains(".claude/commands"));
    }

    #[test]
    #[serial]
    fn test_config_doctor_global_reports_best_effort_surfaces() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fixture.with_existing_claude_desktop_config(r#"{"mcpServers": {}}"#).unwrap();

        let codex_skill_dir = fixture.config.join("skills").join("codex").join("reviewer");
        fs::create_dir_all(&codex_skill_dir).unwrap();
        fs::write(codex_skill_dir.join("SKILL.md"), "# Reviewer Skill").unwrap();

        let mut sync = Command::new(env!("CARGO_BIN_EXE_claudius"));
        sync.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["skills", "sync", "--global", "--agent", "codex"])
            .assert()
            .success();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "doctor", "--global"])
            .assert()
            .success()
            .stdout(predicate::str::contains("BEST-EFFORT"))
            .stdout(predicate::str::contains(
                "Claude Desktop JSON target is present as a legacy / best-effort surface.",
            ))
            .stdout(predicate::str::contains("claude_desktop_config.json"))
            .stdout(predicate::str::contains("Codex-specific skills source is present."))
            .stdout(predicate::str::contains("EXPERIMENTAL").not());
    }

    #[test]
    #[serial]
    fn test_config_doctor_reports_skill_renderer_migration_warnings() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fixture
            .with_skill(
                "shared-review",
                "---\nname: shared-review\ndescription: Review code changes.\ndisable-model-invocation: true\n---\n\nReview code changes.\n",
            )
            .unwrap();
        fixture
            .with_agent_skill(
                "codex",
                "codex-only",
                "---\nname: codex-only\ndescription: Codex-specific override.\n---\n\nUse Codex-specific instructions.\n",
            )
            .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "doctor", "--agent", "codex"])
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "Codex full override skill directories are still in use.",
            ))
            .stdout(predicate::str::contains(
                "Shared legacy skill contains Claude-specific metadata that Codex rendering will drop.",
            ))
            .stdout(predicate::str::contains("shared-review"))
            .stdout(predicate::str::contains("Run `claudius skills migrate` for these overrides"));
    }
}
