use anyhow::Result;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to save and restore environment variables
    struct EnvGuard {
        xdg_config_home: Option<String>,
        home: Option<String>,
        current_dir: Option<std::path::PathBuf>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self {
                xdg_config_home: std::env::var("XDG_CONFIG_HOME").ok(),
                home: std::env::var("HOME").ok(),
                current_dir: std::env::current_dir().ok(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.xdg_config_home {
                Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }

            match &self.home {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }

            if let Some(dir) = &self.current_dir {
                let _ = std::env::set_current_dir(dir);
            }
        }
    }

    #[test]
    #[serial]
    fn test_gemini_system_settings_sync_writes_system_file() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let home_dir = temp_dir.path().join("home");
        let claudius_dir = config_dir.join("claudius");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&home_dir)?;
        fs::create_dir_all(&claudius_dir)?;

        std::env::set_var("XDG_CONFIG_HOME", &config_dir);
        std::env::set_var("HOME", &home_dir);

        fs::write(
            claudius_dir.join("mcpServers.json"),
            r#"{"mcpServers":{"filesystem":{"command":"node","args":["server.js"],"env":{}}}}"#,
        )?;
        fs::write(claudius_dir.join("gemini.settings.json"), r#"{"general":{"vimMode":true}}"#)?;

        let system_settings_path =
            temp_dir.path().join("etc").join("gemini-cli").join("settings.json");

        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .env("GEMINI_CLI_SYSTEM_SETTINGS_PATH", &system_settings_path)
            .args(["config", "sync", "--global", "--agent", "gemini", "--gemini-system"])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            anyhow::bail!("sync command failed");
        }

        anyhow::ensure!(
            system_settings_path.exists(),
            "Expected Gemini system settings file to be written",
        );
        anyhow::ensure!(
            !home_dir.join(".gemini").join("settings.json").exists(),
            "Did not expect user settings file to be written when --gemini-system is set",
        );

        let content = fs::read_to_string(&system_settings_path)?;
        anyhow::ensure!(content.contains("\"mcpServers\""));
        anyhow::ensure!(content.contains("\"filesystem\""));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_gemini_system_settings_dry_run_writes_nothing() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let home_dir = temp_dir.path().join("home");
        let claudius_dir = config_dir.join("claudius");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&home_dir)?;
        fs::create_dir_all(&claudius_dir)?;

        std::env::set_var("XDG_CONFIG_HOME", &config_dir);
        std::env::set_var("HOME", &home_dir);

        fs::write(claudius_dir.join("mcpServers.json"), r#"{"mcpServers":{}}"#)?;
        fs::write(claudius_dir.join("gemini.settings.json"), r#"{"general":{"vimMode":true}}"#)?;

        let system_settings_path =
            temp_dir.path().join("etc").join("gemini-cli").join("settings.json");

        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .env("GEMINI_CLI_SYSTEM_SETTINGS_PATH", &system_settings_path)
            .args([
                "config",
                "sync",
                "--global",
                "--agent",
                "gemini",
                "--gemini-system",
                "--dry-run",
            ])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            anyhow::bail!("sync command failed");
        }

        anyhow::ensure!(
            !system_settings_path.exists(),
            "Did not expect system settings to be created in dry-run mode",
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::ensure!(stdout.contains(system_settings_path.to_string_lossy().as_ref()));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_gemini_system_settings_backup_creates_backup_file() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let home_dir = temp_dir.path().join("home");
        let claudius_dir = config_dir.join("claudius");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&home_dir)?;
        fs::create_dir_all(&claudius_dir)?;

        std::env::set_var("XDG_CONFIG_HOME", &config_dir);
        std::env::set_var("HOME", &home_dir);

        fs::write(claudius_dir.join("mcpServers.json"), r#"{"mcpServers":{}}"#)?;
        fs::write(claudius_dir.join("gemini.settings.json"), r#"{"general":{"vimMode":true}}"#)?;

        let system_settings_path =
            temp_dir.path().join("etc").join("gemini-cli").join("settings.json");
        fs::create_dir_all(system_settings_path.parent().unwrap())?;
        fs::write(&system_settings_path, r#"{"general":{"vimMode":false}}"#)?;

        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .env("GEMINI_CLI_SYSTEM_SETTINGS_PATH", &system_settings_path)
            .args([
                "config",
                "sync",
                "--global",
                "--agent",
                "gemini",
                "--gemini-system",
                "--backup",
            ])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            anyhow::bail!("sync command failed");
        }

        let target_dir = system_settings_path.parent().unwrap();
        let entries: Vec<_> = fs::read_dir(target_dir)?
            .map(|entry| entry.map(|e| e.file_name()))
            .collect::<std::result::Result<Vec<_>, std::io::Error>>()?;

        anyhow::ensure!(
            entries
                .iter()
                .any(|name| name.to_string_lossy().starts_with("settings.json.backup.")),
            "Expected Gemini system settings backup to exist",
        );

        Ok(())
    }
}
