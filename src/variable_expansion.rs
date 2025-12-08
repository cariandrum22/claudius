use anyhow::{anyhow, Result};
use regex::Regex;
use std::collections::{HashMap, VecDeque};
use tracing::{debug, warn};

/// Represents a node in the dependency graph
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct VariableNode {
    /// The full variable name (e.g., `CLAUDIUS_SECRET_API_KEY`)
    name: String,
    /// The raw value containing potential references
    raw_value: String,
    /// Names of variables this node depends on
    dependencies: Vec<String>,
    /// The resolved value after expansion
    resolved_value: Option<String>,
}

/// Dependency graph for variable resolution
#[derive(Debug)]
pub struct VariableGraph {
    nodes: HashMap<String, VariableNode>,
    /// Regex to match variable references like `$CLAUDIUS_SECRET_VAR_NAME`
    var_ref_regex: Regex,
}

impl VariableGraph {
    /// Creates a new variable graph for dependency resolution.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Unable to compile the regex pattern for variable references
    pub fn new() -> Result<Self> {
        // Match $CLAUDIUS_SECRET_VARNAME with optional braces ${CLAUDIUS_SECRET_VARNAME}
        let var_ref_regex = Regex::new(r"\$\{?(CLAUDIUS_SECRET_[A-Z_][A-Z0-9_]*)\}?")?;

        Ok(Self { nodes: HashMap::new(), var_ref_regex })
    }

    /// Add a variable to the graph
    pub fn add_variable(&mut self, name: String, value: String) {
        let dependencies = self.extract_dependencies(&value);

        let node = VariableNode {
            name: name.clone(),
            raw_value: value,
            dependencies,
            resolved_value: None,
        };

        self.nodes.insert(name, node);
    }

    /// Extract variable references from a value
    fn extract_dependencies(&self, value: &str) -> Vec<String> {
        let mut deps = Vec::new();

        for cap in self.var_ref_regex.captures_iter(value) {
            if let Some(var_name) = cap.get(1) {
                deps.push(var_name.as_str().to_string());
            }
        }

        deps
    }

    /// Helper function to add dependency relationship
    fn add_dependency(
        adjacency_list: &mut HashMap<String, Vec<String>>,
        in_degree: &mut HashMap<String, usize>,
        dep: &str,
        dependent: &str,
    ) -> Result<()> {
        let dependents = adjacency_list
            .get_mut(dep)
            .ok_or_else(|| anyhow!("dependency {dep} should exist in adjacency_list"))?;
        dependents.push(dependent.to_string());

        let degree = in_degree
            .get_mut(dependent)
            .ok_or_else(|| anyhow!("node {dependent} should exist in in_degree map"))?;
        *degree = degree.saturating_add(1);

        Ok(())
    }

    /// Perform topological sort and detect cycles
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A circular dependency is detected in the variable graph
    /// - Internal data structure inconsistency is found
    pub fn topological_sort(&self) -> Result<Vec<String>> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut adjacency_list: HashMap<String, Vec<String>> = HashMap::new();

        // Initialize structures
        for name in self.nodes.keys() {
            in_degree.insert(name.clone(), 0);
            adjacency_list.insert(name.clone(), vec![]);
        }

        // Build adjacency list and calculate in-degrees
        for (name, node) in &self.nodes {
            for dep in &node.dependencies {
                // Only consider dependencies that exist in our graph
                if self.nodes.contains_key(dep) {
                    Self::add_dependency(&mut adjacency_list, &mut in_degree, dep, name)?;
                }
            }
        }

        // Kahn's algorithm for topological sort
        let mut queue = VecDeque::new();
        let mut sorted_order = Vec::new();

