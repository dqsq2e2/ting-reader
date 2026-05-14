use super::*;
use crate::plugin::types::{PluginType, PluginDependency, PluginMetadata};
use std::sync::Arc;

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
        format!("{}@{}", name, version),
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
        "dependent-plugin@1.0.0".to_string(),
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
        "dependent-plugin@1.0.0".to_string(),
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
        "dependent-plugin@1.0.0".to_string(),
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
        "dependent-plugin@1.0.0".to_string(),
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
        "dependent-plugin@1.0.0".to_string(),
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
        "dependent-plugin@1.0.0".to_string(),
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
        "dependent-plugin@1.0.0".to_string(),
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
        "dependent-plugin@1.0.0".to_string(),
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
        "dependent@1.0.0".to_string(),
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
        "middle@1.0.0".to_string(),
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
        "top@1.0.0".to_string(),
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
        "middle@1.0.0".to_string(),
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
        "top@1.0.0".to_string(),
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
        "dependent@1.0.0".to_string(),
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
        "plugin-b@1.0.0".to_string(),
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
        "plugin-c@1.0.0".to_string(),
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
        "middle@1.0.0".to_string(),
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
        "top@1.0.0".to_string(),
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
