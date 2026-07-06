use super::AppState;
use crate::api::models::{
    FindContentProcessorsQuery, FindEventHandlersQuery, FindTaskHandlersQuery,
    FindToolProvidersQuery, InstallPluginResponse, InstallStorePluginRequest,
    InvokePluginCapabilityRequest, InvokePluginCapabilityResponse, InvokePluginHostRequest,
    InvokePluginHostResponse, ListPluginCapabilitiesQuery, PluginCapabilityRegistrationResponse,
    PluginConfigResponse, PluginDependencyResponse, PluginDetailResponse, PluginInfoResponse,
    PluginStatsResponse, ReloadPluginResponse, ScraperSearchRequest, ScraperSourcesResponse,
    SearchResponse, SignPluginRouteRequest, SignPluginRouteResponse,
    ToolProviderRegistrationResponse, UninstallPluginResponse, UnverifiedPluginInstallResponse,
    UpdatePluginConfigRequest, UpdatePluginConfigResponse,
};
use crate::auth::middleware::AuthUser;
use crate::core::error::{Result, TingError};
use crate::core::signing::{
    constant_time_eq, normalize_plugin_route_sign_path, sign_plugin_route_request,
    signature_expires_from_ttl, signature_has_expired,
    DEFAULT_PLUGIN_ROUTE_SIGNATURE_TTL_SECONDS, MAX_PLUGIN_ROUTE_SIGNATURE_TTL_SECONDS,
};
use crate::db::repository::Repository;
use crate::plugin::tr_package::{self, TrPackageSignatureStatus};
use crate::plugin::types::metadata::parse_plugin_metadata_content;
use crate::plugin::types::{PluginCapability, PluginState};
use crate::plugin::PluginHostUser;
use axum::{
    body::{Body, Bytes},
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine;
use serde::Deserialize;
use serde_json::Value;
use std::path::{Component, Path as FsPath, PathBuf};
use uuid::Uuid;

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
            runtime: info.runtime,
            author: Some(info.author),
            description: Some(info.description),
            description_i18n: info.description_i18n,
            is_enabled: true, // All loaded plugins are enabled
            state: format!("{:?}", info.state).to_lowercase(),
            error: info.error,
            stats: Some(PluginStatsResponse {
                total_calls: info.total_calls,
                successful_calls: info.successful_calls,
                failed_calls: info.failed_calls,
                avg_execution_time_ms: 0.0, // Not available in PluginInfo
            }),
            config_schema: info.config_schema,
            permissions: Some(info.permissions),
            license: info.license,
            repo: info.repo,
            min_core_version: info.min_core_version,
            min_flutter_version: info.min_flutter_version,
            scraper: info.scraper,
            capabilities: info.capabilities,
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
        runtime: metadata.runtime.clone(),
        author: Some(metadata.author.clone()),
        description: Some(metadata.description.clone()),
        description_i18n: metadata.description_i18n.clone(),
        license: metadata.license.clone(),
        repo: metadata.repo.clone(),
        min_core_version: metadata.min_core_version.clone(),
        min_flutter_version: metadata.min_flutter_version.clone(),
        is_enabled: true, // All loaded plugins are enabled
        state: format!("{:?}", plugin_info.state).to_lowercase(),
        error: plugin_info.error.clone(),
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
        supported_extensions: metadata.supported_extensions.clone(),
        config_schema: metadata.config_schema.clone(),
        scraper: metadata.scraper.clone(),
        capabilities: metadata.effective_capabilities(),
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
) -> Result<Response> {
    let temp_dir = std::env::temp_dir().join("ting-reader-uploads");
    if !temp_dir.exists() {
        tokio::fs::create_dir_all(&temp_dir)
            .await
            .map_err(TingError::IoError)?;
    }

    let temp_path = temp_dir.join(format!("plugin-{}.tr", Uuid::new_v4()));
    let mut file_saved = false;
    let mut accept_unverified = false;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| TingError::InvalidRequest(e.to_string()))?
    {
        let field_name = field.name().map(ToOwned::to_owned);
        if field_name.as_deref() == Some("file") {
            let data = field
                .bytes()
                .await
                .map_err(|e| TingError::InvalidRequest(e.to_string()))?;
            tokio::fs::write(&temp_path, data)
                .await
                .map_err(TingError::IoError)?;
            file_saved = true;
        } else if field_name.as_deref() == Some("accept_unverified") {
            let value = field
                .text()
                .await
                .map_err(|e| TingError::InvalidRequest(e.to_string()))?;
            accept_unverified = matches!(value.trim(), "true" | "1" | "yes" | "on");
        }
    }

    if !file_saved {
        return Err(TingError::InvalidRequest("No file uploaded".to_string()));
    }

    match unverified_plugin_install_confirmation(&temp_path, accept_unverified) {
        Ok(Some(response)) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Ok((StatusCode::PRECONDITION_REQUIRED, Json(response)).into_response());
        }
        Ok(None) => {}
        Err(error) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(error);
        }
    }

    let result = state
        .plugin_manager
        .install_plugin_package(&temp_path)
        .await;

    let _ = tokio::fs::remove_file(&temp_path).await;

    let plugin_id = result?;

    Ok((
        StatusCode::CREATED,
        Json(InstallPluginResponse {
            plugin_id: plugin_id.clone(),
            message: format!("Plugin {} installed successfully", plugin_id),
        }),
    )
        .into_response())
}

