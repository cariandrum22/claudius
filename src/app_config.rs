use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct AppConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_manager: Option<SecretManagerConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<DefaultConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DefaultConfig {
    pub agent: Agent,
    #[serde(skip_serializing_if = "Option::is_none", rename = "context-file")]
    pub context_file: Option<String>,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SecretManagerConfig {
    #[serde(rename = "type")]
    pub manager_type: SecretManagerType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SecretManagerType {
    Vault,
    #[serde(rename = "1password")]
    OnePassword, // Represents 1Password
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
            serde_json::to_string(&Agent::ClaudeCode).expect("Failed to serialize Agent::ClaudeCode"),
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
            }),
            default: Some(DefaultConfig {
                agent: Agent::Claude,
                context_file: Some("CUSTOM.md".to_string()),
            }),
        };

        let toml_str = toml::to_string(&config).expect("Failed to serialize AppConfig");
        assert!(toml_str.contains("[secret-manager]"));
        assert!(toml_str.contains("type = \"1password\""));
        assert!(toml_str.contains("[default]"));
        assert!(toml_str.contains("agent = \"claude\""));
        assert!(toml_str.contains("context-file = \"CUSTOM.md\""));
    }

    #[test]
    fn test_app_config_deserialization() {
        let toml_str = r#"
[secret-manager]
type = "1password"

[default]
agent = "codex"
context-file = "AGENTS.md"
"#;

        let config: AppConfig = toml::from_str(toml_str).expect("Failed to deserialize AppConfig");
        assert!(config.secret_manager.is_some());
        assert_eq!(
            config.secret_manager.expect("Secret manager should be present").manager_type,
            SecretManagerType::OnePassword
        );
        assert!(config.default.is_some());
        let default = config.default.expect("Default config should be present");
        assert_eq!(default.agent, Agent::Codex);
        assert_eq!(default.context_file, Some("AGENTS.md".to_string()));
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
