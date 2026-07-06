//! Shared plugin metadata reading
//!
//! Reads and validates plugin.yml/plugin.yaml, producing a PluginMetadata struct.

use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tracing::warn;

use super::super::js::npm::NpmManager;
use super::super::wasm::sandbox::Permission;
use super::{
    LocalizedText, PluginCapability, PluginDependency, PluginMetadata, PluginType,
    ScraperCapabilities,
};
use crate::core::error::TingError;

const PLUGIN_MANIFEST_NAMES: [&str; 2] = ["plugin.yml", "plugin.yaml"];

pub fn find_plugin_manifest(path: &Path) -> Option<PathBuf> {
    PLUGIN_MANIFEST_NAMES
        .iter()
        .map(|name| path.join(name))
        .find(|candidate| candidate.exists())
}

pub fn has_plugin_manifest(path: &Path) -> bool {
    find_plugin_manifest(path).is_some()
}

/// Read plugin metadata from a plugin.yml/plugin.yaml file in the given directory.
pub fn read_plugin_metadata(path: &Path) -> Result<PluginMetadata, TingError> {
    let metadata_path = find_plugin_manifest(path).ok_or_else(|| {
        TingError::PluginLoadError(format!("plugin.yml not found in: {}", path.display()))
    })?;

    let content = std::fs::read_to_string(&metadata_path).map_err(|e| {
        TingError::PluginLoadError(format!("Failed to read {}: {}", metadata_path.display(), e))
    })?;

    parse_plugin_metadata_content(&content, &metadata_path.display().to_string())
}

pub fn parse_plugin_metadata_content(
    content: &str,
    manifest_label: &str,
) -> Result<PluginMetadata, TingError> {
    let json: Value = serde_yaml::from_str(content).map_err(|e| {
        TingError::PluginLoadError(format!("Invalid YAML in {}: {}", manifest_label, e))
    })?;

    parse_plugin_metadata_value(json, manifest_label)
}

pub fn parse_plugin_metadata_value(
    json: Value,
    manifest_label: &str,
) -> Result<PluginMetadata, TingError> {
    let name = json["name"]
        .as_str()
        .ok_or_else(|| {
            TingError::PluginLoadError(format!("Missing 'name' field in {}", manifest_label))
        })?
        .to_string();

    let version = json["version"]
        .as_str()
        .ok_or_else(|| {
            TingError::PluginLoadError(format!("Missing 'version' field in {}", manifest_label))
        })?
        .to_string();

    let author = json["author"]
        .as_str()
        .ok_or_else(|| {
            TingError::PluginLoadError(format!("Missing 'author' field in {}", manifest_label))
        })?
        .to_string();

    let description = localized_string(&json["description"], "zh").ok_or_else(|| {
        TingError::PluginLoadError(format!("Missing 'description' field in {}", manifest_label))
    })?;

    let entry_point = json["entry_point"]
        .as_str()
        .ok_or_else(|| {
            TingError::PluginLoadError(format!("Missing 'entry_point' field in {}", manifest_label))
        })?
        .to_string();

    // Optional fields
    let id = json
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| name.clone());
    let license = json["license"].as_str().map(|s| s.to_string());
    let repo = json["repo"].as_str().map(|s| s.to_string());
    let min_core_version = json["min_core_version"].as_str().map(|s| s.to_string());
    let min_flutter_version = json["min_flutter_version"].as_str().map(|s| s.to_string());
    let mut description_i18n = localized_text_from_value(&json["description"]).unwrap_or_default();
    if let Some(text) = json["description_en"]
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        description_i18n
            .entry("en".to_string())
            .or_insert_with(|| text.to_string());
    }
    description_i18n
        .entry("zh".to_string())
        .or_insert_with(|| description.clone());

    // Runtime: explicit field or auto-detected from entry_point extension
    let runtime = json["runtime"].as_str().map(|s| s.to_string()).or_else(|| {
        if entry_point.ends_with(".wasm") {
            Some("wasm".to_string())
        } else if entry_point.ends_with(".js") {
            Some("javascript".to_string())
        } else if entry_point.ends_with(".dll")
            || entry_point.ends_with(".so")
            || entry_point.ends_with(".dylib")
        {
            Some("native".to_string())
        } else {
            None
        }
    });

    let capabilities = parse_capabilities(&json, manifest_label)?;
    let plugin_type = infer_plugin_type(&capabilities);
    let supported_extensions = derive_supported_extensions(&capabilities);
    let scraper = derive_scraper_capabilities(&capabilities, manifest_label)?;

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
        description_i18n,
        license,
        repo,
        entry_point,
        runtime,
        dependencies,
        npm_dependencies,
        permissions,
        config_schema,
        min_core_version,
        min_flutter_version,
        supported_extensions,
        scraper,
        capabilities,
    })
}

