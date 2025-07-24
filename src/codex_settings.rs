use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use toml::Value as TomlValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodexSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_response_storage: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_providers: Option<HashMap<String, ModelProvider>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_environment_policy: Option<ShellEnvironmentPolicy>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<HistoryConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<HashMap<String, TomlValue>>,

    // Catch-all for unknown fields to preserve them
    #[serde(flatten)]
    pub extra: HashMap<String, TomlValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelProvider {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,

    // Catch-all for unknown fields to preserve them (e.g., name, etc.)
    #[serde(flatten)]
    pub extra: HashMap<String, TomlValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShellEnvironmentPolicy {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inherit: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_default_excludes: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub set: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_only: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub writable_roots: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_access: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistence: Option<String>,
}

// Known field names for validation
pub const KNOWN_CODEX_FIELDS: &[&str] = &[
    "model",
    "model_provider",
    "approval_policy",
    "disable_response_storage",
    "notify",
    "model_providers",
    "shell_environment_policy",
    "sandbox",
    "history",
    "mcp_servers",
];

// We no longer validate model provider fields since they can have arbitrary extra fields
// pub const KNOWN_MODEL_PROVIDER_FIELDS: &[&str] = &["base_url", "api_key_env", "headers"];

pub const KNOWN_SHELL_ENV_FIELDS: &[&str] =
    &["inherit", "ignore_default_excludes", "exclude", "set", "include_only"];

pub const KNOWN_SANDBOX_FIELDS: &[&str] = &["mode", "writable_roots", "network_access"];

pub const KNOWN_HISTORY_FIELDS: &[&str] = &["persistence"];

/// Validates Codex TOML settings and returns warnings for unknown fields
#[must_use]
pub fn validate_codex_settings(toml_value: &TomlValue) -> Vec<String> {
    let mut warnings = Vec::new();

    if let TomlValue::Table(table) = toml_value {
        for (key, value) in table {
            if !KNOWN_CODEX_FIELDS.contains(&key.as_str()) {
                warnings.push(format!("Unknown setting '{key}' found in Codex configuration"));
            }

            validate_nested_codex_field(key.as_str(), value, &mut warnings);
        }
    }

    warnings
}

/// Validate nested fields based on the parent field name
fn validate_nested_codex_field(parent_key: &str, value: &TomlValue, warnings: &mut Vec<String>) {
    let validation_config = match parent_key {
        // Skip validation for model_providers since they can have arbitrary extra fields
        "model_providers" => return,
        "shell_environment_policy" => {
            Some((KNOWN_SHELL_ENV_FIELDS, "shell_environment_policy", false))
        },
        "sandbox" => Some((KNOWN_SANDBOX_FIELDS, "sandbox", false)),
        "history" => Some((KNOWN_HISTORY_FIELDS, "history", false)),
        _ => None,
    };

    if let Some((known_fields, field_name, is_nested)) = validation_config {
        if is_nested {
            validate_nested_providers(value, known_fields, field_name, warnings);
        } else {
            validate_simple_table(value, known_fields, field_name, warnings);
        }
    }
}

/// Validate model providers which have an additional nesting level
fn validate_nested_providers(
    value: &TomlValue,
    known_fields: &[&str],
    field_name: &str,
    warnings: &mut Vec<String>,
) {
    let TomlValue::Table(providers) = value else {
        return;
    };

    for (provider_name, provider_value) in providers {
        let TomlValue::Table(provider_table) = provider_value else {
            continue;
        };

        for (field, _) in provider_table {
            if !known_fields.contains(&field.as_str()) {
                warnings.push(format!("Unknown field '{field}' in {field_name}.{provider_name}"));
            }
        }
    }
}

/// Validate simple table fields
fn validate_simple_table(
    value: &TomlValue,
    known_fields: &[&str],
    field_name: &str,
    warnings: &mut Vec<String>,
) {
    let TomlValue::Table(table) = value else {
        return;
    };

    for (field, _) in table {
        if !known_fields.contains(&field.as_str()) {
            warnings.push(format!("Unknown field '{field}' in {field_name}"));
        }
    }
}

/// Convert MCP server configuration from JSON to TOML format
pub fn convert_mcp_to_toml<S: std::hash::BuildHasher>(
    mcp_servers: &HashMap<String, crate::config::McpServerConfig, S>,
) -> HashMap<String, TomlValue> {
    let mut toml_servers = HashMap::new();

    for (name, server) in mcp_servers {
        let mut server_table = toml::map::Map::new();

        server_table.insert("command".to_string(), TomlValue::String(server.command.clone()));

        if !server.args.is_empty() {
            let args: Vec<TomlValue> =
                server.args.iter().map(|arg| TomlValue::String(arg.clone())).collect();
            server_table.insert("args".to_string(), TomlValue::Array(args));
        }

        if !server.env.is_empty() {
            let mut env_table = toml::map::Map::new();
            for (k, v) in &server.env {
                env_table.insert(k.clone(), TomlValue::String(v.clone()));
            }
            server_table.insert("env".to_string(), TomlValue::Table(env_table));
        }

        toml_servers.insert(name.clone(), TomlValue::Table(server_table));
    }

    toml_servers
}
