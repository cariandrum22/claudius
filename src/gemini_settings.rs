use crate::config::McpServerConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GeminiSettings {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<HashMap<String, McpServerConfig>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub general: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub privacy: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub telemetry: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_configs: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub advanced: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ide: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_write_todos: Option<Value>,

    // Catch-all for unknown fields to preserve them
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// Known field names for validation (Gemini CLI v2+ settings schema).
pub const KNOWN_GEMINI_FIELDS: &[&str] = &[
    "$schema",
    "admin",
    "advanced",
    "context",
    "experimental",
    "extensions",
    "general",
    "hooks",
    "ide",
    "mcp",
    "mcpServers",
    "model",
    "modelConfigs",
    "output",
    "privacy",
    "security",
    "skills",
    "telemetry",
    "tools",
    "ui",
    "useWriteTodos",
];

pub const KNOWN_GEMINI_MCP_SERVER_FIELDS: &[&str] = &[
    "command",
    "args",
    "env",
    "cwd",
    "url",
    "httpUrl",
    "headers",
    "tcp",
    "type",
    "timeout",
    "trust",
    "description",
    "includeTools",
    "excludeTools",
    "extension",
    "oauth",
    "authProviderType",
    "targetAudience",
    "targetServiceAccount",
];

pub const KNOWN_GEMINI_TELEMETRY_FIELDS: &[&str] = &[
    "enabled",
    "target",
    "otlpEndpoint",
    "otlpProtocol",
    "logPrompts",
    "outfile",
    "useCollector",
    "useCliAuth",
];

/// Validates a JSON value and returns warnings for unknown fields
#[must_use]
pub fn validate_gemini_settings(json: &Value) -> Vec<String> {
    let mut warnings = Vec::new();

    if let Value::Object(map) = json {
        for (key, value) in map {
            if !KNOWN_GEMINI_FIELDS.contains(&key.as_str()) {
                warnings.push(format!("Unknown setting '{key}' found in Gemini configuration"));
            }

            validate_nested_object(key.as_str(), value, &mut warnings);
        }
    }

    warnings
}

/// Validate nested objects based on the parent field name
fn validate_nested_object(parent_key: &str, value: &Value, warnings: &mut Vec<String>) {
    match parent_key {
        "mcpServers" => validate_gemini_mcp_servers(value, warnings),
        "telemetry" => validate_known_object_fields(
            value,
            KNOWN_GEMINI_TELEMETRY_FIELDS,
            "telemetry",
            warnings,
        ),
        _ => {},
    }
}

fn validate_gemini_mcp_servers(value: &Value, warnings: &mut Vec<String>) {
    let Value::Object(servers) = value else {
        return;
    };

    for (server_name, server_value) in servers {
        let Value::Object(server_map) = server_value else {
            continue;
        };

        for (server_key, server_field_value) in server_map {
            if !KNOWN_GEMINI_MCP_SERVER_FIELDS.contains(&server_key.as_str()) {
                warnings.push(format!("Unknown field '{server_key}' in mcpServers.{server_name}"));
                continue;
            }

            if server_key == "type" {
                let Value::String(kind) = server_field_value else {
                    continue;
                };

                if matches!(kind.as_str(), "stdio" | "sse" | "http") {
                    continue;
                }

                warnings.push(format!(
                    "Unknown mcpServers.{server_name}.type value '{kind}' (expected: stdio|sse|http)"
                ));
            }
        }
    }
}

fn validate_known_object_fields(
    value: &Value,
    known_fields: &[&str],
    field_name: &str,
    warnings: &mut Vec<String>,
) {
    let Value::Object(map) = value else {
        return;
    };

    for (key, _) in map {
        if !known_fields.contains(&key.as_str()) {
            warnings.push(format!("Unknown field '{key}' in {field_name}"));
        }
    }
}
