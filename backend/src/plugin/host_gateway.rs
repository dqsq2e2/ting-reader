use crate::core::error::{Result, TingError};
mod data;
mod files;
mod tasks_cache;

use crate::core::task_queue::{Priority, TaskQueue};
use crate::db::repository::{
    BookRepository, ChapterRepository, LibraryRepository, ProgressRepository,
};
use crate::plugin::manager::PluginManager;
use crate::plugin::wasm::sandbox::Permission;
use crate::plugin::PluginCache;
use serde_json::Value;
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone)]
pub struct PluginHostGateway {
    book_repo: Arc<BookRepository>,
    library_repo: Arc<LibraryRepository>,
    chapter_repo: Arc<ChapterRepository>,
    progress_repo: Arc<ProgressRepository>,
    task_queue: Arc<TaskQueue>,
    plugin_manager: Arc<PluginManager>,
    plugin_cache: Arc<PluginCache>,
}

#[derive(Clone, Default)]
pub struct PluginHostGatewayHandle {
    gateway: Arc<RwLock<Option<Weak<PluginHostGateway>>>>,
}

impl PluginHostGatewayHandle {
    pub fn set(&self, gateway: &Arc<PluginHostGateway>) {
        if let Ok(mut current) = self.gateway.write() {
            *current = Some(Arc::downgrade(gateway));
        }
    }

    pub fn get(&self) -> Option<Arc<PluginHostGateway>> {
        self.gateway
            .read()
            .ok()
            .and_then(|current| current.as_ref().and_then(Weak::upgrade))
    }
}

#[derive(Debug, Clone)]
pub struct PluginHostUser {
    pub id: String,
    pub username: String,
    pub role: String,
}

