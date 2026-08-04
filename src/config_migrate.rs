//! Migration engine for deprecated agent settings.
//!
//! Rewrites Claudius source configuration files to replace deprecated
//! settings with their documented successors. The engine is intentionally
//! conservative: it only applies transformations whose semantics are
//! documented by the agent vendors, it never touches unknown fields, and
//! re-running a migration on already-migrated files produces no changes.

use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, Key};

use crate::app_config::Agent;
use crate::config::writer::backup_file;

/// Official JSON schema for Claude Code `settings.json`.
pub const CLAUDE_SETTINGS_SCHEMA_URL: &str =
    "https://json.schemastore.org/claude-code-settings.json";

/// Result of migrating one file's content: `(migrated, changes, notes)`.
pub type MigrationOutcome = (String, Vec<String>, Vec<String>);

/// Top-level Codex `config.toml` keys that were renamed upstream.
const CODEX_RENAMED_KEYS: &[(&str, &str)] = &[
    ("background_terminal_timeout", "background_terminal_max_timeout"),
    ("experimental_instructions_file", "model_instructions_file"),
];

/// A planned rewrite of a single configuration file.
#[derive(Debug, Clone)]
pub struct FileMigration {
    /// File the migration applies to.
    pub path: PathBuf,
    /// Content currently on disk.
    pub original: String,
    /// Content after applying all migration rules.
    pub migrated: String,
    /// Human-readable descriptions of the applied rules.
    pub changes: Vec<String>,
}

impl FileMigration {
    /// Whether applying this migration would modify the file.
    #[must_use]
    pub fn is_changed(&self) -> bool {
        self.original != self.migrated
    }
}

/// The full set of planned migrations plus advisory notes.
#[derive(Debug, Clone, Default)]
pub struct MigrationPlan {
    /// Per-file migrations (only files with pending changes are included).
    pub files: Vec<FileMigration>,
    /// Findings that require manual action and are never auto-applied.
    pub notes: Vec<String>,
}

/// Plan migrations for the requested agent (or all agents when `None`).
///
/// # Errors
///
/// Returns an error if a source file cannot be read or parsed.
pub fn plan_migration(config_dir: &Path, agent: Option<Agent>) -> Result<MigrationPlan> {
    let mut plan = MigrationPlan::default();

    match agent {
        Some(Agent::Claude | Agent::ClaudeCode) => plan_claude(config_dir, &mut plan)?,
        Some(Agent::Codex) => plan_codex(config_dir, &mut plan)?,
        Some(Agent::Gemini) => plan
            .notes
            .push("No migration rules are defined for Gemini settings yet".to_string()),
        None => {
            plan_claude(config_dir, &mut plan)?;
            plan_codex(config_dir, &mut plan)?;
        },
    }

    Ok(plan)
}

/// Apply a previously computed plan, creating a timestamped backup of every
/// file before it is rewritten. Returns the created backup paths.
///
/// # Errors
///
/// Returns an error if a backup cannot be created or a file cannot be
/// written.
pub fn apply_migration(plan: &MigrationPlan) -> Result<Vec<String>> {
    let changed_files = plan.files.iter().filter(|file| file.is_changed()).collect::<Vec<_>>();

    for file in &changed_files {
        let current = fs::read_to_string(&file.path)
            .with_context(|| format!("Failed to re-read {}", file.path.display()))?;
        if current != file.original {
            anyhow::bail!(
                "Refusing to overwrite {} because it changed after the migration was planned",
                file.path.display()
            );
        }
    }

    changed_files
        .iter()
        .map(|file| {
            let backup = backup_file(&file.path)
                .with_context(|| format!("Failed to back up {}", file.path.display()))?
                .ok_or_else(|| {
                    anyhow::anyhow!("Refusing to migrate missing file: {}", file.path.display())
                })?;
            fs::write(&file.path, &file.migrated)
                .with_context(|| format!("Failed to write {}", file.path.display()))?;
            Ok(backup)
        })
        .collect()
}

