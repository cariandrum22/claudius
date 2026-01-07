#![allow(clippy::self_named_module_files)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub mod reader;
pub mod writer;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServerConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub server_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    #[serde(default, flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServersConfig {
    #[serde(rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeConfig {
    #[serde(rename = "mcpServers", skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<HashMap<String, McpServerConfig>>,
    #[serde(flatten)]
    pub other: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionRule {
    #[serde(flatten)]
    pub rule: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Permissions {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(rename = "defaultMode", skip_serializing_if = "Option::is_none")]
    pub default_mode: Option<String>,
    #[serde(default, flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    #[serde(rename = "apiKeyHelper", skip_serializing_if = "Option::is_none")]
    pub api_key_helper: Option<String>,

    #[serde(rename = "cleanupPeriodDays", skip_serializing_if = "Option::is_none")]
    pub cleanup_period_days: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    #[serde(rename = "includeCoAuthoredBy", skip_serializing_if = "Option::is_none")]
    pub include_co_authored_by: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,

    #[serde(rename = "preferredNotifChannel", skip_serializing_if = "Option::is_none")]
    pub preferred_notif_channel: Option<String>,

    #[serde(rename = "mcpServers", skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<HashMap<String, McpServerConfig>>,

    // Catch-all for unknown fields to preserve them
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug)]
pub struct Config {
    pub mcp_servers_path: PathBuf,
    pub settings_path: PathBuf,
    pub target_config_path: PathBuf,
    pub project_settings_path: Option<PathBuf>,
    pub rules_dir: PathBuf,
    pub commands_dir: PathBuf,
    pub claude_commands_dir: PathBuf,
    pub is_global: bool,
    pub agent: Option<crate::app_config::Agent>,
}

impl Config {
    /// Creates a new Config instance.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The home directory cannot be determined
    /// - Directory creation fails
    pub fn new(use_global: bool) -> anyhow::Result<Self> {
        Self::new_with_agent(use_global, None)
    }

    /// Creates a new Config instance with an optional agent.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The home directory cannot be determined
    /// - The configuration directory cannot be created
    /// - The current directory cannot be determined (when not using global mode)
    pub fn new_with_agent(
        use_global: bool,
        agent: Option<crate::app_config::Agent>,
    ) -> anyhow::Result<Self> {
        let config_dir = Self::get_config_dir()?;
        let home_dir = directories::BaseDirs::new()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
            .home_dir()
            .to_path_buf();

        let (target_config_path, project_settings_path, actual_settings_path) =
            Self::determine_paths(use_global, agent, &config_dir, &home_dir)?;

        let claude_commands_dir = Self::determine_commands_dir(use_global, &home_dir)?;

        Ok(Self {
            mcp_servers_path: config_dir.join("mcpServers.json"),
            settings_path: actual_settings_path,
            target_config_path,
            project_settings_path,
            rules_dir: config_dir.join("rules"),
            commands_dir: config_dir.join("commands"),
            claude_commands_dir,
            is_global: use_global,
            agent,
        })
    }

    /// Determine paths based on mode and agent
    fn determine_paths(
        use_global: bool,
        agent: Option<crate::app_config::Agent>,
        config_dir: &Path,
        home_dir: &Path,
    ) -> anyhow::Result<(PathBuf, Option<PathBuf>, PathBuf)> {
        if use_global {
            Ok(Self::determine_global_paths(agent, config_dir, home_dir))
        } else {
            Self::determine_project_paths(agent, config_dir)
        }
    }

    /// Determine paths for global mode
    fn determine_global_paths(
        agent: Option<crate::app_config::Agent>,
        config_dir: &Path,
        home_dir: &Path,
    ) -> (PathBuf, Option<PathBuf>, PathBuf) {
        let path = home_dir.join(".claude.json");

        match agent {
            Some(crate::app_config::Agent::Gemini) => {
                let gemini_input = config_dir.join("gemini.settings.json");
                (path, None, gemini_input)
            },
            Some(crate::app_config::Agent::Codex) => {
                let codex_input = config_dir.join("codex.settings.toml");
                (path, None, codex_input)
            },
            _ => (path, None, config_dir.join("claude.settings.json")),
        }
    }

