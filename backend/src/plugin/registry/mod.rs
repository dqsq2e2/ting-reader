//! Plugin registry implementation
//!
//! This module provides the plugin registry that manages all loaded plugins,
//! their metadata, and dependency relationships.

mod graph;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;
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

