//! Shared plugin metadata reading
//!
//! Reads and validates plugin.json, producing a PluginMetadata struct.

use std::path::Path;
use serde_json::Value;
use tracing::warn;

use super::super::js::npm::NpmManager;
use super::super::wasm::sandbox::Permission;
use super::{PluginMetadata, PluginType, PluginDependency};
use crate::core::error::TingError;

/// Read plugin metadata from a plugin.json file in the given directory
pub fn read_plugin_metadata(path: &Path) -> Result<PluginMetadata, TingError> {
    let metadata_path = path.join("plugin.json");

    if !metadata_path.exists() {
        return Err(TingError::PluginLoadError(format!(
            "plugin.json not found in: {}", path.display()
        )));
    }

    let content = std::fs::read_to_string(&metadata_path).map_err(|e| {
        TingError::PluginLoadError(format!("Failed to read {}: {}", metadata_path.display(), e))
    })?;

    let json: Value = serde_json::from_str(&content).map_err(|e| {
        TingError::PluginLoadError(format!("Invalid JSON in {}: {}", metadata_path.display(), e))
    })?;

    let name = json["name"]
        .as_str()
        .ok_or_else(|| TingError::PluginLoadError("Missing 'name' field in plugin.json".to_string()))?
        .to_string();

    let version = json["version"]
        .as_str()
        .ok_or_else(|| TingError::PluginLoadError("Missing 'version' field in plugin.json".to_string()))?
        .to_string();

    let plugin_type_str = json["plugin_type"]
        .as_str()
        .ok_or_else(|| TingError::PluginLoadError("Missing 'plugin_type' field in plugin.json".to_string()))?;

    let plugin_type = match plugin_type_str {
        "scraper" => PluginType::Scraper,
        "format" => PluginType::Format,
        "utility" => PluginType::Utility,
        _ => return Err(TingError::PluginLoadError(format!(
            "Invalid plugin_type: {}. Must be 'scraper', 'format', or 'utility'",
            plugin_type_str
        ))),
    };

    let author = json["author"]
        .as_str()
        .ok_or_else(|| TingError::PluginLoadError("Missing 'author' field in plugin.json".to_string()))?
        .to_string();

    let description = json["description"]
        .as_str()
        .ok_or_else(|| TingError::PluginLoadError("Missing 'description' field in plugin.json".to_string()))?
        .to_string();

    let entry_point = json["entry_point"]
        .as_str()
        .ok_or_else(|| TingError::PluginLoadError("Missing 'entry_point' field in plugin.json".to_string()))?
        .to_string();

    // Optional fields
    let id = json.get("id").and_then(|v| v.as_str()).map(|s| s.to_string())
        .unwrap_or_else(|| name.clone());
    let license = json["license"].as_str().map(|s| s.to_string());
    let homepage = json["homepage"].as_str().map(|s| s.to_string());
    let min_core_version = json["min_core_version"].as_str().map(|s| s.to_string());
    let description_en = json["description_en"].as_str().map(|s| s.to_string());

    // Runtime: explicit field or auto-detected from entry_point extension
    let runtime = json["runtime"].as_str().map(|s| s.to_string()).or_else(|| {
        if entry_point.ends_with(".wasm") {
            Some("wasm".to_string())
        } else if entry_point.ends_with(".js") {
            Some("javascript".to_string())
        } else if entry_point.ends_with(".dll") || entry_point.ends_with(".so") || entry_point.ends_with(".dylib") {
            Some("native".to_string())
        } else {
            None
        }
    });

    let supported_extensions = json["supported_extensions"].as_array().map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    });

    // Parse dependencies — supports both simple strings and detailed objects
    let dependencies = parse_dependencies(&json);

    let npm_dependencies = NpmManager::parse_dependencies(&json);

    let permissions = parse_permissions(&json);

    // config_schema: normalize flat format to JSON Schema object format
    let config_schema = normalize_config_schema(json.get("config_schema"));

    Ok(PluginMetadata {
        id,
        name,
        version,
        plugin_type,
        author,
        description,
        description_en,
        license,
        homepage,
        entry_point,
        runtime,
        dependencies,
        npm_dependencies,
        permissions,
        config_schema,
        min_core_version,
        supported_extensions,
    })
}

