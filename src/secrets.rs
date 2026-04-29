use anyhow::{Context, Result};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
#[cfg(not(test))]
use std::process::Command;
use std::sync::{Arc, Mutex};
use tracing::{debug, warn};

#[cfg(test)]
use crate::app_config::OnePasswordConfig;
use crate::app_config::{OnePasswordMode, SecretManagerConfig, SecretManagerType};
use crate::profiling::{SecretResolutionMetrics, Timer};
use crate::variable_expansion::expand_variables;

const ONEPASSWORD_MODE_ENV: &str = "CLAUDIUS_1PASSWORD_MODE";
const LEGACY_ONEPASSWORD_MODE_ENV: &str = "CLAUDIUS_OP_MODE";
const ONEPASSWORD_TOKEN_PATH_ENV: &str = "CLAUDIUS_1PASSWORD_SERVICE_ACCOUNT_TOKEN_PATH";
const LEGACY_ONEPASSWORD_TOKEN_PATH_ENV: &str = "CLAUDIUS_OP_SERVICE_ACCOUNT_TOKEN_PATH";
const OP_SERVICE_ACCOUNT_TOKEN_ENV: &str = "OP_SERVICE_ACCOUNT_TOKEN";
const ONEPASSWORD_AUTH_BASE_ENV_VARS: [&str; 5] = [
    OP_SERVICE_ACCOUNT_TOKEN_ENV,
    "OP_SESSION",
    "OP_ACCOUNT",
    "OP_CONNECT_HOST",
    "OP_CONNECT_TOKEN",
];

fn is_op_session_env(name: &str) -> bool {
    name == "OP_SESSION" || name.starts_with("OP_SESSION_")
}

fn onepassword_auth_env_var_names() -> Vec<String> {
    let mut names: Vec<String> =
        ONEPASSWORD_AUTH_BASE_ENV_VARS.iter().map(|name| (*name).to_string()).collect();

    names.extend(
        std::env::vars()
            .map(|(name, _)| name)
            .filter(|name| is_op_session_env(name) && name != "OP_SESSION"),
    );

    names.sort_unstable();
    names.dedup();
    names
}

fn sanitize_onepassword_account_suffix(account: &str) -> String {
    account
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() { character } else { '_' })
        .collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct EffectiveOnePasswordConfig {
    mode: Option<OnePasswordMode>,
    service_account_token_path: Option<String>,
}

#[derive(Debug)]
struct ScopedEnvVarChanges {
    originals: Vec<(String, Option<String>)>,
}

impl ScopedEnvVarChanges {
    fn capture(var_names: &[String]) -> Self {
        let originals =
            var_names.iter().map(|name| (name.clone(), std::env::var(name).ok())).collect();

        Self { originals }
    }

    fn set_var(name: &str, value: &str) {
        std::env::set_var(name, value);
    }

    fn remove_var(name: &str) {
        std::env::remove_var(name);
    }
}

