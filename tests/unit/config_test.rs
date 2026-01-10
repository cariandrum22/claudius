use claudius::config::{McpServerConfig, McpServersConfig};
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_config_structure() {
        let config = McpServerConfig {
            command: Some("node".to_string()),
            args: vec!["server.js".to_string()],
            env: HashMap::from([("API_KEY".to_string(), "secret".to_string())]),
            server_type: None,
            url: None,
            headers: HashMap::new(),
            extra: HashMap::new(),
        };

        assert_eq!(config.command.as_deref(), Some("node"));
        assert_eq!(config.args, vec!["server.js"]);
        assert_eq!(config.env.get("API_KEY"), Some(&"secret".to_string()));
    }

    #[test]
    fn test_mcp_servers_config_structure() {
        let mut servers = HashMap::new();
        servers.insert(
            "test-server".to_string(),
            McpServerConfig {
                command: Some("python".to_string()),
                args: vec!["-m".to_string(), "server".to_string()],
                env: HashMap::new(),
                server_type: None,
                url: None,
                headers: HashMap::new(),
                extra: HashMap::new(),
            },
        );

        let config = McpServersConfig { mcp_servers: servers };

        assert!(config.mcp_servers.contains_key("test-server"));
        let server = config.mcp_servers.get("test-server").unwrap();
        assert_eq!(server.command.as_deref(), Some("python"));
        assert_eq!(server.args.len(), 2);
    }

    #[test]
    fn test_mcp_server_config_serialization() {
        let config = McpServerConfig {
            command: Some("deno".to_string()),
            args: vec!["run".to_string(), "server.ts".to_string()],
            env: HashMap::from([("PORT".to_string(), "3000".to_string())]),
            server_type: None,
            url: None,
            headers: HashMap::new(),
            extra: HashMap::new(),
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"command\":\"deno\""));
        assert!(json.contains("\"args\":[\"run\",\"server.ts\"]"));
        assert!(json.contains("\"PORT\":\"3000\""));
    }

    #[test]
    fn test_mcp_server_config_deserialization() {
        let json = r#"{
            "command": "bun",
            "args": ["server.js"],
            "env": {
                "DEBUG": "true"
            }
        }"#;

        let config: McpServerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.command.as_deref(), Some("bun"));
        assert_eq!(config.args, vec!["server.js"]);
        assert_eq!(config.env.get("DEBUG"), Some(&"true".to_string()));
    }

    #[test]
    fn test_mcp_servers_config_serialization() {
        let mut servers = HashMap::new();
        servers.insert(
            "my-server".to_string(),
            McpServerConfig {
                command: Some("cargo".to_string()),
                args: vec!["run".to_string()],
                env: HashMap::new(),
                server_type: None,
                url: None,
                headers: HashMap::new(),
                extra: HashMap::new(),
            },
        );

        let config = McpServersConfig { mcp_servers: servers };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"mcpServers\""));
        assert!(json.contains("\"my-server\""));
        assert!(json.contains("\"command\":\"cargo\""));
    }

    #[test]
    fn test_mcp_servers_config_deserialization() {
        let json = r#"{
            "mcpServers": {
                "server1": {
                    "command": "npm",
                    "args": ["start"],
                    "env": {}
                },
                "server2": {
                    "command": "yarn",
                    "args": ["dev"],
                    "env": {
                        "NODE_ENV": "development"
                    }
                }
            }
        }"#;

        let config: McpServersConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.mcp_servers.len(), 2);
        assert!(config.mcp_servers.contains_key("server1"));
        assert!(config.mcp_servers.contains_key("server2"));

        let server1 = config.mcp_servers.get("server1").unwrap();
        assert_eq!(server1.command.as_deref(), Some("npm"));

        let server2 = config.mcp_servers.get("server2").unwrap();
        assert_eq!(server2.env.get("NODE_ENV"), Some(&"development".to_string()));
    }

    #[test]
    fn test_optional_fields() {
        // Test that args and env are optional
        let json = r#"{
            "command": "simple-server"
        }"#;

        let config: McpServerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.command.as_deref(), Some("simple-server"));
        assert!(config.args.is_empty());
        assert!(config.env.is_empty());
    }
}
