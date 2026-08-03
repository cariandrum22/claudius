use crate::fixtures::TestFixture;
use assert_cmd::Command;
use predicates::prelude::*;
use serial_test::serial;
use std::fs;

#[cfg(test)]
mod tests {
    use super::*;

    fn migrate_cmd(fixture: &TestFixture, extra_args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(&fixture.project)
            .env("XDG_CONFIG_HOME", fixture.config_home())
            .env("HOME", fixture.home_dir())
            .args(["config", "migrate"])
            .args(extra_args);
        cmd
    }

    #[test]
    #[serial]
    fn test_config_migrate_reports_nothing_for_clean_sources() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture
            .with_claude_settings(
                r#"{
  "$schema": "https://json.schemastore.org/claude-code-settings.json",
  "attribution": { "commit": "", "pr": "" }
}
"#,
            )
            .unwrap();

        migrate_cmd(&fixture, &[])
            .assert()
            .success()
            .stdout(predicate::str::contains("nothing to migrate"));
    }

    #[test]
    #[serial]
    fn test_config_migrate_dry_run_shows_diff_without_writing() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        let original = r#"{ "includeCoAuthoredBy": false }"#;
        fixture.with_claude_settings(original).unwrap();

        migrate_cmd(&fixture, &["--dry-run"])
            .assert()
            .success()
            .stdout(predicate::str::contains("attribution"))
            .stdout(predicate::str::contains("-{ \"includeCoAuthoredBy\": false }"))
            .stdout(predicate::str::contains("Dry run: no files were modified"));

        let untouched = fs::read_to_string(fixture.config.join("claude.settings.json")).unwrap();
        assert_eq!(untouched, original);
    }

    #[test]
    #[serial]
    fn test_config_migrate_rewrites_claude_settings_with_backup() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture
            .with_claude_settings(r#"{ "cleanupPeriodDays": 30, "includeCoAuthoredBy": false }"#)
            .unwrap();

        migrate_cmd(&fixture, &["--agent", "claude-code"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Backup created:"))
            .stdout(predicate::str::contains("Migration complete: 1 file(s) updated"));

        let migrated = fs::read_to_string(fixture.config.join("claude.settings.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&migrated).unwrap();
        assert_eq!(value.get("includeCoAuthoredBy"), None);
        assert_eq!(
            value.get("attribution"),
            Some(&serde_json::json!({ "commit": "", "pr": "", "sessionUrl": false }))
        );
        assert_eq!(value.get("cleanupPeriodDays"), Some(&serde_json::json!(30)));

        let backups: Vec<_> = fs::read_dir(&fixture.config)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_name().to_string_lossy().starts_with("claude.settings.json.backup.")
            })
            .collect();
        assert_eq!(backups.len(), 1);

        // Second run is idempotent.
        migrate_cmd(&fixture, &["--agent", "claude-code"])
            .assert()
            .success()
            .stdout(predicate::str::contains("nothing to migrate"));
    }

    #[test]
    #[serial]
    fn test_config_migrate_codex_preserves_comments() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fixture
            .with_codex_settings(
                "# Keep this comment\nbackground_terminal_timeout = 300\nmodel = \"gpt-5.5\"\n",
            )
            .unwrap();
        fs::write(
            fixture.config.join("codex.requirements.toml"),
            "# Admin policies\nallowed_approval_policies = [\"untrusted\", \"on-failure\"]\n",
        )
        .unwrap();

        migrate_cmd(&fixture, &["--agent", "codex"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Migration complete: 2 file(s) updated"));

        let settings = fs::read_to_string(fixture.config.join("codex.settings.toml")).unwrap();
        assert!(settings.contains("# Keep this comment"));
        assert!(settings.contains("background_terminal_max_timeout = 300"));
        assert!(!settings.contains("background_terminal_timeout ="));
        assert!(settings.contains("model = \"gpt-5.5\""));

        let requirements =
            fs::read_to_string(fixture.config.join("codex.requirements.toml")).unwrap();
        assert!(requirements.contains("# Admin policies"));
        assert!(!requirements.contains("on-failure"));
        assert!(requirements.contains("\"untrusted\""));
    }

    #[test]
    #[serial]
    fn test_config_migrate_without_agent_migrates_all_sources() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        fs::write(fixture.config.join("config.toml"), "[default]\nagent = \"codex\"\n").unwrap();
        fixture.with_claude_settings(r#"{ "includeCoAuthoredBy": false }"#).unwrap();
        fixture.with_codex_settings("background_terminal_timeout = 300\n").unwrap();

        migrate_cmd(&fixture, &[])
            .assert()
            .success()
            .stdout(predicate::str::contains("Migration complete: 2 file(s) updated"));

        let claude = fs::read_to_string(fixture.config.join("claude.settings.json")).unwrap();
        assert!(claude.contains("attribution"));
        let codex = fs::read_to_string(fixture.config.join("codex.settings.toml")).unwrap();
        assert!(codex.contains("background_terminal_max_timeout"));
    }
}
