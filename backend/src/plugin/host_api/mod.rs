//! Shared plugin Host services, authorization and invocation dispatch.

use self::logger::{PluginLogLevel, emit_plugin_event};
use crate::core::Config;
use crate::core::app::error::{AccessErrorKind, InputErrorKind, LookupErrorKind};
use crate::core::app::error::{Result, TingError};
pub mod cache;
mod capabilities;
mod data;
mod dispatch;
mod events;
mod files;
mod html;
pub(crate) mod http;
pub mod logger;
mod personal;
pub mod resources;
mod storage;
mod tasks_cache;

pub(crate) use dispatch::{http_request, invoke};

use crate::core::task_queue::{Priority, TaskQueue};
use crate::db::repository::{
    BookRepository, ChapterRepository, FavoriteRepository, LibraryRepository, PlaylistRepository,
    ProgressRepository, UserSettingsRepository,
};
use crate::plugin::PluginCache;
use crate::plugin::manager::PluginManager;
use crate::plugin::sandbox::Permission;
use serde_json::Value;
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone)]
pub struct PluginHostGateway {
    book_repo: Arc<BookRepository>,
    library_repo: Arc<LibraryRepository>,
    chapter_repo: Arc<ChapterRepository>,
    progress_repo: Arc<ProgressRepository>,
    playlist_repo: Arc<PlaylistRepository>,
    favorite_repo: Arc<FavoriteRepository>,
    settings_repo: Arc<UserSettingsRepository>,
    task_queue: Arc<TaskQueue>,
    plugin_manager: Arc<PluginManager>,
    plugin_cache: Arc<PluginCache>,
    plugin_storage: Arc<PluginCache>,
    config_manager: Arc<crate::plugin::PluginConfigManager>,
    plugin_route_revocations: Arc<crate::core::security::signing::PluginRouteRevocations>,
    encryption_key: Arc<[u8; 32]>,
    config: Config,
    html_documents: Arc<RwLock<std::collections::HashMap<String, String>>>,
}

pub struct PluginHostGatewayDependencies {
    pub book_repo: Arc<BookRepository>,
    pub library_repo: Arc<LibraryRepository>,
    pub chapter_repo: Arc<ChapterRepository>,
    pub progress_repo: Arc<ProgressRepository>,
    pub playlist_repo: Arc<PlaylistRepository>,
    pub favorite_repo: Arc<FavoriteRepository>,
    pub settings_repo: Arc<UserSettingsRepository>,
    pub task_queue: Arc<TaskQueue>,
    pub plugin_manager: Arc<PluginManager>,
    pub plugin_cache: Arc<PluginCache>,
    pub plugin_storage: Arc<PluginCache>,
    pub config_manager: Arc<crate::plugin::PluginConfigManager>,
    pub plugin_route_revocations: Arc<crate::core::security::signing::PluginRouteRevocations>,
    pub encryption_key: Arc<[u8; 32]>,
    pub config: Config,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginHostPermission {
    /// Built-in runtime scheduling primitives do not expose host data and
    /// therefore never require a manifest permission.
    Runtime,
    NetworkAccess,
    BooksRead,
    BooksWrite,
    LibrariesRead,
    ChaptersRead,
    ChaptersWrite,
    LibrariesWrite,
    ProgressRead,
    MediaReadUrl,
    PluginRouteSign,
    MetadataWrite,
    LibraryFileRead,
    LibraryFileWrite,
    TaskCreate,
    CacheRead,
    CacheWrite,
    PlaylistsRead,
    PlaylistsWrite,
    FavoritesRead,
    FavoritesWrite,
    BookmarksRead,
    BookmarksWrite,
    UserSettingsRead,
    UserSettingsWrite,
    ConfigRead,
    PrivateFileRead,
    PrivateFileWrite,
    StorageRead,
    StorageWrite,
    CapabilityInvoke,
    PluginRouteRevoke,
    TaskRead,
    TaskManage,
    TaskProgress,
    EventPublish,
    HtmlParse,
}

impl PluginHostGateway {
    pub(crate) fn plugin_permissions(&self, plugin_id: &str) -> Result<Vec<Permission>> {
        self.plugin_manager
            .get_plugin(&plugin_id.to_string())
            .map(|metadata| metadata.permissions)
    }