fn unverified_plugin_warning(plugin_name: &str) -> String {
    format!(
        "{}由未知发布者提供，未经Ting Reader验证。单击同意，即表示你同意全权负责因使用该插件而可能导致的任何设备损坏或数据丢失。",
        plugin_name
    )
}

fn unverified_plugin_install_confirmation(
    package_path: &FsPath,
    accept_unverified: bool,
) -> Result<Option<UnverifiedPluginInstallResponse>> {
    if !tr_package::has_tr_magic(package_path)? {
        return Ok(None);
    }

    let signature_status = tr_package::verify_tr_package_signature(package_path)?;
    if let TrPackageSignatureStatus::Invalid { reason, .. } = &signature_status {
        return Err(TingError::PluginLoadError(format!(
            "Invalid plugin package signature: {}",
            reason
        )));
    }
    if matches!(signature_status, TrPackageSignatureStatus::Unsigned) {
        return Err(TingError::PluginLoadError(
            "Plugin package is not signed; please build it with trpack".to_string(),
        ));
    }

    if !signature_status.is_installable_with_confirmation() || accept_unverified {
        return Ok(None);
    }

    let metadata_content = tr_package::read_manifest_file(package_path, "plugin.yml")?;
    let metadata = parse_plugin_metadata_content(&metadata_content, "plugin.yml")?;

    Ok(Some(UnverifiedPluginInstallResponse {
        requires_confirmation: true,
        verification_status: signature_status.label().to_string(),
        plugin_id: metadata.id.clone(),
        plugin_name: metadata.name.clone(),
        plugin_version: metadata.version.to_string(),
        publisher: "未知发布者".to_string(),
        warning: unverified_plugin_warning(&metadata.name),
    }))
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
    let metadata = state
        .plugin_manager
        .get_plugin(&id)
        .map_err(|_| TingError::PluginNotFound(id.clone()))?;

    if let Some(ref schema) = metadata.config_schema {
        let defaults = extract_defaults_from_schema(schema);
        state.config_manager.ensure_config(
            id.clone(),
            metadata.name.clone(),
            Some(schema.clone()),
            defaults,
        )?;
    }

    let config = state
        .config_manager
        .get_redacted_config(&id)
        .unwrap_or_else(|_| serde_json::json!({}));

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
    let metadata = state
        .plugin_manager
        .get_plugin(&id)
        .map_err(|_| TingError::PluginNotFound(id.clone()))?;

    // Auto-initialize or sync config schema before preserving encrypted fields.
    if let Some(ref schema) = metadata.config_schema {
        let defaults = extract_defaults_from_schema(schema);
        state.config_manager.ensure_config(
            id.clone(),
            metadata.name.clone(),
            Some(schema.clone()),
            defaults,
        )?;
    }

    let config = state
        .config_manager
        .merge_preserved_sensitive_fields(&id, req.config)?;
    state.config_manager.update_config(&id, config)?;
    state.plugin_manager.reload_plugin(&id).await?;

    Ok(Json(UpdatePluginConfigResponse {
        message: format!("Plugin {} configuration updated successfully", id),
    }))
}

/// Handler for GET /api/v1/plugin-capabilities - List registered capabilities.
pub async fn list_plugin_capabilities(
    State(state): State<AppState>,
    Query(query): Query<ListPluginCapabilitiesQuery>,
) -> Result<impl IntoResponse> {
    let capabilities = if let Some(kind) = query.kind.as_deref() {
        state.plugin_manager.find_capabilities_by_kind(kind).await
    } else {
        state.plugin_manager.list_capabilities().await
    };

    Ok(Json(
        capabilities
            .into_iter()
            .map(plugin_capability_registration_response)
            .collect::<Vec<_>>(),
    ))
}

/// Handler for GET /api/v1/plugin-capabilities/content-processors.
pub async fn find_content_processors(
    State(state): State<AppState>,
    Query(query): Query<FindContentProcessorsQuery>,
) -> Result<impl IntoResponse> {
    let processors = state
        .plugin_manager
        .find_content_processors(&query.extension, query.operation.as_deref())
        .await;

    Ok(Json(
        processors
            .into_iter()
            .map(|processor| plugin_capability_registration_response(processor.registration))
            .collect::<Vec<_>>(),
    ))
}

/// Handler for GET /api/v1/plugin-capabilities/tools.
pub async fn find_tool_providers(
    State(state): State<AppState>,
    Query(query): Query<FindToolProvidersQuery>,
) -> Result<impl IntoResponse> {
    let providers = state
        .plugin_manager
        .find_tool_providers(query.name.as_deref())
        .await;

    Ok(Json(
        providers
            .into_iter()
            .map(|provider| ToolProviderRegistrationResponse {
                plugin_id: provider.registration.plugin_id,
                plugin_name: provider.registration.plugin_name,
                capability: provider.registration.capability,
                tool: provider.tool,
            })
            .collect::<Vec<_>>(),
    ))
}

/// Handler for GET /api/v1/plugin-capabilities/task-handlers.
pub async fn find_task_handlers(
    State(state): State<AppState>,
    Query(query): Query<FindTaskHandlersQuery>,
) -> Result<impl IntoResponse> {
    let handlers = state
        .plugin_manager
        .find_task_handlers(query.task_type.as_deref())
        .await;

    Ok(Json(
        handlers
            .into_iter()
            .map(|handler| plugin_capability_registration_response(handler.registration))
            .collect::<Vec<_>>(),
    ))
}

