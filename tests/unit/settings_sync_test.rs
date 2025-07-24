use claudius::validation::{
    validate_and_parse_gemini_settings, validate_and_parse_settings, validate_json_file,
};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_claude_settings_parsing() {
        let temp_dir = TempDir::new().unwrap();
        let settings_file = temp_dir.path().join("claude.settings.json");

        let settings_json = json!({
            "apiKeyHelper": "/path/to/helper",
            "cleanupPeriodDays": 30,
            "env": {
                "CUSTOM_VAR": "value"
            },
            "includeCoAuthoredBy": true,
            "permissions": {
                "allow": ["Read", "Write"],
                "deny": ["Delete"],
                "defaultMode": "allow"
            },
            "preferredNotifChannel": "chat"
        });

        fs::write(&settings_file, serde_json::to_string_pretty(&settings_json).unwrap()).unwrap();

        let (settings, validation_result) = validate_and_parse_settings(&settings_file).unwrap();

        assert!(settings.is_some());
        assert!(validation_result.warnings.is_empty());

        let settings_data = settings.unwrap();
        assert_eq!(settings_data.api_key_helper, Some("/path/to/helper".to_string()));
        assert_eq!(settings_data.cleanup_period_days, Some(30));
    }

    #[test]
    fn test_claude_settings_with_unknown_fields() {
        let temp_dir = TempDir::new().unwrap();
        let settings_file = temp_dir.path().join("claude.settings.json");

        let settings_json = json!({
            "apiKeyHelper": "/path/to/helper",
            "unknownField": "some value",
            "permissions": {
                "allow": ["Read"],
                "unknownPermission": true
            }
        });

        fs::write(&settings_file, serde_json::to_string_pretty(&settings_json).unwrap()).unwrap();

        let (json_value, validation_result) = validate_json_file(&settings_file).unwrap();

        // Should have warnings about unknown fields
        assert_eq!(validation_result.warnings.len(), 2);
        assert!(validation_result.warnings.iter().any(|w| w.contains("unknownField")));
        assert!(validation_result.warnings.iter().any(|w| w.contains("unknownPermission")));

        // But the JSON should still be parsed
        assert!(json_value.get("unknownField").is_some());
    }

    #[test]
    fn test_invalid_json_parsing() {
        let temp_dir = TempDir::new().unwrap();
        let settings_file = temp_dir.path().join("invalid.json");

        // Write invalid JSON
        fs::write(&settings_file, "{ invalid json }").unwrap();

        // Should fail with JSON parsing error
        let result = validate_json_file(&settings_file);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid JSON syntax"));
    }

    #[test]
    fn test_valid_gemini_settings_parsing() {
        let temp_dir = TempDir::new().unwrap();
        let settings_file = temp_dir.path().join("gemini.settings.json");

        let settings_json = json!({
            "contextFileName": "GEMINI.md",
            "bugCommand": {
                "urlTemplate": "https://example.com/bug?title={title}"
            },
            "fileFiltering": {
                "respectGitIgnore": true,
                "enableRecursiveFileSearch": false
            },
            "coreTools": ["tool1", "tool2"],
            "autoAccept": true,
            "theme": "dark",
            "sandbox": true,
            "checkpointing": {
                "enabled": true
            },
            "telemetry": {
                "enabled": false,
                "target": "local",
                "otlpEndpoint": "http://localhost:4317",
                "logPrompts": false
            },
            "usageStatisticsEnabled": true,
            "hideTips": false
        });

        fs::write(&settings_file, serde_json::to_string_pretty(&settings_json).unwrap()).unwrap();

        let (settings, validation_result) =
            validate_and_parse_gemini_settings(&settings_file).unwrap();

        assert!(settings.is_some());
        assert!(validation_result.warnings.is_empty());

        let settings_data = settings.unwrap();
        assert_eq!(settings_data.context_file_name, Some("GEMINI.md".to_string()));
        assert_eq!(settings_data.auto_accept, Some(true));
        assert_eq!(settings_data.theme, Some("dark".to_string()));
    }

    #[test]
    fn test_gemini_settings_with_unknown_fields() {
        let temp_dir = TempDir::new().unwrap();
        let settings_file = temp_dir.path().join("gemini.settings.json");

        let settings_json = json!({
            "contextFileName": "GEMINI.md",
            "unknownSetting": "value",
            "bugCommand": {
                "urlTemplate": "https://example.com/bug",
                "unknownBugField": 123
            },
            "telemetry": {
                "enabled": true,
                "unknownTelemetryField": "test"
            }
        });

        fs::write(&settings_file, serde_json::to_string_pretty(&settings_json).unwrap()).unwrap();

        let (json_value, validation_result) = validate_json_file(&settings_file).unwrap();

        // Should have warnings about unknown fields
        assert_eq!(validation_result.warnings.len(), 3);
        assert!(validation_result.warnings.iter().any(|w| w.contains("unknownSetting")));
        assert!(validation_result.warnings.iter().any(|w| w.contains("unknownBugField")));
        assert!(validation_result.warnings.iter().any(|w| w.contains("unknownTelemetryField")));

        // Unknown fields should still be preserved in the JSON
        assert!(json_value.get("unknownSetting").is_some());
    }

    #[test]
    fn test_gemini_settings_preserve_extra_fields() {
        let temp_dir = TempDir::new().unwrap();
        let settings_file = temp_dir.path().join("gemini.settings.json");

        let settings_json = json!({
            "contextFileName": "GEMINI.md",
            "futureFeature": {
                "nested": "value"
            },
            "experimentalFlag": true
        });

        fs::write(&settings_file, serde_json::to_string_pretty(&settings_json).unwrap()).unwrap();

        let (settings, validation_result) =
            validate_and_parse_gemini_settings(&settings_file).unwrap();

        assert!(settings.is_some());
        let settings_data = settings.unwrap();

        // Extra fields should be preserved in the extra map
        assert!(settings_data.extra.contains_key("futureFeature"));
        assert!(settings_data.extra.contains_key("experimentalFlag"));

        // Should have warnings about unknown fields
        assert_eq!(validation_result.warnings.len(), 2);
    }

    #[test]
    fn test_nonexistent_file_handling() {
        let temp_dir = TempDir::new().unwrap();
        let settings_file = temp_dir.path().join("nonexistent.json");

        // Should return None without error for nonexistent files
        let (settings, validation_result) = validate_and_parse_settings(&settings_file).unwrap();
        assert!(settings.is_none());
        assert!(validation_result.warnings.is_empty());
    }

    #[test]
    fn test_empty_json_file() {
        let temp_dir = TempDir::new().unwrap();
        let settings_file = temp_dir.path().join("empty.json");

        fs::write(&settings_file, "{}").unwrap();

        let (settings, validation_result) = validate_and_parse_settings(&settings_file).unwrap();

        assert!(settings.is_some());
        assert!(validation_result.warnings.is_empty());

        // All fields should be None
        let settings_data = settings.unwrap();
        assert!(settings_data.api_key_helper.is_none());
        assert!(settings_data.cleanup_period_days.is_none());
        assert!(settings_data.env.is_none());
    }
}
