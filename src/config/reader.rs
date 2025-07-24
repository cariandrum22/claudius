use super::{ClaudeConfig, McpServersConfig, Settings};
use crate::codex_settings::CodexSettings;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Read MCP servers configuration from a JSON file
///
/// # Errors
///
/// Returns an error if:
/// - Unable to read the file
/// - Unable to parse the JSON content
pub fn read_mcp_servers_config<P: AsRef<Path>>(path: P) -> anyhow::Result<McpServersConfig> {
    let content = fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("Failed to read MCP servers config: {}", e))?;

    let config: McpServersConfig = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse MCP servers config: {}", e))?;

    Ok(config)
}

/// Read Claude configuration from a JSON file
///
/// # Errors
///
/// Returns an error if:
/// - Unable to read the file (when it exists)
/// - Unable to parse the JSON content
pub fn read_claude_config<P: AsRef<Path>>(path: P) -> anyhow::Result<ClaudeConfig> {
    let path_ref = path.as_ref();

    if !path_ref.exists() {
        // Return empty config if file doesn't exist
        return Ok(ClaudeConfig { mcp_servers: None, other: HashMap::default() });
    }

    let content = fs::read_to_string(path_ref)
        .map_err(|e| anyhow::anyhow!("Failed to read claude.json: {}", e))?;

    let config: ClaudeConfig = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse claude.json: {}", e))?;

    Ok(config)
}

/// Read Claude settings from a JSON file
///
/// # Errors
///
/// Returns an error if:
/// - Unable to read the file (when it exists)
/// - Unable to parse the JSON content
pub fn read_settings<P: AsRef<Path>>(path: P) -> anyhow::Result<Option<Settings>> {
    let path_ref = path.as_ref();

    if !path_ref.exists() {
        // Return None if file doesn't exist
        return Ok(None);
    }

    let content = fs::read_to_string(path_ref)
        .map_err(|e| anyhow::anyhow!("Failed to read settings.json: {}", e))?;

    let settings: Settings = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse settings.json: {}", e))?;

    Ok(Some(settings))
}

