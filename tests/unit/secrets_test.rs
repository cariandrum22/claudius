use claudius::app_config::{SecretManagerConfig, SecretManagerType};
use claudius::secrets::SecretResolver;
use serial_test::serial;
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

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
    #[serial]
    fn test_no_secret_manager_config() {
        cleanup_claudius_secrets();
        let resolver = SecretResolver::new(None);

        // Set a test environment variable
        std::env::set_var("CLAUDIUS_SECRET_TEST_VAR", "test_value");

        let resolved = resolver.resolve_env_vars().unwrap();
        assert_eq!(resolved.get("TEST_VAR"), Some(&"test_value".to_string()));

        // Cleanup
        std::env::remove_var("CLAUDIUS_SECRET_TEST_VAR");
    }

    #[test]
    #[serial]
    fn test_onepassword_non_op_reference() {
        cleanup_claudius_secrets();
        let config = SecretManagerConfig { manager_type: SecretManagerType::OnePassword };
        let resolver = SecretResolver::new(Some(config));

        // Set a non-op:// value
        std::env::set_var("CLAUDIUS_SECRET_NORMAL_VAR", "normal_value");

        let resolved = resolver.resolve_env_vars().unwrap();
        assert_eq!(resolved.get("NORMAL_VAR"), Some(&"normal_value".to_string()));

        // Cleanup
        std::env::remove_var("CLAUDIUS_SECRET_NORMAL_VAR");
    }

    #[test]
    #[serial]
    fn test_vault_warning() {
        cleanup_claudius_secrets();
        let config = SecretManagerConfig { manager_type: SecretManagerType::Vault };
        let resolver = SecretResolver::new(Some(config));

        // Set a test environment variable
        std::env::set_var("CLAUDIUS_SECRET_VAULT_VAR", "vault_value");

        let resolved = resolver.resolve_env_vars().unwrap();
        // Vault is not implemented, so it should return the variable but with the original value
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved.get("VAULT_VAR"), Some(&"vault_value".to_string()));

        // Cleanup
        std::env::remove_var("CLAUDIUS_SECRET_VAULT_VAR");
    }

    #[test]
    #[serial]
    fn test_multiple_env_vars() {
        cleanup_claudius_secrets();
        let resolver = SecretResolver::new(None);

        // Set multiple test environment variables
        std::env::set_var("CLAUDIUS_SECRET_VAR1", "value1");
        std::env::set_var("CLAUDIUS_SECRET_VAR2", "value2");
        std::env::set_var("NON_CLAUDIUS_VAR", "ignored");

        let resolved = resolver.resolve_env_vars().unwrap();
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved.get("VAR1"), Some(&"value1".to_string()));
        assert_eq!(resolved.get("VAR2"), Some(&"value2".to_string()));
        assert!(!resolved.contains_key("NON_CLAUDIUS_VAR"));

        // Cleanup
        std::env::remove_var("CLAUDIUS_SECRET_VAR1");
        std::env::remove_var("CLAUDIUS_SECRET_VAR2");
        std::env::remove_var("NON_CLAUDIUS_VAR");
    }

    #[test]
    #[serial]
    fn test_inject_env_vars() {
        let mut vars = HashMap::new();
        vars.insert("TEST_INJECTED".to_string(), "injected_value".to_string());

        SecretResolver::inject_env_vars(vars);

        assert_eq!(std::env::var("TEST_INJECTED").unwrap(), "injected_value");

        // Cleanup
        std::env::remove_var("TEST_INJECTED");
    }

    // Note: Testing actual 1Password integration would require the `op` CLI to be installed
    // and configured, which is not suitable for unit tests. Integration tests could be
    // written separately if needed.
}
