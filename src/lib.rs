#![allow(missing_docs)]

pub mod agent_paths;
pub mod app_config;
pub mod bootstrap;
pub mod cli;
pub mod codex_settings;
pub mod commands;
pub mod config;
pub mod gemini_settings;
pub(crate) mod json_merge;
pub mod merge;
pub mod profiling;
pub mod secrets;
pub mod sync_operations;
pub mod template;
pub mod validation;
pub mod variable_expansion;

pub use config::{Config, McpServersConfig};
pub use merge::{merge_configs, MergeStrategy};

#[derive(Debug, thiserror::Error)]
pub enum ClaudiusError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Merge error: {0}")]
    Merge(String),
}
