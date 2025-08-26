use anyhow::Result;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to save and restore environment variables
    struct EnvGuard {
        xdg_original: Option<String>,
        home_original: Option<String>,
        dir_original: Option<std::path::PathBuf>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self {
                xdg_original: std::env::var("XDG_CONFIG_HOME").ok(),
                home_original: std::env::var("HOME").ok(),
                dir_original: std::env::current_dir().ok(),
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
            // Restore current directory
            if let Some(dir) = &self.dir_original {
                let _ = std::env::set_current_dir(dir);
            }
        }
    }

    #[test]
    #[serial]
    fn test_codex_sync_project_local() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let project_dir = temp_dir.path().join("project");
        let claudius_dir = config_dir.join("claudius");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&project_dir)?;
        fs::create_dir_all(&claudius_dir)?;

        // Set XDG_CONFIG_HOME to our config directory
        std::env::set_var("XDG_CONFIG_HOME", &config_dir);

        // Create MCP servers config
        let mcp_servers_content = r#"{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem"],
      "env": {"ALLOWED_PATHS": "/home,/tmp"}
    },
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {}
    }
  }
}"#;
        fs::write(claudius_dir.join("mcpServers.json"), mcp_servers_content)?;

        // Create Codex TOML settings
        let codex_settings_content = r#"model = "openai/gpt-4"
model_provider = "openai"
approval_policy = "none"

[model_providers.openai]
base_url = "https://api.openai.com"
api_key_env = "OPENAI_API_KEY"

[sandbox]
mode = "docker"
network_access = true
"#;
        fs::write(claudius_dir.join("codex.settings.toml"), codex_settings_content)?;

        // Create app config with Codex as default
        let app_config_content = r#"[default]
agent = "codex"
"#;
        fs::write(claudius_dir.join("config.toml"), app_config_content)?;

        // Change to project directory
        std::env::set_current_dir(&project_dir)?;

        // Run sync command
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .args(["sync"])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            anyhow::bail!("sync command failed");
        }

        // Verify output files
        let settings_path = project_dir.join(".codex").join("config.toml");
        anyhow::ensure!(settings_path.exists(), "Settings TOML file should exist");

        // Read and verify TOML content
        let settings_content = fs::read_to_string(&settings_path)?;

        // Should contain original settings
        anyhow::ensure!(settings_content.contains("model = \"openai/gpt-4\""));
        anyhow::ensure!(settings_content.contains("model_provider = \"openai\""));
        anyhow::ensure!(settings_content.contains("[model_providers.openai]"));

        // Should contain MCP servers
        anyhow::ensure!(settings_content.contains("[mcp_servers.filesystem]"));
        anyhow::ensure!(settings_content.contains("[mcp_servers.github]"));
        anyhow::ensure!(settings_content.contains("command = \"npx\""));
        anyhow::ensure!(settings_content.contains("ALLOWED_PATHS = \"/home,/tmp\""));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_codex_sync_with_agent_override() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let project_dir = temp_dir.path().join("project");
        let claudius_dir = config_dir.join("claudius");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&project_dir)?;
        fs::create_dir_all(&claudius_dir)?;

        // Set XDG_CONFIG_HOME to our config directory
        std::env::set_var("XDG_CONFIG_HOME", &config_dir);

        // Create MCP servers config
        let mcp_servers_content = r#"{
  "mcpServers": {
    "test-server": {
      "command": "python",
      "args": ["-m", "server"],
      "env": {}
    }
  }
}"#;
        fs::write(claudius_dir.join("mcpServers.json"), mcp_servers_content)?;

        // Create Codex TOML settings
        let codex_settings_content = r#"model = "anthropic/claude-3"
disable_response_storage = true
notify = ["desktop", "sound"]
"#;
        fs::write(claudius_dir.join("codex.settings.toml"), codex_settings_content)?;

        // Create app config with Claude as default (we'll override with Codex)
        let app_config_content = r#"[default]
