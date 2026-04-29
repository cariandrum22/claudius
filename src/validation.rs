use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;
use toml::Value as TomlValue;

use crate::app_config::{AppConfig, SecretManagerType};
use crate::config::Settings;
use crate::gemini_settings::{validate_gemini_settings, GeminiSettings};

static YAML_FRONTMATTER_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?s)\A---\r?\n(.*?)\r?\n---\r?\n?(.*)\z")
        .expect("frontmatter regex should compile")
});

#[derive(Debug)]
pub struct ValidationResult {
    pub warnings: Vec<String>,
}

/// Validates Claudius app configuration and returns semantic warnings.
#[must_use]
pub fn validate_app_config(config: &AppConfig) -> ValidationResult {
    let mut warnings = Vec::new();

    if let Some(secret_manager) = &config.secret_manager {
        if secret_manager.onepassword.is_some()
            && secret_manager.manager_type != SecretManagerType::OnePassword
        {
            warnings.push(
                "[secret-manager.onepassword] is configured but [secret-manager].type is not \"1password\"; these settings will be ignored".to_string(),
            );
        }
    }

    ValidationResult { warnings }
}

#[derive(Debug, Deserialize)]
struct GeminiCommandFile {
    prompt: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(flatten)]
    _extra: std::collections::BTreeMap<String, TomlValue>,
}

#[derive(Debug, Deserialize)]
struct MarkdownAgentFrontmatter {
    name: String,
    description: String,
    #[serde(flatten)]
    _extra: std::collections::BTreeMap<String, serde_yaml::Value>,
}

/// Validates a JSON file and returns any warnings about unknown fields
///
/// # Errors
///
/// Returns an error if:
/// - Unable to read the file
/// - File contains invalid JSON syntax
pub fn validate_json_file<P: AsRef<Path>>(path: P) -> Result<(Value, ValidationResult)> {
    let path_ref = path.as_ref();
    let content = fs::read_to_string(path_ref)
        .with_context(|| format!("Failed to read file: {}", path_ref.display()))?;

    // First, try to parse as JSON
    let json_value: Value = serde_json::from_str(&content).with_context(|| {
        format!("Failed to parse JSON from {}: Invalid JSON syntax", path_ref.display())
    })?;

    // Validate based on file type
    let warnings = if path_ref.to_string_lossy().contains("gemini") {
        validate_gemini_settings(&json_value)
    } else if path_ref.to_string_lossy().contains("claude")
        || path_ref.to_string_lossy().contains("codex")
    {
        validate_claude_settings(&json_value)
    } else {
        // For unknown file types, don't validate fields
        Vec::new()
    };

    Ok((json_value, ValidationResult { warnings }))
}

const KNOWN_PERMISSION_FIELDS: &[&str] = &["allow", "deny", "defaultMode"];

// Known Claude/Codex settings fields
const KNOWN_CLAUDE_FIELDS: &[&str] = &[
    "apiKeyHelper",
    "cleanupPeriodDays",
    "env",
    "includeCoAuthoredBy",
    "permissions",
    "preferredNotifChannel",
    "mcpServers",
    "mcp_servers",
    "extra",
    // Codex-specific fields
    "model",
    "modelProvider",
    "model_provider",
    "approvalPolicy",
    "approval_policy",
    "disableResponseStorage",
    "disable_response_storage",
    "notify",
    "modelProviders",
    "model_providers",
    "shellEnvironmentPolicy",
    "shell_environment_policy",
    "sandbox",
    "history",
];

/// Validates Claude/Codex settings and returns warnings for unknown fields
#[must_use]
pub fn validate_claude_settings(json: &Value) -> Vec<String> {
    let mut warnings = Vec::new();

    if let Value::Object(map) = json {
        for (key, value) in map {
            if !KNOWN_CLAUDE_FIELDS.contains(&key.as_str()) {
                warnings
                    .push(format!("Unknown setting '{key}' found in Claude/Codex configuration"));
            }

            // Validate nested permissions object
            if key == "permissions" {
                validate_permissions(value, &mut warnings);
            }
        }
    }

    warnings
}

/// Validate permissions object fields
fn validate_permissions(value: &Value, warnings: &mut Vec<String>) {
    let Value::Object(perm_map) = value else {
        return;
    };

    for (perm_key, _) in perm_map {
        if !KNOWN_PERMISSION_FIELDS.contains(&perm_key.as_str()) {
            warnings.push(format!("Unknown field '{perm_key}' in permissions"));
        }
    }
}

