use assert_fs::prelude::*;
use claudius::config::reader::read_settings;
use claudius::config::{ClaudeConfig, Permissions, Settings};
use claudius::merge::merge_settings;
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_structure() {
        let settings = Settings {
            api_key_helper: Some("/bin/generate_api_key.sh".to_string()),
            cleanup_period_days: Some(20),
            env: Some(HashMap::from([("FOO".to_string(), "bar".to_string())])),
            include_co_authored_by: Some(false),
            permissions: Some(Permissions {
                allow: vec!["Bash(npm run lint)".to_string()],
                deny: vec![],
                default_mode: None,
            }),
            preferred_notif_channel: None,
            mcp_servers: None,
            extra: HashMap::new(),
        };

        assert_eq!(settings.api_key_helper, Some("/bin/generate_api_key.sh".to_string()));
        assert_eq!(settings.cleanup_period_days, Some(20));
        assert_eq!(settings.env.as_ref().unwrap().get("FOO"), Some(&"bar".to_string()));
        assert_eq!(settings.include_co_authored_by, Some(false));
        assert_eq!(settings.permissions.as_ref().unwrap().allow.len(), 1);
    }

    #[test]
    fn test_settings_serialization() {
        let settings = Settings {
            api_key_helper: Some("/bin/generate_api_key.sh".to_string()),
            cleanup_period_days: Some(20),
            env: Some(HashMap::from([("FOO".to_string(), "bar".to_string())])),
            include_co_authored_by: Some(false),
            permissions: Some(Permissions {
                allow: vec!["Bash(npm run lint)".to_string()],
                deny: vec!["Write(/etc/*)".to_string()],
                default_mode: Some("deny".to_string()),
            }),
            preferred_notif_channel: Some("chat".to_string()),
            mcp_servers: None,
            extra: HashMap::new(),
        };

        let json = serde_json::to_value(&settings).unwrap();

        assert_eq!(json.get("apiKeyHelper").unwrap(), "/bin/generate_api_key.sh");
        assert_eq!(json.get("cleanupPeriodDays").unwrap(), 20);
        assert_eq!(json.get("env").unwrap().get("FOO").unwrap(), "bar");
        assert_eq!(json.get("includeCoAuthoredBy").unwrap(), false);
        assert_eq!(
            json.get("permissions").unwrap().get("allow").unwrap().get(0).unwrap(),
            "Bash(npm run lint)"
        );
        assert_eq!(
            json.get("permissions").unwrap().get("deny").unwrap().get(0).unwrap(),
            "Write(/etc/*)"
        );
    }

    #[test]
    fn test_settings_deserialization() {
        let json = r#"{
        "apiKeyHelper": "/bin/generate_api_key.sh",
        "cleanupPeriodDays": 20,
        "env": {"FOO": "bar"},
        "includeCoAuthoredBy": false,
        "permissions": {
            "allow": ["Bash(npm run lint)"],
            "deny": []
        }
        }"#;

        let settings: Settings = serde_json::from_str(json).unwrap();

        assert_eq!(settings.api_key_helper, Some("/bin/generate_api_key.sh".to_string()));
        assert_eq!(settings.cleanup_period_days, Some(20));
        assert_eq!(settings.env.as_ref().unwrap().get("FOO"), Some(&"bar".to_string()));
        assert_eq!(settings.include_co_authored_by, Some(false));
        assert_eq!(settings.permissions.as_ref().unwrap().allow.len(), 1);
        assert_eq!(settings.permissions.as_ref().unwrap().deny.len(), 0);
        assert!(settings.permissions.as_ref().unwrap().default_mode.is_none());
        assert!(settings.preferred_notif_channel.is_none());
    }

    #[test]
    fn test_partial_settings() {
        let json = r#"{
        "apiKeyHelper": "/bin/generate_api_key.sh",
        "cleanupPeriodDays": 20
        }"#;

        let settings: Settings = serde_json::from_str(json).unwrap();

        assert_eq!(settings.api_key_helper, Some("/bin/generate_api_key.sh".to_string()));
        assert_eq!(settings.cleanup_period_days, Some(20));
        assert!(settings.env.is_none());
        assert!(settings.include_co_authored_by.is_none());
        assert!(settings.permissions.is_none());
        assert!(settings.preferred_notif_channel.is_none());
    }

    #[test]
    fn test_read_settings_file() {
        let temp_file = assert_fs::NamedTempFile::new("settings.json").unwrap();
        temp_file
            .write_str(
                r#"{
        "apiKeyHelper": "/bin/script.sh",
        "cleanupPeriodDays": 30,
        "env": {
            "API_URL": "https://api.example.com"
        }
            }"#,
            )
            .unwrap();

        let settings = read_settings(temp_file.path()).unwrap();
        assert!(settings.is_some());

        let settings_data = settings.unwrap();
        assert_eq!(settings_data.api_key_helper, Some("/bin/script.sh".to_string()));
        assert_eq!(settings_data.cleanup_period_days, Some(30));
        assert_eq!(
            settings_data.env.as_ref().unwrap().get("API_URL"),
            Some(&"https://api.example.com".to_string())
        );
    }

    #[test]
    fn test_read_settings_nonexistent_file() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let settings_path = temp_dir.path().join("nonexistent.json");

        let result = read_settings(&settings_path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_merge_settings_into_claude_config() {
        let mut claude_config = ClaudeConfig {
            mcp_servers: None,
            other: HashMap::from([("existingKey".to_string(), serde_json::json!("existingValue"))]),
        };

        let settings = Settings {
            api_key_helper: Some("/bin/generate_api_key.sh".to_string()),
            cleanup_period_days: Some(20),
            env: Some(HashMap::from([("FOO".to_string(), "bar".to_string())])),
            include_co_authored_by: Some(false),
            permissions: Some(Permissions {
                allow: vec!["Bash(npm run lint)".to_string()],
                deny: vec![],
                default_mode: None,
            }),
            preferred_notif_channel: None,
            mcp_servers: None,
            extra: HashMap::new(),
        };

        merge_settings(&mut claude_config, &settings).unwrap();

        // Check existing key preserved
        assert_eq!(claude_config.other.get("existingKey").unwrap(), "existingValue");

        // Check settings merged
        assert_eq!(claude_config.other.get("apiKeyHelper").unwrap(), "/bin/generate_api_key.sh");
        assert_eq!(claude_config.other.get("cleanupPeriodDays").unwrap(), 20);
        assert_eq!(claude_config.other.get("env").unwrap().get("FOO").unwrap(), "bar");
        assert_eq!(claude_config.other.get("includeCoAuthoredBy").unwrap(), false);
        assert_eq!(
            claude_config
                .other
                .get("permissions")
                .unwrap()
                .get("allow")
                .unwrap()
                .get(0)
                .unwrap(),
            "Bash(npm run lint)"
        );
    }

    #[test]
    fn test_merge_partial_settings() {
        let mut claude_config = ClaudeConfig {
            mcp_servers: None,
            other: HashMap::from([
                ("apiKeyHelper".to_string(), serde_json::json!("/old/script.sh")),
                ("existingKey".to_string(), serde_json::json!("keep")),
            ]),
        };

        let settings = Settings {
            api_key_helper: Some("/new/script.sh".to_string()),
            cleanup_period_days: Some(15),
            env: None,
            include_co_authored_by: None,
            permissions: None,
            preferred_notif_channel: None,
            mcp_servers: None,
            extra: HashMap::new(),
        };

        merge_settings(&mut claude_config, &settings).unwrap();

        // Check settings overwritten
        assert_eq!(claude_config.other.get("apiKeyHelper").unwrap(), "/new/script.sh");
        assert_eq!(claude_config.other.get("cleanupPeriodDays").unwrap(), 15);

        // Check existing key preserved
        assert_eq!(claude_config.other.get("existingKey").unwrap(), "keep");

        // Check no extra keys added
        assert!(!claude_config.other.contains_key("env"));
        assert!(!claude_config.other.contains_key("includeCoAuthoredBy"));
        assert!(!claude_config.other.contains_key("permissions"));
    }

    #[test]
    fn test_preferred_notif_channel() {
        // Test deserialization
        let json = r#"{
        "apiKeyHelper": "/bin/script.sh",
        "preferredNotifChannel": "email"
        }"#;

        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.preferred_notif_channel, Some("email".to_string()));

        // Test merge into claude config
        let mut claude_config = ClaudeConfig { mcp_servers: None, other: HashMap::new() };

        merge_settings(&mut claude_config, &settings).unwrap();
        assert_eq!(claude_config.other.get("preferredNotifChannel").unwrap(), "email");
    }

    #[test]
    fn test_permissions_default_mode() {
        // Test deserialization with defaultMode
        let json = r#"{
        "permissions": {
            "allow": ["Bash(npm test)"],
            "deny": ["Write(/*)"],
            "defaultMode": "deny"
        }
        }"#;

        let settings: Settings = serde_json::from_str(json).unwrap();
        let perms = settings.permissions.unwrap();
        assert_eq!(perms.allow, vec!["Bash(npm test)"]);
        assert_eq!(perms.deny, vec!["Write(/*)"]);
        assert_eq!(perms.default_mode, Some("deny".to_string()));

        // Test serialization preserves defaultMode
        let serialized = serde_json::to_value(&Settings {
            api_key_helper: None,
            cleanup_period_days: None,
            env: None,
            include_co_authored_by: None,
            permissions: Some(Permissions {
                allow: vec![],
                deny: vec![],
                default_mode: Some("allow".to_string()),
            }),
            preferred_notif_channel: None,
            mcp_servers: None,
            extra: HashMap::new(),
        })
        .unwrap();

        assert_eq!(serialized.get("permissions").unwrap().get("defaultMode").unwrap(), "allow");
    }
}
