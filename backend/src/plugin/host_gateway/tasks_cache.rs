use super::{plugin_task_priority, required_string_param, string_param, PluginHostGateway};
use crate::core::error::{Result, TingError};
use crate::core::task_queue::{Task, TaskPayload};
use serde_json::Value;

impl PluginHostGateway {
    pub(super) async fn tasks_create(&self, params: &Value) -> Result<Value> {
        let task_type = required_string_param(params, "task_type")?;
        if matches!(task_type.as_str(), "library_scan" | "write_metadata") {
            return Err(TingError::PermissionDenied(format!(
                "Task type {} is reserved for core workflows",
                task_type
            )));
        }

        let handlers = self
            .plugin_manager
            .find_task_handlers(Some(&task_type))
            .await;
        if handlers.is_empty() {
            return Err(TingError::NotFound(format!(
                "No plugin task_handler found for task type {}",
                task_type
            )));
        }

        let data = params.get("data").cloned().unwrap_or(Value::Null);
        let name =
            string_param(params, "name").unwrap_or_else(|| format!("plugin_task_{}", task_type));
        let priority = plugin_task_priority(params);
        let task = Task::new(
            name,
            priority,
            TaskPayload::Custom {
                task_type: task_type.clone(),
                data,
            },
        );
        let task_id = self.task_queue.submit(task).await?;

        Ok(serde_json::json!({
            "task_id": task_id,
            "task_type": task_type,
            "status": "queued",
            "handler_count": handlers.len(),
        }))
    }

    pub(super) async fn cache_get(&self, plugin_id: &str, params: &Value) -> Result<Value> {
        let key = required_string_param(params, "key")?;
        let item = self.plugin_cache.get(plugin_id, &key).await?;

        Ok(match item {
            Some(item) => serde_json::json!({
                "hit": true,
                "key": item.key,
                "value": item.value,
                "created_at": item.created_at,
                "updated_at": item.updated_at,
            }),
            None => serde_json::json!({
                "hit": false,
                "key": key,
                "value": Value::Null,
            }),
        })
    }

    pub(super) async fn cache_set(&self, plugin_id: &str, params: &Value) -> Result<Value> {
        let key = required_string_param(params, "key")?;
        let value = params.get("value").cloned().unwrap_or(Value::Null);
        let item = self.plugin_cache.set(plugin_id, &key, value).await?;

        Ok(serde_json::json!({
            "key": item.key,
            "value": item.value,
            "created_at": item.created_at,
            "updated_at": item.updated_at,
        }))
    }

    pub(super) async fn cache_has(&self, plugin_id: &str, params: &Value) -> Result<Value> {
        let key = required_string_param(params, "key")?;
        let exists = self.plugin_cache.has(plugin_id, &key).await?;

        Ok(serde_json::json!({
            "key": key,
            "hit": exists,
        }))
    }

    pub(super) async fn cache_delete(&self, plugin_id: &str, params: &Value) -> Result<Value> {
        let key = required_string_param(params, "key")?;
        let deleted = self.plugin_cache.delete(plugin_id, &key).await?;

        Ok(serde_json::json!({
            "key": key,
            "deleted": deleted,
        }))
    }
}
