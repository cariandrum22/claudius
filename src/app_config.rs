use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, str::FromStr};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct AppConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_manager: Option<SecretManagerConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<DefaultConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codex: Option<CodexConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DefaultConfig {
    pub agent: Agent,
    #[serde(skip_serializing_if = "Option::is_none", rename = "context-file")]
    pub context_file: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct CodexConfig {
    #[serde(skip_serializing_if = "Option::is_none", rename = "skill-target")]
    pub skill_target: Option<CodexSkillTargetMode>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Agent {
    Claude,
    ClaudeCode,
    Codex,
    Gemini,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum ClaudeCodeScope {
    /// System-level managed configuration (managed-settings.json / managed-mcp.json).
    Managed,
    /// User configuration in the home directory (~/.claude/*).
    User,
    /// Project configuration committed with the repo (.claude/*, .mcp.json).
    Project,
    /// Local (per-repo, per-user) configuration (.claude/*.local.* and ~/.claude.json per-project).
    Local,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum CodexSkillTargetMode {
    /// Follow Claudius's default Codex skill target (`.agents/skills`).
    #[default]
    Auto,
    /// Publish only to the legacy `.codex/skills` directory.
    Codex,
    /// Publish only to the official `.agents/skills` directory.
    Agents,
    /// Publish to both the official `.agents/skills` and legacy `.codex/skills` directories.
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SecretManagerConfig {
    #[serde(rename = "type")]
    pub manager_type: SecretManagerType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onepassword: Option<OnePasswordConfig>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SecretManagerType {
    Vault,
    #[serde(rename = "1password")]
    OnePassword, // Represents 1Password
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct OnePasswordConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<OnePasswordMode>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "service-account-token-path")]
    pub service_account_token_path: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OnePasswordMode {
    Desktop,
    Manual,
    ServiceAccount,
}

impl FromStr for OnePasswordMode {
    type Err = &'static str;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "desktop" => Ok(Self::Desktop),
            "manual" => Ok(Self::Manual),
            "service-account" => Ok(Self::ServiceAccount),
            _ => Err("expected one of: desktop, manual, service-account"),
        }
    }
}

impl AppConfig {
    /// Load the application configuration from the default path
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Unable to determine the config directory
    /// - Unable to read the config file (other than it not existing)
    /// - The config file contains invalid TOML
    pub fn load() -> Result<Option<Self>> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file at {}", config_path.display()))?;

        let config: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse TOML config at {}", config_path.display()))?;

        Ok(Some(config))
    }

    /// Get the path to the configuration file
    ///
    /// # Errors
    ///
    /// Returns an error if unable to determine the config directory
    pub fn config_path() -> Result<PathBuf> {
        if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
            Ok(PathBuf::from(config_home).join("claudius").join("config.toml"))
        } else if let Some(proj_dirs) = ProjectDirs::from("", "", "claudius") {
            Ok(proj_dirs.config_dir().join("config.toml"))
        } else {
            anyhow::bail!("Could not determine config directory")
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_agent_serialization() {
        assert_eq!(
            serde_json::to_string(&Agent::Claude).expect("Failed to serialize Agent::Claude"),
            "\"claude\""
        );
        assert_eq!(
            serde_json::to_string(&Agent::ClaudeCode)
                .expect("Failed to serialize Agent::ClaudeCode"),
            "\"claude-code\""
        );
        assert_eq!(
            serde_json::to_string(&Agent::Codex).expect("Failed to serialize Agent::Codex"),
            "\"codex\""
        );
        assert_eq!(
            serde_json::to_string(&Agent::Gemini).expect("Failed to serialize Agent::Gemini"),
            "\"gemini\""
        );
    }

    #[test]
    fn test_agent_deserialization() {
        assert_eq!(
            serde_json::from_str::<Agent>("\"claude\"")
                .expect("Failed to deserialize Agent::Claude"),
            Agent::Claude
        );
        assert_eq!(
            serde_json::from_str::<Agent>("\"claude-code\"")
                .expect("Failed to deserialize Agent::ClaudeCode"),
            Agent::ClaudeCode
        );
        assert_eq!(
            serde_json::from_str::<Agent>("\"codex\"").expect("Failed to deserialize Agent::Codex"),
            Agent::Codex
        );
        assert_eq!(
            serde_json::from_str::<Agent>("\"gemini\"")
                .expect("Failed to deserialize Agent::Gemini"),
            Agent::Gemini
        );
    }

    #[test]
    fn test_codex_skill_target_mode_serialization() {
        assert_eq!(
            serde_json::to_string(&CodexSkillTargetMode::Auto)
                .expect("Failed to serialize CodexSkillTargetMode::Auto"),
            "\"auto\""
        );
        assert_eq!(
            serde_json::to_string(&CodexSkillTargetMode::Codex)
                .expect("Failed to serialize CodexSkillTargetMode::Codex"),
            "\"codex\""
        );
        assert_eq!(
            serde_json::to_string(&CodexSkillTargetMode::Agents)
                .expect("Failed to serialize CodexSkillTargetMode::Agents"),
            "\"agents\""
        );
        assert_eq!(
            serde_json::to_string(&CodexSkillTargetMode::Both)
                .expect("Failed to serialize CodexSkillTargetMode::Both"),
            "\"both\""
        );
    }

    #[test]
    fn test_codex_skill_target_mode_deserialization() {
        assert_eq!(
            serde_json::from_str::<CodexSkillTargetMode>("\"auto\"")
                .expect("Failed to deserialize CodexSkillTargetMode::Auto"),
            CodexSkillTargetMode::Auto
        );
        assert_eq!(
            serde_json::from_str::<CodexSkillTargetMode>("\"codex\"")
                .expect("Failed to deserialize CodexSkillTargetMode::Codex"),
            CodexSkillTargetMode::Codex
        );
        assert_eq!(
            serde_json::from_str::<CodexSkillTargetMode>("\"agents\"")
                .expect("Failed to deserialize CodexSkillTargetMode::Agents"),
            CodexSkillTargetMode::Agents
        );
        assert_eq!(
            serde_json::from_str::<CodexSkillTargetMode>("\"both\"")
                .expect("Failed to deserialize CodexSkillTargetMode::Both"),
            CodexSkillTargetMode::Both
        );
    }

    #[test]
    fn test_secret_manager_type_serialization() {
        assert_eq!(
            serde_json::to_string(&SecretManagerType::Vault)
                .expect("Failed to serialize SecretManagerType::Vault"),
            "\"vault\""
        );
        assert_eq!(
            serde_json::to_string(&SecretManagerType::OnePassword)
                .expect("Failed to serialize SecretManagerType::OnePassword"),
            "\"1password\""
        );
    }

    #[test]
    fn test_secret_manager_type_deserialization() {
        assert_eq!(
            serde_json::from_str::<SecretManagerType>("\"vault\"")
                .expect("Failed to deserialize SecretManagerType::Vault"),
            SecretManagerType::Vault
        );
        assert_eq!(
            serde_json::from_str::<SecretManagerType>("\"1password\"")
                .expect("Failed to deserialize SecretManagerType::OnePassword"),
            SecretManagerType::OnePassword
        );
    }

    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert!(config.secret_manager.is_none());
        assert!(config.default.is_none());
    }

    #[test]
    fn test_app_config_serialization() {
        let config = AppConfig {
            secret_manager: Some(SecretManagerConfig {
                manager_type: SecretManagerType::OnePassword,
                onepassword: Some(OnePasswordConfig {
                    mode: Some(OnePasswordMode::ServiceAccount),
                    service_account_token_path: Some(
                        "~/.config/op/service-accounts/headless-linux-cli.token".to_string(),
                    ),
                }),
            }),
            default: Some(DefaultConfig {
                agent: Agent::Claude,
                context_file: Some("CUSTOM.md".to_string()),
            }),
            codex: Some(CodexConfig { skill_target: Some(CodexSkillTargetMode::Agents) }),
        };

        let toml_str = toml::to_string(&config).expect("Failed to serialize AppConfig");
        assert!(toml_str.contains("[secret-manager]"));
        assert!(toml_str.contains("type = \"1password\""));
        assert!(toml_str.contains("[secret-manager.onepassword]"));
        assert!(toml_str.contains("mode = \"service-account\""));
        assert!(toml_str.contains("service-account-token-path"));
        assert!(toml_str.contains("[default]"));
        assert!(toml_str.contains("agent = \"claude\""));
        assert!(toml_str.contains("context-file = \"CUSTOM.md\""));
        assert!(toml_str.contains("[codex]"));
        assert!(toml_str.contains("skill-target = \"agents\""));
    }

    #[test]
    fn test_app_config_deserialization() {
        let toml_str = r#"
[secret-manager]
type = "1password"

[secret-manager.onepassword]
mode = "service-account"
service-account-token-path = "~/.config/op/service-accounts/headless-linux-cli.token"

[default]
agent = "codex"
context-file = "AGENTS.md"

[codex]
skill-target = "both"
"#;

        let config: AppConfig = toml::from_str(toml_str).expect("Failed to deserialize AppConfig");
        assert!(config.secret_manager.is_some());
        let secret_manager = config.secret_manager.expect("Secret manager should be present");
        assert_eq!(secret_manager.manager_type, SecretManagerType::OnePassword);
        assert_eq!(
            secret_manager.onepassword,
            Some(OnePasswordConfig {
                mode: Some(OnePasswordMode::ServiceAccount),
                service_account_token_path: Some(
                    "~/.config/op/service-accounts/headless-linux-cli.token".to_string(),
                ),
            })
        );
        assert!(config.default.is_some());
        let default = config.default.expect("Default config should be present");
        assert_eq!(default.agent, Agent::Codex);
        assert_eq!(default.context_file, Some("AGENTS.md".to_string()));
        assert_eq!(
            config.codex.expect("Codex config should be present").skill_target,
            Some(CodexSkillTargetMode::Both)
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_config_path_with_xdg() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        let path = AppConfig::config_path().expect("Failed to get config path");
        assert_eq!(path, temp_dir.path().join("claudius").join("config.toml"));

        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    #[serial_test::serial]
    fn test_config_path_without_xdg() {
        std::env::remove_var("XDG_CONFIG_HOME");
        let path = AppConfig::config_path().expect("Failed to get config path");
        assert!(path.to_string_lossy().contains("claudius"));
        assert!(path.to_string_lossy().ends_with("config.toml"));
    }

    #[test]
    #[serial_test::serial]
    fn test_load_missing_config() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        let result = AppConfig::load().expect("Failed to load AppConfig");
        assert!(result.is_none());

        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    #[serial_test::serial]
    fn test_load_valid_config() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).expect("Failed to create config directory");

        let config_content = r#"
[secret-manager]
type = "vault"

[default]
agent = "gemini"
"#;

        fs::write(config_dir.join("config.toml"), config_content)
            .expect("Failed to write config file");
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        let config = AppConfig::load()
            .expect("Failed to load AppConfig")
            .expect("AppConfig should be present");
        assert_eq!(
            config.secret_manager.expect("Secret manager should be present").manager_type,
            SecretManagerType::Vault
        );
        assert_eq!(config.default.expect("Default config should be present").agent, Agent::Gemini);

        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    #[serial_test::serial]
    fn test_load_invalid_toml() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).expect("Failed to create config directory");

        fs::write(config_dir.join("config.toml"), "[invalid\nunclosed bracket")
            .expect("Failed to write invalid config");
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        let result = AppConfig::load();
        assert!(result.is_err());

        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    fn test_partial_config() {
        let toml_str = r#"
[default]
agent = "claude"
"#;

        let config: AppConfig = toml::from_str(toml_str).expect("Failed to deserialize AppConfig");
        assert!(config.secret_manager.is_none());
        assert_eq!(config.default.expect("Default config should be present").agent, Agent::Claude);
    }

    #[test]
    fn test_onepassword_mode_serialization() {
        assert_eq!(
            serde_json::to_string(&OnePasswordMode::Desktop)
                .expect("Failed to serialize OnePasswordMode::Desktop"),
            "\"desktop\""
        );
        assert_eq!(
            serde_json::to_string(&OnePasswordMode::Manual)
                .expect("Failed to serialize OnePasswordMode::Manual"),
            "\"manual\""
        );
        assert_eq!(
            serde_json::to_string(&OnePasswordMode::ServiceAccount)
                .expect("Failed to serialize OnePasswordMode::ServiceAccount"),
            "\"service-account\""
        );
    }

    #[test]
    fn test_onepassword_mode_deserialization() {
        assert_eq!(
            serde_json::from_str::<OnePasswordMode>("\"desktop\"")
                .expect("Failed to deserialize OnePasswordMode::Desktop"),
            OnePasswordMode::Desktop
        );
        assert_eq!(
            serde_json::from_str::<OnePasswordMode>("\"manual\"")
                .expect("Failed to deserialize OnePasswordMode::Manual"),
            OnePasswordMode::Manual
        );
        assert_eq!(
            serde_json::from_str::<OnePasswordMode>("\"service-account\"")
                .expect("Failed to deserialize OnePasswordMode::ServiceAccount"),
            OnePasswordMode::ServiceAccount
        );
    }

    #[test]
    fn test_onepassword_mode_from_str() {
        assert_eq!(
            "desktop".parse::<OnePasswordMode>().expect("desktop should parse"),
            OnePasswordMode::Desktop
        );
        assert_eq!(
            "manual".parse::<OnePasswordMode>().expect("manual should parse"),
            OnePasswordMode::Manual
        );
        assert_eq!(
            "service-account"
                .parse::<OnePasswordMode>()
                .expect("service-account should parse"),
            OnePasswordMode::ServiceAccount
        );
        assert!("invalid".parse::<OnePasswordMode>().is_err());
    }

    #[test]
    fn test_context_file_serialization() {
        let config = DefaultConfig { agent: Agent::Claude, context_file: None };

        let json = serde_json::to_string(&config).expect("Failed to serialize DefaultConfig");
        assert!(!json.contains("context-file"));

        let config_with_file =
            DefaultConfig { agent: Agent::Claude, context_file: Some("CUSTOM.md".to_string()) };

        let json_with_file = serde_json::to_string(&config_with_file)
            .expect("Failed to serialize DefaultConfig with file");
        assert!(json_with_file.contains("\"context-file\":\"CUSTOM.md\""));
    }
}
