use crate::core::app::error::{
    AccessErrorKind, DeadlineErrorKind, LookupErrorKind, PluginErrorKind,
};
use crate::plugin::types::schema::validate_payload;
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::PluginManager;
use crate::core::app::error::{Result, TingError};
use crate::plugin::host_api::logger::{PluginLogLevel, emit_plugin_event};
use crate::plugin::types::{
    PluginCapability, PluginId, PluginInvocationContext, PluginLogSource, PluginState,
};
use ting_plugin_contract::capability::MetadataOperation;
use ting_plugin_contract::format_calls::{FormatCall, FormatOutput};
use ting_plugin_contract::protocol::CallResult;
use ting_plugin_contract::scraper::{
    ChapterDetail, ChapterId, ChapterListRequest, ChapterPage, CoverAssetRef, FetchCoverRequest,
    MediaSourceDescriptor, ScraperResult, SearchMode, SearchPage, SearchRequest, SourceRef,
};

impl PluginManager {
    /// Core-only scope creation. Registry generation and authenticated identity
    /// are captured here, never accepted from plugin control JSON.
    pub async fn resource_scope(
        &self,
        id: &PluginId,
        context: &PluginInvocationContext,
        staging_dir: std::path::PathBuf,
        limits: crate::plugin::host_api::resources::ResourceLimits,
    ) -> Result<std::sync::Arc<crate::plugin::host_api::resources::ResourceScope>> {
        let registry = self.registry.read().await;
        let entry = registry.get(id).ok_or_else(|| {
            TingError::plugin_not_found(
                LookupErrorKind::Plugin,
                "resource_scope::plugin_not_found",
                id.clone(),
            )
        })?;
        if !matches!(entry.state, PluginState::Active | PluginState::Loaded) {
            return Err(TingError::plugin_execution(
                PluginErrorKind::Unavailable,
                "resource_scope::plugin_execution",
                "Plugin is unavailable".into(),
            ));
        }
        let scope = std::sync::Arc::new(crate::plugin::host_api::resources::ResourceScope::new(
            id.clone(),
            entry.generation,
            context.user.as_ref().map(|user| user.id.clone()),
            staging_dir,
            limits,
        ));
        entry.track_resource_scope(&scope)?;
        Ok(scope)
    }

