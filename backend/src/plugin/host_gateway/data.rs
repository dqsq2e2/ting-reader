use super::{
    bool_param, required_string_param, string_param, usize_param, PluginHostGateway, PluginHostUser,
};
use crate::core::error::{Result, TingError};
use crate::core::signing::{
    normalize_plugin_route_sign_path, sign_media_stream_request, sign_plugin_route_request,
    signature_expires_from_ttl, DEFAULT_MEDIA_SIGNATURE_TTL_SECONDS,
    DEFAULT_PLUGIN_ROUTE_SIGNATURE_TTL_SECONDS, MAX_MEDIA_SIGNATURE_TTL_SECONDS,
    MAX_PLUGIN_ROUTE_SIGNATURE_TTL_SECONDS,
};
use crate::core::task_queue::{Priority, Task, TaskPayload};
use crate::db::models::Library;
use crate::db::repository::Repository;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

impl PluginHostGateway {
    pub(super) async fn books_list(&self, user: &PluginHostUser, params: &Value) -> Result<Value> {
        let limit = usize_param(params, "limit").unwrap_or(50).clamp(1, 200);
        let offset = usize_param(params, "offset").unwrap_or(0);
        let books = self
            .book_repo
            .find_with_filters(
                &user.id,
                user.is_admin(),
                string_param(params, "search"),
                string_param(params, "tag"),
                string_param(params, "library_id"),
            )
            .await?;
        let total = books.len();
        let items = books
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "items": items,
            "total": total,
            "offset": offset,
            "limit": limit,
        }))
    }

    pub(super) async fn books_get(&self, user: &PluginHostUser, params: &Value) -> Result<Value> {
        let book_id = required_string_param(params, "book_id")
            .or_else(|_| required_string_param(params, "id"))?;
        self.ensure_user_can_access_book(user, &book_id).await?;

        let book =
            self.book_repo.find_by_id(&book_id).await?.ok_or_else(|| {
                TingError::NotFound(format!("Book with id {} not found", book_id))
            })?;

        serde_json::to_value(book)
            .map_err(|e| TingError::SerializationError(format!("Book serialization failed: {}", e)))
    }

    pub(super) async fn libraries_list(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let limit = usize_param(params, "limit").unwrap_or(50).clamp(1, 200);
        let offset = usize_param(params, "offset").unwrap_or(0);
        let libraries = if user.is_admin() {
            self.library_repo.find_all().await?
        } else {
            self.library_repo.find_by_user_access(&user.id).await?
        };
        let total = libraries.len();
        let items = libraries
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(plugin_host_library_value)
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "items": items,
            "total": total,
            "offset": offset,
            "limit": limit,
        }))
    }

    pub(super) async fn libraries_get(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let library_id = required_string_param(params, "library_id")
            .or_else(|_| required_string_param(params, "id"))?;
        self.ensure_user_can_access_library(user, &library_id)
            .await?;

        let library = self
            .library_repo
            .find_by_id(&library_id)
            .await?
            .ok_or_else(|| {
                TingError::NotFound(format!("Library with id {} not found", library_id))
            })?;

        Ok(plugin_host_library_value(library))
    }

    pub(super) async fn chapters_list(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let book_id = required_string_param(params, "book_id")?;
        self.ensure_user_can_access_book(user, &book_id).await?;

        let limit = usize_param(params, "limit").unwrap_or(200).clamp(1, 500);
        let offset = usize_param(params, "offset").unwrap_or(0);
        let chapters = self.chapter_repo.find_by_book(&book_id).await?;
        let total = chapters.len();
        let items = chapters
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "items": items,
            "total": total,
            "offset": offset,
            "limit": limit,
        }))
    }

    pub(super) async fn chapters_get(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let chapter_id = required_string_param(params, "chapter_id")
            .or_else(|_| required_string_param(params, "id"))?;
        let chapter = self
            .chapter_repo
            .find_by_id(&chapter_id)
            .await?
            .ok_or_else(|| {
                TingError::NotFound(format!("Chapter with id {} not found", chapter_id))
            })?;

        self.ensure_user_can_access_book(user, &chapter.book_id)
            .await?;

        serde_json::to_value(chapter).map_err(|e| {
            TingError::SerializationError(format!("Chapter serialization failed: {}", e))
        })
    }

    pub(super) async fn progress_recent(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let limit = usize_param(params, "limit").unwrap_or(20).clamp(1, 100) as i32;
        let items = self
            .progress_repo
            .get_recent_enriched(&user.id, Some(limit))
            .await?
            .into_iter()
            .map(
                |(progress, book_title, cover_url, library_id, chapter_title, chapter_duration)| {
                    serde_json::json!({
                        "id": progress.id,
                        "book_id": progress.book_id,
                        "chapter_id": progress.chapter_id,
                        "position": progress.position,
                        "duration": progress.duration,
                        "updated_at": progress.updated_at,
                        "book_title": book_title,
                        "cover_url": cover_url,
                        "library_id": library_id,
                        "chapter_title": chapter_title,
                        "chapter_duration": chapter_duration,
                    })
                },
            )
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "items": items,
            "limit": limit,
        }))
    }

    pub(super) async fn media_get_url(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let chapter_id = required_string_param(params, "chapter_id")
            .or_else(|_| required_string_param(params, "id"))?;
        let chapter = self
            .chapter_repo
            .find_by_id(&chapter_id)
            .await?
            .ok_or_else(|| {
                TingError::NotFound(format!("Chapter with id {} not found", chapter_id))
            })?;

        self.ensure_user_can_access_book(user, &chapter.book_id)
            .await?;

        let mut query = Vec::new();
        if let Some(transcode) = string_param(params, "transcode") {
            if !matches!(transcode.as_str(), "hls" | "mp3" | "wav") {
                return Err(TingError::InvalidRequest(format!(
                    "Unsupported media transcode target: {}",
                    transcode
                )));
            }
            query.push(format!("transcode={}", urlencoding::encode(&transcode)));
        }
        if let Some(seek) = string_param(params, "seek") {
            query.push(format!("seek={}", urlencoding::encode(&seek)));
        }
        if bool_param(params, "download").unwrap_or(false) {
            query.push("download=1".to_string());
        }

        let mut url = format!("/api/stream/{}", urlencoding::encode(&chapter_id));
        if !query.is_empty() {
            url.push('?');
            url.push_str(&query.join("&"));
        }

        Ok(serde_json::json!({
            "chapter_id": chapter.id,
            "book_id": chapter.book_id,
            "url": url,
            "requires_auth": true,
            "auth": "current_user",
        }))
    }

    pub(super) async fn media_get_signed_url(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let chapter_id = required_string_param(params, "chapter_id")
            .or_else(|_| required_string_param(params, "id"))?;
        let chapter = self
            .chapter_repo
            .find_by_id(&chapter_id)
            .await?
            .ok_or_else(|| {
                TingError::NotFound(format!("Chapter with id {} not found", chapter_id))
            })?;

        self.ensure_user_can_access_book(user, &chapter.book_id)
            .await?;

        let expires = signature_expires_from_ttl(
            u64_param(params, "expires_in_seconds"),
            DEFAULT_MEDIA_SIGNATURE_TTL_SECONDS,
            MAX_MEDIA_SIGNATURE_TTL_SECONDS,
        );
        let transcode = string_param(params, "transcode");
        if let Some(transcode) = transcode.as_deref() {
            if !matches!(transcode, "hls" | "mp3" | "wav") {
                return Err(TingError::InvalidRequest(format!(
                    "Unsupported media transcode target: {}",
                    transcode
                )));
            }
        }
        let seek = string_param(params, "seek");
        let download = bool_param(params, "download").unwrap_or(false);
        let signature = sign_media_stream_request(
            self.encryption_key.as_ref(),
            &chapter.id,
            expires,
            &user.id,
            transcode.as_deref(),
            seek.as_deref(),
            download,
        );

        let mut query = vec![
            format!("expires={}", expires),
            format!("user={}", urlencoding::encode(&user.id)),
        ];
        if let Some(transcode) = transcode.as_deref() {
            query.push(format!("transcode={}", urlencoding::encode(transcode)));
        }
        if let Some(seek) = seek.as_deref() {
            query.push(format!("seek={}", urlencoding::encode(seek)));
        }
        if download {
            query.push("download=1".to_string());
        }
        query.push(format!("signature={}", signature));

        let url = format!(
            "/api/v1/public/media/{}?{}",
            urlencoding::encode(&chapter.id),
            query.join("&")
        );

        Ok(serde_json::json!({
            "chapter_id": chapter.id,
            "book_id": chapter.book_id,
            "url": url,
            "expires": expires,
            "signature": signature,
            "user_id": user.id,
            "requires_auth": false,
            "auth": "signed",
            "content_type": signed_media_content_type(&chapter.path, transcode.as_deref()),
            "duration": chapter.duration,
        }))
    }

    pub(super) async fn plugin_routes_sign(
        &self,
        plugin_id: &str,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let method = string_param(params, "method")
            .unwrap_or_else(|| "GET".to_string())
            .to_uppercase();
        let path = normalize_plugin_route_sign_path(&required_string_param(params, "path")?);
        let matched = self
            .plugin_manager
            .find_http_route(&method, &path)
            .await
            .ok_or_else(|| {
                TingError::NotFound(format!("Plugin route not found: {} {}", method, path))
            })?;

        if matched.registration.plugin_id != plugin_id {
            return Err(TingError::PermissionDenied(format!(
                "Plugin {} cannot sign route owned by {}",
                plugin_id, matched.registration.plugin_id
            )));
        }

        if !plugin_route_can_use_public_prefix(&matched.registration.capability.extra) {
            return Err(TingError::PermissionDenied(format!(
                "Plugin route cannot be exposed through public plugin-routes: {} {}",
                method, path
            )));
        }

        let expires = signature_expires_from_ttl(
            u64_param(params, "expires_in_seconds"),
            DEFAULT_PLUGIN_ROUTE_SIGNATURE_TTL_SECONDS,
            MAX_PLUGIN_ROUTE_SIGNATURE_TTL_SECONDS,
        );
        let bind_current_user = params
            .get("bind_current_user")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let signed_user_id = bind_current_user.then(|| user.id.clone());
        let signature = sign_plugin_route_request(
            self.encryption_key.as_ref(),
            &method,
            &path,
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

        Ok(serde_json::json!({
            "path": path,
            "method": method,
            "expires": expires,
            "signature": signature,
            "user_id": signed_user_id,
            "signed_url": signed_url,
        }))
    }

    pub(super) async fn metadata_write(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        if !user.is_admin() {
            return Err(TingError::PermissionDenied(
                "Admin access required for metadata.write".to_string(),
            ));
        }

        let book_id = required_string_param(params, "book_id")
            .or_else(|_| required_string_param(params, "id"))?;
        let book =
            self.book_repo.find_by_id(&book_id).await?.ok_or_else(|| {
                TingError::NotFound(format!("Book with id {} not found", book_id))
            })?;

        let task = Task::new(
            format!("写入元数据: {}", book.title.clone().unwrap_or_default()),
            Priority::Normal,
            TaskPayload::Custom {
                task_type: "write_metadata".to_string(),
                data: serde_json::json!({
                    "book_id": book_id,
                }),
            },
        );
        let task_id = self.task_queue.submit(task).await?;

        Ok(serde_json::json!({
            "task_id": task_id,
            "task_type": "write_metadata",
            "status": "queued",
            "book_id": book.id,
        }))
    }

    pub(super) async fn database_get(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let entity = required_string_param(params, "entity")?;
        let id = required_string_param(params, "id")?;

        match entity.as_str() {
            "book" | "books" => {
                self.ensure_user_can_access_book(user, &id).await?;
                let book =
                    self.book_repo.find_by_id(&id).await?.ok_or_else(|| {
                        TingError::NotFound(format!("Book with id {} not found", id))
                    })?;
                serde_json::to_value(book).map_err(|e| {
                    TingError::SerializationError(format!("Book serialization failed: {}", e))
                })
            }
            "chapter" | "chapters" => {
                let chapter = self.chapter_repo.find_by_id(&id).await?.ok_or_else(|| {
                    TingError::NotFound(format!("Chapter with id {} not found", id))
                })?;
                self.ensure_user_can_access_book(user, &chapter.book_id)
                    .await?;
                serde_json::to_value(chapter).map_err(|e| {
                    TingError::SerializationError(format!("Chapter serialization failed: {}", e))
                })
            }
            "library" | "libraries" => {
                self.ensure_user_can_access_library(user, &id).await?;
                let library = self.library_repo.find_by_id(&id).await?.ok_or_else(|| {
                    TingError::NotFound(format!("Library with id {} not found", id))
                })?;
                Ok(plugin_host_library_value(library))
            }
            _ => Err(TingError::InvalidRequest(format!(
                "Unsupported database entity: {}",
                entity
            ))),
        }
    }

    pub(super) async fn database_list(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let entity = required_string_param(params, "entity")?;
        match entity.as_str() {
            "book" | "books" => self.books_list(user, params).await,
            "chapter" | "chapters" => self.chapters_list(user, params).await,
            "library" | "libraries" => self.libraries_list(user, params).await,
            "progress" => self.progress_recent(user, params).await,
            _ => Err(TingError::InvalidRequest(format!(
                "Unsupported database entity: {}",
                entity
            ))),
        }
    }

    pub(super) async fn database_update(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        if !user.is_admin() {
            return Err(TingError::PermissionDenied(
                "Admin access required for database.update".to_string(),
            ));
        }

        let entity = required_string_param(params, "entity")?;
        let id = required_string_param(params, "id")?;
        let patch = required_object_param(params, "patch")?;

        match entity.as_str() {
            "book" | "books" => {
                let mut book =
                    self.book_repo.find_by_id(&id).await?.ok_or_else(|| {
                        TingError::NotFound(format!("Book with id {} not found", id))
                    })?;
                patch_optional_string(patch, "title", &mut book.title)?;
                patch_optional_string(patch, "author", &mut book.author)?;
                patch_optional_string(patch, "narrator", &mut book.narrator)?;
                patch_optional_string(patch, "cover_url", &mut book.cover_url)?;
                patch_optional_string(patch, "theme_color", &mut book.theme_color)?;
                patch_optional_string(patch, "description", &mut book.description)?;
                patch_optional_string(patch, "tags", &mut book.tags)?;
                patch_optional_string(patch, "genre", &mut book.genre)?;
                patch_optional_i32(patch, "year", &mut book.year)?;
                patch_i32(patch, "skip_intro", &mut book.skip_intro)?;
                patch_i32(patch, "skip_outro", &mut book.skip_outro)?;
                patch_i32(patch, "manual_corrected", &mut book.manual_corrected)?;
                patch_optional_string(patch, "match_pattern", &mut book.match_pattern)?;
                patch_optional_string(patch, "chapter_regex", &mut book.chapter_regex)?;
                self.book_repo.update(&book).await?;
                serde_json::to_value(book).map_err(|e| {
                    TingError::SerializationError(format!("Book serialization failed: {}", e))
                })
            }
            "chapter" | "chapters" => {
                let mut chapter = self.chapter_repo.find_by_id(&id).await?.ok_or_else(|| {
                    TingError::NotFound(format!("Chapter with id {} not found", id))
                })?;
                patch_optional_string(patch, "title", &mut chapter.title)?;
                patch_required_string(patch, "path", &mut chapter.path)?;
                patch_optional_i32(patch, "duration", &mut chapter.duration)?;
                patch_optional_i32(patch, "chapter_index", &mut chapter.chapter_index)?;
                patch_i32(patch, "is_extra", &mut chapter.is_extra)?;
                patch_optional_string(patch, "hash", &mut chapter.hash)?;
                patch_i32(patch, "manual_corrected", &mut chapter.manual_corrected)?;
                self.chapter_repo.update(&chapter).await?;
                serde_json::to_value(chapter).map_err(|e| {
                    TingError::SerializationError(format!("Chapter serialization failed: {}", e))
                })
            }
            "library" | "libraries" => {
                let mut library = self.library_repo.find_by_id(&id).await?.ok_or_else(|| {
                    TingError::NotFound(format!("Library with id {} not found", id))
                })?;
                patch_required_string(patch, "name", &mut library.name)?;
                patch_optional_string(patch, "scraper_config", &mut library.scraper_config)?;
                self.library_repo.update(&library).await?;
                Ok(plugin_host_library_value(library))
            }
            _ => Err(TingError::InvalidRequest(format!(
                "Unsupported database entity: {}",
                entity
            ))),
        }
    }
}

