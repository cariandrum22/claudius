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
        "contextFileName": "GEMINI.md",
        "autoAccept": true,
        "theme": "dark"
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

        // Gemini CLI stores settings and MCP servers together in .gemini/settings.json
        assert!(!fixture.project_file_exists(".mcp.json"));

        let settings_content = fixture.read_project_file(".gemini/settings.json").unwrap();
        let settings_json: serde_json::Value = serde_json::from_str(&settings_content).unwrap();

        assert_eq!(
            settings_json
                .get("mcpServers")
                .and_then(|s| s.get("test-server"))
                .and_then(|t| t.get("command")),
            Some(&serde_json::Value::String("test".to_string()))
        );

        assert_eq!(
            settings_json.get("contextFileName"),
            Some(&serde_json::Value::String("GEMINI.md".to_string()))
        );
        assert_eq!(settings_json.get("autoAccept"), Some(&serde_json::Value::Bool(true)));
        assert_eq!(
            settings_json.get("theme"),
            Some(&serde_json::Value::String("dark".to_string()))
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

        // Create claude.settings.json (used by Claude Code; Claude Desktop ignores it)
        fixture
            .with_claude_settings(
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

        // Create initial Claude Desktop config to verify key preservation
        fixture
            .with_existing_claude_desktop_config(r#"{"existingKey": "value"}"#)
            .unwrap();

        let claude_desktop_config_path =
            fixture.config_home().join("Claude").join("claude_desktop_config.json");

        // Run sync with --global flag
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "sync"])
            .arg("--global")
            .arg("--agent")
            .arg("claude")
            .assert()
            .success();

        // Verify Claude Desktop config contains MCP servers and preserves existing keys
        let content = std::fs::read_to_string(&claude_desktop_config_path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Check MCP servers
        assert_eq!(
            json.get("mcpServers")
                .and_then(|s| s.get("test-server"))
                .and_then(|t| t.get("command")),
            Some(&serde_json::Value::String("test".to_string()))
        );

        // Settings are not merged into Claude Desktop config
        assert!(json.get("apiKeyHelper").is_none());
        assert!(json.get("cleanupPeriodDays").is_none());
        assert!(json.get("env").is_none());
        assert!(json.get("includeCoAuthoredBy").is_none());
        assert!(json.get("permissions").is_none());

        // Check existing key preserved
        assert_eq!(json.get("existingKey"), Some(&serde_json::Value::String("value".to_string())));

        // Verify legacy ~/.claude.json was NOT created
        assert!(!fixture.home_file_exists(".claude.json"));
    }

    #[test]
    #[serial]
    fn test_sync_claude_code_global_writes_settings_json() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create app config with Claude Code as default agent to avoid multi-agent global sync.
        let app_config_path = fixture.config.join("config.toml");
        std::fs::write(
            app_config_path,
            r#"
[default]
agent = "claude-code"
"#,
        )
        .unwrap();

        // Create mcpServers.json
        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "type": "http",
                "url": "https://example.com/mcp",
                "headers": {
                    "Authorization": "Bearer token"
                }
            }
        }
    }"#,
            )
            .unwrap();

        // Create claude.settings.json (source settings) with Claude Code-specific fields
        fixture
            .with_claude_settings(
                r#"{
        "apiKeyHelper": "/bin/generate_temp_api_key.sh",
        "sandbox": {
            "enabled": true
        },
        "companyAnnouncements": ["hello"]
    }"#,
            )
            .unwrap();

        // Create initial claude.json in home directory (should not receive settings fields)
        fixture.with_existing_global_config(r#"{"existingKey": "value"}"#).unwrap();

        // Create existing ~/.claude/settings.json with unrelated keys to ensure merge preservation
        let claude_dir = fixture.home_dir().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{
        "env": {"KEEP": "1"},
        "sandbox": {"autoAllowBashIfSandboxed": true}
    }"#,
        )
        .unwrap();

        // Run sync with --global flag (no --agent, should pick config.toml default)
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "sync"])
            .arg("--global")
            .assert()
            .success();

        // Verify ~/.claude.json contains MCP servers and preserves existing keys
        let content = fixture.read_home_file(".claude.json").unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(json.get("existingKey"), Some(&serde_json::Value::String("value".to_string())));
        assert_eq!(
            json.get("mcpServers")
                .and_then(|s| s.get("test-server"))
                .and_then(|t| t.get("type")),
            Some(&serde_json::Value::String("http".to_string()))
        );
        assert!(json.get("apiKeyHelper").is_none());

        // Verify ~/.claude/settings.json contains merged settings and preserves existing fields
        let settings_content = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
        let settings_json: serde_json::Value = serde_json::from_str(&settings_content).unwrap();
        assert_eq!(
            settings_json.get("apiKeyHelper"),
            Some(&serde_json::Value::String("/bin/generate_temp_api_key.sh".to_string()))
        );
        assert_eq!(
            settings_json.get("env").and_then(|env| env.get("KEEP")),
            Some(&serde_json::Value::String("1".to_string()))
        );
        assert_eq!(
            settings_json.get("sandbox").and_then(|s| s.get("enabled")),
            Some(&serde_json::Value::Bool(true))
        );
        assert_eq!(
            settings_json.get("sandbox").and_then(|s| s.get("autoAllowBashIfSandboxed")),
            Some(&serde_json::Value::Bool(true))
        );
        assert!(settings_json.get("mcpServers").is_none());
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

        // Gemini CLI stores settings and MCP servers together in .gemini/settings.json
        assert!(!fixture.project_file_exists(".mcp.json"));

        let content = fixture.read_project_file(".gemini/settings.json").unwrap();
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
