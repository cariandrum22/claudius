use assert_fs::prelude::*;
use claudius::config::{writer, ClaudeConfig, McpServerConfig, McpServersConfig, Settings};
use serde_json::json;
use std::collections::HashMap;
use std::fs;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_claude_config_success() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let config_path = temp_dir.path().join("claude.json");

        let mut mcp_servers = HashMap::new();
        mcp_servers.insert(
            "test-server".to_string(),
            McpServerConfig {
                command: Some("python".to_string()),
                args: vec!["-m".to_string(), "server".to_string()],
                env: HashMap::from([("DEBUG".to_string(), "true".to_string())]),
                server_type: None,
                url: None,
                headers: HashMap::new(),
                extra: HashMap::new(),
            },
        );

        let config = ClaudeConfig {
            mcp_servers: Some(mcp_servers),
            other: HashMap::from([
                ("theme".to_string(), serde_json::json!("dark")),
                ("autoSave".to_string(), serde_json::json!(true)),
            ]),
        };

        writer::write_claude_config(&config_path, &config).unwrap();

        // Verify file was created
        assert!(config_path.exists());

        // Verify content
        let content = fs::read_to_string(&config_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed.get("theme"), Some(&json!("dark")));
        assert_eq!(parsed.get("autoSave"), Some(&json!(true)));
        assert_eq!(
            parsed
                .get("mcpServers")
                .and_then(|m| m.get("test-server"))
                .and_then(|s| s.get("command")),
            Some(&json!("python"))
        );
        assert_eq!(
            parsed
                .get("mcpServers")
                .and_then(|m| m.get("test-server"))
                .and_then(|s| s.get("env"))
                .and_then(|e| e.get("DEBUG")),
            Some(&json!("true"))
        );
    }

    #[test]
    fn test_write_claude_config_creates_parent_directory() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let nested_path = temp_dir.path().join("nested/dir/claude.json");

        let config = ClaudeConfig { mcp_servers: None, other: HashMap::new() };

        writer::write_claude_config(&nested_path, &config).unwrap();

        assert!(nested_path.exists());
        assert!(nested_path.parent().unwrap().exists());
    }

    #[test]
    fn test_write_preserves_json_formatting() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let config_path = temp_dir.path().join("claude.json");

        let config = ClaudeConfig {
            mcp_servers: Some(HashMap::from([(
                "server1".to_string(),
                McpServerConfig {
                    command: Some("cmd".to_string()),
                    args: vec![],
                    env: HashMap::new(),
                    server_type: None,
                    url: None,
                    headers: HashMap::new(),
                    extra: HashMap::new(),
                },
            )])),
            other: HashMap::new(),
        };

        writer::write_claude_config(&config_path, &config).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        // Check that output is pretty-printed (contains newlines and indentation)
        assert!(content.contains('\n'));
        assert!(content.contains("  ")); // indentation
    }

    #[test]
    fn test_backup_file_success() {
        let temp_file = assert_fs::NamedTempFile::new("claude.json").unwrap();
        temp_file.write_str(r#"{"test": "data"}"#).unwrap();

        let backup_path = writer::backup_file(temp_file.path()).unwrap();
        assert!(backup_path.is_some());

        let backup_file_path = backup_path.unwrap();
        assert!(backup_file_path.contains("claude.json.backup."));
        assert!(std::path::Path::new(&backup_file_path).exists());

        // Verify backup content matches original
        let original_content = fs::read_to_string(temp_file.path()).unwrap();
        let backup_content = fs::read_to_string(&backup_file_path).unwrap();
        assert_eq!(original_content, backup_content);
    }

    #[test]
    fn test_backup_file_nonexistent_returns_none() {
        let result = writer::backup_file("/nonexistent/file.json").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_backup_file_timestamp_format() {
        let temp_file = assert_fs::NamedTempFile::new("test.json").unwrap();
        temp_file.write_str("{}").unwrap();

        let backup_path = writer::backup_file(temp_file.path()).unwrap().unwrap();

        // Check that backup filename contains timestamp pattern
        assert!(backup_path.contains("test.json.backup."));

        // Extract timestamp part and verify format (YYYYMMDD_HHMMSS)
        let parts: Vec<&str> = backup_path.split('.').collect();
        let timestamp = parts.last().expect("Should have timestamp part");
        assert_eq!(timestamp.len(), 15); // YYYYMMDD_HHMMSS
        assert!(timestamp.contains('_'));
    }

    #[test]
    fn test_write_mcp_servers_config() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".mcp.json");

        let mut mcp_servers = HashMap::new();
        mcp_servers.insert(
            "test-server".to_string(),
            McpServerConfig {
                command: Some("node".to_string()),
                args: vec!["server.js".to_string()],
                env: HashMap::from([("PORT".to_string(), "3000".to_string())]),
                server_type: None,
                url: None,
                headers: HashMap::new(),
                extra: HashMap::new(),
            },
        );

        let config = McpServersConfig { mcp_servers };

        writer::write_mcp_servers_config(&config_path, &config).unwrap();

        // Verify file was created
        assert!(config_path.exists());

        // Verify content
        let content = fs::read_to_string(&config_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(
            parsed
                .get("mcpServers")
                .and_then(|m| m.get("test-server"))
                .and_then(|s| s.get("command")),
            Some(&json!("node"))
        );
        assert_eq!(
            parsed
                .get("mcpServers")
                .and_then(|m| m.get("test-server"))
                .and_then(|s| s.get("args"))
                .and_then(|a| a.get(0)),
            Some(&json!("server.js"))
        );
        assert_eq!(
            parsed
                .get("mcpServers")
                .and_then(|m| m.get("test-server"))
                .and_then(|s| s.get("env"))
                .and_then(|e| e.get("PORT")),
            Some(&json!("3000"))
        );
    }

    #[test]
    fn test_write_settings() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let settings_path = temp_dir.path().join(".claude").join("settings.json");

        let settings = Settings {
            api_key_helper: Some("/bin/test.sh".to_string()),
            cleanup_period_days: Some(15),
            env: Some(HashMap::from([("TEST".to_string(), "value".to_string())])),
            include_co_authored_by: Some(false),
            permissions: None,
            preferred_notif_channel: Some("email".to_string()),
            mcp_servers: None,
            extra: HashMap::new(),
        };

        writer::write_settings(&settings_path, &settings).unwrap();

        // Verify file was created
        assert!(settings_path.exists());

        // Verify content
        let content = fs::read_to_string(&settings_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(parsed.get("apiKeyHelper"), Some(&json!("/bin/test.sh")));
        assert_eq!(parsed.get("cleanupPeriodDays"), Some(&json!(15)));
        assert_eq!(parsed.get("env").and_then(|e| e.get("TEST")), Some(&json!("value")));
        assert_eq!(parsed.get("includeCoAuthoredBy"), Some(&json!(false)));
        assert_eq!(parsed.get("preferredNotifChannel"), Some(&json!("email")));
    }

    #[test]
    fn test_write_settings_creates_parent_directory() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let settings_path = temp_dir.path().join("nested/.claude/settings.json");

        let settings = Settings {
            api_key_helper: None,
            cleanup_period_days: None,
            env: None,
            include_co_authored_by: None,
            permissions: None,
            preferred_notif_channel: None,
            mcp_servers: None,
            extra: HashMap::new(),
        };

        writer::write_settings(&settings_path, &settings).unwrap();

        assert!(settings_path.exists());
        assert!(settings_path.parent().unwrap().exists());
    }
}