    /// Validate the target declaration before reaching a runtime adapter.
    /// Callers supply the capability ID; bare method and tool names are not
    /// sufficient to authorize an invocation.
    pub async fn invoke_capability(
        &self,
        id: &PluginId,
        capability_id: &str,
        operation: &str,
        params: Value,
        context: &PluginInvocationContext,
    ) -> Result<Value> {
        let (capability, stable_plugin_id) = {
            let registry = self.registry.read().await;
            let entry = registry.get(id).ok_or_else(|| {
                TingError::plugin_not_found(
                    LookupErrorKind::Plugin,
                    "invoke_capability::plugin_not_found",
                    id.clone(),
                )
            })?;
            if let Some(scope) = &context.resources {
                let principal = context.user.as_ref().map(|user| user.id.as_str());
                let authorization = if matches!(operation, "close" | "cancel") {
                    scope.authorize_owner(id, entry.generation, principal)
                } else {
                    scope.authorize(id, entry.generation, principal)
                };
                authorization.map_err(|error| {
                    let message = error.to_string();
                    let failure = match error.code {
                        ting_plugin_contract::protocol::PluginErrorCode::PermissionDenied => {
                            TingError::permission(
                                AccessErrorKind::ResourceDenied,
                                "plugin.authorize_resources",
                                message,
                            )
                        }
                        ting_plugin_contract::protocol::PluginErrorCode::Cancelled => {
                            TingError::timeout(
                                DeadlineErrorKind::Cancelled,
                                "plugin.authorize_resources",
                                message,
                            )
                        }
                        _ => TingError::plugin_execution(
                            PluginErrorKind::Runtime,
                            "plugin.authorize_resources",
                            message,
                        ),
                    };
                    failure.with_source(anyhow::anyhow!(error))
                })?;
            }
            if matches!(entry.state, PluginState::Failed | PluginState::Unloaded) {
                return Err(TingError::plugin_execution(
                    PluginErrorKind::Unavailable,
                    "invoke_capability::plugin_execution",
                    "Plugin is unavailable".into(),
                ));
            }
            let capability = entry
                .metadata
                .capabilities
                .iter()
                .find(|cap| cap.id() == capability_id)
                .cloned()
                .ok_or_else(|| {
                    TingError::plugin_execution(
                        PluginErrorKind::AbiMismatch,
                        "invoke_capability::plugin_execution",
                        format!("Undeclared capability: {capability_id}"),
                    )
                })?;
            (capability, entry.metadata.id.clone())
        };
        if !capability.supports(operation) {
            return Err(TingError::plugin_execution(
                PluginErrorKind::AbiMismatch,
                "invoke_capability::plugin_execution",
                format!("Capability {capability_id} does not declare operation {operation}"),
            ));
        }
        let output_schema = validate_capability_input(&capability, operation, &params)?;
        let request = params.clone();
        let mut invocation = context.clone();
        if invocation.resources.is_none() {
            invocation.resources = Some(
                self.resource_scope(
                    id,
                    context,
                    self.config.plugin_dir.join("staging"),
                    crate::plugin::host_api::resources::ResourceLimits::default(),
                )
                .await?,
            );
        }
        let raw = self
            .invoke_plugin(id, operation, params, &invocation)
            .await
            .map_err(|error| {
                error.bind_plugin_error(&stable_plugin_id, capability_id, operation)
            })?;
        let output =
            match serde_json::from_value::<CallResult<Value>>(raw).map_err(|_| {
                TingError::plugin_execution(
                    PluginErrorKind::InvalidOutput,
                    "invoke_capability::plugin_execution",
                    "Invalid plugin result envelope".into(),
                )
            })? {
                CallResult::Success(success) => success.data,
                CallResult::Failure(failure) => {
                    failure.error.validate().map_err(|_| {
                        TingError::plugin_execution(
                            PluginErrorKind::InvalidOutput,
                            "invoke_capability::plugin_execution",
                            "Invalid plugin error envelope".into(),
                        )
                    })?;
                    return Err(TingError::PluginReported(Box::new(failure.error))
                        .bind_plugin_error(&stable_plugin_id, capability_id, operation));
                }
            };
        if let Some(schema) = output_schema {
            validate_payload(&schema, &output, "output")?;
        }
        validate_capability_output(&capability, operation, &request, &output)?;
        Ok(output)
    }

    /// Invoke one validated operation through the runtime adapter trait.
    async fn invoke_plugin(
        &self,
        id: &PluginId,
        method: &str,
        params: Value,
        context: &PluginInvocationContext,
    ) -> Result<Value> {
        let instance = {
            let registry = self.registry.read().await;
            let entry = registry.get(id).ok_or_else(|| {
                TingError::plugin_not_found(
                    LookupErrorKind::Plugin,
                    "invoke_plugin::plugin_not_found",
                    id.clone(),
                )
            })?;
            entry.instance.clone()
        };

        let started_at = Instant::now();
        let result = instance
            .invoke(method, params, context)
            .await
            .map_err(Arc::new);

        if result.is_err() && !instance.is_available() {
            let mut registry = self.registry.write().await;
            if let Some(entry) = registry.get_mut(id) {
                // A reload may have installed a different generation in flight.
                if Arc::ptr_eq(&entry.instance, &instance) {
                    entry.set_state(PluginState::Failed);
                    entry.load_error = result.as_ref().err().cloned();
                    entry.cancel_resource_scopes();
                }
            }
        }

        self.record_plugin_call(
            id,
            method,
            started_at.elapsed(),
            result.as_ref().err().map(Arc::as_ref),
            PluginLogLevel::Debug,
        )
        .await;
        result
            .map_err(|error| Arc::try_unwrap(error).unwrap_or_else(TingError::Shared))
            .map_err(TingError::mark_plugin_execution_logged)
    }