    pub fn new(dependencies: PluginHostGatewayDependencies) -> Self {
        let PluginHostGatewayDependencies {
            book_repo,
            library_repo,
            chapter_repo,
            progress_repo,
            playlist_repo,
            favorite_repo,
            settings_repo,
            task_queue,
            plugin_manager,
            plugin_cache,
            plugin_storage,
            config_manager,
            plugin_route_revocations,
            encryption_key,
            config,
        } = dependencies;
        Self {
            book_repo,
            library_repo,
            chapter_repo,
            progress_repo,
            playlist_repo,
            favorite_repo,
            settings_repo,
            task_queue,
            plugin_manager,
            plugin_cache,
            plugin_storage,
            config_manager,
            plugin_route_revocations,
            encryption_key,
            config,
            html_documents: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub(crate) async fn delete_plugin_cache(&self, plugin_id: &str) -> Result<bool> {
        self.plugin_cache.delete_plugin(plugin_id).await
    }

    pub(crate) async fn migrate_plugin_cache(
        &self,
        old_plugin_id: &str,
        new_plugin_id: &str,
    ) -> Result<bool> {
        self.plugin_cache
            .migrate_plugin(old_plugin_id, new_plugin_id)
            .await
    }

    pub async fn invoke_plugin(
        &self,
        plugin_id: &str,
        user: Option<&PluginHostUser>,
        method: &str,
        params: Value,
        resources: Option<&Arc<crate::plugin::host_api::resources::ResourceScope>>,
    ) -> Result<Value> {
        let plugin_id_owned = plugin_id.to_string();
        let metadata = self
            .plugin_manager
            .get_plugin(&plugin_id_owned)
            .map_err(|_| {
                TingError::plugin_not_found(
                    LookupErrorKind::Plugin,
                    "invoke_plugin::plugin_not_found",
                    plugin_id_owned,
                )
            })?;

        let result = self
            .invoke_with_permissions(
                plugin_id,
                &metadata.permissions,
                user,
                method,
                params,
                resources,
            )
            .await;
        let denied = result.as_ref().err().is_some_and(|error| {
            matches!(
                error.root_error(),
                TingError::PermissionDenied(_)
                    | TingError::SecurityViolation(_)
                    | TingError::InvalidRequest(_)
            )
        });
        let fields = serde_json::json!({
            "op": "host_gateway.invoke",
            "method": method,
            "status": if result.is_ok() {
                "success"
            } else if denied {
                "denied"
            } else {
                "error"
            },
        });
        emit_plugin_event(
            &metadata,
            if denied {
                crate::plugin::types::PluginLogSource::Security
            } else {
                crate::plugin::types::PluginLogSource::Gateway
            },
            if result.is_ok() {
                PluginLogLevel::Debug
            } else if denied {
                PluginLogLevel::Warn
            } else {
                PluginLogLevel::Error
            },
            if result.is_ok() {
                "HostGateway invocation completed"
            } else if denied {
                "HostGateway invocation denied"
            } else {
                "HostGateway invocation failed"
            },
            Some(&fields),
        );
        result
    }

    pub async fn invoke_with_permissions(
        &self,
        plugin_id: &str,
        permissions: &[Permission],
        user: Option<&PluginHostUser>,
        method: &str,
        params: Value,
        resources: Option<&Arc<crate::plugin::host_api::resources::ResourceScope>>,
    ) -> Result<Value> {
        Self::authorize(plugin_id, permissions, method)?;
        self.invoke_authorized(plugin_id, user, permissions, method, params, resources)
            .await
    }

    pub fn authorize(plugin_id: &str, permissions: &[Permission], method: &str) -> Result<()> {
        let required_permission = Self::required_permission(method).ok_or_else(|| {
            TingError::invalid_request(
                InputErrorKind::UnsupportedValue,
                "authorize::invalid_request",
                format!("Unknown plugin host method: {}", method),
            )
        })?;

        if !Self::has_permission(permissions, required_permission) {
            return Err(TingError::permission(
                AccessErrorKind::CapabilityDenied,
                "authorize::permission",
                format!(
                    "Plugin {} lacks permission required for host method {}",
                    plugin_id, method
                ),
            ));
        }

        Ok(())
    }

    pub fn required_permission(method: &str) -> Option<PluginHostPermission> {
        match method {
            "runtime.sleep" => Some(PluginHostPermission::Runtime),
            "http.request" => Some(PluginHostPermission::NetworkAccess),
            "books.list" | "books.get" => Some(PluginHostPermission::BooksRead),
            "libraries.list" | "libraries.get" => Some(PluginHostPermission::LibrariesRead),
            "books.update" => Some(PluginHostPermission::BooksWrite),
            "chapters.list" | "chapters.get" => Some(PluginHostPermission::ChaptersRead),
            "chapters.update" => Some(PluginHostPermission::ChaptersWrite),
            "libraries.update" => Some(PluginHostPermission::LibrariesWrite),
            "progress.recent" => Some(PluginHostPermission::ProgressRead),
            "media.get_url" | "media.get_signed_url" => Some(PluginHostPermission::MediaReadUrl),
            "plugin_routes.sign" => Some(PluginHostPermission::PluginRouteSign),
            "plugin_routes.revoke" => Some(PluginHostPermission::PluginRouteRevoke),
            "metadata.write" => Some(PluginHostPermission::MetadataWrite),
            "library.file.list" | "library.file.stat" | "library.file.open" => {
                Some(PluginHostPermission::LibraryFileRead)
            }
            "files.open" | "files.stat" | "files.list" => {
                Some(PluginHostPermission::PrivateFileRead)
            }
            "files.remove" => Some(PluginHostPermission::PrivateFileWrite),
            "assets.commit" => Some(PluginHostPermission::LibraryFileWrite),
            "tasks.create" => Some(PluginHostPermission::TaskCreate),
            "tasks.get" => Some(PluginHostPermission::TaskRead),
            "tasks.cancel" => Some(PluginHostPermission::TaskManage),
            "tasks.report_progress" => Some(PluginHostPermission::TaskProgress),
            "cache.get" | "cache.has" => Some(PluginHostPermission::CacheRead),
            "cache.set" | "cache.delete" => Some(PluginHostPermission::CacheWrite),
            "storage.get" | "storage.list" => Some(PluginHostPermission::StorageRead),
            "storage.set" | "storage.delete" => Some(PluginHostPermission::StorageWrite),
            "config.get" => Some(PluginHostPermission::ConfigRead),
            "capabilities.invoke" => Some(PluginHostPermission::CapabilityInvoke),
            "events.publish" => Some(PluginHostPermission::EventPublish),
            "html.parse" | "html.select" | "html.text" | "html.attr" | "html.extract"
            | "html.close" => Some(PluginHostPermission::HtmlParse),
            "playlists.list" | "playlists.get" => Some(PluginHostPermission::PlaylistsRead),
            "playlists.create"
            | "playlists.update"
            | "playlists.delete"
            | "playlists.add_item"
            | "playlists.remove_item" => Some(PluginHostPermission::PlaylistsWrite),
            "favorites.list" => Some(PluginHostPermission::FavoritesRead),
            "favorites.add" | "favorites.remove" => Some(PluginHostPermission::FavoritesWrite),
            "bookmarks.books" | "bookmarks.list" | "bookmarks.get" => {
                Some(PluginHostPermission::BookmarksRead)
            }
            "bookmarks.create" | "bookmarks.update" | "bookmarks.delete" => {
                Some(PluginHostPermission::BookmarksWrite)
            }
            "user_settings.get" => Some(PluginHostPermission::UserSettingsRead),
            "user_settings.set" => Some(PluginHostPermission::UserSettingsWrite),
            _ => None,
        }
    }

    pub fn has_permission(
        permissions: &[Permission],
        required_permission: PluginHostPermission,
    ) -> bool {
        if required_permission == PluginHostPermission::Runtime {
            return true;
        }
        permissions.iter().any(|permission| {
            matches!(
                (required_permission, permission),
                (
                    PluginHostPermission::NetworkAccess,
                    Permission::NetworkAccess { .. }
                ) | (PluginHostPermission::BooksRead, Permission::BooksRead)
                    | (PluginHostPermission::BooksRead, Permission::BooksWrite)
                    | (
                        PluginHostPermission::LibrariesRead,
                        Permission::LibrariesRead
                    )
                    | (PluginHostPermission::BooksWrite, Permission::BooksWrite)
                    | (PluginHostPermission::ChaptersRead, Permission::ChaptersRead)
                    | (
                        PluginHostPermission::ChaptersRead,
                        Permission::ChaptersWrite
                    )
                    | (
                        PluginHostPermission::ChaptersWrite,
                        Permission::ChaptersWrite
                    )
                    | (
                        PluginHostPermission::LibrariesWrite,
                        Permission::LibrariesWrite
                    )
                    | (PluginHostPermission::ProgressRead, Permission::ProgressRead)
                    | (PluginHostPermission::MediaReadUrl, Permission::MediaReadUrl)
                    | (
                        PluginHostPermission::PluginRouteSign,
                        Permission::PluginRouteSign
                    )
                    | (
                        PluginHostPermission::MetadataWrite,
                        Permission::MetadataWrite
                    )
                    | (
                        PluginHostPermission::LibraryFileRead,
                        Permission::FileRead { .. }
                    )
                    | (
                        PluginHostPermission::LibraryFileWrite,
                        Permission::FileWrite { .. }
                    )
                    | (PluginHostPermission::TaskCreate, Permission::TaskCreate)
                    | (PluginHostPermission::CacheRead, Permission::CacheRead)
                    | (PluginHostPermission::CacheRead, Permission::CacheWrite)
                    | (PluginHostPermission::CacheWrite, Permission::CacheWrite)
                    | (
                        PluginHostPermission::PlaylistsRead,
                        Permission::PlaylistsRead
                    )
                    | (
                        PluginHostPermission::PlaylistsRead,
                        Permission::PlaylistsWrite
                    )
                    | (
                        PluginHostPermission::PlaylistsWrite,
                        Permission::PlaylistsWrite
                    )
                    | (
                        PluginHostPermission::FavoritesRead,
                        Permission::FavoritesRead
                    )
                    | (
                        PluginHostPermission::FavoritesRead,
                        Permission::FavoritesWrite
                    )
                    | (
                        PluginHostPermission::FavoritesWrite,
                        Permission::FavoritesWrite
                    )
                    | (
                        PluginHostPermission::BookmarksRead,
                        Permission::BookmarksRead
                    )
                    | (
                        PluginHostPermission::BookmarksRead,
                        Permission::BookmarksWrite
                    )
                    | (
                        PluginHostPermission::BookmarksWrite,
                        Permission::BookmarksWrite
                    )
                    | (
                        PluginHostPermission::UserSettingsRead,
                        Permission::UserSettingsRead
                    )
                    | (
                        PluginHostPermission::UserSettingsRead,
                        Permission::UserSettingsWrite
                    )
                    | (
                        PluginHostPermission::UserSettingsWrite,
                        Permission::UserSettingsWrite
                    )
                    | (PluginHostPermission::ConfigRead, Permission::ConfigRead)
                    | (
                        PluginHostPermission::PrivateFileRead,
                        Permission::FileRead { .. }
                    )
                    | (
                        PluginHostPermission::PrivateFileWrite,
                        Permission::FileWrite { .. }
                    )
                    | (PluginHostPermission::StorageRead, Permission::StorageRead)
                    | (PluginHostPermission::StorageRead, Permission::StorageWrite)
                    | (PluginHostPermission::StorageWrite, Permission::StorageWrite)
                    | (
                        PluginHostPermission::CapabilityInvoke,
                        Permission::CapabilityInvoke { .. }
                    )
                    | (
                        PluginHostPermission::PluginRouteRevoke,
                        Permission::PluginRouteRevoke
                    )
                    | (PluginHostPermission::TaskRead, Permission::TaskRead)
                    | (PluginHostPermission::TaskManage, Permission::TaskManage)
                    | (PluginHostPermission::TaskProgress, Permission::TaskProgress)
                    | (PluginHostPermission::EventPublish, Permission::EventPublish)
                    | (PluginHostPermission::HtmlParse, Permission::HtmlParse)
            )
        })
    }

    async fn invoke_authorized(
        &self,
        plugin_id: &str,
        user: Option<&PluginHostUser>,
        permissions: &[Permission],
        method: &str,
        params: Value,
        resources: Option<&Arc<crate::plugin::host_api::resources::ResourceScope>>,
    ) -> Result<Value> {
        match method {
            "runtime.sleep" => {
                let milliseconds = params
                    .get("milliseconds")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| {
                        TingError::invalid_request(
                            InputErrorKind::InvalidValue,
                            "runtime.sleep",
                            "milliseconds must be an integer".into(),
                        )
                    })?;
                if milliseconds > 15_000 {
                    return Err(TingError::invalid_request(
                        InputErrorKind::InvalidValue,
                        "runtime.sleep",
                        "milliseconds must not exceed 15000".into(),
                    ));
                }
                tokio::time::sleep(std::time::Duration::from_millis(milliseconds)).await;
                Ok(Value::Object(serde_json::Map::new()))
            }
            "http.request" => {
                crate::plugin::host_api::http_request(permissions, &params, resources).await
            }
            // HTML documents are plugin-owned parsing resources, including in
            // background scraping jobs that have no user principal.
            "html.parse" => self.html_parse(plugin_id, &params).await,
            "html.select" => self.html_select(plugin_id, &params).await,
            "html.text" => self.html_text(plugin_id, &params).await,
            "html.attr" => self.html_attr(plugin_id, &params).await,
            "html.extract" => self.html_extract(plugin_id, &params).await,
            "html.close" => self.html_close(plugin_id, &params).await,
            _ => {
                let user = user.ok_or_else(|| {
                    TingError::permission(
                        AccessErrorKind::UserRequired,
                        "invoke_authorized::permission",
                        "This Host method requires a user principal".into(),
                    )
                })?;
                self.invoke_authorized_user(plugin_id, user, permissions, method, params, resources)
                    .await
            }
        }
    }

    async fn invoke_authorized_user(
        &self,
        plugin_id: &str,
        user: &PluginHostUser,
        permissions: &[Permission],
        method: &str,
        params: Value,
        resources: Option<&Arc<crate::plugin::host_api::resources::ResourceScope>>,
    ) -> Result<Value> {
        match method {
            "books.list" => self.books_list(user, &params).await,
            "books.get" => self.books_get(user, &params).await,
            "books.update" => self.books_update(user, &params).await,
            "libraries.list" => self.libraries_list(user, &params).await,
            "libraries.get" => self.libraries_get(user, &params).await,
            "libraries.update" => self.libraries_update(user, &params).await,
            "chapters.list" => self.chapters_list(user, &params).await,
            "chapters.get" => self.chapters_get(user, &params).await,
            "chapters.update" => self.chapters_update(user, &params).await,
            "progress.recent" => self.progress_recent(user, &params).await,
            "media.get_url" => self.media_get_url(user, &params).await,
            "media.get_signed_url" => self.media_get_signed_url(user, &params).await,
            "plugin_routes.sign" => self.plugin_routes_sign(plugin_id, user, &params).await,
            "plugin_routes.revoke" => self.plugin_routes_revoke(plugin_id, user, &params).await,
            "metadata.write" => self.metadata_write(user, &params).await,
            "config.get" => self.config_get(plugin_id).await,
            "files.open" => self.private_file_open(plugin_id, &params, resources).await,
            "files.stat" => self.private_file_stat(plugin_id, &params).await,
            "files.list" => self.private_file_list(plugin_id, &params).await,
            "files.remove" => self.private_file_remove(plugin_id, &params).await,
            "library.file.list" => self.library_file_list(user, &params).await,
            "library.file.stat" => self.library_file_stat(user, &params).await,
            "library.file.open" => self.library_file_open(user, &params, resources).await,
            "assets.commit" => self.assets_commit(user, &params, resources).await,
            "tasks.create" => self.tasks_create(plugin_id, user, &params).await,
            "tasks.get" => self.tasks_get(plugin_id, user, &params).await,
            "tasks.cancel" => self.tasks_cancel(plugin_id, user, &params).await,
            "tasks.report_progress" => self.tasks_report_progress(plugin_id, user, &params).await,
            "cache.get" => self.cache_get(plugin_id, &params).await,
            "cache.set" => self.cache_set(plugin_id, &params).await,
            "cache.has" => self.cache_has(plugin_id, &params).await,
            "cache.delete" => self.cache_delete(plugin_id, &params).await,
            "storage.get" => self.storage_get(plugin_id, user, &params).await,
            "storage.set" => self.storage_set(plugin_id, user, &params).await,
            "storage.delete" => self.storage_delete(plugin_id, user, &params).await,
            "storage.list" => self.storage_list(plugin_id, user, &params).await,
            "capabilities.invoke" => {
                self.capabilities_invoke(plugin_id, user, permissions, &params, resources)
                    .await
            }
            "events.publish" => self.events_publish(plugin_id, user, &params).await,
            "playlists.list" => self.playlists_list(user, &params).await,
            "playlists.get" => self.playlists_get(user, &params).await,
            "playlists.create" => self.playlists_create(user, &params).await,
            "playlists.update" => self.playlists_update(user, &params).await,
            "playlists.delete" => self.playlists_delete(user, &params).await,
            "playlists.add_item" => self.playlists_add_item(user, &params).await,
            "playlists.remove_item" => self.playlists_remove_item(user, &params).await,
            "favorites.list" => self.favorites_list(user, &params).await,
            "favorites.add" => self.favorites_add(user, &params).await,
            "favorites.remove" => self.favorites_remove(user, &params).await,
            "bookmarks.books" | "bookmarks.list" | "bookmarks.get" | "bookmarks.create"
            | "bookmarks.update" | "bookmarks.delete" => {
                self.bookmarks_invoke(user, method, &params).await
            }
            "user_settings.get" => self.user_settings_get(user, &params).await,
            "user_settings.set" => self.user_settings_set(user, &params).await,
            _ => Err(TingError::invalid_request(
                InputErrorKind::UnsupportedValue,
                "invoke_authorized_user::invalid_request",
                format!("Unknown plugin host method: {}", method),
            )),
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
            Err(TingError::permission(
                AccessErrorKind::ResourceDenied,
                "ensure_user_can_access_book::permission",
                format!("User cannot access book {}", book_id),
            ))
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
            Err(TingError::permission(
                AccessErrorKind::ResourceDenied,
                "ensure_user_can_access_library::permission",
                format!("User cannot access library {}", library_id),
            ))
        }
    }
}

/// Native and WASM share the contract's numeric Host transport status codes.
pub(crate) fn status_for_plugin_error(
    code: ting_plugin_contract::protocol::PluginErrorCode,
) -> ting_plugin_contract::native_abi::NativeStatus {
    use ting_plugin_contract::native_abi::NativeStatus;
    use ting_plugin_contract::protocol::PluginErrorCode;
    match code {
        PluginErrorCode::InvalidInput => NativeStatus::InvalidInput,
        PluginErrorCode::NotFound => NativeStatus::NotFound,
        PluginErrorCode::PermissionDenied => NativeStatus::PermissionDenied,
        PluginErrorCode::ResourceLimit => NativeStatus::ResourceLimit,
        PluginErrorCode::Cancelled => NativeStatus::Cancelled,
        PluginErrorCode::UnsupportedOperation => NativeStatus::UnsupportedOperation,
        _ => NativeStatus::InternalError,
    }
}

fn required_string_param(params: &Value, name: &str) -> Result<String> {
    string_param(params, name).ok_or_else(|| {
        TingError::invalid_request(
            InputErrorKind::MissingField,
            "required_string_param::invalid_request",
            format!("Missing required plugin host parameter: {}", name),
        )
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
#[path = "../../../tests/unit/plugin/host_api.rs"]
mod tests;
