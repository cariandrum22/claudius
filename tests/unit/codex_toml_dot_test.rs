#[cfg(test)]
mod tests {
    use claudius::codex_settings::{convert_mcp_to_toml, CodexSettings};
    use claudius::config::McpServerConfig;
    use std::collections::HashMap;
    use toml::Value as TomlValue;

    fn empty_codex_settings() -> CodexSettings {
        CodexSettings {
            model: None,
            review_model: None,
            model_provider: None,
            model_context_window: None,
            approval_policy: None,
            disable_response_storage: None,
            notify: None,
            model_providers: None,
            shell_environment_policy: None,
            sandbox_mode: None,
            sandbox_workspace_write: None,
            sandbox: None,
            history: None,
            mcp_servers: None,
            extra: HashMap::new(),
        }
    }

    fn make_mcp_server(command: &str, args: &[&str]) -> McpServerConfig {
        McpServerConfig {
            command: Some(command.to_string()),
            args: args.iter().map(|arg| (*arg).to_string()).collect(),
            env: HashMap::new(),
            server_type: None,
            url: None,
            headers: HashMap::new(),
            extra: HashMap::new(),
        }
    }

    fn serialize_codex_settings(settings: &CodexSettings) -> String {
        toml::to_string_pretty(settings).expect("CodexSettings should serialize to TOML")
    }

    fn parse_codex_settings(toml_str: &str) -> CodexSettings {
        toml::from_str(toml_str).expect("CodexSettings TOML should parse")
    }

    fn assert_server_name_present(toml_output: &str, name: &str) {
        assert!(
            toml_output.contains(name) || toml_output.contains(&format!("\"{name}\"")),
            "Server name '{name}' should be in TOML output",
        );
    }

    #[test]
    fn test_mcp_server_name_with_dots_in_toml() {
        let mut mcp_servers = HashMap::new();
        mcp_servers.insert(
            "awslabs.aws-documentation-mcp-server".to_string(),
            make_mcp_server("npx", &["-y", "@awslabs/mcp-server-aws-docs"]),
        );
        mcp_servers.insert("simple-server".to_string(), make_mcp_server("node", &["server.js"]));

        let mut settings = empty_codex_settings();
        settings.model = Some("claude-3".to_string());
        settings.mcp_servers = Some(convert_mcp_to_toml(&mcp_servers));

        let toml_str = serialize_codex_settings(&settings);
        assert!(toml_str.contains("\"awslabs.aws-documentation-mcp-server\""));

        let parsed_settings = parse_codex_settings(&toml_str);
        let parsed_servers = parsed_settings.mcp_servers.expect("mcp_servers should be present");

        assert_eq!(parsed_servers.len(), 2);
        assert!(parsed_servers.contains_key("awslabs.aws-documentation-mcp-server"));
        assert!(parsed_servers.contains_key("simple-server"));
    }

    #[test]
    fn test_quoted_key_preservation_in_toml() {
        let test_names = [
            "org.example.server",
            "com.github.mcp-server",
            "awslabs.aws-documentation-mcp-server",
            "server.with.many.dots",
        ];

        let mut mcp_servers = HashMap::new();
        for &name in &test_names {
            mcp_servers.insert(name.to_string(), make_mcp_server("test", &[]));
        }

        let toml_servers = convert_mcp_to_toml(&mcp_servers);
        let mut root = toml::map::Map::new();
        root.insert(
            "mcp_servers".to_string(),
            TomlValue::Table(toml_servers.into_iter().collect()),
        );

        let toml_output = toml::to_string_pretty(&root).expect("Should serialize successfully");
        for &name in &test_names {
            assert_server_name_present(&toml_output, name);
        }

        let _: toml::Value = toml::from_str(&toml_output).expect("Generated TOML should be valid");
    }

    #[test]
    fn test_mcp_server_rename_with_underscores() {
        let mut mcp_servers = HashMap::new();
        mcp_servers.insert(
            "awslabs_aws-documentation-mcp-server".to_string(),
            make_mcp_server("npx", &["-y", "@awslabs/mcp-server-aws-docs"]),
        );

        let mut settings = empty_codex_settings();
        settings.mcp_servers = Some(convert_mcp_to_toml(&mcp_servers));

        let toml_str = serialize_codex_settings(&settings);
        assert!(toml_str.contains("awslabs_aws-documentation-mcp-server"));

        let parsed_settings = parse_codex_settings(&toml_str);
        let parsed_servers = parsed_settings.mcp_servers.expect("mcp_servers should be present");
        assert!(parsed_servers.contains_key("awslabs_aws-documentation-mcp-server"));
    }
}
