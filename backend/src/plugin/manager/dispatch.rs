use serde_json::Value;
use std::time::{Duration, Instant};

use super::enums::{FormatMethod, ScraperMethod};
use super::PluginManager;
use crate::core::error::{Result, TingError};
use crate::plugin::js::JavaScriptPluginWrapper;
use crate::plugin::native::NativePlugin;
use crate::plugin::scraper::ScraperPlugin;
use crate::plugin::types::PluginId;
use crate::plugin::wasm::WasmPlugin;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfmpegToolPaths {
    pub ffmpeg: String,
    pub ffprobe: String,
}

impl PluginManager {
    /// Invoke a plugin method through the runtime-neutral compatibility path.
    pub async fn invoke_plugin(&self, id: &PluginId, method: &str, params: Value) -> Result<Value> {
        let instance = {
            let registry = self.registry.read().await;
            let entry = registry
                .get(id)
                .ok_or_else(|| TingError::PluginNotFound(id.clone()))?;
            entry.instance.clone()
        };

        let started_at = Instant::now();
        let result =
            if let Some(wrapper) = instance.as_any().downcast_ref::<JavaScriptPluginWrapper>() {
                wrapper.call_function(method, params).await
            } else if let Some(native) = instance.as_any().downcast_ref::<NativePlugin>() {
                native.call_method(method, params).await
            } else if let Some(wasm_plugin) = instance.as_any().downcast_ref::<WasmPlugin>() {
                wasm_plugin.invoke_json(method, params).await
            } else {
                Err(TingError::PluginExecutionError(format!(
                    "Plugin {} does not support unified invoke",
                    id
                )))
            };

        self.record_plugin_call(id, started_at.elapsed(), result.as_ref().err())
            .await;
        result
    }

    /// Call a scraper plugin method
    pub async fn call_scraper(
        &self,
        id: &PluginId,
        method: ScraperMethod,
        params: Value,
    ) -> Result<Value> {
        let (instance, method_name) = {
            let registry = self.registry.read().await;
            let entry = registry
                .get(id)
                .ok_or_else(|| TingError::PluginNotFound(id.clone()))?;
            let metadata_provider = entry
                .metadata
                .effective_capabilities()
                .into_iter()
                .find(|capability| capability.kind == "metadata_provider")
                .ok_or_else(|| {
                    TingError::PluginExecutionError(format!(
                        "Plugin {} does not declare metadata_provider capability",
                        id
                    ))
                })?;
            let method_name = match method {
                ScraperMethod::Search => metadata_provider
                    .invoke
                    .clone()
                    .unwrap_or_else(|| "search".to_string()),
                ScraperMethod::GetChapterList => "getChapters".to_string(),
                ScraperMethod::GetChapterDetail => "getChapterDetail".to_string(),
                ScraperMethod::DownloadCover => "downloadCover".to_string(),
                ScraperMethod::GetAudioUrl => "getAudioUrl".to_string(),
            };
            (entry.instance.clone(), method_name)
        };

        let started_at = Instant::now();
        let result =
            if let Some(wrapper) = instance.as_any().downcast_ref::<JavaScriptPluginWrapper>() {
                wrapper.call_function(&method_name, params).await
            } else if let Some(wasm_plugin) = instance.as_any().downcast_ref::<WasmPlugin>() {
                match method {
                    ScraperMethod::Search => {
                        let result = wasm_plugin.search_with_params(params).await?;
                        Ok(serde_json::to_value(result).map_err(|e| {
                            TingError::PluginExecutionError(format!("Serialization error: {}", e))
                        })?)
                    }
                    ScraperMethod::GetChapterList => {
                        let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let result = wasm_plugin.get_chapters(id).await?;
                        Ok(serde_json::to_value(result).map_err(|e| {
                            TingError::PluginExecutionError(format!("Serialization error: {}", e))
                        })?)
                    }
                    ScraperMethod::DownloadCover => {
                        let url = params.get("url").and_then(|v| v.as_str()).unwrap_or("");
                        let result = wasm_plugin.download_cover(url).await?;
                        use base64::Engine;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(result);
                        Ok(serde_json::json!({ "data": b64 }))
                    }
                    ScraperMethod::GetAudioUrl => {
                        let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let result = wasm_plugin.get_audio_url(id).await?;
                        Ok(serde_json::json!({ "url": result }))
                    }
                    _ => Err(TingError::PluginExecutionError(format!(
                        "Unsupported method for WASM: {:?}",
                        method
                    ))),
                }
            } else {
                Err(TingError::PluginExecutionError(
                    "Native scrapers not supported yet".to_string(),
                ))
            };

        self.record_plugin_call(id, started_at.elapsed(), result.as_ref().err())
            .await;
        result
    }

