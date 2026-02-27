//! Plugin registry implementation
//!
//! This module provides the plugin registry that manages all loaded plugins,
//! their metadata, and dependency relationships.

use std::collections::HashMap;
use std::sync::Arc;
use semver::{Version, VersionReq};
use crate::core::error::{Result, TingError};
use super::types::{Plugin, PluginId, PluginMetadata, PluginState, PluginStats};

/// Plugin registry
///
/// Maintains a registry of all loaded plugins with their metadata,
/// instances, and dependency relationships.
pub struct PluginRegistry {
    /// Map of plugin ID to plugin entry
    plugins: HashMap<PluginId, PluginEntry>,
    
    /// Dependency graph: plugin ID -> list of plugin IDs it depends on
    dependencies: HashMap<PluginId, Vec<PluginId>>,
    
    /// Reverse dependency graph: plugin ID -> list of plugin IDs that depend on it
    dependents: HashMap<PluginId, Vec<PluginId>>,
}

impl PluginRegistry {
    /// Create a new empty plugin registry
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            dependencies: HashMap::new(),
            dependents: HashMap::new(),
        }
    }
    
    /// Register a new plugin
    ///
    /// # Arguments
    /// * `metadata` - Plugin metadata
    /// * `instance` - Plugin instance
    ///
    /// # Returns
    /// The plugin ID assigned to this plugin
    ///
    /// # Errors
    /// Returns an error if:
    /// - A plugin with the same ID already exists
    /// - Dependencies are not satisfied
    pub fn register(
        &mut self,
        metadata: PluginMetadata,
        instance: Arc<dyn Plugin>,
    ) -> Result<PluginId> {
        // Generate plugin ID from name and version
        let plugin_id = format!("{}@{}", metadata.name, metadata.version);
        
        // Check if plugin already exists
        if self.plugins.contains_key(&plugin_id) {
            return Err(TingError::PluginLoadError(
                format!("Plugin {} is already registered", plugin_id)
            ));
        }
        
        // Check dependencies before registering
        self.check_dependencies(&metadata)?;
        
        // Create plugin entry
        let entry = PluginEntry::new(
            plugin_id.clone(),
            metadata.clone(),
            instance,
            PluginState::Loaded,
        );
        
        // Register plugin
        self.plugins.insert(plugin_id.clone(), entry);
        
        // Update dependency graph
        self.update_dependency_graph(&plugin_id, &metadata)?;
        
        Ok(plugin_id)
    }
    
    /// Unregister a plugin
    ///
    /// # Arguments
    /// * `id` - Plugin ID to unregister
    ///
    /// # Errors
    /// Returns an error if:
    /// - Plugin not found
    /// - Other plugins depend on this plugin
    pub fn unregister(&mut self, id: &PluginId) -> Result<()> {
        // Check if plugin exists
        if !self.plugins.contains_key(id) {
            return Err(TingError::PluginNotFound(id.clone()));
        }
        
        // Check if other plugins depend on this one
        if let Some(dependents) = self.dependents.get(id) {
            if !dependents.is_empty() {
                return Err(TingError::DependencyError(
                    format!(
                        "Cannot unregister plugin {}: {} plugin(s) depend on it: {:?}",
                        id,
                        dependents.len(),
                        dependents
                    )
                ));
            }
        }
        
        // Remove from registry
        self.plugins.remove(id);
        
        // Clean up dependency graph
        self.remove_from_dependency_graph(id);
        
        Ok(())
    }
    
    /// Get a plugin entry by ID
    ///
    /// # Arguments
    /// * `id` - Plugin ID
    ///
    /// # Returns
    /// Reference to the plugin entry, or None if not found
    pub fn get(&self, id: &PluginId) -> Option<&PluginEntry> {
        self.plugins.get(id)
    }
    
    /// Get a mutable plugin entry by ID
    ///
    /// # Arguments
    /// * `id` - Plugin ID
    ///
    /// # Returns
    /// Mutable reference to the plugin entry, or None if not found
    pub fn get_mut(&mut self, id: &PluginId) -> Option<&mut PluginEntry> {
        self.plugins.get_mut(id)
    }
    
    /// List all registered plugins
    ///
    /// # Returns
    /// Vector of references to all plugin entries
    pub fn list(&self) -> Vec<&PluginEntry> {
        self.plugins.values().collect()
    }
    
    /// Find plugins by type
    ///
    /// # Arguments
    /// * `plugin_type` - Type of plugins to find
    ///
    /// # Returns
    /// Vector of plugin entries matching the type
    pub fn find_by_type(&self, plugin_type: super::types::PluginType) -> Vec<&PluginEntry> {
        self.plugins
            .values()
            .filter(|entry| entry.metadata.plugin_type == plugin_type)
            .collect()
    }
    
    /// Find plugins by name (all versions)
    ///
    /// # Arguments
    /// * `name` - Plugin name
    ///
    /// # Returns
    /// Vector of plugin entries with matching name
    pub fn find_by_name(&self, name: &str) -> Vec<&PluginEntry> {
        self.plugins
            .values()
            .filter(|entry| entry.metadata.name == name)
            .collect()
    }
    
    /// Check if all dependencies are satisfied
    ///
    /// # Arguments
    /// * `metadata` - Plugin metadata to check
    ///
    /// # Errors
    /// Returns an error if any dependency is not satisfied
    pub fn check_dependencies(&self, metadata: &PluginMetadata) -> Result<()> {
        let mut missing_deps = Vec::new();
        
        for dep in &metadata.dependencies {
            // Find plugins with matching name
            let matching_plugins = self.find_by_name(&dep.plugin_name);
            
            if matching_plugins.is_empty() {
                missing_deps.push(format!(
                    "{} ({})",
                    dep.plugin_name,
                    dep.version_requirement
                ));
                continue;
            }
            
            // Check if any version satisfies the requirement
            let version_satisfied = matching_plugins.iter().any(|entry| {
                // Simple version check - in production, use semver crate
                self.version_matches(&entry.metadata.version, &dep.version_requirement)
            });
            
            if !version_satisfied {
                missing_deps.push(format!(
                    "{} ({}) - available versions: {:?}",
                    dep.plugin_name,
                    dep.version_requirement,
                    matching_plugins.iter().map(|e| &e.metadata.version).collect::<Vec<_>>()
                ));
            }
        }
        
        if !missing_deps.is_empty() {
            return Err(TingError::DependencyError(
                format!(
                    "Missing or incompatible dependencies for plugin {}: {}",
                    metadata.name,
                    missing_deps.join(", ")
                )
            ));
        }
        
        Ok(())
    }
    
    /// Get load order for a set of plugins
    ///
    /// Uses topological sort to determine the correct load order
    /// based on dependencies.
    ///
    /// # Arguments
    /// * `ids` - Plugin IDs to order
    ///
    /// # Returns
    /// Vector of plugin IDs in load order (dependencies first)
    ///
    /// # Errors
    /// Returns an error if circular dependencies are detected
    pub fn get_load_order(&self, ids: &[PluginId]) -> Result<Vec<PluginId>> {
        let mut result = Vec::new();
        let mut visited = HashMap::new();
        let mut rec_stack = HashMap::new();
        
        for id in ids {
            if !visited.contains_key(id) {
                self.topological_sort(id, &mut visited, &mut rec_stack, &mut result)?;
            }
        }
        
        Ok(result)
    }
    
    /// Get plugins that depend on the given plugin
    ///
    /// # Arguments
    /// * `id` - Plugin ID
    ///
    /// # Returns
    /// Vector of plugin IDs that depend on this plugin
    pub fn get_dependents(&self, id: &PluginId) -> Vec<PluginId> {
        self.dependents
            .get(id)
            .cloned()
            .unwrap_or_default()
    }
    
    /// Get direct dependencies of a plugin
    ///
    /// # Arguments
    /// * `id` - Plugin ID
    ///
    /// # Returns
    /// Vector of plugin IDs that this plugin depends on
    pub fn get_dependencies(&self, id: &PluginId) -> Vec<PluginId> {
        self.dependencies
            .get(id)
            .cloned()
            .unwrap_or_default()
    }
    
    /// Get all transitive dependencies of a plugin
    ///
    /// Returns all plugins that the given plugin depends on, directly or indirectly.
    ///
    /// # Arguments
    /// * `id` - Plugin ID
    ///
    /// # Returns
    /// Vector of all transitive dependency plugin IDs
    pub fn get_all_dependencies(&self, id: &PluginId) -> Vec<PluginId> {
        let mut result = Vec::new();
        let mut visited = HashMap::new();
        self.collect_dependencies(id, &mut visited, &mut result);
        result
    }
    
    /// Helper to recursively collect all dependencies
    fn collect_dependencies(
        &self,
        id: &PluginId,
        visited: &mut HashMap<PluginId, bool>,
        result: &mut Vec<PluginId>,
    ) {
        if visited.contains_key(id) {
            return;
        }
        
        visited.insert(id.clone(), true);
        
        if let Some(deps) = self.dependencies.get(id) {
            for dep_id in deps {
                self.collect_dependencies(dep_id, visited, result);
                if !result.contains(dep_id) {
                    result.push(dep_id.clone());
                }
            }
        }
    }
    
    /// Get all transitive dependents of a plugin
    ///
    /// Returns all plugins that depend on the given plugin, directly or indirectly.
    ///
    /// # Arguments
    /// * `id` - Plugin ID
    ///
    /// # Returns
    /// Vector of all transitive dependent plugin IDs
    pub fn get_all_dependents(&self, id: &PluginId) -> Vec<PluginId> {
        let mut result = Vec::new();
        let mut visited = HashMap::new();
        self.collect_dependents(id, &mut visited, &mut result);
        result
    }
    
    /// Helper to recursively collect all dependents
    fn collect_dependents(
        &self,
        id: &PluginId,
        visited: &mut HashMap<PluginId, bool>,
        result: &mut Vec<PluginId>,
    ) {
        if visited.contains_key(id) {
            return;
        }
        
        visited.insert(id.clone(), true);
        
        if let Some(deps) = self.dependents.get(id) {
            for dep_id in deps {
                if !result.contains(dep_id) {
                    result.push(dep_id.clone());
                }
                self.collect_dependents(dep_id, visited, result);
            }
        }
    }
    
    /// Update dependency graph when a plugin is registered
    fn update_dependency_graph(&mut self, plugin_id: &PluginId, metadata: &PluginMetadata) -> Result<()> {
        // Build list of dependency IDs
        let mut dep_ids = Vec::new();
        
        for dep in &metadata.dependencies {
            // Find the best matching plugin using semver
            if let Some(dep_id) = self.find_best_match(&dep.plugin_name, &dep.version_requirement) {
                dep_ids.push(dep_id.clone());
                
                // Add to reverse dependency graph
                self.dependents
                    .entry(dep_id)
                    .or_insert_with(Vec::new)
                    .push(plugin_id.clone());
            }
        }
        
        // Store dependencies
        if !dep_ids.is_empty() {
            self.dependencies.insert(plugin_id.clone(), dep_ids);
        }
        
        // Check for circular dependencies
        self.detect_circular_dependencies(plugin_id)?;
        
        Ok(())
    }
    
    /// Remove plugin from dependency graph
    fn remove_from_dependency_graph(&mut self, plugin_id: &PluginId) {
        // Remove from dependencies map
        self.dependencies.remove(plugin_id);
        
        // Remove from dependents map
        self.dependents.remove(plugin_id);
        
        // Remove from other plugins' dependent lists
        for dependents in self.dependents.values_mut() {
            dependents.retain(|id| id != plugin_id);
        }
        
        // Remove from other plugins' dependency lists
        for dependencies in self.dependencies.values_mut() {
            dependencies.retain(|id| id != plugin_id);
        }
    }
    
    /// Detect circular dependencies starting from a plugin
    fn detect_circular_dependencies(&self, start_id: &PluginId) -> Result<()> {
        let mut visited = HashMap::new();
        let mut rec_stack = HashMap::new();
        
        self.detect_cycle(start_id, &mut visited, &mut rec_stack)?;
        
        Ok(())
    }
    
    /// Validate the entire dependency graph for consistency
    ///
    /// Checks that:
    /// - All dependencies exist
    /// - No circular dependencies
    /// - All version requirements are satisfied
    ///
    /// # Returns
    /// Ok if the graph is valid, Err with details if invalid
    pub fn validate_dependency_graph(&self) -> Result<()> {
        // Check each plugin's dependencies
        for (plugin_id, entry) in &self.plugins {
            // Verify all dependencies are satisfied
            self.check_dependencies(&entry.metadata)?;
            
            // Verify no circular dependencies from this plugin
            let mut visited = HashMap::new();
            let mut rec_stack = HashMap::new();
            self.detect_cycle(plugin_id, &mut visited, &mut rec_stack)?;
        }
        
        // Verify dependency graph consistency
        for (plugin_id, dep_ids) in &self.dependencies {
            for dep_id in dep_ids {
                // Verify dependency exists
                if !self.plugins.contains_key(dep_id) {
                    return Err(TingError::DependencyError(
                        format!("Plugin {} depends on non-existent plugin {}", plugin_id, dep_id)
                    ));
                }
                
                // Verify reverse dependency is recorded
                if let Some(dependents) = self.dependents.get(dep_id) {
                    if !dependents.contains(plugin_id) {
                        return Err(TingError::DependencyError(
                            format!("Dependency graph inconsistency: {} -> {} not in reverse map", plugin_id, dep_id)
                        ));
                    }
                } else {
                    return Err(TingError::DependencyError(
                        format!("Dependency graph inconsistency: {} has no dependents entry", dep_id)
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    /// Recursive cycle detection helper
    fn detect_cycle(
        &self,
        id: &PluginId,
        visited: &mut HashMap<PluginId, bool>,
        rec_stack: &mut HashMap<PluginId, bool>,
    ) -> Result<()> {
        visited.insert(id.clone(), true);
        rec_stack.insert(id.clone(), true);
        
        if let Some(deps) = self.dependencies.get(id) {
            for dep_id in deps {
                if !visited.get(dep_id).copied().unwrap_or(false) {
                    self.detect_cycle(dep_id, visited, rec_stack)?;
                } else if rec_stack.get(dep_id).copied().unwrap_or(false) {
                    return Err(TingError::DependencyError(
                        format!("Circular dependency detected: {} -> {}", id, dep_id)
                    ));
                }
            }
        }
        
        rec_stack.insert(id.clone(), false);
        Ok(())
    }
    
    /// Topological sort helper for load order
    fn topological_sort(
        &self,
        id: &PluginId,
        visited: &mut HashMap<PluginId, bool>,
        rec_stack: &mut HashMap<PluginId, bool>,
        result: &mut Vec<PluginId>,
    ) -> Result<()> {
        visited.insert(id.clone(), true);
        rec_stack.insert(id.clone(), true);
        
        if let Some(deps) = self.dependencies.get(id) {
            for dep_id in deps {
                if !visited.get(dep_id).copied().unwrap_or(false) {
                    self.topological_sort(dep_id, visited, rec_stack, result)?;
                } else if rec_stack.get(dep_id).copied().unwrap_or(false) {
                    return Err(TingError::DependencyError(
                        format!("Circular dependency detected during load order calculation")
                    ));
                }
            }
        }
        
        rec_stack.insert(id.clone(), false);
        result.push(id.clone());
        Ok(())
    }
    
    /// Check if a version satisfies a version requirement using semantic versioning
    ///
    /// # Arguments
    /// * `version` - The version string to check (e.g., "1.2.3")
    /// * `requirement` - The version requirement (e.g., "^1.0.0", ">=2.0.0", "~1.2")
    ///
    /// # Returns
    /// `true` if the version satisfies the requirement, `false` otherwise
    fn version_matches(&self, version: &str, requirement: &str) -> bool {
        // Parse version and requirement using semver crate
        let Ok(ver) = Version::parse(version) else {
            tracing::warn!("Invalid version format: {}", version);
            return false;
        };
        
        let Ok(req) = VersionReq::parse(requirement) else {
            tracing::warn!("Invalid version requirement format: {}", requirement);
            // Fallback to exact match for invalid requirements
            return version == requirement;
        };
        
        req.matches(&ver)
    }
    
    /// Find the best matching plugin for a dependency
    ///
    /// Returns the plugin with the highest version that satisfies the requirement.
    ///
    /// # Arguments
    /// * `plugin_name` - Name of the plugin to find
    /// * `version_requirement` - Version requirement string
    ///
    /// # Returns
    /// The plugin ID of the best match, or None if no match found
    pub fn find_best_match(&self, plugin_name: &str, version_requirement: &str) -> Option<PluginId> {
        let matching_plugins = self.find_by_name(plugin_name);
        
        if matching_plugins.is_empty() {
            return None;
        }
        
        // Parse version requirement
        let Ok(req) = VersionReq::parse(version_requirement) else {
            tracing::warn!("Invalid version requirement: {}", version_requirement);
            return None;
        };
        
        // Find all plugins that satisfy the requirement
        let mut satisfying: Vec<_> = matching_plugins
            .into_iter()
            .filter_map(|entry| {
                Version::parse(&entry.metadata.version)
                    .ok()
                    .filter(|ver| req.matches(ver))
                    .map(|ver| (entry.id.clone(), ver))
            })
            .collect();
        
        // Sort by version (highest first)
        satisfying.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Return the highest version
        satisfying.first().map(|(id, _)| id.clone())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Plugin entry in the registry
///
/// Contains all information about a registered plugin including
/// its metadata, instance, state, and statistics.
#[derive(Clone)]
pub struct PluginEntry {
    /// Unique plugin ID
    pub id: PluginId,
    
    /// Plugin metadata
    pub metadata: PluginMetadata,
    
    /// Plugin instance
    pub instance: Arc<dyn Plugin>,
    
    /// Current plugin state
    pub state: PluginState,
    
    /// Plugin statistics
    pub stats: PluginStats,
    
    /// Number of active tasks currently using this plugin
    /// This is used to prevent unloading a plugin while it's in use
    pub active_tasks: Arc<std::sync::atomic::AtomicU32>,
}

impl PluginEntry {
    /// Create a new plugin entry
    pub fn new(
        id: PluginId,
        metadata: PluginMetadata,
        instance: Arc<dyn Plugin>,
        state: PluginState,
    ) -> Self {
        Self {
            id,
            metadata,
            instance,
            state,
            stats: PluginStats::new(),
            active_tasks: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }
    
    /// Update plugin state
    pub fn set_state(&mut self, state: PluginState) {
        self.state = state;
    }
    
    /// Get plugin state
    pub fn state(&self) -> PluginState {
        self.state
    }
    
    /// Get mutable reference to statistics
    pub fn stats_mut(&mut self) -> &mut PluginStats {
        &mut self.stats
    }
    
    /// Increment active task count
    pub fn increment_active_tasks(&self) -> u32 {
        self.active_tasks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
    }
    
    /// Decrement active task count
    pub fn decrement_active_tasks(&self) -> u32 {
        self.active_tasks.fetch_sub(1, std::sync::atomic::Ordering::SeqCst).saturating_sub(1)
    }
    
    /// Get current active task count
    pub fn active_task_count(&self) -> u32 {
        self.active_tasks.load(std::sync::atomic::Ordering::SeqCst)
    }
    
    /// Check if plugin has active tasks
    pub fn has_active_tasks(&self) -> bool {
        self.active_task_count() > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{PluginType, PluginDependency};
    
    // Mock plugin for testing
    struct MockPlugin {
        metadata: PluginMetadata,
    }
    
    #[async_trait::async_trait]
    impl Plugin for MockPlugin {
        fn metadata(&self) -> &PluginMetadata {
            &self.metadata
        }
        
        async fn initialize(&self, _context: &super::super::types::PluginContext) -> Result<()> {
            Ok(())
        }
        
        async fn shutdown(&self) -> Result<()> {
            Ok(())
        }
        
        fn plugin_type(&self) -> PluginType {
            self.metadata.plugin_type
        }
        
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }
    
    fn create_test_plugin(name: &str, version: &str) -> (PluginMetadata, Arc<dyn Plugin>) {
        let metadata = PluginMetadata::new(
            name.to_string(),
            version.to_string(),
            PluginType::Utility,
            "Test Author".to_string(),
            "Test plugin".to_string(),
            "test.wasm".to_string(),
        );
        
        let plugin = Arc::new(MockPlugin {
            metadata: metadata.clone(),
        }) as Arc<dyn Plugin>;
        
        (metadata, plugin)
    }
    
    #[test]
    fn test_register_plugin() {
        let mut registry = PluginRegistry::new();
        let (metadata, instance) = create_test_plugin("test-plugin", "1.0.0");
        
        let result = registry.register(metadata, instance);
        assert!(result.is_ok());
        
        let plugin_id = result.unwrap();
        assert_eq!(plugin_id, "test-plugin@1.0.0");
        assert!(registry.get(&plugin_id).is_some());
    }
    
    #[test]
    fn test_register_duplicate_plugin() {
        let mut registry = PluginRegistry::new();
        let (metadata, instance) = create_test_plugin("test-plugin", "1.0.0");
        
        registry.register(metadata.clone(), instance.clone()).unwrap();
        let result = registry.register(metadata, instance);
        
        assert!(result.is_err());
    }
    
    #[test]
    fn test_unregister_plugin() {
        let mut registry = PluginRegistry::new();
        let (metadata, instance) = create_test_plugin("test-plugin", "1.0.0");
        
        let plugin_id = registry.register(metadata, instance).unwrap();
        let result = registry.unregister(&plugin_id);
        
        assert!(result.is_ok());
        assert!(registry.get(&plugin_id).is_none());
    }
    
    #[test]
    fn test_unregister_nonexistent_plugin() {
        let mut registry = PluginRegistry::new();
        let result = registry.unregister(&"nonexistent@1.0.0".to_string());
        
        assert!(result.is_err());
    }
    
    #[test]
    fn test_find_by_type() {
        let mut registry = PluginRegistry::new();
        let (metadata1, instance1) = create_test_plugin("plugin1", "1.0.0");
        let (metadata2, instance2) = create_test_plugin("plugin2", "1.0.0");
        
        registry.register(metadata1, instance1).unwrap();
        registry.register(metadata2, instance2).unwrap();
        
        let plugins = registry.find_by_type(PluginType::Utility);
        assert_eq!(plugins.len(), 2);
    }
    
    #[test]
    fn test_find_by_name() {
        let mut registry = PluginRegistry::new();
        let (metadata1, instance1) = create_test_plugin("test-plugin", "1.0.0");
        let (metadata2, instance2) = create_test_plugin("test-plugin", "2.0.0");
        
        registry.register(metadata1, instance1).unwrap();
        registry.register(metadata2, instance2).unwrap();
        
        let plugins = registry.find_by_name("test-plugin");
        assert_eq!(plugins.len(), 2);
    }
    
    #[test]
    fn test_dependency_check_missing() {
        let registry = PluginRegistry::new();
        
        let mut metadata = PluginMetadata::new(
            "dependent-plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        
        metadata.dependencies.push(PluginDependency::new(
            "missing-plugin".to_string(),
            "1.0.0".to_string(),
        ));
        
        let result = registry.check_dependencies(&metadata);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_dependency_check_satisfied() {
        let mut registry = PluginRegistry::new();
        
        // Register dependency first
        let (dep_metadata, dep_instance) = create_test_plugin("base-plugin", "1.0.0");
        registry.register(dep_metadata, dep_instance).unwrap();
        
        // Create dependent plugin
        let mut metadata = PluginMetadata::new(
            "dependent-plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        
        metadata.dependencies.push(PluginDependency::new(
            "base-plugin".to_string(),
            "1.0.0".to_string(),
        ));
        
        let result = registry.check_dependencies(&metadata);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_unregister_with_dependents() {
        let mut registry = PluginRegistry::new();
        
        // Register base plugin
        let (base_metadata, base_instance) = create_test_plugin("base-plugin", "1.0.0");
        let base_id = registry.register(base_metadata, base_instance).unwrap();
        
        // Register dependent plugin
        let mut dep_metadata = PluginMetadata::new(
            "dependent-plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        dep_metadata.dependencies.push(PluginDependency::new(
            "base-plugin".to_string(),
            "1.0.0".to_string(),
        ));
        
        let (_, dep_instance) = create_test_plugin("dependent-plugin", "1.0.0");
        registry.register(dep_metadata, dep_instance).unwrap();
        
        // Try to unregister base plugin - should fail
        let result = registry.unregister(&base_id);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_get_dependents() {
        let mut registry = PluginRegistry::new();
        
        // Register base plugin
        let (base_metadata, base_instance) = create_test_plugin("base-plugin", "1.0.0");
        let base_id = registry.register(base_metadata, base_instance).unwrap();
        
        // Register dependent plugin
        let mut dep_metadata = PluginMetadata::new(
            "dependent-plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        dep_metadata.dependencies.push(PluginDependency::new(
            "base-plugin".to_string(),
            "1.0.0".to_string(),
        ));
        
        let (_, dep_instance) = create_test_plugin("dependent-plugin", "1.0.0");
        registry.register(dep_metadata, dep_instance).unwrap();
        
        // Check dependents
        let dependents = registry.get_dependents(&base_id);
        assert_eq!(dependents.len(), 1);
        assert_eq!(dependents[0], "dependent-plugin@1.0.0");
    }
    
    #[test]
    fn test_semver_caret_requirement() {
        let mut registry = PluginRegistry::new();
        
        // Register base plugin version 1.2.3
        let (base_metadata, base_instance) = create_test_plugin("base-plugin", "1.2.3");
        registry.register(base_metadata, base_instance).unwrap();
        
        // Create dependent plugin with caret requirement ^1.0.0
        let mut dep_metadata = PluginMetadata::new(
            "dependent-plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        dep_metadata.dependencies.push(PluginDependency::new(
            "base-plugin".to_string(),
            "^1.0.0".to_string(),
        ));
        
        // Should succeed because 1.2.3 satisfies ^1.0.0
        let result = registry.check_dependencies(&dep_metadata);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_semver_caret_requirement_fails() {
        let mut registry = PluginRegistry::new();
        
        // Register base plugin version 2.0.0
        let (base_metadata, base_instance) = create_test_plugin("base-plugin", "2.0.0");
        registry.register(base_metadata, base_instance).unwrap();
        
        // Create dependent plugin with caret requirement ^1.0.0
        let mut dep_metadata = PluginMetadata::new(
            "dependent-plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        dep_metadata.dependencies.push(PluginDependency::new(
            "base-plugin".to_string(),
            "^1.0.0".to_string(),
        ));
        
        // Should fail because 2.0.0 does not satisfy ^1.0.0
        let result = registry.check_dependencies(&dep_metadata);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_semver_tilde_requirement() {
        let mut registry = PluginRegistry::new();
        
        // Register base plugin version 1.2.5
        let (base_metadata, base_instance) = create_test_plugin("base-plugin", "1.2.5");
        registry.register(base_metadata, base_instance).unwrap();
        
        // Create dependent plugin with tilde requirement ~1.2.0
        let mut dep_metadata = PluginMetadata::new(
            "dependent-plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        dep_metadata.dependencies.push(PluginDependency::new(
            "base-plugin".to_string(),
            "~1.2.0".to_string(),
        ));
        
        // Should succeed because 1.2.5 satisfies ~1.2.0
        let result = registry.check_dependencies(&dep_metadata);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_semver_range_requirement() {
        let mut registry = PluginRegistry::new();
        
        // Register base plugin version 1.5.0
        let (base_metadata, base_instance) = create_test_plugin("base-plugin", "1.5.0");
        registry.register(base_metadata, base_instance).unwrap();
        
        // Create dependent plugin with range requirement >=1.2.0, <2.0.0
        let mut dep_metadata = PluginMetadata::new(
            "dependent-plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        dep_metadata.dependencies.push(PluginDependency::new(
            "base-plugin".to_string(),
            ">=1.2.0, <2.0.0".to_string(),
        ));
        
        // Should succeed because 1.5.0 satisfies >=1.2.0, <2.0.0
        let result = registry.check_dependencies(&dep_metadata);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_find_best_match_highest_version() {
        let mut registry = PluginRegistry::new();
        
        // Register multiple versions
        let (v1, i1) = create_test_plugin("test-plugin", "1.0.0");
        let (v2, i2) = create_test_plugin("test-plugin", "1.5.0");
        let (v3, i3) = create_test_plugin("test-plugin", "1.2.0");
        
        registry.register(v1, i1).unwrap();
        registry.register(v2, i2).unwrap();
        registry.register(v3, i3).unwrap();
        
        // Find best match for ^1.0.0 - should return highest compatible version
        let best = registry.find_best_match("test-plugin", "^1.0.0");
        assert_eq!(best, Some("test-plugin@1.5.0".to_string()));
    }
    
    #[test]
    fn test_find_best_match_with_constraint() {
        let mut registry = PluginRegistry::new();
        
        // Register multiple versions
        let (v1, i1) = create_test_plugin("test-plugin", "1.0.0");
        let (v2, i2) = create_test_plugin("test-plugin", "1.5.0");
        let (v3, i3) = create_test_plugin("test-plugin", "2.0.0");
        
        registry.register(v1, i1).unwrap();
        registry.register(v2, i2).unwrap();
        registry.register(v3, i3).unwrap();
        
        // Find best match for ^1.0.0 - should return 1.5.0, not 2.0.0
        let best = registry.find_best_match("test-plugin", "^1.0.0");
        assert_eq!(best, Some("test-plugin@1.5.0".to_string()));
    }
    
    #[test]
    fn test_get_dependencies() {
        let mut registry = PluginRegistry::new();
        
        // Register base plugins
        let (base1, inst1) = create_test_plugin("base1", "1.0.0");
        let (base2, inst2) = create_test_plugin("base2", "1.0.0");
        registry.register(base1, inst1).unwrap();
        registry.register(base2, inst2).unwrap();
        
        // Register dependent plugin
        let mut dep_metadata = PluginMetadata::new(
            "dependent".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        dep_metadata.dependencies.push(PluginDependency::new("base1".to_string(), "1.0.0".to_string()));
        dep_metadata.dependencies.push(PluginDependency::new("base2".to_string(), "1.0.0".to_string()));
        
        let (_, dep_inst) = create_test_plugin("dependent", "1.0.0");
        let dep_id = registry.register(dep_metadata, dep_inst).unwrap();
        
        // Check dependencies
        let deps = registry.get_dependencies(&dep_id);
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&"base1@1.0.0".to_string()));
        assert!(deps.contains(&"base2@1.0.0".to_string()));
    }
    
    #[test]
    fn test_get_all_dependencies_transitive() {
        let mut registry = PluginRegistry::new();
        
        // Register base plugin
        let (base, base_inst) = create_test_plugin("base", "1.0.0");
        registry.register(base, base_inst).unwrap();
        
        // Register middle plugin that depends on base
        let mut middle_meta = PluginMetadata::new(
            "middle".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        middle_meta.dependencies.push(PluginDependency::new("base".to_string(), "1.0.0".to_string()));
        let (_, middle_inst) = create_test_plugin("middle", "1.0.0");
        registry.register(middle_meta, middle_inst).unwrap();
        
        // Register top plugin that depends on middle
        let mut top_meta = PluginMetadata::new(
            "top".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        top_meta.dependencies.push(PluginDependency::new("middle".to_string(), "1.0.0".to_string()));
        let (_, top_inst) = create_test_plugin("top", "1.0.0");
        let top_id = registry.register(top_meta, top_inst).unwrap();
        
        // Get all dependencies - should include both middle and base
        let all_deps = registry.get_all_dependencies(&top_id);
        assert_eq!(all_deps.len(), 2);
        assert!(all_deps.contains(&"middle@1.0.0".to_string()));
        assert!(all_deps.contains(&"base@1.0.0".to_string()));
    }
    
    #[test]
    fn test_get_all_dependents_transitive() {
        let mut registry = PluginRegistry::new();
        
        // Register base plugin
        let (base, base_inst) = create_test_plugin("base", "1.0.0");
        let base_id = registry.register(base, base_inst).unwrap();
        
        // Register middle plugin that depends on base
        let mut middle_meta = PluginMetadata::new(
            "middle".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        middle_meta.dependencies.push(PluginDependency::new("base".to_string(), "1.0.0".to_string()));
        let (_, middle_inst) = create_test_plugin("middle", "1.0.0");
        registry.register(middle_meta, middle_inst).unwrap();
        
        // Register top plugin that depends on middle
        let mut top_meta = PluginMetadata::new(
            "top".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        top_meta.dependencies.push(PluginDependency::new("middle".to_string(), "1.0.0".to_string()));
        let (_, top_inst) = create_test_plugin("top", "1.0.0");
        registry.register(top_meta, top_inst).unwrap();
        
        // Get all dependents of base - should include both middle and top
        let all_deps = registry.get_all_dependents(&base_id);
        assert_eq!(all_deps.len(), 2);
        assert!(all_deps.contains(&"middle@1.0.0".to_string()));
        assert!(all_deps.contains(&"top@1.0.0".to_string()));
    }
    
    #[test]
    fn test_validate_dependency_graph_valid() {
        let mut registry = PluginRegistry::new();
        
        // Register base plugin
        let (base, base_inst) = create_test_plugin("base", "1.0.0");
        registry.register(base, base_inst).unwrap();
        
        // Register dependent plugin
        let mut dep_meta = PluginMetadata::new(
            "dependent".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        dep_meta.dependencies.push(PluginDependency::new("base".to_string(), "1.0.0".to_string()));
        let (_, dep_inst) = create_test_plugin("dependent", "1.0.0");
        registry.register(dep_meta, dep_inst).unwrap();
        
        // Validate graph - should succeed
        let result = registry.validate_dependency_graph();
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_circular_dependency_detection() {
        let mut registry = PluginRegistry::new();
        
        // Register plugin A
        let (a_meta, a_inst) = create_test_plugin("plugin-a", "1.0.0");
        registry.register(a_meta, a_inst).unwrap();
        
        // Register plugin B that depends on A
        let mut b_meta = PluginMetadata::new(
            "plugin-b".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        b_meta.dependencies.push(PluginDependency::new("plugin-a".to_string(), "1.0.0".to_string()));
        let (_, b_inst) = create_test_plugin("plugin-b", "1.0.0");
        registry.register(b_meta, b_inst).unwrap();
        
        // Try to register plugin C that depends on B
        let mut c_meta = PluginMetadata::new(
            "plugin-c".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        c_meta.dependencies.push(PluginDependency::new("plugin-b".to_string(), "1.0.0".to_string()));
        let (_, c_inst) = create_test_plugin("plugin-c", "1.0.0");
        registry.register(c_meta, c_inst).unwrap();
        
        // Now manually create a circular dependency by modifying the graph
        // In a real scenario, this would be prevented by the registration logic
        // This test verifies that the detection algorithm works
        
        // Add C as a dependency of A (creating A -> C -> B -> A cycle)
        registry.dependencies.insert(
            "plugin-a@1.0.0".to_string(),
            vec!["plugin-c@1.0.0".to_string()]
        );
        
        // Detect cycle - should fail
        let result = registry.detect_circular_dependencies(&"plugin-a@1.0.0".to_string());
        assert!(result.is_err());
        if let Err(TingError::DependencyError(msg)) = result {
            assert!(msg.contains("Circular dependency"));
        } else {
            panic!("Expected DependencyError");
        }
    }
    
    #[test]
    fn test_load_order_respects_dependencies() {
        let mut registry = PluginRegistry::new();
        
        // Register base plugin
        let (base, base_inst) = create_test_plugin("base", "1.0.0");
        let base_id = registry.register(base, base_inst).unwrap();
        
        // Register middle plugin
        let mut middle_meta = PluginMetadata::new(
            "middle".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        middle_meta.dependencies.push(PluginDependency::new("base".to_string(), "1.0.0".to_string()));
        let (_, middle_inst) = create_test_plugin("middle", "1.0.0");
        let middle_id = registry.register(middle_meta, middle_inst).unwrap();
        
        // Register top plugin
        let mut top_meta = PluginMetadata::new(
            "top".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Test".to_string(),
            "Test".to_string(),
            "test.wasm".to_string(),
        );
        top_meta.dependencies.push(PluginDependency::new("middle".to_string(), "1.0.0".to_string()));
        let (_, top_inst) = create_test_plugin("top", "1.0.0");
        let top_id = registry.register(top_meta, top_inst).unwrap();
        
        // Get load order
        let order = registry.get_load_order(&[top_id.clone(), middle_id.clone(), base_id.clone()]).unwrap();
        
        // Base should come before middle, middle before top
        let base_pos = order.iter().position(|id| id == &base_id).unwrap();
        let middle_pos = order.iter().position(|id| id == &middle_id).unwrap();
        let top_pos = order.iter().position(|id| id == &top_id).unwrap();
        
        assert!(base_pos < middle_pos);
        assert!(middle_pos < top_pos);
    }
}
