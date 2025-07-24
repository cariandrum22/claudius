use claudius::app_config::{Agent, AppConfig, SecretManagerType};
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to save and restore `XDG_CONFIG_HOME`
    struct EnvGuard {
        original: Option<String>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self { original: std::env::var("XDG_CONFIG_HOME").ok() }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
        }
    }

    #[test]
    #[serial]
    fn test_load_nonexistent_config() {
        let _guard = EnvGuard::new();

        // Set XDG_CONFIG_HOME to a temp directory
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        let result = AppConfig::load().unwrap();
        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn test_load_valid_config() {
        let _guard = EnvGuard::new();

        // Set XDG_CONFIG_HOME to a temp directory
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create config directory and file
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        let config_content = r#"
[secret-manager]
type = "1password"
"#;

        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        let result = AppConfig::load().unwrap();
        assert!(result.is_some());

        let config = result.unwrap();
        assert!(config.secret_manager.is_some());

        let secret_manager = config.secret_manager.unwrap();
        assert_eq!(secret_manager.manager_type, SecretManagerType::OnePassword);
    }

    #[test]
    #[serial]
    fn test_load_vault_config() {
        let _guard = EnvGuard::new();

        // Set XDG_CONFIG_HOME to a temp directory
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create config directory and file
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        let config_content = r#"
[secret-manager]
type = "vault"
"#;

        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        let result = AppConfig::load().unwrap();
        assert!(result.is_some());

        let config = result.unwrap();
        assert!(config.secret_manager.is_some());

        let secret_manager = config.secret_manager.unwrap();
        assert_eq!(secret_manager.manager_type, SecretManagerType::Vault);
    }

    #[test]
    #[serial]
    fn test_empty_config() {
        let _guard = EnvGuard::new();

        // Set XDG_CONFIG_HOME to a temp directory
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create config directory and empty file
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("config.toml"), "").unwrap();

        let result = AppConfig::load().unwrap();
        assert!(result.is_some());

        let config = result.unwrap();
        assert!(config.secret_manager.is_none());
    }

    #[test]
    #[serial]
    fn test_invalid_toml_config() {
        let _guard = EnvGuard::new();

        // Set XDG_CONFIG_HOME to a temp directory
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create config directory and invalid file
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("config.toml"), "invalid toml {{{").unwrap();

        let result = AppConfig::load();
        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn test_load_config_with_default_agent() {
        let _guard = EnvGuard::new();

        // Set XDG_CONFIG_HOME to a temp directory
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create config directory and file
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        let config_content = r#"
[default]
agent = "claude"
"#;

        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        let result = AppConfig::load().unwrap();
        assert!(result.is_some());

        let config = result.unwrap();
        assert!(config.default.is_some());

        let default_config = config.default.unwrap();
        assert_eq!(default_config.agent, Agent::Claude);
    }

    #[test]
    #[serial]
    fn test_load_config_with_codex_agent() {
        let _guard = EnvGuard::new();

        // Set XDG_CONFIG_HOME to a temp directory
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create config directory and file
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        let config_content = r#"
[default]
agent = "codex"
"#;

        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        let result = AppConfig::load().unwrap();
        assert!(result.is_some());

        let config = result.unwrap();
        assert!(config.default.is_some());

        let default_config = config.default.unwrap();
        assert_eq!(default_config.agent, Agent::Codex);
    }

    #[test]
    #[serial]
    fn test_load_config_with_gemini_agent() {
        let _guard = EnvGuard::new();

        // Set XDG_CONFIG_HOME to a temp directory
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create config directory and file
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        let config_content = r#"
[default]
agent = "gemini"
"#;

        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        let result = AppConfig::load().unwrap();
        assert!(result.is_some());

        let config = result.unwrap();
        assert!(config.default.is_some());

        let default_config = config.default.unwrap();
        assert_eq!(default_config.agent, Agent::Gemini);
    }

    #[test]
    #[serial]
    fn test_load_config_with_both_sections() {
        let _guard = EnvGuard::new();

        // Set XDG_CONFIG_HOME to a temp directory
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create config directory and file
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        let config_content = r#"
[default]
agent = "claude"

[secret-manager]
type = "1password"
"#;

        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        let result = AppConfig::load().unwrap();
        assert!(result.is_some());

        let config = result.unwrap();

        // Check default section
        assert!(config.default.is_some());
        let default_config = config.default.unwrap();
        assert_eq!(default_config.agent, Agent::Claude);

        // Check secret-manager section
        assert!(config.secret_manager.is_some());
        let secret_manager = config.secret_manager.unwrap();
        assert_eq!(secret_manager.manager_type, SecretManagerType::OnePassword);
    }
}
