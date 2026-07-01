use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use super::PluginManager;
use crate::plugin::types::{PluginCapability, PluginId, PluginState};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegisteredCapability {
    pub plugin_id: PluginId,
    pub plugin_name: String,
    pub capability: PluginCapability,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchedHttpRoute {
    pub registration: RegisteredCapability,
    #[serde(default)]
    pub params: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchedContentProcessor {
    pub registration: RegisteredCapability,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchedToolProvider {
    pub registration: RegisteredCapability,
    pub tool: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchedTaskHandler {
    pub registration: RegisteredCapability,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchedEventHandler {
    pub registration: RegisteredCapability,
}

impl PluginManager {
    pub async fn list_capabilities(&self) -> Vec<RegisteredCapability> {
        let registry = self.registry.read().await;
        let mut capabilities = Vec::new();

        for entry in registry.values() {
            if entry.state == PluginState::Failed {
                continue;
            }

            let plugin_id = entry.metadata.instance_id();
            let plugin_name = entry.metadata.name.clone();

            capabilities.extend(entry.metadata.effective_capabilities().into_iter().map(
                |capability| RegisteredCapability {
                    plugin_id: plugin_id.clone(),
                    plugin_name: plugin_name.clone(),
                    capability,
                },
            ));
        }

        capabilities
    }

    pub async fn find_capabilities_by_kind(&self, kind: &str) -> Vec<RegisteredCapability> {
        self.list_capabilities()
            .await
            .into_iter()
            .filter(|registration| registration.capability.kind == kind)
            .collect()
    }

    pub async fn find_http_route(&self, method: &str, path: &str) -> Option<MatchedHttpRoute> {
        let candidates = self.find_capabilities_by_kind("http_route").await;
        let path = normalize_route_path(path);
        let mut best_match: Option<(usize, MatchedHttpRoute)> = None;

        for registration in candidates {
            let Some((route_method, route_path)) = http_route_declaration(&registration.capability)
            else {
                continue;
            };

            if route_method != "*" && !route_method.eq_ignore_ascii_case(method) {
                continue;
            }

            let Some(path_match) = match_route_path(&route_path, &path) else {
                continue;
            };

            let should_replace = best_match
                .as_ref()
                .map(|(specificity, _)| path_match.specificity > *specificity)
                .unwrap_or(true);

            if should_replace {
                best_match = Some((
                    path_match.specificity,
                    MatchedHttpRoute {
                        registration,
                        params: path_match.params,
                    },
                ));
            }
        }

        best_match.map(|(_, route)| route)
    }

    pub async fn find_content_processors(
        &self,
        extension: &str,
        operation: Option<&str>,
    ) -> Vec<MatchedContentProcessor> {
        let extension = normalize_extension(extension);
        let candidates = self.find_capabilities_by_kind("content_processor").await;

        candidates
            .into_iter()
            .filter(|registration| {
                content_processor_matches(&registration.capability, extension.as_deref(), operation)
            })
            .map(|registration| MatchedContentProcessor { registration })
            .collect()
    }

    pub async fn find_tool_providers(&self, tool_name: Option<&str>) -> Vec<MatchedToolProvider> {
        let candidates = self.find_capabilities_by_kind("tool_provider").await;

        candidates
            .into_iter()
            .filter_map(|registration| {
                let tool =
                    tool_name.and_then(|name| find_declared_tool(&registration.capability, name));
                if tool_name.is_some() && tool.is_none() {
                    return None;
                }

                Some(MatchedToolProvider { registration, tool })
            })
            .collect()
    }

    pub async fn find_task_handlers(&self, task_type: Option<&str>) -> Vec<MatchedTaskHandler> {
        let candidates = self.find_capabilities_by_kind("task_handler").await;

        candidates
            .into_iter()
            .filter(|registration| task_handler_matches(&registration.capability, task_type))
            .map(|registration| MatchedTaskHandler { registration })
            .collect()
    }

    pub async fn find_event_handlers(&self, event: Option<&str>) -> Vec<MatchedEventHandler> {
        let candidates = self.find_capabilities_by_kind("event_handler").await;

        candidates
            .into_iter()
            .filter(|registration| event_handler_matches(&registration.capability, event))
            .map(|registration| MatchedEventHandler { registration })
            .collect()
    }
}

fn http_route_declaration(capability: &PluginCapability) -> Option<(String, String)> {
    let route = capability.extra.get("route");

    let method = route
        .and_then(|value| value.get("method"))
        .or_else(|| capability.extra.get("method"))
        .and_then(Value::as_str)
        .unwrap_or("*");

    let path = route
        .and_then(|value| value.get("path"))
        .or_else(|| capability.extra.get("path"))
        .and_then(Value::as_str)?;

    Some((method.to_string(), normalize_route_path(path)))
}

fn content_processor_matches(
    capability: &PluginCapability,
    extension: Option<&str>,
    operation: Option<&str>,
) -> bool {
    let matches = capability.extra.get("matches");
    let declared_extensions = matches
        .and_then(|value| value.get("extensions"))
        .or_else(|| capability.extra.get("extensions"));

    let extension_matches = match (extension, declared_extensions) {
        (None, _) => true,
        (Some(_), None) => true,
        (Some(extension), Some(value)) => string_array_contains(value, extension),
    };

    if !extension_matches {
        return false;
    }

    let Some(operation) = operation else {
        return true;
    };

    let declared_operations = capability
        .extra
        .get("operations")
        .or_else(|| matches.and_then(|value| value.get("operations")));

    match declared_operations {
        None => true,
        Some(value) => string_array_contains(value, operation),
    }
}

fn normalize_extension(extension: &str) -> Option<String> {
    let extension = extension.trim().trim_start_matches('.').to_lowercase();
    (!extension.is_empty()).then_some(extension)
}

fn string_array_contains(value: &Value, needle: &str) -> bool {
    if let Some(values) = value.as_array() {
        return values
            .iter()
            .filter_map(Value::as_str)
            .any(|value| value.eq_ignore_ascii_case(needle) || value == "*");
    }

    value
        .as_str()
        .map(|value| value.eq_ignore_ascii_case(needle) || value == "*")
        .unwrap_or(false)
}

fn find_declared_tool(capability: &PluginCapability, tool_name: &str) -> Option<Value> {
    let tools = capability.extra.get("tools")?;

    if let Some(array) = tools.as_array() {
        return array
            .iter()
            .find(|tool| {
                tool.get("name")
                    .or_else(|| tool.get("id"))
                    .and_then(Value::as_str)
                    .map(|name| name == tool_name)
                    .unwrap_or(false)
            })
            .cloned();
    }

    if tools
        .get(tool_name)
        .map(|tool| tool.is_object())
        .unwrap_or(false)
    {
        return tools.get(tool_name).cloned();
    }

    None
}

fn task_handler_matches(capability: &PluginCapability, task_type: Option<&str>) -> bool {
    let Some(task_type) = task_type else {
        return true;
    };

    let declared_tasks = capability
        .extra
        .get("task_types")
        .or_else(|| capability.extra.get("tasks"))
        .or_else(|| capability.extra.get("task_type"));

    declared_tasks
        .map(|value| string_array_contains(value, task_type))
        .unwrap_or(true)
}

fn event_handler_matches(capability: &PluginCapability, event: Option<&str>) -> bool {
    let Some(event) = event else {
        return true;
    };

    let declared_events = capability
        .extra
        .get("events")
        .or_else(|| capability.extra.get("event"));

    declared_events
        .map(|value| string_array_contains(value, event))
        .unwrap_or(true)
}

#[derive(Debug, Clone, PartialEq)]
struct RoutePathMatch {
    params: BTreeMap<String, String>,
    specificity: usize,
}

fn match_route_path(pattern: &str, path: &str) -> Option<RoutePathMatch> {
    let pattern = normalize_route_path(pattern);
    let path = normalize_route_path(path);

    if pattern == path {
        return Some(RoutePathMatch {
            params: BTreeMap::new(),
            specificity: usize::MAX,
        });
    }

    let pattern_segments = route_segments(&pattern);
    let path_segments = route_segments(&path);
    let mut params = BTreeMap::new();
    let mut specificity = 0usize;
    let mut path_index = 0usize;

    for pattern_segment in pattern_segments.iter() {
        if is_wildcard_segment(pattern_segment) {
            params.insert(
                wildcard_name(pattern_segment).to_string(),
                path_segments[path_index..].join("/"),
            );
            return Some(RoutePathMatch {
                params,
                specificity,
            });
        }

        let Some(path_segment) = path_segments.get(path_index) else {
            return None;
        };

        if let Some(dynamic) = parse_dynamic_segment(pattern_segment) {
            let value = dynamic.capture(path_segment)?;
            params.insert(dynamic.name.to_string(), value.to_string());
        } else if pattern_segment == path_segment {
            specificity += pattern_segment.len();
        } else {
            return None;
        }

        path_index += 1;
    }

    if path_index == path_segments.len() {
        Some(RoutePathMatch {
            params,
            specificity,
        })
    } else {
        None
    }
}

fn normalize_route_path(path: &str) -> String {
    let path = path.split('?').next().unwrap_or(path).trim();
    let mut normalized = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    };

    while normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }

    normalized
}

fn route_segments(path: &str) -> Vec<&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn is_wildcard_segment(segment: &str) -> bool {
    segment == "*" || segment.starts_with('*') || segment.starts_with("{*")
}

fn wildcard_name(segment: &str) -> &str {
    segment
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim_start_matches('*')
        .trim_start_matches(':')
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DynamicSegment<'a> {
    name: &'a str,
    suffix: &'a str,
}

impl<'a> DynamicSegment<'a> {
    fn capture<'b>(&self, segment: &'b str) -> Option<&'b str> {
        if self.suffix.is_empty() {
            return (!segment.is_empty()).then_some(segment);
        }

        let value = segment.strip_suffix(self.suffix)?;
        (!value.is_empty()).then_some(value)
    }
}

fn parse_dynamic_segment(segment: &str) -> Option<DynamicSegment<'_>> {
    if let Some(rest) = segment.strip_prefix(':') {
        let (name, suffix) = split_param_suffix(rest);
        return (!name.is_empty()).then_some(DynamicSegment { name, suffix });
    }

    if let Some(rest) = segment.strip_prefix('{') {
        let (name, suffix) = rest.split_once('}')?;
        return (!name.is_empty()).then_some(DynamicSegment { name, suffix });
    }

    None
}