fn parse_capabilities(
    json: &Value,
    manifest_label: &str,
) -> Result<Vec<PluginCapability>, TingError> {
    let Some(value) = json.get("capabilities").filter(|value| !value.is_null()) else {
        return Err(TingError::PluginLoadError(format!(
            "Missing required 'capabilities' declaration in {}",
            manifest_label
        )));
    };

    let capabilities =
        serde_json::from_value::<Vec<PluginCapability>>(value.clone()).map_err(|e| {
            TingError::PluginLoadError(format!(
                "Invalid 'capabilities' declaration in {}: {}",
                manifest_label, e
            ))
        })?;
    if capabilities.is_empty() {
        return Err(TingError::PluginLoadError(format!(
            "'capabilities' must contain at least one entry in {}",
            manifest_label
        )));
    }
    Ok(capabilities)
}

fn infer_plugin_type(capabilities: &[PluginCapability]) -> PluginType {
    if capabilities
        .iter()
        .any(|capability| capability.kind == "metadata_provider")
    {
        return PluginType::Scraper;
    }
    if capabilities.iter().any(|capability| {
        capability.kind == "format_handler" || capability.kind == "content_processor"
    }) {
        return PluginType::Format;
    }
    PluginType::Utility
}

fn derive_supported_extensions(capabilities: &[PluginCapability]) -> Option<Vec<String>> {
    let mut extensions = Vec::new();

    for capability in capabilities {
        if capability.kind == "format_handler" || capability.kind == "content_processor" {
            for extension in capability_extensions(capability) {
                push_unique(&mut extensions, extension);
            }
        }
    }

    (!extensions.is_empty()).then_some(extensions)
}

fn capability_extensions(capability: &PluginCapability) -> Vec<String> {
    let direct = capability.extra.get("extensions");
    let matches = capability.extra.get("matches");
    let nested = matches.and_then(|value| value.get("extensions"));

    direct
        .or(nested)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(|extension| extension.trim().trim_start_matches('.').to_lowercase())
                .filter(|extension| !extension.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn derive_scraper_capabilities(
    capabilities: &[PluginCapability],
    manifest_label: &str,
) -> Result<Option<ScraperCapabilities>, TingError> {
    let mut first = None;

    for capability in capabilities
        .iter()
        .filter(|capability| capability.kind == "metadata_provider")
    {
        let scraper = scraper_capabilities_from_capability(capability, manifest_label)?;
        validate_scraper_capabilities(&scraper, &capability.id)?;
        if first.is_none() {
            first = Some(scraper);
        }
    }

    Ok(first)
}

fn scraper_capabilities_from_capability(
    capability: &PluginCapability,
    manifest_label: &str,
) -> Result<ScraperCapabilities, TingError> {
    let object: Map<String, Value> = capability.extra.clone().into_iter().collect();
    let raw = object
        .get("metadata")
        .cloned()
        .unwrap_or_else(|| Value::Object(object));
    let normalized = normalize_scraper_capabilities(&raw);
    serde_json::from_value::<ScraperCapabilities>(normalized).map_err(|e| {
        TingError::PluginLoadError(format!(
            "Invalid metadata_provider capability '{}' in {}: {}",
            capability.id, manifest_label, e
        ))
    })
}

fn validate_scraper_capabilities(
    capabilities: &ScraperCapabilities,
    capability_id: &str,
) -> Result<(), TingError> {
    if capabilities.search_fields.is_empty() {
        return Err(TingError::PluginLoadError(format!(
            "metadata_provider capability '{}' must declare at least one search field",
            capability_id
        )));
    }

    if capabilities.auto_scrape {
        let has_required_title = capabilities.search_fields.iter().any(|field| {
            field.required
                && (field.key == "title"
                    || field.key == "query"
                    || field.default_from.as_deref() == Some("book.title"))
        });

        if !has_required_title {
            return Err(TingError::PluginLoadError(format!(
                "metadata_provider capability '{}' with auto_scrape=true must declare a required title search field",
                capability_id
            )));
        }
    }

    Ok(())
}

/// Parse plugin dependencies from plugin.yml.
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
            warn!("Invalid dependency entry in plugin.yml: {:?}", dep);
            None
        })
        .collect()
}

