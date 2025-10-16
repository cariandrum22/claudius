use assert_cmd::Command;
use predicates::prelude::*;
use serial_test::serial;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    // ===========================
    // Helper Functions
    // ===========================

    fn setup_mock_op_path() -> PathBuf {
        // Use cargo manifest dir to reliably find the test fixtures
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(manifest_dir).join("tests").join("fixtures").join("mock_op.sh")
    }

    // ===========================
    // Basic Run Command Tests
    // ===========================

    #[test]
    #[serial]
    fn test_run_command_basic() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.args(["secrets", "run"]).arg("--").arg("echo").arg("hello world");

        cmd.assert().success().stdout(predicate::str::contains("hello world"));
    }

    #[test]
    #[serial]
    fn test_run_command_with_exit_code() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.args(["secrets", "run"]).arg("--").arg("sh").arg("-c").arg("exit 42");

        cmd.assert().failure().code(42);
    }

    #[test]
    #[serial]
    fn test_run_command_pipeline() {
        // Test that pipes work correctly
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.args(["secrets", "run"])
            .arg("--")
            .arg("sh")
            .arg("-c")
            .arg("echo hello | tr a-z A-Z");

        cmd.assert().success().stdout(predicate::str::contains("HELLO"));
    }

    // ===========================
    // Environment and Argument Tests
    // ===========================

    #[test]
    #[serial]
    fn test_run_command_with_args() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.args(["secrets", "run"]).arg("--").arg("echo").arg("-n").arg("test");

        cmd.assert().success().stdout(predicate::str::is_match("^test$").unwrap());
    }

    #[test]
    #[serial]
    fn test_run_command_preserves_env() {
        // Test that existing environment variables are preserved
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.env("EXISTING_VAR", "existing_value")
            .args(["secrets", "run"])
            .arg("--")
            .arg("sh")
            .arg("-c")
            .arg("echo $EXISTING_VAR");

        cmd.assert().success().stdout(predicate::str::contains("existing_value"));
    }

    #[test]
    #[serial]
    fn test_run_command_with_env_resolution() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config").join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        // Create empty config (no secret manager)
        let config_content = "\n# Empty config\n";
        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .env("XDG_CONFIG_HOME", temp_dir.path().join("config"))
            .env("CLAUDIUS_SECRET_TEST_VAR", "secret_value")
            .arg("--debug")
            .args(["secrets", "run"])
            .arg("--")
            .arg("/bin/sh")
            .arg("-c")
            .arg("echo $TEST_VAR");

        cmd.assert().success().stdout(predicate::str::contains("secret_value"));
    }

    #[test]
    #[serial]
    fn test_run_command_with_mock_secret() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config").join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        // Create config without secret manager (so secrets are used as-is)
        let config_content = "# No secret manager";
        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .env("XDG_CONFIG_HOME", temp_dir.path().join("config"))
            .env("CLAUDIUS_SECRET_API_KEY", "test-api-key-123")
            .env("CLAUDIUS_SECRET_DB_PASS", "test-db-pass-456")
            .arg("--debug")
            .args(["secrets", "run"])
            .arg("--")
            .arg("sh")
            .arg("-c")
            .arg("echo API_KEY=$API_KEY DB_PASS=$DB_PASS");

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("API_KEY=test-api-key-123"))
            .stdout(predicate::str::contains("DB_PASS=test-db-pass-456"));
    }

    // ===========================
    // Secret Resolution Tests (1Password)
    // ===========================

    #[test]
    #[serial]
    fn test_run_with_onepassword_secrets() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config").join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        // Create config with 1Password
        let config_content = r#"
