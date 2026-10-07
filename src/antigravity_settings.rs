//! Antigravity CLI (`agy`) support.
//!
//! Antigravity keeps MCP servers in their own `mcp_config.json`
//! (`~/.gemini/config/mcp_config.json` globally, `.agents/mcp_config.json` per
//! workspace) and CLI preferences in `~/.gemini/antigravity-cli/settings.json`.
//! Claudius keeps the shared `mcpServers.json` format as its source of truth and
//! converts each entry into the Antigravity shape at write time. Existing target
//! content that Claudius does not manage (servers added with `agy mcp add`,
//! unknown keys) is preserved.

use crate::config::McpServerConfig;
use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::hash::BuildHasher;
use std::path::Path;

/// Source file name for Antigravity CLI settings in the Claudius config directory.
pub const ANTIGRAVITY_SETTINGS_SOURCE_FILE: &str = "antigravity.settings.json";

/// File name Antigravity reads MCP servers from.
pub const ANTIGRAVITY_MCP_CONFIG_FILE: &str = "mcp_config.json";

/// File name of the Antigravity CLI settings inside its CLI directory.
pub const ANTIGRAVITY_CLI_SETTINGS_FILE: &str = "settings.json";

const MCP_SERVERS_KEY: &str = "mcpServers";

/// Documented Antigravity MCP server fields that pass through unchanged.
const PASSTHROUGH_MCP_SERVER_FIELDS: &[&str] =
    &["cwd", "disabledTools", "authProviderType", "oauth"];

/// MCP server fields that only apply to stdio servers.
const LOCAL_ONLY_MCP_SERVER_FIELDS: &[&str] = &["cwd"];

/// Read a JSON object file, returning `None` when it does not exist.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be read, is not valid JSON
/// (Antigravity also accepts comments, which Claudius cannot round-trip), or its
/// top level is not an object.
pub fn read_json_object(path: &Path) -> Result<Option<Map<String, Value>>> {
    if !path.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    match serde_json::from_str::<Value>(content.trim_start_matches('\u{feff}')).with_context(
        || {
            format!(
                "Failed to parse {} as JSON; Claudius does not rewrite files with comments or trailing commas",
                path.display()
            )
        },
    )? {
        Value::Object(map) => Ok(Some(map)),
        _ => Err(anyhow::anyhow!("{} must contain a JSON object", path.display())),
    }
}

/// Return informational warnings for the Antigravity settings source.
#[must_use]
pub fn validate_antigravity_settings(json: &Value) -> Vec<String> {
    let Value::Object(map) = json else {
        return vec!["Antigravity settings must be a JSON object".to_string()];
    };

    map.contains_key(MCP_SERVERS_KEY)
        .then(|| {
            "Antigravity setting 'mcpServers' is not read from settings.json; define MCP servers in mcpServers.json so Claudius writes them to mcp_config.json".to_string()
        })
        .into_iter()
        .collect()
}

/// Return warnings for shared MCP server definitions that do not translate
/// cleanly into Antigravity.
#[must_use]
pub fn validate_antigravity_mcp_server_configs<S: BuildHasher>(
    mcp_servers: &HashMap<String, McpServerConfig, S>,
) -> Vec<String> {
    convert_mcp_servers(mcp_servers).1
}

/// Convert shared MCP server definitions into Antigravity server objects.
///
/// Returns the converted servers (sorted by name) and conversion warnings.
#[must_use]
pub fn convert_mcp_servers<S: BuildHasher>(
    mcp_servers: &HashMap<String, McpServerConfig, S>,
) -> (BTreeMap<String, Value>, Vec<String>) {
    let sorted: BTreeMap<&String, &McpServerConfig> = mcp_servers.iter().collect();
    sorted.into_iter().fold(
        (BTreeMap::new(), Vec::new()),
        |(mut servers, mut warnings), (name, server)| {
            let (converted, server_warnings) = convert_mcp_server(name, server);
            if let Some(value) = converted {
                servers.insert(name.clone(), value);
            }
            warnings.extend(server_warnings);
            (servers, warnings)
        },
    )
}

/// Convert a single shared MCP server definition into an Antigravity server.
///
/// Returns `None` when the definition has no `command`, `url`, or `serverUrl`.
#[must_use]
pub fn convert_mcp_server(name: &str, server: &McpServerConfig) -> (Option<Value>, Vec<String>) {
    let mut warnings = Vec::new();
    let server_url = server
        .url
        .clone()
        .or_else(|| server.extra.get("serverUrl").and_then(Value::as_str).map(str::to_string));

    let mut output = match (&server.command, server_url) {
        (Some(command), url) => {
            if url.is_some() {
                warnings.push(format!(
                    "mcpServers.{name} defines both `command` and `url`; Antigravity uses the stdio `command` and ignores `url`"
                ));
            }
            stdio_server(command, server)
        },
        (None, Some(url)) => remote_server(url, server),
        (None, None) => {
            warnings.push(format!(
                "mcpServers.{name} has neither `command` nor `url` and will be skipped for Antigravity"
            ));
            return (None, warnings);
        },
    };

    let is_local = server.command.is_some();
    let sorted_extra: BTreeMap<&String, &Value> =
        server.extra.iter().filter(|(key, _)| key.as_str() != "serverUrl").collect();
    for (key, value) in sorted_extra {
        apply_extra_field(name, key, value, is_local, &mut output, &mut warnings);
    }

    (Some(Value::Object(output)), warnings)
}

