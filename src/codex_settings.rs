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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental_use_profile: Option<bool>,

    #[serde(flatten)]
    pub extra: HashMap<String, TomlValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SandboxConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub writable_roots: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_access: Option<bool>,

    #[serde(flatten)]
    pub extra: HashMap<String, TomlValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SandboxWorkspaceWrite {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writable_roots: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_access: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_tmpdir_env_var: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_slash_tmp: Option<bool>,

    #[serde(flatten)]
    pub extra: HashMap<String, TomlValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistence: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<usize>,

    #[serde(flatten)]
    pub extra: HashMap<String, TomlValue>,
}

const CODEX_MCP_STDIO_UNSUPPORTED_FIELDS: &[&str] =
    &["url", "bearer_token_env_var", "http_headers", "env_http_headers"];

const CODEX_MCP_STREAMABLE_HTTP_UNSUPPORTED_FIELDS: &[&str] = &["command", "args", "env", "cwd"];

/// Validates Codex TOML settings for known compatibility concerns.
///
/// Unknown fields are preserved without warnings so newer Codex settings remain forward-compatible.
#[must_use]
pub fn validate_codex_settings(toml_value: &TomlValue) -> Vec<String> {
    let mut warnings = Vec::new();

    if let TomlValue::Table(table) = toml_value {
        for (key, value) in table {
            validate_deprecated_codex_field(key.as_str(), value, &mut warnings);
        }
    }

    warnings
}

fn validate_deprecated_codex_field(
    parent_key: &str,
    value: &TomlValue,
    warnings: &mut Vec<String>,
) {
    match parent_key {
        "approval_policy" => {
            if matches!(value, TomlValue::String(policy) if policy == "on-failure") {
                warnings.push(
                    "approval_policy = \"on-failure\" is deprecated; use \"on-request\" or \"never\""
                        .to_string(),
                );
            }
        },
        "background_terminal_timeout" => warnings.push(
            "background_terminal_timeout is deprecated; rename it to background_terminal_max_timeout"
                .to_string(),
        ),
        "disable_response_storage" => warnings.push(
            "disable_response_storage is a legacy Codex setting and no longer appears in the current config reference"
                .to_string(),
        ),
        "experimental_instructions_file" => warnings.push(
            "experimental_instructions_file is deprecated; rename it to model_instructions_file"
                .to_string(),
        ),
        "experimental_use_unified_exec_tool" => warnings.push(
            "experimental_use_unified_exec_tool is a legacy flag; prefer features.unified_exec"
                .to_string(),
        ),
        "instructions" => warnings.push(
            "instructions is reserved for future use; prefer model_instructions_file or AGENTS.md"
                .to_string(),
        ),
        "features" => validate_deprecated_codex_feature_flags(value, warnings),
        "shell_environment_policy" => validate_shell_environment_policy(value, warnings),
        _ => {},
    }
}

fn validate_shell_environment_policy(value: &TomlValue, warnings: &mut Vec<String>) {
    let TomlValue::Table(policy) = value else {
        return;
    };

    for legacy_field in ["exclude", "include_only"] {
        if policy.contains_key(legacy_field) {
            warnings.push(format!(
                "shell_environment_policy.{legacy_field} is legacy; prefer shell_environment_policy.filters"
            ));
        }
    }

    if policy.contains_key("filters")
        && (policy.contains_key("exclude") || policy.contains_key("include_only"))
    {
        warnings.push(
            "shell_environment_policy.filters cannot be combined with legacy exclude or include_only in the same configuration layer"
                .to_string(),
        );
    }
}

/// Validates Codex `requirements.toml` for known compatibility concerns.
#[must_use]
pub fn validate_codex_requirements(toml_value: &TomlValue) -> Vec<String> {
    let Some(policies) = toml_value.get("allowed_approval_policies").and_then(TomlValue::as_array)
    else {
        return Vec::new();
    };

    policies
        .iter()
        .any(|policy| policy.as_str() == Some("on-failure"))
        .then(|| {
            "allowed_approval_policies contains deprecated \"on-failure\"; remove it or use \"on-request\""
                .to_string()
        })
        .into_iter()
        .collect()
}

fn validate_deprecated_codex_feature_flags(value: &TomlValue, warnings: &mut Vec<String>) {
    let TomlValue::Table(features) = value else {
        return;
    };

    [
        ("web_search", "web_search"),
        ("web_search_cached", "web_search"),
        ("web_search_request", "web_search"),
    ]
    .into_iter()
    .filter(|(field, _)| features.contains_key(*field))
    .for_each(|(field, replacement)| {
        warnings.push(format!(
            "features.{field} is deprecated; prefer the top-level {replacement} setting"
        ));
    });
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