    /// Invoke a declared metadata operation using the shared contract.
    pub async fn invoke_metadata(
        &self,
        id: &PluginId,
        operation: MetadataOperation,
        params: Value,
    ) -> Result<Value> {
        let (capability_id, method_name) = {
            let registry = self.registry.read().await;
            let entry = registry.get(id).ok_or_else(|| {
                TingError::plugin_not_found(
                    LookupErrorKind::Plugin,
                    "invoke_metadata::plugin_not_found",
                    id.clone(),
                )
            })?;
            let metadata_provider = entry
                .metadata
                .effective_capabilities()
                .into_iter()
                .find(|capability| matches!(capability, PluginCapability::MetadataProvider(_)))
                .ok_or_else(|| {
                    TingError::plugin_execution(
                        PluginErrorKind::AbiMismatch,
                        "invoke_metadata::plugin_execution",
                        format!(
                            "Plugin {} does not declare metadata_provider capability",
                            id
                        ),
                    )
                })?;
            let method_name = operation.as_str();
            if !metadata_provider.supports(method_name) {
                return Err(TingError::plugin_execution(
                    PluginErrorKind::AbiMismatch,
                    "invoke_metadata::plugin_execution",
                    format!(
                        "Capability {} does not declare operation {}",
                        metadata_provider.id(),
                        method_name
                    ),
                ));
            }
            (metadata_provider.id().to_owned(), method_name)
        };
        self.invoke_capability(
            id,
            &capability_id,
            method_name,
            params,
            &PluginInvocationContext::default(),
        )
        .await
    }

    pub(crate) async fn record_plugin_call(
        &self,
        id: &PluginId,
        method: &str,
        duration: Duration,
        error: Option<&TingError>,
        success_level: PluginLogLevel,
    ) {
        let mut registry = self.registry.write().await;
        let Some(entry) = registry.get_mut(id) else {
            return;
        };
        let metadata = entry.metadata.clone();
        let duration_ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);

        match error {
            Some(error) => {
                entry.stats.record_failure(Some(error.error_type()));
            }
            None => {
                entry.stats.record_success(duration_ms);
            }
        }
        drop(registry);

        let failed = error.is_some();
        let error_message = error.map(ToString::to_string);
        let fields = serde_json::json!({
            "op": "plugin.invoke",
            "method": method,
            "duration_ms": duration_ms,
            "status": if failed { "error" } else { "success" },
            "error": error_message,
            "error_type": error.map(TingError::error_type),
            "details": error.map(TingError::diagnostic_details),
        });
        emit_plugin_event(
            &metadata,
            PluginLogSource::Runtime,
            if failed {
                PluginLogLevel::Error
            } else {
                success_level
            },
            if failed {
                "Plugin invocation failed"
            } else {
                "Plugin invocation completed"
            },
            Some(&fields),
        );
    }
}

