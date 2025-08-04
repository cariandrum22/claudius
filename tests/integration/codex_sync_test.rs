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
}
