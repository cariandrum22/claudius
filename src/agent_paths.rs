use std::path::PathBuf;

const CLAUDE_CODE_MANAGED_DIR_ENV: &str = "CLAUDIUS_CLAUDE_CODE_MANAGED_DIR";
const CODEX_REQUIREMENTS_PATH_ENV: &str = "CLAUDIUS_CODEX_REQUIREMENTS_PATH";
const CODEX_MANAGED_CONFIG_PATH_ENV: &str = "CLAUDIUS_CODEX_MANAGED_CONFIG_PATH";
const GEMINI_CLI_SYSTEM_SETTINGS_PATH_ENV: &str = "GEMINI_CLI_SYSTEM_SETTINGS_PATH";

#[must_use]
pub fn claude_code_managed_dir() -> PathBuf {
    std::env::var(CLAUDE_CODE_MANAGED_DIR_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map_or_else(default_claude_code_managed_dir, PathBuf::from)
}

#[must_use]
pub fn claude_code_managed_settings_path() -> PathBuf {
    claude_code_managed_dir().join("managed-settings.json")
}

#[must_use]
pub fn claude_code_managed_mcp_path() -> PathBuf {
    claude_code_managed_dir().join("managed-mcp.json")
}

#[must_use]
pub fn codex_requirements_path() -> PathBuf {
    std::env::var(CODEX_REQUIREMENTS_PATH_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map_or_else(default_codex_requirements_path, PathBuf::from)
}

#[must_use]
pub fn codex_managed_config_path() -> PathBuf {
    std::env::var(CODEX_MANAGED_CONFIG_PATH_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map_or_else(default_codex_managed_config_path, PathBuf::from)
}

#[must_use]
pub fn gemini_cli_system_settings_path() -> PathBuf {
    std::env::var(GEMINI_CLI_SYSTEM_SETTINGS_PATH_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map_or_else(default_gemini_cli_system_settings_path, PathBuf::from)
}

#[cfg(target_os = "macos")]
fn default_claude_code_managed_dir() -> PathBuf {
    PathBuf::from("/Library/Application Support/ClaudeCode")
}

#[cfg(target_os = "linux")]
fn default_claude_code_managed_dir() -> PathBuf {
    PathBuf::from("/etc/claude-code")
}

#[cfg(windows)]
fn default_claude_code_managed_dir() -> PathBuf {
    PathBuf::from(r"C:\Program Files\ClaudeCode")
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn default_claude_code_managed_dir() -> PathBuf {
    PathBuf::from("/etc/claude-code")
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn default_codex_requirements_path() -> PathBuf {
    PathBuf::from("/etc/codex/requirements.toml")
}

#[cfg(windows)]
fn default_codex_requirements_path() -> PathBuf {
    PathBuf::from(r"C:\ProgramData\codex\requirements.toml")
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn default_codex_requirements_path() -> PathBuf {
    PathBuf::from("/etc/codex/requirements.toml")
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn default_codex_managed_config_path() -> PathBuf {
    PathBuf::from("/etc/codex/managed_config.toml")
}

#[cfg(windows)]
fn default_codex_managed_config_path() -> PathBuf {
    directories::BaseDirs::new().map_or_else(
        || PathBuf::from(r"C:\ProgramData\codex\managed_config.toml"),
        |dirs| dirs.home_dir().join(".codex").join("managed_config.toml"),
    )
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn default_codex_managed_config_path() -> PathBuf {
    directories::BaseDirs::new().map_or_else(
        || PathBuf::from("/etc/codex/managed_config.toml"),
        |dirs| dirs.home_dir().join(".codex").join("managed_config.toml"),
    )
}

#[cfg(target_os = "macos")]
fn default_gemini_cli_system_settings_path() -> PathBuf {
    PathBuf::from("/Library/Application Support/GeminiCli/settings.json")
}

#[cfg(target_os = "linux")]
fn default_gemini_cli_system_settings_path() -> PathBuf {
    PathBuf::from("/etc/gemini-cli/settings.json")
}

#[cfg(windows)]
fn default_gemini_cli_system_settings_path() -> PathBuf {
    PathBuf::from(r"C:\ProgramData\gemini-cli\settings.json")
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn default_gemini_cli_system_settings_path() -> PathBuf {
    PathBuf::from("/etc/gemini-cli/settings.json")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::TempDir;

    #[test]
    #[serial]
    fn test_claude_code_managed_dir_env_override() {
        let temp_dir = TempDir::new().expect("temp dir should be created for test");
        std::env::set_var(CLAUDE_CODE_MANAGED_DIR_ENV, temp_dir.path());

        assert_eq!(claude_code_managed_dir(), temp_dir.path());
        assert_eq!(
            claude_code_managed_settings_path(),
            temp_dir.path().join("managed-settings.json")
        );
        assert_eq!(claude_code_managed_mcp_path(), temp_dir.path().join("managed-mcp.json"));

        std::env::remove_var(CLAUDE_CODE_MANAGED_DIR_ENV);
    }

    #[test]
    #[serial]
    fn test_codex_requirements_path_env_override() {
        let temp_dir = TempDir::new().expect("temp dir should be created for test");
        let path = temp_dir.path().join("requirements.toml");
        std::env::set_var(CODEX_REQUIREMENTS_PATH_ENV, &path);

        assert_eq!(codex_requirements_path(), path);

        std::env::remove_var(CODEX_REQUIREMENTS_PATH_ENV);
    }

    #[test]
    #[serial]
    fn test_codex_managed_config_path_env_override() {
        let temp_dir = TempDir::new().expect("temp dir should be created for test");
        let path = temp_dir.path().join("managed_config.toml");
        std::env::set_var(CODEX_MANAGED_CONFIG_PATH_ENV, &path);

        assert_eq!(codex_managed_config_path(), path);

        std::env::remove_var(CODEX_MANAGED_CONFIG_PATH_ENV);
    }

    #[test]
    #[serial]
    fn test_gemini_cli_system_settings_path_env_override() {
        let temp_dir = TempDir::new().expect("temp dir should be created for test");
        let path = temp_dir.path().join("settings.json");
        std::env::set_var(GEMINI_CLI_SYSTEM_SETTINGS_PATH_ENV, &path);

        assert_eq!(gemini_cli_system_settings_path(), path);

        std::env::remove_var(GEMINI_CLI_SYSTEM_SETTINGS_PATH_ENV);
    }
}
