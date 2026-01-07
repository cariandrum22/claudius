use claudius::app_config::Agent;
use claudius::config::Config;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to save and restore environment variables
    struct EnvGuard {
        xdg: Option<String>,
        home: Option<String>,
        dir: Option<std::path::PathBuf>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self {
                xdg: std::env::var("XDG_CONFIG_HOME").ok(),
                home: std::env::var("HOME").ok(),
                dir: std::env::current_dir().ok(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // Restore XDG_CONFIG_HOME
            match &self.xdg {
                Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
            // Restore HOME
            match &self.home {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }
            // Restore current directory
            if let Some(dir) = &self.dir {
                let _ = std::env::set_current_dir(dir);
            }
        }
    }

    #[test]
    #[serial]
    fn test_config_with_claude_agent() {
        let _env_guard = EnvGuard::new();
        let temp_dir = TempDir::new().unwrap();

        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create the claudius config directory and mcpServers.json
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("mcpServers.json"), "{}").unwrap();

        // Create a project directory and change to it
        let project_dir = temp_dir.path().join("project");
        fs::create_dir_all(&project_dir).unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        let config = Config::new_with_agent(false, Some(Agent::Claude)).unwrap();
        assert!(config.settings_path.to_string_lossy().contains("claude.settings.json"));
    }

    #[test]
    #[serial]
    fn test_config_with_codex_agent() {
        let _env_guard = EnvGuard::new();
        let temp_dir = TempDir::new().unwrap();

        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create the claudius config directory and mcpServers.json
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("mcpServers.json"), "{}").unwrap();

        // Create a project directory and change to it
        let project_dir = temp_dir.path().join("project");
        fs::create_dir_all(&project_dir).unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        let config = Config::new_with_agent(false, Some(Agent::Codex)).unwrap();
        assert!(config.settings_path.to_string_lossy().contains("codex.settings.toml"));
    }

    #[test]
    #[serial]
    fn test_config_with_gemini_agent_local() {
        let _env_guard = EnvGuard::new();
        let temp_dir = TempDir::new().unwrap();

        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create the claudius config directory and mcpServers.json
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("mcpServers.json"), "{}").unwrap();

        // Create a project directory and change to it
        let project_dir = temp_dir.path().join("project");
        fs::create_dir_all(&project_dir).unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        let config = Config::new_with_agent(false, Some(Agent::Gemini)).unwrap();
        // In local mode, gemini uses settings from config dir
        assert!(config.settings_path.to_string_lossy().contains("gemini.settings.json"));
        // But project settings path should be ./gemini/settings.json
        assert!(config
            .project_settings_path
            .as_ref()
            .unwrap()
            .to_string_lossy()
            .contains("gemini/settings.json"));
        assert!(!config
            .project_settings_path
            .as_ref()
            .unwrap()
            .to_string_lossy()
            .contains(".claude"));
    }

    #[test]
    #[serial]
    fn test_config_with_gemini_agent_global() {
        let _env_guard = EnvGuard::new();
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        std::env::set_var("HOME", temp_dir.path());

        // Create the claudius config directory and mcpServers.json
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("mcpServers.json"), "{}").unwrap();

        let config = Config::new_with_agent(true, Some(Agent::Gemini)).unwrap();
        // In global mode, gemini reads from config_dir/gemini.settings.json
        assert!(config.settings_path.to_string_lossy().contains("gemini.settings.json"));
    }

    #[test]
    #[serial]
    fn test_config_without_agent_defaults_to_claude() {
        let _env_guard = EnvGuard::new();
        let temp_dir = TempDir::new().unwrap();

        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create the claudius config directory and mcpServers.json
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("mcpServers.json"), "{}").unwrap();

        // Create a project directory and change to it
        let project_dir = temp_dir.path().join("project");
        fs::create_dir_all(&project_dir).unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        let config = Config::new_with_agent(false, None).unwrap();
        // Should default to claude.settings.json when no agent is specified
        assert!(config.settings_path.to_string_lossy().contains("claude.settings.json"));
        assert!(!config.settings_path.to_string_lossy().contains("codex.settings.toml"));
        assert!(!config.settings_path.to_string_lossy().contains("gemini.settings.json"));
    }

    #[test]
    #[serial]
    fn test_global_config_with_agent() {
        let _env_guard = EnvGuard::new();
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());

        // Create the claudius config directory and mcpServers.json
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("mcpServers.json"), "{}").unwrap();

        let config = Config::new_with_agent(true, Some(Agent::Claude)).unwrap();
        // In global mode, Claude Desktop uses a dedicated config file in the system config dir
        assert!(
            config
                .target_config_path
                .file_name()
                .is_some_and(|n| n == "claude_desktop_config.json"),
            "Expected Claude Desktop global config to be claude_desktop_config.json"
        );
        // But the settings_path should still reflect the agent
        assert!(config.settings_path.to_string_lossy().contains("claude.settings.json"));
    }

    #[test]
    #[serial]
    fn test_global_config_with_claude_code_agent() {
        let _env_guard = EnvGuard::new();
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        std::env::set_var("HOME", temp_dir.path());

        // Create the claudius config directory and mcpServers.json
        let config_dir = temp_dir.path().join("claudius");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("mcpServers.json"), "{}").unwrap();

        let config = Config::new_with_agent(true, Some(Agent::ClaudeCode)).unwrap();
        assert!(config.target_config_path.to_string_lossy().contains(".claude.json"));
        assert!(config.settings_path.to_string_lossy().contains("claude.settings.json"));
    }
}
