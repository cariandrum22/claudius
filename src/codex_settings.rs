use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use toml::Value as TomlValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodexSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_model: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_context_window: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<String>,

    // Legacy field (not present in the latest Codex CLI config reference).
    // Kept for backwards compatibility with older Codex configs and existing tests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_response_storage: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_providers: Option<HashMap<String, ModelProvider>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_environment_policy: Option<ShellEnvironmentPolicy>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_mode: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_workspace_write: Option<SandboxWorkspaceWrite>,

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
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", alias = "api_key_env")]
    pub env_key: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none", alias = "headers")]
    pub http_headers: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_http_headers: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_params: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_api: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_openai_auth: Option<bool>,

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
pub struct SandboxWorkspaceWrite {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writable_roots: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_access: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_tmpdir_env_var: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_slash_tmp: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistence: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<usize>,
}

// Known field names for validation
pub const KNOWN_CODEX_FIELDS: &[&str] = &[
    "model",
    "review_model",
    "model_provider",
    "model_context_window",
    "approval_policy",
    "notify",
    "model_providers",
    "shell_environment_policy",
    "sandbox_mode",
    "sandbox_workspace_write",
    "sandbox",
    "history",
    "mcp_servers",
    // Newer Codex CLI fields (non-exhaustive; used only for warning suppression)
    "check_for_update_on_startup",
    "instructions",
    "developer_instructions",
    "features",
    "profile",
    "profiles",
    "projects",
    "project_root_markers",
    "project_doc_max_bytes",
    "project_doc_fallback_filenames",
    "tool_output_token_limit",
    "tui",
    "hide_agent_reasoning",
    "show_raw_agent_reasoning",
    "file_opener",
    "cli_auth_credentials_store",
    "forced_chatgpt_workspace_id",
    "forced_login_method",
    "chatgpt_base_url",
    "otel",
    "oss_provider",
    // Legacy / compatibility fields
    "disable_response_storage",
];

// We no longer validate model provider fields since they can have arbitrary extra fields
// pub const KNOWN_MODEL_PROVIDER_FIELDS: &[&str] = &["name", "base_url", "env_key", "http_headers"];

pub const KNOWN_SHELL_ENV_FIELDS: &[&str] = &[
    "inherit",
    "ignore_default_excludes",
    "exclude",
    "set",
    "include_only",
    "experimental_use_profile",
];

pub const KNOWN_SANDBOX_FIELDS: &[&str] = &["mode", "writable_roots", "network_access"];

pub const KNOWN_SANDBOX_WORKSPACE_WRITE_FIELDS: &[&str] =
    &["writable_roots", "network_access", "exclude_tmpdir_env_var", "exclude_slash_tmp"];

pub const KNOWN_HISTORY_FIELDS: &[&str] = &["persistence", "max_bytes"];

const CODEX_MCP_STDIO_UNSUPPORTED_FIELDS: &[&str] =
    &["url", "bearer_token_env_var", "http_headers", "env_http_headers"];

const CODEX_MCP_STREAMABLE_HTTP_UNSUPPORTED_FIELDS: &[&str] = &["command", "args", "env", "cwd"];

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
        "sandbox_workspace_write" => {
            Some((KNOWN_SANDBOX_WORKSPACE_WRITE_FIELDS, "sandbox_workspace_write", false))
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

fn json_to_toml_value(value: &JsonValue) -> Option<TomlValue> {
    match value {
        JsonValue::Null => None,
        JsonValue::Bool(b) => Some(TomlValue::Boolean(*b)),
        JsonValue::Number(number) => number
            .as_i64()
            .map(TomlValue::Integer)
            .or_else(|| number.as_u64().and_then(|n| i64::try_from(n).ok()).map(TomlValue::Integer))
            .or_else(|| number.as_f64().map(TomlValue::Float)),
        JsonValue::String(s) => Some(TomlValue::String(s.clone())),
        JsonValue::Array(array) => {
            Some(TomlValue::Array(array.iter().filter_map(json_to_toml_value).collect()))
        },
        JsonValue::Object(json_object) => Some(TomlValue::Table(
            json_object
                .iter()
                .filter_map(|(k, v)| json_to_toml_value(v).map(|tv| (k.clone(), tv)))
                .collect(),
        )),
    }
}

fn extend_toml_table_with_json_extra(
    table: &mut toml::map::Map<String, TomlValue>,
    extra: &HashMap<String, JsonValue>,
    unsupported_fields: &[&str],
) {
    extra
        .iter()
        .filter(|(key, _)| !unsupported_fields.contains(&key.as_str()))
        .filter_map(|(key, value)| json_to_toml_value(value).map(|tv| (key.clone(), tv)))
        .for_each(|(key, value)| {
            table.insert(key, value);
        });
}

/// Convert MCP server configuration from JSON to TOML format
pub fn convert_mcp_to_toml<S: std::hash::BuildHasher>(
    mcp_servers: &HashMap<String, crate::config::McpServerConfig, S>,
) -> HashMap<String, TomlValue> {
    let mut toml_servers = HashMap::new();

    for (name, server) in mcp_servers {
        let mut server_table = toml::map::Map::new();

        if let Some(url) = server.url.as_ref() {
            server_table.insert("url".to_string(), TomlValue::String(url.clone()));

            if !server.headers.is_empty() {
                let mut headers_table = toml::map::Map::new();
                for (k, v) in &server.headers {
                    headers_table.insert(k.clone(), TomlValue::String(v.clone()));
                }
                server_table.insert("http_headers".to_string(), TomlValue::Table(headers_table));
            }

            extend_toml_table_with_json_extra(
                &mut server_table,
                &server.extra,
                CODEX_MCP_STREAMABLE_HTTP_UNSUPPORTED_FIELDS,
            );
        } else if let Some(command) = server.command.as_ref() {
            server_table.insert("command".to_string(), TomlValue::String(command.clone()));

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

            extend_toml_table_with_json_extra(
                &mut server_table,
                &server.extra,
                CODEX_MCP_STDIO_UNSUPPORTED_FIELDS,
            );
        } else {
            continue;
        }

        toml_servers.insert(name.clone(), TomlValue::Table(server_table));
    }

    toml_servers
}