/// Pre-validate settings before sync to catch JSON errors early
///
/// # Errors
///
/// Returns an error if:
/// - Unable to read the file
/// - File contains invalid JSON syntax
pub fn pre_validate_settings<P: AsRef<Path>>(path: P) -> Result<ValidationResult> {
    let path_ref = path.as_ref();

    if !path_ref.exists() {
        // If file doesn't exist, that's fine - no validation needed
        return Ok(ValidationResult { warnings: Vec::new() });
    }

    let (_, validation_result) = validate_json_file(path_ref)?;
    Ok(validation_result)
}

/// Validates settings and returns parsed settings object with warnings
///
/// # Errors
///
/// Returns an error if:
/// - Unable to read the file
/// - File contains invalid JSON syntax
/// - Unable to parse JSON into Settings structure
pub fn validate_and_parse_settings<P: AsRef<Path>>(
    path: P,
) -> Result<(Option<Settings>, ValidationResult)> {
    let path_ref = path.as_ref();

    if !path_ref.exists() {
        return Ok((None, ValidationResult { warnings: Vec::new() }));
    }

    let (json_value, validation_result) = validate_json_file(path_ref)?;

    // Try to deserialize into Settings
    let settings: Settings = serde_json::from_value(json_value)
        .with_context(|| format!("Failed to parse settings from {}", path_ref.display()))?;

    Ok((Some(settings), validation_result))
}

/// Validates and parses Gemini settings
///
/// # Errors
///
/// Returns an error if:
/// - Unable to read the file
/// - File contains invalid JSON syntax
/// - Unable to parse JSON into `GeminiSettings` structure
pub fn validate_and_parse_gemini_settings<P: AsRef<Path>>(
    path: P,
) -> Result<(Option<GeminiSettings>, ValidationResult)> {
    let path_ref = path.as_ref();

    if !path_ref.exists() {
        return Ok((None, ValidationResult { warnings: Vec::new() }));
    }

    let (json_value, validation_result) = validate_json_file(path_ref)?;

    // Try to deserialize into GeminiSettings
    let settings: GeminiSettings = serde_json::from_value(json_value)
        .with_context(|| format!("Failed to parse Gemini settings from {}", path_ref.display()))?;

    Ok((Some(settings), validation_result))
}

/// Validates a Gemini custom command file.
///
/// # Errors
///
/// Returns an error if:
/// - Unable to read the file
/// - File contains invalid TOML syntax
/// - Required fields are missing or have invalid types
pub fn validate_gemini_command_file<P: AsRef<Path>>(path: P) -> Result<ValidationResult> {
    let path_ref = path.as_ref();
    let content = fs::read_to_string(path_ref)
        .with_context(|| format!("Failed to read Gemini command file: {}", path_ref.display()))?;

    let command: GeminiCommandFile = toml::from_str(&content)
        .with_context(|| format!("Failed to parse Gemini command file: {}", path_ref.display()))?;

    let mut warnings = Vec::new();
    if command.prompt.trim().is_empty() {
        warnings.push("Required field 'prompt' should not be empty".to_string());
    }

    if command
        .description
        .as_ref()
        .is_some_and(|description| description.trim().is_empty())
    {
        warnings.push("Optional field 'description' should not be empty when present".to_string());
    }

    Ok(ValidationResult { warnings })
}

/// Validates a Claude Code subagent definition file.
///
/// # Errors
///
/// Returns an error if:
/// - Unable to read the file
/// - The file is missing YAML frontmatter
/// - The YAML frontmatter is invalid
/// - Required metadata fields are missing or have invalid types
pub fn validate_claude_code_subagent_file<P: AsRef<Path>>(path: P) -> Result<ValidationResult> {
    let path_ref = path.as_ref();
    let content = fs::read_to_string(path_ref).with_context(|| {
        format!("Failed to read Claude Code subagent file: {}", path_ref.display())
    })?;

    let captures = YAML_FRONTMATTER_RE.captures(&content).ok_or_else(|| {
        anyhow::anyhow!(
            "Claude Code subagent file must start with YAML frontmatter delimited by ---: {}",
            path_ref.display()
        )
    })?;

    let frontmatter = captures.get(1).map(|capture| capture.as_str()).ok_or_else(|| {
        anyhow::anyhow!("Failed to extract YAML frontmatter from {}", path_ref.display())
    })?;
    let body = captures.get(2).map(|capture| capture.as_str()).ok_or_else(|| {
        anyhow::anyhow!("Failed to extract Markdown body from {}", path_ref.display())
    })?;

    let metadata: MarkdownAgentFrontmatter =
        serde_yaml::from_str(frontmatter).with_context(|| {
            format!("Failed to parse Claude Code subagent frontmatter: {}", path_ref.display())
        })?;

    Ok(validate_markdown_agent_metadata(&metadata, body))
}