impl PluginHostUser {
    fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

pub fn plugin_host_user_from_invocation_args(args: &Value) -> Option<PluginHostUser> {
    let route_context = args
        .get("_context")
        .and_then(|context| context.get("route"))
        .or_else(|| args.get("context"))?;

    if route_context.get("authenticated").and_then(Value::as_bool) != Some(true) {
        return None;
    }

    let user = route_context.get("user")?;
    Some(PluginHostUser {
        id: user.get("id")?.as_str()?.to_string(),
        username: user.get("username")?.as_str()?.to_string(),
        role: user.get("role")?.as_str()?.to_string(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginHostPermission {
    BooksRead,
    ChaptersRead,
    ProgressRead,
    MediaReadUrl,
    MetadataWrite,
    LibraryFileRead,
    LibraryFileWrite,
    DatabaseRead,
    DatabaseWrite,
    TaskCreate,
    CacheRead,
    CacheWrite,
}

impl PluginHostGateway {
    pub fn new(
        book_repo: Arc<BookRepository>,
        library_repo: Arc<LibraryRepository>,
        chapter_repo: Arc<ChapterRepository>,
        progress_repo: Arc<ProgressRepository>,
        task_queue: Arc<TaskQueue>,
        plugin_manager: Arc<PluginManager>,
        plugin_cache: Arc<PluginCache>,
    ) -> Self {
        Self {
            book_repo,
            library_repo,
            chapter_repo,
            progress_repo,
            task_queue,
            plugin_manager,
            plugin_cache,
        }
    }

    pub async fn invoke_plugin(
        &self,
        plugin_id: &str,
        user: &PluginHostUser,
        method: &str,
        params: Value,
    ) -> Result<Value> {
        let plugin_id_owned = plugin_id.to_string();
        let metadata = self
            .plugin_manager
            .get_plugin(&plugin_id_owned)
            .map_err(|_| TingError::PluginNotFound(plugin_id_owned))?;

        self.invoke_with_permissions(plugin_id, &metadata.permissions, user, method, params)
            .await
    }

    pub async fn invoke_with_permissions(
        &self,
        plugin_id: &str,
        permissions: &[Permission],
        user: &PluginHostUser,
        method: &str,
        params: Value,
    ) -> Result<Value> {
        Self::authorize(plugin_id, permissions, method)?;
        self.invoke_authorized(plugin_id, user, method, params)
            .await
    }

    pub fn authorize(plugin_id: &str, permissions: &[Permission], method: &str) -> Result<()> {
        let required_permission = Self::required_permission(method).ok_or_else(|| {
            TingError::InvalidRequest(format!("Unknown plugin host method: {}", method))
        })?;

        if !Self::has_permission(permissions, required_permission) {
            return Err(TingError::PermissionDenied(format!(
                "Plugin {} lacks permission required for host method {}",
                plugin_id, method
            )));
        }

        Ok(())
    }

    pub fn required_permission(method: &str) -> Option<PluginHostPermission> {
        match method {
            "books.list" | "books.get" | "libraries.list" | "libraries.get" => {
                Some(PluginHostPermission::BooksRead)
            }
            "chapters.list" | "chapters.get" => Some(PluginHostPermission::ChaptersRead),
            "progress.recent" => Some(PluginHostPermission::ProgressRead),
            "media.get_url" => Some(PluginHostPermission::MediaReadUrl),
            "metadata.write" => Some(PluginHostPermission::MetadataWrite),
            "library.file.list" | "library.file.stat" | "library.file.read" => {
                Some(PluginHostPermission::LibraryFileRead)
            }
            "library.file.write" => Some(PluginHostPermission::LibraryFileWrite),
            "database.get" | "database.list" => Some(PluginHostPermission::DatabaseRead),
            "database.update" => Some(PluginHostPermission::DatabaseWrite),
            "tasks.create" => Some(PluginHostPermission::TaskCreate),
            "cache.get" | "cache.has" => Some(PluginHostPermission::CacheRead),
            "cache.set" | "cache.delete" => Some(PluginHostPermission::CacheWrite),
            _ => None,
        }
    }

    pub fn has_permission(
        permissions: &[Permission],
        required_permission: PluginHostPermission,
    ) -> bool {
        permissions.iter().any(|permission| {
            matches!(
                (required_permission, permission),
                (PluginHostPermission::BooksRead, Permission::BooksRead)
                    | (PluginHostPermission::BooksRead, Permission::DatabaseRead)
                    | (PluginHostPermission::ChaptersRead, Permission::ChaptersRead)
                    | (PluginHostPermission::ChaptersRead, Permission::DatabaseRead)
                    | (PluginHostPermission::ProgressRead, Permission::ProgressRead)
                    | (PluginHostPermission::ProgressRead, Permission::DatabaseRead)
                    | (PluginHostPermission::MediaReadUrl, Permission::MediaReadUrl)
                    | (PluginHostPermission::MediaReadUrl, Permission::MediaRead)
                    | (
                        PluginHostPermission::MetadataWrite,
                        Permission::MetadataWrite
                    )
                    | (
                        PluginHostPermission::LibraryFileRead,
                        Permission::FileRead(_)
                    )
                    | (
                        PluginHostPermission::LibraryFileWrite,
                        Permission::FileWrite(_)
                    )
                    | (PluginHostPermission::DatabaseRead, Permission::DatabaseRead)
                    | (
                        PluginHostPermission::DatabaseWrite,
                        Permission::DatabaseWrite
                    )
                    | (PluginHostPermission::TaskCreate, Permission::TaskCreate)
                    | (PluginHostPermission::CacheRead, Permission::CacheRead)
                    | (PluginHostPermission::CacheRead, Permission::CacheWrite)
                    | (PluginHostPermission::CacheWrite, Permission::CacheWrite)
            )
        })
    }

    async fn invoke_authorized(
        &self,
        plugin_id: &str,
        user: &PluginHostUser,
        method: &str,
        params: Value,
    ) -> Result<Value> {
        match method {
            "books.list" => self.books_list(user, &params).await,
            "books.get" => self.books_get(user, &params).await,
            "libraries.list" => self.libraries_list(user, &params).await,
            "libraries.get" => self.libraries_get(user, &params).await,
            "chapters.list" => self.chapters_list(user, &params).await,
            "chapters.get" => self.chapters_get(user, &params).await,
            "progress.recent" => self.progress_recent(user, &params).await,
            "media.get_url" => self.media_get_url(user, &params).await,
            "metadata.write" => self.metadata_write(user, &params).await,
            "library.file.list" => self.library_file_list(user, &params).await,
            "library.file.stat" => self.library_file_stat(user, &params).await,
            "library.file.read" => self.library_file_read(user, &params).await,
            "library.file.write" => self.library_file_write(user, &params).await,
            "database.get" => self.database_get(user, &params).await,
            "database.list" => self.database_list(user, &params).await,
            "database.update" => self.database_update(user, &params).await,
            "tasks.create" => self.tasks_create(&params).await,
            "cache.get" => self.cache_get(plugin_id, &params).await,
            "cache.set" => self.cache_set(plugin_id, &params).await,
            "cache.has" => self.cache_has(plugin_id, &params).await,
            "cache.delete" => self.cache_delete(plugin_id, &params).await,
            _ => Err(TingError::InvalidRequest(format!(
                "Unknown plugin host method: {}",
                method
            ))),
        }
    }

    async fn ensure_user_can_access_book(
        &self,
        user: &PluginHostUser,
        book_id: &str,
    ) -> Result<()> {
        let can_access = self
            .book_repo
            .check_access(book_id, &user.id, user.is_admin())
            .await?;

        if can_access {
            Ok(())
        } else {
            Err(TingError::PermissionDenied(format!(
                "User cannot access book {}",
                book_id
            )))
        }
    }

    async fn ensure_user_can_access_library(
        &self,
        user: &PluginHostUser,
        library_id: &str,
    ) -> Result<()> {
        if user.is_admin() {
            return Ok(());
        }

        let accessible = self
            .library_repo
            .find_by_user_access(&user.id)
            .await?
            .into_iter()
            .any(|library| library.id == library_id);

        if accessible {
            Ok(())
        } else {
            Err(TingError::PermissionDenied(format!(
                "User cannot access library {}",
                library_id
            )))
        }
    }
}

fn required_string_param(params: &Value, name: &str) -> Result<String> {
    string_param(params, name).ok_or_else(|| {
        TingError::InvalidRequest(format!("Missing required plugin host parameter: {}", name))
    })
}

fn string_param(params: &Value, name: &str) -> Option<String> {
    params
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn usize_param(params: &Value, name: &str) -> Option<usize> {
    params
        .get(name)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn bool_param(params: &Value, name: &str) -> Option<bool> {
    params.get(name).and_then(Value::as_bool)
}

fn plugin_task_priority(params: &Value) -> Priority {
    match string_param(params, "priority")
        .unwrap_or_default()
        .as_str()
    {
        "low" => Priority::Low,
        "high" => Priority::High,
        _ => Priority::Normal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plugin_host_permission_requires_matching_manifest_permission() {
        assert_eq!(
            PluginHostGateway::required_permission("books.list"),
            Some(PluginHostPermission::BooksRead)
        );
        assert_eq!(
            PluginHostGateway::required_permission("chapters.get"),
            Some(PluginHostPermission::ChaptersRead)
        );
        assert_eq!(
            PluginHostGateway::required_permission("libraries.list"),
            Some(PluginHostPermission::BooksRead)
        );
        assert_eq!(
            PluginHostGateway::required_permission("progress.recent"),
            Some(PluginHostPermission::ProgressRead)
        );
        assert_eq!(
            PluginHostGateway::required_permission("media.get_url"),
            Some(PluginHostPermission::MediaReadUrl)
        );
        assert_eq!(
            PluginHostGateway::required_permission("metadata.write"),
            Some(PluginHostPermission::MetadataWrite)
        );
        assert_eq!(
            PluginHostGateway::required_permission("tasks.create"),
            Some(PluginHostPermission::TaskCreate)
        );
        assert_eq!(
            PluginHostGateway::required_permission("cache.get"),
            Some(PluginHostPermission::CacheRead)
        );
        assert_eq!(
            PluginHostGateway::required_permission("cache.set"),
            Some(PluginHostPermission::CacheWrite)
        );
        assert_eq!(
            PluginHostGateway::required_permission("unknown.method"),
            None
        );

        assert!(PluginHostGateway::has_permission(
            &[Permission::BooksRead],
            PluginHostPermission::BooksRead
        ));
        assert!(!PluginHostGateway::has_permission(
            &[Permission::BooksRead],
            PluginHostPermission::ChaptersRead
        ));
        assert!(PluginHostGateway::has_permission(
            &[Permission::DatabaseRead],
            PluginHostPermission::ChaptersRead
        ));
        assert!(PluginHostGateway::has_permission(
            &[Permission::ProgressRead],
            PluginHostPermission::ProgressRead
        ));
        assert!(!PluginHostGateway::has_permission(
            &[Permission::BooksRead],
            PluginHostPermission::ProgressRead
        ));
        assert!(PluginHostGateway::has_permission(
            &[Permission::MediaReadUrl],
            PluginHostPermission::MediaReadUrl
        ));
        assert!(PluginHostGateway::has_permission(
            &[Permission::MediaRead],
            PluginHostPermission::MediaReadUrl
        ));
        assert!(!PluginHostGateway::has_permission(
            &[Permission::BooksRead],
            PluginHostPermission::MediaReadUrl
        ));
        assert!(PluginHostGateway::has_permission(
            &[Permission::MetadataWrite],
            PluginHostPermission::MetadataWrite
        ));
        assert!(!PluginHostGateway::has_permission(
            &[Permission::DatabaseWrite],
            PluginHostPermission::MetadataWrite
        ));
        assert!(PluginHostGateway::has_permission(
            &[Permission::TaskCreate],
            PluginHostPermission::TaskCreate
        ));
        assert!(!PluginHostGateway::has_permission(
            &[Permission::DatabaseWrite],
            PluginHostPermission::TaskCreate
        ));
        assert!(PluginHostGateway::has_permission(
            &[Permission::CacheRead],
            PluginHostPermission::CacheRead
        ));
        assert!(PluginHostGateway::has_permission(
            &[Permission::CacheWrite],
            PluginHostPermission::CacheRead
        ));
        assert!(PluginHostGateway::has_permission(
            &[Permission::CacheWrite],
            PluginHostPermission::CacheWrite
        ));
        assert!(!PluginHostGateway::has_permission(
            &[Permission::CacheRead],
            PluginHostPermission::CacheWrite
        ));
    }

    #[test]
    fn plugin_host_user_from_invocation_args_reads_capability_context() {
        let args = json!({
            "_context": {
                "route": {
                    "authenticated": true,
                    "user": {
                        "id": "user-1",
                        "username": "alice",
                        "role": "admin"
                    }
                }
            }
        });

        let user = super::plugin_host_user_from_invocation_args(&args).unwrap();
        assert_eq!(user.id, "user-1");
        assert_eq!(user.username, "alice");
        assert_eq!(user.role, "admin");
    }

    #[test]
    fn plugin_host_user_from_invocation_args_rejects_public_context() {
        let args = json!({
            "context": {
                "authenticated": false,
                "user": {
                    "id": "user-1",
                    "username": "alice",
                    "role": "user"
                }
            }
        });

        assert!(super::plugin_host_user_from_invocation_args(&args).is_none());
    }

    #[test]
    fn plugin_host_authorize_rejects_unknown_or_missing_permissions() {
        assert!(
            PluginHostGateway::authorize("demo", &[Permission::BooksRead], "books.list").is_ok()
        );
        assert!(
            PluginHostGateway::authorize("demo", &[Permission::DatabaseRead], "chapters.get")
                .is_ok()
        );

        let unknown =
            PluginHostGateway::authorize("demo", &[Permission::BooksRead], "unknown.method")
                .unwrap_err();
        assert!(matches!(unknown, TingError::InvalidRequest(_)));

        let missing = PluginHostGateway::authorize("demo", &[Permission::BooksRead], "cache.set")
            .unwrap_err();
        assert!(matches!(missing, TingError::PermissionDenied(_)));
    }

    #[test]
    fn plugin_host_param_helpers_parse_optional_values() {
        let params = json!({
            "book_id": " book-1 ",
            "limit": 25,
            "download": true,
            "empty": " "
        });

        assert_eq!(string_param(&params, "book_id"), Some("book-1".to_string()));
        assert_eq!(string_param(&params, "empty"), None);
        assert_eq!(usize_param(&params, "limit"), Some(25));
        assert_eq!(bool_param(&params, "download"), Some(true));
        assert!(required_string_param(&params, "missing").is_err());
    }

    #[test]
    fn plugin_task_priority_defaults_to_normal() {
        assert_eq!(plugin_task_priority(&json!({})), Priority::Normal);
        assert_eq!(
            plugin_task_priority(&json!({ "priority": "high" })),
            Priority::High
        );
        assert_eq!(
            plugin_task_priority(&json!({ "priority": "low" })),
            Priority::Low
        );
    }
}
