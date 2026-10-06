//! `OpenCode` v2 (`opencode.json`) support.
//!
//! `OpenCode` v2 stores MCP servers under `mcp.servers.<name>` using its own
//! schema (`type: "local"` with a `command` array, or `type: "remote"` with a
//! `url`). Claudius keeps the shared `mcpServers.json` format as its source of
//! truth and converts each entry into the v2-native shape at write time.

use crate::config::{ClaudeConfig, McpServerConfig};
use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::hash::BuildHasher;
use std::path::Path;

/// Source file name for `OpenCode` settings in the Claudius config directory.
pub const OPENCODE_SETTINGS_SOURCE_FILE: &str = "opencode.settings.json";

/// Target file name `OpenCode` reads in project roots and its config directory.
pub const OPENCODE_CONFIG_FILE: &str = "opencode.json";

const MCP_KEY: &str = "mcp";
const MCP_SERVERS_KEY: &str = "servers";

/// Extra MCP server fields that pass through unchanged into the v2 schema.
const PASSTHROUGH_MCP_SERVER_FIELDS: &[&str] = &["cwd", "codemode", "protocol"];

/// MCP server fields that only apply to local (stdio) servers.
const LOCAL_ONLY_MCP_SERVER_FIELDS: &[&str] = &["cwd"];

/// Shared MCP server field holding an `OpenCode` timeout (number or object).
const TIMEOUT_FIELD: &str = "timeout";

/// Codex per-server timeouts in seconds and the `OpenCode` v2 timeout phase
/// (milliseconds) each one maps to.
const SECONDS_TIMEOUT_FIELDS: &[(&str, &str)] =
    &[("startup_timeout_sec", "startup"), ("tool_timeout_sec", "execution")];

/// Largest millisecond value `OpenCode` can read as an exact integer
/// (`Number.MAX_SAFE_INTEGER`).
const MAX_TIMEOUT_MILLIS: u64 = 9_007_199_254_740_991;

/// V1 `OAuth` keys and their v2 snake-case equivalents.
const OAUTH_KEY_RENAMES: &[(&str, &str)] = &[
    ("clientId", "client_id"),
    ("clientSecret", "client_secret"),
    ("callbackPort", "callback_port"),
    ("redirectUri", "redirect_uri"),
    ("authServerMetadataUrl", "auth_server_metadata_url"),
];

/// Top-level V1 keys that `OpenCode` v2 still normalizes, with their native v2
/// successor.
pub const LEGACY_OPENCODE_KEYS: &[(&str, &str)] = &[
    ("agent", "agents"),
    ("mode", "agents"),
    ("command", "commands"),
    ("permission", "permissions"),
    ("tools", "permissions"),
    ("provider", "providers"),
    ("plugin", "plugins"),
    ("reference", "references"),
    ("autoshare", "share"),
    ("snapshot", "snapshots"),
    ("attachment", "media"),
    ("autoupdate", "update"),
];