/// Validates a Gemini custom agent definition file.
///
/// # Errors
///
/// Returns an error if:
/// - Unable to read the file
/// - The file is missing YAML frontmatter
/// - The YAML frontmatter is invalid
/// - Required metadata fields are missing or have invalid types
pub fn validate_gemini_agent_file<P: AsRef<Path>>(path: P) -> Result<ValidationResult> {
    let path_ref = path.as_ref();
    let content = fs::read_to_string(path_ref)
        .with_context(|| format!("Failed to read Gemini agent file: {}", path_ref.display()))?;

    let captures = YAML_FRONTMATTER_RE.captures(&content).ok_or_else(|| {
        anyhow::anyhow!(
            "Gemini agent file must start with YAML frontmatter delimited by ---: {}",
            path_ref.display()
        )
    })?;

    let frontmatter = captures.get(1).map(|capture| capture.as_str()).ok_or_else(|| {
        anyhow::anyhow!("Failed to extract YAML frontmatter from {}", path_ref.display())
    })?;
    let body = captures.get(2).map(|capture| capture.as_str()).ok_or_else(|| {
        anyhow::anyhow!("Failed to extract Markdown body from {}", path_ref.display())
    })?;

    let metadata: MarkdownAgentFrontmatter =
        serde_yaml::from_str(frontmatter).with_context(|| {
            format!("Failed to parse Gemini agent frontmatter: {}", path_ref.display())
        })?;

    Ok(validate_markdown_agent_metadata(&metadata, body))
}

fn validate_markdown_agent_metadata(
    metadata: &MarkdownAgentFrontmatter,
    body: &str,
) -> ValidationResult {
    let mut warnings = Vec::new();
    if metadata.name.trim().is_empty() {
        warnings.push("Required frontmatter field 'name' should not be empty".to_string());
    }
    if metadata.description.trim().is_empty() {
        warnings.push("Required frontmatter field 'description' should not be empty".to_string());
    }
    if body.trim().is_empty() {
        warnings.push("Agent Markdown body should not be empty".to_string());
    }

    ValidationResult { warnings }
}

