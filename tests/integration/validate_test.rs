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
    fn test_config_validate_passes_with_minimal_config() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "validate"])
            .assert()
            .success();
    }

    #[test]
    #[serial]
    fn test_config_validate_strict_fails_on_warnings() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // MCP server missing both command and url should trigger a warning.
        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "broken-server": {
                "args": ["--help"],
                "env": {}
            }
        }
    }"#,
            )
            .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "validate", "--strict"])
            .assert()
            .failure();
    }

    #[test]
    #[serial]
    fn test_config_validate_codex_managed_config_is_supported() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        std::fs::write(fixture.config.join("codex.managed_config.toml"), "").unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "validate", "--agent", "codex"])
            .assert()
            .success();
    }

    #[test]
    #[serial]
    fn test_config_validate_codex_managed_config_invalid_toml_fails() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        std::fs::write(fixture.config.join("codex.managed_config.toml"), "not =").unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "validate", "--agent", "codex"])
            .assert()
            .failure();
    }

    #[test]
    #[serial]
    fn test_config_validate_gemini_command_missing_prompt_fails() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fixture
            .with_gemini_command("review", "description = \"Review the current diff\"")
            .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "validate", "--agent", "gemini"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("missing field `prompt`"));
    }

    #[test]
    #[serial]
    fn test_config_validate_claude_code_subagent_missing_frontmatter_fails() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fixture.with_claude_code_agent("reviewer", "Focus on regressions.").unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "validate", "--agent", "claude-code"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("must start with YAML frontmatter delimited by ---"));
    }

    #[test]
    #[serial]
    fn test_config_validate_codex_skills_emit_compatibility_warning() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fixture.with_skill("shared-skill", "# Shared Skill").unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "validate", "--agent", "codex"])
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "Codex skills sync remains experimental and publishes to both .codex/skills and .agents/skills for compatibility",
            ));
    }

    #[test]
    #[serial]
    fn test_config_validate_warns_when_onepassword_subtable_is_ignored() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fs::write(
            fixture.config.join("config.toml"),
            r#"
[secret-manager]
type = "vault"

[secret-manager.onepassword]
mode = "service-account"
service-account-token-path = "~/.config/op/service-accounts/headless-linux-cli.token"
"#,
        )
        .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "validate"])
            .assert()
            .success()
            .stdout(predicate::str::contains("[secret-manager.onepassword]"))
            .stderr(predicate::str::contains("[secret-manager.onepassword]").not());
    }

    #[test]
    #[serial]
    fn test_config_validate_strict_fails_when_onepassword_subtable_is_ignored() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();
        fs::write(
            fixture.config.join("config.toml"),
            r#"
[secret-manager]
type = "vault"

[secret-manager.onepassword]
mode = "service-account"
"#,
        )
        .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "validate", "--strict"])
            .assert()
            .failure()
            .stdout(predicate::str::contains("[secret-manager.onepassword]"))
            .stderr(predicate::str::contains("Validation failed due to warnings"))
            .stderr(predicate::str::contains("[secret-manager.onepassword]").not());
    }
}
