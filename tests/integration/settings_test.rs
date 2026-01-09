use crate::fixtures::TestFixture;
use assert_cmd::Command;
use predicates::prelude::*;
use serial_test::serial;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial]
    fn test_sync_with_settings_project_local() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create mcpServers.json
        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "command": "test",
                "args": [],
                "env": {}
            }
        }
    }"#,
            )
            .unwrap();

        // Create gemini.settings.json
        fixture
            .with_gemini_settings(
                r#"{
        "context": {
            "fileName": "GEMINI.md"
        },
        "tools": {
            "autoAccept": true
        },
        "ui": {
            "theme": "GitHub"
        }
    }"#,
            )
            .unwrap();

        // Run sync (project-local mode by default)
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync"])
            .arg("--agent")
            .arg("gemini")
            .assert()
            .success();

        // Gemini CLI stores settings and MCP servers together in .gemini/settings.json
        assert!(!fixture.project_file_exists(".mcp.json"));

        let settings_content = fixture.read_project_file(".gemini/settings.json").unwrap();
        let settings_json: serde_json::Value = serde_json::from_str(&settings_content).unwrap();

        assert_eq!(
            settings_json
                .get("mcpServers")
                .and_then(|s| s.get("test-server"))
                .and_then(|t| t.get("command")),
            Some(&serde_json::Value::String("test".to_string()))
        );

        assert_eq!(
            settings_json.get("context").and_then(|c| c.get("fileName")),
            Some(&serde_json::Value::String("GEMINI.md".to_string()))
        );
        assert_eq!(
            settings_json.get("tools").and_then(|t| t.get("autoAccept")),
            Some(&serde_json::Value::Bool(true))
        );
        assert_eq!(
            settings_json.get("ui").and_then(|u| u.get("theme")),
            Some(&serde_json::Value::String("GitHub".to_string()))
        );
    }

    #[test]
    #[serial]
    fn test_sync_with_settings_global() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create mcpServers.json
        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "command": "test",
                "args": [],
                "env": {}
            }
        }
    }"#,
            )
            .unwrap();

        // Create claude.settings.json (used by Claude Code; Claude Desktop ignores it)
        fixture
            .with_claude_settings(
                r#"{
        "apiKeyHelper": "/bin/generate_api_key.sh",
        "cleanupPeriodDays": 20,
        "env": {"FOO": "bar"},
        "includeCoAuthoredBy": false,
        "permissions": {
            "allow": ["Bash(npm run lint)"]
        }
    }"#,
            )
            .unwrap();

        // Create initial Claude Desktop config to verify key preservation
        fixture
            .with_existing_claude_desktop_config(r#"{"existingKey": "value"}"#)
            .unwrap();

        let claude_desktop_config_path =
            fixture.config_home().join("Claude").join("claude_desktop_config.json");

        // Run sync with --global flag
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "sync"])
            .arg("--global")
            .arg("--agent")
            .arg("claude")
            .assert()
            .success();

        // Verify Claude Desktop config contains MCP servers and preserves existing keys
        let content = std::fs::read_to_string(&claude_desktop_config_path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Check MCP servers
        assert_eq!(
            json.get("mcpServers")
                .and_then(|s| s.get("test-server"))
                .and_then(|t| t.get("command")),
            Some(&serde_json::Value::String("test".to_string()))
        );

        // Settings are not merged into Claude Desktop config
        assert!(json.get("apiKeyHelper").is_none());
        assert!(json.get("cleanupPeriodDays").is_none());
        assert!(json.get("env").is_none());
        assert!(json.get("includeCoAuthoredBy").is_none());
        assert!(json.get("permissions").is_none());

        // Check existing key preserved
        assert_eq!(json.get("existingKey"), Some(&serde_json::Value::String("value".to_string())));

        // Verify legacy ~/.claude.json was NOT created
        assert!(!fixture.home_file_exists(".claude.json"));
    }

    #[test]
    #[serial]
    fn test_sync_claude_code_global_writes_settings_json() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create app config with Claude Code as default agent to avoid multi-agent global sync.
        let app_config_path = fixture.config.join("config.toml");
        std::fs::write(
            app_config_path,
            r#"
[default]
agent = "claude-code"
"#,
        )
        .unwrap();

        // Create mcpServers.json
        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "type": "http",
                "url": "https://example.com/mcp",
                "headers": {
                    "Authorization": "Bearer token"
                }
            }
        }
    }"#,
            )
            .unwrap();

        // Create claude.settings.json (source settings) with Claude Code-specific fields
        fixture
            .with_claude_settings(
                r#"{
        "apiKeyHelper": "/bin/generate_temp_api_key.sh",
        "sandbox": {
            "enabled": true
        },
        "companyAnnouncements": ["hello"]
    }"#,
            )
            .unwrap();

        // Create initial claude.json in home directory (should not receive settings fields)
        fixture.with_existing_global_config(r#"{"existingKey": "value"}"#).unwrap();

        // Create existing ~/.claude/settings.json with unrelated keys to ensure merge preservation
        let claude_dir = fixture.home_dir().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{
        "env": {"KEEP": "1"},
        "sandbox": {"autoAllowBashIfSandboxed": true}
    }"#,
        )
        .unwrap();

        // Run sync with --global flag (no --agent, should pick config.toml default)
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "sync"])
            .arg("--global")
            .assert()
            .success();

        // Verify ~/.claude.json contains MCP servers and preserves existing keys
        let content = fixture.read_home_file(".claude.json").unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(json.get("existingKey"), Some(&serde_json::Value::String("value".to_string())));
        assert_eq!(
            json.get("mcpServers")
                .and_then(|s| s.get("test-server"))
                .and_then(|t| t.get("type")),
            Some(&serde_json::Value::String("http".to_string()))
        );
        assert!(json.get("apiKeyHelper").is_none());

        // Verify ~/.claude/settings.json contains merged settings and preserves existing fields
        let settings_content = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
        let settings_json: serde_json::Value = serde_json::from_str(&settings_content).unwrap();
        assert_eq!(
            settings_json.get("apiKeyHelper"),
            Some(&serde_json::Value::String("/bin/generate_temp_api_key.sh".to_string()))
        );
        assert_eq!(
            settings_json.get("env").and_then(|env| env.get("KEEP")),
            Some(&serde_json::Value::String("1".to_string()))
        );
        assert_eq!(
            settings_json.get("sandbox").and_then(|s| s.get("enabled")),
            Some(&serde_json::Value::Bool(true))
        );
        assert_eq!(
            settings_json.get("sandbox").and_then(|s| s.get("autoAllowBashIfSandboxed")),
            Some(&serde_json::Value::Bool(true))
        );
        assert!(settings_json.get("mcpServers").is_none());
    }

    #[test]
    #[serial]
    fn test_sync_claude_code_global_supports_legacy_settings_json() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "type": "http",
                "url": "https://example.com/mcp",
                "headers": {
                    "Authorization": "Bearer token"
                }
            }
        }
    }"#,
            )
            .unwrap();

        // Write legacy settings.json only (no claude.settings.json).
        fixture
            .with_settings(
                r#"{
        "apiKeyHelper": "/bin/legacy_helper.sh",
        "sandbox": { "enabled": true },
        "companyAnnouncements": ["legacy"]
    }"#,
            )
            .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "sync"])
            .arg("--global")
            .arg("--agent")
            .arg("claude-code")
            .assert()
            .success();

        let settings_content =
            std::fs::read_to_string(fixture.home_dir().join(".claude/settings.json")).unwrap();
        let settings_json: serde_json::Value = serde_json::from_str(&settings_content).unwrap();

        assert_eq!(
            settings_json.get("apiKeyHelper"),
            Some(&serde_json::Value::String("/bin/legacy_helper.sh".to_string()))
        );
        assert_eq!(
            settings_json.get("sandbox").and_then(|s| s.get("enabled")),
            Some(&serde_json::Value::Bool(true))
        );
        assert_eq!(
            settings_json.get("companyAnnouncements"),
            Some(&serde_json::Value::Array(vec![serde_json::Value::String("legacy".to_string())]))
        );
    }

    #[test]
    #[serial]
    fn test_sync_claude_code_global_backup_creates_two_backups() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "type": "http",
                "url": "https://example.com/mcp",
                "headers": {
                    "Authorization": "Bearer token"
                }
            }
        }
    }"#,
            )
            .unwrap();

        fixture
            .with_claude_settings(
                r#"{
        "apiKeyHelper": "/bin/generate_temp_api_key.sh"
    }"#,
            )
            .unwrap();

        // Ensure both global files exist so backup has something to copy.
        fixture.with_existing_global_config(r#"{"existingKey": "value"}"#).unwrap();

        let claude_dir = fixture.home_dir().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(claude_dir.join("settings.json"), r#"{"env":{"KEEP":"1"}}"#).unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "sync"])
            .arg("--global")
            .arg("--agent")
            .arg("claude-code")
            .arg("--backup")
            .assert()
            .success();

        let home_entries: Vec<_> = std::fs::read_dir(fixture.home_dir())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();

        assert!(
            home_entries
                .iter()
                .any(|name| name.to_string_lossy().starts_with(".claude.json.backup.")),
            "Expected ~/.claude.json backup to exist",
        );

        let claude_entries: Vec<_> = std::fs::read_dir(&claude_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();

        assert!(
            claude_entries
                .iter()
                .any(|name| name.to_string_lossy().starts_with("settings.json.backup.")),
            "Expected ~/.claude/settings.json backup to exist",
        );
    }

    #[test]
    #[serial]
    fn test_claude_code_global_dry_run_prints_both_files_and_writes_nothing() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "type": "http",
                "url": "https://example.com/mcp",
                "headers": {
                    "Authorization": "Bearer token"
                }
            }
        }
    }"#,
            )
            .unwrap();

        fixture
            .with_claude_settings(
                r#"{
        "apiKeyHelper": "/bin/generate_temp_api_key.sh"
    }"#,
            )
            .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "sync"])
            .arg("--global")
            .arg("--agent")
            .arg("claude-code")
            .arg("--dry-run")
            .assert()
            .success()
            .stdout(predicate::str::contains("MCP servers ("))
            .stdout(predicate::str::contains("Settings ("))
            .stdout(predicate::str::contains("test-server"))
            .stdout(predicate::str::contains("apiKeyHelper"));

        // Ensure no files were created in dry-run mode.
        assert!(!fixture.home_file_exists(".claude.json"));
        assert!(!fixture.home_dir().join(".claude").join("settings.json").exists());
    }

    #[test]
    #[serial]
    fn test_sync_claude_code_managed_scope_writes_managed_files() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "type": "http",
                "url": "https://example.com/mcp",
                "headers": {
                    "Authorization": "Bearer token"
                }
            }
        }
    }"#,
            )
            .unwrap();

        fixture
            .with_claude_settings(
                r#"{
        "apiKeyHelper": "/bin/generate_temp_api_key.sh",
        "companyAnnouncements": ["hello"]
    }"#,
            )
            .unwrap();

        let managed_dir = fixture.temp.path().join("managed");

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .env("CLAUDIUS_CLAUDE_CODE_MANAGED_DIR", &managed_dir)
            .args(["config", "sync"])
            .arg("--agent")
            .arg("claude-code")
            .arg("--scope")
            .arg("managed")
            .assert()
            .success();

        let mcp_content = std::fs::read_to_string(managed_dir.join("managed-mcp.json")).unwrap();
        let mcp_json: serde_json::Value = serde_json::from_str(&mcp_content).unwrap();
        assert_eq!(
            mcp_json
                .get("mcpServers")
                .and_then(|s| s.get("test-server"))
                .and_then(|t| t.get("type")),
            Some(&serde_json::Value::String("http".to_string()))
        );

        let settings_content =
            std::fs::read_to_string(managed_dir.join("managed-settings.json")).unwrap();
        let settings_json: serde_json::Value = serde_json::from_str(&settings_content).unwrap();
        assert_eq!(
            settings_json.get("apiKeyHelper"),
            Some(&serde_json::Value::String("/bin/generate_temp_api_key.sh".to_string()))
        );
        assert_eq!(
            settings_json.get("companyAnnouncements"),
            Some(&serde_json::Value::Array(vec![serde_json::Value::String("hello".to_string())]))
        );
        assert!(settings_json.get("mcpServers").is_none());
    }

    #[test]
    #[serial]
    fn test_claude_code_managed_scope_backup_creates_two_backups() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "type": "http",
                "url": "https://example.com/mcp",
                "headers": {
                    "Authorization": "Bearer token"
                }
            }
        }
    }"#,
            )
            .unwrap();

        fixture.with_claude_settings(r#"{ "apiKeyHelper": "/bin/helper.sh" }"#).unwrap();

        let managed_dir = fixture.temp.path().join("managed");
        std::fs::create_dir_all(&managed_dir).unwrap();
        std::fs::write(managed_dir.join("managed-mcp.json"), r#"{"mcpServers":{}}"#).unwrap();
        std::fs::write(managed_dir.join("managed-settings.json"), r#"{"env":{"KEEP":"1"}}"#)
            .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .env("CLAUDIUS_CLAUDE_CODE_MANAGED_DIR", &managed_dir)
            .args(["config", "sync"])
            .arg("--agent")
            .arg("claude-code")
            .arg("--scope")
            .arg("managed")
            .arg("--backup")
            .assert()
            .success();

        let entries: Vec<_> = std::fs::read_dir(&managed_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();

        assert!(
            entries
                .iter()
                .any(|name| name.to_string_lossy().starts_with("managed-mcp.json.backup.")),
            "Expected managed-mcp.json backup to exist",
        );
        assert!(
            entries
                .iter()
                .any(|name| name.to_string_lossy().starts_with("managed-settings.json.backup.")),
            "Expected managed-settings.json backup to exist",
        );
    }

    #[test]
    #[serial]
    fn test_claude_code_managed_scope_dry_run_prints_and_writes_nothing() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "type": "http",
                "url": "https://example.com/mcp",
                "headers": {
                    "Authorization": "Bearer token"
                }
            }
        }
    }"#,
            )
            .unwrap();

        fixture.with_claude_settings(r#"{ "apiKeyHelper": "/bin/helper.sh" }"#).unwrap();

        let managed_dir = fixture.temp.path().join("managed");

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .env("CLAUDIUS_CLAUDE_CODE_MANAGED_DIR", &managed_dir)
            .args(["config", "sync"])
            .arg("--agent")
            .arg("claude-code")
            .arg("--scope")
            .arg("managed")
            .arg("--dry-run")
            .assert()
            .success()
            .stdout(predicate::str::contains("managed-mcp.json"))
            .stdout(predicate::str::contains("managed-settings.json"))
            .stdout(predicate::str::contains("test-server"))
            .stdout(predicate::str::contains("apiKeyHelper"));

        assert!(!managed_dir.join("managed-mcp.json").exists());
        assert!(!managed_dir.join("managed-settings.json").exists());
    }

    #[test]
    #[serial]
    fn test_sync_claude_code_local_scope_writes_per_project_mcp_and_settings_local() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "type": "http",
                "url": "https://example.com/mcp",
                "headers": {
                    "Authorization": "Bearer token"
                }
            }
        }
    }"#,
            )
            .unwrap();

        fixture
            .with_claude_settings(
                r#"{
        "apiKeyHelper": "/bin/generate_temp_api_key.sh",
        "companyAnnouncements": ["hello"]
    }"#,
            )
            .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "sync"])
            .arg("--agent")
            .arg("claude-code")
            .arg("--scope")
            .arg("local")
            .assert()
            .success();

        assert!(!fixture.project_file_exists(".mcp.json"));
        assert!(!fixture.project.join(".claude").join("settings.json").exists());

        let home_content = fixture.read_home_file(".claude.json").unwrap();
        let home_json: serde_json::Value = serde_json::from_str(&home_content).unwrap();

        let project_key = fixture.project.to_string_lossy().to_string();
        assert_eq!(
            home_json
                .get(&project_key)
                .and_then(|p| p.get("mcpServers"))
                .and_then(|s| s.get("test-server"))
                .and_then(|t| t.get("type")),
            Some(&serde_json::Value::String("http".to_string()))
        );

        let local_settings_path = fixture.project.join(".claude").join("settings.local.json");
        let settings_content = std::fs::read_to_string(&local_settings_path).unwrap();
        let settings_json: serde_json::Value = serde_json::from_str(&settings_content).unwrap();
        assert_eq!(
            settings_json.get("apiKeyHelper"),
            Some(&serde_json::Value::String("/bin/generate_temp_api_key.sh".to_string()))
        );
        assert!(settings_json.get("mcpServers").is_none());
    }

    #[test]
    #[serial]
    fn test_claude_code_local_scope_backup_creates_two_backups() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture
            .with_mcp_servers(
                r#"{"mcpServers":{"test-server":{"command":"test","args":[],"env":{}}}}"#,
            )
            .unwrap();
        fixture.with_claude_settings(r#"{ "apiKeyHelper": "/bin/helper.sh" }"#).unwrap();

        // Ensure both target files exist so backup has something to copy.
        fixture.with_existing_global_config(r#"{"existingKey": "value"}"#).unwrap();
        let local_settings_dir = fixture.project.join(".claude");
        std::fs::create_dir_all(&local_settings_dir).unwrap();
        std::fs::write(local_settings_dir.join("settings.local.json"), r#"{"env":{"KEEP":"1"}}"#)
            .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "sync"])
            .arg("--agent")
            .arg("claude-code")
            .arg("--scope")
            .arg("local")
            .arg("--backup")
            .assert()
            .success();

        let home_entries: Vec<_> = std::fs::read_dir(fixture.home_dir())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();

        assert!(
            home_entries
                .iter()
                .any(|name| name.to_string_lossy().starts_with(".claude.json.backup.")),
            "Expected ~/.claude.json backup to exist",
        );

        let local_entries: Vec<_> = std::fs::read_dir(&local_settings_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();

        assert!(
            local_entries
                .iter()
                .any(|name| name.to_string_lossy().starts_with("settings.local.json.backup.")),
            "Expected .claude/settings.local.json backup to exist",
        );
    }

    #[test]
    #[serial]
    fn test_claude_code_local_scope_dry_run_prints_and_writes_nothing() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture
            .with_mcp_servers(
                r#"{"mcpServers":{"test-server":{"command":"test","args":[],"env":{}}}}"#,
            )
            .unwrap();
        fixture.with_claude_settings(r#"{ "apiKeyHelper": "/bin/helper.sh" }"#).unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "sync"])
            .arg("--agent")
            .arg("claude-code")
            .arg("--scope")
            .arg("local")
            .arg("--dry-run")
            .assert()
            .success()
            .stdout(predicate::str::contains(".claude.json"))
            .stdout(predicate::str::contains("settings.local.json"))
            .stdout(predicate::str::contains("test-server"))
            .stdout(predicate::str::contains("apiKeyHelper"));

        assert!(!fixture.home_file_exists(".claude.json"));
        assert!(!fixture.project.join(".claude/settings.local.json").exists());
    }

    #[test]
    #[serial]
    fn test_gemini_legacy_settings_migrated_on_sync() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "command": "test",
                "args": [],
                "env": {}
            }
        }
    }"#,
            )
            .unwrap();

        // Legacy Gemini v1-style keys (should be migrated into category-based schema).
        fixture
            .with_gemini_settings(
                r#"{
        "contextFileName": "GEMINI.md",
        "autoAccept": true,
        "theme": "GitHub",
        "usageStatisticsEnabled": false
    }"#,
            )
            .unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync"])
            .arg("--agent")
            .arg("gemini")
            .assert()
            .success();

        let content = fixture.read_project_file(".gemini/settings.json").unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(
            json.get("context").and_then(|c| c.get("fileName")),
            Some(&serde_json::Value::String("GEMINI.md".to_string()))
        );
        assert_eq!(
            json.get("tools").and_then(|t| t.get("autoAccept")),
            Some(&serde_json::Value::Bool(true))
        );
        assert_eq!(
            json.get("ui").and_then(|u| u.get("theme")),
            Some(&serde_json::Value::String("GitHub".to_string()))
        );
        assert_eq!(
            json.get("privacy").and_then(|p| p.get("usageStatisticsEnabled")),
            Some(&serde_json::Value::Bool(false))
        );

        // Legacy keys should be absent after migration.
        assert!(json.get("contextFileName").is_none());
        assert!(json.get("autoAccept").is_none());
        assert!(json.get("theme").is_none());
        assert!(json.get("usageStatisticsEnabled").is_none());
    }

    #[test]
    #[serial]
    fn test_sync_without_settings() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create only mcpServers.json
        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "command": "test",
                "args": [],
                "env": {}
            }
        }
    }"#,
            )
            .unwrap();

        // Run sync
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync"])
            .arg("--agent")
            .arg("gemini")
            .assert()
            .success();

        // Gemini CLI stores settings and MCP servers together in .gemini/settings.json
        assert!(!fixture.project_file_exists(".mcp.json"));

        let content = fixture.read_project_file(".gemini/settings.json").unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Check MCP servers
        assert_eq!(
            json.get("mcpServers")
                .and_then(|s| s.get("test-server"))
                .and_then(|t| t.get("command")),
            Some(&serde_json::Value::String("test".to_string()))
        );

        // Check no settings added
        assert!(json.get("apiKeyHelper").is_none());
        assert!(json.get("cleanupPeriodDays").is_none());
        assert!(!fixture.project_file_exists("gemini/settings.json"));
    }

    #[test]
    #[serial]
    fn test_dry_run_with_settings() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create mcpServers.json
        fixture
            .with_mcp_servers(
                r#"{
        "mcpServers": {
            "test-server": {
                "command": "test",
                "args": [],
                "env": {}
            }
        }
    }"#,
            )
            .unwrap();

        // Create gemini.settings.json
        fixture
            .with_gemini_settings(
                r#"{
        "general": {
            "preferredEditor": "code"
        }
    }"#,
            )
            .unwrap();

        // Run sync with dry-run
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .args(["config", "sync"])
            .arg("--agent")
            .arg("gemini")
            .arg("--dry-run")
            .assert()
            .success()
            .stdout(predicate::str::contains("test-server"))
            .stdout(predicate::str::contains("preferredEditor"));

        // Verify files were NOT created
        assert!(!fixture.project_file_exists(".mcp.json"));
        assert!(!fixture.project_file_exists("gemini"));
    }
}
