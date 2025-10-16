use crate::fixtures::TestFixture;
use assert_cmd::Command;
use predicates::prelude::*;
use serial_test::serial;

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to save and restore environment variables
    struct EnvGuard {
        xdg_original: Option<String>,
        home_original: Option<String>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self {
                xdg_original: std::env::var("XDG_CONFIG_HOME").ok(),
                home_original: std::env::var("HOME").ok(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // Restore XDG_CONFIG_HOME
            match &self.xdg_original {
                Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
            // Restore HOME
            match &self.home_original {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }
        }
    }

    #[test]
    #[serial]
    fn test_commands_sync_project_local() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create test commands
        fixture.with_command("hello", "# Hello Command\nTest command").unwrap();
        fixture.with_command("debug", "# Debug Command\nDebug info").unwrap();

        // Create minimal config
        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();

        // Run sync in project-local mode (default)
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["commands", "sync"])
            .assert()
            .success();

        // Verify commands were synced to project-local .claude/commands/
        assert!(fixture.project_file_exists(".claude/commands"));

        // Check commands exist without .md extension
        assert!(fixture.project_file_exists(".claude/commands/hello"));
        assert!(fixture.project_file_exists(".claude/commands/debug"));

        // Verify content
        let hello_content = fixture.read_project_file(".claude/commands/hello").unwrap();
        assert_eq!(hello_content, "# Hello Command\nTest command");

        let debug_content = fixture.read_project_file(".claude/commands/debug").unwrap();
        assert_eq!(debug_content, "# Debug Command\nDebug info");
    }

    #[test]
    #[serial]
    fn test_commands_sync_global() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create test commands
        fixture.with_command("test", "# Test Command").unwrap();

        // Create minimal config
        fixture.with_mcp_servers(r#"{"mcpServers": {}}"#).unwrap();

        // Run sync in global mode
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["commands", "sync"])
            .arg("--global")
            .assert()
            .success();

        // Verify commands were synced to ~/.claude/commands/
        assert!(fixture.home_file_exists(".claude/commands"));

        // Check command exists
        assert!(fixture.home_file_exists(".claude/commands/test"));

        let test_content = fixture.read_home_file(".claude/commands/test").unwrap();
        assert_eq!(test_content, "# Test Command");
    }

    #[test]
    #[serial]
    fn test_commands_only_mode_project_local() {
        let _env_guard = EnvGuard::new();
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create test command
        fixture.with_command("cmd", "Command content").unwrap();

        // Create config files
        fixture
            .with_mcp_servers(r#"{"mcpServers": {"test": {"command": "test"}}}"#)
            .unwrap();
        fixture.with_settings(r#"{"apiKeyHelper": "test"}"#).unwrap();

        // Run sync using dedicated commands subcommand
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["commands", "sync"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Successfully synced"));

        // Verify only commands were synced (no .mcp.json or settings.json)
        assert!(!fixture.project_file_exists(".mcp.json"));
        assert!(!fixture.project_file_exists(".claude/settings.json"));

        // But commands should exist
        assert!(fixture.project_file_exists(".claude/commands/cmd"));
    }
}
