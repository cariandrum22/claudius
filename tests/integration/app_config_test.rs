use assert_cmd::Command;
use predicates::prelude::*;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial]
    fn test_app_config_loading() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config").join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        // Create a config file
        let config_content = r#"
[secret-manager]
type = "1password"
"#;
        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        // Create minimal mcpServers.json
        let mcp_content = r#"{
        "mcpServers": {}
    }"#;
        fs::write(config_dir.join("mcpServers.json"), mcp_content).unwrap();

        // Run the command with custom XDG_CONFIG_HOME
        let mut cmd = Command::cargo_bin("claudius").unwrap();
        cmd.env("XDG_CONFIG_HOME", temp_dir.path().join("config"))
            .env("HOME", temp_dir.path().join("home"))
            // Clear any existing CLAUDIUS_SECRET_* env vars from the test environment
            .env_clear()
            .env("XDG_CONFIG_HOME", temp_dir.path().join("config"))
            .env("HOME", temp_dir.path().join("home"))
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .arg("--debug")
            .arg("sync")
            .arg("--dry-run");

        cmd.assert().success().success();
    }

    #[test]
    #[serial]
    fn test_vault_warning() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config").join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        // Create a config file with Vault
        let config_content = r#"
[secret-manager]
type = "vault"
"#;
        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        // Create minimal mcpServers.json
        let mcp_content = r#"{
        "mcpServers": {}
    }"#;
        fs::write(config_dir.join("mcpServers.json"), mcp_content).unwrap();

        // Create a project directory for the sync operation
        let project_dir = temp_dir.path().join("project");
        fs::create_dir_all(&project_dir).unwrap();

        // Set a secret env var
        let mut cmd = Command::cargo_bin("claudius").unwrap();
        cmd.current_dir(&project_dir)
            .env("XDG_CONFIG_HOME", temp_dir.path().join("config"))
            .env("CLAUDIUS_SECRET_TEST", "test_value")
            .arg("--debug")
            .arg("sync")
            .arg("--dry-run");

        cmd.assert().success().stderr(predicate::str::contains(
            "Vault secret manager is configured but not yet implemented",
        ));
    }

    #[test]
    #[serial]
    fn test_no_config_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config").join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        // Create minimal mcpServers.json
        let mcp_content = r#"{
        "mcpServers": {}
    }"#;
        fs::write(config_dir.join("mcpServers.json"), mcp_content).unwrap();

        // Create a project directory
        let project_dir = temp_dir.path().join("project");
        fs::create_dir_all(&project_dir).unwrap();

        // Run without config file
        let mut cmd = Command::cargo_bin("claudius").unwrap();
        cmd.current_dir(&project_dir)
            .env("XDG_CONFIG_HOME", temp_dir.path().join("config"))
            .arg("--debug")
            .arg("sync")
            .arg("--dry-run");

        cmd.assert().success().success();
    }

    #[test]
    #[serial]
    fn test_env_var_resolution() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config").join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        // Create config without secret manager
        let config_content = "\n# Empty config file\n";
        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        // Create minimal mcpServers.json
        let mcp_content = r#"{
        "mcpServers": {}
    }"#;
        fs::write(config_dir.join("mcpServers.json"), mcp_content).unwrap();

        // Create a project directory for the sync operation
        let project_dir = temp_dir.path().join("project");
        fs::create_dir_all(&project_dir).unwrap();

        // Set a secret env var
        let mut cmd = Command::cargo_bin("claudius").unwrap();
        cmd.current_dir(&project_dir)
            .env("XDG_CONFIG_HOME", temp_dir.path().join("config"))
            .env("CLAUDIUS_SECRET_TEST_KEY", "test_value")
            .arg("--debug")
            .arg("sync")
            .arg("--dry-run");

        cmd.assert().success().success();
    }
}
