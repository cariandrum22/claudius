use assert_fs::prelude::*;
use claudius::config::reader;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_mcp_servers_config_success() {
        let temp_file = assert_fs::NamedTempFile::new("mcpServers.json").unwrap();
        temp_file
            .write_str(
                r#"{
        "mcpServers": {
            "test-server": {
                "command": "node",
                "args": ["index.js"],
                "env": {
                    "PORT": "8080"
                }
            }
        }
    }"#,
            )
            .unwrap();

        let config = reader::read_mcp_servers_config(temp_file.path()).unwrap();
        assert_eq!(config.mcp_servers.len(), 1);
        assert!(config.mcp_servers.contains_key("test-server"));

        let server = config.mcp_servers.get("test-server").unwrap();
        assert_eq!(server.command.as_deref(), Some("node"));
        assert_eq!(server.args, vec!["index.js"]);
        assert_eq!(server.env.get("PORT"), Some(&"8080".to_string()));
    }

    #[test]
    fn test_read_mcp_servers_config_file_not_found() {
        let result = reader::read_mcp_servers_config("/nonexistent/path/mcpServers.json");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to read MCP servers config"));
    }

    #[test]
    fn test_read_mcp_servers_config_invalid_json() {
        let temp_file = assert_fs::NamedTempFile::new("invalid.json").unwrap();
        temp_file.write_str("{ invalid json }").unwrap();

        let result = reader::read_mcp_servers_config(temp_file.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to parse MCP servers config"));
    }

    #[test]
    fn test_read_claude_config_success() {
        let temp_file = assert_fs::NamedTempFile::new("claude.json").unwrap();
        temp_file
            .write_str(
                r#"{
        "mcpServers": {
            "existing-server": {
                "command": "python",
                "args": ["-m", "server"],
                "env": {}
            }
        },
        "theme": "dark",
        "fontSize": 14
    }"#,
            )
            .unwrap();

        let config = reader::read_claude_config(temp_file.path()).unwrap();

        // Check MCP servers
        assert!(config.mcp_servers.is_some());
        let servers = config.mcp_servers.as_ref().unwrap();
        assert_eq!(servers.len(), 1);
        assert!(servers.contains_key("existing-server"));

        // Check other fields are preserved
        assert_eq!(config.other.get("theme"), Some(&serde_json::json!("dark")));
        assert_eq!(config.other.get("fontSize"), Some(&serde_json::json!(14)));
    }

    #[test]
    fn test_read_claude_config_no_mcp_servers() {
        let temp_file = assert_fs::NamedTempFile::new("claude.json").unwrap();
        temp_file
            .write_str(
                r#"{
        "theme": "light",
        "autoSave": true
    }"#,
            )
            .unwrap();

        let config = reader::read_claude_config(temp_file.path()).unwrap();
        assert!(config.mcp_servers.is_none());
        assert_eq!(config.other.get("theme"), Some(&serde_json::json!("light")));
        assert_eq!(config.other.get("autoSave"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn test_read_claude_config_nonexistent_returns_empty() {
        let config = reader::read_claude_config("/nonexistent/claude.json").unwrap();
        assert!(config.mcp_servers.is_none());
        assert!(config.other.is_empty());
    }

    #[test]
    fn test_read_claude_config_empty_file() {
        let temp_file = assert_fs::NamedTempFile::new("empty.json").unwrap();
        temp_file.write_str("{}").unwrap();

        let config = reader::read_claude_config(temp_file.path()).unwrap();
        assert!(config.mcp_servers.is_none());
        assert!(config.other.is_empty());
    }
}