fn validate_capability_input(
    capability: &PluginCapability,
    operation: &str,
    params: &Value,
) -> Result<Option<Value>> {
    let field = |name: &str| {
        params.get(name).ok_or_else(|| {
            TingError::plugin_execution(
                PluginErrorKind::AbiMismatch,
                "validate_capability_input::plugin_execution",
                format!("Missing capability input field: {name}"),
            )
        })
    };
    match capability {
        PluginCapability::MetadataProvider(cap) => {
            match operation {
                "search" => {
                    let request: SearchRequest =
                        serde_json::from_value(params.clone()).map_err(|error| {
                            let message = format!("Invalid search request: {error}");
                            TingError::plugin_execution(
                                PluginErrorKind::InvalidOutput,
                                "validate_capability_input::plugin_execution",
                                message,
                            )
                            .with_source(anyhow::anyhow!(error))
                        })?;
                    request.validate().map_err(|error| {
                        let message = format!("Invalid search request: {error}");
                        TingError::plugin_execution(
                            PluginErrorKind::InvalidOutput,
                            "validate_capability_input::plugin_execution",
                            message,
                        )
                        .with_source(anyhow::anyhow!(error))
                    })?;
                    if request.context.as_ref().is_some_and(|context| {
                        !matches!(context.mode, SearchMode::Search) && !cap.aggregate_auto_scrape
                    }) {
                        return Err(TingError::plugin_execution(
                            PluginErrorKind::Runtime,
                            "validate_capability_input::plugin_execution",
                            "Aggregate search context requires aggregate_auto_scrape".into(),
                        ));
                    }
                    for field in &cap.search_fields {
                        if !field.required {
                            continue;
                        }
                        let present = match field.key.as_str() {
                            "title" => request
                                .title
                                .as_deref()
                                .is_some_and(|text| !text.trim().is_empty()),
                            "author" => request
                                .author
                                .as_deref()
                                .is_some_and(|text| !text.trim().is_empty()),
                            "narrator" => request
                                .narrator
                                .as_deref()
                                .is_some_and(|text| !text.trim().is_empty()),
                            key => request.filters.get(key).is_some_and(|value| {
                                !value.is_null()
                                    && value.as_str().is_none_or(|text| !text.trim().is_empty())
                            }),
                        };
                        if !present {
                            return Err(TingError::plugin_execution(
                                PluginErrorKind::Runtime,
                                "validate_capability_input::plugin_execution",
                                format!("Required search field {} is empty", field.key),
                            ));
                        }
                    }
                    if request
                        .title
                        .as_deref()
                        .is_some_and(|text| text.trim().is_empty())
                        || request
                            .author
                            .as_deref()
                            .is_some_and(|text| text.trim().is_empty())
                        || request
                            .narrator
                            .as_deref()
                            .is_some_and(|text| text.trim().is_empty())
                    {
                        return Err(TingError::plugin_execution(
                            PluginErrorKind::Runtime,
                            "validate_capability_input::plugin_execution",
                            "Empty search fields must use null".into(),
                        ));
                    }
                    if let Some(schema) = &cap.filters_schema {
                        validate_payload(
                            schema,
                            &serde_json::Value::Object(request.filters),
                            "search filters",
                        )?;
                    } else if !request.filters.is_empty() {
                        return Err(TingError::plugin_execution(
                            PluginErrorKind::AbiMismatch,
                            "validate_capability_input::plugin_execution",
                            "Search filters are not declared by this capability".into(),
                        ));
                    }
                }
                "detail" => {
                    let reference: SourceRef =
                        serde_json::from_value(params.clone()).map_err(|error| {
                            let message = format!("Invalid detail reference: {error}");
                            TingError::plugin_execution(
                                PluginErrorKind::InvalidOutput,
                                "validate_capability_input::plugin_execution",
                                message,
                            )
                            .with_source(anyhow::anyhow!(error))
                        })?;
                    reference.validate().map_err(|error| {
                        let message = format!("Invalid detail reference: {error}");
                        TingError::plugin_execution(
                            PluginErrorKind::InvalidOutput,
                            "validate_capability_input::plugin_execution",
                            message,
                        )
                        .with_source(anyhow::anyhow!(error))
                    })?;
                }
                "list_chapters" => {
                    let page: ChapterListRequest =
                        serde_json::from_value(params.clone()).map_err(|error| {
                            let message = format!("Invalid chapter list request: {error}");
                            TingError::plugin_execution(
                                PluginErrorKind::InvalidOutput,
                                "validate_capability_input::plugin_execution",
                                message,
                            )
                            .with_source(anyhow::anyhow!(error))
                        })?;
                    page.validate().map_err(|error| {
                        let message = format!("Invalid chapter list request: {error}");
                        TingError::plugin_execution(
                            PluginErrorKind::InvalidOutput,
                            "validate_capability_input::plugin_execution",
                            message,
                        )
                        .with_source(anyhow::anyhow!(error))
                    })?;
                }
                "chapter_detail" | "resolve_audio" => {
                    let chapter: ChapterId =
                        serde_json::from_value(params.clone()).map_err(|error| {
                            let message = format!("Invalid chapter request: {error}");
                            TingError::plugin_execution(
                                PluginErrorKind::InvalidOutput,
                                "validate_capability_input::plugin_execution",
                                message,
                            )
                            .with_source(anyhow::anyhow!(error))
                        })?;
                    chapter.validate().map_err(|error| {
                        let message = format!("Invalid chapter request: {error}");
                        TingError::plugin_execution(
                            PluginErrorKind::InvalidOutput,
                            "validate_capability_input::plugin_execution",
                            message,
                        )
                        .with_source(anyhow::anyhow!(error))
                    })?;
                }
                "fetch_cover" => {
                    let request: FetchCoverRequest = serde_json::from_value(params.clone())
                        .map_err(|error| {
                            let message = format!("Invalid cover request: {error}");
                            TingError::plugin_execution(
                                PluginErrorKind::InvalidOutput,
                                "validate_capability_input::plugin_execution",
                                message,
                            )
                            .with_source(anyhow::anyhow!(error))
                        })?;
                    request.source.validate().map_err(|error| {
                        let message = format!("Invalid cover request: {error}");
                        TingError::plugin_execution(
                            PluginErrorKind::InvalidOutput,
                            "validate_capability_input::plugin_execution",
                            message,
                        )
                        .with_source(anyhow::anyhow!(error))
                    })?;
                }
                _ => {}
            }
            Ok(None)
        }
        PluginCapability::FormatHandler(cap) => {
            let call: FormatCall = serde_json::from_value(serde_json::json!({
                "operation": operation, "input": params
            }))
            .map_err(|error| {
                let message = format!("Invalid format call: {error}");
                TingError::plugin_execution(
                    PluginErrorKind::InvalidOutput,
                    "validate_capability_input::plugin_execution",
                    message,
                )
                .with_source(anyhow::anyhow!(error))
            })?;
            call.validate(cap).map_err(|error| {
                let message = format!("Invalid format call: {error}");
                TingError::plugin_execution(
                    PluginErrorKind::InvalidOutput,
                    "validate_capability_input::plugin_execution",
                    message,
                )
                .with_source(anyhow::anyhow!(error))
            })?;
            Ok(None)
        }
        PluginCapability::ToolProvider(cap) => {
            let name = field("tool_name")?.as_str().ok_or_else(|| {
                TingError::plugin_execution(
                    PluginErrorKind::Runtime,
                    "validate_capability_input::plugin_execution",
                    "tool_name must be a string".into(),
                )
            })?;
            let tool = cap
                .tools
                .iter()
                .find(|tool| tool.name == name)
                .ok_or_else(|| {
                    TingError::plugin_execution(
                        PluginErrorKind::AbiMismatch,
                        "validate_capability_input::plugin_execution",
                        "Tool is not declared by this capability".into(),
                    )
                })?;
            validate_payload(&tool.input_schema, field("params")?, "tool input")?;
            Ok(Some(tool.output_schema.clone()))
        }
        PluginCapability::TaskHandler(cap) => {
            let name = field("task_type")?.as_str().ok_or_else(|| {
                TingError::plugin_execution(
                    PluginErrorKind::Runtime,
                    "validate_capability_input::plugin_execution",
                    "task_type must be a string".into(),
                )
            })?;
            let task = cap
                .tasks
                .iter()
                .find(|task| task.task_type == name)
                .ok_or_else(|| {
                    TingError::plugin_execution(
                        PluginErrorKind::AbiMismatch,
                        "validate_capability_input::plugin_execution",
                        "Task type is not declared by this capability".into(),
                    )
                })?;
            validate_payload(&task.input_schema, field("data")?, "task input")?;
            Ok(Some(task.output_schema.clone()))
        }
        PluginCapability::EventHandler(cap) => {
            let name = field("event")?.as_str().ok_or_else(|| {
                TingError::plugin_execution(
                    PluginErrorKind::Runtime,
                    "validate_capability_input::plugin_execution",
                    "event must be a string".into(),
                )
            })?;
            let event = cap
                .events
                .iter()
                .find(|event| event.name == name)
                .ok_or_else(|| {
                    TingError::plugin_execution(
                        PluginErrorKind::AbiMismatch,
                        "validate_capability_input::plugin_execution",
                        "Event is not declared by this capability".into(),
                    )
                })?;
            validate_payload(&event.schema, field("data")?, "event input")?;
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn validate_capability_output(
    capability: &PluginCapability,
    operation: &str,
    request: &Value,
    output: &Value,
) -> Result<()> {
    if let PluginCapability::MetadataProvider(cap) = capability {
        match operation {
            "search" => {
                let page: SearchPage = serde_json::from_value(output.clone()).map_err(|error| {
                    let message = format!("Invalid search page: {error}");
                    TingError::plugin_execution(
                        PluginErrorKind::InvalidOutput,
                        "validate_capability_output::plugin_execution",
                        message,
                    )
                    .with_source(anyhow::anyhow!(error))
                })?;
                page.validate().map_err(|error| {
                    let message = format!("Invalid search page: {error}");
                    TingError::plugin_execution(
                        PluginErrorKind::InvalidOutput,
                        "validate_capability_output::plugin_execution",
                        message,
                    )
                    .with_source(anyhow::anyhow!(error))
                })?;
                let requested: SearchRequest =
                    serde_json::from_value(request.clone()).map_err(|error| {
                        let message = format!("Invalid search request: {error}");
                        TingError::plugin_execution(
                            PluginErrorKind::InvalidOutput,
                            "validate_capability_output::plugin_execution",
                            message,
                        )
                        .with_source(anyhow::anyhow!(error))
                    })?;
                if page.page != requested.page {
                    return Err(TingError::plugin_execution(
                        PluginErrorKind::Runtime,
                        "validate_capability_output::plugin_execution",
                        "Search page must match the requested page".into(),
                    ));
                }
                if !requested.chapter_candidates.is_empty()
                    && page.items.iter().any(|item| {
                        !item.chapter_titles.is_empty()
                            && item.chapter_titles.len() != requested.chapter_candidates.len()
                    })
                {
                    return Err(TingError::plugin_execution(
                        PluginErrorKind::Runtime,
                        "validate_capability_output::plugin_execution",
                        "Cleaned chapter titles must match the supplied candidates".into(),
                    ));
                }
            }
            "detail" if cap.book_source => {
                let detail: ting_plugin_contract::scraper::SourceBookDetail =
                    serde_json::from_value(output.clone()).map_err(|error| {
                        TingError::plugin_execution(
                            PluginErrorKind::InvalidOutput,
                            "metadata.detail",
                            format!("Invalid book source detail: {error}"),
                        )
                    })?;
                detail.validate().map_err(|error| {
                    TingError::plugin_execution(
                        PluginErrorKind::InvalidOutput,
                        "metadata.detail",
                        error.into(),
                    )
                })?;
                if !detail.chapters_complete
                    && !cap.operations.contains(
                        &ting_plugin_contract::capability::MetadataOperation::ListChapters,
                    )
                {
                    return Err(TingError::plugin_execution(
                        PluginErrorKind::AbiMismatch,
                        "metadata.detail",
                        "Incomplete book source details require the list_chapters operation".into(),
                    ));
                }
            }
            "detail" => {
                let result: ScraperResult =
                    serde_json::from_value(output.clone()).map_err(|error| {
                        let message = format!("Invalid detail result: {error}");
                        TingError::plugin_execution(
                            PluginErrorKind::InvalidOutput,
                            "validate_capability_output::plugin_execution",
                            message,
                        )
                        .with_source(anyhow::anyhow!(error))
                    })?;
                result.validate().map_err(|error| {
                    let message = format!("Invalid detail result: {error}");
                    TingError::plugin_execution(
                        PluginErrorKind::InvalidOutput,
                        "validate_capability_output::plugin_execution",
                        message,
                    )
                    .with_source(anyhow::anyhow!(error))
                })?;
            }
            "list_chapters" => {
                let page: ChapterPage =
                    serde_json::from_value(output.clone()).map_err(|error| {
                        let message = format!("Invalid chapter page: {error}");
                        TingError::plugin_execution(
                            PluginErrorKind::InvalidOutput,
                            "validate_capability_output::plugin_execution",
                            message,
                        )
                        .with_source(anyhow::anyhow!(error))
                    })?;
                page.validate().map_err(|error| {
                    let message = format!("Invalid chapter page: {error}");
                    TingError::plugin_execution(
                        PluginErrorKind::InvalidOutput,
                        "validate_capability_output::plugin_execution",
                        message,
                    )
                    .with_source(anyhow::anyhow!(error))
                })?;
                let requested: ChapterListRequest = serde_json::from_value(request.clone())
                    .map_err(|error| {
                        let message = format!("Invalid chapter list request: {error}");
                        TingError::plugin_execution(
                            PluginErrorKind::InvalidOutput,
                            "validate_capability_output::plugin_execution",
                            message,
                        )
                        .with_source(anyhow::anyhow!(error))
                    })?;
                if page.page != requested.page {
                    return Err(TingError::plugin_execution(
                        PluginErrorKind::Runtime,
                        "validate_capability_output::plugin_execution",
                        "Chapter page must match the requested page".into(),
                    ));
                }
            }
            "chapter_detail" => {
                let detail: ChapterDetail =
                    serde_json::from_value(output.clone()).map_err(|error| {
                        let message = format!("Invalid chapter detail: {error}");
                        TingError::plugin_execution(
                            PluginErrorKind::InvalidOutput,
                            "validate_capability_output::plugin_execution",
                            message,
                        )
                        .with_source(anyhow::anyhow!(error))
                    })?;
                detail.validate().map_err(|error| {
                    let message = format!("Invalid chapter detail: {error}");
                    TingError::plugin_execution(
                        PluginErrorKind::InvalidOutput,
                        "validate_capability_output::plugin_execution",
                        message,
                    )
                    .with_source(anyhow::anyhow!(error))
                })?;
            }
            "resolve_audio" => {
                let source: MediaSourceDescriptor = serde_json::from_value(output.clone())
                    .map_err(|error| {
                        let message = format!("Invalid media source: {error}");
                        TingError::plugin_execution(
                            PluginErrorKind::InvalidOutput,
                            "validate_capability_output::plugin_execution",
                            message,
                        )
                        .with_source(anyhow::anyhow!(error))
                    })?;
                source.validate().map_err(|error| {
                    let message = format!("Invalid media source: {error}");
                    TingError::plugin_execution(
                        PluginErrorKind::InvalidOutput,
                        "validate_capability_output::plugin_execution",
                        message,
                    )
                    .with_source(anyhow::anyhow!(error))
                })?;
            }
            "fetch_cover" => {
                let cover: CoverAssetRef =
                    serde_json::from_value(output.clone()).map_err(|error| {
                        let message = format!("Invalid cover resource: {error}");
                        TingError::plugin_execution(
                            PluginErrorKind::InvalidOutput,
                            "validate_capability_output::plugin_execution",
                            message,
                        )
                        .with_source(anyhow::anyhow!(error))
                    })?;
                cover.validate().map_err(|error| {
                    let message = format!("Invalid cover resource: {error}");
                    TingError::plugin_execution(
                        PluginErrorKind::InvalidOutput,
                        "validate_capability_output::plugin_execution",
                        message,
                    )
                    .with_source(anyhow::anyhow!(error))
                })?;
            }
            _ => {}
        }
    }
    if let PluginCapability::FormatHandler(_) = capability {
        let call: FormatCall = serde_json::from_value(serde_json::json!({
            "operation": operation, "input": request
        }))
        .map_err(|error| {
            let message = format!("Invalid format request: {error}");
            TingError::plugin_execution(
                PluginErrorKind::InvalidOutput,
                "validate_capability_output::plugin_execution",
                message,
            )
            .with_source(anyhow::anyhow!(error))
        })?;
        let response: FormatOutput = serde_json::from_value(serde_json::json!({
            "operation": operation,
            "data": output,
        }))
        .map_err(|error| {
            let message = format!("Invalid format response: {error}");
            TingError::plugin_execution(
                PluginErrorKind::InvalidOutput,
                "validate_capability_output::plugin_execution",
                message,
            )
            .with_source(anyhow::anyhow!(error))
        })?;
        response.validate_for(&call).map_err(|error| {
            let message = format!("Invalid format response: {error}");
            TingError::plugin_execution(
                PluginErrorKind::InvalidOutput,
                "validate_capability_output::plugin_execution",
                message,
            )
            .with_source(anyhow::anyhow!(error))
        })?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/unit/plugin/manager/dispatch.rs"]
mod tests;
