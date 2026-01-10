use std::collections::HashMap;
use serde_json::{Map, Value};

pub fn create_test_claude_config() -> serde_json::Value {
    let mut root = Map::new();

    // Create mcpServers
    let mut mcp_servers = Map::new();

    let mut filesystem = Map::new();
    filesystem.insert("command".to_string(), Value::String("old-fs-command".to_string()));
    filesystem.insert("args".to_string(), Value::Array(vec![Value::String("--old".to_string())]));
    let mut fs_env = Map::new();
    fs_env.insert("OLD_VAR".to_string(), Value::String("old_value".to_string()));
    filesystem.insert("env".to_string(), Value::Object(fs_env));
    mcp_servers.insert("filesystem".to_string(), Value::Object(filesystem));

    let mut existing = Map::new();
    existing.insert("command".to_string(), Value::String("existing-cmd".to_string()));
    existing.insert("args".to_string(), Value::Array(vec![]));
    existing.insert("env".to_string(), Value::Object(Map::new()));
    mcp_servers.insert("existing".to_string(), Value::Object(existing));

    root.insert("mcpServers".to_string(), Value::Object(mcp_servers));

    // Add other fields
    root.insert("apiKeyHelper".to_string(), Value::String("/old/helper".to_string()));
    root.insert("cleanupPeriodDays".to_string(), Value::Number(serde_json::Number::from(10)));

    let mut env = Map::new();
    env.insert("OLD_ENV".to_string(), Value::String("old_value".to_string()));
    root.insert("env".to_string(), Value::Object(env));

    root.insert("includeCoAuthoredBy".to_string(), Value::Bool(false));

    let mut permissions = Map::new();
    permissions.insert("allow".to_string(), Value::Array(vec![Value::String("Read".to_string())]));
    permissions.insert("deny".to_string(), Value::Array(vec![]));
    permissions.insert("defaultMode".to_string(), Value::String("allow".to_string()));
    root.insert("permissions".to_string(), Value::Object(permissions));

    root.insert("preferredNotifChannel".to_string(), Value::String("email".to_string()));
    root.insert("unknownField".to_string(), Value::String("should_be_preserved".to_string()));

    let mut custom_settings = Map::new();
    custom_settings.insert("nested".to_string(), Value::String("value".to_string()));
    root.insert("customSettings".to_string(), Value::Object(custom_settings));

    Value::Object(root)
}

pub fn create_test_mcp_servers() -> serde_json::Value {
    let mut root = Map::new();
    let mut mcp_servers = Map::new();

    // Filesystem server
    let mut filesystem = Map::new();
    filesystem.insert("command".to_string(), Value::String("new-fs-command".to_string()));
    filesystem.insert("args".to_string(), Value::Array(vec![
        Value::String("--new".to_string()),
        Value::String("--updated".to_string())
    ]));
    let mut fs_env = Map::new();
    fs_env.insert("NEW_VAR".to_string(), Value::String("new_value".to_string()));
    filesystem.insert("env".to_string(), Value::Object(fs_env));
    mcp_servers.insert("filesystem".to_string(), Value::Object(filesystem));

    // GitHub server
    let mut github = Map::new();
    github.insert("command".to_string(), Value::String("gh-cmd".to_string()));
    github.insert("args".to_string(), Value::Array(vec![Value::String("-y".to_string())]));
    let mut gh_env = Map::new();
    gh_env.insert("GITHUB_TOKEN".to_string(), Value::String("$TOKEN".to_string()));
    github.insert("env".to_string(), Value::Object(gh_env));
    mcp_servers.insert("github".to_string(), Value::Object(github));

    root.insert("mcpServers".to_string(), Value::Object(mcp_servers));
    Value::Object(root)
}

#[allow(dead_code)]
pub fn create_test_settings() -> serde_json::Value {
    let mut root = Map::new();

    root.insert("apiKeyHelper".to_string(), Value::String("/new/helper".to_string()));
    root.insert("cleanupPeriodDays".to_string(), Value::Number(serde_json::Number::from(30)));

    let mut env = Map::new();
    env.insert("NEW_ENV".to_string(), Value::String("new_value".to_string()));
    env.insert("PATH".to_string(), Value::String("/custom/path".to_string()));
    root.insert("env".to_string(), Value::Object(env));

    root.insert("includeCoAuthoredBy".to_string(), Value::Bool(true));

    let mut permissions = Map::new();
    permissions.insert("allow".to_string(), Value::Array(vec![
        Value::String("Read".to_string()),
        Value::String("Write".to_string())
    ]));
    permissions.insert("deny".to_string(), Value::Array(vec![Value::String("Execute".to_string())]));
    permissions.insert("defaultMode".to_string(), Value::String("deny".to_string()));
    root.insert("permissions".to_string(), Value::Object(permissions));

    root.insert("preferredNotifChannel".to_string(), Value::String("slack".to_string()));

    Value::Object(root)
}

pub fn create_test_codex_config() -> String {
    r#"model = "openai/gpt-4"
model_provider = "openai"
approval_policy = "none"
custom_field = "should_be_preserved"

	[model_providers.openai]
	base_url = "https://api.openai.com"
	env_key = "OPENAI_API_KEY"

	sandbox_mode = "workspace-write"

	[sandbox_workspace_write]
	network_access = true

[mcp_servers.filesystem]
command = "old-fs-command"
args = ["--old"]

[mcp_servers.existing]
command = "existing-cmd"
args = []

[extra_section]
key = "value"
"#.to_string()
}

#[allow(dead_code)]
pub fn create_new_codex_mcp_servers() -> HashMap<String, toml::Value> {
    let mut servers = HashMap::new();

    // Conflicting filesystem server
    let mut fs_table = toml::map::Map::new();
    fs_table.insert("command".to_string(), toml::Value::String("new-fs-command".to_string()));
    fs_table.insert("args".to_string(), toml::Value::Array(vec![
        toml::Value::String("--new".to_string()),
        toml::Value::String("--updated".to_string()),
    ]));
    servers.insert("filesystem".to_string(), toml::Value::Table(fs_table));

    // New github server
    let mut gh_table = toml::map::Map::new();
    gh_table.insert("command".to_string(), toml::Value::String("gh-cmd".to_string()));
    gh_table.insert("args".to_string(), toml::Value::Array(vec![
        toml::Value::String("-y".to_string()),
    ]));
    servers.insert("github".to_string(), toml::Value::Table(gh_table));

    servers
}