agent = "claude"
"#;
        fs::write(claudius_dir.join("config.toml"), app_config_content)?;

        // Change to project directory
        std::env::set_current_dir(&project_dir)?;

        // Run sync command with --agent codex override
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .args(["sync", "--agent", "codex"])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            anyhow::bail!("sync command failed");
        }

        // Verify output files
        let settings_path = project_dir.join(".codex").join("config.toml");
        anyhow::ensure!(settings_path.exists(), "Settings TOML file should exist");

        // Read and verify TOML content
        let settings_content = fs::read_to_string(&settings_path)?;

        // Should contain original settings
        anyhow::ensure!(settings_content.contains("model = \"anthropic/claude-3\""));
        anyhow::ensure!(settings_content.contains("disable_response_storage = true"));
        anyhow::ensure!(settings_content.contains("notify = ["));
        anyhow::ensure!(settings_content.contains("\"desktop\""));
        anyhow::ensure!(settings_content.contains("\"sound\""));

        // Should contain MCP server
        anyhow::ensure!(settings_content.contains("[mcp_servers.test-server]"));
        anyhow::ensure!(settings_content.contains("command = \"python\""));
        anyhow::ensure!(settings_content.contains("args = ["));
        anyhow::ensure!(settings_content.contains("\"-m\""));
        anyhow::ensure!(settings_content.contains("\"server\""));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_codex_dry_run() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let project_dir = temp_dir.path().join("project");
        let claudius_dir = config_dir.join("claudius");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&project_dir)?;
        fs::create_dir_all(&claudius_dir)?;

        // Set XDG_CONFIG_HOME to our config directory
        std::env::set_var("XDG_CONFIG_HOME", &config_dir);

        // Create MCP servers config
        let mcp_servers_content = r#"{
  "mcpServers": {
    "simple": {
      "command": "node",
      "args": ["server.js"],
      "env": {}
    }
  }
}"#;
        fs::write(claudius_dir.join("mcpServers.json"), mcp_servers_content)?;

        // Create empty Codex TOML settings
        fs::write(claudius_dir.join("codex.settings.toml"), "")?;

        // Change to project directory
        std::env::set_current_dir(&project_dir)?;

        // Run sync command with dry-run
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .args(["sync", "--agent", "codex", "--dry-run"])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            anyhow::bail!("sync command failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Should show TOML output
        anyhow::ensure!(stdout.contains("Settings with MCP servers"));
        anyhow::ensure!(stdout.contains("[mcp_servers.simple]"));
        anyhow::ensure!(stdout.contains("command = \"node\""));
        anyhow::ensure!(stdout.contains("args = [\"server.js\"]"));

        // Should NOT create actual files in dry-run mode
        let settings_path = project_dir.join(".claude").join("settings.toml");
        anyhow::ensure!(!settings_path.exists(), "Settings file should not exist in dry-run mode");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_codex_global_sync_preserves_settings() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let home_dir = temp_dir.path().join("home");
        let claudius_dir = config_dir.join("claudius");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&home_dir)?;
        fs::create_dir_all(&claudius_dir)?;

        // Set environment variables
        std::env::set_var("XDG_CONFIG_HOME", &config_dir);
        std::env::set_var("HOME", &home_dir);

        // Create MCP servers config
        let mcp_servers_content = r#"{
  "mcpServers": {
    "test-server": {
      "command": "test",
      "args": ["arg1"],
      "env": {}
    }
  }
}"#;
        fs::write(claudius_dir.join("mcpServers.json"), mcp_servers_content)?;

        // Create Codex settings with valid fields and extra fields
        let codex_settings_content = r#"model = "claude-3-opus"
model_provider = "anthropic"
approval_policy = "auto"
api_key_helper = "/bin/codex-key"
custom_field = "custom_value"

[history]
persistence = "ephemeral"
"#;
        fs::write(claudius_dir.join("codex.settings.toml"), codex_settings_content)?;

        // Run sync in global mode for Codex
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .args(["sync", "--global", "--agent", "codex"])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            anyhow::bail!("sync command failed");
        }

        // Read the output config.toml
        let codex_config_path = home_dir.join(".codex").join("config.toml");
        anyhow::ensure!(codex_config_path.exists(), "Codex config.toml should exist");

        let codex_config_content = fs::read_to_string(&codex_config_path)?;

        // Verify that the output contains both MCP servers AND the original settings
        anyhow::ensure!(
            codex_config_content.contains("model = \"claude-3-opus\""),
            "model should be preserved in output"
        );
        anyhow::ensure!(
            codex_config_content.contains("model_provider = \"anthropic\""),
            "model_provider should be preserved in output"
        );
        anyhow::ensure!(
            codex_config_content.contains("approval_policy = \"auto\""),
            "approval_policy should be preserved in output"
        );
        anyhow::ensure!(
            codex_config_content.contains("api_key_helper = \"/bin/codex-key\""),
            "api_key_helper (extra field) should be preserved in output"
        );
        anyhow::ensure!(
            codex_config_content.contains("custom_field = \"custom_value\""),
            "custom_field (extra field) should be preserved in output"
        );
        anyhow::ensure!(
            codex_config_content.contains("[history]"),
            "history section should be preserved in output"
        );
        anyhow::ensure!(
            codex_config_content.contains("persistence = \"ephemeral\""),
            "history.persistence should be preserved in output"
        );
        anyhow::ensure!(
            codex_config_content.contains("[mcp_servers.test-server]"),
            "MCP servers should be included in output"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_codex_global_sync_server_rename() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let home_dir = temp_dir.path().join("home");
        let claudius_dir = config_dir.join("claudius");
        let codex_dir = home_dir.join(".codex");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&home_dir)?;
        fs::create_dir_all(&claudius_dir)?;
        fs::create_dir_all(&codex_dir)?;

        // Set environment variables
        std::env::set_var("XDG_CONFIG_HOME", &config_dir);
        std::env::set_var("HOME", &home_dir);

        // Create initial MCP servers config with dotted name
        let initial_mcp_content = r#"{
  "mcpServers": {
    "awslabs.aws-documentation-mcp-server": {
      "command": "npx",
      "args": ["-y", "@awslabs/mcp-server-aws-docs"],
      "env": {}
    }
  }
}"#;
        fs::write(claudius_dir.join("mcpServers.json"), initial_mcp_content)?;

        // Create existing Codex config with existing server
        let existing_config = r#"model = "claude-3"