/// Prompt user to continue after a warning
///
/// # Errors
///
/// Returns an error if:
/// - Unable to flush stdout
/// - Unable to read from stdin
pub fn prompt_continue() -> Result<bool> {
    use std::io::{self, Write};

    print!("Continue anyway? [y/N] ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().eq_ignore_ascii_case("y"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_config::{
        OnePasswordConfig, OnePasswordMode, SecretManagerConfig, SecretManagerType,
    };
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_validate_app_config_warns_when_onepassword_subtable_is_ignored() {
        let config = AppConfig {
            secret_manager: Some(SecretManagerConfig {
                manager_type: SecretManagerType::Vault,
                onepassword: Some(OnePasswordConfig {
                    mode: Some(OnePasswordMode::ServiceAccount),
                    service_account_token_path: Some("~/.config/op/service-account.token".into()),
                }),
            }),
            default: None,
            codex: None,
        };

        let result = validate_app_config(&config);
        assert_eq!(result.warnings.len(), 1);
        assert!(result
            .warnings
            .first()
            .is_some_and(|warning| warning.contains("[secret-manager.onepassword]")));
    }

    #[test]
    fn test_validate_app_config_allows_matching_onepassword_config() {
        let config = AppConfig {
            secret_manager: Some(SecretManagerConfig {
                manager_type: SecretManagerType::OnePassword,
                onepassword: Some(OnePasswordConfig {
                    mode: Some(OnePasswordMode::ServiceAccount),
                    service_account_token_path: Some("~/.config/op/service-account.token".into()),
                }),
            }),
            default: None,
            codex: None,
        };

        let result = validate_app_config(&config);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_validate_claude_settings_known_fields() {
        let json = json!({
            "apiKeyHelper": "/bin/helper",
            "cleanupPeriodDays": 30,
            "env": {"KEY": "value"},
            "includeCoAuthoredBy": true,
            "permissions": {
                "allow": ["Read"],
                "deny": ["Write"],
                "defaultMode": "allow"
            },
            "preferredNotifChannel": "email",
            "mcpServers": {}
        });

        let warnings = validate_claude_settings(&json);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_validate_claude_settings_unknown_fields() {
        let json = json!({
            "apiKeyHelper": "/bin/helper",
            "unknownField": "value",
            "anotherUnknown": 123
        });

        let warnings = validate_claude_settings(&json);
        assert_eq!(warnings.len(), 2);
        assert!(warnings.first().is_some_and(|w| w.contains("unknownField")));
        assert!(warnings.get(1).is_some_and(|w| w.contains("anotherUnknown")));
    }

    #[test]
    fn test_validate_claude_settings_unknown_permission_fields() {
        let json = json!({
            "permissions": {
                "allow": ["Read"],
                "unknownPerm": "value"
            }
        });

        let warnings = validate_claude_settings(&json);
        assert_eq!(warnings.len(), 1);
        assert!(warnings.first().is_some_and(|w| w.contains("unknownPerm")));
    }

    #[test]
    fn test_validate_claude_settings_not_object() {
        let json = json!("not an object");
        let warnings = validate_claude_settings(&json);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_validate_json_file_valid_claude() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("claude.json");

        let content = json!({
            "apiKeyHelper": "/bin/helper",
            "cleanupPeriodDays": 30
        });

        fs::write(&file_path, content.to_string()).expect("Failed to write file");

        let (value, result) = validate_json_file(&file_path).expect("Failed to validate JSON file");
        assert_eq!(value.get("apiKeyHelper"), Some(&serde_json::json!("/bin/helper")));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_validate_json_file_valid_gemini() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("gemini.json");

        let content = json!({
            "some_field": "value"
        });

        fs::write(&file_path, content.to_string()).expect("Failed to write file");

        let (value, _result) =
            validate_json_file(&file_path).expect("Failed to validate JSON file");
        assert_eq!(value.get("some_field"), Some(&serde_json::json!("value")));
        // Gemini validation would happen via validate_gemini_settings
    }

    #[test]
    fn test_validate_json_file_invalid_json() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("invalid.json");

        fs::write(&file_path, "{ invalid json").expect("Failed to write invalid JSON");

        let result = validate_json_file(&file_path);
        assert!(result.is_err());
        assert!(result
            .expect_err("Should fail with invalid JSON")
            .to_string()
            .contains("Invalid JSON syntax"));
    }

    #[test]
    fn test_validate_json_file_missing_file() {
        let result = validate_json_file("/nonexistent/file.json");
        assert!(result.is_err());
        assert!(result
            .expect_err("Should fail with missing file")
            .to_string()
            .contains("Failed to read file"));
    }

    #[test]
    fn test_validate_json_file_unknown_type() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("unknown.json");

        let content = json!({
            "someField": "value"
        });

        fs::write(&file_path, content.to_string()).expect("Failed to write file");

        let (_, result) = validate_json_file(&file_path).expect("Failed to validate JSON file");
        assert!(result.warnings.is_empty()); // Unknown file types don't validate
    }

    #[test]
    fn test_pre_validate_settings_missing_file() {
        let result = pre_validate_settings("/nonexistent/settings.json")
            .expect("Failed to pre-validate settings");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_pre_validate_settings_valid_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("claude.json");

        let content = json!({
            "apiKeyHelper": "/bin/helper"
        });

        fs::write(&file_path, content.to_string()).expect("Failed to write file");

        let result = pre_validate_settings(&file_path).expect("Failed to pre-validate settings");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_pre_validate_settings_with_warnings() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("claude.json");

        let content = json!({
            "unknownField": "value"
        });

        fs::write(&file_path, content.to_string()).expect("Failed to write file");

        let result = pre_validate_settings(&file_path).expect("Failed to pre-validate settings");
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn test_validate_and_parse_settings_missing_file() {
        let (settings, result) = validate_and_parse_settings("/nonexistent/settings.json")
            .expect("Failed to validate and parse settings");
        assert!(settings.is_none());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_validate_and_parse_settings_valid() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("claude.json");

        let content = json!({
            "apiKeyHelper": "/bin/helper",
            "cleanupPeriodDays": 30,
            "includeCoAuthoredBy": true
        });

        fs::write(&file_path, content.to_string()).expect("Failed to write file");

        let (settings_opt, result) =
            validate_and_parse_settings(&file_path).expect("Failed to validate and parse settings");
        assert!(settings_opt.is_some());
        let settings = settings_opt.expect("Settings should be present");
        assert_eq!(settings.api_key_helper, Some("/bin/helper".to_string()));
        assert_eq!(settings.cleanup_period_days, Some(30));
        assert_eq!(settings.include_co_authored_by, Some(true));
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_validate_and_parse_settings_invalid_structure() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("claude.json");

        let content = json!({
            "apiKeyHelper": 123 // Wrong type
        });

        fs::write(&file_path, content.to_string()).expect("Failed to write file");

        let result = validate_and_parse_settings(&file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_and_parse_gemini_settings_missing_file() {
        let (settings, result) = validate_and_parse_gemini_settings("/nonexistent/gemini.json")
            .expect("Failed to validate and parse gemini settings");
        assert!(settings.is_none());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_validate_and_parse_gemini_settings_valid() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("gemini.json");

        // Create a valid Gemini settings JSON
        let content = json!({
            "$schema": "https://raw.githubusercontent.com/google-gemini/gemini-cli/main/schemas/settings.schema.json",
            "general": {
                "preferredEditor": "code"
            },
            "ui": {
                "theme": "GitHub"
            },
            "tools": {
                "autoAccept": true
            },
            "privacy": {
                "usageStatisticsEnabled": true
            },
            "telemetry": {
                "enabled": false
            },
            "mcpServers": {
                "server": {
                    "command": "node",
                    "args": ["server.js"]
                }
            }
        });

        fs::write(&file_path, content.to_string()).expect("Failed to write file");

        let (settings_opt, result) = validate_and_parse_gemini_settings(&file_path)
            .expect("Failed to validate and parse gemini settings");
        assert!(result.warnings.is_empty());

        let settings = settings_opt.expect("Settings should be present");
        assert_eq!(
            settings.schema.as_deref(),
            Some("https://raw.githubusercontent.com/google-gemini/gemini-cli/main/schemas/settings.schema.json")
        );
        assert!(
            settings
                .mcp_servers
                .as_ref()
                .is_some_and(|servers| servers.contains_key("server")),
            "Expected mcpServers.server to be present"
        );
    }

    #[test]
    fn test_validate_gemini_command_file_valid() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("review.toml");

        fs::write(
            &file_path,
            "description = \"Review the current diff\"\nprompt = \"Review the patch.\"",
        )
        .expect("Failed to write Gemini command");

        let result =
            validate_gemini_command_file(&file_path).expect("Gemini command should validate");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_validate_gemini_command_file_missing_prompt_fails() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("review.toml");

        fs::write(&file_path, "description = \"Review the current diff\"")
            .expect("Failed to write Gemini command");

        let error = validate_gemini_command_file(&file_path)
            .expect_err("Gemini command without prompt should fail");
        assert!(format!("{error:#}").contains("missing field `prompt`"));
    }

    #[test]
    fn test_validate_claude_code_subagent_file_valid() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("reviewer.md");

        fs::write(
            &file_path,
            "---\nname: reviewer\ndescription: Review code changes\n---\nFocus on regressions.\n",
        )
        .expect("Failed to write subagent");

        let result = validate_claude_code_subagent_file(&file_path)
            .expect("Claude Code subagent should validate");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_validate_claude_code_subagent_file_missing_frontmatter_fails() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("reviewer.md");

        fs::write(&file_path, "Focus on regressions.\n").expect("Failed to write subagent");

        let error = validate_claude_code_subagent_file(&file_path)
            .expect_err("Subagent without frontmatter should fail");
        assert!(error.to_string().contains("must start with YAML frontmatter delimited by ---"));
    }

    #[test]
    fn test_validate_claude_code_subagent_file_empty_body_warns() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("reviewer.md");

        fs::write(&file_path, "---\nname: reviewer\ndescription: Review code changes\n---\n")
            .expect("Failed to write subagent");

        let result = validate_claude_code_subagent_file(&file_path)
            .expect("Subagent with empty body should still parse");
        assert_eq!(result.warnings.len(), 1);
        assert!(result
            .warnings
            .first()
            .is_some_and(|warning| warning.contains("Markdown body should not be empty")));
    }
}