fn stdio_server(command: &str, server: &McpServerConfig) -> Map<String, Value> {
    let mut output = Map::new();
    output.insert("command".to_string(), Value::String(command.to_string()));
    if !server.args.is_empty() {
        output.insert(
            "args".to_string(),
            Value::Array(server.args.iter().cloned().map(Value::String).collect()),
        );
    }
    if !server.env.is_empty() {
        output.insert("env".to_string(), string_map_value(&server.env));
    }
    output
}

fn remote_server(url: String, server: &McpServerConfig) -> Map<String, Value> {
    let mut output = Map::new();
    output.insert("serverUrl".to_string(), Value::String(url));
    if !server.headers.is_empty() {
        output.insert("headers".to_string(), string_map_value(&server.headers));
    }
    output
}

fn string_map_value<S: BuildHasher>(values: &HashMap<String, String, S>) -> Value {
    let sorted: BTreeMap<&String, &String> = values.iter().collect();
    Value::Object(
        sorted
            .into_iter()
            .map(|(key, value)| (key.clone(), Value::String(value.clone())))
            .collect(),
    )
}

fn apply_extra_field(
    name: &str,
    key: &str,
    value: &Value,
    is_local: bool,
    output: &mut Map<String, Value>,
    warnings: &mut Vec<String>,
) {
    match key {
        "disabled" => match value.as_bool() {
            Some(disabled) => {
                output.insert("disabled".to_string(), Value::Bool(disabled));
            },
            None => warnings.push(format!(
                "mcpServers.{name}.disabled must be a boolean and will be dropped for Antigravity"
            )),
        },
        "enabled" => match value.as_bool() {
            Some(enabled) => {
                output.entry("disabled".to_string()).or_insert(Value::Bool(!enabled));
            },
            None => warnings.push(format!(
                "mcpServers.{name}.enabled must be a boolean and will be dropped for Antigravity"
            )),
        },
        _ if PASSTHROUGH_MCP_SERVER_FIELDS.contains(&key)
            && (is_local || !LOCAL_ONLY_MCP_SERVER_FIELDS.contains(&key)) =>
        {
            output.insert(key.to_string(), value.clone());
        },
        _ => warnings.push(format!(
            "mcpServers.{name}.{key} is not supported by Antigravity and will be dropped during Antigravity sync"
        )),
    }
}

/// Render the Antigravity `mcp_config.json` document.
///
/// Converted servers replace same-named entries under `mcpServers`; servers
/// and top-level keys already present in `existing` are preserved.
///
/// # Errors
///
/// Returns an error if the existing `mcpServers` value is not a JSON object,
/// since overwriting it would discard user data.
pub fn render_mcp_config<S: BuildHasher>(
    existing: Option<Map<String, Value>>,
    mcp_servers: &HashMap<String, McpServerConfig, S>,
) -> Result<(Value, Vec<String>)> {
    let mut document = existing.unwrap_or_default();
    let mut servers = match document.remove(MCP_SERVERS_KEY) {
        None => Map::new(),
        Some(Value::Object(map)) => map,
        Some(_) => anyhow::bail!(
            "Existing Antigravity `mcpServers` setting is not an object; refusing to overwrite it"
        ),
    };

    let (converted, warnings) = convert_mcp_servers(mcp_servers);
    servers.extend(converted);
    let sorted_servers: BTreeMap<String, Value> = servers.into_iter().collect();
    document
        .insert(MCP_SERVERS_KEY.to_string(), Value::Object(sorted_servers.into_iter().collect()));

    let sorted_document: BTreeMap<String, Value> = document.into_iter().collect();
    Ok((Value::Object(sorted_document.into_iter().collect()), warnings))
}

/// Deep-merge the Antigravity settings source into the existing CLI settings.
#[must_use]
pub fn render_cli_settings(
    existing: Option<Map<String, Value>>,
    source: &Map<String, Value>,
) -> Value {
    let mut merged: HashMap<String, Value> = existing.unwrap_or_default().into_iter().collect();
    let overlay: HashMap<String, Value> =
        source.iter().map(|(key, value)| (key.clone(), value.clone())).collect();
    crate::json_merge::deep_merge_json_maps(&mut merged, &overlay);

    let sorted: BTreeMap<String, Value> = merged.into_iter().collect();
    Value::Object(sorted.into_iter().collect())
}

