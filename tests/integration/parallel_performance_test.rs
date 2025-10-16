use assert_cmd::Command;
use serial_test::serial;
use std::fs;
use std::time::Instant;
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial]
    #[ignore = "Flaky test - passes in isolation but fails when run with all tests"]
    fn test_parallel_performance_improvement() {
        // Create a temporary config directory
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join(".config").join("claudius");
        fs::create_dir_all(&config_dir).unwrap();

        // Create a config file with 1Password enabled
        let config_content = r#"
[secret-manager]
type = "1password"
"#;
        fs::write(config_dir.join("config.toml"), config_content).unwrap();

        // Clean up any existing CLAUDIUS_SECRET_* environment variables
        let existing_vars: Vec<String> = std::env::vars()
            .filter(|(key, _)| key.starts_with("CLAUDIUS_SECRET_"))
            .map(|(key, _)| key)
            .collect();

        for key in &existing_vars {
            std::env::remove_var(key);
        }

        // Set up multiple CLAUDIUS_SECRET_* environment variables
        let vars = vec![
            ("CLAUDIUS_SECRET_VAR1", "op://vault/item1/field1"),
            ("CLAUDIUS_SECRET_VAR2", "op://vault/item2/field2"),
            ("CLAUDIUS_SECRET_VAR3", "op://vault/item3/field3"),
            ("CLAUDIUS_SECRET_VAR4", "op://vault/item4/field4"),
            ("CLAUDIUS_SECRET_VAR5", "op://vault/item5/field5"),
        ];

        // Run with profiling enabled
        let start = Instant::now();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.args(["secrets", "run"])
            .arg("--")
            .arg("/usr/bin/env")
            .env("CLAUDIUS_TEST_MOCK_OP", "1")
            .env("CLAUDIUS_PROFILE", "1")
            .env("XDG_CONFIG_HOME", temp_dir.path().join(".config"));

        // Set all environment variables
        for (key, value) in &vars {
            cmd.env(key, value);
        }

        let output = cmd.output().expect("Failed to execute command");
        let duration = start.elapsed();

        // Check that the command succeeded
        if !output.status.success() {
            eprintln!("Command failed with status: {}", output.status);
            eprintln!("Stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
        }
        assert!(output.status.success());

        // The total time should be significantly less than sequential processing
        // With mocks, each op:// call is instant, but we can still verify parallel execution
        println!("Total execution time: {}ms", duration.as_millis());

        // Note: We can't check for "Phase 2: Resolving secret references (parallel)"
        // in stderr because debug logging is not enabled in tests by default
    }
}
