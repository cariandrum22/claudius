use super::{ClaudeConfig, McpServersConfig, Settings};
use crate::codex_settings::CodexSettings;
use anyhow::Context;
use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Write Claude configuration to a JSON file
///
/// # Errors
///
/// Returns an error if:
/// - Unable to create parent directories
/// - Unable to serialize the configuration
/// - Unable to write to the file
pub fn write_claude_config<P: AsRef<Path>>(path: P, config: &ClaudeConfig) -> anyhow::Result<()> {
    let path_ref = path.as_ref();

    // Ensure parent directory exists
    if let Some(parent) = path_ref.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(config)?;
    fs::write(path_ref, json)?;

    Ok(())
}

/// Create a backup of a file with timestamp
///
/// # Errors
///
/// Returns an error if unable to copy the file
pub fn backup_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Option<String>> {
    let path_ref = path.as_ref();

    if !path_ref.exists() {
        return Ok(None);
    }

    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let file_name = path_ref.file_name().and_then(|name| name.to_str()).unwrap_or("claude.json");
    let source_metadata = fs::metadata(path_ref)?;

    let mut suffix = 0_u64;
    loop {
        let backup_path = backup_path(path_ref, file_name, &timestamp.to_string(), suffix);
        match OpenOptions::new().write(true).create_new(true).open(&backup_path) {
            Ok(mut backup) => {
                let backup_result = (|| -> anyhow::Result<()> {
                    let mut source = fs::File::open(path_ref)?;
                    io::copy(&mut source, &mut backup)?;
                    backup.flush()?;
                    backup.set_permissions(source_metadata.permissions())?;
                    backup.sync_all()?;
                    Ok(())
                })();
                if let Err(error) = backup_result {
                    drop(backup);
                    let _ = fs::remove_file(&backup_path);
                    return Err(error);
                }
                return Ok(Some(backup_path.to_string_lossy().to_string()));
            },
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                suffix = suffix.checked_add(1).ok_or_else(|| {
                    anyhow::anyhow!("Exhausted backup names for {}", path_ref.display())
                })?;
            },
            Err(error) => return Err(error.into()),
        }
    }
}

fn backup_path(path: &Path, file_name: &str, timestamp: &str, suffix: u64) -> PathBuf {
    let suffix_text = if suffix > 0 { format!(".{suffix}") } else { String::new() };
    path.with_file_name(format!("{file_name}.backup.{timestamp}{suffix_text}"))
}

/// Atomically replace a file while preserving its permissions.
///
/// The temporary file is created beside the destination to keep the final
/// rename on the same filesystem. If `path` is a symlink, its resolved target
/// is replaced so the symlink itself remains intact.
///
/// # Errors
///
/// Returns an error if metadata cannot be read, the temporary file cannot be
/// written or synchronized, permissions cannot be copied, or the atomic
/// replacement fails.
pub fn atomic_write_preserving_permissions(path: &Path, content: &[u8]) -> anyhow::Result<()> {
    let destination = resolve_write_destination(path)?;
    let metadata = fs::metadata(&destination)
        .with_context(|| format!("Failed to read metadata for {}", destination.display()))?;
    let parent = destination.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "Cannot atomically replace path without a parent: {}",
            destination.display()
        )
    })?;

    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("Failed to create temporary file in {}", parent.display()))?;
    temporary.write_all(content)?;
    temporary.flush()?;
    temporary.as_file().set_permissions(metadata.permissions())?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(&destination)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to atomically replace {}", destination.display()))?;
    sync_parent_directory(parent)?;
    Ok(())
}

fn resolve_write_destination(path: &Path) -> anyhow::Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("Failed to inspect {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        fs::canonicalize(path)
            .with_context(|| format!("Failed to resolve symlink {}", path.display()))
    } else {
        Ok(path.to_path_buf())
    }
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) -> anyhow::Result<()> {
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) -> anyhow::Result<()> {
    Ok(())
}

