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
    fn test_init_command_creates_files() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Run config init command
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "init"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Claudius configuration bootstrapped successfully"));

        // Verify files were created
        let config_dir = &fixture.config;
        assert!(config_dir.join("mcpServers.json").exists());
        assert!(config_dir.join("settings.json").exists());
        assert!(config_dir.join("commands").exists());
        assert!(config_dir.join("commands/example.md").exists());
        assert!(config_dir.join("rules").exists());
        assert!(config_dir.join("rules/example.md").exists());

        // Verify content is valid JSON
        let mcp_content = fs::read_to_string(config_dir.join("mcpServers.json")).unwrap();
        let _: serde_json::Value = serde_json::from_str(&mcp_content).unwrap();

        let settings_content = fs::read_to_string(config_dir.join("settings.json")).unwrap();
        let _: serde_json::Value = serde_json::from_str(&settings_content).unwrap();
    }

    #[test]
    #[serial]
    fn test_init_command_preserves_existing() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create existing file with custom content
        let existing_content = r#"{"mcpServers": {"existing": {"command": "test"}}}"#;
        fixture.with_mcp_servers(existing_content).unwrap();

        // Run init without force
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "init"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Claudius configuration bootstrapped successfully"));

        // Verify existing file was preserved
        let content = fs::read_to_string(fixture.config.join("mcpServers.json")).unwrap();
        assert_eq!(content, existing_content);
    }

    #[test]
    #[serial]
    fn test_init_command_force_overwrites() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create existing file
        let existing_content = r#"{"mcpServers": {"existing": {"command": "test"}}}"#;
        fixture.with_mcp_servers(existing_content).unwrap();

        // Run init with force
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "init"])
            .arg("--force")
            .assert()
            .success()
            .stdout(predicate::str::contains("Claudius configuration bootstrapped successfully"));

        // Verify file was overwritten
        let content = fs::read_to_string(fixture.config.join("mcpServers.json")).unwrap();
        assert_ne!(content, existing_content);
        assert!(content.contains("filesystem")); // Default template content
    }
}
