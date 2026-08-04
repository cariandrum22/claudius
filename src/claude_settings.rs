use serde_json::Value;

use crate::app_config::ClaudeCodeScope;
use crate::validation::Diagnostic;

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

/// Report settings that Claude Code explicitly documents as ignored outside
/// particular configuration scopes.
///
/// This is intentionally not a complete field catalog. Only documented scope
/// restrictions are encoded so new upstream settings remain forward-compatible.
#[must_use]
pub fn validate_claude_settings_scope(settings: &Value, scope: ClaudeCodeScope) -> Vec<Diagnostic> {
    let Value::Object(fields) = settings else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    if matches!(scope, ClaudeCodeScope::Project | ClaudeCodeScope::Local) {
        for field in ["askUserQuestionTimeout", "autoMode"] {
            if fields.contains_key(field) {
                diagnostics.push(Diagnostic::warning(format!(
                    "{field} is ignored by Claude Code in {} scope; move it to user or managed settings",
                    claude_scope_name(scope),
                )));
            }
        }
    }

    if scope != ClaudeCodeScope::Managed {
        for field in [
            "allowAllClaudeAiMcps",
            "allowedChannelPlugins",
            "allowManagedHooksOnly",
            "allowManagedMcpServersOnly",
            "allowManagedPermissionRulesOnly",
            "claudeMd",
        ] {
            if fields.contains_key(field) {
                diagnostics.push(Diagnostic::warning(format!(
                    "{field} is a managed-only Claude Code setting and is ignored in {} scope",
                    claude_scope_name(scope),
                )));
            }
        }
    }

    diagnostics
}

const fn claude_scope_name(scope: ClaudeCodeScope) -> &'static str {
    match scope {
        ClaudeCodeScope::Managed => "managed",
        ClaudeCodeScope::User => "user",
        ClaudeCodeScope::Project => "project",
        ClaudeCodeScope::Local => "local",
    }
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

    #[test]
    fn project_scope_warns_only_for_documented_restrictions() {
        let diagnostics = validate_claude_settings_scope(
            &json!({
                "askUserQuestionTimeout": "5m",
                "autoMode": {"allow": ["$defaults"]},
                "futureSetting": true
            }),
            ClaudeCodeScope::Project,
        );

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().all(|diagnostic| !diagnostic.contains("futureSetting")));
    }

    #[test]
    fn managed_only_setting_warns_in_user_scope() {
        let diagnostics = validate_claude_settings_scope(
            &json!({"allowManagedHooksOnly": true}),
            ClaudeCodeScope::User,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics
            .first()
            .is_some_and(|diagnostic| diagnostic.contains("managed-only")));
    }

    #[test]
    fn documented_scope_settings_are_allowed_in_managed_scope() {
        let diagnostics = validate_claude_settings_scope(
            &json!({"autoMode": {}, "claudeMd": "Follow policy"}),
            ClaudeCodeScope::Managed,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn mcp_allowlist_is_valid_outside_managed_scope() {
        let diagnostics = validate_claude_settings_scope(
            &json!({"allowedMcpServers": [{"serverName": "github"}]}),
            ClaudeCodeScope::User,
        );

        assert!(diagnostics.is_empty());
    }
}
