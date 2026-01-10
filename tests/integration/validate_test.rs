use crate::fixtures::TestFixture;
use assert_cmd::Command;
use serial_test::serial;

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
}
