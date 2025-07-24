use claudius::config::Config;
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_paths() {
        // Test project-local mode (without global flag)
        let config_local = Config::new(false).unwrap();
        assert!(config_local.target_config_path.ends_with(".mcp.json"));
        assert!(!config_local.target_config_path.to_string_lossy().contains("claude.json"));
        assert!(!config_local.is_global);

        // Check project settings path
        assert!(config_local.project_settings_path.is_some());
        let settings_path = config_local.project_settings_path.unwrap();
        assert!(settings_path.ends_with(".claude/settings.json"));

        // Test global mode (with global flag)
        let config_global = Config::new(true).unwrap();
        assert!(config_global.target_config_path.ends_with("claude.json"));
        assert!(!config_global.target_config_path.to_string_lossy().contains("mcp.json"));
        assert!(config_global.is_global);
        assert!(config_global.project_settings_path.is_none());
    }

    #[test]
    fn test_custom_paths() {
        // Test with custom paths
        let config = Config::with_paths(
            PathBuf::from("/custom/servers.json"),
            PathBuf::from("/custom/target.json")
        );
        assert_eq!(config.mcp_servers_path, PathBuf::from("/custom/servers.json"));
        assert_eq!(config.target_config_path, PathBuf::from("/custom/target.json"));
    }
}
