use crate::api::models::{
    PluginInfoResponse, PluginDetailResponse, PluginStatsResponse,
    PluginDependencyResponse, InstallPluginResponse,
    ReloadPluginResponse, UninstallPluginResponse,
    PluginConfigResponse, UpdatePluginConfigRequest, UpdatePluginConfigResponse,
    ScraperSourcesResponse, ScraperSearchRequest, ScraperDetailResponse, SearchResponse,
};
use crate::core::error::{Result, TingError};
use axum::{
    extract::{Path, State, Multipart},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;
use super::AppState;

/// Handler for GET /api/v1/plugins - List all plugins
pub async fn list_plugins(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let plugins = state.plugin_manager.list_plugins().await;

    let plugin_responses: Vec<PluginInfoResponse> = plugins
        .into_iter()
        .map(|info| PluginInfoResponse {
            id: info.id,
            name: info.name,
            version: info.version,
            plugin_type: format!("{:?}", info.plugin_type).to_lowercase(),
            author: Some(info.author),
            description: Some(info.description),
            is_enabled: true, // All loaded plugins are enabled
            state: format!("{:?}", info.state).to_lowercase(),
            stats: Some(PluginStatsResponse {
                total_calls: info.total_calls,
                successful_calls: info.successful_calls,
                failed_calls: info.failed_calls,
                avg_execution_time_ms: 0.0, // Not available in PluginInfo
            }),
        })
        .collect();

    Ok(Json(plugin_responses))
}

/// Handler for GET /api/v1/plugins/:id - Get plugin details
pub async fn get_plugin_detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let plugin = state.plugin_manager.get_plugin(&id)?;
    let metadata = plugin;

    let plugins = state.plugin_manager.list_plugins().await;
    let plugin_info = plugins
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| TingError::PluginNotFound(id.clone()))?;

    let response = PluginDetailResponse {
        id: plugin_info.id.clone(),
        name: metadata.name.clone(),
        version: metadata.version.to_string(),
        plugin_type: format!("{:?}", metadata.plugin_type).to_lowercase(),
        author: Some(metadata.author.clone()),
        description: Some(metadata.description.clone()),
        license: metadata.license.clone(),
        homepage: metadata.homepage.clone(),
        is_enabled: true, // All loaded plugins are enabled
        state: format!("{:?}", plugin_info.state).to_lowercase(),
        entry_point: metadata.entry_point.clone(),
        dependencies: metadata
            .dependencies
            .iter()
            .map(|dep| PluginDependencyResponse {
                plugin_name: dep.plugin_name.clone(),
                version_requirement: dep.version_requirement.to_string(),
            })
            .collect(),
        permissions: metadata
            .permissions
            .iter()
            .map(|perm| format!("{:?}", perm))
            .collect(),
        stats: Some(PluginStatsResponse {
            total_calls: plugin_info.total_calls,
            successful_calls: plugin_info.successful_calls,
            failed_calls: plugin_info.failed_calls,
            avg_execution_time_ms: 0.0, // Not available in PluginInfo
        }),
    };

    Ok(Json(response))
}

/// Handler for POST /api/v1/plugins/install - Install a plugin
pub async fn install_plugin(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse> {
    let temp_dir = std::env::temp_dir().join("ting-reader-uploads");
    if !temp_dir.exists() {
        tokio::fs::create_dir_all(&temp_dir).await.map_err(TingError::IoError)?;
    }
    
    let temp_path = temp_dir.join(format!("plugin-{}.zip", Uuid::new_v4()));
    let mut file_saved = false;
    
    while let Some(field) = multipart.next_field().await.map_err(|e| TingError::InvalidRequest(e.to_string()))? {
        if field.name() == Some("file") {
            let data = field.bytes().await.map_err(|e| TingError::InvalidRequest(e.to_string()))?;
            tokio::fs::write(&temp_path, data).await.map_err(TingError::IoError)?;
            file_saved = true;
            break;
        }
    }
    
    if !file_saved {
        return Err(TingError::InvalidRequest("No file uploaded".to_string()));
    }
    
    let result = state.plugin_manager.install_plugin_package(&temp_path).await;
    
    let _ = tokio::fs::remove_file(&temp_path).await;
    
    let plugin_id = result?;

    Ok((
        StatusCode::CREATED,
        Json(InstallPluginResponse {
            plugin_id: plugin_id.clone(),
            message: format!("Plugin {} installed successfully", plugin_id),
        }),
    ))
}

/// Handler for POST /api/v1/plugins/:id/reload - Reload a plugin
pub async fn reload_plugin(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    state.plugin_manager.reload_plugin(&id).await?;

    Ok(Json(ReloadPluginResponse {
        message: format!("Plugin {} reloaded successfully", id),
    }))
}

/// Handler for DELETE /api/v1/plugins/:id - Uninstall a plugin
pub async fn uninstall_plugin(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    if state.plugin_manager.get_plugin(&id).is_err() {
        return Err(TingError::PluginNotFound(id.clone()));
    }

    state.plugin_manager.uninstall_plugin(&id).await?;

    Ok((
        StatusCode::OK,
        Json(UninstallPluginResponse {
            message: format!("Plugin {} uninstalled successfully", id),
        }),
    ))
}

/// Handler for GET /api/v1/plugins/:id/config - Get plugin configuration
pub async fn get_plugin_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    if state.plugin_manager.get_plugin(&id).is_err() {
        return Err(TingError::PluginNotFound(id.clone()));
    }

    let config = state.config_manager.get_config(&id)?;

    Ok(Json(PluginConfigResponse {
        plugin_id: id,
        config,
    }))
}

/// Handler for PUT /api/v1/plugins/:id/config - Update plugin configuration
pub async fn update_plugin_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdatePluginConfigRequest>,
) -> Result<impl IntoResponse> {
    if state.plugin_manager.get_plugin(&id).is_err() {
        return Err(TingError::PluginNotFound(id.clone()));
    }

    state.config_manager.update_config(&id, req.config)?;

    Ok(Json(UpdatePluginConfigResponse {
        message: format!("Plugin {} configuration updated successfully", id),
    }))
}

/// Handler for GET /api/v1/scraper/sources - Get list of scraper sources
pub async fn get_scraper_sources(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let sources = state.scraper_service.get_sources().await;

    Ok(Json(ScraperSourcesResponse { sources }))
}

/// Handler for POST /api/v1/scraper/search - Search for books using scraper
pub async fn scraper_search(
    State(state): State<AppState>,
    Json(request): Json<ScraperSearchRequest>,
) -> Result<impl IntoResponse> {
    tracing::info!("Received scraper search request: {:?}", request);

    let page = request.page.unwrap_or(1);
    let page_size = request.page_size.unwrap_or(20);

    let result = state.scraper_service
        .search(&request.query, request.source.as_deref(), page, page_size)
        .await?;

    Ok(Json(SearchResponse {
        items: result.items,
        total: result.total,
        page: result.page,
        page_size: result.page_size,
    }))
}

/// Handler for GET /api/v1/scraper/detail/:source/:id - Get book detail from scraper
pub async fn get_scraper_detail(
    State(state): State<AppState>,
    Path((source, id)): Path<(String, String)>,
) -> Result<impl IntoResponse> {
    let detail = state.scraper_service.get_detail(&source, &id).await?;

    Ok(Json(ScraperDetailResponse { detail }))
}
