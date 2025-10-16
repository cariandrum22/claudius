use crate::fixtures::TestFixture;
use assert_cmd::Command;
use predicates::prelude::*;
use serial_test::serial;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial]
    fn test_sync_with_settings_project_local() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create mcpServers.json
        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "command": "test",
                "args": [],
                "env": {}
            }
        }
    }"#,
            )
            .unwrap();

        // Create gemini.settings.json
        fixture
            .with_gemini_settings(
                r#"{
        "apiKeyHelper": "/bin/generate_api_key.sh",
        "cleanupPeriodDays": 20,
        "env": {"FOO": "bar"},
        "includeCoAuthoredBy": false,
        "permissions": {
            "allow": ["Bash(npm run lint)"]
        }
    }"#,
            )
            .unwrap();

        // Run sync (project-local mode by default)
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync"])
            .arg("--agent")
            .arg("gemini")
            .assert()
            .success();

        // Verify .mcp.json contains only mcpServers
        let mcp_content = fixture.read_project_file(".mcp.json").unwrap();
        let mcp_json: serde_json::Value = serde_json::from_str(&mcp_content).unwrap();

        assert_eq!(
            mcp_json
                .get("mcpServers")
                .and_then(|s| s.get("test-server"))
                .and_then(|t| t.get("command")),
            Some(&serde_json::Value::String("test".to_string()))
        );
        assert!(mcp_json.get("apiKeyHelper").is_none());
        assert!(mcp_json.get("cleanupPeriodDays").is_none());

        // Verify gemini/settings.json contains settings (for gemini agent)
        let settings_content = fixture.read_project_file("gemini/settings.json").unwrap();
        let settings_json: serde_json::Value = serde_json::from_str(&settings_content).unwrap();

        assert_eq!(
            settings_json.get("apiKeyHelper"),
            Some(&serde_json::Value::String("/bin/generate_api_key.sh".to_string()))
        );
        assert_eq!(
            settings_json.get("cleanupPeriodDays"),
            Some(&serde_json::Value::Number(serde_json::Number::from(20)))
        );
        assert_eq!(
            settings_json.get("env").and_then(|e| e.get("FOO")),
            Some(&serde_json::Value::String("bar".to_string()))
        );
        assert_eq!(settings_json.get("includeCoAuthoredBy"), Some(&serde_json::Value::Bool(false)));
        assert_eq!(
            settings_json
                .get("permissions")
                .and_then(|p| p.get("allow"))
                .and_then(|a| a.get(0)),
            Some(&serde_json::Value::String("Bash(npm run lint)".to_string()))
        );
    }

    #[test]
    #[serial]
    fn test_sync_with_settings_global() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create mcpServers.json
        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "command": "test",
                "args": [],
                "env": {}
            }
        }
    }"#,
            )
            .unwrap();

        // Create claude.settings.json (default when no agent specified)
        let claude_settings_path = fixture.config.join("claude.settings.json");
        std::fs::write(
            claude_settings_path,
            r#"{
        "apiKeyHelper": "/bin/generate_api_key.sh",
        "cleanupPeriodDays": 20,
        "env": {"FOO": "bar"},
        "includeCoAuthoredBy": false,
        "permissions": {
            "allow": ["Bash(npm run lint)"]
        }
    }"#,
        )
        .unwrap();

        // Create initial claude.json in home directory
        fixture.with_existing_global_config(r#"{"existingKey": "value"}"#).unwrap();

        let claude_file_path = fixture.home_dir().join(".claude.json");

        // Run sync with --global flag
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "sync"])
            .arg("--global")
            .arg("--target-config")
            .arg(&claude_file_path)
            .assert()
            .success();

        // Verify claude.json contains both mcpServers and settings
        let content = fixture.read_home_file(".claude.json").unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Check MCP servers
        assert_eq!(
            json.get("mcpServers")
                .and_then(|s| s.get("test-server"))
                .and_then(|t| t.get("command")),
            Some(&serde_json::Value::String("test".to_string()))
        );

        // Check settings
        assert_eq!(
            json.get("apiKeyHelper"),
            Some(&serde_json::Value::String("/bin/generate_api_key.sh".to_string()))
        );
        assert_eq!(
            json.get("cleanupPeriodDays"),
            Some(&serde_json::Value::Number(serde_json::Number::from(20)))
        );
        assert_eq!(
            json.get("env").and_then(|e| e.get("FOO")),
            Some(&serde_json::Value::String("bar".to_string()))
        );
        assert_eq!(json.get("includeCoAuthoredBy"), Some(&serde_json::Value::Bool(false)));
        assert_eq!(
            json.get("permissions").and_then(|p| p.get("allow")).and_then(|a| a.get(0)),
            Some(&serde_json::Value::String("Bash(npm run lint)".to_string()))
        );

        // Check existing key preserved
        assert_eq!(json.get("existingKey"), Some(&serde_json::Value::String("value".to_string())));
    }

    #[test]
    #[serial]
    fn test_sync_without_settings() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create only mcpServers.json
        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "command": "test",
                "args": [],
                "env": {}
            }
        }
    }"#,
            )
            .unwrap();

        // Run sync
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync"])
            .arg("--agent")
            .arg("gemini")
            .assert()
            .success();

        // Verify .mcp.json created
        let content = fixture.read_project_file(".mcp.json").unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Check MCP servers
        assert_eq!(
            json.get("mcpServers")
                .and_then(|s| s.get("test-server"))
                .and_then(|t| t.get("command")),
            Some(&serde_json::Value::String("test".to_string()))
        );

        // Check no settings added
        assert!(json.get("apiKeyHelper").is_none());
        assert!(json.get("cleanupPeriodDays").is_none());

        // Check gemini/settings.json was not created
        assert!(!fixture.project_file_exists("gemini/settings.json"));
    }

    #[test]
    #[serial]
    fn test_dry_run_with_settings() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create mcpServers.json
        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "command": "test",
                "args": [],
                "env": {}
            }
        }
    }"#,
            )
            .unwrap();

        // Create gemini.settings.json
        fixture
            .with_gemini_settings(
                r#"{
        "apiKeyHelper": "/bin/generate_api_key.sh",
        "cleanupPeriodDays": 20
    }"#,
            )
            .unwrap();

        // Run sync with dry-run
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync"])
            .arg("--agent")
            .arg("gemini")
            .arg("--dry-run")
            .assert()
            .success()
            .stdout(predicate::str::contains("test-server"))
            .stdout(predicate::str::contains("apiKeyHelper"));

        // Verify files were NOT created
        assert!(!fixture.project_file_exists(".mcp.json"));
        assert!(!fixture.project_file_exists("gemini"));
    }
}
