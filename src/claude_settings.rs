use serde_json::Value;

/// Validate Claude Code settings for known compatibility concerns.
///
/// Unknown fields are intentionally preserved without warnings because Claude Code evolves faster
/// than Claudius and its published JSON schema can lag behind the CLI.
#[must_use]
pub fn validate_claude_settings(settings: &Value) -> Vec<String> {
    let Value::Object(fields) = settings else {
        return Vec::new();
    };

    fields
        .contains_key("includeCoAuthoredBy")
        .then(|| {
            "includeCoAuthoredBy is deprecated; use attribution.commit, attribution.pr, and attribution.sessionUrl"
                .to_string()
        })
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn current_documented_settings_do_not_warn() {
        let settings = json!({
            "$schema": "https://json.schemastore.org/claude-code-settings.json",
            "attribution": {"commit": "", "pr": "", "sessionUrl": false},
            "effortLevel": "xhigh",
            "hooks": {},
            "model": "opus",
            "outputStyle": "default",
            "permissions": {
                "allow": ["Read"],
                "ask": ["Bash(git push *)"],
                "deny": ["Read(./.env)"],
                "additionalDirectories": ["../shared"],
                "disableBypassPermissionsMode": "disable"
            },
            "preferredNotifChannel": "terminal_bell",
            "statusLine": {"type": "command", "command": "statusline"}
        });

        assert!(validate_claude_settings(&settings).is_empty());
    }

    #[test]
    fn deprecated_attribution_setting_warns() {
        let warnings = validate_claude_settings(&json!({"includeCoAuthoredBy": false}));
        assert_eq!(warnings.len(), 1);
        assert!(warnings.first().is_some_and(|warning| warning.contains("attribution")));
    }

    #[test]
    fn future_settings_are_preserved_without_speculative_warnings() {
        assert!(validate_claude_settings(&json!({"futureSetting": true})).is_empty());
    }
}