/// Write MCP servers configuration to a JSON file
///
/// # Errors
///
/// Returns an error if:
/// - Unable to create parent directories
/// - Unable to serialize the configuration
/// - Unable to write to the file
pub fn write_mcp_servers_config<P: AsRef<Path>>(
    path: P,
    config: &McpServersConfig,
) -> anyhow::Result<()> {
    let path_ref = path.as_ref();

    // Ensure parent directory exists
    if let Some(parent) = path_ref.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(config)?;
    fs::write(path_ref, json)?;

    Ok(())
}

/// Write settings to a JSON file
///
/// # Errors
///
/// Returns an error if:
/// - Unable to create parent directories
/// - Unable to serialize the settings
/// - Unable to write to the file
pub fn write_settings<P: AsRef<Path>>(path: P, settings: &Settings) -> anyhow::Result<()> {
    let path_ref = path.as_ref();

    // Ensure parent directory exists
    if let Some(parent) = path_ref.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(settings)?;
    fs::write(path_ref, json)?;

    Ok(())
}

/// Write Codex settings to a TOML file
///
/// # Errors
///
/// Returns an error if:
/// - Unable to create parent directories
/// - Unable to serialize the settings to TOML
/// - Unable to write to the file
pub fn write_codex_settings<P: AsRef<Path>>(
    path: P,
    settings: &CodexSettings,
) -> anyhow::Result<()> {
    let path_ref = path.as_ref();

    // Ensure parent directory exists
    if let Some(parent) = path_ref.parent() {
        fs::create_dir_all(parent)?;
    }

    let toml = toml::to_string_pretty(settings)?;
    fs::write(path_ref, toml)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpServerConfig, Permissions};
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn create_test_claude_config() -> ClaudeConfig {
        ClaudeConfig {
            mcp_servers: Some(HashMap::from([(
                "server1".to_string(),
                McpServerConfig {
                    command: Some("cmd1".to_string()),
                    args: vec!["arg1".to_string()],
                    env: HashMap::new(),
                    server_type: None,
                    url: None,
                    headers: HashMap::new(),
                    extra: HashMap::new(),
                },
            )])),
            other: HashMap::from([("apiKeyHelper".to_string(), serde_json::json!("/bin/helper"))]),
        }
    }

    fn create_test_settings() -> Settings {
        Settings {
            api_key_helper: Some("/bin/helper".to_string()),
            cleanup_period_days: Some(30),
            env: Some(HashMap::from([("KEY".to_string(), "value".to_string())])),
            include_co_authored_by: Some(true),
            permissions: Some(Permissions {
                allow: vec!["Read".to_string()],
                deny: vec!["Write".to_string()],
                default_mode: Some("allow".to_string()),
                extra: HashMap::new(),
            }),
            preferred_notif_channel: Some("email".to_string()),
            mcp_servers: None,
            extra: HashMap::new(),
        }
    }

    fn create_test_codex_settings() -> CodexSettings {
        CodexSettings {
            model: Some("claude-3".to_string()),
            review_model: None,
            model_provider: Some("anthropic".to_string()),
            model_context_window: None,
            approval_policy: Some("manual".to_string()),
            disable_response_storage: Some(true),
            notify: Some(vec!["slack".to_string()]),
            model_providers: None,
            shell_environment_policy: None,
            sandbox_mode: None,
            sandbox_workspace_write: None,
            sandbox: None,
            history: None,
            mcp_servers: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_write_claude_config() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_path = temp_dir.path().join("claude.json");

        let config = create_test_claude_config();
        write_claude_config(&config_path, &config).expect("write_claude_config should succeed");

        assert!(config_path.exists());
        let content = fs::read_to_string(&config_path).expect("Failed to read config file");
        let parsed: ClaudeConfig = serde_json::from_str(&content).expect("Failed to parse JSON");

        assert_eq!(parsed.mcp_servers.expect("MCP servers should be present").len(), 1);
        assert_eq!(
            parsed.other.get("apiKeyHelper"),
            Some(&serde_json::Value::String("/bin/helper".to_string()))
        );
    }

    #[test]
    fn test_write_claude_config_creates_parent_dir() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_path = temp_dir.path().join("nested/dir/claude.json");

        let config = create_test_claude_config();
        write_claude_config(&config_path, &config).expect("write_claude_config should succeed");

        assert!(config_path.exists());
        assert!(config_path.parent().expect("Config path should have parent").exists());
    }

    #[test]
    fn test_backup_file_existing() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("test.json");

        fs::write(&file_path, "original content").expect("Failed to write original content");

        let backup_result = backup_file(&file_path).expect("backup_file should succeed");
        assert!(backup_result.is_some());

        let backup_path = backup_result.expect("Backup path should be present");
        assert!(Path::new(&backup_path).exists());
        assert!(backup_path.contains(".backup."));

        let backup_content = fs::read_to_string(&backup_path).expect("Failed to read backup file");
        assert_eq!(backup_content, "original content");
    }

    #[test]
    fn test_backup_file_nonexistent() {
        let result = backup_file("/nonexistent/file.json")
            .expect("backup_file should succeed for non-existent file");
        assert!(result.is_none());
    }

    #[test]
    fn test_backup_file_timestamp_format() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("test.json");

        fs::write(&file_path, "content").expect("Failed to write content");

        let backup_path = backup_file(&file_path)
            .expect("backup_file should succeed")
            .expect("Backup path should be present");

        // Check that backup path contains expected pattern
        assert!(backup_path.contains("test.json.backup."));

        // Extract timestamp and verify format (YYYYMMDD_HHMMSS)
        let parts: Vec<&str> = backup_path.split('.').collect();
        let timestamp = parts.last().expect("Parts should have last element");
        assert_eq!(timestamp.len(), 15); // YYYYMMDD_HHMMSS
        assert_eq!(timestamp.chars().nth(8).expect("Timestamp should have 9th character"), '_');
    }

    #[test]
    fn test_backup_file_never_overwrites_an_existing_backup() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("test.json");
        fs::write(&file_path, "first version").expect("Failed to write first version");
        let first = backup_file(&file_path)
            .expect("First backup should succeed")
            .expect("First backup should exist");

        fs::write(&file_path, "second version").expect("Failed to write second version");
        let second = backup_file(&file_path)
            .expect("Second backup should succeed")
            .expect("Second backup should exist");

        assert_ne!(first, second);
        assert_eq!(
            fs::read_to_string(first).expect("First backup should be readable"),
            "first version"
        );
        assert_eq!(
            fs::read_to_string(second).expect("Second backup should be readable"),
            "second version"
        );
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_preserves_mode() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("settings.json");
        fs::write(&file_path, "old").expect("Failed to write original file");
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o640))
            .expect("Failed to set original mode");

        atomic_write_preserving_permissions(&file_path, b"new")
            .expect("Atomic write should succeed");

        assert_eq!(fs::read_to_string(&file_path).expect("File should be readable"), "new");
        assert_eq!(
            fs::metadata(file_path).expect("Metadata should load").permissions().mode() & 0o777,
            0o640
        );
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_preserves_symlink() {
        use std::os::unix::fs::symlink;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let target = temp_dir.path().join("source.json");
        let link = temp_dir.path().join("settings.json");
        fs::write(&target, "old").expect("Failed to write symlink target");
        symlink(&target, &link).expect("Failed to create symlink");

        atomic_write_preserving_permissions(&link, b"new").expect("Atomic write should succeed");

        assert!(fs::symlink_metadata(&link)
            .expect("Symlink metadata should load")
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_to_string(target).expect("Target should be readable"), "new");
    }

    #[test]
    fn test_write_mcp_servers_config() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_path = temp_dir.path().join("mcpServers.json");

        let config = McpServersConfig {
            mcp_servers: HashMap::from([(
                "test-server".to_string(),
                McpServerConfig {
                    command: Some("test-cmd".to_string()),
                    args: vec!["--arg".to_string()],
                    env: HashMap::from([("ENV_VAR".to_string(), "value".to_string())]),
                    server_type: None,
                    url: None,
                    headers: HashMap::new(),
                    extra: HashMap::new(),
                },
            )]),
        };

        write_mcp_servers_config(&config_path, &config)
            .expect("write_mcp_servers_config should succeed");

        assert!(config_path.exists());
        let content = fs::read_to_string(&config_path).expect("Failed to read config file");
        let parsed: McpServersConfig =
            serde_json::from_str(&content).expect("Failed to parse MCP servers JSON");

        assert_eq!(parsed.mcp_servers.len(), 1);
        let server = parsed.mcp_servers.get("test-server").expect("test-server should exist");
        assert_eq!(server.command.as_deref(), Some("test-cmd"));
        assert_eq!(server.env.get("ENV_VAR"), Some(&"value".to_string()));
    }

    #[test]
    fn test_write_mcp_servers_config_creates_parent_dir() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_path = temp_dir.path().join("config/mcp/mcpServers.json");

        let config = McpServersConfig { mcp_servers: HashMap::new() };

        write_mcp_servers_config(&config_path, &config)
            .expect("write_mcp_servers_config should succeed");

        assert!(config_path.exists());
        assert!(config_path.parent().expect("Config path should have parent").exists());
    }

    #[test]
    fn test_write_settings() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let settings_path = temp_dir.path().join("settings.json");

        let settings = create_test_settings();
        write_settings(&settings_path, &settings).expect("write_settings should succeed");

        assert!(settings_path.exists());
        let content = fs::read_to_string(&settings_path).expect("Failed to read settings file");
        let parsed: Settings =
            serde_json::from_str(&content).expect("Failed to parse settings JSON");

        assert_eq!(parsed.api_key_helper, Some("/bin/helper".to_string()));
        assert_eq!(parsed.cleanup_period_days, Some(30));
        assert_eq!(parsed.permissions.expect("Permissions should be present").allow, vec!["Read"]);
    }

    #[test]
    fn test_write_settings_creates_parent_dir() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let settings_path = temp_dir.path().join("nested/.claude/settings.json");

        let settings = create_test_settings();
        write_settings(&settings_path, &settings).expect("write_settings should succeed");

        assert!(settings_path.exists());
        assert!(settings_path.parent().expect("Settings path should have parent").exists());
    }

    #[test]
    fn test_write_codex_settings() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let settings_path = temp_dir.path().join("codex.toml");

        let settings = create_test_codex_settings();
        write_codex_settings(&settings_path, &settings)
            .expect("write_codex_settings should succeed");

        assert!(settings_path.exists());
        let content = fs::read_to_string(&settings_path).expect("Failed to read settings file");

        // Verify TOML format
        assert!(content.contains("model = \"claude-3\""));
        assert!(content.contains("model_provider = \"anthropic\""));
        assert!(content.contains("approval_policy = \"manual\""));
        assert!(content.contains("disable_response_storage = true"));
        assert!(content.contains("notify = [\"slack\"]"));

        // Parse back to verify correctness
        let parsed: CodexSettings = toml::from_str(&content).expect("Failed to parse TOML");
        assert_eq!(parsed.model, Some("claude-3".to_string()));
        assert_eq!(parsed.model_provider, Some("anthropic".to_string()));
        assert_eq!(parsed.disable_response_storage, Some(true));
    }

    #[test]
    fn test_write_codex_settings_creates_parent_dir() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let settings_path = temp_dir.path().join("config/codex/settings.toml");

        let settings = create_test_codex_settings();
        write_codex_settings(&settings_path, &settings)
            .expect("write_codex_settings should succeed");

        assert!(settings_path.exists());
        assert!(settings_path.parent().expect("Settings path should have parent").exists());
    }
}
