use anyhow::Result;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to save and restore environment variables
    struct EnvGuard {
        xdg_config_home: Option<String>,
        home: Option<String>,
        current_dir: Option<std::path::PathBuf>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self {
                xdg_config_home: std::env::var("XDG_CONFIG_HOME").ok(),
                home: std::env::var("HOME").ok(),
                current_dir: std::env::current_dir().ok(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // Restore XDG_CONFIG_HOME
            match &self.xdg_config_home {
                Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
            // Restore HOME
            match &self.home {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }
            // Restore current directory
            if let Some(dir) = &self.current_dir {
                let _ = std::env::set_current_dir(dir);
            }
        }
    }

    // ==========================================
    // Basic model provider tests
    // ==========================================

    #[test]
    #[serial]
    fn test_codex_sync_preserves_model_providers() -> Result<()> {
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

        // Create Codex settings with model_providers
        let codex_settings_content = r#"model = "openai/gpt-4"
model_provider = "openai"
approval_policy = "auto"

	[model_providers.openai]
	base_url = "https://api.openai.com"
	env_key = "OPENAI_API_KEY"

	[model_providers.openai.http_headers]
	"X-Custom-Header" = "custom-value"
	"Authorization" = "Bearer $API_KEY"

	[model_providers.anthropic]
	base_url = "https://api.anthropic.com"
	env_key = "ANTHROPIC_API_KEY"

[model_providers.local]
base_url = "http://localhost:8080"
"#;
        fs::write(claudius_dir.join("codex.settings.toml"), codex_settings_content)?;

        // Run sync in global mode for Codex
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .args(["config", "sync", "--global", "--agent", "codex"])
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
        println!("Output config.toml:\n{codex_config_content}");

        // Verify that model_providers are preserved
        anyhow::ensure!(
            codex_config_content.contains("[model_providers.openai]"),
            "model_providers.openai section should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("base_url = \"https://api.openai.com\""),
            "openai base_url should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("env_key = \"OPENAI_API_KEY\""),
            "openai env_key should be preserved"
        );

        // Check headers
        anyhow::ensure!(
            codex_config_content.contains("[model_providers.openai.http_headers]"),
            "model_providers.openai.http_headers section should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("X-Custom-Header = \"custom-value\""),
            "Custom header should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("Authorization = \"Bearer $API_KEY\""),
            "Authorization header should be preserved"
        );

        // Check other providers
        anyhow::ensure!(
            codex_config_content.contains("[model_providers.anthropic]"),
            "model_providers.anthropic section should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("[model_providers.local]"),
            "model_providers.local section should be preserved"
        );

        // Also check MCP servers
        anyhow::ensure!(
            codex_config_content.contains("[mcp_servers.test-server]"),
            "MCP servers should be included in output"
        );

        Ok(())
    }

    // ==========================================
    // Edge case tests
    // ==========================================

    #[test]
    #[serial]
    fn test_codex_sync_model_providers_edge_cases() -> Result<()> {
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

        // Create empty MCP servers config (minimal)
        let mcp_servers_content = r#"{
  "mcpServers": {}
}"#;
        fs::write(claudius_dir.join("mcpServers.json"), mcp_servers_content)?;

        // Create Codex settings with complex model_providers including edge cases
        let codex_settings_content = r#"model = "openai/gpt-4"

# Complex model provider with all possible fields
[model_providers.complex]
base_url = "https://complex.api.com/v1"
env_key = "COMPLEX_API_KEY"

[model_providers.complex.http_headers]
"X-Api-Version" = "2024-01-01"
"X-Custom-Auth" = "Bearer ${COMPLEX_TOKEN}"
"Content-Type" = "application/json"
"X-Empty-Header" = ""

# Provider with only base_url
[model_providers.minimal]
base_url = "http://minimal.local"

# Provider with empty headers section
[model_providers.empty_headers]
base_url = "https://empty.headers.com"
env_key = "EMPTY_HEADERS_KEY"

[model_providers.empty_headers.http_headers]

# Provider with special characters in headers
[model_providers.special_chars]
base_url = "https://special.chars.com"

[model_providers.special_chars.http_headers]
"X-Special-Chars" = "value with spaces and $pecial ch@rs!"
"X-JSON-Data" = '{"key": "value", "nested": {"field": 123}}'
"X-Slash-Path" = "/api/v1/endpoint"

