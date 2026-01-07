use claudius::codex_settings::{convert_mcp_to_toml, CodexSettings};
use claudius::config::McpServerConfig;
use std::collections::HashMap;
use toml::Value as TomlValue;

#[test]
fn test_mcp_server_name_with_dots_in_toml() {
    let mut mcp_servers = HashMap::new();

    // Server name with dots like "awslabs.aws-documentation-mcp-server"
    mcp_servers.insert(
        "awslabs.aws-documentation-mcp-server".to_string(),
        McpServerConfig {
            command: Some("npx".to_string()),
            args: vec!["-y".to_string(), "@awslabs/mcp-server-aws-docs".to_string()],
            env: HashMap::new(),
            server_type: None,
            url: None,
            headers: HashMap::new(),
            extra: HashMap::new(),
        },
    );

    // Server name without dots for comparison
    mcp_servers.insert(
        "simple-server".to_string(),
        McpServerConfig {
            command: Some("node".to_string()),
            args: vec!["server.js".to_string()],
            env: HashMap::new(),
            server_type: None,
            url: None,
            headers: HashMap::new(),
            extra: HashMap::new(),
        },
    );

    let toml_servers = convert_mcp_to_toml(&mcp_servers);

    // Create CodexSettings with the MCP servers
    let settings = CodexSettings {
        model: Some("claude-3".to_string()),
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
        mcp_servers: Some(toml_servers),
        extra: HashMap::new(),
    };

    // Try to serialize to TOML string
    let toml_result = toml::to_string_pretty(&settings);

    // This will fail if server names with dots cannot be serialized properly
    assert!(toml_result.is_ok(), "Should be able to serialize server names with dots");

    let toml_str = toml_result.unwrap();

    // The TOML should contain quoted keys for names with dots
    assert!(toml_str.contains("mcp_servers"), "Should contain mcp_servers section");

    // Parse back the TOML to verify it's valid
    let parsed: Result<CodexSettings, _> = toml::from_str(&toml_str);
    assert!(parsed.is_ok(), "Should be able to parse the generated TOML");

    let parsed_settings = parsed.unwrap();
    assert!(parsed_settings.mcp_servers.is_some());

    let parsed_servers = parsed_settings.mcp_servers.unwrap();

    // Both servers should be present
    assert_eq!(parsed_servers.len(), 2);
    assert!(parsed_servers.contains_key("awslabs.aws-documentation-mcp-server"));
    assert!(parsed_servers.contains_key("simple-server"));
}

#[test]
fn test_quoted_key_preservation_in_toml() {
    let mut mcp_servers = HashMap::new();

    // Multiple server names with dots to test various patterns
    let test_names = vec![
        "org.example.server",
        "com.github.mcp-server",
        "awslabs.aws-documentation-mcp-server",
        "server.with.many.dots",
    ];

    for name in &test_names {
        mcp_servers.insert(
            name.to_string(),
            McpServerConfig {
                command: Some("test".to_string()),
                args: vec![],
                env: HashMap::new(),
                server_type: None,
                url: None,
                headers: HashMap::new(),
                extra: HashMap::new(),
            },
        );
    }

    let toml_servers = convert_mcp_to_toml(&mcp_servers);

    // Manually create a TOML table to test serialization
    let mut root = toml::map::Map::new();
    root.insert(
        "mcp_servers".to_string(),
        TomlValue::Table(toml_servers.into_iter().map(|(k, v)| (k, v)).collect()),
    );

    let toml_str = toml::to_string_pretty(&root);
    assert!(toml_str.is_ok(), "Should serialize successfully");

    let toml_output = toml_str.unwrap();

    // Verify all server names are preserved
    for name in &test_names {
        // The name should appear in the TOML, either quoted or unquoted
        assert!(
            toml_output.contains(name) || toml_output.contains(&format!("\"{}\"", name)),
            "Server name '{}' should be in TOML output",
            name
        );
    }

    // Parse back to ensure validity
    let parsed: Result<toml::Value, _> = toml::from_str(&toml_output);
    assert!(parsed.is_ok(), "Generated TOML should be valid");
}

#[test]
fn test_mcp_server_rename_with_underscores() {
    let mut mcp_servers = HashMap::new();

    // Renamed version with underscores
    mcp_servers.insert(
        "awslabs_aws-documentation-mcp-server".to_string(),
        McpServerConfig {
            command: Some("npx".to_string()),
            args: vec!["-y".to_string(), "@awslabs/mcp-server-aws-docs".to_string()],
            env: HashMap::new(),
            server_type: None,
            url: None,
            headers: HashMap::new(),
            extra: HashMap::new(),
        },
    );

    let toml_servers = convert_mcp_to_toml(&mcp_servers);

    let settings = CodexSettings {
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
        mcp_servers: Some(toml_servers),
        extra: HashMap::new(),
    };

    let toml_result = toml::to_string_pretty(&settings);
    assert!(toml_result.is_ok(), "Should serialize successfully with underscores");

    let toml_str = toml_result.unwrap();
    assert!(toml_str.contains("awslabs_aws-documentation-mcp-server"));

    // Parse back
    let parsed: Result<CodexSettings, _> = toml::from_str(&toml_str);
    assert!(parsed.is_ok(), "Should parse back successfully");

    let parsed_settings = parsed.unwrap();
    let parsed_servers = parsed_settings.mcp_servers.unwrap();
    assert!(parsed_servers.contains_key("awslabs_aws-documentation-mcp-server"));
}