/// Handler for GET /api/v1/plugin-capabilities/event-handlers.
pub async fn find_event_handlers(
    State(state): State<AppState>,
    Query(query): Query<FindEventHandlersQuery>,
) -> Result<impl IntoResponse> {
    let handlers = state
        .plugin_manager
        .find_event_handlers(query.event.as_deref())
        .await;

    Ok(Json(
        handlers
            .into_iter()
            .map(|handler| plugin_capability_registration_response(handler.registration))
            .collect::<Vec<_>>(),
    ))
}

fn plugin_capability_registration_response(
    registration: crate::plugin::manager::capabilities::RegisteredCapability,
) -> PluginCapabilityRegistrationResponse {
    PluginCapabilityRegistrationResponse {
        plugin_id: registration.plugin_id,
        plugin_name: registration.plugin_name,
        capability: registration.capability,
    }
}

fn plugin_capability_not_found(plugin_id: &str, capability_id: &str) -> TingError {
    TingError::NotFound(format!(
        "Capability {} not found for plugin {}",
        capability_id, plugin_id
    ))
}

/// Handler for POST /api/v1/plugins/:id/capabilities/:capability_id/invoke.
pub async fn invoke_plugin_capability(
    State(state): State<AppState>,
    user: AuthUser,
    Path((id, capability_id)): Path<(String, String)>,
    Json(req): Json<InvokePluginCapabilityRequest>,
) -> Result<impl IntoResponse> {
    let metadata = state
        .plugin_manager
        .get_plugin(&id)
        .map_err(|_| TingError::PluginNotFound(id.clone()))?;

    let capability = metadata
        .effective_capabilities()
        .into_iter()
        .find(|capability| capability.id == capability_id)
        .ok_or_else(|| plugin_capability_not_found(&id, &capability_id))?;

    let invoke_method = capability
        .invoke
        .clone()
        .unwrap_or_else(|| capability.id.clone());

    let params = attach_plugin_invocation_context(
        req.params,
        &id,
        &capability_id,
        PluginRouteAccess::Authenticated,
        Some(&user),
    );

    let result = state
        .plugin_manager
        .invoke_plugin(&id, &invoke_method, params)
        .await?;

    Ok(Json(InvokePluginCapabilityResponse { result }))
}

