use assert_cmd::Command;
use serial_test::serial;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn setup_mock_op_path() -> PathBuf {
    // Use cargo manifest dir to reliably find the test fixtures
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(manifest_dir).join("tests").join("fixtures").join("mock_op.sh")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial]
    fn test_onepassword_secret_resolution_with_fixture() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config").join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        // Create config with 1Password
        let config_content = r#"
[secret-manager]
type = "1password"
"#;
        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        // Create empty mcpServers.json to prevent "No such file" error
        fs::write(config_dir.join("mcpServers.json"), r#"{"mcpServers": {}}"#).unwrap();

        // Create a test mcp servers config
        let mcp_config = r#"{
  "mcpServers": {
    "test-server": {
      "command": "echo",
      "args": ["test"]
    }
  }
}"#;
        fs::write(config_dir.join("mcpServers.json"), mcp_config).unwrap();

        // Get the mock op path
        let mock_op = setup_mock_op_path();
        let mock_bin_dir = temp_dir.path().join("bin");
        fs::create_dir_all(&mock_bin_dir).unwrap();

        // Create a symlink to our mock op
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&mock_op, mock_bin_dir.join("op")).unwrap();
        }

        // Create a project directory and change to it
        let project_dir = temp_dir.path().join("project");
        fs::create_dir_all(&project_dir).unwrap();

        // Run claudius with the mock in PATH
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&project_dir)
            .env("XDG_CONFIG_HOME", temp_dir.path().join("config"))
            .env("CLAUDIUS_TEST_MOCK_OP", "1")
            .env(
                "PATH",
                format!("{}:{}", mock_bin_dir.display(), std::env::var("PATH").unwrap_or_default()),
            )
            .env("CLAUDIUS_SECRET_API_KEY", "op://vault/test-item/api-key")
            .env("CLAUDIUS_SECRET_DB_PASSWORD", "op://vault/database/password")
            .arg("--debug")
            .args(["config", "sync"])
            .arg("--dry-run");

        cmd.assert().success();
    }

    #[test]
    #[serial]
    fn test_onepassword_error_handling_with_fixture() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config").join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        // Create config with 1Password
        let config_content = r#"
[secret-manager]
type = "1password"
"#;
        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        // Create empty mcpServers.json to prevent "No such file" error
        fs::write(config_dir.join("mcpServers.json"), r#"{"mcpServers": {}}"#).unwrap();

        // Get the mock op path
        let mock_op = setup_mock_op_path();
        let mock_bin_dir = temp_dir.path().join("bin");
        fs::create_dir_all(&mock_bin_dir).unwrap();

        // Create a symlink to our mock op
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&mock_op, mock_bin_dir.join("op")).unwrap();
        }

        // Create a project directory and change to it
        let project_dir = temp_dir.path().join("project");
        fs::create_dir_all(&project_dir).unwrap();

        // Run claudius with an invalid reference
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&project_dir)
            .env("XDG_CONFIG_HOME", temp_dir.path().join("config"))
            .env("CLAUDIUS_TEST_MOCK_OP", "1")
            .env(
                "PATH",
                format!("{}:{}", mock_bin_dir.display(), std::env::var("PATH").unwrap_or_default()),
            )
            .env("CLAUDIUS_SECRET_INVALID", "op://invalid/reference/field")
            .args(["config", "sync"])
            .arg("--dry-run");

        // The command should succeed but keep the unresolved reference
        // This is the resilient behavior - we warn but don't fail
        cmd.assert().success();
    }

    #[test]
    #[serial]
    fn test_mixed_secret_types() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config").join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        // Create config with 1Password
        let config_content = r#"
[secret-manager]
type = "1password"
"#;
        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        // Create empty mcpServers.json to prevent "No such file" error
        fs::write(config_dir.join("mcpServers.json"), r#"{"mcpServers": {}}"#).unwrap();

        // Create a test mcp servers config
        let mcp_config = r#"{
  "mcpServers": {
    "test-server": {
      "command": "echo",
      "args": ["test"]
    }
  }
}"#;
        fs::write(config_dir.join("mcpServers.json"), mcp_config).unwrap();

        // Get the mock op path
        let mock_op = setup_mock_op_path();
        let mock_bin_dir = temp_dir.path().join("bin");
        fs::create_dir_all(&mock_bin_dir).unwrap();

        // Create a symlink to our mock op
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&mock_op, mock_bin_dir.join("op")).unwrap();
        }

        // Create a project directory and change to it
        let project_dir = temp_dir.path().join("project");
        fs::create_dir_all(&project_dir).unwrap();

        // Run claudius with mixed secret types
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&project_dir)
            .env("XDG_CONFIG_HOME", temp_dir.path().join("config"))
            .env("CLAUDIUS_TEST_MOCK_OP", "1")
            .env(
                "PATH",
                format!("{}:{}", mock_bin_dir.display(), std::env::var("PATH").unwrap_or_default()),
            )
            .env("CLAUDIUS_SECRET_OP_SECRET", "op://vault/test-item/api-key")
            .env("CLAUDIUS_SECRET_PLAIN_SECRET", "plain-text-value")
            .arg("--debug")
            .args(["config", "sync"])
            .arg("--dry-run");

        cmd.assert().success().success();
    }
}