fn plan_claude(config_dir: &Path, plan: &mut MigrationPlan) -> Result<()> {
    let preferred = config_dir.join("claude.settings.json");
    let legacy = config_dir.join("settings.json");
    let source = if preferred.exists() {
        if legacy.exists() {
            plan.notes.push(format!(
                "Legacy {} also exists; only {} is migrated because sync prefers it",
                legacy.display(),
                preferred.display()
            ));
        }
        preferred
    } else if legacy.exists() {
        legacy
    } else {
        return Ok(());
    };

    let content = fs::read_to_string(&source)
        .with_context(|| format!("Failed to read {}", source.display()))?;
    let (migrated, changes, notes) = migrate_claude_settings_content(&content)
        .with_context(|| format!("Failed to migrate {}", source.display()))?;

    plan.notes.extend(notes);
    if migrated != content {
        plan.files
            .push(FileMigration { path: source, original: content, migrated, changes });
    }
    Ok(())
}

fn plan_codex(config_dir: &Path, plan: &mut MigrationPlan) -> Result<()> {
    plan_toml_file(config_dir.join("codex.settings.toml"), migrate_codex_settings_content, plan)?;
    plan_toml_file(
        config_dir.join("codex.requirements.toml"),
        migrate_codex_requirements_content,
        plan,
    )?;
    plan_codex_managed_config(config_dir, plan)
}

fn plan_codex_managed_config(config_dir: &Path, plan: &mut MigrationPlan) -> Result<()> {
    let preferred = config_dir.join("codex.managed_config.toml");
    let legacy = config_dir.join("managed_config.toml");
    let source = if preferred.exists() {
        if legacy.exists() {
            plan.notes.push(format!(
                "Legacy {} also exists; only {} is migrated because sync prefers it",
                legacy.display(),
                preferred.display()
            ));
        }
        preferred
    } else if legacy.exists() {
        plan.notes.push(format!(
            "Using legacy {}; rename it to codex.managed_config.toml manually",
            legacy.display()
        ));
        legacy
    } else {
        return Ok(());
    };

    plan_toml_file(source, migrate_codex_managed_config_content, plan)
}

fn plan_toml_file(
    path: PathBuf,
    migrate: fn(&str) -> Result<MigrationOutcome>,
    plan: &mut MigrationPlan,
) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    let (migrated, changes, notes) =
        migrate(&content).with_context(|| format!("Failed to migrate {}", path.display()))?;

    plan.notes.extend(notes);
    if migrated != content {
        plan.files.push(FileMigration { path, original: content, migrated, changes });
    }
    Ok(())
}

/// Migrate Claude settings JSON content.
///
/// Rules:
/// - Replace deprecated `includeCoAuthoredBy` with the `attribution` block.
/// - Add the official `$schema` reference when missing.
///
/// Returns `(migrated, changes, notes)`. When no rule applies, the original
/// content is returned byte-for-byte so re-runs stay idempotent.
///
/// # Errors
///
/// Returns an error if the content is not a JSON object.
pub fn migrate_claude_settings_content(content: &str) -> Result<MigrationOutcome> {
    let value: Value = serde_json::from_str(content).context("Settings file is not valid JSON")?;
    let Value::Object(fields) = value else {
        anyhow::bail!("Settings file must contain a JSON object");
    };

    let mut changes = Vec::new();
    let mut notes = Vec::new();
    let without_flag = migrate_include_co_authored_by(fields, &mut changes, &mut notes);
    let with_schema = ensure_schema_field(without_flag, &mut changes);

    if changes.is_empty() {
        return Ok((content.to_string(), changes, notes));
    }

    let mut rendered = serde_json::to_string_pretty(&Value::Object(with_schema))
        .context("Failed to render migrated settings")?;
    rendered.push('\n');
    Ok((rendered, changes, notes))
}

fn migrate_include_co_authored_by(
    fields: Map<String, Value>,
    changes: &mut Vec<String>,
    notes: &mut Vec<String>,
) -> Map<String, Value> {
    let Some(flag_value) = fields.get("includeCoAuthoredBy") else {
        return fields;
    };
    let Some(flag) = flag_value.as_bool() else {
        notes.push(
            "includeCoAuthoredBy has a non-boolean value; migrate it to attribution manually"
                .to_string(),
        );
        return fields;
    };

    let has_attribution = fields.contains_key("attribution");
    changes.push(match (has_attribution, flag) {
        (true, _) => {
            "removed deprecated includeCoAuthoredBy (attribution is already configured)"
        },
        (false, false) => {
            "replaced includeCoAuthoredBy = false with attribution { \"commit\": \"\", \"pr\": \"\", \"sessionUrl\": false }"
        },
        (false, true) => {
            "removed includeCoAuthoredBy = true (matches the default attribution behavior)"
        },
    }.to_string());

    fields
        .into_iter()
        .flat_map(|(key, value)| {
            if key == "includeCoAuthoredBy" {
                (!has_attribution && !flag)
                    .then(|| {
                        (
                            "attribution".to_string(),
                            serde_json::json!({ "commit": "", "pr": "", "sessionUrl": false }),
                        )
                    })
                    .into_iter()
                    .collect::<Vec<_>>()
            } else {
                vec![(key, value)]
            }
        })
        .collect()
}