        // Find all nodes with no dependencies
        for (name, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(name.clone());
            }
        }

        while let Some(current) = queue.pop_front() {
            sorted_order.push(current.clone());

            // Process all nodes that depend on current
            let Some(dependents) = adjacency_list.get(&current) else {
                continue;
            };

            for dependent in dependents {
                let degree = in_degree.get_mut(dependent).ok_or_else(|| {
                    anyhow!("dependent {dependent} should exist in in_degree map")
                })?;
                *degree = degree.saturating_sub(1);

                if *degree == 0 {
                    queue.push_back(dependent.clone());
                }
            }
        }

        // Check for cycles
        if sorted_order.len() != self.nodes.len() {
            let cycle_nodes = Self::find_cycle_nodes(&in_degree);
            return Err(anyhow!(
                "Circular dependency detected involving variables: {cycle_nodes:?}"
            ));
        }

        Ok(sorted_order)
    }

    /// Find nodes involved in cycles
    fn find_cycle_nodes(in_degree: &HashMap<String, usize>) -> Vec<String> {
        in_degree
            .iter()
            .filter(|(_, &degree)| degree > 0)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Resolve all variables in topological order
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Circular dependency is detected during topological sort
    /// - Unable to resolve a variable reference
    /// - Variable not found in the graph
    pub fn resolve_all<S: std::hash::BuildHasher>(
        &mut self,
        external_values: &HashMap<String, String, S>,
    ) -> Result<HashMap<String, String>> {
        let sorted_order = self.topological_sort()?;

        debug!("Topological sort order: {:?}", sorted_order);

        // Resolve in topological order
        for var_name in sorted_order {
            self.resolve_variable(&var_name, external_values)?;
        }

        // Collect resolved values
        let mut result = HashMap::new();
        for (name, node) in &self.nodes {
            if let Some(resolved) = &node.resolved_value {
                // Remove CLAUDIUS_SECRET_ prefix for the final environment variable
                let clean_name = name.strip_prefix("CLAUDIUS_SECRET_").unwrap_or(name);
                result.insert(clean_name.to_string(), resolved.clone());
            }
        }

        Ok(result)
    }

    /// Resolve a single variable
    fn resolve_variable<S: std::hash::BuildHasher>(
        &mut self,
        var_name: &str,
        external_values: &HashMap<String, String, S>,
    ) -> Result<()> {
        let current_node = self
            .nodes
            .get(var_name)
            .ok_or_else(|| anyhow!("Variable {var_name} not found in graph"))?
            .clone();

        if current_node.resolved_value.is_some() {
            // Already resolved
            return Ok(());
        }

        // Replace all variable references in the value
        let (resolved, unresolved_refs) =
            self.expand_variable_references(&current_node.raw_value, external_values);

        // Warn about unresolved references
        if !unresolved_refs.is_empty() {
            warn!("Variable {} contains unresolved references: {:?}", var_name, unresolved_refs);
        }

        // Update the node with resolved value
        if let Some(node) = self.nodes.get_mut(var_name) {
            node.resolved_value = Some(resolved);
        }

        Ok(())
    }

    /// Replace variable references in a string
    fn expand_variable_references<S: std::hash::BuildHasher>(
        &self,
        value: &str,
        external_values: &HashMap<String, String, S>,
    ) -> (String, Vec<String>) {
        let mut unresolved_refs = Vec::new();

        let expanded = self
            .var_ref_regex
            .replace_all(value, |caps: &regex::Captures| {
                let var_ref = caps
                    .get(1)
                    .expect("regex pattern guarantees capture group 1 exists for CLAUDIUS_SECRET_*")
                    .as_str();

                // Try to resolve the reference
                self.lookup_variable_value(var_ref, external_values).map_or_else(
                    || {
                        // If not found, keep track and preserve the reference
                        unresolved_refs.push(var_ref.to_string());
                        caps.get(0)
                            .expect("regex match guarantees capture group 0 (full match) exists")
                            .as_str()
                            .to_string()
                    },
                    |resolved_value| resolved_value,
                )
            })
            .to_string();

        (expanded, unresolved_refs)
    }

    /// Look up a variable's value from internal nodes or external values
    fn lookup_variable_value<S: std::hash::BuildHasher>(
        &self,
        var_ref: &str,
        external_values: &HashMap<String, String, S>,
    ) -> Option<String> {
        // First check if it's already resolved in our graph
        if let Some(dep_node) = self.nodes.get(var_ref) {
            if let Some(dep_value) = &dep_node.resolved_value {
                return Some(dep_value.clone());
            }
        }

        // Then check external values (e.g., from 1Password resolution)
        external_values.get(var_ref).cloned()
    }
}