/// Parse permissions from plugin.yml
fn parse_permissions(json: &Value) -> Vec<Permission> {
    let perms_array = match json["permissions"].as_array() {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    perms_array
        .iter()
        .filter_map(|perm| {
            let perm_type = perm["type"].as_str()?;
            let value = perm.get("value").and_then(Value::as_str);

            match perm_type {
                "network_access" => value
                    .map(|value| Permission::NetworkAccess(value.to_string()))
                    .or_else(|| {
                        warn!("Permission '{}' requires a value", perm_type);
                        None
                    }),
                "file_read" => value
                    .map(|value| Permission::FileRead(std::path::PathBuf::from(value)))
                    .or_else(|| {
                        warn!("Permission '{}' requires a value", perm_type);
                        None
                    }),
                "file_write" => value
                    .map(|value| Permission::FileWrite(std::path::PathBuf::from(value)))
                    .or_else(|| {
                        warn!("Permission '{}' requires a value", perm_type);
                        None
                    }),
                "database_read" => Some(Permission::DatabaseRead),
                "database_write" => Some(Permission::DatabaseWrite),
                "books_read" => Some(Permission::BooksRead),
                "chapters_read" => Some(Permission::ChaptersRead),
                "progress_read" => Some(Permission::ProgressRead),
                "media_read" => Some(Permission::MediaRead),
                "media_read_url" => Some(Permission::MediaReadUrl),
                "plugin_route_sign" => Some(Permission::PluginRouteSign),
                "metadata_write" => Some(Permission::MetadataWrite),
                "task_create" => Some(Permission::TaskCreate),
                "cache_read" => Some(Permission::CacheRead),
                "cache_write" => Some(Permission::CacheWrite),
                "playlists_read" => Some(Permission::PlaylistsRead),
                "playlists_write" => Some(Permission::PlaylistsWrite),
                "favorites_read" => Some(Permission::FavoritesRead),
                "favorites_write" => Some(Permission::FavoritesWrite),
                "user_settings_read" => Some(Permission::UserSettingsRead),
                "user_settings_write" => Some(Permission::UserSettingsWrite),
                "event_publish" => Some(Permission::EventPublish),
                "event_subscribe" => value
                    .map(|value| Permission::EventSubscribe(value.to_string()))
                    .or_else(|| {
                        warn!("Permission '{}' requires a value", perm_type);
                        None
                    }),
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
        let mut normalized = schema.clone();
        normalize_config_schema_i18n(&mut normalized);
        return Some(normalized);
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
                let mut normalized = serde_json::json!({
                    "type": "object",
                    "properties": properties
                });
                normalize_config_schema_i18n(&mut normalized);
                return Some(normalized);
            }
        }
    }

    // Fallback: return as-is
    let mut normalized = schema.clone();
    normalize_config_schema_i18n(&mut normalized);
    Some(normalized)
}

fn localized_string(value: &Value, preferred: &str) -> Option<String> {
    if let Some(text) = value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return Some(text.to_string());
    }

    let object = value.as_object()?;
    localized_object_string(object, preferred)
}

fn localized_object_string(object: &Map<String, Value>, preferred: &str) -> Option<String> {
    let preferred_keys: &[&str] = if preferred.starts_with("en") {
        &["en", "en_US", "en-US"]
    } else {
        &["zh", "zh_CN", "zh-CN", "zh_Hans", "zh-Hans"]
    };
    let fallback_keys: &[&str] = if preferred.starts_with("en") {
        &["zh", "zh_CN", "zh-CN", "zh_Hans", "zh-Hans"]
    } else {
        &["en", "en_US", "en-US"]
    };

    preferred_keys
        .iter()
        .chain(fallback_keys.iter())
        .find_map(|key| {
            object
                .get(*key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(ToString::to_string)
        })
}

fn localized_text_from_value(value: &Value) -> Option<LocalizedText> {
    if let Some(text) = value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return Some(BTreeMap::from([("zh".to_string(), text.to_string())]));
    }

    let object = value.as_object()?;
    let localized: LocalizedText = object
        .iter()
        .filter_map(|(key, value)| {
            value
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(|text| (normalize_locale_key(key), text.to_string()))
        })
        .collect();

    if localized.is_empty() {
        None
    } else {
        Some(localized)
    }
}