    /// Determine paths for project-local mode
    fn determine_project_paths(
        agent: Option<crate::app_config::Agent>,
        config_dir: &Path,
    ) -> anyhow::Result<(PathBuf, Option<PathBuf>, PathBuf)> {
        let current_dir = std::env::current_dir()?;
        let mcp_path = current_dir.join(".mcp.json");

        match agent {
            Some(crate::app_config::Agent::Gemini) => {
                let settings_path = current_dir.join("gemini").join("settings.json");
                Ok((mcp_path, Some(settings_path), config_dir.join("gemini.settings.json")))
            },
            Some(crate::app_config::Agent::Codex) => {
                let settings_path = current_dir.join(".codex").join("config.toml");
                Ok((mcp_path, Some(settings_path), config_dir.join("codex.settings.toml")))
            },
            _ => {
                let settings_path = current_dir.join(".claude").join("settings.json");
                Ok((mcp_path, Some(settings_path), config_dir.join("claude.settings.json")))
            },
        }
    }

    /// Determine commands directory based on mode
    fn determine_commands_dir(use_global: bool, home_dir: &Path) -> anyhow::Result<PathBuf> {
        if use_global {
            Ok(home_dir.join(".claude").join("commands"))
        } else {
            Ok(std::env::current_dir()?.join(".claude").join("commands"))
        }
    }

    pub fn with_paths<P: Into<PathBuf>>(mcp_servers: P, target_config: P) -> Self {
        let config_dir = Self::get_config_dir().unwrap_or_else(|_| PathBuf::from("."));
        let claude_commands_dir = directories::BaseDirs::new().map_or_else(
            || PathBuf::from(".claude/commands"),
            |d| d.home_dir().join(".claude").join("commands"),
        );

        Self {
            mcp_servers_path: mcp_servers.into(),
            settings_path: config_dir.join("settings.json"),
            target_config_path: target_config.into(),
            project_settings_path: None,
            rules_dir: config_dir.join("rules"),
            commands_dir: config_dir.join("commands"),
            claude_commands_dir,
            is_global: true,
            agent: None,
        }
    }

    pub fn with_all_paths<P: Into<PathBuf>>(mcp_servers: P, settings: P, target_config: P) -> Self {
        let config_dir = Self::get_config_dir().unwrap_or_else(|_| PathBuf::from("."));
        let claude_commands_dir = directories::BaseDirs::new().map_or_else(
            || PathBuf::from(".claude/commands"),
            |d| d.home_dir().join(".claude").join("commands"),
        );

        Self {
            mcp_servers_path: mcp_servers.into(),
            settings_path: settings.into(),
            target_config_path: target_config.into(),
            project_settings_path: None,
            rules_dir: config_dir.join("rules"),
            commands_dir: config_dir.join("commands"),
            claude_commands_dir,
            is_global: true,
            agent: None,
        }
    }

    /// Gets the configuration directory path.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration directory cannot be created.
    pub fn get_config_dir() -> anyhow::Result<PathBuf> {
        // Use XDG_CONFIG_HOME or fallback to ~/.config
        let config_dir = if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg_config).join("claudius")
        } else {
            directories::BaseDirs::new()
                .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
                .home_dir()
                .join(".config")
                .join("claudius")
        };
        Ok(config_dir)
    }

    /// Detect which agents have configuration files
    /// Detects available AI agents based on their configuration files.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The home directory cannot be determined
    /// - Directory reading fails
    pub fn detect_available_agents() -> anyhow::Result<Vec<crate::app_config::Agent>> {
        let config_dir = Self::get_config_dir()?;
        let mut agents = Vec::new();

        // Check for Claude settings
        if config_dir.join("claude.settings.json").exists() {
            agents.push(crate::app_config::Agent::Claude);
        }

        // Check for Codex settings
        if config_dir.join("codex.settings.toml").exists() {
            agents.push(crate::app_config::Agent::Codex);
        }

        // Check for Gemini settings
        if config_dir.join("gemini.settings.json").exists() {
            agents.push(crate::app_config::Agent::Gemini);
        }

        Ok(agents)
    }
}
