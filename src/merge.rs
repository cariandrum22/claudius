#![allow(clippy::self_named_module_files)]

use crate::config::{ClaudeConfig, McpServersConfig, Settings};
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::hash::BuildHasher;

pub mod strategy;

pub use strategy::MergeStrategy;

/// Structure to hold conflict information
#[derive(Debug)]
pub struct MergeConflict {
    pub field_name: String,
    pub existing_value: String,
    pub new_value: String,
}

/// Detect conflicts between existing and new MCP server configurations
pub fn detect_mcp_conflicts<S>(
    existing: &HashMap<String, crate::config::McpServerConfig, S>,
    new: &HashMap<String, crate::config::McpServerConfig, S>,
) -> Vec<(String, MergeConflict)>
where
    S: BuildHasher,
{
    let mut conflicts = Vec::new();

    for (name, new_config) in new {
        if let Some(existing_config) = existing.get(name) {
            if existing_config != new_config {
                conflicts.push((
                    name.clone(),
                    MergeConflict {
                        field_name: format!("mcpServers.{name}"),
                        existing_value: format!("{existing_config:?}"),
                        new_value: format!("{new_config:?}"),
                    },
                ));
            }
        }
    }

    conflicts
}

/// Prompt user to resolve a merge conflict
///
/// # Errors
///
/// Returns an error if:
/// - Standard output cannot be flushed
/// - Reading from standard input fails
pub fn prompt_resolve_conflict(conflict: &MergeConflict) -> Result<bool> {
    use std::io::{self, Write};

    println!("\n=== Configuration conflict detected ===");
    println!("  Field: {}", conflict.field_name);
    println!("  Current value: {}", conflict.existing_value);
    println!("  New value: {}", conflict.new_value);
    print!("\nOverwrite with new value? [y/N] ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().eq_ignore_ascii_case("y"))
}

/// Merges MCP server configurations into Claude configuration.
///
/// # Errors
///
/// Returns an error if JSON parsing fails.
pub fn merge_configs(
    claude_config: &mut ClaudeConfig,
    mcp_servers: &McpServersConfig,
    strategy: MergeStrategy,
) -> anyhow::Result<()> {
    let new_servers = &mcp_servers.mcp_servers;

    match strategy {
        MergeStrategy::Replace => {
            claude_config.mcp_servers = Some(new_servers.clone());
        },
        MergeStrategy::Merge => {
            let existing = claude_config.mcp_servers.get_or_insert_with(HashMap::new);

            for (name, config) in new_servers {
                existing.insert(name.clone(), config.clone());
            }
        },
        MergeStrategy::MergePreserveExisting => {
            let existing = claude_config.mcp_servers.get_or_insert_with(HashMap::new);

            for (name, config) in new_servers {
                existing.entry(name.clone()).or_insert_with(|| config.clone());
            }
        },
        MergeStrategy::InteractiveMerge => {
            let existing = claude_config.mcp_servers.get_or_insert_with(HashMap::new);

            // Detect conflicts
            let conflicts = detect_mcp_conflicts(existing, new_servers);

            // Resolve each conflict interactively
            for (name, conflict) in conflicts {
                if prompt_resolve_conflict(&conflict)? {
                    // User chose to overwrite
                    if let Some(server_config) = new_servers.get(&name) {
                        existing.insert(name.clone(), server_config.clone());
                    }
                }
                // Otherwise, keep existing value
            }

            // Add new servers that don't conflict
            for (name, config) in new_servers {
                if !existing.contains_key(name) {
                    existing.insert(name.clone(), config.clone());
                }
            }
        },
    }

    Ok(())
}

/// Detect conflicts in settings
pub fn detect_settings_conflicts<S: std::hash::BuildHasher>(
    existing: &HashMap<String, Value, S>,
    settings: &Settings,
) -> Vec<MergeConflict> {
    let mut conflicts = Vec::new();

    // Check each field using a helper to reduce duplication
    check_field_conflict(
        &mut conflicts,
        existing,
        "apiKeyHelper",
        settings.api_key_helper.as_ref().map(|v| Value::String(v.clone())),
    );

    check_field_conflict(
        &mut conflicts,
        existing,
        "cleanupPeriodDays",
        settings.cleanup_period_days.map(|v| Value::Number(serde_json::Number::from(v))),
    );

    check_field_conflict(
        &mut conflicts,
        existing,
        "env",
        settings.env.as_ref().and_then(|v| serde_json::to_value(v).ok()),
    );

    check_field_conflict(
        &mut conflicts,
        existing,
        "includeCoAuthoredBy",
        settings.include_co_authored_by.map(Value::Bool),
    );

    check_field_conflict(
        &mut conflicts,
        existing,
        "permissions",
        settings.permissions.as_ref().and_then(|v| serde_json::to_value(v).ok()),
    );

    check_field_conflict(
        &mut conflicts,
        existing,
        "preferredNotifChannel",
        settings.preferred_notif_channel.as_ref().map(|v| Value::String(v.clone())),
    );

    conflicts
}

/// Helper function to check for conflicts in a single field
fn check_field_conflict<S: std::hash::BuildHasher>(
    conflicts: &mut Vec<MergeConflict>,
    existing: &HashMap<String, Value, S>,
    field_name: &str,
    new_value_opt: Option<Value>,
) {
    if let Some(new_value) = new_value_opt {
        if let Some(existing_value) = existing.get(field_name) {
            if existing_value != &new_value {
                conflicts.push(MergeConflict {
                    field_name: field_name.to_string(),
                    existing_value: format_value(existing_value),
                    new_value: format_value(&new_value),
                });
            }
        }
    }
}

/// Format a JSON value for display
fn format_value(value: &Value) -> String {
    match value {
        Value::String(_) | Value::Number(_) | Value::Bool(_) => value.to_string(),
        _ => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
    }
}

/// Merges settings into Claude configuration using the default merge strategy.
///
/// # Errors
///
/// Returns an error if:
/// - User cancels during interactive conflict resolution
/// - I/O errors occur during prompting
pub fn merge_settings(claude_config: &mut ClaudeConfig, settings: &Settings) -> anyhow::Result<()> {
    merge_settings_with_strategy(claude_config, settings, MergeStrategy::Merge)
}

/// Merges settings into Claude configuration with a specified strategy.
///
/// # Errors
///
/// Returns an error if:
/// - User cancels during interactive conflict resolution
/// - I/O errors occur during prompting
pub fn merge_settings_with_strategy(
    claude_config: &mut ClaudeConfig,
    settings: &Settings,
    strategy: MergeStrategy,
) -> anyhow::Result<()> {
    // Track which fields should be skipped in interactive mode
    let skip_fields = if strategy == MergeStrategy::InteractiveMerge {
        resolve_interactive_conflicts(&claude_config.other, settings)?
    } else {
        std::collections::HashSet::new()
    };

    // Merge each setting field
    merge_field(
        &mut claude_config.other,
        &skip_fields,
        "apiKeyHelper",
        settings.api_key_helper.as_ref().map(|v| Value::String(v.clone())),
    );

    merge_field(
        &mut claude_config.other,
        &skip_fields,
        "cleanupPeriodDays",
        settings.cleanup_period_days.map(|v| Value::Number(serde_json::Number::from(v))),
    );

    merge_field(
        &mut claude_config.other,
        &skip_fields,
        "env",
        settings.env.as_ref().and_then(|v| serde_json::to_value(v).ok()),
    );

    merge_field(
        &mut claude_config.other,
        &skip_fields,
        "includeCoAuthoredBy",
        settings.include_co_authored_by.map(Value::Bool),
    );

    merge_field(
        &mut claude_config.other,
        &skip_fields,
        "permissions",
        settings.permissions.as_ref().and_then(|v| serde_json::to_value(v).ok()),
    );

    merge_field(
        &mut claude_config.other,
        &skip_fields,
        "preferredNotifChannel",
        settings.preferred_notif_channel.as_ref().map(|v| Value::String(v.clone())),
    );

    Ok(())
}

/// Resolve conflicts interactively and return fields to skip
fn resolve_interactive_conflicts<S: std::hash::BuildHasher>(
    existing: &HashMap<String, Value, S>,
    settings: &Settings,
) -> anyhow::Result<std::collections::HashSet<String>> {
    let mut skip_fields = std::collections::HashSet::new();
    let conflicts = detect_settings_conflicts(existing, settings);

    for conflict in conflicts {
        if !prompt_resolve_conflict(&conflict)? {
            skip_fields.insert(conflict.field_name);
        }
    }

    Ok(skip_fields)
}

/// Merge a single field if not skipped
fn merge_field<S: std::hash::BuildHasher>(
    target: &mut HashMap<String, Value, S>,
    skip_fields: &std::collections::HashSet<String>,
    field_name: &str,
    value: Option<Value>,
) {
    if let Some(val) = value {
        if !skip_fields.contains(field_name) {
            target.insert(field_name.to_string(), val);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpServerConfig, Permissions};
    use serde_json::json;

    fn create_test_server_config(command: &str) -> McpServerConfig {
        McpServerConfig { command: command.to_string(), args: vec![], env: HashMap::new() }
    }

    #[test]
    fn test_merge_configs_replace_strategy() {
        let mut claude_config = ClaudeConfig {
            mcp_servers: Some(HashMap::from([(
                "existing".to_string(),
                create_test_server_config("old-command"),
            )])),
            other: HashMap::new(),
        };

        let new_servers = McpServersConfig {
            mcp_servers: HashMap::from([(
                "new".to_string(),
                create_test_server_config("new-command"),
            )]),
        };

        merge_configs(&mut claude_config, &new_servers, MergeStrategy::Replace)
            .expect("merge_configs should succeed for Replace strategy");

        let servers = claude_config.mcp_servers.expect("MCP servers should be present after merge");
        assert_eq!(servers.len(), 1);
        assert!(servers.contains_key("new"));
        assert!(!servers.contains_key("existing"));
        assert_eq!(servers.get("new").map(|s| &s.command), Some(&"new-command".to_string()));
    }

    #[test]
    fn test_merge_configs_merge_strategy() {
        let mut claude_config = ClaudeConfig {
            mcp_servers: Some(HashMap::from([(
                "existing".to_string(),
                create_test_server_config("old-command"),
            )])),
            other: HashMap::new(),
        };

        let new_servers = McpServersConfig {
            mcp_servers: HashMap::from([
                ("new".to_string(), create_test_server_config("new-command")),
                ("existing".to_string(), create_test_server_config("updated-command")),
            ]),
        };

        merge_configs(&mut claude_config, &new_servers, MergeStrategy::Merge)
            .expect("merge_configs should succeed for Merge strategy");

        let servers = claude_config.mcp_servers.expect("MCP servers should be present after merge");
        assert_eq!(servers.len(), 2);
        assert_eq!(
            servers.get("existing").map(|s| &s.command),
            Some(&"updated-command".to_string())
        ); // Overwritten
        assert_eq!(servers.get("new").map(|s| &s.command), Some(&"new-command".to_string()));
    }

    #[test]
    fn test_merge_configs_preserve_existing_strategy() {
        let mut claude_config = ClaudeConfig {
            mcp_servers: Some(HashMap::from([(
                "existing".to_string(),
                create_test_server_config("old-command"),
            )])),
            other: HashMap::new(),
        };

        let new_servers = McpServersConfig {
            mcp_servers: HashMap::from([
                ("new".to_string(), create_test_server_config("new-command")),
                ("existing".to_string(), create_test_server_config("updated-command")),
            ]),
        };

        merge_configs(&mut claude_config, &new_servers, MergeStrategy::MergePreserveExisting)
            .expect("merge_configs should succeed for MergePreserveExisting strategy");

        let servers = claude_config.mcp_servers.expect("MCP servers should be present after merge");
        assert_eq!(servers.len(), 2);
        assert_eq!(servers.get("existing").map(|s| &s.command), Some(&"old-command".to_string())); // Preserved
        assert_eq!(servers.get("new").map(|s| &s.command), Some(&"new-command".to_string()));
    }

    #[test]
    fn test_merge_configs_empty_existing() {
        let mut claude_config = ClaudeConfig { mcp_servers: None, other: HashMap::new() };

        let new_servers = McpServersConfig {
            mcp_servers: HashMap::from([(
                "server1".to_string(),
                create_test_server_config("cmd1"),
            )]),
        };

        merge_configs(&mut claude_config, &new_servers, MergeStrategy::Merge)
            .expect("merge_configs should succeed for Merge strategy");

        let servers = claude_config.mcp_servers.expect("MCP servers should be present after merge");
        assert_eq!(servers.len(), 1);
        assert_eq!(servers.get("server1").map(|s| &s.command), Some(&"cmd1".to_string()));
    }

    #[test]
    fn test_merge_settings_all_fields() {
        let mut claude_config = ClaudeConfig { mcp_servers: None, other: HashMap::new() };

        let settings = Settings {
            api_key_helper: Some("/bin/helper".to_string()),
            cleanup_period_days: Some(30),
            env: Some(HashMap::from([("KEY".to_string(), "value".to_string())])),
            include_co_authored_by: Some(true),
            permissions: Some(Permissions {
                allow: vec!["Read".to_string()],
                deny: vec!["Write".to_string()],
                default_mode: Some("allow".to_string()),
            }),
            preferred_notif_channel: Some("email".to_string()),
            mcp_servers: None,
            extra: HashMap::new(),
        };

        merge_settings(&mut claude_config, &settings).expect("merge_settings should succeed");

        assert_eq!(claude_config.other.get("apiKeyHelper"), Some(&json!("/bin/helper")));
        assert_eq!(claude_config.other.get("cleanupPeriodDays"), Some(&json!(30)));
        assert_eq!(
            claude_config.other.get("env").and_then(|v| v.get("KEY")),
            Some(&json!("value"))
        );
        assert_eq!(claude_config.other.get("includeCoAuthoredBy"), Some(&json!(true)));
        assert_eq!(
            claude_config
                .other
                .get("permissions")
                .and_then(|v| v.get("allow"))
                .and_then(|v| v.get(0)),
            Some(&json!("Read"))
        );
        assert_eq!(
            claude_config
                .other
                .get("permissions")
                .and_then(|v| v.get("deny"))
                .and_then(|v| v.get(0)),
            Some(&json!("Write"))
        );
        assert_eq!(
            claude_config.other.get("permissions").and_then(|v| v.get("defaultMode")),
            Some(&json!("allow"))
        );
        assert_eq!(claude_config.other.get("preferredNotifChannel"), Some(&json!("email")));
    }

    #[test]
    fn test_merge_settings_partial() {
        let mut claude_config = ClaudeConfig {
            mcp_servers: None,
            other: HashMap::from([("existingKey".to_string(), json!("existingValue"))]),
        };

        let settings = Settings {
            api_key_helper: Some("/bin/helper".to_string()),
            cleanup_period_days: None,
            env: None,
            include_co_authored_by: Some(false),
            permissions: None,
            preferred_notif_channel: None,
            mcp_servers: None,
            extra: HashMap::new(),
        };

        merge_settings(&mut claude_config, &settings).expect("merge_settings should succeed");

        assert_eq!(claude_config.other.get("apiKeyHelper"), Some(&json!("/bin/helper")));
        assert_eq!(claude_config.other.get("includeCoAuthoredBy"), Some(&json!(false)));
        assert_eq!(claude_config.other.get("existingKey"), Some(&json!("existingValue"))); // Preserved
        assert!(!claude_config.other.contains_key("cleanupPeriodDays")); // Not added
    }

    #[test]
    fn test_merge_settings_overwrite_existing() {
        let mut claude_config = ClaudeConfig {
            mcp_servers: None,
            other: HashMap::from([
                ("apiKeyHelper".to_string(), json!("/old/helper")),
                ("cleanupPeriodDays".to_string(), json!(10)),
            ]),
        };

        let settings = Settings {
            api_key_helper: Some("/new/helper".to_string()),
            cleanup_period_days: Some(20),
            env: None,
            include_co_authored_by: None,
            permissions: None,
            preferred_notif_channel: None,
            mcp_servers: None,
            extra: HashMap::new(),
        };

        merge_settings(&mut claude_config, &settings).expect("merge_settings should succeed");

        assert_eq!(claude_config.other.get("apiKeyHelper"), Some(&json!("/new/helper")));
        assert_eq!(claude_config.other.get("cleanupPeriodDays"), Some(&json!(20)));
    }

    #[test]
    fn test_merge_settings_complex_permissions() {
        let mut claude_config = ClaudeConfig { mcp_servers: None, other: HashMap::new() };

        let settings = Settings {
            api_key_helper: None,
            cleanup_period_days: None,
            env: None,
            include_co_authored_by: None,
            permissions: Some(Permissions {
                allow: vec!["Bash(ls)".to_string(), "Read(*.txt)".to_string()],
                deny: vec!["Write(/etc/*)".to_string()],
                default_mode: Some("deny".to_string()),
            }),
            preferred_notif_channel: None,
            mcp_servers: None,
            extra: HashMap::new(),
        };

        merge_settings(&mut claude_config, &settings).expect("merge_settings should succeed");

        let permissions = claude_config
            .other
            .get("permissions")
            .expect("permissions should be present after merge");
        assert_eq!(
            permissions.get("allow").and_then(|v| v.as_array()).map(std::vec::Vec::len),
            Some(2)
        );
        assert_eq!(permissions.get("allow").and_then(|v| v.get(0)), Some(&json!("Bash(ls)")));
        assert_eq!(permissions.get("allow").and_then(|v| v.get(1)), Some(&json!("Read(*.txt)")));
        assert_eq!(permissions.get("deny").and_then(|v| v.get(0)), Some(&json!("Write(/etc/*)")));
        assert_eq!(permissions.get("defaultMode"), Some(&json!("deny")));
    }

    #[test]
    fn test_merge_settings_env_multiple_entries() {
        let mut claude_config = ClaudeConfig { mcp_servers: None, other: HashMap::new() };

        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/custom/path".to_string());
        env.insert("DEBUG".to_string(), "true".to_string());
        env.insert("API_KEY".to_string(), "secret123".to_string());

        let settings = Settings {
            api_key_helper: None,
            cleanup_period_days: None,
            env: Some(env),
            include_co_authored_by: None,
            permissions: None,
            preferred_notif_channel: None,
            mcp_servers: None,
            extra: HashMap::new(),
        };

        merge_settings(&mut claude_config, &settings).expect("merge_settings should succeed");

        let env_value = claude_config.other.get("env").expect("env should be present after merge");
        assert_eq!(env_value.get("PATH"), Some(&json!("/custom/path")));
        assert_eq!(env_value.get("DEBUG"), Some(&json!("true")));
        assert_eq!(env_value.get("API_KEY"), Some(&json!("secret123")));
    }
}