fn normalize_locale_key(key: &str) -> String {
    match key.replace('_', "-").to_lowercase().as_str() {
        "zh" | "zh-cn" | "zh-hans" => "zh".to_string(),
        "en" | "en-us" | "en-gb" => "en".to_string(),
        other => other.to_string(),
    }
}

fn normalize_localized_property(
    object: &mut Map<String, Value>,
    property: &str,
    i18n_property: &str,
) {
    let Some(raw) = object.get(property).cloned() else {
        return;
    };

    if let Some(localized) = localized_text_from_value(&raw) {
        if let Ok(value) = serde_json::to_value(localized) {
            object.insert(i18n_property.to_string(), value);
        }
        if let Some(fallback) = localized_string(&raw, "zh") {
            object.insert(property.to_string(), Value::String(fallback));
        }
    }
}

fn normalize_config_schema_i18n(schema: &mut Value) {
    let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) else {
        return;
    };

    for prop in properties.values_mut() {
        let Some(prop_object) = prop.as_object_mut() else {
            continue;
        };
        normalize_localized_property(prop_object, "title", "title_i18n");
        normalize_localized_property(prop_object, "label", "label_i18n");
        normalize_localized_property(prop_object, "description", "description_i18n");
        normalize_localized_property(prop_object, "placeholder", "placeholder_i18n");
    }
}

fn normalize_scraper_capabilities(raw: &Value) -> Value {
    let mut scraper = raw.clone();
    let Some(scraper_object) = scraper.as_object_mut() else {
        return scraper;
    };

    if let Some(fields) = scraper_object.get_mut("search_fields") {
        normalize_scraper_search_fields(fields);
    }

    let mut result_field_labels =
        parse_result_field_labels(scraper_object.get("result_field_labels"));

    if let Some(result_fields) = scraper_object.get_mut("result_fields") {
        normalize_scraper_result_fields(result_fields, &mut result_field_labels);
    }

    if !result_field_labels.is_empty() {
        if let Ok(value) = serde_json::to_value(result_field_labels) {
            scraper_object.insert("result_field_labels".to_string(), value);
        }
    }

    scraper
}

fn normalize_scraper_search_fields(fields: &mut Value) {
    let Some(fields) = fields.as_array_mut() else {
        return;
    };

    for field in fields {
        let Some(field_object) = field.as_object_mut() else {
            continue;
        };

        normalize_localized_property(field_object, "label", "label_i18n");
        normalize_localized_property(field_object, "placeholder", "placeholder_i18n");

        let has_label = field_object
            .get("label")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|label| !label.is_empty());
        if !has_label {
            let fallback = field_object
                .get("key")
                .and_then(Value::as_str)
                .map(humanize_field_key)
                .unwrap_or_else(|| "Field".to_string());
            field_object.insert("label".to_string(), Value::String(fallback));
        }
    }
}