impl Drop for ScopedEnvVarChanges {
    fn drop(&mut self) {
        for (name, original_value) in self.originals.iter().rev() {
            match original_value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SecretResolver {
    config: Option<SecretManagerConfig>,
    cache: Arc<Mutex<HashMap<String, String>>>,
    op_ref_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    metrics: Arc<Mutex<SecretResolutionMetrics>>,
}

impl SecretResolver {
    #[must_use]
    pub fn new(config: Option<SecretManagerConfig>) -> Self {
        Self {
            config,
            cache: Arc::new(Mutex::new(HashMap::new())),
            op_ref_locks: Arc::new(Mutex::new(HashMap::new())),
            metrics: Arc::new(Mutex::new(SecretResolutionMetrics::new())),
        }
    }

    /// Resolves environment variables starting with `CLAUDIUS_SECRET_` prefix.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Secret manager resolution fails
    /// - Variable expansion contains circular dependencies
    /// - 1Password CLI is not available when needed
    pub fn resolve_env_vars(&self) -> Result<HashMap<String, String>> {
        let total_timer = Timer::new("Total secret resolution");

        // Phase 1: Collect environment variables
        let claudius_secrets = self.collect_claudius_secrets();
        let _auth_guard = self.prepare_onepassword_environment(&claudius_secrets)?;

        // Phase 1.5: If using 1Password, perform a single preflight resolution so any interactive
        // unlock happens before parallel resolution begins.
        self.preflight_onepassword_unlock(&claudius_secrets);

        // Phase 2: Resolve secrets in parallel
        let resolved_vars = self.resolve_secrets_parallel(&claudius_secrets)?;

        // Phase 3: Expand variable references
        let expanded_vars = Self::expand_variables(resolved_vars)?;

        // Phase 4: Remove prefixes and finalize
        let result = Self::remove_prefixes(expanded_vars);

        self.log_metrics(total_timer.stop());
        Ok(result)
    }

    fn preflight_onepassword_unlock(&self, secrets: &HashMap<String, String>) {
        if !matches!(
            self.config.as_ref().map(|c| c.manager_type),
            Some(SecretManagerType::OnePassword)
        ) {
            return;
        }

        let Some(op_ref) = secrets.values().find_map(|v| Self::extract_first_op_reference(v))
        else {
            return;
        };

        let cache = self.cache.clone();
        let _ = self.resolve_with_cache(&op_ref, &cache);
    }

    fn extract_first_op_reference(value: &str) -> Option<String> {
        if let Some(start_pos) = value.find("{{op://") {
            let op_ref_start = start_pos.saturating_add(2);
            let inline_reference = value
                .get(op_ref_start..)
                .and_then(|search_area| {
                    search_area.find("}}").and_then(|end_pos| search_area.get(..end_pos))
                })
                .map(str::to_string);
            if inline_reference.is_some() {
                return inline_reference;
            }
        }

        let start_pos = value.find("op://")?;
        let op_ref_start = value.get(start_pos..).unwrap_or("");
        let op_ref = Self::extract_op_reference(op_ref_start);
        if op_ref.is_empty() {
            None
        } else {
            Some(op_ref)
        }
    }

    fn collect_claudius_secrets(&self) -> HashMap<String, String> {
        let _timer = Timer::new("Phase 1: Collecting env vars");
        let mut secrets = HashMap::new();

        for (key, value) in std::env::vars() {
            if key.starts_with("CLAUDIUS_SECRET_") {
                secrets.insert(key, value);
            }
        }

        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.total_secrets = secrets.len();
        }

        debug!("Found {} CLAUDIUS_SECRET_* variables", secrets.len());
        secrets
    }

    fn prepare_onepassword_environment(
        &self,
        secrets: &HashMap<String, String>,
    ) -> Result<Option<ScopedEnvVarChanges>> {
        if !Self::contains_onepassword_reference(secrets) {
            return Ok(None);
        }

        let Some(config) = self.effective_onepassword_config()? else {
            return Ok(None);
        };
        let Some(mode) = config.mode else {
            return Ok(None);
        };

        let auth_env_vars = onepassword_auth_env_var_names();
        let existing_service_account_token = Self::read_env_non_empty(OP_SERVICE_ACCOUNT_TOKEN_ENV);
        let guard = ScopedEnvVarChanges::capture(&auth_env_vars);

        match mode {
            OnePasswordMode::Desktop => {
                for name in &auth_env_vars {
                    ScopedEnvVarChanges::remove_var(name);
                }
            },
            OnePasswordMode::Manual => {
                ScopedEnvVarChanges::remove_var(OP_SERVICE_ACCOUNT_TOKEN_ENV);
                ScopedEnvVarChanges::remove_var("OP_CONNECT_HOST");
                ScopedEnvVarChanges::remove_var("OP_CONNECT_TOKEN");

                if !Self::has_manual_onepassword_session() {
                    anyhow::bail!(Self::manual_mode_session_error_message());
                }
            },
            OnePasswordMode::ServiceAccount => {
                for name in &auth_env_vars {
                    ScopedEnvVarChanges::remove_var(name);
                }

                let service_account_token = if let Some(token) = existing_service_account_token {
                    token
                } else if let Some(token_path) = config.service_account_token_path.as_deref() {
                    Self::read_service_account_token(token_path)?
                } else {
                    anyhow::bail!(
                        "1Password service-account mode requires OP_SERVICE_ACCOUNT_TOKEN or a configured service-account-token-path."
                    );
                };

                ScopedEnvVarChanges::set_var(OP_SERVICE_ACCOUNT_TOKEN_ENV, &service_account_token);
            },
        }

        Ok(Some(guard))
    }

    fn effective_onepassword_config(&self) -> Result<Option<EffectiveOnePasswordConfig>> {
        let Some(config) = self.config.as_ref() else {
            return Ok(None);
        };

        if config.manager_type != SecretManagerType::OnePassword {
            return Ok(None);
        }

        let onepassword = config.onepassword.as_ref();
        Ok(Some(EffectiveOnePasswordConfig {
            mode: Self::read_onepassword_mode_override()?
                .or_else(|| onepassword.and_then(|cfg| cfg.mode)),
            service_account_token_path: Self::read_string_override(&[
                ONEPASSWORD_TOKEN_PATH_ENV,
                LEGACY_ONEPASSWORD_TOKEN_PATH_ENV,
            ])
            .or_else(|| onepassword.and_then(|cfg| cfg.service_account_token_path.clone())),
        }))
    }

    fn read_onepassword_mode_override() -> Result<Option<OnePasswordMode>> {
        Self::read_string_override(&[ONEPASSWORD_MODE_ENV, LEGACY_ONEPASSWORD_MODE_ENV])
            .map(|value| {
                value.parse::<OnePasswordMode>().map_err(|error| {
                    anyhow::anyhow!(
                        "Invalid 1Password mode `{value}` from {}: {error}",
                        Self::mode_override_source()
                    )
                })
            })
            .transpose()
    }

    fn mode_override_source() -> &'static str {
        if Self::read_env_non_empty(ONEPASSWORD_MODE_ENV).is_some() {
            ONEPASSWORD_MODE_ENV
        } else {
            LEGACY_ONEPASSWORD_MODE_ENV
        }
    }

    fn read_string_override(names: &[&str]) -> Option<String> {
        names.iter().find_map(|name| Self::read_env_non_empty(name))
    }

    fn read_env_non_empty(name: &str) -> Option<String> {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    fn has_manual_onepassword_session() -> bool {
        Self::manual_mode_session_env_names()
            .iter()
            .any(|name| Self::read_env_non_empty(name).is_some())
    }

    fn manual_mode_session_env_names() -> Vec<String> {
        let mut names = vec!["OP_SESSION".to_string()];

        if let Some(account) = Self::read_env_non_empty("OP_ACCOUNT") {
            names.extend(Self::account_specific_session_env_names(&account));
        } else {
            names.extend(
                std::env::vars()
                    .map(|(name, _)| name)
                    .filter(|name| is_op_session_env(name) && name != "OP_SESSION"),
            );
        }

        names.sort_unstable();
        names.dedup();
        names
    }

    fn account_specific_session_env_names(account: &str) -> Vec<String> {
        let mut names = vec![format!("OP_SESSION_{account}")];
        let sanitized = sanitize_onepassword_account_suffix(account);
        if sanitized != account {
            names.push(format!("OP_SESSION_{sanitized}"));
        }

        names.sort_unstable();
        names.dedup();
        names
    }

    fn manual_mode_session_error_message() -> String {
        let required_envs = Self::manual_mode_session_env_names();
        let requirements = required_envs.join(" or ");

        Self::read_env_non_empty("OP_ACCOUNT").map_or_else(
            || {
                format!(
                    "1Password manual mode requires {requirements} to already be set. Run `op signin` first or choose another mode."
                )
            },
            |account| {
                format!(
                "1Password manual mode requires {requirements} to already be set. OP_ACCOUNT is `{account}`, so the matching session token must be available. Run `op signin` first or choose another mode."
                )
            },
        )
    }

    fn contains_onepassword_reference(secrets: &HashMap<String, String>) -> bool {
        secrets.values().any(|value| value.contains("op://"))
    }

    fn read_service_account_token(path: &str) -> Result<String> {
        let expanded_path = Self::expand_path(path);
        let token = std::fs::read_to_string(&expanded_path).with_context(|| {
            format!(
                "Failed to read 1Password service account token from {}",
                expanded_path.display()
            )
        })?;
        let trimmed = token.trim();

        if trimmed.is_empty() {
            anyhow::bail!(
                "1Password service account token file is empty: {}",
                expanded_path.display()
            );
        }

        Ok(trimmed.to_string())
    }

    fn expand_path(path: &str) -> PathBuf {
        if path == "~" {
            return std::env::var_os("HOME").map_or_else(|| PathBuf::from(path), PathBuf::from);
        }

        if let Some(stripped) = path.strip_prefix("~/") {
            return std::env::var_os("HOME")
                .map(PathBuf::from)
                .map_or_else(|| PathBuf::from(path), |home| home.join(stripped));
        }

        PathBuf::from(path)
    }

    fn resolve_secrets_parallel(
        &self,
        secrets: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        let _timer = Timer::new("Phase 2: Resolving secret references (parallel)");
        let items: Vec<_> = secrets.iter().collect();

        let resolved: Result<Vec<_>> =
            items.par_iter().map(|(k, v)| self.resolve_single_secret(k, v)).collect();

        Ok(resolved?.into_iter().collect())
    }

    fn resolve_single_secret(&self, key: &str, value: &str) -> Result<(String, String)> {
        debug!("Processing key: {}, value: {}", key, value);
        let resolution_result = self.resolve_value(key, value)?;
        debug!("Resolution result for {}: {:?}", key, resolution_result);

        let final_value = resolution_result.unwrap_or_else(|| {
            debug!("No resolution for {}, keeping original", key);
            value.to_string()
        });

        Ok((key.to_string(), final_value))
    }

    fn expand_variables(variables: HashMap<String, String>) -> Result<HashMap<String, String>> {
        let _timer = Timer::new("Phase 3: Variable expansion");
        expand_variables(variables, &HashMap::new())
    }

    fn remove_prefixes(variables: HashMap<String, String>) -> HashMap<String, String> {
        let _timer = Timer::new("Phase 4: Removing prefixes");
        let mut result = HashMap::new();

        for (key, value) in variables {
            let new_key = key.strip_prefix("CLAUDIUS_SECRET_").unwrap_or(&key);
            result.insert(new_key.to_string(), value);
        }

        result
    }

    fn log_metrics(&self, duration: std::time::Duration) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.total_duration = duration;
            if std::env::var("CLAUDIUS_PROFILE").is_ok() {
                metrics.log_summary();
            }
        }
    }

