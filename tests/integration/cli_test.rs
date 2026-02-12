use assert_cmd::Command;
use assert_fs::fixture::ChildPath;
use assert_fs::prelude::*;
use predicates::prelude::*;
use serial_test::serial;
use std::fs;

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to set up test environment with proper configuration
    fn setup_test_env(temp_dir: &assert_fs::TempDir) -> (ChildPath, ChildPath) {
        let config_dir = temp_dir.child("config").child("claudius");
        config_dir.create_dir_all().unwrap();

        // Create empty gemini settings file
        let gemini_settings = config_dir.child("gemini.settings.json");
        gemini_settings.write_str("{}").unwrap();

        // Create minimal mcpServers.json in config dir
        let servers_file = config_dir.child("mcpServers.json");
        // Create an empty mcpServers.json if it doesn't exist
        servers_file.write_str(r#"{"mcpServers": {}}"#).unwrap();

        let mcp_file = temp_dir.child(".mcp.json");

        (servers_file, mcp_file)
    }

    #[test]
    #[serial]
    fn test_cli_help() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.arg("--help").assert().success().stdout(predicate::str::contains(
            "Claudius is a comprehensive configuration management tool for Claude Desktop/CLI",
        ));
    }

    #[test]
    #[serial]
    fn test_cli_list_commands() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.arg("--list-commands").assert().success().stdout(
            predicate::str::contains("Available commands:")
                .and(predicate::str::contains("config"))
                .and(predicate::str::contains("skills"))
                .and(predicate::str::contains("context"))
                .and(predicate::str::contains("secrets"))
                .and(predicate::str::contains("Use `claudius <command> --help`")),
        );
    }

    #[test]
    #[serial]
    fn test_cli_version() {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.arg("--version")
            .assert()
            .success()
            .stdout(predicate::str::contains("claudius"));
    }

    #[test]
    #[serial]
    fn test_sync_basic() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let (servers_file, mcp_file) = setup_test_env(&temp_dir);

        // Create mcpServers.json
        servers_file
            .write_str(
                r#"{
            "mcpServers": {
                "test-server": {
                    "command": "node",
                    "args": ["server.js"],
                    "env": {
                        "PORT": "3000"
                    }
                }
            }
        }"#,
            )
            .unwrap();

        // Create initial .mcp.json (empty)
        mcp_file.write_str("{}").unwrap();

        // Run sync
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .args(["config", "sync"])
            .arg("--agent")
            .arg("gemini")
            .arg("--target-config")
            .arg(mcp_file.path())
            .env("XDG_CONFIG_HOME", temp_dir.child("config").path())
            .assert()
            .success();

        // Verify .mcp.json was updated
        let content = fs::read_to_string(mcp_file.path()).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        // In project-local mode, only mcpServers should be in .mcp.json
        assert_eq!(
            json.get("mcpServers")
                .and_then(|s| s.get("test-server"))
                .and_then(|t| t.get("command")),
            Some(&serde_json::Value::String("node".to_string()))
        );
        assert!(json.get("theme").is_none());
        assert!(json.get("fontSize").is_none());
    }

    #[test]
    #[serial]
    fn test_sync_merge_existing() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let (servers_file, mcp_file) = setup_test_env(&temp_dir);

        // Create mcpServers.json
        servers_file
            .write_str(
                r#"{
            "mcpServers": {
                "new-server": {
                    "command": "python",
                    "args": ["-m", "server"],
                    "env": {}
                }
            }
        }"#,
            )
            .unwrap();

        // Create .mcp.json with existing server
        mcp_file
            .write_str(
                r#"{
            "mcpServers": {
                "existing-server": {
                    "command": "deno",
                    "args": ["run", "server.ts"],
                    "env": {}
                }
            }
        }"#,
            )
            .unwrap();

        // Run sync
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .args(["config", "sync"])
            .arg("--agent")
            .arg("gemini")
            .arg("--target-config")
            .arg(mcp_file.path())
            .env("XDG_CONFIG_HOME", temp_dir.child("config").path())
            .assert()
            .success();

        // Verify both servers exist
        let content = fs::read_to_string(mcp_file.path()).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(
            json.get("mcpServers")
                .and_then(|s| s.get("existing-server"))
                .and_then(|t| t.get("command")),
            Some(&serde_json::Value::String("deno".to_string()))
        );
        assert_eq!(
            json.get("mcpServers")
                .and_then(|s| s.get("new-server"))
                .and_then(|t| t.get("command")),
            Some(&serde_json::Value::String("python".to_string()))
        );
        // No theme in .mcp.json
        assert!(json.get("theme").is_none());
    }

    #[test]
    #[serial]
    fn test_dry_run() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let (servers_file, mcp_file) = setup_test_env(&temp_dir);

        // Create mcpServers.json
        servers_file
            .write_str(
                r#"{
            "mcpServers": {
                "test-server": {
                    "command": "node",
                    "args": [],
                    "env": {}
                }
            }
        }"#,
            )
            .unwrap();

        // Create empty .mcp.json
        mcp_file.write_str("{}").unwrap();

        // Run sync with dry-run
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .args(["config", "sync"])
            .arg("--agent")
            .arg("gemini")
            .arg("--dry-run")
            .arg("--target-config")
            .arg(mcp_file.path())
            .env("XDG_CONFIG_HOME", temp_dir.child("config").path())
            .assert()
            .success()
            .stdout(predicate::str::contains("test-server"));

        // Verify .mcp.json was NOT modified
        let content = fs::read_to_string(mcp_file.path()).unwrap();
        assert_eq!(content, "{}");
    }

    #[test]
    #[serial]
    fn test_backup_creation() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let (servers_file, mcp_file) = setup_test_env(&temp_dir);

        // Create mcpServers.json
        servers_file
            .write_str(
                r#"{
            "mcpServers": {
                "test-server": {
                    "command": "node",
                    "args": [],
                    "env": {}
                }
            }
        }"#,
            )
            .unwrap();

        // Create .mcp.json with content
        mcp_file
            .write_str(
                r#"{
            "mcpServers": {}
        }"#,
            )
            .unwrap();

        // Run sync with backup
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .args(["config", "sync"])
            .arg("--agent")
            .arg("gemini")
            .arg("--backup")
            .arg("--target-config")
            .arg(mcp_file.path())
            .env("XDG_CONFIG_HOME", temp_dir.child("config").path())
            .assert()
            .success();

        // Verify backup was created
        let entries: Vec<_> =
            fs::read_dir(temp_dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();

        let backup_exists = entries
            .iter()
            .any(|name| name.to_string_lossy().starts_with(".mcp.json.backup."));

        assert!(backup_exists, "Backup file should exist");
    }

    #[test]
    #[serial]
    fn test_missing_servers_file() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let (_, mcp_file) = setup_test_env(&temp_dir);

        // Note: We're using a path that doesn't exist to simulate missing file
        let servers_file = temp_dir.child("config").child("claudius").child("nonexistent.json");

        // Run sync with missing servers file
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .args(["config", "sync"])
            .arg("--agent")
            .arg("gemini")
            .arg("--config")
            .arg(servers_file.path())
            .arg("--target-config")
            .arg(mcp_file.path())
            .env("XDG_CONFIG_HOME", temp_dir.child("config").path())
            .assert()
            .failure()
            .stderr(predicate::str::contains("Failed to read MCP servers configuration"));
    }

    #[test]
    #[serial]
    fn test_invalid_json() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let (servers_file, mcp_file) = setup_test_env(&temp_dir);

        // Create invalid JSON
        servers_file.write_str("{ invalid json }").unwrap();
        mcp_file.write_str("{}").unwrap();

        // Run sync
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .args(["config", "sync"])
            .arg("--agent")
            .arg("gemini")
            .arg("--target-config")
            .arg(mcp_file.path())
            .env("XDG_CONFIG_HOME", temp_dir.child("config").path())
            .assert()
            .failure()
            .stderr(predicate::str::contains("Failed to read MCP servers configuration"));
    }

    #[test]
    #[serial]
    fn test_sync_with_global_flag() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let servers_file = temp_dir.child("mcpServers.json");
        let claude_file = temp_dir.child("claude.json");
        let settings_file = temp_dir.child("settings.json");

        // Create mcpServers.json
        servers_file
            .write_str(
                r#"{
            "mcpServers": {
                "test-server": {
                    "command": "node",
                    "args": ["server.js"],
                    "env": {}
                }
            }
        }"#,
            )
            .unwrap();

        // Create settings.json
        settings_file
            .write_str(
                r#"{
            "apiKeyHelper": "/bin/test.sh",
            "cleanupPeriodDays": 10
        }"#,
            )
            .unwrap();

        // Create initial claude.json with existing data
        claude_file
            .write_str(
                r#"{
            "theme": "dark",
            "fontSize": 14
        }"#,
            )
            .unwrap();

        // Run sync with global flag
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.args(["config", "sync"])
            .arg("--global")
            .arg("--config")
            .arg(servers_file.path())
            .arg("--target-config")
            .arg(claude_file.path())
            .env("XDG_CONFIG_HOME", temp_dir.path())
            .assert()
            .success();

        // Verify claude.json contains both mcpServers and settings
        let content = fs::read_to_string(claude_file.path()).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Should have mcpServers
        assert_eq!(
            json.get("mcpServers")
                .and_then(|m| m.get("test-server"))
                .and_then(|s| s.get("command")),
            Some(&serde_json::json!("node"))
        );

        // Should preserve existing data
        assert_eq!(json.get("theme"), Some(&serde_json::json!("dark")));
        assert_eq!(json.get("fontSize"), Some(&serde_json::json!(14)));
    }
}