fn normalize_scraper_result_fields(
    fields: &mut Value,
    labels: &mut BTreeMap<String, LocalizedText>,
) {
    let Some(items) = fields.as_array() else {
        return;
    };

    let mut normalized_items = Vec::new();
    for item in items {
        if let Some(key) = item.as_str().map(str::trim).filter(|key| !key.is_empty()) {
            normalized_items.push(Value::String(key.to_string()));
            continue;
        }

        let Some(object) = item.as_object() else {
            continue;
        };
        let Some(key) = object
            .get("key")
            .or_else(|| object.get("name"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|key| !key.is_empty())
        else {
            continue;
        };

        normalized_items.push(Value::String(key.to_string()));

        let label = object.get("label").or_else(|| object.get("label_i18n"));
        if let Some(label) = label.and_then(localized_text_from_value) {
            insert_result_field_label(labels, key, label);
        }
    }

    *fields = Value::Array(normalized_items);
}

fn parse_result_field_labels(raw: Option<&Value>) -> BTreeMap<String, LocalizedText> {
    let mut labels = BTreeMap::new();
    let Some(object) = raw.and_then(Value::as_object) else {
        return labels;
    };

    for (key, value) in object {
        if let Some(label) = localized_text_from_value(value) {
            insert_result_field_label(&mut labels, key, label);
        }
    }

    labels
}

fn insert_result_field_label(
    labels: &mut BTreeMap<String, LocalizedText>,
    key: &str,
    label: LocalizedText,
) {
    let key = key.trim();
    if key.is_empty() {
        return;
    }
    labels.insert(key.to_string(), label.clone());
    labels
        .entry(normalize_scraper_field_key(key))
        .or_insert(label);
}

fn normalize_scraper_field_key(key: &str) -> String {
    let mut normalized = String::new();
    for ch in key.chars() {
        if ch.is_ascii_uppercase() {
            normalized.push('_');
            normalized.push(ch.to_ascii_lowercase());
        } else {
            normalized.push(ch);
        }
    }

    match normalized.to_ascii_lowercase().as_str() {
        "intro" => "description".to_string(),
        "published_year" => "year".to_string(),
        other => other.to_string(),
    }
}

fn humanize_field_key(key: &str) -> String {
    key.replace('_', " ")
        .split_whitespace()
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut word = first.to_uppercase().collect::<String>();
                    word.push_str(chars.as_str());
                    word
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_declared_capabilities() {
        let manifest = r#"
id: ai-assistant
name: AI Assistant
version: 1.0.0
runtime: javascript
entry_point: assistant.js
author: Ting Reader
description: AI assistant
capabilities:
  - id: assistant.float
    kind: ui_extension
    invoke: assistant.open
    slot: global.floating_action
    icon: message-circle
    render:
      mode: web_container
      entry: ui/assistant.html
"#;

        let metadata = parse_plugin_metadata_content(manifest, "test-plugin.yml").unwrap();

        assert_eq!(metadata.capabilities.len(), 1);
        assert_eq!(metadata.capabilities[0].id, "assistant.float");
        assert_eq!(metadata.capabilities[0].kind, "ui_extension");
        assert_eq!(
            metadata.capabilities[0].invoke.as_deref(),
            Some("assistant.open")
        );
        assert_eq!(
            metadata.capabilities[0].extra["slot"],
            serde_json::json!("global.floating_action")
        );
        assert_eq!(
            metadata.capabilities[0].extra["icon"],
            serde_json::json!("message-circle")
        );
        assert_eq!(
            metadata.capabilities[0].extra["render"]["mode"],
            serde_json::json!("web_container")
        );
        assert_eq!(metadata.plugin_type, PluginType::Utility);
    }

    #[test]
    fn parses_host_gateway_permissions_without_values() {
        let manifest = r#"
id: rss-feed
name: RSS Feed
version: 1.0.0
runtime: javascript
entry_point: rss.js
author: Ting Reader
description: RSS feed plugin
capabilities:
  - id: rss.feed
    kind: http_route
    invoke: generate
permissions:
  - type: books_read
  - type: chapters_read
  - type: progress_read
  - type: media_read_url
  - type: plugin_route_sign
  - type: network_access
    value: example.com
"#;

        let metadata = parse_plugin_metadata_content(manifest, "test-plugin.yml").unwrap();

        assert!(metadata.permissions.contains(&Permission::BooksRead));
        assert!(metadata.permissions.contains(&Permission::ChaptersRead));
        assert!(metadata.permissions.contains(&Permission::ProgressRead));
        assert!(metadata.permissions.contains(&Permission::MediaReadUrl));
        assert!(metadata.permissions.contains(&Permission::PluginRouteSign));
        assert!(metadata
            .permissions
            .contains(&Permission::NetworkAccess("example.com".to_string())));
    }

    #[test]
    fn derives_metadata_provider_declarations_from_capability() {
        let manifest = r#"
id: metadata-source
name: Metadata Source
version: 1.0.0
runtime: javascript
entry_point: plugin.js
author: Ting Reader
description: Metadata source
capabilities:
  - id: metadata.search
    kind: metadata_provider
    invoke: search
    auto_scrape: true
    search_fields:
      - key: title
        label: Title
        required: true
        default_from: book.title
    result_fields:
      - key: title
        label: Title
"#;

        let metadata = parse_plugin_metadata_content(manifest, "test-plugin.yml").unwrap();

        assert_eq!(metadata.plugin_type, PluginType::Scraper);
        assert_eq!(metadata.supported_extensions, None);
        let scraper = metadata.scraper.unwrap();
        assert_eq!(scraper.search_fields.len(), 1);
        assert_eq!(scraper.result_fields, vec!["title"]);
    }
}