    fn resolve_value(&self, key: &str, value: &str) -> Result<Option<String>> {
        debug!("resolve_value called for key: {}, value: {}", key, value);

        self.config.as_ref().map_or_else(|| {
            debug!("No secret manager configured");
            // No secret manager configured, return None to indicate no change
            Ok(None)
        }, |config| match config.manager_type {
            SecretManagerType::Vault => {
                warn!("Vault secret manager is configured but not yet implemented. Skipping resolution for {}", key);
                Ok(None)
            },
            SecretManagerType::OnePassword => {
                // Check if value contains any op:// references
                if value.contains("op://") {
                    debug!("Found op:// references in value, resolving...");
                    Ok(Some(self.resolve_inline_op_references(value)))
                } else {
                    debug!("No op:// references found in value");
                    // No op:// references found, return None to indicate no change
                    Ok(None)
                }
            },
        })
    }

    fn resolve_inline_op_references(&self, value: &str) -> String {
        debug!("Resolving inline op:// references in: {}", value);

        // First pass: resolve {{op://...}} references (unambiguous)
        let mut result = self.resolve_delimited_references(value);

        // Second pass: resolve bare op:// references (for backward compatibility)
        result = self.resolve_bare_references(&result);

        debug!("Final resolved value: {}", result);
        result
    }

