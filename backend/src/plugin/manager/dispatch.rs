use serde_json::Value;

use crate::core::error::{Result, TingError};
use crate::plugin::types::PluginId;
use crate::plugin::scraper::ScraperPlugin;
use crate::plugin::wasm::WasmPlugin;
use crate::plugin::native::NativePlugin;
use crate::plugin::js::JavaScriptPluginWrapper;
use super::PluginManager;
use super::enums::{ScraperMethod, FormatMethod, UtilityMethod};

impl PluginManager {
    /// Call a scraper plugin method
    pub async fn call_scraper(&self, id: &PluginId, method: ScraperMethod, params: Value) -> Result<Value> {
        let instance = {
            let registry = self.registry.read().await;
            let entry = registry.get(id).ok_or_else(|| TingError::PluginNotFound(id.clone()))?;
            if entry.metadata.plugin_type != crate::plugin::types::PluginType::Scraper {
                return Err(TingError::PluginExecutionError(format!("Plugin {} is not a scraper", id)));
            }
            entry.instance.clone()
        };

        let method_name = match method {
            ScraperMethod::Search => "search",
            ScraperMethod::GetDetail => "getDetail",
            ScraperMethod::GetChapterList => "getChapters",
            ScraperMethod::GetChapterDetail => "getChapterDetail",
            ScraperMethod::DownloadCover => "downloadCover",
            ScraperMethod::GetAudioUrl => "getAudioUrl",
        };

        if let Some(wrapper) = instance.as_any().downcast_ref::<JavaScriptPluginWrapper>() {
            wrapper.call_function(method_name, params).await
        } else if let Some(wasm_plugin) = instance.as_any().downcast_ref::<WasmPlugin>() {
            match method {
                ScraperMethod::Search => {
                    let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
                    let author = params.get("author").and_then(|v| v.as_str());
                    let narrator = params.get("narrator").and_then(|v| v.as_str());
                    let page = params.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                    let result = wasm_plugin.search(query, author, narrator, page).await?;
                    Ok(serde_json::to_value(result).map_err(|e| TingError::PluginExecutionError(format!("Serialization error: {}", e)))?)
                },
                ScraperMethod::GetDetail => {
                    let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let result = wasm_plugin.get_detail(id).await?;
                    Ok(serde_json::to_value(result).map_err(|e| TingError::PluginExecutionError(format!("Serialization error: {}", e)))?)
                },
                ScraperMethod::GetChapterList => {
                    let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let result = wasm_plugin.get_chapters(id).await?;
                    Ok(serde_json::to_value(result).map_err(|e| TingError::PluginExecutionError(format!("Serialization error: {}", e)))?)
                },
                ScraperMethod::DownloadCover => {
                    let url = params.get("url").and_then(|v| v.as_str()).unwrap_or("");
                    let result = wasm_plugin.download_cover(url).await?;
                    use base64::Engine;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(result);
                    Ok(serde_json::json!({ "data": b64 }))
                },
                ScraperMethod::GetAudioUrl => {
                    let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let result = wasm_plugin.get_audio_url(id).await?;
                    Ok(serde_json::json!({ "url": result }))
                },
                _ => Err(TingError::PluginExecutionError(format!("Unsupported method for WASM: {:?}", method))),
            }
        } else {
            Err(TingError::PluginExecutionError("Native scrapers not supported yet".to_string()))
        }
    }

    /// Call a format plugin method
    pub async fn call_format(&self, id: &PluginId, method: FormatMethod, params: Value) -> Result<Value> {
        let instance = {
            let registry = self.registry.read().await;
            let entry = registry.get(id).ok_or_else(|| TingError::PluginNotFound(id.clone()))?;
            if entry.metadata.plugin_type != crate::plugin::types::PluginType::Format {
                return Err(TingError::PluginExecutionError(format!("Plugin {} is not a format plugin", id)));
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

        if let Some(native) = instance.as_any().downcast_ref::<NativePlugin>() {
            native.call_method(method_name, params).await
        } else if let Some(wrapper) = instance.as_any().downcast_ref::<JavaScriptPluginWrapper>() {
            wrapper.call_function(method_name, params).await
        } else {
            Err(TingError::PluginExecutionError("Unknown plugin type".to_string()))
        }
    }

    /// Call a utility plugin method
    pub async fn call_utility(&self, id: &PluginId, method: UtilityMethod, params: Value) -> Result<Value> {
        let instance = {
            let registry = self.registry.read().await;
            let entry = registry.get(id).ok_or_else(|| TingError::PluginNotFound(id.clone()))?;
            if entry.metadata.plugin_type != crate::plugin::types::PluginType::Utility {
                return Err(TingError::PluginExecutionError(format!("Plugin {} is not a utility plugin", id)));
            }
            entry.instance.clone()
        };

        let method_name = match method {
            UtilityMethod::GetFfmpegPath => "get_ffmpeg_path",
            UtilityMethod::GetFfprobePath => "get_ffprobe_path",
            UtilityMethod::CheckVersion => "check_version",
        };

        if let Some(native) = instance.as_any().downcast_ref::<NativePlugin>() {
            native.call_method(method_name, params).await
        } else if let Some(wrapper) = instance.as_any().downcast_ref::<JavaScriptPluginWrapper>() {
            wrapper.call_function(method_name, params).await
        } else {
            Err(TingError::PluginExecutionError("Unknown plugin type".to_string()))
        }
    }

    /// Helper to find and call ffmpeg-utils to get ffmpeg path
    pub async fn get_ffmpeg_path(&self) -> Option<String> {
        let registry = self.registry.read().await;
        let plugin_id = registry.values()
            .find(|e| e.metadata.name == "FFmpeg Provider")
            .map(|e| e.metadata.instance_id());

        drop(registry);

        if let Some(id) = plugin_id {
            if let Ok(result) = self.call_utility(&id, UtilityMethod::GetFfmpegPath, serde_json::json!({})).await {
                return result.get("path").and_then(|v| v.as_str()).map(|s| s.to_string());
            }
        }

        None
    }
}