/// Read Codex settings from a TOML file
///
/// # Errors
///
/// Returns an error if:
/// - Unable to read the file (when it exists)
/// - Unable to parse the TOML content
pub fn read_codex_settings<P: AsRef<Path>>(path: P) -> anyhow::Result<Option<CodexSettings>> {
    let path_ref = path.as_ref();

    if !path_ref.exists() {
        // Return None if file doesn't exist
        return Ok(None);
    }

    let content = fs::read_to_string(path_ref)
        .map_err(|e| anyhow::anyhow!("Failed to read codex settings TOML: {}", e))?;

    let settings: CodexSettings = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse codex settings TOML: {}", e))?;

    Ok(Some(settings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpServerConfig, Permissions};
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn create_test_mcp_servers_config() -> McpServersConfig {
        let mut servers = HashMap::new();
        servers.insert(
            "test-server".to_string(),
            McpServerConfig {
                command: "test-command".to_string(),
                args: vec!["arg1".to_string(), "arg2".to_string()],
                env: HashMap::new(),
            },
        );
        McpServersConfig { mcp_servers: servers }
    }

    fn create_test_settings() -> Settings {
        Settings {
            api_key_helper: Some("/bin/helper".to_string()),
            cleanup_period_days: Some(30),
            env: Some(HashMap::from([("KEY".to_string(), "value".to_string())])),
            include_co_authored_by: Some(true),
            permissions: Some(Permissions {
                allow: vec!["Read".to_string()],
                deny: vec!["Write".to_string()],
                default_mode: Some("allow".to_string()),
            }),
            preferred_notif_channel: Some("email".to_string()),
            mcp_servers: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_read_mcp_servers_config_success() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_path = temp_dir.path().join("mcpServers.json");

        let config = create_test_mcp_servers_config();
        let json = serde_json::to_string_pretty(&config).expect("Failed to serialize JSON");
        fs::write(&config_path, json).expect("Failed to write file");

        let result =
            read_mcp_servers_config(&config_path).expect("read_mcp_servers_config should succeed");
        assert_eq!(result.mcp_servers.len(), 1);
        assert!(result.mcp_servers.contains_key("test-server"));
        assert_eq!(
            result.mcp_servers.get("test-server").map(|s| &s.command),
            Some(&"test-command".to_string())
        );
    }

    #[test]
    fn test_read_mcp_servers_config_missing_file() {
        let result = read_mcp_servers_config("/nonexistent/mcpServers.json");
        assert!(result.is_err());
        assert!(result
            .expect_err("Should return error for missing file")
            .to_string()
            .contains("Failed to read MCP servers config"));
    }

    #[test]
    fn test_read_mcp_servers_config_invalid_json() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_path = temp_dir.path().join("mcpServers.json");

        fs::write(&config_path, "{ invalid json").expect("Failed to write invalid JSON");

        let result = read_mcp_servers_config(&config_path);
        assert!(result.is_err());
        assert!(result
            .expect_err("Should return error for invalid JSON")
            .to_string()
            .contains("Failed to parse MCP servers config"));
    }

    #[test]
    fn test_read_claude_config_success() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_path = temp_dir.path().join("claude.json");

        let config = ClaudeConfig {
            mcp_servers: Some(HashMap::from([(
                "server1".to_string(),
                McpServerConfig { command: "cmd1".to_string(), args: vec![], env: HashMap::new() },
            )])),
            other: HashMap::from([("key".to_string(), serde_json::json!("value"))]),
        };

        let json = serde_json::to_string_pretty(&config).expect("Failed to serialize JSON");
        fs::write(&config_path, json).expect("Failed to write file");

        let result = read_claude_config(&config_path).expect("read_claude_config should succeed");
        assert!(result.mcp_servers.is_some());
        assert_eq!(result.mcp_servers.expect("MCP servers should be present").len(), 1);
        assert_eq!(result.other.get("key"), Some(&serde_json::Value::String("value".to_string())));
    }

    #[test]
    fn test_read_claude_config_missing_file() {
        let result = read_claude_config("/nonexistent/claude.json")
            .expect("read_claude_config should succeed for missing file");
        assert!(result.mcp_servers.is_none());
        assert!(result.other.is_empty());
    }

    #[test]
    fn test_read_claude_config_invalid_json() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_path = temp_dir.path().join("claude.json");

        fs::write(&config_path, "{ invalid json").expect("Failed to write invalid JSON");

        let result = read_claude_config(&config_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to parse claude.json"));
    }

    #[test]
    fn test_read_settings_success() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let settings_path = temp_dir.path().join("settings.json");

        let settings = create_test_settings();
        let json = serde_json::to_string_pretty(&settings).expect("Failed to serialize settings");
        fs::write(&settings_path, json).expect("Failed to write settings file");

        let result = read_settings(&settings_path).expect("read_settings should succeed");
        assert!(result.is_some());
        let parsed_settings = result.expect("Settings should be present");
        assert_eq!(parsed_settings.api_key_helper, Some("/bin/helper".to_string()));
        assert_eq!(parsed_settings.cleanup_period_days, Some(30));
    }

    #[test]
    fn test_read_settings_missing_file() {
        let result = read_settings("/nonexistent/settings.json")
            .expect("read_settings should succeed for missing file");
        assert!(result.is_none());
    }

    #[test]
    fn test_read_settings_invalid_json() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let settings_path = temp_dir.path().join("settings.json");

        fs::write(&settings_path, "{ invalid json").expect("Failed to write invalid JSON");

        let result = read_settings(&settings_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to parse settings.json"));
    }

    #[test]
    fn test_read_codex_settings_success() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let settings_path = temp_dir.path().join("codex.toml");

        let toml_content = r#"
model = "gpt-4"
model_provider = "openai"
approval_policy = "auto"
disable_response_storage = false
notify = ["email", "slack"]

[shell_environment_policy]
policy = "allow"
allowed = ["PATH", "HOME"]
"#;

        fs::write(&settings_path, toml_content).expect("Failed to write TOML content");

        let result =
            read_codex_settings(&settings_path).expect("read_codex_settings should succeed");
        assert!(result.is_some());
        let settings = result.expect("Codex settings should be present");
        assert_eq!(settings.model, Some("gpt-4".to_string()));
        assert_eq!(settings.model_provider, Some("openai".to_string()));
        assert_eq!(settings.approval_policy, Some("auto".to_string()));
        assert_eq!(settings.disable_response_storage, Some(false));
        assert_eq!(settings.notify, Some(vec!["email".to_string(), "slack".to_string()]));
    }

    #[test]
    fn test_read_codex_settings_missing_file() {
        let result = read_codex_settings("/nonexistent/codex.toml")
            .expect("read_codex_settings should succeed for missing file");
        assert!(result.is_none());
    }

    #[test]
    fn test_read_codex_settings_invalid_toml() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let settings_path = temp_dir.path().join("codex.toml");

        fs::write(&settings_path, "invalid toml [[").expect("Failed to write invalid TOML");

        let result = read_codex_settings(&settings_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to parse codex settings TOML"));
    }
}