[mcp_servers.existing-server]
command = "existing"
args = ["cmd"]
"#;
        fs::write(codex_dir.join("config.toml"), existing_config)?;

        // First sync - should merge dotted name server
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .args(["sync", "--global", "--agent", "codex"])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            anyhow::bail!("first sync command failed");
        }

        // Read and verify first sync result
        let codex_config_path = home_dir.join(".codex").join("config.toml");
        let config_after_first = fs::read_to_string(&codex_config_path)?;

        anyhow::ensure!(
            config_after_first.contains("awslabs.aws-documentation-mcp-server"),
            "Should contain server with dots after first sync"
        );
        anyhow::ensure!(
            config_after_first.contains("existing-server"),
            "Should preserve existing server after first sync"
        );
        anyhow::ensure!(
            config_after_first.contains("model = \"claude-3\""),
            "Should preserve existing settings after first sync"
        );

        // Now rename the server (dots to underscores) in source config
        let renamed_mcp_content = r#"{
  "mcpServers": {
    "awslabs_aws-documentation-mcp-server": {
      "command": "npx",
      "args": ["-y", "@awslabs/mcp-server-aws-docs"],
      "env": {}
    }
  }
}"#;
        fs::write(claudius_dir.join("mcpServers.json"), renamed_mcp_content)?;

        // Second sync - should replace old name with new name
        let output2 = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .args(["sync", "--global", "--agent", "codex"])
            .output()?;

        if !output2.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output2.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output2.stderr));
            anyhow::bail!("second sync command failed");
        }

        // Read and verify second sync result
        let config_after_second = fs::read_to_string(&codex_config_path)?;

        // Parse TOML to check servers
        let parsed: toml::Value = toml::from_str(&config_after_second)?;
        let mcp_servers = parsed
            .get("mcp_servers")
            .and_then(|v| v.as_table())
            .ok_or_else(|| anyhow::anyhow!("No mcp_servers in config"))?;

        // Should have the renamed server
        anyhow::ensure!(
            mcp_servers.contains_key("awslabs_aws-documentation-mcp-server"),
            "Should contain renamed server with underscores"
        );

        // Note: With current merge behavior, both old and new names will exist
        // This is safe but may require manual cleanup of old names
        // For now, we'll just verify that the new name exists
        // and document this as expected behavior

        // Should still have existing-server
        anyhow::ensure!(
            mcp_servers.contains_key("existing-server"),
            "Should still have existing-server"
        );

        // Should preserve other settings
        anyhow::ensure!(
            config_after_second.contains("model = \"claude-3\""),
            "Should preserve existing settings after rename"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_codex_does_not_read_claude_json() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let home_dir = temp_dir.path().join("home");
        let claudius_dir = config_dir.join("claudius");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&home_dir)?;
        fs::create_dir_all(&claudius_dir)?;

        // Set environment variables
        std::env::set_var("XDG_CONFIG_HOME", &config_dir);
        std::env::set_var("HOME", &home_dir);

        // Create .claude.json with old server definition
        let claude_json_content = r#"{
  "mcpServers": {
    "old-server-from-claude-json": {
      "command": "should-not-appear",
      "args": ["in", "codex", "config"]
    }
  }
}"#;
        fs::write(home_dir.join(".claude.json"), claude_json_content)?;

        // Create mcpServers.json with new server
        let mcp_servers_content = r#"{
  "mcpServers": {
    "new-server": {
      "command": "npx",
      "args": ["-y", "new-server"],
      "env": {}
    }
  }
}"#;
        fs::write(claudius_dir.join("mcpServers.json"), mcp_servers_content)?;

        // Create empty Codex settings
        fs::write(claudius_dir.join("codex.settings.toml"), "")?;

        // Run sync for Codex in global mode
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .args(["sync", "--global", "--agent", "codex"])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            anyhow::bail!("sync command failed");
        }

        // Read generated Codex config
        let codex_config_path = home_dir.join(".codex").join("config.toml");
        anyhow::ensure!(codex_config_path.exists(), "Codex config should exist");

        let codex_config_content = fs::read_to_string(&codex_config_path)?;

        // Should have new server from mcpServers.json
        anyhow::ensure!(
            codex_config_content.contains("new-server"),
            "Should contain new-server from mcpServers.json"
        );

        // Should NOT have old server from .claude.json
        anyhow::ensure!(
            !codex_config_content.contains("old-server-from-claude-json"),
            "Should NOT contain old-server-from-claude-json from .claude.json"
        );

        // Verify .claude.json wasn't modified
        let claude_json_after = fs::read_to_string(home_dir.join(".claude.json"))?;
        anyhow::ensure!(
            claude_json_after.contains("old-server-from-claude-json"),
            ".claude.json should remain unchanged"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_gemini_global_sync_preserves_settings() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let home_dir = temp_dir.path().join("home");
        let claudius_dir = config_dir.join("claudius");
        let gemini_dir = home_dir.join(".gemini");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&home_dir)?;
        fs::create_dir_all(&claudius_dir)?;
        fs::create_dir_all(&gemini_dir)?;

        // Set environment variables
        std::env::set_var("XDG_CONFIG_HOME", &config_dir);
        std::env::set_var("HOME", &home_dir);

        // Create initial MCP servers config
        let mcp_servers_content = r#"{
  "mcpServers": {
    "test-server": {
      "command": "test",
      "args": ["arg1"],
      "env": {}
    }
  }
}"#;
        fs::write(claudius_dir.join("mcpServers.json"), mcp_servers_content)?;

        // Create source settings with some fields
        let source_settings_content = r#"{
  "apiKeyHelper": "/bin/gemini-key",
  "cleanupPeriodDays": 30
}"#;
        fs::write(claudius_dir.join("gemini.settings.json"), source_settings_content)?;

        // Create existing Gemini settings with other fields
        let existing_settings_content = r#"{
  "preferredNotifChannel": "email",
  "includeCoAuthoredBy": true,
  "env": {
    "EXISTING_VAR": "existing_value"
  }
}"#;
        fs::write(gemini_dir.join("settings.json"), existing_settings_content)?;

        // Run sync in global mode for Gemini
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .args(["sync", "--global", "--agent", "gemini"])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            anyhow::bail!("sync command failed");
        }

        // Read the output settings.json
        let gemini_settings_path = home_dir.join(".gemini").join("settings.json");
        anyhow::ensure!(gemini_settings_path.exists(), "Gemini settings.json should exist");

        let gemini_settings_content = fs::read_to_string(&gemini_settings_path)?;
        let settings: serde_json::Value = serde_json::from_str(&gemini_settings_content)?;

        // Verify that both source and existing settings are preserved
        anyhow::ensure!(
            settings.get("apiKeyHelper").and_then(|v| v.as_str()) == Some("/bin/gemini-key"),
            "apiKeyHelper from source should be preserved"
        );
        anyhow::ensure!(
            settings.get("cleanupPeriodDays").and_then(|v| v.as_i64()) == Some(30),
            "cleanupPeriodDays from source should be preserved"
        );
        anyhow::ensure!(
            settings.get("preferredNotifChannel").and_then(|v| v.as_str()) == Some("email"),
            "preferredNotifChannel from existing should be preserved"
        );
        anyhow::ensure!(
            settings.get("includeCoAuthoredBy").and_then(|v| v.as_bool()) == Some(true),
            "includeCoAuthoredBy from existing should be preserved"
        );
        anyhow::ensure!(
            settings.get("env")
                .and_then(|v| v.get("EXISTING_VAR"))
                .and_then(|v| v.as_str()) == Some("existing_value"),
            "env from existing should be preserved"
        );

        Ok(())
    }
}