    /// Call a format plugin method
    pub async fn call_format(
        &self,
        id: &PluginId,
        method: FormatMethod,
        params: Value,
    ) -> Result<Value> {
        let instance = {
            let registry = self.registry.read().await;
            let entry = registry
                .get(id)
                .ok_or_else(|| TingError::PluginNotFound(id.clone()))?;
            let has_format_handler = entry
                .metadata
                .effective_capabilities()
                .iter()
                .any(|capability| capability.kind == "format_handler");
            if !has_format_handler {
                return Err(TingError::PluginExecutionError(format!(
                    "Plugin {} does not declare format_handler capability",
                    id
                )));
            }
            entry.instance.clone()
        };

        let method_name = match method {
            FormatMethod::Detect => "detect",
            FormatMethod::ExtractMetadata => "extract_metadata",
            FormatMethod::Decode => "decode",
            FormatMethod::Encode => "encode",
            FormatMethod::Decrypt => "decrypt",
            FormatMethod::DecryptChunk => "decrypt_chunk",
            FormatMethod::GetMetadataReadSize => "get_metadata_read_size",
            FormatMethod::GetDecryptionPlan => "get_decryption_plan",
            FormatMethod::GetStreamUrl => "get_stream_url",
            FormatMethod::WriteMetadata => "write_metadata",
        };

        let started_at = Instant::now();
        let result = if let Some(native) = instance.as_any().downcast_ref::<NativePlugin>() {
            native.call_method(method_name, params).await
        } else if let Some(wrapper) = instance.as_any().downcast_ref::<JavaScriptPluginWrapper>() {
            wrapper.call_function(method_name, params).await
        } else {
            Err(TingError::PluginExecutionError(
                "Unknown plugin type".to_string(),
            ))
        };

        self.record_plugin_call(id, started_at.elapsed(), result.as_ref().err())
            .await;
        result
    }

    /// Helper to find and call an FFmpeg tool provider by capability.
    pub async fn get_ffmpeg_path(&self) -> Option<String> {
        self.call_tool_path("ffmpeg.get_path").await
    }

    pub async fn get_ffprobe_path(&self) -> Option<String> {
        self.call_tool_path("ffprobe.get_path").await
    }

    pub async fn get_ffmpeg_tool_paths(&self) -> Option<FfmpegToolPaths> {
        let ffmpeg = self.get_ffmpeg_path().await?;
        let ffprobe = self.get_ffprobe_path().await?;

        Some(FfmpegToolPaths { ffmpeg, ffprobe })
    }

    async fn call_tool_path(&self, tool_name: &str) -> Option<String> {
        let mut providers = self.find_tool_providers(Some(tool_name)).await;
        providers.sort_by(|left, right| {
            left.registration
                .plugin_id
                .cmp(&right.registration.plugin_id)
                .then_with(|| {
                    left.registration
                        .capability
                        .id
                        .cmp(&right.registration.capability.id)
                })
        });

        for provider in providers {
            let invoke = provider
                .registration
                .capability
                .invoke
                .clone()
                .unwrap_or_else(|| "execute".to_string());
            let result = self
                .invoke_plugin(
                    &provider.registration.plugin_id,
                    &invoke,
                    serde_json::json!({
                        "name": tool_name,
                        "tool": tool_name,
                        "tool_name": tool_name,
                    }),
                )
                .await;

            match result {
                Ok(value) => {
                    if let Some(path) = tool_path_from_value(&value) {
                        return Some(path);
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        plugin_id = %provider.registration.plugin_id,
                        tool_name,
                        error = %error,
                        "FFmpeg tool provider call failed"
                    );
                }
            }
        }

        None
    }

