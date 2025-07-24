use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;
use serial_test::serial;
use std::env;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial]
    fn test_variable_expansion_with_nested_references() {
        // Create a temporary directory for testing
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let config_dir = temp_dir.child(".config/claudius");
        config_dir.create_dir_all().unwrap();

        // Create config.toml with 1Password configuration
        let config_toml = config_dir.child("config.toml");
        config_toml
            .write_str(
                r#"
[secret-manager]
type = "1password"
"#,
            )
            .unwrap();

        // Set up environment with nested variable references
        env::set_var("CLAUDIUS_TEST_MOCK_OP", "1");
        env::set_var("XDG_CONFIG_HOME", temp_dir.child(".config").path());

        // Set up variables with nested references - the key test case
        env::set_var("CLAUDIUS_SECRET_CF_AIG_ACCOUNT_ID", "12345");
        env::set_var("CLAUDIUS_SECRET_ANTHROPIC_API_KEY", "op://vault/test-item/api-key");
        env::set_var(
            "CLAUDIUS_SECRET_ANTHROPIC_BASE_URL",
            "https://claudehub.api.cloudflare.com/$CLAUDIUS_SECRET_CF_AIG_ACCOUNT_ID/v1",
        );

        // Run claudius with a test command that will show resolved environment variables
        let mut cmd = Command::cargo_bin("claudius").unwrap();
        let assert = cmd
            .arg("run")
            .arg("--")
            .arg("sh")
            .arg("-c")
            .arg("echo \"API_KEY=$ANTHROPIC_API_KEY\" && echo \"BASE_URL=$ANTHROPIC_BASE_URL\"")
            .assert();

        assert
            .success()
            .stdout(predicate::str::contains("API_KEY=secret-api-key-12345"))
            .stdout(predicate::str::contains(
                "BASE_URL=https://claudehub.api.cloudflare.com/12345/v1",
            ));

        // Cleanup
        env::remove_var("CLAUDIUS_TEST_MOCK_OP");
        env::remove_var("CLAUDIUS_SECRET_CF_AIG_ACCOUNT_ID");
        env::remove_var("CLAUDIUS_SECRET_ANTHROPIC_API_KEY");
        env::remove_var("CLAUDIUS_SECRET_ANTHROPIC_BASE_URL");
    }

    #[test]
    #[serial]
    fn test_complex_variable_dependency_chain() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let config_dir = temp_dir.child(".config/claudius");
        config_dir.create_dir_all().unwrap();

        env::set_var("XDG_CONFIG_HOME", temp_dir.child(".config").path());

        // Create a complex dependency chain
        env::set_var("CLAUDIUS_SECRET_HOST", "api.example.com");
        env::set_var("CLAUDIUS_SECRET_PORT", "8443");
        env::set_var("CLAUDIUS_SECRET_PROTOCOL", "https");
        env::set_var("CLAUDIUS_SECRET_BASE_PATH", "/v2/production");
        env::set_var(
            "CLAUDIUS_SECRET_SERVER_URL",
            "$CLAUDIUS_SECRET_PROTOCOL://$CLAUDIUS_SECRET_HOST:$CLAUDIUS_SECRET_PORT",
        );
        env::set_var(
            "CLAUDIUS_SECRET_API_ENDPOINT",
            "$CLAUDIUS_SECRET_SERVER_URL$CLAUDIUS_SECRET_BASE_PATH",
        );

        let mut cmd = Command::cargo_bin("claudius").unwrap();
        let assert = cmd
            .arg("run")
            .arg("--")
            .arg("sh")
            .arg("-c")
            .arg("echo \"ENDPOINT=$API_ENDPOINT\"")
            .assert();

        assert.success().stdout(predicate::str::contains(
            "ENDPOINT=https://api.example.com:8443/v2/production",
        ));

        // Cleanup
        env::remove_var("CLAUDIUS_SECRET_HOST");
        env::remove_var("CLAUDIUS_SECRET_PORT");
        env::remove_var("CLAUDIUS_SECRET_PROTOCOL");
        env::remove_var("CLAUDIUS_SECRET_BASE_PATH");
        env::remove_var("CLAUDIUS_SECRET_SERVER_URL");
        env::remove_var("CLAUDIUS_SECRET_API_ENDPOINT");
    }

    #[test]
    #[serial]
    fn test_circular_dependency_detection() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let config_dir = temp_dir.child(".config/claudius");
        config_dir.create_dir_all().unwrap();

        env::set_var("XDG_CONFIG_HOME", temp_dir.child(".config").path());

        // Create circular dependencies
        env::set_var("CLAUDIUS_SECRET_A", "$CLAUDIUS_SECRET_B");
        env::set_var("CLAUDIUS_SECRET_B", "$CLAUDIUS_SECRET_C");
        env::set_var("CLAUDIUS_SECRET_C", "$CLAUDIUS_SECRET_A");

        let mut cmd = Command::cargo_bin("claudius").unwrap();
        let assert = cmd.arg("run").arg("--").arg("echo").arg("test").assert();

        assert
            .failure()
            .stderr(predicate::str::contains("Circular dependency detected"));

        // Cleanup
        env::remove_var("CLAUDIUS_SECRET_A");
        env::remove_var("CLAUDIUS_SECRET_B");
        env::remove_var("CLAUDIUS_SECRET_C");
    }
}
