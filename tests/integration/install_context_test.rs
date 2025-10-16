use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;
use std::fs;

#[cfg(test)]
mod tests {
    use super::*;

    // ========== Basic context install tests ==========

    #[test]
    fn test_install_context_basic() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let config_dir = assert_fs::TempDir::new().unwrap();

        // Create test rule in config directory (under claudius subdirectory)
        let claudius_dir = config_dir.child("claudius");
        let rules_dir = claudius_dir.child("rules");
        rules_dir.create_dir_all().unwrap();

        let test_rule = rules_dir.child("test-rule.md");
        test_rule.write_str("# Test Rule\nThis is a test rule.").unwrap();

        // Run context install command
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .env("XDG_CONFIG_HOME", config_dir.path())
            .args(["context", "install"])
            .arg("test-rule")
            .assert()
            .success();

        // Verify rule was copied
        let installed_rule = temp_dir.child(".agents/rules/test-rule.md");
        installed_rule.assert(predicate::path::exists());
        installed_rule.assert(predicate::str::contains("# Test Rule"));

        // Verify reference directive was added to CLAUDE.md
        let claude_md = temp_dir.child("CLAUDE.md");
        claude_md.assert(predicate::path::exists());
        claude_md.assert(predicate::str::contains("<!-- CLAUDIUS_RULES_START -->"));
        claude_md.assert(predicate::str::contains("# External Rule References"));
        claude_md.assert(predicate::str::contains(".agents/rules/test-rule.md"));
        claude_md.assert(predicate::str::contains("<!-- CLAUDIUS_RULES_END -->"));
    }

    #[test]
    fn test_install_context_idempotent() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let config_dir = assert_fs::TempDir::new().unwrap();

        // Create test rule
        let claudius_dir = config_dir.child("claudius");
        let rules_dir = claudius_dir.child("rules");
        rules_dir.create_dir_all().unwrap();

        let test_rule = rules_dir.child("test-rule.md");
        test_rule.write_str("# Test Rule\nThis is a test rule.").unwrap();

        // Pre-create CLAUDE.md with the directive
        let claude_md = temp_dir.child("CLAUDE.md");
        claude_md
            .write_str(
                "# Existing Content\n\
                <!-- CLAUDIUS_RULES_START -->\n\
                # External Rule References\n\
                \n\
                The following rules from `.agents/rules` are installed:\n\
                \n\
                - `.agents/rules/test-rule.md`: test-rule\n\
                \n\
                Read these files to understand the project conventions and guidelines.\n\
                <!-- CLAUDIUS_RULES_END -->\n",
            )
            .unwrap();

        // Run context install command
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .env("XDG_CONFIG_HOME", config_dir.path())
            .args(["context", "install"])
            .arg("test-rule")
            .assert()
            .success()
            .stdout(predicate::str::contains("Updated reference directive"));

        // Verify directive appears only once
        let content = fs::read_to_string(claude_md.path()).unwrap();
        let start_count = content.matches("<!-- CLAUDIUS_RULES_START -->").count();
        let end_count = content.matches("<!-- CLAUDIUS_RULES_END -->").count();
        assert_eq!(start_count, 1, "Start marker should appear only once");
        assert_eq!(end_count, 1, "End marker should appear only once");
    }

    #[test]
    fn test_install_context_multiple_rules() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let config_dir = assert_fs::TempDir::new().unwrap();

        // Create multiple test rules
        let claudius_dir = config_dir.child("claudius");
        let rules_dir = claudius_dir.child("rules");
        rules_dir.create_dir_all().unwrap();

        let rule1 = rules_dir.child("rule1.md");
        rule1.write_str("# Rule 1").unwrap();

        let rule2 = rules_dir.child("rule2.md");
        rule2.write_str("# Rule 2").unwrap();

        let rule3 = rules_dir.child("rule3.md");
        rule3.write_str("# Rule 3").unwrap();

        // Run context install with multiple rules
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .env("XDG_CONFIG_HOME", config_dir.path())
            .args(["context", "install"])
            .arg("rule1")
            .arg("rule2")
            .arg("rule3")
            .assert()
            .success()
            .stdout(predicate::str::contains("Successfully installed 3 rule(s)"));

        // Verify all rules were copied
        temp_dir.child(".agents/rules/rule1.md").assert(predicate::path::exists());
        temp_dir.child(".agents/rules/rule2.md").assert(predicate::path::exists());
        temp_dir.child(".agents/rules/rule3.md").assert(predicate::path::exists());
    }

    #[test]
    fn test_install_context_missing_rule() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let config_dir = assert_fs::TempDir::new().unwrap();

        // Create rules directory without the requested rule
        let claudius_dir = config_dir.child("claudius");
        let rules_dir = claudius_dir.child("rules");
        rules_dir.create_dir_all().unwrap();

        // Run context install with non-existent rule
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .env("XDG_CONFIG_HOME", config_dir.path())
            .args(["context", "install"])
            .arg("non-existent-rule")
            .assert()
            .failure()
            .stderr(predicate::str::contains("No valid rules found"));
    }

    #[test]
    fn test_install_context_with_agent() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let config_dir = assert_fs::TempDir::new().unwrap();

        // Create test rule (under claudius subdirectory)
        let claudius_dir = config_dir.child("claudius");
        let rules_dir = claudius_dir.child("rules");
        rules_dir.create_dir_all().unwrap();

        let test_rule = rules_dir.child("test-rule.md");
        test_rule.write_str("# Test Rule").unwrap();

        // Run context install with Gemini agent
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .env("XDG_CONFIG_HOME", config_dir.path())
            .args(["context", "install"])
            .arg("test-rule")
            .arg("--agent")
            .arg("gemini")
            .assert()
            .success();

        // Verify AGENTS.md was created instead of CLAUDE.md (Gemini uses AGENTS.md)
        let agents_md = temp_dir.child("AGENTS.md");
        agents_md.assert(predicate::path::exists());
        agents_md.assert(predicate::str::contains("<!-- CLAUDIUS_RULES_START -->"));
        agents_md.assert(predicate::str::contains("# External Rule References"));
        agents_md.assert(predicate::str::contains(".agents/rules/test-rule.md"));
        agents_md.assert(predicate::str::contains("<!-- CLAUDIUS_RULES_END -->"));
    }

    #[test]
    fn test_install_context_with_path() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let config_dir = assert_fs::TempDir::new().unwrap();
        let target_dir = temp_dir.child("project");
        target_dir.create_dir_all().unwrap();

        // Create test rule (under claudius subdirectory)
        let claudius_dir = config_dir.child("claudius");
        let rules_dir = claudius_dir.child("rules");
        rules_dir.create_dir_all().unwrap();

        let test_rule = rules_dir.child("test-rule.md");
        test_rule.write_str("# Test Rule").unwrap();

        // Run context install with custom path
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .env("XDG_CONFIG_HOME", config_dir.path())
            .args(["context", "install"])
            .arg("test-rule")
            .arg("--path")
            .arg("project")
            .assert()
            .success();

        // Verify rule was installed in the target directory
        let installed_rule = target_dir.child(".agents/rules/test-rule.md");
        installed_rule.assert(predicate::path::exists());

        let claude_md = target_dir.child("CLAUDE.md");
        claude_md.assert(predicate::path::exists());
    }

    #[test]
    fn test_install_context_with_custom_install_dir() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let config_dir = assert_fs::TempDir::new().unwrap();

        // Create test rule (under claudius subdirectory)
        let claudius_dir = config_dir.child("claudius");
        let rules_dir = claudius_dir.child("rules");
        rules_dir.create_dir_all().unwrap();

        let test_rule = rules_dir.child("test-rule.md");
        test_rule.write_str("# Test Rule").unwrap();

        // Run context install with custom install directory
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .env("XDG_CONFIG_HOME", config_dir.path())
            .args(["context", "install"])
            .arg("test-rule")
            .arg("--install-dir")
            .arg("./.custom/rules")
            .assert()
            .success();

        // Verify rule was installed in custom directory
        let installed_rule = temp_dir.child(".custom/rules/test-rule.md");
        installed_rule.assert(predicate::path::exists());

        // Verify reference directive uses custom path
        let claude_md = temp_dir.child("CLAUDE.md");
        claude_md.assert(predicate::path::exists());
        claude_md.assert(predicate::str::contains("<!-- CLAUDIUS_RULES_START -->"));
        claude_md.assert(predicate::str::contains("# External Rule References"));
        claude_md.assert(predicate::str::contains(".custom/rules/test-rule.md"));
        claude_md.assert(predicate::str::contains("<!-- CLAUDIUS_RULES_END -->"));
    }

    // ========== context install --all tests ==========

    #[test]
    fn test_install_context_all_option() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let config_dir = assert_fs::TempDir::new().unwrap();

        // Create multiple test rules in various directories
        let claudius_dir = config_dir.child("claudius");
        let rules_dir = claudius_dir.child("rules");
        rules_dir.create_dir_all().unwrap();

        // Create rules in root directory
        let security_rule = rules_dir.child("security.md");
        security_rule.write_str("# Security Rule").unwrap();

        let testing_rule = rules_dir.child("testing.md");
        testing_rule.write_str("# Testing Rule").unwrap();

        // Create rules in subdirectory
        let advanced_dir = rules_dir.child("advanced");
        advanced_dir.create_dir_all().unwrap();

        let advanced_security = advanced_dir.child("security-advanced.md");
        advanced_security.write_str("# Advanced Security").unwrap();

        let advanced_testing = advanced_dir.child("testing-advanced.md");
        advanced_testing.write_str("# Advanced Testing").unwrap();

        // Run context install with --all option
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .env("XDG_CONFIG_HOME", config_dir.path())
            .args(["context", "install"])
            .arg("--all")
            .assert()
            .success()
            .stdout(predicate::str::contains("Installing ALL rules from"))
            .stdout(predicate::str::contains("Successfully installed 4 rule(s)"));

        // Verify all rules were copied with directory structure
        let agents_rules = temp_dir.child(".agents/rules");
        agents_rules.child("security.md").assert(predicate::path::exists());
        agents_rules.child("testing.md").assert(predicate::path::exists());
        agents_rules
            .child("advanced/security-advanced.md")
            .assert(predicate::path::exists());
        agents_rules
            .child("advanced/testing-advanced.md")
            .assert(predicate::path::exists());

        // Verify reference directive was added to CLAUDE.md
        let claude_md = temp_dir.child("CLAUDE.md");
        claude_md.assert(predicate::path::exists());
        claude_md.assert(predicate::str::contains("<!-- CLAUDIUS_RULES_START -->"));
        claude_md.assert(predicate::str::contains("# External Rule References"));
        claude_md.assert(predicate::str::contains(".agents/rules/security.md"));
        claude_md.assert(predicate::str::contains(".agents/rules/testing.md"));
        claude_md.assert(predicate::str::contains(".agents/rules/advanced/security-advanced.md"));
        claude_md.assert(predicate::str::contains(".agents/rules/advanced/testing-advanced.md"));
        claude_md.assert(predicate::str::contains("<!-- CLAUDIUS_RULES_END -->"));
    }

    #[test]
    fn test_install_context_all_with_no_rules() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let config_dir = assert_fs::TempDir::new().unwrap();

        // Create empty rules directory
        let claudius_dir = config_dir.child("claudius");
        let rules_dir = claudius_dir.child("rules");
        rules_dir.create_dir_all().unwrap();

        // Run context install with --all option on empty directory
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .env("XDG_CONFIG_HOME", config_dir.path())
            .args(["context", "install"])
            .arg("--all")
            .assert()
            .failure()
            .stderr(predicate::str::contains("No rules found"));
    }

    #[test]
    fn test_install_context_all_with_custom_install_dir() {
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let config_dir = assert_fs::TempDir::new().unwrap();

        // Create test rules
        let claudius_dir = config_dir.child("claudius");
        let rules_dir = claudius_dir.child("rules");
        rules_dir.create_dir_all().unwrap();

        let rule1 = rules_dir.child("rule1.md");
        rule1.write_str("# Rule 1").unwrap();

        let rule2 = rules_dir.child("rule2.md");
        rule2.write_str("# Rule 2").unwrap();

        // Run context install with --all and custom install directory
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_claudius"));
        cmd.current_dir(temp_dir.path())
            .env("XDG_CONFIG_HOME", config_dir.path())
            .args(["context", "install"])
            .arg("--all")
            .arg("--install-dir")
            .arg(".custom/rules")
            .assert()
            .success();

        // Verify rules were installed in custom directory
        let custom_rules = temp_dir.child(".custom/rules");
        custom_rules.child("rule1.md").assert(predicate::path::exists());
        custom_rules.child("rule2.md").assert(predicate::path::exists());

        // Verify reference directive uses custom path
        let claude_md = temp_dir.child("CLAUDE.md");
        claude_md.assert(predicate::path::exists());
        claude_md.assert(predicate::str::contains("<!-- CLAUDIUS_RULES_START -->"));
        claude_md.assert(predicate::str::contains("# External Rule References"));
        claude_md.assert(predicate::str::contains(".custom/rules/rule1.md"));
        claude_md.assert(predicate::str::contains(".custom/rules/rule2.md"));
        claude_md.assert(predicate::str::contains("<!-- CLAUDIUS_RULES_END -->"));
    }
}