    pub(crate) async fn record_plugin_call(
        &self,
        id: &PluginId,
        duration: Duration,
        error: Option<&TingError>,
    ) {
        let mut registry = self.registry.write().await;
        let Some(entry) = registry.get_mut(id) else {
            return;
        };

        match error {
            Some(error) => {
                let error_type = error.to_string();
                entry.stats.record_failure(Some(error_type.as_str()));
            }
            None => {
                let duration_ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
                entry.stats.record_success(duration_ms);
            }
        }
    }
}

fn tool_path_from_value(value: &Value) -> Option<String> {
    value
        .get("path")
        .and_then(Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manager::{FailedPlugin, PluginConfig, PluginEntry};
    use crate::plugin::types::{Plugin, PluginMetadata, PluginType};
    use std::sync::Arc;

    #[tokio::test]
    async fn record_plugin_call_updates_listed_stats() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manager = PluginManager::new(PluginConfig {
            plugin_dir: temp_dir.path().join("plugins"),
            enable_hot_reload: false,
            max_memory_per_plugin: 128 * 1024 * 1024,
            max_execution_time: Duration::from_secs(30),
        })
        .unwrap();

        let metadata = PluginMetadata::new(
            "stats-plugin".to_string(),
            "Stats Plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Ting Reader".to_string(),
            "Stats plugin".to_string(),
            "plugin.js".to_string(),
        );
        let plugin_id = metadata.instance_id();
        let instance =
            Arc::new(FailedPlugin::new(metadata.clone(), "unused".to_string())) as Arc<dyn Plugin>;

        manager
            .registry
            .write()
            .await
            .insert(plugin_id.clone(), PluginEntry::new(metadata, instance));

        manager
            .record_plugin_call(&plugin_id, Duration::from_millis(25), None)
            .await;
        manager
            .record_plugin_call(
                &plugin_id,
                Duration::from_millis(5),
                Some(&TingError::PluginExecutionError("boom".to_string())),
            )
            .await;

        let plugins = manager.list_plugins().await;
        let stats_plugin = plugins
            .iter()
            .find(|plugin| plugin.id == plugin_id)
            .expect("stats plugin should be listed");

        assert_eq!(stats_plugin.total_calls, 2);
        assert_eq!(stats_plugin.successful_calls, 1);
        assert_eq!(stats_plugin.failed_calls, 1);
    }

    #[tokio::test]
    async fn invoke_plugin_records_failure_for_unsupported_runtime() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manager = PluginManager::new(PluginConfig {
            plugin_dir: temp_dir.path().join("plugins"),
            enable_hot_reload: false,
            max_memory_per_plugin: 128 * 1024 * 1024,
            max_execution_time: Duration::from_secs(30),
        })
        .unwrap();

        let metadata = PluginMetadata::new(
            "unsupported-plugin".to_string(),
            "Unsupported Plugin".to_string(),
            "1.0.0".to_string(),
            PluginType::Utility,
            "Ting Reader".to_string(),
            "Unsupported plugin".to_string(),
            "plugin.js".to_string(),
        );
        let plugin_id = metadata.instance_id();
        let instance =
            Arc::new(FailedPlugin::new(metadata.clone(), "unused".to_string())) as Arc<dyn Plugin>;

        manager
            .registry
            .write()
            .await
            .insert(plugin_id.clone(), PluginEntry::new(metadata, instance));

        let result = manager
            .invoke_plugin(&plugin_id, "anything", serde_json::json!({}))
            .await;

        assert!(result.is_err());

        let plugins = manager.list_plugins().await;
        let plugin = plugins
            .iter()
            .find(|plugin| plugin.id == plugin_id)
            .expect("plugin should be listed");

        assert_eq!(plugin.total_calls, 1);
        assert_eq!(plugin.failed_calls, 1);
    }
}