/// Read the `OpenCode` settings source as a raw JSON object.
///
/// The settings are kept untyped because `OpenCode` v2 uses shapes (for example
/// `permissions` as a rule array) that do not fit Claude's `Settings` model.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be read, is not valid JSON,
/// or its top level is not an object.
pub fn read_opencode_settings(path: &Path) -> Result<Option<Map<String, Value>>> {
    if !path.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    match serde_json::from_str::<Value>(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?
    {
        Value::Object(map) => Ok(Some(map)),
        _ => Err(anyhow::anyhow!("{} must contain a JSON object", path.display())),
    }
}

/// Return informational warnings for `OpenCode` settings that use V1 keys.
#[must_use]
pub fn validate_opencode_settings(json: &Value) -> Vec<String> {
    let Value::Object(map) = json else {
        return vec!["OpenCode settings must be a JSON object".to_string()];
    };

    let legacy_key_warnings = LEGACY_OPENCODE_KEYS.iter().filter(|(legacy, _)| map.contains_key(*legacy)).map(
        |(legacy, native)| {
            format!(
                "OpenCode setting '{legacy}' is a V1 key; OpenCode v2 still migrates it in memory, but the native V2 key is '{native}'"
            )
        },
    );

    let skills_warning = map.get("skills").filter(|value| value.is_object()).map(|_| {
        "OpenCode setting 'skills' uses the V1 {paths, urls} object; OpenCode v2 expects a flat array of paths and URLs".to_string()
    });

    let mcp_warnings = map
        .get(MCP_KEY)
        .and_then(Value::as_object)
        .map(|mcp| {
            mcp.iter()
                .filter(|(name, value)| *name != MCP_SERVERS_KEY && is_direct_legacy_mcp(value))
                .map(|(name, _)| {
                    format!(
                        "OpenCode setting 'mcp.{name}' uses the V1 flat MCP layout; OpenCode v2 expects 'mcp.servers.{name}'"
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    legacy_key_warnings.chain(skills_warning).chain(mcp_warnings).collect()
}

/// Return warnings for shared MCP server definitions that do not translate
/// cleanly into `OpenCode` v2.
#[must_use]
pub fn validate_opencode_mcp_server_configs<S: BuildHasher>(
    mcp_servers: &HashMap<String, McpServerConfig, S>,
) -> Vec<String> {
    convert_mcp_servers(mcp_servers).1
}

/// Convert shared MCP server definitions into `OpenCode` v2 server objects.
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

/// Convert a single shared MCP server definition into an `OpenCode` v2 server.
///
/// Returns `None` when the definition has neither a `command` nor a `url`.
#[must_use]
pub fn convert_mcp_server(name: &str, server: &McpServerConfig) -> (Option<Value>, Vec<String>) {
    let mut warnings = Vec::new();

    let mut output = match (&server.command, &server.url) {
        (Some(command), url) => {
            if url.is_some() {
                warnings.push(format!(
                    "mcpServers.{name} defines both `command` and `url`; OpenCode uses the local `command` and ignores `url`"
                ));
            }
            local_server(command, server)
        },
        (None, Some(url)) => remote_server(url, server),
        (None, None) => {
            warnings.push(format!(
                "mcpServers.{name} has neither `command` nor `url` and will be skipped for OpenCode"
            ));
            return (None, warnings);
        },
    };

    let is_local = server.command.is_some();
    let sorted_extra: BTreeMap<&String, &Value> =
        server.extra.iter().filter(|(key, _)| !is_timeout_field(key)).collect();
    for (key, value) in sorted_extra {
        apply_extra_field(name, key, value, is_local, &mut output, &mut warnings);
    }
    if let Some(timeout) = convert_timeouts(name, &server.extra, &mut warnings) {
        output.insert(TIMEOUT_FIELD.to_string(), timeout);
    }

    (Some(Value::Object(output)), warnings)
}

fn local_server(command: &str, server: &McpServerConfig) -> Map<String, Value> {
    let command_line = std::iter::once(command.to_string())
        .chain(server.args.iter().cloned())
        .map(Value::String)
        .collect();

    let mut output = Map::new();
    output.insert("type".to_string(), Value::String("local".to_string()));
    output.insert("command".to_string(), Value::Array(command_line));
    if !server.env.is_empty() {
        output.insert("environment".to_string(), string_map_value(&server.env));
    }
    output
}

fn remote_server(url: &str, server: &McpServerConfig) -> Map<String, Value> {
    let mut output = Map::new();
    output.insert("type".to_string(), Value::String("remote".to_string()));
    output.insert("url".to_string(), Value::String(url.to_string()));
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
                "mcpServers.{name}.disabled must be a boolean and will be dropped for OpenCode"
            )),
        },
        "enabled" => match value.as_bool() {
            Some(enabled) => {
                output.entry("disabled".to_string()).or_insert(Value::Bool(!enabled));
            },
            None => warnings.push(format!(
                "mcpServers.{name}.enabled must be a boolean and will be dropped for OpenCode"
            )),
        },
        "oauth" if !is_local => match convert_oauth(value) {
            Some(oauth) => {
                output.insert("oauth".to_string(), oauth);
            },
            None => warnings.push(format!(
                "mcpServers.{name}.oauth must be `false` or an object and will be dropped for OpenCode"
            )),
        },
        _ if PASSTHROUGH_MCP_SERVER_FIELDS.contains(&key)
            && (is_local || !LOCAL_ONLY_MCP_SERVER_FIELDS.contains(&key)) =>
        {
            output.insert(key.to_string(), value.clone());
        },
        _ => warnings.push(format!(
            "mcpServers.{name}.{key} is not supported by OpenCode v2 and will be dropped during OpenCode sync"
        )),
    }
}

fn is_timeout_field(key: &str) -> bool {
    key == TIMEOUT_FIELD || SECONDS_TIMEOUT_FIELDS.iter().any(|(field, _)| *field == key)
}

/// Combine `timeout` with the Codex second-based timeout fields into a single
/// `OpenCode` v2 timeout object.
///
/// A phase set explicitly by `timeout` wins over the matching seconds field,
/// so the result does not depend on the order in which fields are read.
fn convert_timeouts<S: BuildHasher>(
    name: &str,
    extra: &HashMap<String, Value, S>,
    warnings: &mut Vec<String>,
) -> Option<Value> {
    let mut timeout = extra.get(TIMEOUT_FIELD).and_then(|value| {
        let converted = convert_timeout(value);
        if converted.is_none() {
            warnings.push(format!(
                "mcpServers.{name}.timeout must be a positive integer (milliseconds) or an object with startup/catalog/execution and will be dropped for OpenCode"
            ));
        }
        converted
    });

    for (field, phase) in SECONDS_TIMEOUT_FIELDS {
        let Some(value) = extra.get(*field) else {
            continue;
        };
        match seconds_to_millis(value) {
            None => warnings.push(format!(
                "mcpServers.{name}.{field} must be a positive number of seconds and will be dropped for OpenCode"
            )),
            Some(_) if timeout.as_ref().is_some_and(|phases| phases.contains_key(*phase)) => {
                warnings.push(format!(
                    "mcpServers.{name}.{field} is ignored for OpenCode because mcpServers.{name}.timeout already sets {phase}"
                ));
            },
            Some(millis) => {
                timeout.get_or_insert_with(Map::new).insert((*phase).to_string(), Value::from(millis));
            },
        }
    }

    timeout.map(Value::Object)
}

/// V1 used a single millisecond timeout; v2 splits it into phases. This mirrors
/// `OpenCode`'s own V1 migration, which maps the number to `catalog` and
/// `execution`.
fn convert_timeout(value: &Value) -> Option<Map<String, Value>> {
    match value {
        Value::Number(number) if number.as_u64().is_some_and(|ms| ms > 0) => {
            let mut timeout = Map::new();
            timeout.insert("catalog".to_string(), value.clone());
            timeout.insert("execution".to_string(), value.clone());
            Some(timeout)
        },
        Value::Object(map) => Some(map.clone()),
        _ => None,
    }
}

/// Convert a positive number of seconds into whole milliseconds, rejecting
/// values that round below one millisecond or exceed what `OpenCode` can read.
fn seconds_to_millis(value: &Value) -> Option<u64> {
    let Value::Number(number) = value else {
        return None;
    };
    let millis = number.as_u64().map_or_else(
        || decimal_seconds_to_millis(&number.to_string()),
        |seconds| seconds.checked_mul(1000),
    )?;
    (1..=MAX_TIMEOUT_MILLIS).contains(&millis).then_some(millis)
}

/// Round a non-integer number of seconds, as rendered by `serde_json` (for
/// example `1.5`, `-2.25` or `1.5e-7`), half up to whole milliseconds.
///
/// Working on the decimal digits avoids float rounding error and casts.
fn decimal_seconds_to_millis(text: &str) -> Option<u64> {
    if text.starts_with('-') {
        return None;
    }
    let (mantissa, exponent) = match text.split_once(['e', 'E']) {
        Some((base, power)) => (base, power.parse::<i64>().ok()?),
        None => (text, 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits: Vec<u32> = whole
        .chars()
        .chain(fraction.chars())
        .map(|digit| digit.to_digit(10))
        .collect::<Option<_>>()?;

    // Index of the decimal point within `digits` once seconds become milliseconds.
    let scaled_point = i64::try_from(whole.len()).ok()?.checked_add(exponent)?.checked_add(3)?;
    let Ok(point) = usize::try_from(scaled_point) else {
        return Some(0);
    };
    let millis = digits
        .iter()
        .copied()
        .chain(std::iter::repeat(0))
        .take(point)
        .try_fold(0_u64, |acc, digit| acc.checked_mul(10)?.checked_add(u64::from(digit)))?;
    let round_up = digits.get(point).is_some_and(|digit| *digit >= 5);
    millis.checked_add(u64::from(round_up))
}

fn convert_oauth(value: &Value) -> Option<Value> {
    match value {
        Value::Bool(false) => Some(Value::Bool(false)),
        Value::Object(map) => Some(Value::Object(
            map.iter()
                .map(|(key, field)| {
                    let renamed = OAUTH_KEY_RENAMES
                        .iter()
                        .find(|(legacy, _)| legacy == key)
                        .map_or_else(|| key.clone(), |(_, native)| (*native).to_string());
                    (renamed, field.clone())
                })
                .collect(),
        )),
        _ => None,
    }
}

fn is_direct_legacy_mcp(value: &Value) -> bool {
    value
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "local" || kind == "remote")
}

/// Render a merged config into the `OpenCode` v2 `opencode.json` document.
///
/// Shared MCP servers (`config.mcp_servers`) are converted and written under
/// `mcp.servers`, replacing same-named entries. Servers already present under
/// `mcp.servers` but absent from the source are preserved. A V1 flat entry
/// (`mcp.<name>`) with the same name as a synced server is removed so the
/// server is not defined twice.
///
/// # Errors
///
/// Returns an error if the existing `mcp` or `mcp.servers` value is not a JSON
/// object, since overwriting it would discard user data.
pub fn render_opencode_config(config: &ClaudeConfig) -> Result<(Value, Vec<String>)> {
    let mut document: BTreeMap<String, Value> =
        config.other.iter().map(|(key, value)| (key.clone(), value.clone())).collect();

    let Some(mcp_servers) = config.mcp_servers.as_ref() else {
        return Ok((Value::Object(document.into_iter().collect()), Vec::new()));
    };

    let (converted, warnings) = convert_mcp_servers(mcp_servers);
    let mcp = match document.remove(MCP_KEY) {
        None => Map::new(),
        Some(Value::Object(map)) => map,
        Some(_) => {
            return Err(anyhow::anyhow!(
                "Existing OpenCode `mcp` setting is not an object; refusing to overwrite it"
            ))
        },
    };

    let merged_mcp = merge_mcp_servers(mcp, converted)?;
    document.insert(MCP_KEY.to_string(), Value::Object(merged_mcp));

    Ok((Value::Object(document.into_iter().collect()), warnings))
}

fn merge_mcp_servers(
    mut mcp: Map<String, Value>,
    converted: BTreeMap<String, Value>,
) -> Result<Map<String, Value>> {
    let mut servers = match mcp.remove(MCP_SERVERS_KEY) {
        None => Map::new(),
        Some(Value::Object(map)) => map,
        Some(_) => {
            return Err(anyhow::anyhow!(
                "Existing OpenCode `mcp.servers` setting is not an object; refusing to overwrite it"
            ))
        },
    };

    for (name, server) in converted {
        if mcp.get(&name).is_some_and(is_direct_legacy_mcp) {
            mcp.remove(&name);
        }
        servers.insert(name, server);
    }

    let sorted_servers: BTreeMap<String, Value> = servers.into_iter().collect();
    mcp.insert(MCP_SERVERS_KEY.to_string(), Value::Object(sorted_servers.into_iter().collect()));
    Ok(mcp)
}

/// Merge `OpenCode` settings into the working config.
///
/// MCP servers defined in the settings source under `mcp.servers` are already
/// in the v2-native shape, so they are deep-merged as-is alongside all other
/// keys.
pub fn merge_opencode_settings_into_config(
    config: &mut ClaudeConfig,
    settings: &Map<String, Value>,
) {
    let overlay: HashMap<String, Value> =
        settings.iter().map(|(key, value)| (key.clone(), value.clone())).collect();
    crate::json_merge::deep_merge_json_maps(&mut config.other, &overlay);
}

/// Serialize an `OpenCode` document with a trailing newline.
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
    fn converts_stdio_server_to_local_command_array() {
        let (converted, warnings) = convert_mcp_server(
            "fs",
            &server(json!({
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
                "env": {"B": "2", "A": "1"}
            })),
        );

        assert!(warnings.is_empty());
        assert_eq!(
            converted,
            Some(json!({
                "type": "local",
                "command": ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
                "environment": {"A": "1", "B": "2"}
            }))
        );
    }

    #[test]
    fn converts_url_server_to_remote_with_snake_case_oauth() {
        let (converted, warnings) = convert_mcp_server(
            "remote",
            &server(json!({
                "type": "http",
                "url": "https://mcp.example.com/mcp",
                "headers": {"Authorization": "Bearer token"},
                "oauth": {"clientId": "abc", "scope": "read"},
                "timeout": 30000
            })),
        );

        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            converted,
            Some(json!({
                "type": "remote",
                "url": "https://mcp.example.com/mcp",
                "headers": {"Authorization": "Bearer token"},
                "oauth": {"client_id": "abc", "scope": "read"},
                "timeout": {"catalog": 30000, "execution": 30000}
            }))
        );
    }

    #[test]
    fn maps_enabled_false_to_disabled_and_drops_unknown_fields() {
        let (converted, warnings) = convert_mcp_server(
            "x",
            &server(json!({"command": "srv", "enabled": false, "autoApprove": ["a"]})),
        );

        assert_eq!(converted, Some(json!({"type": "local", "command": ["srv"], "disabled": true})));
        assert_eq!(warnings.len(), 1);
        assert!(warnings.first().is_some_and(|w| w.contains("mcpServers.x.autoApprove")));
    }

    #[test]
    fn maps_startup_timeout_sec_to_timeout_startup() {
        let (converted, warnings) = convert_mcp_server(
            "slow",
            &server(json!({"command": "srv", "startup_timeout_sec": 300})),
        );

        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            converted,
            Some(json!({"type": "local", "command": ["srv"], "timeout": {"startup": 300_000}}))
        );
    }

    #[test]
    fn combines_startup_timeout_sec_with_numeric_timeout() {
        let (converted, warnings) = convert_mcp_server(
            "slow",
            &server(json!({"command": "srv", "timeout": 30000, "startup_timeout_sec": 300})),
        );

        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            converted.as_ref().and_then(|value| value.get("timeout")),
            Some(&json!({"catalog": 30000, "execution": 30000, "startup": 300_000}))
        );
    }

    #[test]
    fn explicit_timeout_startup_wins_over_startup_timeout_sec() {
        let (converted, warnings) = convert_mcp_server(
            "slow",
            &server(json!({
                "command": "srv",
                "timeout": {"startup": 5000, "catalog": 1000},
                "startup_timeout_sec": 300
            })),
        );

        assert_eq!(
            converted.as_ref().and_then(|value| value.get("timeout")),
            Some(&json!({"startup": 5000, "catalog": 1000}))
        );
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings
            .first()
            .is_some_and(|w| w.contains("mcpServers.slow.startup_timeout_sec is ignored")));
    }

    #[test]
    fn rounds_fractional_startup_timeout_sec_to_whole_milliseconds() {
        for (seconds, millis) in [(json!(1.5), 1500), (json!(0.0015), 2)] {
            let (converted, warnings) = convert_mcp_server(
                "slow",
                &server(json!({"command": "srv", "startup_timeout_sec": seconds})),
            );

            assert!(warnings.is_empty(), "{warnings:?}");
            assert_eq!(
                converted.as_ref().and_then(|value| value.pointer("/timeout/startup")),
                Some(&json!(millis))
            );
        }
    }

    #[test]
    fn drops_invalid_startup_timeout_sec_with_warning() {
        for invalid in [
            json!("300"),
            json!(0),
            json!(-1),
            json!(-1.5),
            json!(0.0004),
            json!(1e-7),
            json!(1e20),
            json!(9_007_199_254_741_u64),
            json!(true),
        ] {
            let (converted, warnings) = convert_mcp_server(
                "slow",
                &server(json!({"command": "srv", "startup_timeout_sec": invalid})),
            );

            assert_eq!(converted, Some(json!({"type": "local", "command": ["srv"]})));
            assert_eq!(warnings.len(), 1, "{warnings:?}");
            assert!(warnings.first().is_some_and(|w| w.contains(
                "mcpServers.slow.startup_timeout_sec must be a positive number of seconds"
            )));
        }
    }

    #[test]
    fn maps_tool_timeout_sec_to_execution_unless_timeout_sets_it() {
        let (converted, warnings) = convert_mcp_server(
            "slow",
            &server(json!({"url": "https://mcp.example.com", "tool_timeout_sec": 120})),
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            converted.as_ref().and_then(|value| value.get("timeout")),
            Some(&json!({"execution": 120_000}))
        );

        let (overridden, override_warnings) = convert_mcp_server(
            "slow",
            &server(json!({"command": "srv", "timeout": 30000, "tool_timeout_sec": 120})),
        );
        assert_eq!(
            overridden.as_ref().and_then(|value| value.get("timeout")),
            Some(&json!({"catalog": 30000, "execution": 30000}))
        );
        assert_eq!(override_warnings.len(), 1, "{override_warnings:?}");
    }

    #[test]
    fn skips_server_without_command_or_url() {
        let (converted, warnings) = convert_mcp_server("broken", &server(json!({"args": ["a"]})));
        assert!(converted.is_none());
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn render_preserves_existing_servers_and_removes_same_named_legacy_entries() {
        let existing: ClaudeConfig = serde_json::from_value(json!({
            "model": "anthropic/claude-sonnet-4-5",
            "mcp": {
                "timeout": {"startup": 1000},
                "fs": {"type": "local", "command": ["old"]},
                "servers": {"keep": {"type": "remote", "url": "https://keep.example.com"}}
            }
        }))
        .expect("existing config should deserialize");
        let mut config = existing;
        config.mcp_servers = Some(HashMap::from([(
            "fs".to_string(),
            server(json!({"command": "new", "args": ["--flag"]})),
        )]));

        let (document, warnings) = render_opencode_config(&config).expect("render should succeed");

        assert!(warnings.is_empty());
        assert_eq!(
            document,
            json!({
                "mcp": {
                    "timeout": {"startup": 1000},
                    "servers": {
                        "fs": {"type": "local", "command": ["new", "--flag"]},
                        "keep": {"type": "remote", "url": "https://keep.example.com"}
                    }
                },
                "model": "anthropic/claude-sonnet-4-5"
            })
        );
        assert!(document.get("mcpServers").is_none());
    }

    #[test]
    fn render_rejects_non_object_mcp() {
        let config = ClaudeConfig {
            mcp_servers: Some(HashMap::new()),
            other: HashMap::from([("mcp".to_string(), json!(true))]),
        };
        assert!(render_opencode_config(&config).is_err());
    }

    #[test]
    fn validate_reports_v1_keys() {
        let warnings = validate_opencode_settings(&json!({
            "permission": {"edit": "allow"},
            "skills": {"paths": ["./skills"]},
            "mcp": {"fs": {"type": "local", "command": ["x"]}},
            "permissions": []
        }));
        assert_eq!(warnings.len(), 3, "{warnings:?}");
    }
}
