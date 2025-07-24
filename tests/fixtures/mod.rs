#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use tempfile::{tempdir, TempDir};

/// Test fixture for managing temporary configuration directories
pub struct TestFixture {
    /// Temporary directory that will be cleaned up on drop
    pub temp: TempDir,
    /// Path to the config directory (`XDG_CONFIG_HOME/claudius`)
    pub config: PathBuf,
    /// Path to the test project directory
    pub project: PathBuf,
}

impl TestFixture {
    /// Create a new test fixture with temporary directories
    pub fn new() -> std::io::Result<Self> {
        let temp_dir = tempdir()?;
        let config_dir = temp_dir.path().join("config").join("claudius");
        let project_dir = temp_dir.path().join("project");

        // Create the directories
        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&project_dir)?;

        Ok(Self { temp: temp_dir, config: config_dir, project: project_dir })
    }

    /// Get the config home directory (parent of claudius config)
    pub fn config_home(&self) -> PathBuf {
        self.config.parent().unwrap().to_path_buf()
    }

    /// Create a test mcpServers.json file
    pub fn with_mcp_servers(&self, content: &str) -> std::io::Result<&Self> {
        let path = self.config.join("mcpServers.json");
        fs::write(path, content)?;
        Ok(self)
    }

    /// Create a test settings.json file
    pub fn with_settings(&self, content: &str) -> std::io::Result<&Self> {
        let path = self.config.join("settings.json");
        fs::write(path, content)?;
        Ok(self)
    }

    /// Create a test gemini.settings.json file
    pub fn with_gemini_settings(&self, content: &str) -> std::io::Result<&Self> {
        let path = self.config.join("gemini.settings.json");
        fs::write(path, content)?;
        Ok(self)
    }

    /// Create a test claude.settings.json file
    pub fn with_claude_settings(&self, content: &str) -> std::io::Result<&Self> {
        let path = self.config.join("claude.settings.json");
        fs::write(path, content)?;
        Ok(self)
    }

    /// Create a test codex.settings.toml file
    pub fn with_codex_settings(&self, content: &str) -> std::io::Result<&Self> {
        let path = self.config.join("codex.settings.toml");
        fs::write(path, content)?;
        Ok(self)
    }

    /// Create a test command file
    pub fn with_command(&self, name: &str, content: &str) -> std::io::Result<&Self> {
        let commands_dir = self.config.join("commands");
        fs::create_dir_all(&commands_dir)?;
        let path = commands_dir.join(format!("{name}.md"));
        fs::write(path, content)?;
        Ok(self)
    }

    /// Create a test rule file
    pub fn with_rule(&self, name: &str, content: &str) -> std::io::Result<&Self> {
        let rules_dir = self.config.join("rules");
        fs::create_dir_all(&rules_dir)?;
        let path = rules_dir.join(format!("{name}.md"));
        fs::write(path, content)?;
        Ok(self)
    }

    /// Create a CLAUDE.md file in the project directory
    pub fn with_claude_md(&self, content: &str) -> std::io::Result<&Self> {
        let path = self.project.join("CLAUDE.md");
        fs::write(path, content)?;
        Ok(self)
    }

    /// Create an existing .mcp.json file in the project directory
    pub fn with_existing_mcp_json(&self, content: &str) -> std::io::Result<&Self> {
        let path = self.project.join(".mcp.json");
        fs::write(path, content)?;
        Ok(self)
    }

    /// Create an existing .claude/settings.json file in the project directory
    pub fn with_existing_claude_settings(&self, content: &str) -> std::io::Result<&Self> {
        let claude_dir = self.project.join(".claude");
        fs::create_dir_all(&claude_dir)?;
        let path = claude_dir.join("settings.json");
        fs::write(path, content)?;
        Ok(self)
    }

    /// Create an existing ~/.claude.json file (for global mode tests)
    pub fn with_existing_global_config(&self, content: &str) -> std::io::Result<&Self> {
        let home_dir = self.temp.path().join("home");
        fs::create_dir_all(&home_dir)?;
        let path = home_dir.join(".claude.json");
        fs::write(path, content)?;
        Ok(self)
    }

    /// Get the home directory for global config tests
    pub fn home_dir(&self) -> PathBuf {
        self.temp.path().join("home")
    }

    /// Set environment variables for the test
    pub fn setup_env(&self) {
        std::env::set_var("XDG_CONFIG_HOME", self.config_home());
        std::env::set_var("HOME", self.home_dir());
    }

    /// Read a file from the project directory
    pub fn read_project_file(&self, path: &str) -> std::io::Result<String> {
        fs::read_to_string(self.project.join(path))
    }

    /// Read a file from the home directory
    pub fn read_home_file(&self, path: &str) -> std::io::Result<String> {
        fs::read_to_string(self.home_dir().join(path))
    }

    /// Check if a file or directory exists in the project directory
    pub fn project_file_exists(&self, path: &str) -> bool {
        self.project.join(path).exists()
    }

    /// Check if a file or directory exists in the home directory
    pub fn home_file_exists(&self, path: &str) -> bool {
        self.home_dir().join(path).exists()
    }
}

/// Builder pattern for creating test fixtures with default content
pub struct TestFixtureBuilder {
    fixture: TestFixture,
}

impl TestFixtureBuilder {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self { fixture: TestFixture::new()? })
    }

    /// Add default MCP servers configuration
    pub fn with_default_mcp_servers(self) -> std::io::Result<Self> {
        self.fixture.with_mcp_servers(
            r#"{
            "mcpServers": {
                "filesystem": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-filesystem"],
                    "env": {
                        "FILESYSTEM_ROOT": "/home/user"
                    }
                }
            }
        }"#,
        )?;
        Ok(self)
    }

    /// Add default settings configuration
    pub fn with_default_settings(self) -> std::io::Result<Self> {
        self.fixture.with_settings(
            r#"{
            "apiKeyHelper": "/usr/local/bin/api-key-helper",
            "cleanupPeriodDays": 20,
            "env": {
                "CUSTOM_VAR": "test_value"
            }
        }"#,
        )?;
        Ok(self)
    }

    /// Add a sample command
    pub fn with_sample_command(self) -> std::io::Result<Self> {
        self.fixture.with_command("test", "# Test Command\n\nThis is a test command.")?;
        Ok(self)
    }

    /// Add a sample rule
    pub fn with_sample_rule(self) -> std::io::Result<Self> {
        self.fixture.with_rule("test", "## Test Rule\n\nThis is a test rule.")?;
        Ok(self)
    }

    /// Build and return the fixture
    pub fn build(self) -> TestFixture {
        self.fixture
    }
}
