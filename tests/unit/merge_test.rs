use claudius::{
    config::{ClaudeConfig, McpServerConfig, McpServersConfig},
    merge::{merge_configs, MergeStrategy},
};
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_server(command: &str) -> McpServerConfig {
        McpServerConfig {
            command: Some(command.to_string()),
            args: vec![],
            env: HashMap::new(),
            server_type: None,
            url: None,
            headers: HashMap::new(),
            extra: HashMap::new(),
        }
    }

    fn create_claude_config_with_servers(servers: Vec<(&str, &str)>) -> ClaudeConfig {
        let mut mcp_servers = HashMap::new();
        for (name, command) in servers {
            mcp_servers.insert(name.to_string(), create_test_server(command));
        }

        ClaudeConfig {
            mcp_servers: Some(mcp_servers),
            other: HashMap::from([("theme".to_string(), serde_json::json!("dark"))]),
        }
    }

    fn create_mcp_servers_config(servers: Vec<(&str, &str)>) -> McpServersConfig {
        let mut mcp_servers = HashMap::new();
        for (name, command) in servers {
            mcp_servers.insert(name.to_string(), create_test_server(command));
        }

        McpServersConfig { mcp_servers }
    }

    #[test]
    fn test_merge_replace_strategy() {
        let mut claude_config = create_claude_config_with_servers(vec![
            ("server1", "old-command1"),
            ("server2", "old-command2"),
        ]);

        let new_servers = create_mcp_servers_config(vec![
            ("server3", "new-command3"),
            ("server4", "new-command4"),
        ]);

        merge_configs(&mut claude_config, &new_servers, MergeStrategy::Replace).unwrap();

        // Old servers should be gone
        let servers = claude_config.mcp_servers.as_ref().unwrap();
        assert_eq!(servers.len(), 2);
        assert!(!servers.contains_key("server1"));
        assert!(!servers.contains_key("server2"));

        // New servers should be present
        assert!(servers.contains_key("server3"));
        assert!(servers.contains_key("server4"));
        assert_eq!(servers.get("server3").and_then(|s| s.command.as_deref()), Some("new-command3"));
        assert_eq!(servers.get("server4").and_then(|s| s.command.as_deref()), Some("new-command4"));

        // Other config should be preserved
        assert_eq!(claude_config.other.get("theme"), Some(&serde_json::json!("dark")));
    }

    #[test]
    fn test_merge_strategy() {
        let mut claude_config = create_claude_config_with_servers(vec![
            ("server1", "old-command1"),
            ("server2", "old-command2"),
        ]);

        let new_servers = create_mcp_servers_config(vec![
            ("server2", "new-command2"), // Override existing
            ("server3", "new-command3"), // Add new
        ]);

        merge_configs(&mut claude_config, &new_servers, MergeStrategy::Merge).unwrap();

        let servers = claude_config.mcp_servers.as_ref().unwrap();
        assert_eq!(servers.len(), 3);

        // server1 should remain unchanged
        assert_eq!(servers.get("server1").and_then(|s| s.command.as_deref()), Some("old-command1"));

        // server2 should be updated
        assert_eq!(servers.get("server2").and_then(|s| s.command.as_deref()), Some("new-command2"));

        // server3 should be added
        assert_eq!(servers.get("server3").and_then(|s| s.command.as_deref()), Some("new-command3"));
    }

    #[test]
    fn test_merge_preserve_existing_strategy() {
        let mut claude_config = create_claude_config_with_servers(vec![
            ("server1", "old-command1"),
            ("server2", "old-command2"),
        ]);

        let new_servers = create_mcp_servers_config(vec![
            ("server2", "new-command2"), // Should NOT override
            ("server3", "new-command3"), // Should add
        ]);

        merge_configs(&mut claude_config, &new_servers, MergeStrategy::MergePreserveExisting)
            .unwrap();

        let servers = claude_config.mcp_servers.as_ref().unwrap();
        assert_eq!(servers.len(), 3);

        // server1 should remain unchanged
        assert_eq!(servers.get("server1").and_then(|s| s.command.as_deref()), Some("old-command1"));

        // server2 should NOT be updated (preserve existing)
        assert_eq!(servers.get("server2").and_then(|s| s.command.as_deref()), Some("old-command2"));

        // server3 should be added
        assert_eq!(servers.get("server3").and_then(|s| s.command.as_deref()), Some("new-command3"));
    }

    #[test]
    fn test_merge_with_empty_claude_config() {
        let mut claude_config = ClaudeConfig { mcp_servers: None, other: HashMap::new() };

        let new_servers =
            create_mcp_servers_config(vec![("server1", "command1"), ("server2", "command2")]);

        merge_configs(&mut claude_config, &new_servers, MergeStrategy::Merge).unwrap();

        assert!(claude_config.mcp_servers.is_some());
        let servers = claude_config.mcp_servers.as_ref().unwrap();
        assert_eq!(servers.len(), 2);
        assert_eq!(servers.get("server1").and_then(|s| s.command.as_deref()), Some("command1"));
        assert_eq!(servers.get("server2").and_then(|s| s.command.as_deref()), Some("command2"));
    }

    #[test]
    fn test_merge_with_complex_server_config() {
        let mut claude_config = ClaudeConfig {
            mcp_servers: None,
            other: HashMap::from([
                ("theme".to_string(), serde_json::json!("light")),
                ("fontSize".to_string(), serde_json::json!(14)),
            ]),
        };

        let mut servers = HashMap::new();
        servers.insert(
            "complex-server".to_string(),
            McpServerConfig {
                command: Some("node".to_string()),
                args: vec!["--experimental".to_string(), "server.js".to_string()],
                env: HashMap::from([
                    ("NODE_ENV".to_string(), "production".to_string()),
                    ("PORT".to_string(), "3000".to_string()),
                ]),
                server_type: None,
                url: None,
                headers: HashMap::new(),
                extra: HashMap::new(),
            },
        );

        let new_servers = McpServersConfig { mcp_servers: servers };

        merge_configs(&mut claude_config, &new_servers, MergeStrategy::Merge).unwrap();

        // Check that server was added correctly
        let merged_servers = claude_config.mcp_servers.as_ref().unwrap();
        let server = merged_servers.get("complex-server").unwrap();
        assert_eq!(server.command.as_deref(), Some("node"));
        assert_eq!(server.args, vec!["--experimental", "server.js"]);
        assert_eq!(server.env.get("NODE_ENV"), Some(&"production".to_string()));
        assert_eq!(server.env.get("PORT"), Some(&"3000".to_string()));

        // Check that other config is preserved
        assert_eq!(claude_config.other.get("theme"), Some(&serde_json::json!("light")));
        assert_eq!(claude_config.other.get("fontSize"), Some(&serde_json::json!(14)));
    }

    #[test]
    fn test_merge_empty_new_servers() {
        let mut claude_config = create_claude_config_with_servers(vec![("server1", "command1")]);

        let new_servers = McpServersConfig { mcp_servers: HashMap::new() };

        merge_configs(&mut claude_config, &new_servers, MergeStrategy::Merge).unwrap();

        // Original servers should remain
        let servers = claude_config.mcp_servers.as_ref().unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers.get("server1").and_then(|s| s.command.as_deref()), Some("command1"));
    }
}
