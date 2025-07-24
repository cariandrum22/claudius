use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GeminiSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_file_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub bug_command: Option<BugCommand>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_filtering: Option<FileFiltering>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub core_tools: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_tools: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_accept: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_discovery_command: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_command: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpointing: Option<Checkpointing>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_editor: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub telemetry: Option<Telemetry>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_statistics_enabled: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_tips: Option<bool>,

    // Catch-all for unknown fields to preserve them
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BugCommand {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_template: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileFiltering {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub respect_git_ignore: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_recursive_file_search: Option<bool>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Checkpointing {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Telemetry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub otlp_endpoint: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_prompts: Option<bool>,
}

// Known field names for validation
pub const KNOWN_GEMINI_FIELDS: &[&str] = &[
    "contextFileName",
    "bugCommand",
    "fileFiltering",
    "coreTools",
    "excludeTools",
    "autoAccept",
    "theme",
    "sandbox",
    "toolDiscoveryCommand",
    "toolCallCommand",
    "checkpointing",
    "preferredEditor",
    "telemetry",
    "usageStatisticsEnabled",
    "hideTips",
];

pub const KNOWN_BUG_COMMAND_FIELDS: &[&str] = &["urlTemplate"];

pub const KNOWN_FILE_FILTERING_FIELDS: &[&str] = &["respectGitIgnore", "enableRecursiveFileSearch"];

pub const KNOWN_CHECKPOINTING_FIELDS: &[&str] = &["enabled"];

pub const KNOWN_TELEMETRY_FIELDS: &[&str] = &["enabled", "target", "otlpEndpoint", "logPrompts"];

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
    let (known_fields, field_name) = match parent_key {
        "bugCommand" => (KNOWN_BUG_COMMAND_FIELDS, "bugCommand"),
        "fileFiltering" => (KNOWN_FILE_FILTERING_FIELDS, "fileFiltering"),
        "checkpointing" => (KNOWN_CHECKPOINTING_FIELDS, "checkpointing"),
        "telemetry" => (KNOWN_TELEMETRY_FIELDS, "telemetry"),
        _ => return,
    };

    if let Value::Object(nested_map) = value {
        for (nested_key, _) in nested_map {
            if !known_fields.contains(&nested_key.as_str()) {
                warnings.push(format!("Unknown field '{nested_key}' in {field_name}"));
            }
        }
    }
}