    fn process_delimited_reference<'a>(
        &self,
        remaining: &'a str,
        start_pos: usize,
        cache: &Arc<Mutex<HashMap<String, String>>>,
    ) -> (String, &'a str) {
        let delimiter_start = start_pos.saturating_add(2); // Skip {{
        let search_area = remaining.get(delimiter_start..).unwrap_or("");

        if let Some(end_pos) = search_area.find("}}") {
            let op_ref_end = delimiter_start.saturating_add(end_pos);
            if let Some(op_ref) = remaining.get(delimiter_start..op_ref_end) {
                debug!("Found delimited op:// reference: {}", op_ref);
                let resolved = self.resolve_with_cache(op_ref, cache);
                let new_start = op_ref_end.saturating_add(2);
                let new_remaining = remaining.get(new_start..).unwrap_or("");
                return (resolved, new_remaining);
            }
        }

        // No closing delimiter found or invalid reference
        warn!("Unclosed delimiter at position {}", start_pos);
        let new_start = start_pos.saturating_add(7);
        let new_remaining = remaining.get(new_start..).unwrap_or("");
        ("{{op://".to_string(), new_remaining)
    }

    fn resolve_delimited_references(&self, value: &str) -> String {
        let mut result = String::new();
        let mut remaining = value;
        let cache = self.cache.clone();

        // Look for {{op://...}} patterns
        while let Some(start_pos) = remaining.find("{{op://") {
            // Add everything before the delimiter
            if let Some(prefix) = remaining.get(..start_pos) {
                result.push_str(prefix);
            }

            // Process the delimited reference
            let (resolved, new_remaining) =
                self.process_delimited_reference(remaining, start_pos, &cache);
            result.push_str(&resolved);
            remaining = new_remaining;
        }

        // Add any remaining text
        result.push_str(remaining);
        result
    }

    fn resolve_bare_references(&self, value: &str) -> String {
        let mut result = String::new();
        let mut remaining = value;
        let cache = self.cache.clone();

        while let Some(start_pos) = remaining.find("op://") {
            // Add everything before the op:// reference
            if let Some(prefix) = remaining.get(..start_pos) {
                result.push_str(prefix);
            }

            // Check if this looks like it's in a URL context
            let is_url_context = start_pos > 0
                && remaining.get(..start_pos).is_some_and(|s| s.ends_with('/') || s.ends_with('='));

            // Extract the op:// reference
            let op_ref_start = remaining.get(start_pos..).unwrap_or("");
            let op_ref = Self::extract_op_reference(op_ref_start);

            if is_url_context && op_ref.contains(' ') {
                warn!(
                    "Ambiguous op:// reference in URL context: '{}'. Consider using {{{{op://...}}}} syntax for clarity.",
                    op_ref
                );
            }

            debug!("Found bare op:// reference: {}", op_ref);

            let resolved = self.resolve_with_cache(&op_ref, &cache);
            result.push_str(&resolved);
            let new_start = start_pos.saturating_add(op_ref.len());
            remaining = remaining.get(new_start..).unwrap_or("");
        }

        // Add any remaining text
        result.push_str(remaining);
        result
    }

    fn resolve_with_cache(
        &self,
        op_ref: &str,
        cache: &Arc<Mutex<HashMap<String, String>>>,
    ) -> String {
        // Check cache first
        let cached = cache.lock().map_or(None, |cache_guard| cache_guard.get(op_ref).cloned());

        if let Some(cached_value) = cached {
            debug!("Using cached value for {}", op_ref);
            return cached_value;
        }

        let op_ref_lock = {
            let mut locks =
                self.op_ref_locks.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            locks
                .entry(op_ref.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _guard = op_ref_lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner);

        // Re-check cache after waiting for any in-flight resolution of the same reference.
        let cached_after_wait =
            cache.lock().map_or(None, |cache_guard| cache_guard.get(op_ref).cloned());
        if let Some(cached_value) = cached_after_wait {
            debug!("Using cached value for {}", op_ref);
            return cached_value;
        }

        // Resolve the reference
        let _op_timer = Timer::new(&format!("op read {op_ref}"));
        let start_time = std::time::Instant::now();

        match Self::resolve_onepassword_reference(op_ref) {
            Ok(secret) => {
                let duration = start_time.elapsed();
                debug!("Resolved {} to {} in {:?}", op_ref, secret, duration);

                // Update metrics
                if let Ok(mut metrics) = self.metrics.lock() {
                    metrics.add_op_call(op_ref.to_string(), duration, true);
                }

                // Cache the resolved value
                if let Ok(mut cache_guard) = cache.lock() {
                    cache_guard.insert(op_ref.to_string(), secret.clone());
                }
                secret
            },
            Err(e) => {
                let duration = start_time.elapsed();
                warn!("Failed to resolve {} in {:?}: {}", op_ref, duration, e);

                // Update metrics
                if let Ok(mut metrics) = self.metrics.lock() {
                    metrics.add_op_call(op_ref.to_string(), duration, false);
                }

                op_ref.to_string() // Keep original reference on failure
            },
        }
    }

    fn extract_op_reference(text: &str) -> String {
        // Extract an op:// reference from the beginning of the text
        // Format: op://vault/item/field or op://vault/item/section/field

        if !text.starts_with("op://") {
            return String::new();
        }

        // Check for space-terminated reference
        if let Some(ref_until_space) = Self::extract_until_space(text) {
            return ref_until_space;
        }

        let parts: Vec<&str> = text.split('/').collect();

        // Need at least 5 parts: "op:", "", "vault", "item", "field"
        if parts.len() < 5 {
            return text.to_string();
        }

        // Standard format: op://vault/item/field
        if parts.len() == 5 {
            return text.to_string();
        }

        // Check for nested op:// reference
        if let Some(nested_end) = Self::find_nested_op_reference(&parts) {
            return parts
                .get(..nested_end)
                .map_or_else(|| text.to_string(), |slice| slice.join("/"));
        }

        // Check if next segment looks like URL path
        if Self::is_url_path_component(parts.get(5)) {
            return parts.get(..5).map_or_else(|| text.to_string(), |slice| slice.join("/"));
        }

        // Default: return everything
        text.to_string()
    }

    /// Extract reference until first space
    fn extract_until_space(text: &str) -> Option<String> {
        text.find(' ').and_then(|space_pos| {
            text.get(..space_pos).and_then(|before_space| {
                if before_space.split('/').count() >= 5 {
                    Some(before_space.to_string())
                } else {
                    None
                }
            })
        })
    }

    /// Find nested op:// reference in parts
    fn find_nested_op_reference(parts: &[&str]) -> Option<usize> {
        for i in 5..parts.len() {
            if let Some(part) = parts.get(i) {
                if *part == "op:" && parts.get(i.saturating_add(1)).is_some_and(|p| p.is_empty()) {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Check if a string looks like a URL path component
    fn is_url_path_component(part: Option<&&str>) -> bool {
        part.is_some_and(|p| {
            p.chars().all(|c| c.is_ascii_lowercase() || c == '-' || c == '_')
                && !p.is_empty()
                && !p.contains(' ')
        })
    }

    fn resolve_onepassword_reference(reference: &str) -> Result<String> {
        debug!("Resolving 1Password reference: {}", reference);

        // Always use mock in test builds
        #[cfg(test)]
        return mock_op_read(reference);

        // Check for mock mode in non-test builds (for integration testing)
        #[cfg(not(test))]
        if std::env::var("CLAUDIUS_TEST_MOCK_OP").is_ok() {
            return mock_op_read(reference);
        }

        // Check if `op` command is available
        #[cfg(not(test))]
        {
            let op_check = Command::new("op").arg("--version").output();

            if op_check.is_err() {
                anyhow::bail!("1Password CLI (op) is not installed or not in PATH");
            }

            // Use `op read` to resolve the reference
            let output = Command::new("op")
                .arg("read")
                .arg(reference)
                .output()
                .context("Failed to execute 1Password CLI")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("1Password CLI failed: {stderr}");
            }

            let resolved = String::from_utf8(output.stdout)
                .context("Failed to parse 1Password output")?
                .trim()
                .to_string();

            Ok(resolved)
        }
    }

    pub fn inject_env_vars(env_vars: HashMap<String, String>) {
        for (key, value) in env_vars {
            std::env::set_var(key, value);
        }
    }

    /// Get a copy of the current metrics for analysis
    #[must_use]
    pub fn get_metrics(&self) -> Option<SecretResolutionMetrics> {
        self.metrics.lock().ok().map(|metrics| SecretResolutionMetrics {
            total_secrets: metrics.total_secrets,
            successful_resolutions: metrics.successful_resolutions,
            failed_resolutions: metrics.failed_resolutions,
            op_calls: metrics.op_calls.clone(),
            total_duration: metrics.total_duration,
        })
    }
}

// Mock function available in all builds when CLAUDIUS_TEST_MOCK_OP is set
fn mock_op_read(reference: &str) -> Result<String> {
    match reference {
        "op://vault/test-item/api-key" => Ok("secret-api-key-12345".to_string()),
        "op://vault/database/password" => Ok("db-password-xyz789".to_string()),
        "op://Private/CLOUDFLARE_AI_Gateway/Account_ID"
        | "op://Private/CLOUDFLARE AI Gateway/Account ID" => Ok("cf-account-12345".to_string()),
        "op://Private/CLOUDFLARE_AI_Gateway/Gateway_ID"
        | "op://Private/CLOUDFLARE AI Gateway/Gateway ID" => Ok("cf-gateway-67890".to_string()),
        "op://Private/CLOUDFLARE_AI_Gateway/credential"
        | "op://Private/CLOUDFLARE AI Gateway/credential" => Ok("cf-credential-secret".to_string()),
        // Support parallel performance test references
        "op://vault/item1/field1" => Ok("secret-value-1".to_string()),
        "op://vault/item2/field2" => Ok("secret-value-2".to_string()),
        "op://vault/item3/field3" => Ok("secret-value-3".to_string()),
        "op://vault/item4/field4" => Ok("secret-value-4".to_string()),
        "op://vault/item5/field5" => Ok("secret-value-5".to_string()),
        "op://invalid/reference/field" => {
            anyhow::bail!("1Password CLI failed: ERROR: Item not found")
        },
        _ => anyhow::bail!("1Password CLI failed: ERROR: Unknown reference"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::TempDir;

    // Mutex to ensure tests that modify environment variables run one at a time
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    // Helper function to clean up all CLAUDIUS_SECRET_* environment variables
    fn cleanup_claudius_secrets() {
        let vars_to_remove: Vec<String> = std::env::vars()
            .filter(|(key, _)| key.starts_with("CLAUDIUS_SECRET_"))
            .map(|(key, _)| key)
            .collect();

        for key in vars_to_remove {
            std::env::remove_var(&key);
        }
    }

    fn cleanup_onepassword_env() {
        let mut vars_to_remove = vec![
            ONEPASSWORD_MODE_ENV.to_string(),
            LEGACY_ONEPASSWORD_MODE_ENV.to_string(),
            ONEPASSWORD_TOKEN_PATH_ENV.to_string(),
            LEGACY_ONEPASSWORD_TOKEN_PATH_ENV.to_string(),
            OP_SERVICE_ACCOUNT_TOKEN_ENV.to_string(),
            "OP_SESSION".to_string(),
            "OP_ACCOUNT".to_string(),
            "OP_CONNECT_HOST".to_string(),
            "OP_CONNECT_TOKEN".to_string(),
            "HOME".to_string(),
        ];

        vars_to_remove.extend(
            std::env::vars()
                .map(|(name, _)| name)
                .filter(|name| is_op_session_env(name) && name != "OP_SESSION"),
        );

        vars_to_remove.sort_unstable();
        vars_to_remove.dedup();

        for name in vars_to_remove {
            std::env::remove_var(name);
        }
    }

    #[test]
    fn test_secret_resolver_new() {
        let resolver = SecretResolver::new(None);
        assert!(resolver.config.is_none());

        let config =
            SecretManagerConfig { manager_type: SecretManagerType::Vault, onepassword: None };
        let resolver_with_config = SecretResolver::new(Some(config));
        assert!(resolver_with_config.config.is_some());
        assert_eq!(
            resolver_with_config
                .config
                .as_ref()
                .expect("Config should be present")
                .manager_type,
            SecretManagerType::Vault
        );
    }

    #[test]
    #[serial]
    fn test_resolve_env_vars_no_secrets() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        // Clear any existing CLAUDIUS_SECRET_* vars
        cleanup_claudius_secrets();

        let resolver = SecretResolver::new(None);
        let resolved = resolver.resolve_env_vars().expect("resolve_env_vars should succeed");
        assert!(resolved.is_empty());
    }

    #[test]
    #[serial]
    fn test_resolve_env_vars_with_plain_secrets() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        // Clear any existing CLAUDIUS_SECRET_* vars
        cleanup_claudius_secrets();

        std::env::set_var("CLAUDIUS_SECRET_API_KEY", "my-api-key");
        std::env::set_var("CLAUDIUS_SECRET_DB_PASSWORD", "secret123");
        std::env::set_var("REGULAR_VAR", "should-not-be-included");

        let resolver = SecretResolver::new(None);
        let resolved = resolver.resolve_env_vars().expect("resolve_env_vars should succeed");

        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved.get("API_KEY"), Some(&"my-api-key".to_string()));
        assert_eq!(resolved.get("DB_PASSWORD"), Some(&"secret123".to_string()));
        assert!(!resolved.contains_key("REGULAR_VAR"));

        // Cleanup
        std::env::remove_var("CLAUDIUS_SECRET_API_KEY");
        std::env::remove_var("CLAUDIUS_SECRET_DB_PASSWORD");
        std::env::remove_var("REGULAR_VAR");
    }

    #[test]
    #[serial]
    fn test_resolve_env_vars_strip_prefix() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        // Clear any existing CLAUDIUS_SECRET_* vars
        cleanup_claudius_secrets();

        std::env::set_var("CLAUDIUS_SECRET_GITHUB_TOKEN", "ghp_123456");

        let resolver = SecretResolver::new(None);
        let resolved = resolver.resolve_env_vars().expect("resolve_env_vars should succeed");

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved.get("GITHUB_TOKEN"), Some(&"ghp_123456".to_string()));
        assert!(!resolved.contains_key("CLAUDIUS_SECRET_GITHUB_TOKEN"));

        // Cleanup
        std::env::remove_var("CLAUDIUS_SECRET_GITHUB_TOKEN");
    }

    #[test]
    fn test_resolve_value_no_config() {
        let resolver = SecretResolver::new(None);

        let result = resolver
            .resolve_value("CLAUDIUS_SECRET_KEY", "plain-value")
            .expect("resolve_value should succeed");
        assert_eq!(result, None); // No secret manager configured, so no resolution
    }

    #[test]
    fn test_resolve_value_vault_config() {
        let config =
            SecretManagerConfig { manager_type: SecretManagerType::Vault, onepassword: None };
        let resolver = SecretResolver::new(Some(config));

        let result = resolver
            .resolve_value("CLAUDIUS_SECRET_KEY", "vault://secret/data")
            .expect("resolve_value should succeed with vault config");
        assert!(result.is_none()); // Vault not implemented yet
    }

    #[test]
    fn test_resolve_value_onepassword_non_reference() {
        let config =
            SecretManagerConfig { manager_type: SecretManagerType::OnePassword, onepassword: None };
        let resolver = SecretResolver::new(Some(config));

        let result = resolver
            .resolve_value("CLAUDIUS_SECRET_KEY", "plain-value")
            .expect("resolve_value should succeed");
        assert_eq!(result, None); // No op:// references, so no resolution needed
    }

    #[test]
    #[serial]
    fn test_manual_mode_requires_existing_session() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        cleanup_claudius_secrets();
        cleanup_onepassword_env();

        std::env::set_var("CLAUDIUS_TEST_MOCK_OP", "1");
        std::env::set_var("CLAUDIUS_SECRET_API_KEY", "op://vault/test-item/api-key");

        let config = SecretManagerConfig {
            manager_type: SecretManagerType::OnePassword,
            onepassword: Some(OnePasswordConfig {
                mode: Some(OnePasswordMode::Manual),
                service_account_token_path: None,
            }),
        };
        let resolver = SecretResolver::new(Some(config));

        let error = resolver.resolve_env_vars().expect_err("manual mode should require OP_SESSION");
        assert!(error.to_string().contains("requires OP_SESSION"));

        std::env::remove_var("CLAUDIUS_TEST_MOCK_OP");
        std::env::remove_var("CLAUDIUS_SECRET_API_KEY");
        cleanup_onepassword_env();
    }

    #[test]
    #[serial]
    fn test_manual_mode_accepts_account_specific_session_env() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        cleanup_claudius_secrets();
        cleanup_onepassword_env();

        std::env::set_var("CLAUDIUS_TEST_MOCK_OP", "1");
        std::env::set_var("CLAUDIUS_SECRET_API_KEY", "op://vault/test-item/api-key");
        std::env::set_var("OP_ACCOUNT", "my");
        std::env::set_var("OP_SESSION_my", "session-token");

        let config = SecretManagerConfig {
            manager_type: SecretManagerType::OnePassword,
            onepassword: Some(OnePasswordConfig {
                mode: Some(OnePasswordMode::Manual),
                service_account_token_path: None,
            }),
        };
        let resolver = SecretResolver::new(Some(config));

        let resolved = resolver.resolve_env_vars().expect("manual mode should accept OP_SESSION_*");
        assert_eq!(resolved.get("API_KEY"), Some(&"secret-api-key-12345".to_string()));

        std::env::remove_var("CLAUDIUS_TEST_MOCK_OP");
        std::env::remove_var("CLAUDIUS_SECRET_API_KEY");
        cleanup_onepassword_env();
    }

    #[test]
    #[serial]
    fn test_manual_mode_rejects_mismatched_account_specific_session_env() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        cleanup_claudius_secrets();
        cleanup_onepassword_env();

        std::env::set_var("CLAUDIUS_TEST_MOCK_OP", "1");
        std::env::set_var("CLAUDIUS_SECRET_API_KEY", "op://vault/test-item/api-key");
        std::env::set_var("OP_ACCOUNT", "my");
        std::env::set_var("OP_SESSION_other", "session-token");

        let config = SecretManagerConfig {
            manager_type: SecretManagerType::OnePassword,
            onepassword: Some(OnePasswordConfig {
                mode: Some(OnePasswordMode::Manual),
                service_account_token_path: None,
            }),
        };
        let resolver = SecretResolver::new(Some(config));

        let error = resolver
            .resolve_env_vars()
            .expect_err("manual mode should reject mismatched OP_SESSION_<account>");
        assert!(error.to_string().contains("OP_SESSION_my"));

        std::env::remove_var("CLAUDIUS_TEST_MOCK_OP");
        std::env::remove_var("CLAUDIUS_SECRET_API_KEY");
        cleanup_onepassword_env();
    }

    #[test]
    #[serial]
    fn test_service_account_mode_reads_token_file_and_restores_env() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        cleanup_claudius_secrets();
        cleanup_onepassword_env();

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let token_path = temp_dir.path().join("service-account.token");
        std::fs::write(&token_path, "service-account-token-123\n")
            .expect("Failed to write service account token");

        std::env::set_var("CLAUDIUS_TEST_MOCK_OP", "1");
        std::env::set_var("CLAUDIUS_SECRET_API_KEY", "op://vault/test-item/api-key");
        std::env::set_var("OP_SESSION", "session-before");
        std::env::set_var("OP_SESSION_my", "session-before-account");
        std::env::set_var("OP_ACCOUNT", "account-before");

        let config = SecretManagerConfig {
            manager_type: SecretManagerType::OnePassword,
            onepassword: Some(OnePasswordConfig {
                mode: Some(OnePasswordMode::ServiceAccount),
                service_account_token_path: Some(token_path.to_string_lossy().to_string()),
            }),
        };
        let resolver = SecretResolver::new(Some(config));

        let resolved = resolver.resolve_env_vars().expect("service-account mode should resolve");
        assert_eq!(resolved.get("API_KEY"), Some(&"secret-api-key-12345".to_string()));

        assert_eq!(std::env::var("OP_SESSION").ok().as_deref(), Some("session-before"));
        assert_eq!(std::env::var("OP_SESSION_my").ok().as_deref(), Some("session-before-account"));
        assert_eq!(std::env::var("OP_ACCOUNT").ok().as_deref(), Some("account-before"));
        assert!(std::env::var(OP_SERVICE_ACCOUNT_TOKEN_ENV).is_err());

        std::env::remove_var("CLAUDIUS_TEST_MOCK_OP");
        std::env::remove_var("CLAUDIUS_SECRET_API_KEY");
        cleanup_onepassword_env();
    }

    #[test]
    #[serial]
    fn test_onepassword_mode_override_takes_precedence() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        cleanup_claudius_secrets();
        cleanup_onepassword_env();

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let token_path = temp_dir.path().join("service-account.token");
        std::fs::write(&token_path, "service-account-token-override\n")
            .expect("Failed to write service account token");

        std::env::set_var("CLAUDIUS_TEST_MOCK_OP", "1");
        std::env::set_var("CLAUDIUS_SECRET_API_KEY", "op://vault/test-item/api-key");
        std::env::set_var(ONEPASSWORD_MODE_ENV, "service-account");
        std::env::set_var(ONEPASSWORD_TOKEN_PATH_ENV, token_path);

        let config = SecretManagerConfig {
            manager_type: SecretManagerType::OnePassword,
            onepassword: Some(OnePasswordConfig {
                mode: Some(OnePasswordMode::Manual),
                service_account_token_path: None,
            }),
        };
        let resolver = SecretResolver::new(Some(config));

        let resolved = resolver.resolve_env_vars().expect("env override should win over config");
        assert_eq!(resolved.get("API_KEY"), Some(&"secret-api-key-12345".to_string()));

        std::env::remove_var("CLAUDIUS_TEST_MOCK_OP");
        std::env::remove_var("CLAUDIUS_SECRET_API_KEY");
        cleanup_onepassword_env();
    }

    #[test]
    #[serial]
    fn test_invalid_onepassword_mode_override_errors() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        cleanup_claudius_secrets();
        cleanup_onepassword_env();

        std::env::set_var("CLAUDIUS_SECRET_API_KEY", "op://vault/test-item/api-key");
        std::env::set_var(ONEPASSWORD_MODE_ENV, "unsupported");

        let config =
            SecretManagerConfig { manager_type: SecretManagerType::OnePassword, onepassword: None };
        let resolver = SecretResolver::new(Some(config));

        let error = resolver.resolve_env_vars().expect_err("invalid mode override should fail");
        assert!(error.to_string().contains("Invalid 1Password mode"));

        std::env::remove_var("CLAUDIUS_SECRET_API_KEY");
        cleanup_onepassword_env();
    }

    #[test]
    #[serial]
    fn test_inject_env_vars() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        let mut env_vars = HashMap::new();
        env_vars.insert("TEST_VAR_1".to_string(), "value1".to_string());
        env_vars.insert("TEST_VAR_2".to_string(), "value2".to_string());

        SecretResolver::inject_env_vars(env_vars);

        assert_eq!(std::env::var("TEST_VAR_1").expect("TEST_VAR_1 should be set"), "value1");
        assert_eq!(std::env::var("TEST_VAR_2").expect("TEST_VAR_2 should be set"), "value2");

        // Cleanup
        std::env::remove_var("TEST_VAR_1");
        std::env::remove_var("TEST_VAR_2");
    }

    #[test]
    #[serial]
    fn test_inject_env_vars_overwrite() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        std::env::set_var("EXISTING_VAR", "old-value");

        let mut env_vars = HashMap::new();
        env_vars.insert("EXISTING_VAR".to_string(), "new-value".to_string());

        SecretResolver::inject_env_vars(env_vars);

        assert_eq!(std::env::var("EXISTING_VAR").expect("EXISTING_VAR should be set"), "new-value");

        // Cleanup
        std::env::remove_var("EXISTING_VAR");
    }

    #[test]
    #[serial]
    fn test_full_flow_no_config() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        // Clear any existing CLAUDIUS_SECRET_* vars
        cleanup_claudius_secrets();

        // Setup
        std::env::set_var("CLAUDIUS_SECRET_TOKEN", "abc123");
        std::env::set_var("CLAUDIUS_SECRET_API_KEY", "xyz789");

        // Resolve
        let resolver = SecretResolver::new(None);
        let resolved = resolver.resolve_env_vars().expect("resolve_env_vars should succeed");

        // Verify resolution
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved.get("TOKEN"), Some(&"abc123".to_string()));
        assert_eq!(resolved.get("API_KEY"), Some(&"xyz789".to_string()));

        // Inject
        SecretResolver::inject_env_vars(resolved);

        // Verify injection
        assert_eq!(std::env::var("TOKEN").expect("TOKEN should be set"), "abc123");
        assert_eq!(std::env::var("API_KEY").expect("API_KEY should be set"), "xyz789");

        // Cleanup
        std::env::remove_var("CLAUDIUS_SECRET_TOKEN");
        std::env::remove_var("CLAUDIUS_SECRET_API_KEY");
        std::env::remove_var("TOKEN");
        std::env::remove_var("API_KEY");
    }

    // Note: Testing resolve_onepassword_reference would require mocking the Command
    // execution, which is complex. In a real-world scenario, you might use a trait
    // for command execution that can be mocked in tests.

    #[test]
    #[serial]
    fn test_onepassword_mock_integration() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        // Clear any existing CLAUDIUS_SECRET_* vars
        cleanup_claudius_secrets();

        // Enable mock mode
        std::env::set_var("CLAUDIUS_TEST_MOCK_OP", "1");

        let config =
            SecretManagerConfig { manager_type: SecretManagerType::OnePassword, onepassword: None };
        let resolver = SecretResolver::new(Some(config));

        // Set op:// references
        std::env::set_var("CLAUDIUS_SECRET_API_KEY", "op://vault/test-item/api-key");
        std::env::set_var("CLAUDIUS_SECRET_DB_PASSWORD", "op://vault/database/password");

        let resolved = resolver.resolve_env_vars().expect("resolve_env_vars should succeed");
        assert_eq!(resolved.get("API_KEY"), Some(&"secret-api-key-12345".to_string()));
        assert_eq!(resolved.get("DB_PASSWORD"), Some(&"db-password-xyz789".to_string()));

        // Cleanup
        std::env::remove_var("CLAUDIUS_TEST_MOCK_OP");
        std::env::remove_var("CLAUDIUS_SECRET_API_KEY");
        std::env::remove_var("CLAUDIUS_SECRET_DB_PASSWORD");
    }

    #[test]
    #[serial]
    fn test_onepassword_preflight_warms_cache_and_dedupes_calls() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        cleanup_claudius_secrets();

        // Enable mock mode
        std::env::set_var("CLAUDIUS_TEST_MOCK_OP", "1");

        std::env::set_var("CLAUDIUS_SECRET_A", "op://vault/item1/field1");
        std::env::set_var("CLAUDIUS_SECRET_B", "op://vault/item1/field1");

        let config =
            SecretManagerConfig { manager_type: SecretManagerType::OnePassword, onepassword: None };
        let resolver = SecretResolver::new(Some(config));

        let resolved = resolver.resolve_env_vars().expect("resolve_env_vars should succeed");
        assert_eq!(resolved.get("A"), Some(&"secret-value-1".to_string()));
        assert_eq!(resolved.get("B"), Some(&"secret-value-1".to_string()));

        let metrics = resolver
            .get_metrics()
            .expect("metrics should be available after resolve_env_vars");
        assert_eq!(metrics.op_calls.len(), 1);

        std::env::remove_var("CLAUDIUS_TEST_MOCK_OP");
        std::env::remove_var("CLAUDIUS_SECRET_A");
        std::env::remove_var("CLAUDIUS_SECRET_B");
    }

    #[test]
    #[serial]
    fn test_onepassword_mock_error() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        // Clear any existing CLAUDIUS_SECRET_* vars
        cleanup_claudius_secrets();

        // Enable mock mode
        std::env::set_var("CLAUDIUS_TEST_MOCK_OP", "1");

        let config =
            SecretManagerConfig { manager_type: SecretManagerType::OnePassword, onepassword: None };
        let resolver = SecretResolver::new(Some(config));

        // Set invalid reference (must have 3 segments to match regex)
        std::env::set_var("CLAUDIUS_SECRET_INVALID", "op://invalid/reference/field");

        // The resolver keeps the original reference on failure (resilient behavior)
        let result = resolver
            .resolve_env_vars()
            .expect("resolve_env_vars should succeed for mock error test");
        assert_eq!(result.get("INVALID"), Some(&"op://invalid/reference/field".to_string()));

        // Cleanup
        std::env::remove_var("CLAUDIUS_TEST_MOCK_OP");
        std::env::remove_var("CLAUDIUS_SECRET_INVALID");
    }

    #[test]
    #[serial]
    fn test_inline_op_references() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        // Clear any existing CLAUDIUS_SECRET_* vars
        cleanup_claudius_secrets();

        // Enable mock mode
        std::env::set_var("CLAUDIUS_TEST_MOCK_OP", "1");

        let config =
            SecretManagerConfig { manager_type: SecretManagerType::OnePassword, onepassword: None };
        let resolver = SecretResolver::new(Some(config));

        // Set up inline op:// references (Cloudflare AI Gateway example)
        // Using underscores instead of spaces for 1Password references
        std::env::set_var(
            "CLAUDIUS_SECRET_BASE_URL",
            "https://gateway.ai.cloudflare.com/v1/op://Private/CLOUDFLARE_AI_Gateway/Account_ID/op://Private/CLOUDFLARE_AI_Gateway/Gateway_ID/anthropic"
        );
        std::env::set_var(
            "CLAUDIUS_SECRET_HEADERS",
            "cf-aig-authorization: Bearer op://Private/CLOUDFLARE_AI_Gateway/credential",
        );

        let resolved = resolver.resolve_env_vars().expect("resolve_env_vars should succeed");

        // Debug output

        assert_eq!(
            resolved.get("BASE_URL"),
            Some(
                &"https://gateway.ai.cloudflare.com/v1/cf-account-12345/cf-gateway-67890/anthropic"
                    .to_string()
            )
        );
        assert_eq!(
            resolved.get("HEADERS"),
            Some(&"cf-aig-authorization: Bearer cf-credential-secret".to_string())
        );

        // Cleanup
        std::env::remove_var("CLAUDIUS_TEST_MOCK_OP");
        std::env::remove_var("CLAUDIUS_SECRET_BASE_URL");
        std::env::remove_var("CLAUDIUS_SECRET_HEADERS");
    }

    #[test]
    #[serial]
    fn test_mixed_references() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        // Clear any existing CLAUDIUS_SECRET_* vars
        cleanup_claudius_secrets();

        // Enable mock mode
        std::env::set_var("CLAUDIUS_TEST_MOCK_OP", "1");

        let config =
            SecretManagerConfig { manager_type: SecretManagerType::OnePassword, onepassword: None };
        let resolver = SecretResolver::new(Some(config));

        // Mix of inline op:// and variable references
        std::env::set_var("CLAUDIUS_SECRET_ACCOUNT_ID", "cf-account-12345");
        std::env::set_var(
            "CLAUDIUS_SECRET_URL",
            "https://gateway.ai.cloudflare.com/v1/$CLAUDIUS_SECRET_ACCOUNT_ID/op://Private/CLOUDFLARE_AI_Gateway/Gateway_ID/anthropic"
        );

        let resolved = resolver.resolve_env_vars().expect("resolve_env_vars should succeed");

        assert_eq!(
            resolved.get("URL"),
            Some(
                &"https://gateway.ai.cloudflare.com/v1/cf-account-12345/cf-gateway-67890/anthropic"
                    .to_string()
            )
        );

        // Cleanup
        std::env::remove_var("CLAUDIUS_TEST_MOCK_OP");
        std::env::remove_var("CLAUDIUS_SECRET_ACCOUNT_ID");
        std::env::remove_var("CLAUDIUS_SECRET_URL");
    }

    #[test]
    fn test_extract_op_reference() {
        // Test with spaces in the reference
        let text_spaces = "op://Private/CLOUDFLARE AI Gateway/Account ID/anthropic";
        let extracted_spaces = SecretResolver::extract_op_reference(text_spaces);
        assert_eq!(extracted_spaces, "op://Private/CLOUDFLARE AI Gateway/Account ID");

        // Test with underscores
        let text_underscores = "op://Private/CLOUDFLARE_AI_Gateway/Account_ID/anthropic";
        let extracted_underscores = SecretResolver::extract_op_reference(text_underscores);
        assert_eq!(extracted_underscores, "op://Private/CLOUDFLARE_AI_Gateway/Account_ID");

        // Test simple reference
        let text_simple = "op://vault/item/field";
        let extracted_simple = SecretResolver::extract_op_reference(text_simple);
        assert_eq!(extracted_simple, "op://vault/item/field");

        // Test with multiple op:// references
        let text_multiple = "op://vault/item/field op://another/ref/here";
        let extracted_multiple = SecretResolver::extract_op_reference(text_multiple);
        assert_eq!(extracted_multiple, "op://vault/item/field");

        // Test with concatenated op:// references (URL case)
        let text_concat = "op://Private/CLOUDFLARE AI Gateway/Account ID/op://Private/CLOUDFLARE AI Gateway/Gateway ID";
        let extracted_concat = SecretResolver::extract_op_reference(text_concat);
        assert_eq!(extracted_concat, "op://Private/CLOUDFLARE AI Gateway/Account ID");
    }

    #[test]
    #[serial]
    fn test_cache_functionality() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        // Clear any existing CLAUDIUS_SECRET_* vars
        cleanup_claudius_secrets();

        // Enable mock mode
        std::env::set_var("CLAUDIUS_TEST_MOCK_OP", "1");

        let config =
            SecretManagerConfig { manager_type: SecretManagerType::OnePassword, onepassword: None };
        let resolver = SecretResolver::new(Some(config));

        // Set value with duplicate op:// references
        std::env::set_var(
            "CLAUDIUS_SECRET_DUPLICATE",
            "op://vault/test-item/api-key and op://vault/test-item/api-key again",
        );

        let resolved = resolver
            .resolve_env_vars()
            .expect("resolve_env_vars should succeed for cache test");

        // Both references should resolve to the same value (cached on second access)
        assert_eq!(
            resolved.get("DUPLICATE"),
            Some(&"secret-api-key-12345 and secret-api-key-12345 again".to_string())
        );

        // Cleanup
        std::env::remove_var("CLAUDIUS_TEST_MOCK_OP");
        std::env::remove_var("CLAUDIUS_SECRET_DUPLICATE");
    }

    #[test]
    #[serial]
    fn test_delimited_references() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        // Clear any existing CLAUDIUS_SECRET_* vars
        cleanup_claudius_secrets();

        // Enable mock mode
        std::env::set_var("CLAUDIUS_TEST_MOCK_OP", "1");

        let config =
            SecretManagerConfig { manager_type: SecretManagerType::OnePassword, onepassword: None };
        let resolver = SecretResolver::new(Some(config));

        // Test with delimited references in URL
        std::env::set_var(
            "CLAUDIUS_SECRET_URL",
            "https://api.example.com/v1/{{op://vault/test-item/api-key}}/{{op://vault/database/password}}/endpoint",
        );

        let resolved = resolver.resolve_env_vars().expect("resolve_env_vars should succeed");
        assert_eq!(
            resolved.get("URL"),
            Some(
                &"https://api.example.com/v1/secret-api-key-12345/db-password-xyz789/endpoint"
                    .to_string()
            )
        );

        // Cleanup
        std::env::remove_var("CLAUDIUS_TEST_MOCK_OP");
        std::env::remove_var("CLAUDIUS_SECRET_URL");
    }

    #[test]
    #[serial]
    fn test_mixed_delimited_and_bare_references() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        // Clear any existing CLAUDIUS_SECRET_* vars
        cleanup_claudius_secrets();

        // Enable mock mode
        std::env::set_var("CLAUDIUS_TEST_MOCK_OP", "1");

        let config =
            SecretManagerConfig { manager_type: SecretManagerType::OnePassword, onepassword: None };
        let resolver = SecretResolver::new(Some(config));

        // Mix of delimited and bare references
        std::env::set_var(
            "CLAUDIUS_SECRET_MIXED",
            "key=op://vault/test-item/api-key url={{op://vault/database/password}}",
        );

        let resolved = resolver.resolve_env_vars().expect("resolve_env_vars should succeed");
        assert_eq!(
            resolved.get("MIXED"),
            Some(&"key=secret-api-key-12345 url=db-password-xyz789".to_string())
        );

        // Cleanup
        std::env::remove_var("CLAUDIUS_TEST_MOCK_OP");
        std::env::remove_var("CLAUDIUS_SECRET_MIXED");
    }

    #[test]
    #[serial]
    fn test_cloudflare_example_with_delimiters() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        // Clear any existing CLAUDIUS_SECRET_* vars
        cleanup_claudius_secrets();

        // Enable mock mode
        std::env::set_var("CLAUDIUS_TEST_MOCK_OP", "1");

        let config =
            SecretManagerConfig { manager_type: SecretManagerType::OnePassword, onepassword: None };
        let resolver = SecretResolver::new(Some(config));

        // Use delimited syntax for clarity
        std::env::set_var(
            "CLAUDIUS_SECRET_CF_URL",
            "https://gateway.ai.cloudflare.com/v1/{{op://Private/CLOUDFLARE AI Gateway/Account ID}}/{{op://Private/CLOUDFLARE AI Gateway/Gateway ID}}/anthropic",
        );

        let resolved = resolver.resolve_env_vars().expect("resolve_env_vars should succeed");
        assert_eq!(
            resolved.get("CF_URL"),
            Some(
                &"https://gateway.ai.cloudflare.com/v1/cf-account-12345/cf-gateway-67890/anthropic"
                    .to_string()
            )
        );

        // Cleanup
        std::env::remove_var("CLAUDIUS_TEST_MOCK_OP");
        std::env::remove_var("CLAUDIUS_SECRET_CF_URL");
    }
}
