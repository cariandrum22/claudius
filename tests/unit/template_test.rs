use assert_fs::prelude::*;
use std::fs;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_template_to_new_claude_md() {
        let temp_dir = assert_fs::TempDir::new().unwrap();

        // Append template (should create new file)
        claudius::template::append_template_to_claude_md(None, Some(temp_dir.path())).unwrap();

        // Verify CLAUDE.md was created
        let claude_md_path = temp_dir.path().join("CLAUDE.md");
        assert!(claude_md_path.exists());

        let content = fs::read_to_string(&claude_md_path).unwrap();
        assert!(content.contains("## MCP Servers Configuration"));
        assert!(content.contains("claudius"));
    }

    #[test]
    fn test_append_template_to_existing_claude_md() {
        let temp_dir = assert_fs::TempDir::new().unwrap();

        // Create existing CLAUDE.md
        let claude_md = temp_dir.child("CLAUDE.md");
        claude_md.write_str("# Existing Content\n\nSome documentation here.\n").unwrap();

        // Append template
        claudius::template::append_template_to_claude_md(None, Some(temp_dir.path())).unwrap();

        // Verify content was appended
        let content = fs::read_to_string(claude_md.path()).unwrap();
        assert!(content.starts_with("# Existing Content"));
        assert!(content.contains("## MCP Servers Configuration"));
        assert!(content.contains("Some documentation here"));
    }

    #[test]
    fn test_append_custom_template() {
        let temp_dir = assert_fs::TempDir::new().unwrap();

        // Create custom template
        let template_file = temp_dir.child("custom_template.md");
        template_file
            .write_str("## Custom Template\n\nThis is a custom template content.\n")
            .unwrap();

        // Append custom template
        claudius::template::append_template_to_claude_md(
            Some(template_file.path()),
            Some(temp_dir.path()),
        )
        .unwrap();

        // Verify custom content was used
        let claude_md_path = temp_dir.path().join("CLAUDE.md");
        let content = fs::read_to_string(&claude_md_path).unwrap();
        assert!(content.contains("## Custom Template"));
        assert!(content.contains("This is a custom template content"));
    }

    #[test]
    fn test_no_duplicate_append() {
        let temp_dir = assert_fs::TempDir::new().unwrap();

        // Create CLAUDE.md with existing MCP section
        let claude_md = temp_dir.child("CLAUDE.md");
        claude_md
            .write_str("# Project\n\n## MCP Servers Configuration\n\nAlready exists.\n")
            .unwrap();

        // Try to append template
        claudius::template::append_template_to_claude_md(None, Some(temp_dir.path())).unwrap();

        // Verify content was not duplicated
        let content = fs::read_to_string(claude_md.path()).unwrap();
        let mcp_count = content.matches("## MCP Servers Configuration").count();
        assert_eq!(mcp_count, 1, "MCP section should not be duplicated");
    }
}
