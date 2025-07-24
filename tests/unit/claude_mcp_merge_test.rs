use claudius::config::{McpServerConfig, Permissions, Settings};
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_includes_mcp_servers() {
        // Test that Settings struct can hold MCP servers
        let mut mcp_servers = HashMap::new();
        mcp_servers.insert(
            "test-server".to_string(),
            McpServerConfig {
                command: "node".to_string(),
                args: vec!["server.js".to_string()],
                env: HashMap::new(),
            },
        );

        let settings = Settings {
            api_key_helper: Some("/path/to/helper".to_string()),
            cleanup_period_days: Some(30),
            env: Some(HashMap::new()),
            include_co_authored_by: Some(true),
            permissions: Some(Permissions {
                allow: vec!["Read".to_string()],
                deny: vec!["Write".to_string()],
                default_mode: Some("allow".to_string()),
            }),
            preferred_notif_channel: Some("chat".to_string()),
            mcp_servers: Some(mcp_servers.clone()),
            extra: HashMap::new(),
        };

        assert!(settings.mcp_servers.is_some());
        assert_eq!(settings.mcp_servers.unwrap().len(), 1);
    }

    #[test]
    fn test_settings_serialization_with_mcp_servers() {
        let mut mcp_servers = HashMap::new();
        mcp_servers.insert(
            "filesystem".to_string(),
            McpServerConfig {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string()],
                env: HashMap::new(),
            },
        );

        let settings = Settings {
            api_key_helper: None,
            cleanup_period_days: None,
            env: None,
            include_co_authored_by: None,
            permissions: None,
            preferred_notif_channel: None,
            mcp_servers: Some(mcp_servers),
            extra: HashMap::new(),
        };

        let json = serde_json::to_string_pretty(&settings).unwrap();
        assert!(json.contains("\"mcpServers\""));
        assert!(json.contains("\"filesystem\""));
        assert!(json.contains("\"command\": \"npx\""));
    }

    #[test]
    fn test_settings_deserialization_with_mcp_servers() {
        let json = r#"{
            "apiKeyHelper": "/helper.sh",
            "mcpServers": {
                "server1": {
                    "command": "python",
                    "args": ["-m", "server"],
                    "env": {}
                }
            }
        }"#;

        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.api_key_helper, Some("/helper.sh".to_string()));
        assert!(settings.mcp_servers.is_some());

        let mcp_servers = settings.mcp_servers.unwrap();
        assert_eq!(mcp_servers.len(), 1);
        assert!(mcp_servers.contains_key("server1"));

        let server = mcp_servers.get("server1").unwrap();
        assert_eq!(server.command, "python");
        assert_eq!(server.args.len(), 2);
    }

    #[test]
    fn test_settings_with_empty_mcp_servers() {
        let settings = Settings {
            api_key_helper: Some("/path".to_string()),
            cleanup_period_days: None,
            env: None,
            include_co_authored_by: None,
            permissions: None,
            preferred_notif_channel: None,
            mcp_servers: None,
            extra: HashMap::new(),
        };

        let json = serde_json::to_string(&settings).unwrap();
        // mcpServers should not appear when None (skip_serializing_if)
        assert!(!json.contains("mcpServers"));
    }
}