fn ensure_schema_field(
    fields: Map<String, Value>,
    changes: &mut Vec<String>,
) -> Map<String, Value> {
    if fields.contains_key("$schema") {
        return fields;
    }

    changes.push(format!("added \"$schema\": \"{CLAUDE_SETTINGS_SCHEMA_URL}\""));
    std::iter::once(("$schema".to_string(), Value::String(CLAUDE_SETTINGS_SCHEMA_URL.to_string())))
        .chain(fields)
        .collect()
}

/// Migrate Codex `config.toml` content, preserving comments and layout.
///
/// Rules (documented upstream renames only):
/// - `background_terminal_timeout` → `background_terminal_max_timeout`
/// - `experimental_instructions_file` → `model_instructions_file`
///
/// `shell_environment_policy.exclude` / `include_only` are intentionally NOT
/// auto-converted to `filters`: the exact translation semantics are not
/// documented upstream, so `claudius config validate` only reports them.
///
/// # Errors
///
/// Returns an error if the content is not valid TOML.
pub fn migrate_codex_settings_content(content: &str) -> Result<MigrationOutcome> {
    migrate_codex_settings_like_content(content, false)
}

/// Migrate Codex managed defaults, preserving comments and layout.
///
/// Documented key renames are applied mechanically. A deprecated
/// `approval_policy = "on-failure"` value is only reported because choosing
/// between `on-request` and `never` requires an administrator decision.
///
/// # Errors
///
/// Returns an error if the content is not valid TOML.
pub fn migrate_codex_managed_config_content(content: &str) -> Result<MigrationOutcome> {
    migrate_codex_settings_like_content(content, true)
}

fn migrate_codex_settings_like_content(
    content: &str,
    report_approval_policy: bool,
) -> Result<MigrationOutcome> {
    let mut doc: DocumentMut = content.parse().context("Codex settings file is not valid TOML")?;
    let mut changes = Vec::new();
    let mut notes = Vec::new();

    for (old, new) in CODEX_RENAMED_KEYS {
        rename_top_level_key(&mut doc, old, new, &mut changes, &mut notes);
    }

    if report_approval_policy
        && doc.get("approval_policy").and_then(Item::as_str) == Some("on-failure")
    {
        notes.push(
            "approval_policy = \"on-failure\" requires a manual choice: use \"on-request\" for interactive approvals or \"never\" for non-interactive operation"
                .to_string(),
        );
    }

    if changes.is_empty() {
        return Ok((content.to_string(), changes, notes));
    }
    Ok((doc.to_string(), changes, notes))
}

fn rename_top_level_key(
    doc: &mut DocumentMut,
    old: &str,
    new: &str,
    changes: &mut Vec<String>,
    notes: &mut Vec<String>,
) {
    let table = doc.as_table_mut();
    if !table.contains_key(old) {
        return;
    }
    if table.contains_key(new) {
        notes.push(format!("{old} and {new} are both set; remove the deprecated {old} manually"));
        return;
    }

    if let Some((key, item)) = table.remove_entry(old) {
        let mut renamed = Key::new(new);
        *renamed.leaf_decor_mut() = key.leaf_decor().clone();
        table.insert_formatted(&renamed, item);
        changes.push(format!("renamed {old} to {new}"));
    }
}