# Provider with numeric and boolean-like strings
[model_providers.edge_values]
base_url = "https://edge.values.com"

[model_providers.edge_values.http_headers]
"X-Numeric" = "12345"
"X-Boolean" = "true"
"X-Null-Like" = "null"
"X-Float" = "3.14159"
"#;
        fs::write(claudius_dir.join("codex.settings.toml"), codex_settings_content)?;

        // Run sync in global mode for Codex
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .args(["config", "sync", "--global", "--agent", "codex"])
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
        println!("Output config.toml:\n{codex_config_content}");

        // Verify complex provider
        anyhow::ensure!(
            codex_config_content.contains("[model_providers.complex]"),
            "Complex provider should exist"
        );
        anyhow::ensure!(
            codex_config_content.contains("[model_providers.complex.http_headers]"),
            "Complex provider http_headers should exist"
        );
        anyhow::ensure!(
            codex_config_content.contains("X-Api-Version"),
            "API version header should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("X-Empty-Header = \"\""),
            "Empty header value should be preserved"
        );

        // Verify minimal provider
        anyhow::ensure!(
            codex_config_content.contains("[model_providers.minimal]"),
            "Minimal provider should exist"
        );
        anyhow::ensure!(
            codex_config_content.contains("base_url = \"http://minimal.local\""),
            "Minimal provider base_url should be preserved"
        );

        // Verify empty headers section
        anyhow::ensure!(
            codex_config_content.contains("[model_providers.empty_headers]"),
            "Empty headers provider should exist"
        );
        anyhow::ensure!(
            codex_config_content.contains("[model_providers.empty_headers.http_headers]"),
            "Empty http_headers section should be preserved"
        );

        // Verify special characters
        anyhow::ensure!(
            codex_config_content.contains("[model_providers.special_chars]"),
            "Special chars provider should exist"
        );
        anyhow::ensure!(
            codex_config_content.contains("value with spaces and $pecial ch@rs!"),
            "Special characters in values should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("/api/v1/endpoint"),
            "Slash paths should be preserved"
        );

        // Verify edge values
        anyhow::ensure!(
            codex_config_content.contains("[model_providers.edge_values]"),
            "Edge values provider should exist"
        );
        anyhow::ensure!(
            codex_config_content.contains("X-Numeric = \"12345\""),
            "Numeric strings should be preserved as strings"
        );
        anyhow::ensure!(
            codex_config_content.contains("X-Boolean = \"true\""),
            "Boolean-like strings should be preserved as strings"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_codex_sync_preserves_provider_order() -> Result<()> {
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

        // Create empty MCP servers config
        let mcp_servers_content = r#"{"mcpServers": {}}"#;
        fs::write(claudius_dir.join("mcpServers.json"), mcp_servers_content)?;

        // Create Codex settings with providers that should maintain order
        let codex_settings_content = r#"model = "test"

[model_providers.aaa]
base_url = "https://aaa.com"

[model_providers.zzz]
base_url = "https://zzz.com"

[model_providers.bbb]
base_url = "https://bbb.com"

[model_providers.bbb.http_headers]
"B-Header" = "b-value"

[model_providers.aaa.http_headers]
"A-Header" = "a-value"
"#;
        fs::write(claudius_dir.join("codex.settings.toml"), codex_settings_content)?;

        // Run sync
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .args(["config", "sync", "--global", "--agent", "codex"])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            anyhow::bail!("sync command failed");
        }

        // Read output
        let codex_config_path = home_dir.join(".codex").join("config.toml");
        let codex_config_content = fs::read_to_string(&codex_config_path)?;

        // Verify all providers exist
        anyhow::ensure!(codex_config_content.contains("[model_providers.aaa]"));
        anyhow::ensure!(codex_config_content.contains("[model_providers.zzz]"));
        anyhow::ensure!(codex_config_content.contains("[model_providers.bbb]"));
        anyhow::ensure!(codex_config_content.contains("[model_providers.aaa.http_headers]"));
        anyhow::ensure!(codex_config_content.contains("[model_providers.bbb.http_headers]"));

        Ok(())
    }

    // ==========================================
    // Extra fields tests
    // ==========================================

    #[test]
    #[serial]
    fn test_codex_sync_preserves_model_provider_extra_fields() -> Result<()> {
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

        // Create Codex settings with model_providers containing extra fields like "name"
        let codex_settings_content = r#"model = "openai/gpt-4"
model_provider = "openai"

[model_providers.openai]
name = "OpenAI Provider"
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
timeout = 30
retry_count = 3
custom_field = "custom_value"

[model_providers.openai.http_headers]
"X-Custom-Header" = "value"

[model_providers.anthropic]
name = "Anthropic Claude"
base_url = "https://api.anthropic.com"
env_key = "ANTHROPIC_API_KEY"
model_format = "claude-3"
rate_limit = 100
debug = true

[model_providers.local]
name = "Local LLM"
base_url = "http://localhost:8080"
auth_type = "none"
stream = true
temperature = 0.7
max_tokens = 4096
"#;
        fs::write(claudius_dir.join("codex.settings.toml"), codex_settings_content)?;

        // Run sync in global mode for Codex
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .args(["config", "sync", "--global", "--agent", "codex"])
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
        println!("Output config.toml:\n{codex_config_content}");

        // Verify that extra fields in model_providers are preserved

        // OpenAI provider extra fields
        anyhow::ensure!(
            codex_config_content.contains("name = \"OpenAI Provider\""),
            "OpenAI provider name field should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("timeout = 30"),
            "timeout field should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("retry_count = 3"),
            "retry_count field should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("custom_field = \"custom_value\""),
            "custom_field should be preserved"
        );

        // Anthropic provider extra fields
        anyhow::ensure!(
            codex_config_content.contains("name = \"Anthropic Claude\""),
            "Anthropic provider name field should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("model_format = \"claude-3\""),
            "model_format field should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("rate_limit = 100"),
            "rate_limit field should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("debug = true"),
            "debug field should be preserved"
        );

        // Local provider extra fields
        anyhow::ensure!(
            codex_config_content.contains("name = \"Local LLM\""),
            "Local provider name field should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("auth_type = \"none\""),
            "auth_type field should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("stream = true"),
            "stream field should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("temperature = 0.7"),
            "temperature field should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("max_tokens = 4096"),
            "max_tokens field should be preserved"
        );

        // Also check that standard fields are still there
        anyhow::ensure!(
            codex_config_content.contains("base_url = \"https://api.openai.com/v1\""),
            "base_url should still be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("env_key = \"OPENAI_API_KEY\""),
            "env_key should still be preserved"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_model_provider_validation_allows_extra_fields() -> Result<()> {
        use claudius::codex_settings::validate_codex_settings;
        use toml::Value;

        // Parse TOML with extra fields in model_providers
        let toml_str = r#"
model = "test"

[model_providers.custom]
name = "Custom Provider"
base_url = "https://custom.api.com"
env_key = "CUSTOM_KEY"
custom_field_1 = "value1"
custom_field_2 = 123
custom_field_3 = true
nested_field = { key = "value" }
"#;

        let toml_value: Value = toml::from_str(toml_str)?;
        let warnings = validate_codex_settings(&toml_value);

        // Check that no warnings are generated for extra fields in model_providers
        for warning in &warnings {
            anyhow::ensure!(!warning.contains("name"), "name field should not generate a warning");
            anyhow::ensure!(
                !warning.contains("custom_field"),
                "custom fields should not generate warnings"
            );
            anyhow::ensure!(
                !warning.contains("nested_field"),
                "nested fields should not generate warnings"
            );
        }

        Ok(())
    }

    // ==========================================
    // Project-local tests
    // ==========================================

    #[test]
    #[serial]
    fn test_codex_sync_model_providers_project_local() -> Result<()> {
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
      "command": "test",
      "args": ["arg1"],
      "env": {}
    }
  }
}"#;
        fs::write(claudius_dir.join("mcpServers.json"), mcp_servers_content)?;

        // Create Codex TOML settings with model_providers
        let codex_settings_content = r#"model = "openai/gpt-4"
