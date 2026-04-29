use assert_cmd::Command;
use predicates::prelude::*;
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
            .env(
                "PATH",
                format!("{}:{}", mock_bin_dir.display(), std::env::var("PATH").unwrap_or_default()),
            )
            .env("CLAUDIUS_SECRET_API_KEY", "op://vault/test-item/api-key")
            .env("CLAUDIUS_SECRET_DB_PASSWORD", "op://vault/database/password")
            .args(["secrets", "run"])
            .arg("--")
            .arg("/bin/sh")
            .arg("-c")
            .arg("printf 'API_KEY=%s\nDB_PASSWORD=%s\n' \"$API_KEY\" \"$DB_PASSWORD\"");

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("API_KEY=secret-api-key-12345"))
            .stdout(predicate::str::contains("DB_PASSWORD=db-password-xyz789"));
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
            .env(
                "PATH",
                format!("{}:{}", mock_bin_dir.display(), std::env::var("PATH").unwrap_or_default()),
            )
            .env("CLAUDIUS_SECRET_INVALID", "op://invalid/reference/field")
            .args(["secrets", "run"])
            .arg("--")
            .arg("/bin/sh")
            .arg("-c")
            .arg("printf 'INVALID=%s\n' \"$INVALID\"");

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("INVALID=op://invalid/reference/field"));
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
            .env(
                "PATH",
                format!("{}:{}", mock_bin_dir.display(), std::env::var("PATH").unwrap_or_default()),
            )
            .env("CLAUDIUS_SECRET_OP_SECRET", "op://vault/test-item/api-key")
            .env("CLAUDIUS_SECRET_PLAIN_SECRET", "plain-text-value")
            .args(["secrets", "run"])
            .arg("--")
            .arg("/bin/sh")
            .arg("-c")
            .arg("printf 'OP_SECRET=%s\nPLAIN_SECRET=%s\n' \"$OP_SECRET\" \"$PLAIN_SECRET\"");

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("OP_SECRET=secret-api-key-12345"))
            .stdout(predicate::str::contains("PLAIN_SECRET=plain-text-value"));
    }
}
