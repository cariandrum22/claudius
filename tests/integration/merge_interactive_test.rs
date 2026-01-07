use anyhow::Result;
use claudius::config::{ClaudeConfig, McpServersConfig, Settings};
use claudius::merge::{merge_configs, merge_settings_with_strategy, MergeStrategy};
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

mod fixtures {
    include!("../fixtures/merge_test_fixtures.rs");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial]
    fn test_merge_configs_interactive_no_conflicts() -> Result<()> {
        // When there are no conflicts, interactive merge should behave like regular merge
        let mut claude_config: ClaudeConfig = serde_json::from_value(serde_json::json!({
            "mcpServers": {
                "existing": {
                    "command": "existing-cmd",
                    "args": [],
                    "env": {}
                }
            }
        }))?;

        let new_servers: McpServersConfig = serde_json::from_value(serde_json::json!({
            "mcpServers": {
                "github": {
                    "command": "gh-cmd",
                    "args": ["-y"],
                    "env": {}
                }
            }
        }))?;

        merge_configs(&mut claude_config, &new_servers, MergeStrategy::InteractiveMerge)?;

        // Should have both servers
        anyhow::ensure!(claude_config.mcp_servers.is_some());
        let servers = claude_config.mcp_servers.as_ref().unwrap();
        anyhow::ensure!(servers.len() == 2, "Expected 2 servers");
        anyhow::ensure!(servers.contains_key("existing"));
        anyhow::ensure!(servers.contains_key("github"));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_merge_settings_interactive_no_conflicts() -> Result<()> {
        let mut claude_config: ClaudeConfig = serde_json::from_value(serde_json::json!({
            "apiKeyHelper": "/old/helper",
            "cleanupPeriodDays": 10,
            "unknownField": "preserved"
        }))?;

        let settings: Settings = serde_json::from_value(serde_json::json!({
            "env": {"NEW": "value"},
            "includeCoAuthoredBy": true
        }))?;

        merge_settings_with_strategy(
            &mut claude_config,
            &settings,
            MergeStrategy::InteractiveMerge,
        )?;

        // Check that settings were merged
        anyhow::ensure!(
            claude_config.other.get("apiKeyHelper").unwrap() == "/old/helper",
            "apiKeyHelper should be unchanged"
        );
        anyhow::ensure!(
            claude_config.other.get("cleanupPeriodDays").unwrap() == 10,
            "cleanupPeriodDays should be unchanged"
        );
        anyhow::ensure!(
            claude_config.other.get("env").unwrap().get("NEW").unwrap() == "value",
            "env.NEW should be added"
        );
        anyhow::ensure!(
            claude_config.other.get("includeCoAuthoredBy").unwrap() == true,
            "includeCoAuthoredBy should be added"
        );
        anyhow::ensure!(
            claude_config.other.get("unknownField").unwrap() == "preserved",
            "unknownField should be preserved"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_preserve_unknown_fields_in_json() -> Result<()> {
        let mut claude_config: ClaudeConfig =
            serde_json::from_value(fixtures::create_test_claude_config())?;

        // Verify unknown fields are loaded
        anyhow::ensure!(claude_config.other.contains_key("unknownField"));
        anyhow::ensure!(claude_config.other.contains_key("customSettings"));

        let new_servers: McpServersConfig = serde_json::from_value(serde_json::json!({
            "mcpServers": {
                "new-server": {
                    "command": "new",
                    "args": [],
                    "env": {}
                }
            }
        }))?;

        // Merge with a strategy that doesn't require interaction
        merge_configs(&mut claude_config, &new_servers, MergeStrategy::Merge)?;

        // Verify unknown fields are still present
        anyhow::ensure!(
            claude_config.other.get("unknownField").unwrap() == "should_be_preserved",
            "unknownField should be preserved"
        );
        anyhow::ensure!(
            claude_config.other.get("customSettings").unwrap().get("nested").unwrap() == "value",
            "customSettings.nested should be preserved"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_codex_toml_preserves_unknown_fields() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("codex.settings.toml");

        // Write test config with unknown fields
        fs::write(&config_path, fixtures::create_test_codex_config())?;

        // Read and parse
        let settings = claudius::config::reader::read_codex_settings(&config_path)?;
        anyhow::ensure!(settings.is_some());

        let settings_data = settings.unwrap();

        // Verify known fields
        anyhow::ensure!(
            settings_data.model == Some("openai/gpt-4".to_string()),
            "Model should be openai/gpt-4"
        );

        // Verify unknown fields are preserved in extra
        anyhow::ensure!(settings_data.extra.contains_key("custom_field"));
        anyhow::ensure!(settings_data.extra.contains_key("extra_section"));

        // Write back
        claudius::config::writer::write_codex_settings(&config_path, &settings_data)?;

        // Read the written file and verify structure is preserved
        let content = fs::read_to_string(&config_path)?;
        anyhow::ensure!(content.contains("custom_field"));
        anyhow::ensure!(content.contains("extra_section"));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_merge_strategy_comparison() -> Result<()> {
        // Test that different merge strategies produce expected results
        let original_config = fixtures::create_test_claude_config();
        let new_servers: McpServersConfig =
            serde_json::from_value(fixtures::create_test_mcp_servers())?;

        // Test Replace strategy
        {
            let mut claude_config: ClaudeConfig = serde_json::from_value(original_config.clone())?;
            merge_configs(&mut claude_config, &new_servers, MergeStrategy::Replace)?;

            let servers = claude_config.mcp_servers.as_ref().unwrap();
            anyhow::ensure!(servers.len() == 2, "Expected 2 servers"); // Only new servers
            anyhow::ensure!(!servers.contains_key("existing")); // Old server removed
            anyhow::ensure!(servers.contains_key("filesystem"));
            anyhow::ensure!(servers.contains_key("github"));
        }

        // Test Merge strategy
        {
            let mut claude_config: ClaudeConfig = serde_json::from_value(original_config.clone())?;
            merge_configs(&mut claude_config, &new_servers, MergeStrategy::Merge)?;

            let servers = claude_config.mcp_servers.as_ref().unwrap();
            anyhow::ensure!(servers.len() == 3, "Expected 3 servers (all)");
            anyhow::ensure!(servers.contains_key("existing"));
            anyhow::ensure!(servers.contains_key("filesystem"));
            anyhow::ensure!(servers.contains_key("github"));
            anyhow::ensure!(
                servers.get("filesystem").and_then(|s| s.command.as_deref()) == Some("new-fs-command"),
                "filesystem command should be updated"
            );
        }

        // Test MergePreserveExisting strategy
        {
            let mut claude_config: ClaudeConfig = serde_json::from_value(original_config)?;
            merge_configs(&mut claude_config, &new_servers, MergeStrategy::MergePreserveExisting)?;

            let servers = claude_config.mcp_servers.as_ref().unwrap();
            anyhow::ensure!(servers.len() == 3, "Expected 3 servers (all)");
            anyhow::ensure!(servers.contains_key("existing"));
            anyhow::ensure!(servers.contains_key("filesystem"));
            anyhow::ensure!(servers.contains_key("github"));
            anyhow::ensure!(
                servers.get("filesystem").and_then(|s| s.command.as_deref()) == Some("old-fs-command"),
                "filesystem command should be preserved"
            );
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn test_settings_merge_preserves_extra_fields() -> Result<()> {
        let mut claude_config: ClaudeConfig = serde_json::from_value(serde_json::json!({
            "mcpServers": {},
            "apiKeyHelper": "/old/helper",
            "unknownField1": "value1",
            "unknownField2": {
                "nested": "data"
            }
        }))?;

        let settings: Settings = serde_json::from_value(serde_json::json!({
            "apiKeyHelper": "/new/helper",
            "cleanupPeriodDays": 30
        }))?;

        // Merge with regular strategy (overwrites)
        merge_settings_with_strategy(&mut claude_config, &settings, MergeStrategy::Merge)?;

        // Check known fields were updated
        anyhow::ensure!(
            claude_config.other.get("apiKeyHelper").unwrap() == "/new/helper",
            "apiKeyHelper should be updated"
        );
        anyhow::ensure!(
            claude_config.other.get("cleanupPeriodDays").unwrap() == 30,
            "cleanupPeriodDays should be updated"
        );

        // Check unknown fields were preserved
        anyhow::ensure!(
            claude_config.other.get("unknownField1").unwrap() == "value1",
            "unknownField1 should be preserved"
        );
        anyhow::ensure!(
            claude_config.other.get("unknownField2").unwrap().get("nested").unwrap() == "data",
            "unknownField2.nested should be preserved"
        );

        Ok(())
    }
}