/// Expand variables with dependency resolution
///
/// # Errors
///
/// Returns an error if:
/// - Unable to create the variable graph (regex compilation failure)
/// - Circular dependency is detected between variables
/// - Unable to resolve variable references
pub fn expand_variables<S: std::hash::BuildHasher, S2: std::hash::BuildHasher>(
    variables: HashMap<String, String, S2>,
    resolved_secrets: &HashMap<String, String, S>,
) -> Result<HashMap<String, String>> {
    let mut graph = VariableGraph::new()?;

    // Add all variables to the graph
    for (name, value) in variables {
        graph.add_variable(name, value);
    }

    // Resolve all variables
    graph.resolve_all(resolved_secrets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_variable_reference() {
        let mut variables = HashMap::new();
        variables.insert(
            "CLAUDIUS_SECRET_BASE_URL".to_string(),
            "https://api.example.com/$CLAUDIUS_SECRET_API_KEY".to_string(),
        );

        let mut resolved_secrets = HashMap::new();
        resolved_secrets.insert("CLAUDIUS_SECRET_API_KEY".to_string(), "secret123".to_string());

        let result = expand_variables(variables, &resolved_secrets)
            .expect("expand_variables should succeed");

        assert_eq!(
            result.get("BASE_URL").expect("BASE_URL should be present"),
            "https://api.example.com/secret123"
        );
    }

    #[test]
    fn test_multiple_references() {
        let mut variables = HashMap::new();
        variables.insert(
            "CLAUDIUS_SECRET_URL".to_string(),
            "https://$CLAUDIUS_SECRET_HOST:$CLAUDIUS_SECRET_PORT/api".to_string(),
        );

        let mut resolved_secrets = HashMap::new();
        resolved_secrets.insert("CLAUDIUS_SECRET_HOST".to_string(), "example.com".to_string());
        resolved_secrets.insert("CLAUDIUS_SECRET_PORT".to_string(), "8080".to_string());

        let result = expand_variables(variables, &resolved_secrets)
            .expect("expand_variables should succeed");

        assert_eq!(
            result.get("URL").expect("URL should be present"),
            "https://example.com:8080/api"
        );
    }

    #[test]
    fn test_braces_syntax() {
        let mut variables = HashMap::new();
        variables.insert(
            "CLAUDIUS_SECRET_PATH".to_string(),
            "${CLAUDIUS_SECRET_BASE}/data/${CLAUDIUS_SECRET_ID}".to_string(),
        );

        let mut resolved_secrets = HashMap::new();
        resolved_secrets.insert("CLAUDIUS_SECRET_BASE".to_string(), "/var/lib".to_string());
        resolved_secrets.insert("CLAUDIUS_SECRET_ID".to_string(), "12345".to_string());

        let result = expand_variables(variables, &resolved_secrets)
            .expect("expand_variables should succeed");

        assert_eq!(result.get("PATH").expect("PATH should be present"), "/var/lib/data/12345");
    }

    #[test]
    fn test_chain_dependencies() {
        let mut variables = HashMap::new();
        variables.insert("CLAUDIUS_SECRET_A".to_string(), "value_a".to_string());
        variables.insert("CLAUDIUS_SECRET_B".to_string(), "prefix_$CLAUDIUS_SECRET_A".to_string());
        variables.insert("CLAUDIUS_SECRET_C".to_string(), "$CLAUDIUS_SECRET_B-suffix".to_string());

        let resolved_secrets = HashMap::new();

        let result = expand_variables(variables, &resolved_secrets)
            .expect("expand_variables should succeed");

        assert_eq!(result.get("A").expect("A should be present"), "value_a");
        assert_eq!(result.get("B").expect("B should be present"), "prefix_value_a");
        assert_eq!(result.get("C").expect("C should be present"), "prefix_value_a-suffix");
    }

    #[test]
    fn test_circular_dependency() {
        let mut variables = HashMap::new();
        variables.insert("CLAUDIUS_SECRET_A".to_string(), "$CLAUDIUS_SECRET_B".to_string());
        variables.insert("CLAUDIUS_SECRET_B".to_string(), "$CLAUDIUS_SECRET_C".to_string());
        variables.insert("CLAUDIUS_SECRET_C".to_string(), "$CLAUDIUS_SECRET_A".to_string());

        let resolved_secrets = HashMap::new();

        let result = expand_variables(variables, &resolved_secrets);
        assert!(result.is_err());
        assert!(result
            .expect_err("Should return error for circular dependency")
            .to_string()
            .contains("Circular dependency"));
    }

    #[test]
    fn test_unresolved_reference_warning() {
        let mut variables = HashMap::new();
        variables.insert(
            "CLAUDIUS_SECRET_URL".to_string(),
            "https://api.example.com/$CLAUDIUS_SECRET_UNKNOWN".to_string(),
        );

        let resolved_secrets = HashMap::new();

        let result = expand_variables(variables, &resolved_secrets)
            .expect("expand_variables should succeed");

        // The unresolved reference should be preserved
        assert_eq!(
            result.get("URL").expect("URL should be present"),
            "https://api.example.com/$CLAUDIUS_SECRET_UNKNOWN"
        );
    }

    #[test]
    fn test_complex_dependency_graph() {
        let mut variables = HashMap::new();
        // A depends on nothing
        variables.insert("CLAUDIUS_SECRET_A".to_string(), "a".to_string());
        // B depends on A
        variables.insert("CLAUDIUS_SECRET_B".to_string(), "$CLAUDIUS_SECRET_A-b".to_string());
        // C depends on A
        variables.insert("CLAUDIUS_SECRET_C".to_string(), "$CLAUDIUS_SECRET_A-c".to_string());
        // D depends on B and C
        variables.insert(
            "CLAUDIUS_SECRET_D".to_string(),
            "$CLAUDIUS_SECRET_B-$CLAUDIUS_SECRET_C-d".to_string(),
        );

        let resolved_secrets = HashMap::new();

        let result = expand_variables(variables, &resolved_secrets)
            .expect("expand_variables should succeed");

        assert_eq!(result.get("A").expect("A should be present"), "a");
        assert_eq!(result.get("B").expect("B should be present"), "a-b");
        assert_eq!(result.get("C").expect("C should be present"), "a-c");
        assert_eq!(result.get("D").expect("D should be present"), "a-b-a-c-d");
    }
}