fn u64_param(params: &Value, name: &str) -> Option<u64> {
    params.get(name).and_then(Value::as_u64)
}

fn signed_media_content_type(path: &str, transcode: Option<&str>) -> String {
    match transcode {
        Some("mp3") => "audio/mpeg".to_string(),
        Some("wav") => "audio/wav".to_string(),
        Some("hls") => "application/vnd.apple.mpegurl".to_string(),
        _ => {
            let ext = std::path::Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            match ext.as_str() {
                "m4a" | "mp4" => "audio/mp4".to_string(),
                "mp3" => "audio/mpeg".to_string(),
                "aac" => "audio/aac".to_string(),
                "flac" => "audio/flac".to_string(),
                "ogg" => "audio/ogg".to_string(),
                "opus" => "audio/opus".to_string(),
                "wav" => "audio/wav".to_string(),
                _ => mime_guess::from_path(path)
                    .first_or_octet_stream()
                    .to_string(),
            }
        }
    }
}

fn plugin_route_can_use_public_prefix(extra: &BTreeMap<String, Value>) -> bool {
    let auth = extra
        .get("route")
        .and_then(|route| route.get("auth"))
        .and_then(Value::as_str)
        .unwrap_or("user");
    matches!(auth, "public" | "signed" | "public_or_signed")
}

