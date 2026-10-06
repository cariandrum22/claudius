use assert_cmd::Command;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    /// Isolated `HOME` / `XDG_CONFIG_HOME` / project layout for `OpenCode` tests.
    struct OpenCodeFixture {
        _temp: TempDir,
        home: PathBuf,
        xdg: PathBuf,
        project: PathBuf,
    }

    impl OpenCodeFixture {
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
                .env("XDG_CONFIG_HOME", &self.xdg)
                .env_remove("OPENCODE_CONFIG_DIR");
            cmd
        }
    }

    /// Look up a JSON pointer, failing the test when it is absent.
    fn at<'a>(value: &'a Value, pointer: &str) -> &'a Value {
        value.pointer(pointer).expect("JSON pointer should exist in synced config")
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
      "headers": {"Authorization": "Bearer token"}
    }
  }
}"#;

    #[test]
    fn test_opencode_project_sync_writes_v2_mcp_servers() {
        let fixture = OpenCodeFixture::new();
        fixture.source("mcpServers.json", MCP_SERVERS);

        fixture
            .claudius()
            .args(["config", "sync", "--agent", "opencode"])
            .assert()
            .success();

        let config = read_json(&fixture.project.join("opencode.json"));
        assert_eq!(
            *at(&config, "/mcp/servers/filesystem"),
            json!({
                "type": "local",
                "command": ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
                "environment": {"ALLOWED_PATHS": "/tmp"}
            })
        );
        assert_eq!(
            *at(&config, "/mcp/servers/remote"),
            json!({
                "type": "remote",
                "url": "https://mcp.example.com/mcp",
                "headers": {"Authorization": "Bearer token"}
            })
        );
        assert!(config.get("mcpServers").is_none(), "Claude-style key must not be written");
        assert!(!fixture.project.join(".mcp.json").exists());
    }

    #[test]
    fn test_opencode_project_sync_merges_settings_and_preserves_existing_config() {
        let fixture = OpenCodeFixture::new();
        fixture.source("mcpServers.json", MCP_SERVERS).source(
            "opencode.settings.json",
            r#"{"permissions": [{"action": "edit", "resource": "*", "effect": "ask"}]}"#,
        );
        fs::write(
            fixture.project.join("opencode.json"),
            r#"{
  "model": "anthropic/claude-sonnet-4-5",
  "mcp": {
    "filesystem": {"type": "local", "command": ["old-fs"]},
    "servers": {"existing": {"type": "remote", "url": "https://existing.example.com"}}
  }
}"#,
        )
        .expect("existing opencode.json should be written");

        fixture
            .claudius()
            .args(["config", "sync", "--agent", "opencode"])
            .assert()
            .success();

        let config = read_json(&fixture.project.join("opencode.json"));
        assert_eq!(at(&config, "/model"), "anthropic/claude-sonnet-4-5");
        assert_eq!(
            *at(&config, "/permissions"),
            json!([{"action": "edit", "resource": "*", "effect": "ask"}])
        );
        assert_eq!(
            at(&config, "/mcp/servers/existing/url"),
            "https://existing.example.com",
            "servers absent from the source must be preserved"
        );
        assert!(
            at(&config, "/mcp").get("filesystem").is_none(),
            "a same-named V1 flat entry must be replaced by the v2 entry"
        );
        assert_eq!(at(&config, "/mcp/servers/filesystem/command/0"), "npx");
    }

    #[test]
    fn test_opencode_global_sync_uses_xdg_config_dir() {
        let fixture = OpenCodeFixture::new();
        fixture.source("mcpServers.json", MCP_SERVERS);

        fixture
            .claudius()
            .args(["config", "sync", "--global", "--agent", "opencode"])
            .assert()
            .success();

        let config = read_json(&fixture.xdg.join("opencode").join("opencode.json"));
        assert_eq!(at(&config, "/mcp/servers/remote/type"), "remote");
        assert!(!fixture.home.join(".claude.json").exists());
    }

    #[test]
    fn test_opencode_global_sync_honors_opencode_config_dir() {
        let fixture = OpenCodeFixture::new();
        fixture.source("mcpServers.json", MCP_SERVERS);
        let custom_dir = fixture.home.join("custom-opencode");

        fixture
            .claudius()
            .env("OPENCODE_CONFIG_DIR", &custom_dir)
            .args(["config", "sync", "--global", "--agent", "opencode"])
            .assert()
            .success();

        assert!(custom_dir.join("opencode.json").exists());
        assert!(!fixture.xdg.join("opencode").join("opencode.json").exists());
    }

    #[test]
    fn test_opencode_dry_run_does_not_write() {
        let fixture = OpenCodeFixture::new();
        fixture.source("mcpServers.json", MCP_SERVERS);

        let output = fixture
            .claudius()
            .args(["config", "sync", "--agent", "opencode", "--dry-run"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let stdout = String::from_utf8(output).expect("stdout should be UTF-8");
        assert!(stdout.contains("\"servers\""), "dry run should print the v2 layout");
        assert!(!fixture.project.join("opencode.json").exists());
    }

    #[test]
    fn test_opencode_skills_sync_project_and_global() {
        let fixture = OpenCodeFixture::new();
        fixture
            .source(
                "skills/greet/skill.yaml",
                "version: 1\nname: greet\ndescription: Greet the user\ntargets:\n  opencode:\n    disable-model-invocation: true\n",
            )
            .source("skills/greet/instructions.md", "Say hello.\n");

        fixture
            .claudius()
            .args(["skills", "sync", "--agent", "opencode"])
            .assert()
            .success();
        let project_skill = fs::read_to_string(
            fixture.project.join(".opencode").join("skills").join("greet").join("SKILL.md"),
        )
        .expect("project skill should be deployed");
        assert!(project_skill.contains("name: greet"));
        assert!(project_skill.contains("disable-model-invocation: true"));
        assert!(project_skill.contains("Say hello."));

        fixture
            .claudius()
            .args(["skills", "sync", "--global", "--agent", "opencode"])
            .assert()
            .success();
        assert!(fixture
            .xdg
            .join("opencode")
            .join("skills")
            .join("greet")
            .join("SKILL.md")
            .exists());
    }

    #[test]
    fn test_opencode_context_append_targets_agents_md() {
        let fixture = OpenCodeFixture::new();
        fixture.source("rules/security.md", "# Security\n");

        fixture
            .claudius()
            .args(["context", "append", "security", "--agent", "opencode"])
            .assert()
            .success();

        assert!(fixture.project.join("AGENTS.md").exists());
        assert!(!fixture.project.join("CLAUDE.md").exists());
    }

    #[test]
    fn test_opencode_sync_maps_codex_timeouts_without_warnings() {
        let fixture = OpenCodeFixture::new();
        fixture.source(
            "mcpServers.json",
            r#"{"mcpServers": {"slow": {"command": "srv", "startup_timeout_sec": 300, "tool_timeout_sec": 120}}}"#,
        );

        let output = fixture
            .claudius()
            .args(["config", "sync", "--agent", "opencode"])
            .output()
            .expect("sync should run");
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(output.status.success(), "{combined}");
        let config = read_json(&fixture.project.join("opencode.json"));
        assert_eq!(
            *at(&config, "/mcp/servers/slow/timeout"),
            json!({"startup": 300_000, "execution": 120_000})
        );
        assert!(!combined.contains("startup_timeout_sec"), "{combined}");
        assert!(!combined.contains("tool_timeout_sec"), "{combined}");
    }

    #[test]
    fn test_opencode_validate_reports_unsupported_mcp_fields() {
        let fixture = OpenCodeFixture::new();
        fixture.source(
            "mcpServers.json",
            r#"{"mcpServers": {"x": {"command": "srv", "autoApprove": ["tool"]}}}"#,
        );

        let output = fixture
            .claudius()
            .args(["config", "validate", "--agent", "opencode"])
            .output()
            .expect("validate should run");
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(combined.contains("mcpServers.x.autoApprove"), "{combined}");
    }
}
