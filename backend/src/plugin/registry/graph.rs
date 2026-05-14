use std::collections::HashMap;
use semver::{Version, VersionReq};
use crate::core::error::{Result, TingError};
use super::super::types::{PluginId, PluginMetadata};
use super::PluginRegistry;

impl PluginRegistry {
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
    pub(super) fn update_dependency_graph(&mut self, plugin_id: &PluginId, metadata: &PluginMetadata) -> Result<()> {
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
    pub(super) fn remove_from_dependency_graph(&mut self, plugin_id: &PluginId) {
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
    pub(super) fn detect_circular_dependencies(&self, start_id: &PluginId) -> Result<()> {
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
            tracing::warn!("无效的版本格式: {}", version);
            return false;
        };
        
        let Ok(req) = VersionReq::parse(requirement) else {
            tracing::warn!("无效的版本要求格式: {}", requirement);
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
            tracing::warn!("无效的版本要求: {}", version_requirement);
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