fn plugin_host_library_value(library: Library) -> Value {
    serde_json::json!({
        "id": library.id,
        "name": library.name,
        "type": library.library_type,
        "url": library.url,
        "root_path": library.root_path,
        "last_scanned_at": library.last_scanned_at,
        "created_at": library.created_at,
        "scraper_config": library.scraper_config,
    })
}

fn required_object_param<'a>(params: &'a Value, name: &str) -> Result<&'a Map<String, Value>> {
    params.get(name).and_then(Value::as_object).ok_or_else(|| {
        TingError::InvalidRequest(format!(
            "Missing required object plugin host parameter: {}",
            name
        ))
    })
}

fn patch_required_string(patch: &Map<String, Value>, key: &str, target: &mut String) -> Result<()> {
    let Some(value) = patch.get(key) else {
        return Ok(());
    };
    let Some(text) = value.as_str() else {
        return Err(TingError::InvalidRequest(format!(
            "Patch field {} must be a string",
            key
        )));
    };
    *target = text.to_string();
    Ok(())
}

fn patch_optional_string(
    patch: &Map<String, Value>,
    key: &str,
    target: &mut Option<String>,
) -> Result<()> {
    let Some(value) = patch.get(key) else {
        return Ok(());
    };
    if value.is_null() {
        *target = None;
        return Ok(());
    }
    let Some(text) = value.as_str() else {
        return Err(TingError::InvalidRequest(format!(
            "Patch field {} must be a string or null",
            key
        )));
    };
    *target = Some(text.to_string());
    Ok(())
}

fn patch_i32(patch: &Map<String, Value>, key: &str, target: &mut i32) -> Result<()> {
    let Some(value) = patch.get(key) else {
        return Ok(());
    };
    *target = value_to_i32(value, key)?;
    Ok(())
}

fn patch_optional_i32(
    patch: &Map<String, Value>,
    key: &str,
    target: &mut Option<i32>,
) -> Result<()> {
    let Some(value) = patch.get(key) else {
        return Ok(());
    };
    if value.is_null() {
        *target = None;
        return Ok(());
    }
    *target = Some(value_to_i32(value, key)?);
    Ok(())
}

fn value_to_i32(value: &Value, key: &str) -> Result<i32> {
    let Some(number) = value.as_i64() else {
        return Err(TingError::InvalidRequest(format!(
            "Patch field {} must be an integer",
            key
        )));
    };
    i32::try_from(number)
        .map_err(|_| TingError::InvalidRequest(format!("Patch field {} is out of i32 range", key)))
}