/// Parse plugin dependencies from plugin.json.
///
/// Supports two formats:
/// 1. Simple strings: `["ffmpeg-utils"]` → version_requirement defaults to "*"
/// 2. Detailed objects: `[{"plugin_name": "ffmpeg-utils", "version_requirement": "^1.0"}]`
fn parse_dependencies(json: &Value) -> Vec<PluginDependency> {
    let deps_array = match json["dependencies"].as_array() {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    deps_array
        .iter()
        .filter_map(|dep| {
            // Simple string format: "plugin-name"
            if let Some(name) = dep.as_str() {
                return Some(PluginDependency::new(name.to_string(), "*".to_string()));
            }
            // Detailed object format: {"plugin_name": "...", "version_requirement": "..."}
            if let (Some(name), Some(ver)) = (
                dep.get("plugin_name").and_then(|v| v.as_str()),
                dep.get("version_requirement").and_then(|v| v.as_str()),
            ) {
                return Some(PluginDependency::new(name.to_string(), ver.to_string()));
            }
            // Legacy format: {"name": "...", "version": "..."} (npm-style, auto-correct)
            if let (Some(name), Some(ver)) = (
                dep.get("name").and_then(|v| v.as_str()),
                dep.get("version").and_then(|v| v.as_str()),
            ) {
                warn!("Dependency '{}' uses npm-style format (name/version). Use plugin_name/version_requirement instead.", name);
                return Some(PluginDependency::new(name.to_string(), ver.to_string()));
            }
            warn!("Invalid dependency entry in plugin.json: {:?}", dep);
            None
        })
        .collect()
}

/// Parse permissions from plugin.json
fn parse_permissions(json: &Value) -> Vec<Permission> {
    let perms_array = match json["permissions"].as_array() {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    perms_array
        .iter()
        .filter_map(|perm| {
            let perm_type = perm["type"].as_str()?;
            let value = perm["value"].as_str()?;

            match perm_type {
                "network_access" => Some(Permission::NetworkAccess(value.to_string())),
                "file_read" => Some(Permission::FileRead(std::path::PathBuf::from(value))),
                "file_write" => Some(Permission::FileWrite(std::path::PathBuf::from(value))),
                "database_read" => Some(Permission::DatabaseRead),
                "database_write" => Some(Permission::DatabaseWrite),
                "event_publish" => Some(Permission::EventPublish),
                _ => {
                    warn!("Unknown permission type: {}", perm_type);
                    None
                }
            }
        })
        .collect()
}

/// Normalize config_schema to JSON Schema object format.
///
/// Accepts both:
/// 1. Proper JSON Schema: `{"type": "object", "properties": {...}}`
/// 2. Flat key-value format: `{"key": {"type": "string", "default": "val"}}`
///
/// Always returns the normalized JSON Schema object format.
fn normalize_config_schema(raw: Option<&Value>) -> Option<Value> {
    let schema = raw?;

    // Already in proper JSON Schema format (has "type" at top level)
    if schema.get("type").is_some() && schema.get("properties").is_some() {
        return Some(schema.clone());
    }

    // Flat format: each top-level key is a property definition
    // Wrap into proper JSON Schema object format
    if let Some(obj) = schema.as_object() {
        if !obj.is_empty() {
            let mut properties = serde_json::Map::new();
            for (key, prop_schema) in obj {
                // Skip non-object values (shouldn't happen in valid flat format)
                if prop_schema.is_object() {
                    properties.insert(key.clone(), prop_schema.clone());
                }
            }
            if !properties.is_empty() {
                return Some(serde_json::json!({
                    "type": "object",
                    "properties": properties
                }));
            }
        }
    }

    // Fallback: return as-is
    Some(schema.clone())
}
