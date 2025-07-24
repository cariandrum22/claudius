use crate::fixtures::TestFixture;
use claudius::template::{append_rules_to_claude_md, ensure_rules_directory};
use serial_test::serial;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial]
    fn test_append_single_rule() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create a rule file
        let rule_content = "# Test Rule\n\nThis is a test rule.";
        fixture.with_rule("test-rule", rule_content).unwrap();

        // Append the rule
        let result = append_rules_to_claude_md(&["test-rule".to_string()], Some(&fixture.project));

        assert!(result.is_ok());

        // Check that CLAUDE.md was created with the rule content
        let claude_md_content = fixture.read_project_file("CLAUDE.md").unwrap();
        assert!(claude_md_content.contains("# Test Rule"));
        assert!(claude_md_content.contains("This is a test rule."));
    }

    #[test]
    #[serial]
    fn test_append_multiple_rules() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Create multiple rule files
        fixture.with_rule("rule1", "# Rule 1\n\nContent 1").unwrap();
        fixture.with_rule("rule2", "# Rule 2\n\nContent 2").unwrap();

        // Append multiple rules
        let result = append_rules_to_claude_md(
            &["rule1".to_string(), "rule2".to_string()],
            Some(&fixture.project),
        );

        assert!(result.is_ok());

        // Check that CLAUDE.md contains both rules
        let claude_md_content = fixture.read_project_file("CLAUDE.md").unwrap();
        assert!(claude_md_content.contains("# Rule 1"));
        assert!(claude_md_content.contains("Content 1"));
        assert!(claude_md_content.contains("# Rule 2"));
        assert!(claude_md_content.contains("Content 2"));
    }

    #[test]
    #[serial]
    fn test_nonexistent_rule() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Try to append a non-existent rule (rules dir is created but empty)
        let result =
            append_rules_to_claude_md(&["nonexistent".to_string()], Some(&fixture.project));

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No valid rules found"));
    }

    #[test]
    #[serial]
    fn test_ensure_rules_directory() {
        let fixture = TestFixture::new().unwrap();
        fixture.setup_env();

        // Ensure rules directory is created
        let result = ensure_rules_directory();

        assert!(result.is_ok());

        let rules_dir = result.unwrap();
        assert!(rules_dir.exists());
        assert!(rules_dir.is_dir());
        assert!(rules_dir.ends_with("rules"));
    }
}