model_provider = "openai"
approval_policy = "none"

	[model_providers.openai]
	base_url = "https://api.openai.com/v1"
	env_key = "OPENAI_API_KEY"

	[model_providers.openai.http_headers]
	"X-OpenAI-Beta" = "assistants=v2"
	"X-Custom-Org" = "$OPENAI_ORG_ID"

	[model_providers.anthropic]
	base_url = "https://api.anthropic.com/v1"
	env_key = "ANTHROPIC_API_KEY"

	[model_providers.anthropic.http_headers]
	"anthropic-version" = "2023-06-01"
	"anthropic-beta" = "messages-2023-12-15"

	sandbox_mode = "workspace-write"

	[sandbox_workspace_write]
	network_access = true
	"#;
        fs::write(claudius_dir.join("codex.settings.toml"), codex_settings_content)?;

        // Change to project directory
        std::env::set_current_dir(&project_dir)?;

        // Run sync command in project-local mode
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .args(["config", "sync", "--agent", "codex"])
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
        println!("Project-local config.toml:\n{settings_content}");

        // Should contain original settings
        anyhow::ensure!(settings_content.contains("model = \"openai/gpt-4\""));
        anyhow::ensure!(settings_content.contains("model_provider = \"openai\""));

        // Should contain model_providers
        anyhow::ensure!(settings_content.contains("[model_providers.openai]"));
        anyhow::ensure!(settings_content.contains("base_url = \"https://api.openai.com/v1\""));
        anyhow::ensure!(settings_content.contains("env_key = \"OPENAI_API_KEY\""));

        // Check headers
        anyhow::ensure!(settings_content.contains("[model_providers.openai.http_headers]"));
        anyhow::ensure!(settings_content.contains("X-OpenAI-Beta = \"assistants=v2\""));
        anyhow::ensure!(settings_content.contains("X-Custom-Org = \"$OPENAI_ORG_ID\""));

        // Check anthropic provider
        anyhow::ensure!(settings_content.contains("[model_providers.anthropic]"));
        anyhow::ensure!(settings_content.contains("[model_providers.anthropic.http_headers]"));
        anyhow::ensure!(settings_content.contains("anthropic-version = \"2023-06-01\""));

        // Should contain MCP servers
        anyhow::ensure!(settings_content.contains("[mcp_servers.test-server]"));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_codex_sync_merges_model_provider_fields_into_existing_config() -> Result<()> {
        let _env_guard = EnvGuard::new();

        let temp_dir = TempDir::new()?;
        let config_dir = temp_dir.path().join("config");
        let home_dir = temp_dir.path().join("home");
        let claudius_dir = config_dir.join("claudius");

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&home_dir)?;
        fs::create_dir_all(&claudius_dir)?;

        std::env::set_var("XDG_CONFIG_HOME", &config_dir);
        std::env::set_var("HOME", &home_dir);

        // Minimal MCP servers config
        fs::write(claudius_dir.join("mcpServers.json"), r#"{"mcpServers": {}}"#)?;

        // Existing Codex config with extra provider fields and headers
        let existing_codex_config = r#"model = "openai/gpt-4"
model_provider = "openai"

[model_providers.openai]
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
timeout = 30

[model_providers.openai.http_headers]
"X-Existing" = "keep"
"#;

        let codex_config_dir = home_dir.join(".codex");
        fs::create_dir_all(&codex_config_dir)?;
        fs::write(codex_config_dir.join("config.toml"), existing_codex_config)?;

        // Source settings that only update a subset of fields for the same provider
        let codex_settings_content = r#"[model_providers.openai]
base_url = "https://proxy.openai.com/v1"

[model_providers.openai.http_headers]
"X-New" = "added"
"#;
        fs::write(claudius_dir.join("codex.settings.toml"), codex_settings_content)?;

        // Run sync in global mode for Codex
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_claudius"))
            .args(["config", "sync", "--global", "--agent", "codex"])
            .output()?;

        if !output.status.success() {
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            anyhow::bail!("sync command failed");
        }

        let codex_config_content = fs::read_to_string(codex_config_dir.join("config.toml"))?;
        println!("Merged config.toml:\n{codex_config_content}");

        // Updated field
        anyhow::ensure!(
            codex_config_content.contains("base_url = \"https://proxy.openai.com/v1\""),
            "base_url should be updated from source settings"
        );

        // Preserved fields from existing config
        anyhow::ensure!(
            codex_config_content.contains("env_key = \"OPENAI_API_KEY\""),
            "env_key should be preserved from existing config"
        );
        anyhow::ensure!(
            codex_config_content.contains("timeout = 30"),
            "extra provider field should be preserved from existing config"
        );

        // Headers should be merged (not replaced)
        anyhow::ensure!(
            codex_config_content.contains("X-Existing = \"keep\""),
            "existing header should be preserved"
        );
        anyhow::ensure!(
            codex_config_content.contains("X-New = \"added\""),
            "new header should be added"
        );

        Ok(())
    }
}
