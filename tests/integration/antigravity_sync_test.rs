use assert_cmd::Command;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    /// Isolated `HOME` / `XDG_CONFIG_HOME` / project layout for Antigravity tests.
    struct AntigravityFixture {
        _temp: TempDir,
        home: PathBuf,
        xdg: PathBuf,
        project: PathBuf,
    }

    impl AntigravityFixture {
        fn new() -> Self {
            let temp = TempDir::new().expect("temp dir should be created");
            let home = temp.path().join("home");
            let xdg = temp.path().join("xdg");
            let project = temp.path().join("project");
            for dir in [&home, &xdg.join("claudius"), &project] {
                fs::create_dir_all(dir).expect("fixture dir should be created");
            }
            Self { _temp: temp, home, xdg, project }
        }

        fn source(&self, name: &str, content: &str) -> &Self {
            let path = self.xdg.join("claudius").join(name);
            fs::create_dir_all(path.parent().expect("source path has parent"))
                .expect("source parent should be created");
            fs::write(path, content).expect("source file should be written");
            self
        }

        fn claudius(&self) -> Command {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
            cmd.current_dir(&self.project)
                .env("HOME", &self.home)
                .env("XDG_CONFIG_HOME", &self.xdg);
            cmd
        }

        fn shared_config(&self) -> PathBuf {
            self.home.join(".gemini").join("config")
        }

        fn cli_settings(&self) -> PathBuf {
            self.home.join(".gemini").join("antigravity-cli").join("settings.json")
        }
    }

    fn write_json(path: &Path, value: &Value) {
        fs::create_dir_all(path.parent().expect("path has parent"))
            .expect("parent should be created");
        fs::write(path, serde_json::to_string_pretty(value).expect("JSON should serialize"))
            .expect("JSON file should be written");
    }

    fn read_json(path: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(path).expect("JSON file should exist"))
            .expect("file should contain valid JSON")
    }

    const MCP_SERVERS: &str = r#"{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
      "env": {"ALLOWED_PATHS": "/tmp"}
    },
    "remote": {
      "type": "http",
      "url": "https://mcp.example.com/mcp",
      "headers": {"Authorization": "Bearer token"},
      "enabled": false
    }
  }
}"#;

    #[test]
    fn test_antigravity_project_sync_writes_mcp_config_and_preserves_unmanaged_servers() {
        let fixture = AntigravityFixture::new();
        fixture
            .source("mcpServers.json", MCP_SERVERS)
            .source("antigravity.settings.json", r#"{"verbosity": "medium"}"#);
        let target = fixture.project.join(".agents").join("mcp_config.json");
        write_json(
            &target,
            &json!({
                "mcpServers": {
                    "filesystem": {"command": "old"},
                    "manual": {"serverUrl": "https://manual.example.com", "timeoutSeconds": 30}
                },
                "futureKey": true
            }),
        );

        fixture
            .claudius()
            .args(["config", "sync", "--agent", "antigravity"])
            .assert()
            .success();

        assert_eq!(
            read_json(&target),
            json!({
                "futureKey": true,
                "mcpServers": {
                    "filesystem": {
                        "command": "npx",
                        "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
                        "env": {"ALLOWED_PATHS": "/tmp"}
                    },
                    "manual": {"serverUrl": "https://manual.example.com", "timeoutSeconds": 30},
                    "remote": {
                        "serverUrl": "https://mcp.example.com/mcp",
                        "headers": {"Authorization": "Bearer token"},
                        "disabled": true
                    }
                }
            })
        );
        assert!(!fixture.cli_settings().exists(), "CLI settings are global only");
        assert!(!fixture.project.join(".mcp.json").exists());
    }

    #[test]
    fn test_antigravity_global_sync_writes_shared_mcp_and_merges_cli_settings() {
        let fixture = AntigravityFixture::new();
        fixture.source("mcpServers.json", MCP_SERVERS).source(
            "antigravity.settings.json",
            r#"{"permissions": {"deny": ["command(sudo)"]}, "verbosity": "medium"}"#,
        );
        write_json(
            &fixture.cli_settings(),
            &json!({"colorScheme": "dark", "permissions": {"allow": ["read_file(*)"]}}),
        );

        fixture
            .claudius()
            .args(["config", "sync", "--global", "--agent", "antigravity"])
            .assert()
            .success();

        let mcp = read_json(&fixture.shared_config().join("mcp_config.json"));
        assert_eq!(
            mcp.pointer("/mcpServers/remote/serverUrl"),
            Some(&json!("https://mcp.example.com/mcp"))
        );
        assert_eq!(
            read_json(&fixture.cli_settings()),
            json!({
                "colorScheme": "dark",
                "permissions": {"allow": ["read_file(*)"], "deny": ["command(sudo)"]},
                "verbosity": "medium"
            })
        );
        assert!(!fixture.home.join(".gemini").join("settings.json").exists());
    }

    #[test]
    fn test_antigravity_sync_refuses_to_rewrite_jsonc_mcp_config() {
        let fixture = AntigravityFixture::new();
        fixture.source("mcpServers.json", MCP_SERVERS);
        let target = fixture.project.join(".agents").join("mcp_config.json");
        fs::create_dir_all(target.parent().expect("target has parent"))
            .expect("target parent should be created");
        let original = "{\n  // managed by hand\n  \"mcpServers\": {}\n}\n";
        fs::write(&target, original).expect("JSONC target should be written");

        fixture
            .claudius()
            .args(["config", "sync", "--agent", "antigravity"])
            .assert()
            .failure();

        assert_eq!(fs::read_to_string(&target).expect("target should exist"), original);
    }

    #[test]
    fn test_antigravity_dry_run_does_not_write() {
        let fixture = AntigravityFixture::new();
        fixture.source("mcpServers.json", MCP_SERVERS);

        let output = fixture
            .claudius()
            .args(["config", "sync", "--agent", "antigravity", "--dry-run"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let stdout = String::from_utf8(output).expect("stdout should be UTF-8");
        assert!(stdout.contains("\"serverUrl\""), "dry run should print the Antigravity shape");
        assert!(!fixture.project.join(".agents").join("mcp_config.json").exists());
    }

    #[test]
    fn test_antigravity_skills_and_agents_sync_project_and_global() {
        let fixture = AntigravityFixture::new();
        fixture
            .source("mcpServers.json", r#"{"mcpServers": {}}"#)
            .source(
                "skills/greet/skill.yaml",
                "version: 1\nname: greet\ndescription: Greet the user\n",
            )
            .source("skills/greet/instructions.md", "Say hello.\n")
            .source(
                "agents/antigravity/reviewer.md",
                "---\nname: reviewer\ndescription: Reviews code\n---\nReview carefully.\n",
            );

        fixture
            .claudius()
            .args(["config", "sync", "--agent", "antigravity"])
            .assert()
            .success();
        let project_skill = fs::read_to_string(
            fixture.project.join(".agents").join("skills").join("greet").join("SKILL.md"),
        )
        .expect("project skill should be deployed");
        assert!(project_skill.contains("name: greet"));
        assert!(project_skill.contains("Say hello."));
        assert!(fixture.project.join(".agents").join("agents").join("reviewer.md").exists());

        fixture
            .claudius()
            .args(["config", "sync", "--global", "--agent", "antigravity"])
            .assert()
            .success();
        assert!(fixture.shared_config().join("skills").join("greet").join("SKILL.md").exists());
        assert!(fixture.shared_config().join("agents").join("reviewer.md").exists());
    }

    #[test]
    fn test_antigravity_context_append_targets_agents_md() {
        let fixture = AntigravityFixture::new();
        fixture.source("rules/security.md", "# Security\n");

        fixture
            .claudius()
            .args(["context", "append", "security", "--agent", "antigravity"])
            .assert()
            .success();

        assert!(fixture.project.join("AGENTS.md").exists());
        assert!(!fixture.project.join("GEMINI.md").exists());
    }

    #[test]
    fn test_antigravity_validate_reports_unsupported_mcp_fields() {
        let fixture = AntigravityFixture::new();
        fixture.source(
            "mcpServers.json",
            r#"{"mcpServers": {"x": {"command": "srv", "trust": true}}}"#,
        );

        let output = fixture
            .claudius()
            .args(["config", "validate", "--agent", "antigravity"])
            .output()
            .expect("validate should run");
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(combined.contains("mcpServers.x.trust"), "{combined}");
    }
}