/// Serialize an Antigravity document with a trailing newline.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn to_pretty_json(document: &Value) -> Result<String> {
    Ok(format!("{}\n", serde_json::to_string_pretty(document)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn server(value: Value) -> McpServerConfig {
        serde_json::from_value(value).expect("test server should deserialize")
    }

    #[test]
    fn converts_stdio_server_with_env_and_cwd() {
        let (converted, warnings) = convert_mcp_server(
            "fs",
            &server(json!({
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-filesystem"],
                "env": {"B": "2", "A": "1"},
                "cwd": "/tmp"
            })),
        );

        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            converted,
            Some(json!({
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-filesystem"],
                "env": {"A": "1", "B": "2"},
                "cwd": "/tmp"
            }))
        );
    }

    #[test]
    fn converts_url_server_to_server_url() {
        let (converted, warnings) = convert_mcp_server(
            "remote",
            &server(json!({
                "type": "http",
                "url": "https://mcp.example.com/mcp",
                "headers": {"Authorization": "Bearer token"},
                "oauth": {"clientId": "abc"},
                "disabledTools": ["delete_all"]
            })),
        );

        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            converted,
            Some(json!({
                "serverUrl": "https://mcp.example.com/mcp",
                "headers": {"Authorization": "Bearer token"},
                "oauth": {"clientId": "abc"},
                "disabledTools": ["delete_all"]
            }))
        );
    }

    #[test]
    fn accepts_native_server_url_field() {
        let (converted, warnings) =
            convert_mcp_server("remote", &server(json!({"serverUrl": "https://mcp.example.com"})));

        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(converted, Some(json!({"serverUrl": "https://mcp.example.com"})));
    }

    #[test]
    fn maps_enabled_false_and_drops_unsupported_fields() {
        let (converted, warnings) = convert_mcp_server(
            "x",
            &server(json!({
                "command": "srv",
                "enabled": false,
                "trust": true,
                "startup_timeout_sec": 300
            })),
        );

        assert_eq!(converted, Some(json!({"command": "srv", "disabled": true})));
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("mcpServers.x.trust")));
        assert!(warnings.iter().any(|w| w.contains("mcpServers.x.startup_timeout_sec")));
    }

    #[test]
    fn drops_cwd_for_remote_servers() {
        let (converted, warnings) = convert_mcp_server(
            "remote",
            &server(json!({"url": "https://mcp.example.com", "cwd": "/tmp"})),
        );

        assert_eq!(converted, Some(json!({"serverUrl": "https://mcp.example.com"})));
        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }

    #[test]
    fn skips_server_without_transport() {
        let (converted, warnings) = convert_mcp_server("broken", &server(json!({"args": ["a"]})));
        assert!(converted.is_none());
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn render_mcp_config_preserves_unmanaged_servers_and_keys() {
        let existing = json!({
            "mcpServers": {
                "fs": {"command": "old"},
                "manual": {"serverUrl": "https://manual.example.com", "timeoutSeconds": 30}
            },
            "futureKey": true
        });
        let sources =
            HashMap::from([("fs".to_string(), server(json!({"command": "new", "args": ["--x"]})))]);

        let (document, warnings) =
            render_mcp_config(existing.as_object().cloned(), &sources).expect("render succeeds");

        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            document,
            json!({
                "futureKey": true,
                "mcpServers": {
                    "fs": {"command": "new", "args": ["--x"]},
                    "manual": {"serverUrl": "https://manual.example.com", "timeoutSeconds": 30}
                }
            })
        );
    }

    #[test]
    fn render_mcp_config_rejects_non_object_servers() {
        let existing = json!({"mcpServers": []});
        assert!(render_mcp_config(existing.as_object().cloned(), &HashMap::new()).is_err());
    }

    #[test]
    fn render_cli_settings_deep_merges_and_keeps_existing_keys() {
        let existing = json!({"colorScheme": "dark", "permissions": {"allow": ["read_file(*)"]}});
        let source = json!({"permissions": {"deny": ["command(sudo)"]}, "verbosity": "medium"});

        let merged = render_cli_settings(
            existing.as_object().cloned(),
            source.as_object().expect("source is an object"),
        );

        assert_eq!(
            merged,
            json!({
                "colorScheme": "dark",
                "permissions": {"allow": ["read_file(*)"], "deny": ["command(sudo)"]},
                "verbosity": "medium"
            })
        );
    }

    #[test]
    fn validate_settings_warns_about_mcp_servers_key() {
        assert_eq!(validate_antigravity_settings(&json!({"mcpServers": {}})).len(), 1);
        assert!(validate_antigravity_settings(&json!({"verbosity": "medium"})).is_empty());
        assert_eq!(validate_antigravity_settings(&json!([])).len(), 1);
    }
}
