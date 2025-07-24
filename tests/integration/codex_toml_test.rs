use anyhow::Result;
use claudius::{
    codex_settings::{convert_mcp_to_toml, validate_codex_settings, CodexSettings},
    config::{reader, writer, McpServerConfig},
};
use serial_test::serial;
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;
use toml::Value as TomlValue;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial]
    fn test_read_codex_settings() -> Result<()> {
        let settings_content = r#"
model = "openai/gpt-4"
model_provider = "openai"
approval_policy = "none"

[model_providers.openai]
base_url = "https://api.openai.com"
api_key_env = "OPENAI_API_KEY"

[sandbox]
mode = "docker"
network_access = true
"#;

        let temp_dir = TempDir::new()?;
        let settings_path = temp_dir.path().join("codex.settings.toml");
        fs::write(&settings_path, settings_content)?;

        let settings = reader::read_codex_settings(&settings_path)?;
        anyhow::ensure!(settings.is_some(), "Settings should be Some");

        let settings_data = settings.unwrap();
        anyhow::ensure!(settings_data.model == Some("openai/gpt-4".to_string()), "Model mismatch");
        anyhow::ensure!(
            settings_data.model_provider == Some("openai".to_string()),
            "Model provider mismatch"
        );
        anyhow::ensure!(
            settings_data.approval_policy == Some("none".to_string()),
            "Approval policy mismatch"
        );
        anyhow::ensure!(settings_data.model_providers.is_some(), "Model providers should be Some");
        anyhow::ensure!(settings_data.sandbox.is_some(), "Sandbox should be Some");

        let sandbox = settings_data.sandbox.unwrap();
        anyhow::ensure!(sandbox.mode == Some("docker".to_string()), "Sandbox mode mismatch");
        anyhow::ensure!(sandbox.network_access == Some(true), "Network access mismatch");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_write_codex_settings() -> Result<()> {
        let mut model_providers = HashMap::new();
        model_providers.insert(
            "anthropic".to_string(),
            claudius::codex_settings::ModelProvider {
                base_url: Some("https://api.anthropic.com".to_string()),
                api_key_env: Some("ANTHROPIC_API_KEY".to_string()),
                headers: None,
                extra: HashMap::new(),
            },
        );

        let settings = CodexSettings {
            model: Some("anthropic/claude-3".to_string()),
            model_provider: Some("anthropic".to_string()),
            approval_policy: Some("required".to_string()),
            disable_response_storage: Some(true),
            notify: Some(vec!["desktop".to_string(), "sound".to_string()]),
            model_providers: Some(model_providers),
            shell_environment_policy: None,
            sandbox: None,
            history: None,
            mcp_servers: None,
            extra: HashMap::new(),
        };

        let temp_dir = TempDir::new()?;
        let settings_path = temp_dir.path().join("codex.settings.toml");

        writer::write_codex_settings(&settings_path, &settings)?;

        // Read back and verify
        let content = fs::read_to_string(&settings_path)?;
        anyhow::ensure!(
            content.contains("model = \"anthropic/claude-3\""),
            "Model not found in content"
        );
        anyhow::ensure!(
            content.contains("model_provider = \"anthropic\""),
            "Model provider not found in content"
        );
        anyhow::ensure!(
            content.contains("approval_policy = \"required\""),
            "Approval policy not found in content"
        );
        anyhow::ensure!(
            content.contains("disable_response_storage = true"),
            "Disable response storage not found in content"
        );
        anyhow::ensure!(
            content.contains("[model_providers.anthropic]"),
            "Model providers section not found in content"
        );
        anyhow::ensure!(
            content.contains("base_url = \"https://api.anthropic.com\""),
            "Base URL not found in content"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_convert_mcp_to_toml() -> Result<()> {
        let mut mcp_servers = HashMap::new();

        let mut env = HashMap::new();
        env.insert("ALLOWED_PATHS".to_string(), "/home,/tmp".to_string());

        mcp_servers.insert(
            "filesystem".to_string(),
            McpServerConfig {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string()],
                env: env.clone(),
            },
        );

        mcp_servers.insert(
            "github".to_string(),
            McpServerConfig {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@modelcontextprotocol/server-github".to_string()],
                env: HashMap::new(),
            },
        );

        let toml_servers = convert_mcp_to_toml(&mcp_servers);

        anyhow::ensure!(toml_servers.len() == 2, "Expected 2 servers");
        anyhow::ensure!(toml_servers.contains_key("filesystem"), "filesystem server not found");
        anyhow::ensure!(toml_servers.contains_key("github"), "github server not found");

        // Check filesystem server
        if let Some(TomlValue::Table(fs_table)) = toml_servers.get("filesystem") {
            anyhow::ensure!(
                fs_table.get("command") == Some(&TomlValue::String("npx".to_string())),
                "Command mismatch"
            );

            if let Some(TomlValue::Array(args)) = fs_table.get("args") {
                anyhow::ensure!(args.len() == 2, "Expected 2 args");
            } else {
                anyhow::bail!("Expected args to be an array");
            }

            if let Some(TomlValue::Table(env_table)) = fs_table.get("env") {
                anyhow::ensure!(
                    env_table.get("ALLOWED_PATHS")
                        == Some(&TomlValue::String("/home,/tmp".to_string())),
                    "ALLOWED_PATHS mismatch"
                );
            } else {
                anyhow::bail!("Expected env to be a table");
            }
        } else {
            anyhow::bail!("Expected filesystem to be a table");
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn test_validate_codex_settings_with_unknown_fields() -> Result<()> {
        let toml_str = r#"
model = "openai/gpt-4"
unknown_top_level = "value"

[model_providers.openai]
base_url = "https://api.openai.com"
unknown_provider_field = "value"

[sandbox]
mode = "docker"
unknown_sandbox_field = true
"#;

        let toml_value: TomlValue = toml::from_str(toml_str)?;
        let warnings = validate_codex_settings(&toml_value);

        anyhow::ensure!(warnings.len() == 2, "Expected 2 warnings");
        anyhow::ensure!(
            warnings.iter().any(|w| w.contains("unknown_top_level")),
            "Expected unknown_top_level warning"
        );
        // Model provider fields are no longer validated since they can have arbitrary extra fields
        // assert!(warnings.iter().any(|w| w.contains("unknown_provider_field")));
        anyhow::ensure!(
            warnings.iter().any(|w| w.contains("unknown_sandbox_field")),
            "Expected unknown_sandbox_field warning"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_codex_settings_preserves_unknown_fields() -> Result<()> {
        let toml_str = r#"
model = "openai/gpt-4"
unknown_field = "should be preserved"
another_unknown = 42

[extra_section]
key = "value"
"#;

        let temp_dir = TempDir::new()?;
        let settings_path = temp_dir.path().join("codex.settings.toml");
        fs::write(&settings_path, toml_str)?;

        let settings = reader::read_codex_settings(&settings_path)?;
        anyhow::ensure!(settings.is_some(), "Settings should be Some");

        let settings_value = settings.unwrap();
        anyhow::ensure!(settings_value.model == Some("openai/gpt-4".to_string()), "Model mismatch");

        // Check that extra fields were preserved
        anyhow::ensure!(!settings_value.extra.is_empty(), "Extra fields should not be empty");
        anyhow::ensure!(
            settings_value.extra.contains_key("unknown_field"),
            "unknown_field not preserved"
        );
        anyhow::ensure!(
            settings_value.extra.contains_key("another_unknown"),
            "another_unknown not preserved"
        );
        anyhow::ensure!(
            settings_value.extra.contains_key("extra_section"),
            "extra_section not preserved"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_round_trip_codex_settings_with_mcp() -> Result<()> {
        let mut mcp_servers = HashMap::new();
        mcp_servers.insert(
            "test-server".to_string(),
            McpServerConfig {
                command: "python".to_string(),
                args: vec!["-m".to_string(), "server".to_string()],
                env: HashMap::new(),
            },
        );

        let settings = CodexSettings {
            model: Some("openai/gpt-4".to_string()),
            model_provider: None,
            approval_policy: None,
            disable_response_storage: None,
            notify: None,
            model_providers: None,
            shell_environment_policy: None,
            sandbox: None,
            history: None,
            mcp_servers: Some(convert_mcp_to_toml(&mcp_servers)),
            extra: HashMap::new(),
        };

        let temp_dir = TempDir::new()?;
        let settings_path = temp_dir.path().join("codex.settings.toml");

        // Write settings
        writer::write_codex_settings(&settings_path, &settings)?;

        // Read back
        let read_settings = reader::read_codex_settings(&settings_path)?;
        anyhow::ensure!(read_settings.is_some(), "Read settings should be Some");

        let settings_data = read_settings.unwrap();
        anyhow::ensure!(
            settings_data.model == Some("openai/gpt-4".to_string()),
            "Model mismatch after round trip"
        );
        anyhow::ensure!(settings_data.mcp_servers.is_some(), "MCP servers should be Some");

        let mcp = settings_data.mcp_servers.unwrap();
        anyhow::ensure!(mcp.contains_key("test-server"), "test-server not found in MCP servers");

        Ok(())
    }
}
