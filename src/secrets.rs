#[cfg(not(test))]
use anyhow::Context;
use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashMap;
#[cfg(not(test))]
use std::process::Command;
use std::sync::{Arc, Mutex};
use tracing::{debug, warn};

use crate::app_config::{SecretManagerConfig, SecretManagerType};
use crate::profiling::{SecretResolutionMetrics, Timer};
use crate::variable_expansion::expand_variables;

#[derive(Debug, Clone)]
pub struct SecretResolver {
    config: Option<SecretManagerConfig>,
    cache: Arc<Mutex<HashMap<String, String>>>,
    metrics: Arc<Mutex<SecretResolutionMetrics>>,
}

impl SecretResolver {
    #[must_use]
    pub fn new(config: Option<SecretManagerConfig>) -> Self {
        Self {
            config,
            cache: Arc::new(Mutex::new(HashMap::new())),
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

        // Phase 2: Resolve secrets in parallel
        let resolved_vars = self.resolve_secrets_parallel(&claudius_secrets)?;

        // Phase 3: Expand variable references
        let expanded_vars = Self::expand_variables(resolved_vars)?;

        // Phase 4: Remove prefixes and finalize
        let result = Self::remove_prefixes(expanded_vars);

        self.log_metrics(total_timer.stop());
        Ok(result)
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

        cached.map_or_else(
            || {
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
            },
            |cached_value| {
                debug!("Using cached value for {}", op_ref);
                cached_value
            },
        )
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

    #[test]
    fn test_secret_resolver_new() {
        let resolver = SecretResolver::new(None);
        assert!(resolver.config.is_none());

        let config = SecretManagerConfig { manager_type: SecretManagerType::Vault };
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
        let config = SecretManagerConfig { manager_type: SecretManagerType::Vault };
        let resolver = SecretResolver::new(Some(config));

        let result = resolver
            .resolve_value("CLAUDIUS_SECRET_KEY", "vault://secret/data")
            .expect("resolve_value should succeed with vault config");
        assert!(result.is_none()); // Vault not implemented yet
    }

    #[test]
    fn test_resolve_value_onepassword_non_reference() {
        let config = SecretManagerConfig { manager_type: SecretManagerType::OnePassword };
        let resolver = SecretResolver::new(Some(config));

        let result = resolver
            .resolve_value("CLAUDIUS_SECRET_KEY", "plain-value")
            .expect("resolve_value should succeed");
        assert_eq!(result, None); // No op:// references, so no resolution needed
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

        let config = SecretManagerConfig { manager_type: SecretManagerType::OnePassword };
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
    fn test_onepassword_mock_error() {
        let _guard = ENV_MUTEX.lock().expect("Failed to acquire mutex lock");

        // Clear any existing CLAUDIUS_SECRET_* vars
        cleanup_claudius_secrets();

        // Enable mock mode
        std::env::set_var("CLAUDIUS_TEST_MOCK_OP", "1");

        let config = SecretManagerConfig { manager_type: SecretManagerType::OnePassword };
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

        let config = SecretManagerConfig { manager_type: SecretManagerType::OnePassword };
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

        let config = SecretManagerConfig { manager_type: SecretManagerType::OnePassword };
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

        let config = SecretManagerConfig { manager_type: SecretManagerType::OnePassword };
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

        let config = SecretManagerConfig { manager_type: SecretManagerType::OnePassword };
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

        let config = SecretManagerConfig { manager_type: SecretManagerType::OnePassword };
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

        let config = SecretManagerConfig { manager_type: SecretManagerType::OnePassword };
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
