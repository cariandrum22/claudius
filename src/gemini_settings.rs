use crate::config::{ClaudeConfig, McpServerConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::hash::BuildHasher;

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
    pub billing: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub advanced: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_policy_paths: Option<Value>,

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
    pub policy_paths: Option<Value>,

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
    "adminPolicyPaths",
    "advanced",
    "billing",
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
    "policyPaths",
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

const GEMINI_MCP_TRUST_FIELD: &str = "trust";
const LEGACY_AUTO_APPROVE_FIELD: &str = "autoApprove";
const LEGACY_DISABLED_FIELD: &str = "disabled";

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

/// Validate shared MCP server definitions against Gemini's supported schema.
#[must_use]
pub fn validate_gemini_mcp_server_configs(
    mcp_servers: &HashMap<String, McpServerConfig, impl BuildHasher>,
) -> Vec<String> {
    let (sanitized_mcp_servers, mut warnings) = sanitize_gemini_mcp_servers(mcp_servers);

    warnings.extend(
        sanitized_mcp_servers
            .iter()
            .filter_map(|(server_name, server)| {
                server
                    .server_type
                    .as_ref()
                    .filter(|kind| !matches!(kind.as_str(), "stdio" | "sse" | "http"))
                    .map(|kind| {
                        format!(
                            "Unknown mcpServers.{server_name}.type value '{kind}' (expected: stdio|sse|http)"
                        )
                    })
            }),
    );

    warnings
}

/// Sanitize a merged Gemini JSON config before writing it to Gemini-native paths.
#[must_use]
pub fn sanitize_claude_config_for_gemini(config: &ClaudeConfig) -> (ClaudeConfig, Vec<String>) {
    let mut sanitized_config = config.clone();
    let warnings = config.mcp_servers.as_ref().map_or_else(Vec::new, |mcp_servers| {
        let (sanitized_servers, warnings) = sanitize_gemini_mcp_servers(mcp_servers);
        sanitized_config.mcp_servers = Some(sanitized_servers);
        warnings
    });

    (sanitized_config, warnings)
}

/// Sanitize MCP server definitions for Gemini output.
#[must_use]
pub fn sanitize_gemini_mcp_servers(
    mcp_servers: &HashMap<String, McpServerConfig, impl BuildHasher>,
) -> (HashMap<String, McpServerConfig>, Vec<String>) {
    mcp_servers.iter().fold(
        (HashMap::new(), Vec::new()),
        |(mut sanitized_servers, mut warnings), (server_name, server)| {
            let (sanitized_server, server_warnings) =
                sanitize_gemini_mcp_server(server_name, server);
            sanitized_servers.insert(server_name.clone(), sanitized_server);
            warnings.extend(server_warnings);
            (sanitized_servers, warnings)
        },
    )
}

fn sanitize_gemini_mcp_server(
    server_name: &str,
    server: &McpServerConfig,
) -> (McpServerConfig, Vec<String>) {
    let mut sanitized_server = server.clone();
    let mut warnings = Vec::new();
    let original_extra = std::mem::take(&mut sanitized_server.extra);
    let trust_already_present = original_extra.contains_key(GEMINI_MCP_TRUST_FIELD);

    sanitized_server.extra = original_extra.into_iter().fold(
        HashMap::new(),
        |mut sanitized_extra, (key, value)| {
            match key.as_str() {
                LEGACY_AUTO_APPROVE_FIELD => {
                    warnings.push(render_auto_approve_warning(
                        server_name,
                        trust_already_present,
                        &value,
                    ));

                    if !trust_already_present && value.is_boolean() {
                        sanitized_extra.insert(GEMINI_MCP_TRUST_FIELD.to_string(), value);
                    }
                },
                LEGACY_DISABLED_FIELD => warnings.push(format!(
                    "mcpServers.{server_name}.disabled is not supported by Gemini settings.json; manage enablement with `gemini mcp enable|disable` and `~/.gemini/mcp-server-enablement.json`, so this field will be dropped"
                )),
                _ if KNOWN_GEMINI_MCP_SERVER_FIELDS.contains(&key.as_str()) => {
                    sanitized_extra.insert(key, value);
                },
                _ => warnings.push(format!(
                    "mcpServers.{server_name}.{key} is not supported by Gemini settings.json and will be dropped during Gemini sync"
                )),
            }

            sanitized_extra
        },
    );

    (sanitized_server, warnings)
}

fn render_auto_approve_warning(
    server_name: &str,
    trust_already_present: bool,
    value: &Value,
) -> String {
    if trust_already_present {
        return format!(
            "mcpServers.{server_name}.autoApprove is not supported by Gemini settings.json; `trust` is already set, so autoApprove will be dropped"
        );
    }

    if value.is_boolean() {
        return format!(
            "mcpServers.{server_name}.autoApprove is not supported by Gemini settings.json; Gemini sync will translate it to `trust`"
        );
    }

    format!(
        "mcpServers.{server_name}.autoApprove is not supported by Gemini settings.json; only boolean values can be translated to `trust`, so this field will be dropped"
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    fn server_with_extra(extra: HashMap<String, Value>) -> McpServerConfig {
        McpServerConfig {
            command: Some("uvx".to_string()),
            args: vec!["mcp-server".to_string()],
            env: HashMap::new(),
            server_type: None,
            url: None,
            headers: HashMap::new(),
            extra,
        }
    }

    #[test]
    fn test_sanitize_gemini_mcp_servers_translates_autoapprove_and_drops_unsupported_fields() {
        let servers = HashMap::from([(
            "aws-docs".to_string(),
            server_with_extra(HashMap::from([
                (LEGACY_AUTO_APPROVE_FIELD.to_string(), Value::Bool(true)),
                (LEGACY_DISABLED_FIELD.to_string(), Value::Bool(true)),
                ("unsupported".to_string(), Value::String("x".to_string())),
                ("cwd".to_string(), Value::String("/tmp".to_string())),
            ])),
        )]);

        let (sanitized, warnings) = sanitize_gemini_mcp_servers(&servers);
        let server = sanitized.get("aws-docs").expect("sanitized server should exist");

        assert_eq!(server.extra.get(GEMINI_MCP_TRUST_FIELD), Some(&Value::Bool(true)));
        assert!(!server.extra.contains_key(LEGACY_AUTO_APPROVE_FIELD));
        assert!(!server.extra.contains_key(LEGACY_DISABLED_FIELD));
        assert!(!server.extra.contains_key("unsupported"));
        assert_eq!(server.extra.get("cwd"), Some(&Value::String("/tmp".to_string())));

        assert!(warnings.iter().any(|warning| warning.contains("translate it to `trust`")));
        assert!(warnings.iter().any(|warning| warning.contains(".disabled")));
        assert!(warnings.iter().any(|warning| warning.contains(".unsupported")));
    }

    #[test]
    fn test_sanitize_gemini_mcp_servers_keeps_existing_trust() {
        let servers = HashMap::from([(
            "aws-docs".to_string(),
            server_with_extra(HashMap::from([
                (LEGACY_AUTO_APPROVE_FIELD.to_string(), Value::Bool(true)),
                (GEMINI_MCP_TRUST_FIELD.to_string(), Value::Bool(false)),
            ])),
        )]);

        let (sanitized, warnings) = sanitize_gemini_mcp_servers(&servers);
        let server = sanitized.get("aws-docs").expect("sanitized server should exist");

        assert_eq!(server.extra.get(GEMINI_MCP_TRUST_FIELD), Some(&Value::Bool(false)));
        assert!(!server.extra.contains_key(LEGACY_AUTO_APPROVE_FIELD));
        assert!(warnings.iter().any(|warning| warning.contains("`trust` is already set")));
    }
}