/// Handler for /api/v1/plugin-routes/*path - Invoke a plugin-declared HTTP route.
pub async fn call_plugin_route(
    State(state): State<AppState>,
    user: AuthUser,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response> {
    call_plugin_route_inner(
        state,
        Some(user),
        method,
        uri,
        headers,
        body,
        PluginRouteAccess::Authenticated,
    )
    .await
}

/// Handler for /api/v1/public/plugin-routes/*path - Invoke public plugin HTTP routes.
pub async fn call_public_plugin_route(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response> {
    call_plugin_route_inner(
        state,
        None,
        method,
        uri,
        headers,
        body,
        PluginRouteAccess::Public,
    )
    .await
}

/// Handler for GET /api/v1/plugin-assets/:id/*path - Serve sandboxed plugin UI assets.
pub async fn get_plugin_asset(
    State(state): State<AppState>,
    Path((id, asset_path)): Path<(String, String)>,
) -> Result<Response> {
    let plugin_info = state
        .plugin_manager
        .list_plugins()
        .await
        .into_iter()
        .find(|plugin| plugin.id == id)
        .ok_or_else(|| TingError::PluginNotFound(id.clone()))?;
    if plugin_info.state == PluginState::Failed {
        return Err(TingError::PermissionDenied(format!(
            "Plugin assets are unavailable for failed plugin {}",
            id
        )));
    }

    let plugin_root = state.plugin_manager.get_plugin_package_path(&id).await?;
    let relative_path = normalize_plugin_asset_path(&asset_path)?;

    let canonical_root = tokio::fs::canonicalize(&plugin_root)
        .await
        .map_err(TingError::IoError)?;
    let candidate_path = canonical_root.join(&relative_path);
    let canonical_asset = tokio::fs::canonicalize(&candidate_path)
        .await
        .map_err(|_| TingError::NotFound(format!("Plugin asset not found: {}", asset_path)))?;

    if !canonical_asset.starts_with(&canonical_root) {
        return Err(TingError::PermissionDenied(format!(
            "Plugin asset path escapes plugin package: {}",
            asset_path
        )));
    }

    let metadata = tokio::fs::metadata(&canonical_asset)
        .await
        .map_err(TingError::IoError)?;
    if !metadata.is_file() {
        return Err(TingError::NotFound(format!(
            "Plugin asset is not a file: {}",
            asset_path
        )));
    }

    let content_type = mime_guess::from_path(&canonical_asset)
        .first_or_octet_stream()
        .to_string();
    let body = tokio::fs::read(&canonical_asset)
        .await
        .map_err(TingError::IoError)?;

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", content_type)
        .header("Cache-Control", "public, max-age=300")
        .body(Body::from(body))
        .map_err(|e| {
            TingError::PluginExecutionError(format!("Plugin asset response failed: {}", e))
        })
}

/// Handler for POST /api/v1/plugin-route-signatures - Generate a signed public plugin route URL.
pub async fn sign_plugin_route(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<SignPluginRouteRequest>,
) -> Result<impl IntoResponse> {
    let method = req.method.trim().to_uppercase();
    if method.is_empty() {
        return Err(TingError::InvalidRequest(
            "Plugin route method is required".to_string(),
        ));
    }

    let path = normalize_plugin_route_sign_path(&req.path);
    let matched = state
        .plugin_manager
        .find_http_route(&method, &path)
        .await
        .ok_or_else(|| {
            TingError::NotFound(format!(
                "Plugin route not found: {} {}",
                method.as_str(),
                path
            ))
        })?;

    if !plugin_route_allows_public_access(&matched.registration.capability) {
        return Err(TingError::PermissionDenied(format!(
            "Plugin route cannot be exposed through public plugin-routes: {} {}",
            method.as_str(),
            path
        )));
    }

    let expires = signature_expires_from_ttl(
        req.expires_in_seconds,
        DEFAULT_PLUGIN_ROUTE_SIGNATURE_TTL_SECONDS,
        MAX_PLUGIN_ROUTE_SIGNATURE_TTL_SECONDS,
    );
    let signed_user_id = req
        .bind_current_user
        .unwrap_or(true)
        .then(|| user.id.clone());
    let signature = sign_plugin_route_request(
        state.encryption_key.as_ref(),
        method.as_str(),
        path.as_str(),
        expires,
        signed_user_id.as_deref(),
    );
    let signed_url = if let Some(user_id) = signed_user_id.as_deref() {
        format!(
            "/api/v1/public/plugin-routes{}?expires={}&user={}&signature={}",
            path,
            expires,
            urlencoding::encode(user_id),
            signature
        )
    } else {
        format!(
            "/api/v1/public/plugin-routes{}?expires={}&signature={}",
            path, expires, signature
        )
    };

    Ok(Json(SignPluginRouteResponse {
        path,
        expires,
        signature,
        user_id: signed_user_id,
        signed_url,
    }))
}

/// Handler for POST /api/v1/plugin-host/invoke - Invoke a HostGateway method.
pub async fn invoke_plugin_host(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<InvokePluginHostRequest>,
) -> Result<impl IntoResponse> {
    let host_user = PluginHostUser {
        id: user.id.clone(),
        username: user.username.clone(),
        role: user.role.clone(),
    };
    let result = state
        .plugin_host_gateway
        .invoke_plugin(&req.plugin_id, &host_user, &req.method, req.params)
        .await?;
    Ok(Json(InvokePluginHostResponse { result }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginRouteAccess {
    Authenticated,
    SignedUser,
    Public,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginRouteAuthPolicy {
    User,
    Public,
    Signed,
    PublicOrSigned,
}

impl PluginRouteAuthPolicy {
    fn can_use_public_prefix(self) -> bool {
        matches!(self, Self::Public | Self::Signed | Self::PublicOrSigned)
    }
}

fn plugin_route_context_json(access: PluginRouteAccess, user: Option<&AuthUser>) -> Value {
    let authenticated = matches!(
        access,
        PluginRouteAccess::Authenticated | PluginRouteAccess::SignedUser
    ) && user.is_some();
    let user_value = if authenticated {
        user.map(|user| {
            serde_json::json!({
                "id": user.id,
                "username": user.username,
                "role": user.role,
            })
        })
        .unwrap_or(Value::Null)
    } else {
        Value::Null
    };

    serde_json::json!({
        "access": match access {
            PluginRouteAccess::Authenticated => "authenticated",
            PluginRouteAccess::SignedUser => "signed",
            PluginRouteAccess::Public => "public",
        },
        "authenticated": authenticated,
        "user": user_value,
    })
}

fn attach_plugin_invocation_context(
    params: Value,
    plugin_id: &str,
    capability_id: &str,
    access: PluginRouteAccess,
    user: Option<&AuthUser>,
) -> Value {
    let context = serde_json::json!({
        "plugin_id": plugin_id,
        "capability_id": capability_id,
        "route": plugin_route_context_json(access, user),
    });

    match params {
        Value::Object(mut object) => {
            object.insert("_context".to_string(), context);
            Value::Object(object)
        }
        value => serde_json::json!({
            "input": value,
            "_context": context,
        }),
    }
}

async fn call_plugin_route_inner(
    state: AppState,
    user: Option<AuthUser>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
    access: PluginRouteAccess,
) -> Result<Response> {
    let route_path = plugin_route_path_from_uri(&uri);
    let matched = state
        .plugin_manager
        .find_http_route(method.as_str(), &route_path)
        .await
        .ok_or_else(|| {
            TingError::NotFound(format!(
                "Plugin route not found: {} {}",
                method.as_str(),
                route_path
            ))
        })?;

    let mut route_user = user;
    let mut route_access = access;

    if access == PluginRouteAccess::Public {
        validate_public_plugin_route_access(
            &matched.registration.capability,
            &method,
            &route_path,
            &uri,
            state.encryption_key.as_ref(),
        )?;

        if let Some(user_id) = signed_plugin_route_user(&uri) {
            let signed_user = state.user_repo.find_by_id(&user_id).await?.ok_or_else(|| {
                TingError::PermissionDenied("Signed route user not found".to_string())
            })?;
            route_user = Some(AuthUser {
                user_id: signed_user.id.clone(),
                id: signed_user.id,
                username: signed_user.username,
                role: signed_user.role,
            });
            route_access = PluginRouteAccess::SignedUser;
        }
    }

    let invoke_method = matched
        .registration
        .capability
        .invoke
        .clone()
        .unwrap_or_else(|| matched.registration.capability.id.clone());

    let params = serde_json::json!({
        "method": method.as_str(),
        "path": route_path,
        "query": uri.query().unwrap_or(""),
        "headers": headers_to_json(&headers),
        "params": matched.params,
        "body_text": std::str::from_utf8(body.as_ref()).ok(),
        "body_base64": base64::engine::general_purpose::STANDARD.encode(body.as_ref()),
        "capability_id": matched.registration.capability.id,
        "plugin_id": matched.registration.plugin_id,
        "context": plugin_route_context_json(route_access, route_user.as_ref()),
    });

    let result = state
        .plugin_manager
        .invoke_plugin(&matched.registration.plugin_id, &invoke_method, params)
        .await?;

    plugin_route_result_to_response(result)
}

fn plugin_route_path_from_uri(uri: &Uri) -> String {
    let path = uri.path();
    let route_path = [
        "/api/v1/public/plugin-routes",
        "/api/public/plugin-routes",
        "/api/v1/plugin-routes",
        "/api/plugin-routes",
    ]
    .iter()
    .find_map(|prefix| path.strip_prefix(prefix))
    .unwrap_or(path);

    if route_path.is_empty() {
        "/".to_string()
    } else {
        route_path.to_string()
    }
}

fn normalize_plugin_asset_path(asset_path: &str) -> Result<PathBuf> {
    let asset_path = asset_path.trim_start_matches('/');
    if asset_path.trim().is_empty() {
        return Err(TingError::InvalidRequest(
            "Plugin asset path is required".to_string(),
        ));
    }

    let mut normalized = PathBuf::new();
    for component in FsPath::new(asset_path).components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            _ => {
                return Err(TingError::PermissionDenied(format!(
                    "Invalid plugin asset path: {}",
                    asset_path
                )));
            }
        }
    }

    let first_component = normalized
        .components()
        .next()
        .and_then(|component| match component {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .unwrap_or_default();

    if !matches!(first_component, "ui" | "assets") {
        return Err(TingError::PermissionDenied(format!(
            "Plugin assets must live under ui/ or assets/: {}",
            asset_path
        )));
    }

    Ok(normalized)
}

fn plugin_route_auth_policy(capability: &PluginCapability) -> PluginRouteAuthPolicy {
    let route = capability.extra.get("route");
    let auth = route
        .and_then(|value| value.get("auth"))
        .or_else(|| capability.extra.get("auth"))
        .and_then(Value::as_str)
        .unwrap_or("user");

    match auth {
        "public" => PluginRouteAuthPolicy::Public,
        "signed" => PluginRouteAuthPolicy::Signed,
        "public_or_signed" => PluginRouteAuthPolicy::PublicOrSigned,
        _ => PluginRouteAuthPolicy::User,
    }
}

fn plugin_route_allows_public_access(capability: &PluginCapability) -> bool {
    plugin_route_auth_policy(capability).can_use_public_prefix()
}

fn validate_public_plugin_route_access(
    capability: &PluginCapability,
    method: &Method,
    route_path: &str,
    uri: &Uri,
    signing_key: &[u8; 32],
) -> Result<()> {
    match plugin_route_auth_policy(capability) {
        PluginRouteAuthPolicy::Public => {
            if plugin_route_has_signature(uri) {
                validate_plugin_route_signature(method.as_str(), route_path, uri, signing_key)
            } else {
                Ok(())
            }
        }
        PluginRouteAuthPolicy::PublicOrSigned => {
            if plugin_route_has_signature(uri) {
                validate_plugin_route_signature(method.as_str(), route_path, uri, signing_key)
            } else {
                Ok(())
            }
        }
        PluginRouteAuthPolicy::Signed => {
            validate_plugin_route_signature(method.as_str(), route_path, uri, signing_key)
        }
        PluginRouteAuthPolicy::User => Err(TingError::PermissionDenied(format!(
            "Plugin route is not public: {} {}",
            method.as_str(),
            route_path
        ))),
    }
}

fn plugin_route_has_signature(uri: &Uri) -> bool {
    query_param(uri, "expires").is_some() || query_param(uri, "signature").is_some()
}

fn signed_plugin_route_user(uri: &Uri) -> Option<String> {
    if plugin_route_has_signature(uri) {
        query_param(uri, "user")
    } else {
        None
    }
}

fn validate_plugin_route_signature(
    method: &str,
    route_path: &str,
    uri: &Uri,
    signing_key: &[u8; 32],
) -> Result<()> {
    let expires = query_param(uri, "expires")
        .ok_or_else(|| {
            TingError::PermissionDenied("Missing plugin route signature expiry".to_string())
        })?
        .parse::<i64>()
        .map_err(|_| {
            TingError::PermissionDenied("Invalid plugin route signature expiry".to_string())
        })?;

    if signature_has_expired(expires) {
        return Err(TingError::PermissionDenied(
            "Plugin route signature has expired".to_string(),
        ));
    }

    let signature = query_param(uri, "signature")
        .ok_or_else(|| TingError::PermissionDenied("Missing plugin route signature".to_string()))?;
    let signed_user_id = query_param(uri, "user");
    let expected = sign_plugin_route_request(
        signing_key,
        method,
        route_path,
        expires,
        signed_user_id.as_deref(),
    );

    if !constant_time_eq(signature.as_bytes(), expected.as_bytes()) {
        return Err(TingError::PermissionDenied(
            "Invalid plugin route signature".to_string(),
        ));
    }

    Ok(())
}

fn query_param(uri: &Uri, name: &str) -> Option<String> {
    uri.query().and_then(|query| {
        url::form_urlencoded::parse(query.as_bytes())
            .find_map(|(key, value)| (key == name).then(|| value.into_owned()))
    })
}

fn headers_to_json(headers: &HeaderMap) -> Value {
    let mut object = serde_json::Map::new();
    for (name, value) in headers {
        if let Ok(value) = value.to_str() {
            object.insert(name.as_str().to_string(), Value::String(value.to_string()));
        }
    }
    Value::Object(object)
}

fn plugin_route_result_to_response(value: Value) -> Result<Response> {
    let status = value
        .get("status")
        .and_then(Value::as_u64)
        .unwrap_or(StatusCode::OK.as_u16() as u64);
    let status = u16::try_from(status)
        .ok()
        .and_then(|status| StatusCode::from_u16(status).ok())
        .ok_or_else(|| {
            TingError::PluginExecutionError("Plugin returned invalid HTTP status".to_string())
        })?;

    let mut builder = Response::builder().status(status);
    if let Some(headers) = value.get("headers").and_then(Value::as_object) {
        for (name, value) in headers {
            let Some(value) = value.as_str() else {
                continue;
            };
            let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|e| {
                TingError::PluginExecutionError(format!(
                    "Plugin returned invalid HTTP header name '{}': {}",
                    name, e
                ))
            })?;
            let header_value = HeaderValue::from_str(value).map_err(|e| {
                TingError::PluginExecutionError(format!(
                    "Plugin returned invalid HTTP header value for '{}': {}",
                    name, e
                ))
            })?;
            builder = builder.header(header_name, header_value);
        }
    }

    let body = plugin_route_response_body(&value)?;
    builder.body(Body::from(body)).map_err(|e| {
        TingError::PluginExecutionError(format!("Plugin HTTP response build failed: {}", e))
    })
}

fn plugin_route_response_body(value: &Value) -> Result<Vec<u8>> {
    if let Some(body_base64) = value.get("body_base64").and_then(Value::as_str) {
        return base64::engine::general_purpose::STANDARD
            .decode(body_base64)
            .map_err(|e| {
                TingError::PluginExecutionError(format!(
                    "Plugin returned invalid base64 response body: {}",
                    e
                ))
            });
    }

    if let Some(body) = value.get("body") {
        if let Some(body) = body.as_str() {
            return Ok(body.as_bytes().to_vec());
        }

        return serde_json::to_vec(body).map_err(|e| {
            TingError::SerializationError(format!(
                "Plugin response body serialization failed: {}",
                e
            ))
        });
    }

    if value.is_object() {
        Ok(Vec::new())
    } else {
        serde_json::to_vec(value).map_err(|e| {
            TingError::SerializationError(format!("Plugin response serialization failed: {}", e))
        })
    }
}

/// Extract default config values from a JSON Schema
fn extract_defaults_from_schema(schema: &serde_json::Value) -> serde_json::Value {
    let mut defaults = serde_json::json!({});
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        let obj = defaults.as_object_mut().unwrap();
        for (key, prop) in properties {
            if let Some(default_val) = prop.get("default") {
                obj.insert(key.clone(), default_val.clone());
            }
        }
    }
    defaults
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
    let mut search_params = request.search_params.unwrap_or_default();
    if let Some(query) = request.query {
        search_params
            .entry("title".to_string())
            .or_insert(query.clone());
        search_params.entry("query".to_string()).or_insert(query);
    }
    if let Some(author) = request.author {
        search_params.entry("author".to_string()).or_insert(author);
    }
    if let Some(narrator) = request.narrator {
        search_params
            .entry("narrator".to_string())
            .or_insert(narrator);
    }

    let result = state
        .scraper_service
        .search_with_params(&search_params, request.source.as_deref(), page, page_size)
        .await?;

    Ok(Json(SearchResponse {
        items: result.items,
        total: result.total,
        page: result.page,
        page_size: result.page_size,
    }))
}

/// Handler for GET /api/v1/store/plugins - Get list of plugins from store
pub async fn get_store_plugins(
    State(state): State<AppState>,
    Query(query): Query<StorePluginsQuery>,
) -> Result<impl IntoResponse> {
    let plugins = if query.refresh.unwrap_or(false) {
        state.plugin_manager.refresh_store_plugins().await?
    } else {
        state.plugin_manager.get_store_plugins().await?
    };
    Ok(Json(plugins))
}

#[derive(Debug, Default, Deserialize)]
pub struct StorePluginsQuery {
    #[serde(default)]
    refresh: Option<bool>,
}

/// Handler for POST /api/v1/store/cache/clear - Clear plugin store cache
pub async fn clear_plugin_cache(State(state): State<AppState>) -> Result<impl IntoResponse> {
    state.plugin_manager.clear_store_cache().await;
    Ok(Json(serde_json::json!({
        "message": "Plugin cache cleared successfully"
    })))
}

/// Handler for POST /api/v1/store/install - Install a plugin from store
pub async fn install_store_plugin(
    State(state): State<AppState>,
    Json(req): Json<InstallStorePluginRequest>,
) -> Result<Response> {
    let temp_path = state
        .plugin_manager
        .download_plugin_from_store(&req.plugin_id)
        .await?;

    match unverified_plugin_install_confirmation(&temp_path, req.accept_unverified) {
        Ok(Some(response)) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Ok((StatusCode::PRECONDITION_REQUIRED, Json(response)).into_response());
        }
        Ok(None) => {}
        Err(error) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(error);
        }
    }

    let result = state
        .plugin_manager
        .install_plugin_package(&temp_path)
        .await;
    let _ = tokio::fs::remove_file(&temp_path).await;

    let plugin_id = result?;

    Ok((
        StatusCode::CREATED,
        Json(InstallPluginResponse {
            plugin_id: plugin_id.clone(),
            message: format!("Plugin {} installed successfully from store", plugin_id),
        }),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use serde_json::json;

    #[test]
    fn plugin_route_path_strips_api_prefixes() {
        let v1_uri: Uri = "/api/v1/plugin-routes/rss/main.xml?token=abc"
            .parse()
            .unwrap();
        let compat_uri: Uri = "/api/plugin-routes/assistant/chat".parse().unwrap();
        let public_uri: Uri = "/api/v1/public/plugin-routes/rss/main.xml".parse().unwrap();

        assert_eq!(plugin_route_path_from_uri(&v1_uri), "/rss/main.xml");
        assert_eq!(plugin_route_path_from_uri(&compat_uri), "/assistant/chat");
        assert_eq!(plugin_route_path_from_uri(&public_uri), "/rss/main.xml");
    }

    #[test]
    fn plugin_route_public_access_requires_explicit_auth_policy() {
        let mut private_capability = PluginCapability {
            id: "private.route".to_string(),
            kind: "http_route".to_string(),
            invoke: None,
            extra: Default::default(),
        };
        assert!(!plugin_route_allows_public_access(&private_capability));

        private_capability.extra.insert(
            "route".to_string(),
            json!({
                "auth": "public_or_signed"
            }),
        );
        assert!(plugin_route_allows_public_access(&private_capability));
    }

    #[test]
    fn plugin_route_context_includes_authenticated_user_only_for_private_routes() {
        let user = AuthUser {
            user_id: "user-1".to_string(),
            id: "user-1".to_string(),
            username: "alice".to_string(),
            role: "admin".to_string(),
        };

        let authenticated =
            plugin_route_context_json(PluginRouteAccess::Authenticated, Some(&user));
        assert_eq!(authenticated["access"], "authenticated");
        assert_eq!(authenticated["authenticated"], true);
        assert_eq!(authenticated["user"]["id"], "user-1");
        assert_eq!(authenticated["user"]["username"], "alice");
        assert_eq!(authenticated["user"]["role"], "admin");

        let public = plugin_route_context_json(PluginRouteAccess::Public, Some(&user));
        assert_eq!(public["access"], "public");
        assert_eq!(public["authenticated"], false);
        assert!(public["user"].is_null());

        let signed = plugin_route_context_json(PluginRouteAccess::SignedUser, Some(&user));
        assert_eq!(signed["access"], "signed");
        assert_eq!(signed["authenticated"], true);
        assert_eq!(signed["user"]["id"], "user-1");
    }

    #[test]
    fn plugin_invocation_context_is_attached_to_capability_params() {
        let user = AuthUser {
            user_id: "user-1".to_string(),
            id: "user-1".to_string(),
            username: "alice".to_string(),
            role: "user".to_string(),
        };

        let params = attach_plugin_invocation_context(
            json!({"prompt": "hello"}),
            "assistant@1.0.0",
            "assistant.ui",
            PluginRouteAccess::Authenticated,
            Some(&user),
        );

        assert_eq!(params["prompt"], "hello");
        assert_eq!(params["_context"]["plugin_id"], "assistant@1.0.0");
        assert_eq!(params["_context"]["capability_id"], "assistant.ui");
        assert_eq!(params["_context"]["route"]["user"]["username"], "alice");

        let wrapped = attach_plugin_invocation_context(
            json!("raw"),
            "assistant@1.0.0",
            "assistant.ui",
            PluginRouteAccess::Authenticated,
            Some(&user),
        );
        assert_eq!(wrapped["input"], "raw");
        assert!(wrapped["_context"].is_object());
    }

    #[test]
    fn signed_plugin_route_requires_valid_signature() {
        let mut capability = PluginCapability {
            id: "signed.route".to_string(),
            kind: "http_route".to_string(),
            invoke: None,
            extra: Default::default(),
        };
        capability.extra.insert(
            "route".to_string(),
            json!({
                "auth": "signed"
            }),
        );

        let key = [7_u8; 32];
        let unsigned_uri: Uri = "/api/v1/public/plugin-routes/rss/main.xml".parse().unwrap();
        let unsigned_error = validate_public_plugin_route_access(
            &capability,
            &Method::GET,
            "/rss/main.xml",
            &unsigned_uri,
            &key,
        )
        .unwrap_err();
        assert!(matches!(unsigned_error, TingError::PermissionDenied(_)));

        let expires = chrono::Utc::now().timestamp() + 60;
        let signature = sign_plugin_route_request(&key, "GET", "/rss/main.xml", expires, None);
        let signed_uri: Uri = format!(
            "/api/v1/public/plugin-routes/rss/main.xml?expires={}&signature={}",
            expires, signature
        )
        .parse()
        .unwrap();
        validate_public_plugin_route_access(
            &capability,
            &Method::GET,
            "/rss/main.xml",
            &signed_uri,
            &key,
        )
        .unwrap();
    }

    #[test]
    fn signed_plugin_route_can_bind_user_context() {
        let mut capability = PluginCapability {
            id: "signed.route".to_string(),
            kind: "http_route".to_string(),
            invoke: None,
            extra: Default::default(),
        };
        capability.extra.insert(
            "route".to_string(),
            json!({
                "auth": "signed"
            }),
        );

        let key = [9_u8; 32];
        let expires = chrono::Utc::now().timestamp() + 60;
        let signature =
            sign_plugin_route_request(&key, "GET", "/rss/main.xml", expires, Some("user-1"));
        let signed_uri: Uri = format!(
            "/api/v1/public/plugin-routes/rss/main.xml?expires={}&user=user-1&signature={}",
            expires, signature
        )
        .parse()
        .unwrap();
        validate_public_plugin_route_access(
            &capability,
            &Method::GET,
            "/rss/main.xml",
            &signed_uri,
            &key,
        )
        .unwrap();
        assert_eq!(
            signed_plugin_route_user(&signed_uri).as_deref(),
            Some("user-1")
        );

        let tampered_uri: Uri = format!(
            "/api/v1/public/plugin-routes/rss/main.xml?expires={}&user=user-2&signature={}",
            expires, signature
        )
        .parse()
        .unwrap();
        assert!(validate_public_plugin_route_access(
            &capability,
            &Method::GET,
            "/rss/main.xml",
            &tampered_uri,
            &key,
        )
        .is_err());
    }

    #[test]
    fn expired_plugin_route_signature_is_rejected() {
        let mut capability = PluginCapability {
            id: "signed.route".to_string(),
            kind: "http_route".to_string(),
            invoke: None,
            extra: Default::default(),
        };
        capability.extra.insert(
            "route".to_string(),
            json!({
                "auth": "signed"
            }),
        );

        let key = [3_u8; 32];
        let expires = chrono::Utc::now().timestamp() - 60;
        let signature = sign_plugin_route_request(&key, "GET", "/rss/main.xml", expires, None);
        let uri: Uri = format!(
            "/api/v1/public/plugin-routes/rss/main.xml?expires={}&signature={}",
            expires, signature
        )
        .parse()
        .unwrap();

        let error = validate_public_plugin_route_access(
            &capability,
            &Method::GET,
            "/rss/main.xml",
            &uri,
            &key,
        )
        .unwrap_err();
        assert!(matches!(error, TingError::PermissionDenied(_)));
    }

    #[test]
    fn plugin_route_sign_path_strips_known_prefixes() {
        assert_eq!(
            normalize_plugin_route_sign_path("/api/v1/plugin-routes/rss/main.xml?x=1"),
            "/rss/main.xml"
        );
        assert_eq!(
            normalize_plugin_route_sign_path("api/public/plugin-routes/tools/ping"),
            "/tools/ping"
        );
    }

    #[test]
    fn plugin_asset_path_allows_only_ui_and_assets_directories() {
        assert_eq!(
            normalize_plugin_asset_path("ui/assistant.html").unwrap(),
            PathBuf::from("ui").join("assistant.html")
        );
        assert_eq!(
            normalize_plugin_asset_path("/assets/icon.png").unwrap(),
            PathBuf::from("assets").join("icon.png")
        );
        assert!(normalize_plugin_asset_path("data/secret.json").is_err());
        assert!(normalize_plugin_asset_path("ui/../plugin.yml").is_err());
    }

    #[tokio::test]
    async fn plugin_route_result_builds_http_response() {
        let response = plugin_route_result_to_response(json!({
            "status": 200,
            "headers": {
                "content-type": "application/rss+xml; charset=utf-8"
            },
            "body": "<rss version=\"2.0\"></rss>"
        }))
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()["content-type"],
            "application/rss+xml; charset=utf-8"
        );

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body.as_ref(), b"<rss version=\"2.0\"></rss>");
    }

    #[test]
    fn plugin_route_body_supports_base64() {
        let body = plugin_route_response_body(&json!({
            "body_base64": base64::engine::general_purpose::STANDARD.encode([0, 1, 2, 3])
        }))
        .unwrap();

        assert_eq!(body, vec![0, 1, 2, 3]);
    }

    #[test]
    fn plugin_route_result_rejects_invalid_status() {
        let error = plugin_route_result_to_response(json!({ "status": 9999 })).unwrap_err();

        assert!(matches!(error, TingError::PluginExecutionError(_)));
    }

    #[test]
    fn plugin_capability_not_found_returns_not_found_error() {
        let error = plugin_capability_not_found("demo@1.0.0", "missing.capability");

        assert!(matches!(error, TingError::NotFound(_)));
        assert_eq!(error.status_code(), StatusCode::NOT_FOUND);
    }
}