fn split_param_suffix(value: &str) -> (&str, &str) {
    value
        .find('.')
        .map(|index| (&value[..index], &value[index..]))
        .unwrap_or((value, ""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manager::{FailedPlugin, PluginConfig, PluginEntry};
    use crate::plugin::types::{Plugin, PluginMetadata, PluginType};
    use serde_json::json;
    use std::sync::Arc;
    use std::time::Duration;

    fn test_manager() -> PluginManager {
        let temp_dir = tempfile::tempdir().unwrap();
        PluginManager::new(PluginConfig {
            plugin_dir: temp_dir.path().join("plugins"),
            enable_hot_reload: false,
            max_memory_per_plugin: 128 * 1024 * 1024,
            max_execution_time: Duration::from_secs(30),
        })
        .unwrap()
    }

    async fn insert_metadata(manager: &PluginManager, metadata: PluginMetadata) -> PluginId {
        let plugin_id = metadata.instance_id();
        let instance =
            Arc::new(FailedPlugin::new(metadata.clone(), "unused".to_string())) as Arc<dyn Plugin>;

        manager
            .registry
            .write()
            .await
            .insert(plugin_id.clone(), PluginEntry::new(metadata, instance));

        plugin_id
    }

    #[tokio::test]
    async fn capability_registry_lists_declared_capabilities() {
        let manager = test_manager();
        let mut metadata = PluginMetadata::new(
            "metadata-plugin".to_string(),
            "Metadata Plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Scraper,
            "Ting Reader".to_string(),
            "Metadata provider".to_string(),
            "plugin.js".to_string(),
        );
        metadata.capabilities.push(PluginCapability {
            id: "metadata.search".to_string(),
            kind: "metadata_provider".to_string(),
            invoke: Some("search".to_string()),
            extra: BTreeMap::new(),
        });
        let plugin_id = insert_metadata(&manager, metadata).await;

        let capabilities = manager.list_capabilities().await;

        assert_eq!(capabilities.len(), 1);
        assert_eq!(capabilities[0].plugin_id, plugin_id);
        assert_eq!(capabilities[0].capability.kind, "metadata_provider");
    }

    #[tokio::test]
    async fn capability_registry_matches_http_route_with_params() {
        let manager = test_manager();
        let mut metadata = PluginMetadata::new(
            "rss-plugin".to_string(),
            "RSS Plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Ting Reader".to_string(),
            "RSS generator".to_string(),
            "plugin.js".to_string(),
        );
        metadata.capabilities.push(PluginCapability {
            id: "rss.feed".to_string(),
            kind: "http_route".to_string(),
            invoke: Some("generateFeed".to_string()),
            extra: BTreeMap::from([(
                "route".to_string(),
                json!({
                    "method": "GET",
                    "path": "/rss/:library_id.xml"
                }),
            )]),
        });
        let plugin_id = insert_metadata(&manager, metadata).await;

        let matched = manager
            .find_http_route("GET", "/rss/main.xml")
            .await
            .expect("route should match");

        assert_eq!(matched.registration.plugin_id, plugin_id);
        assert_eq!(
            matched.registration.capability.invoke.as_deref(),
            Some("generateFeed")
        );
        assert_eq!(matched.params["library_id"], "main");
        assert!(manager
            .find_http_route("POST", "/rss/main.xml")
            .await
            .is_none());
    }

    #[tokio::test]
    async fn capability_registry_finds_content_processor_by_extension_and_operation() {
        let manager = test_manager();
        let mut metadata = PluginMetadata::new(
            "txt-reader".to_string(),
            "TXT Reader".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Ting Reader".to_string(),
            "TXT reader".to_string(),
            "plugin.js".to_string(),
        );
        metadata.capabilities.push(PluginCapability {
            id: "document.reader".to_string(),
            kind: "content_processor".to_string(),
            invoke: Some("documentInvoke".to_string()),
            extra: BTreeMap::from([
                (
                    "matches".to_string(),
                    json!({
                        "extensions": ["txt", "md"]
                    }),
                ),
                (
                    "operations".to_string(),
                    json!(["probe", "extract_metadata", "read_chunk"]),
                ),
            ]),
        });
        let plugin_id = insert_metadata(&manager, metadata).await;

        let matched = manager
            .find_content_processors(".TXT", Some("read_chunk"))
            .await;

        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].registration.plugin_id, plugin_id);
        assert!(manager
            .find_content_processors("pdf", Some("read_chunk"))
            .await
            .is_empty());
        assert!(manager
            .find_content_processors("txt", Some("render_page"))
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn capability_registry_finds_tool_provider_by_declared_tool() {
        let manager = test_manager();
        let mut metadata = PluginMetadata::new(
            "assistant-tools".to_string(),
            "Assistant Tools".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Ting Reader".to_string(),
            "Assistant tools".to_string(),
            "plugin.js".to_string(),
        );
        metadata.capabilities.push(PluginCapability {
            id: "assistant.tools".to_string(),
            kind: "tool_provider".to_string(),
            invoke: Some("invokeTool".to_string()),
            extra: BTreeMap::from([(
                "tools".to_string(),
                json!([
                    {
                        "name": "book.search",
                        "description": "Search books"
                    },
                    {
                        "name": "library.stats",
                        "description": "Read library stats"
                    }
                ]),
            )]),
        });
        let plugin_id = insert_metadata(&manager, metadata).await;

        let matched = manager.find_tool_providers(Some("book.search")).await;

        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].registration.plugin_id, plugin_id);
        assert_eq!(matched[0].tool.as_ref().unwrap()["name"], "book.search");
        assert!(manager
            .find_tool_providers(Some("missing.tool"))
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn capability_registry_finds_task_handler_by_task_type() {
        let manager = test_manager();
        let mut metadata = PluginMetadata::new(
            "batch-tools".to_string(),
            "Batch Tools".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Ting Reader".to_string(),
            "Batch tools".to_string(),
            "plugin.js".to_string(),
        );
        metadata.capabilities.push(PluginCapability {
            id: "batch.summarize".to_string(),
            kind: "task_handler".to_string(),
            invoke: Some("runTask".to_string()),
            extra: BTreeMap::from([(
                "task_types".to_string(),
                json!(["book.summarize", "library.reindex"]),
            )]),
        });
        let plugin_id = insert_metadata(&manager, metadata).await;

        let matched = manager.find_task_handlers(Some("book.summarize")).await;

        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].registration.plugin_id, plugin_id);
        assert!(manager
            .find_task_handlers(Some("missing.task"))
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn capability_registry_finds_event_handler_by_event_name() {
        let manager = test_manager();
        let mut metadata = PluginMetadata::new(
            "event-tools".to_string(),
            "Event Tools".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Ting Reader".to_string(),
            "Event tools".to_string(),
            "plugin.js".to_string(),
        );
        metadata.capabilities.push(PluginCapability {
            id: "events.all".to_string(),
            kind: "event_handler".to_string(),
            invoke: Some("onEvent".to_string()),
            extra: BTreeMap::from([("events".to_string(), json!(["scan.completed", "*"]))]),
        });
        let plugin_id = insert_metadata(&manager, metadata).await;

        let matched = manager.find_event_handlers(Some("book.added")).await;

        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].registration.plugin_id, plugin_id);
    }
}