/// Migrate Codex `requirements.toml` content, preserving comments.
///
/// Removes the deprecated `"on-failure"` entry from
/// `allowed_approval_policies`.
///
/// # Errors
///
/// Returns an error if the content is not valid TOML.
pub fn migrate_codex_requirements_content(content: &str) -> Result<MigrationOutcome> {
    let mut doc: DocumentMut =
        content.parse().context("Codex requirements file is not valid TOML")?;
    let mut changes = Vec::new();
    let mut notes = Vec::new();

    if let Some(array) = doc.get_mut("allowed_approval_policies").and_then(Item::as_array_mut) {
        let before = array.len();
        let retained = array.iter().filter(|value| value.as_str() != Some("on-failure")).count();
        if retained == 0 && before > 0 {
            notes.push(
                "allowed_approval_policies only contains deprecated \"on-failure\"; choose a replacement policy manually"
                    .to_string(),
            );
        } else {
            array.retain(|value| value.as_str() != Some("on-failure"));
        }
        if array.len() < before {
            changes.push(
                "removed deprecated \"on-failure\" from allowed_approval_policies".to_string(),
            );
        }
    }

    if changes.is_empty() {
        return Ok((content.to_string(), changes, notes));
    }
    Ok((doc.to_string(), changes, notes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn migrate_claude(content: &str) -> (String, Vec<String>, Vec<String>) {
        migrate_claude_settings_content(content).expect("migration should succeed")
    }

    #[test]
    fn include_co_authored_by_false_becomes_hiding_attribution() {
        let content = r#"{
  "cleanupPeriodDays": 30,
  "includeCoAuthoredBy": false
}
"#;
        let (migrated, changes, notes) = migrate_claude(content);
        let value: Value = serde_json::from_str(&migrated).expect("migrated JSON should parse");

        assert_eq!(value.get("includeCoAuthoredBy"), None);
        assert_eq!(
            value.get("attribution"),
            Some(&json!({ "commit": "", "pr": "", "sessionUrl": false }))
        );
        assert_eq!(value.get("cleanupPeriodDays"), Some(&json!(30)));
        assert!(value.get("$schema").is_some());
        assert_eq!(changes.len(), 2);
        assert!(notes.is_empty());
    }

    #[test]
    fn include_co_authored_by_true_is_dropped_without_attribution() {
        let (migrated, changes, _) = migrate_claude(r#"{ "includeCoAuthoredBy": true }"#);
        let value: Value = serde_json::from_str(&migrated).expect("migrated JSON should parse");

        assert_eq!(value.get("includeCoAuthoredBy"), None);
        assert_eq!(value.get("attribution"), None);
        assert!(changes.iter().any(|change| change.contains("default attribution")));
    }

    #[test]
    fn existing_attribution_wins_over_deprecated_flag() {
        let content = r#"{
  "attribution": { "commit": "custom", "pr": "" },
  "includeCoAuthoredBy": false
}
"#;
        let (migrated, changes, _) = migrate_claude(content);
        let value: Value = serde_json::from_str(&migrated).expect("migrated JSON should parse");

        assert_eq!(value.get("includeCoAuthoredBy"), None);
        assert_eq!(value.get("attribution"), Some(&json!({ "commit": "custom", "pr": "" })));
        assert!(changes.iter().any(|change| change.contains("already configured")));
    }

    #[test]
    fn non_boolean_flag_is_left_alone_with_a_note() {
        let content = r#"{
  "$schema": "https://json.schemastore.org/claude-code-settings.json",
  "includeCoAuthoredBy": "yes"
}
"#;
        let (migrated, changes, notes) = migrate_claude(content);

        assert_eq!(migrated, content);
        assert!(changes.is_empty());
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn unknown_fields_and_order_are_preserved() {
        let content = r#"{
  "futureSetting": { "nested": [1, 2, 3] },
  "includeCoAuthoredBy": false,
  "zebra": true
}
"#;
        let (migrated, _, _) = migrate_claude(content);
        let value: Value = serde_json::from_str(&migrated).expect("migrated JSON should parse");
        let keys: Vec<&String> = value.as_object().expect("object").keys().collect();

        assert_eq!(keys, ["$schema", "futureSetting", "attribution", "zebra"]);
        assert_eq!(value.get("futureSetting"), Some(&json!({ "nested": [1, 2, 3] })));
    }

    #[test]
    fn claude_migration_is_idempotent() {
        let (first, _, _) = migrate_claude(r#"{ "includeCoAuthoredBy": false }"#);
        let (second, changes, notes) = migrate_claude(&first);

        assert_eq!(first, second);
        assert!(changes.is_empty());
        assert!(notes.is_empty());
    }

    #[test]
    fn codex_rename_preserves_comments() {
        let content = "# How long background terminals may run\n\
                       background_terminal_timeout = 300\n\
                       \n\
                       # Unrelated setting\n\
                       model = \"gpt-5.5\"\n";
        let (migrated, changes, notes) =
            migrate_codex_settings_content(content).expect("migration should succeed");

        assert!(migrated.contains("background_terminal_max_timeout = 300"));
        assert!(!migrated.contains("background_terminal_timeout ="));
        assert!(migrated.contains("# How long background terminals may run"));
        assert!(migrated.contains("# Unrelated setting"));
        assert!(migrated.contains("model = \"gpt-5.5\""));
        assert_eq!(changes.len(), 1);
        assert!(notes.is_empty());
    }

    #[test]
    fn codex_rename_conflict_is_reported_not_applied() {
        let content = "background_terminal_timeout = 300\n\
                       background_terminal_max_timeout = 600\n";
        let (migrated, changes, notes) =
            migrate_codex_settings_content(content).expect("migration should succeed");

        assert_eq!(migrated, content);
        assert!(changes.is_empty());
        assert_eq!(notes.len(), 1);
        assert!(
            notes.first().is_some_and(
                |note| note.contains("remove the deprecated background_terminal_timeout")
            )
        );
    }

    #[test]
    fn codex_migration_is_idempotent() {
        let content = "experimental_instructions_file = \"instructions.md\"\n";
        let (first, _, _) =
            migrate_codex_settings_content(content).expect("migration should succeed");
        let (second, changes, _) =
            migrate_codex_settings_content(&first).expect("migration should succeed");

        assert_eq!(first, second);
        assert!(changes.is_empty());
    }

    #[test]
    fn managed_config_applies_safe_renames_and_reports_approval_policy() {
        let content = "# Managed defaults\n\
                       approval_policy = \"on-failure\"\n\
                       experimental_instructions_file = \"instructions.md\"\n";
        let (migrated, changes, notes) =
            migrate_codex_managed_config_content(content).expect("migration should succeed");

        assert!(migrated.contains("# Managed defaults"));
        assert!(migrated.contains("approval_policy = \"on-failure\""));
        assert!(migrated.contains("model_instructions_file = \"instructions.md\""));
        assert!(!migrated.contains("experimental_instructions_file ="));
        assert_eq!(changes.len(), 1);
        assert!(notes.iter().any(|note| note.contains("manual choice")));
    }

    #[test]
    fn managed_config_does_not_rewrite_approval_policy() {
        let content = "approval_policy = \"on-failure\"\n";
        let (migrated, changes, notes) =
            migrate_codex_managed_config_content(content).expect("migration should succeed");

        assert_eq!(migrated, content);
        assert!(changes.is_empty());
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn requirements_on_failure_is_removed_and_comments_survive() {
        let content = "# Admin approved policies\n\
                       allowed_approval_policies = [\"untrusted\", \"on-failure\", \"never\"]\n";
        let (migrated, changes, _) =
            migrate_codex_requirements_content(content).expect("migration should succeed");

        assert!(migrated.contains("# Admin approved policies"));
        assert!(!migrated.contains("on-failure"));
        assert!(migrated.contains("\"untrusted\""));
        assert!(migrated.contains("\"never\""));
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn requirements_without_on_failure_are_untouched() {
        let content = "allowed_approval_policies = [\"untrusted\", \"never\"]\n";
        let (migrated, changes, _) =
            migrate_codex_requirements_content(content).expect("migration should succeed");

        assert_eq!(migrated, content);
        assert!(changes.is_empty());
    }

    #[test]
    fn requirements_with_only_on_failure_requires_manual_replacement() {
        let content = "allowed_approval_policies = [\"on-failure\"]\n";
        let (migrated, changes, notes) =
            migrate_codex_requirements_content(content).expect("migration should succeed");

        assert_eq!(migrated, content);
        assert!(changes.is_empty());
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn apply_refuses_to_overwrite_a_file_changed_after_planning() {
        let temp_dir = tempfile::TempDir::new().expect("temp directory should be created");
        let path = temp_dir.path().join("settings.json");
        fs::write(&path, "original").expect("original file should be written");
        let plan = MigrationPlan {
            files: vec![FileMigration {
                path: path.clone(),
                original: "original".to_string(),
                migrated: "migrated".to_string(),
                changes: vec!["test migration".to_string()],
            }],
            notes: Vec::new(),
        };
        fs::write(&path, "changed externally").expect("file should be changed");

        let error = apply_migration(&plan).expect_err("stale plan should be rejected");
        assert!(error.to_string().contains("changed after the migration was planned"));
        assert_eq!(
            fs::read_to_string(path).expect("file should remain readable"),
            "changed externally"
        );
    }
}
