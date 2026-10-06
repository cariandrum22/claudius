use std::path::{Path, PathBuf};

const CLAUDE_CODE_MANAGED_DIR_ENV: &str = "CLAUDIUS_CLAUDE_CODE_MANAGED_DIR";
const CODEX_REQUIREMENTS_PATH_ENV: &str = "CLAUDIUS_CODEX_REQUIREMENTS_PATH";
const CODEX_MANAGED_CONFIG_PATH_ENV: &str = "CLAUDIUS_CODEX_MANAGED_CONFIG_PATH";
const GEMINI_CLI_SYSTEM_SETTINGS_PATH_ENV: &str = "GEMINI_CLI_SYSTEM_SETTINGS_PATH";
const GEMINI_CLI_SYSTEM_DEFAULTS_PATH_ENV: &str = "GEMINI_CLI_SYSTEM_DEFAULTS_PATH";
const OPENCODE_CONFIG_DIR_ENV: &str = "OPENCODE_CONFIG_DIR";
const XDG_CONFIG_HOME_ENV: &str = "XDG_CONFIG_HOME";

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

#[must_use]
pub fn gemini_cli_system_defaults_path() -> PathBuf {
    std::env::var(GEMINI_CLI_SYSTEM_DEFAULTS_PATH_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map_or_else(default_gemini_cli_system_defaults_path, PathBuf::from)
}

/// Resolve `OpenCode`'s global config directory the way `OpenCode` does:
/// `OPENCODE_CONFIG_DIR`, then `$XDG_CONFIG_HOME/opencode`, then
/// `~/.config/opencode` (XDG is used on macOS too).
#[must_use]
pub fn opencode_config_dir(home_dir: &Path) -> PathBuf {
    non_empty_env(OPENCODE_CONFIG_DIR_ENV).map_or_else(
        || {
            non_empty_env(XDG_CONFIG_HOME_ENV)
                .map_or_else(|| home_dir.join(".config"), PathBuf::from)
                .join("opencode")
        },
        PathBuf::from,
    )
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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
    PathBuf::from(r"C:\ProgramData\OpenAI\Codex\requirements.toml")
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

#[cfg(target_os = "macos")]
fn default_gemini_cli_system_defaults_path() -> PathBuf {
    PathBuf::from("/Library/Application Support/GeminiCli/system-defaults.json")
}

#[cfg(target_os = "linux")]
fn default_gemini_cli_system_defaults_path() -> PathBuf {
    PathBuf::from("/etc/gemini-cli/system-defaults.json")
}

#[cfg(windows)]
fn default_gemini_cli_system_defaults_path() -> PathBuf {
    PathBuf::from(r"C:\ProgramData\gemini-cli\system-defaults.json")
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn default_gemini_cli_system_defaults_path() -> PathBuf {
    PathBuf::from("/etc/gemini-cli/system-defaults.json")
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

    #[test]
    #[serial]
    fn test_opencode_config_dir_resolution_order() {
        let home = TempDir::new().expect("temp dir should be created for test");
        let original_xdg = std::env::var_os(XDG_CONFIG_HOME_ENV);
        let original_opencode = std::env::var_os(OPENCODE_CONFIG_DIR_ENV);

        std::env::remove_var(OPENCODE_CONFIG_DIR_ENV);
        std::env::remove_var(XDG_CONFIG_HOME_ENV);
        assert_eq!(opencode_config_dir(home.path()), home.path().join(".config").join("opencode"));

        std::env::set_var(XDG_CONFIG_HOME_ENV, home.path().join("xdg"));
        assert_eq!(opencode_config_dir(home.path()), home.path().join("xdg").join("opencode"));

        std::env::set_var(OPENCODE_CONFIG_DIR_ENV, home.path().join("custom"));
        assert_eq!(opencode_config_dir(home.path()), home.path().join("custom"));

        match original_xdg {
            Some(value) => std::env::set_var(XDG_CONFIG_HOME_ENV, value),
            None => std::env::remove_var(XDG_CONFIG_HOME_ENV),
        }
        match original_opencode {
            Some(value) => std::env::set_var(OPENCODE_CONFIG_DIR_ENV, value),
            None => std::env::remove_var(OPENCODE_CONFIG_DIR_ENV),
        }
    }

    #[test]
    #[serial]
    fn test_gemini_cli_system_defaults_path_env_override() {
        let temp_dir = TempDir::new().expect("temp dir should be created for test");
        let path = temp_dir.path().join("system-defaults.json");
        std::env::set_var(GEMINI_CLI_SYSTEM_DEFAULTS_PATH_ENV, &path);

        assert_eq!(gemini_cli_system_defaults_path(), path);

        std::env::remove_var(GEMINI_CLI_SYSTEM_DEFAULTS_PATH_ENV);
    }
}