[secret-manager]
type = "1password"
"#;
        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        // Get the mock op path
        let mock_op = setup_mock_op_path();
        let mock_bin_dir = temp_dir.path().join("bin");
        fs::create_dir_all(&mock_bin_dir).unwrap();

        // Create a symlink to our mock op
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&mock_op, mock_bin_dir.join("op")).unwrap();
        }

        // Run claudius with mock 1Password secrets
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .env("XDG_CONFIG_HOME", temp_dir.path().join("config"))
            .env("CLAUDIUS_TEST_MOCK_OP", "1")
            .env(
                "PATH",
                format!("{}:{}", mock_bin_dir.display(), std::env::var("PATH").unwrap_or_default()),
            )
            .env("CLAUDIUS_SECRET_API_KEY", "op://vault/test-item/api-key")
            .env("CLAUDIUS_SECRET_DB_PASSWORD", "op://vault/database/password")
            .arg("--debug")
            .args(["secrets", "run"])
            .arg("--")
            .arg("/bin/sh")
            .arg("-c")
            .arg("echo API_KEY=$API_KEY DB_PASSWORD=$DB_PASSWORD");

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("API_KEY=secret-api-key-12345"))
            .stdout(predicate::str::contains("DB_PASSWORD=db-password-xyz789"));
    }

    #[test]
    #[serial]
    fn test_run_with_mixed_secrets() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config").join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        // Create config with 1Password
        let config_content = r#"
[secret-manager]
type = "1password"
"#;
        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        // Get the mock op path
        let mock_op = setup_mock_op_path();
        let mock_bin_dir = temp_dir.path().join("bin");
        fs::create_dir_all(&mock_bin_dir).unwrap();

        // Create a symlink to our mock op
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&mock_op, mock_bin_dir.join("op")).unwrap();
        }

        // Run with both op:// and plain secrets
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .env("XDG_CONFIG_HOME", temp_dir.path().join("config"))
            .env("CLAUDIUS_TEST_MOCK_OP", "1")
            .env(
                "PATH",
                format!("{}:{}", mock_bin_dir.display(), std::env::var("PATH").unwrap_or_default()),
            )
            .env("CLAUDIUS_SECRET_OP_SECRET", "op://vault/test-item/api-key")
            .env("CLAUDIUS_SECRET_PLAIN_SECRET", "plain-value")
            .arg("--debug")
            .args(["secrets", "run"])
            .arg("--")
            .arg("/bin/sh")
            .arg("-c")
            .arg("echo OP=$OP_SECRET PLAIN=$PLAIN_SECRET");

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("OP=secret-api-key-12345"))
            .stdout(predicate::str::contains("PLAIN=plain-value"));
    }

    // ===========================
    // Error Handling Tests
    // ===========================

    #[test]
    #[serial]
    fn test_run_command_no_command() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.args(["secrets", "run"]);

        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("required arguments were not provided"));
    }

    #[test]
    #[serial]
    fn test_run_command_nonexistent() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.args(["secrets", "run"]).arg("--").arg("nonexistent_command_12345");

        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("Failed to execute command"));
    }

    #[test]
    #[serial]
    fn test_run_with_onepassword_error() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config").join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        // Create config with 1Password
        let config_content = r#"
[secret-manager]
type = "1password"
"#;
        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        // Get the mock op path
        let mock_op = setup_mock_op_path();
        let mock_bin_dir = temp_dir.path().join("bin");
        fs::create_dir_all(&mock_bin_dir).unwrap();

        // Create a symlink to our mock op
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&mock_op, mock_bin_dir.join("op")).unwrap();
        }

        // Run claudius with invalid 1Password reference
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.env("XDG_CONFIG_HOME", temp_dir.path().join("config"))
            .env("CLAUDIUS_TEST_MOCK_OP", "1")
            .env(
                "PATH",
                format!("{}:{}", mock_bin_dir.display(), std::env::var("PATH").unwrap_or_default()),
            )
            .env("CLAUDIUS_SECRET_INVALID", "op://invalid/reference/field")
            .args(["secrets", "run"])
            .arg("--")
            .arg("echo")
            .arg("test");

        // The command should succeed but the reference remains unresolved
        // This is the resilient behavior - we don't fail, we just pass through
        cmd.assert().success().stdout(predicate::str::contains("test"));
    }
}
