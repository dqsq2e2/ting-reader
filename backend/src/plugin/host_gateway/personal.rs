use super::{required_string_param, string_param, usize_param, PluginHostGateway, PluginHostUser};
use crate::core::error::{Result, TingError};
use crate::db::models::{Favorite, Playlist, PlaylistItem, UserSettings};
use crate::db::repository::Repository;
use serde_json::{Map, Value};
use uuid::Uuid;

impl PluginHostGateway {
    pub(super) async fn playlists_list(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let limit = usize_param(params, "limit").unwrap_or(50).clamp(1, 200);
        let offset = usize_param(params, "offset").unwrap_or(0);
        let playlists = self.playlist_repo.find_by_user(&user.id).await?;
        let total = playlists.len();
        let items = playlists
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(plugin_host_playlist_value)
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "items": items,
            "total": total,
            "offset": offset,
            "limit": limit,
        }))
    }

    pub(super) async fn playlists_get(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let playlist_id = required_string_param(params, "playlist_id")
            .or_else(|_| required_string_param(params, "id"))?;
        let playlist = self.load_owned_playlist(user, &playlist_id).await?;
        let plugin_items = self
            .playlist_repo
            .find_items_by_playlist(&playlist.id)
            .await?
            .into_iter()
            .map(plugin_host_playlist_item_value)
            .collect::<Vec<_>>();

        let mut response = plugin_host_playlist_value(playlist);
        if let Value::Object(ref mut object) = response {
            object.insert("items".to_string(), Value::Array(plugin_items));
        }
        Ok(response)
    }

    pub(super) async fn playlists_create(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let name = required_playlist_name(params)?;

        let now = chrono::Utc::now().to_rfc3339();
        let playlist = Playlist {
            id: Uuid::new_v4().to_string(),
            user_id: user.id.clone(),
            title: name,
            description: optional_trimmed_string(params, "description"),
            created_at: now.clone(),
            updated_at: now,
        };

        self.playlist_repo.create(&playlist).await?;

        let raw_items = params
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if !raw_items.is_empty() {
            let items = parse_playlist_items(&raw_items)?;
            self.playlist_repo
                .replace_items(&playlist.id, &user.id, user.is_admin(), items)
                .await?;
        }

        let stored_items = self
            .playlist_repo
            .find_items_by_playlist(&playlist.id)
            .await?
            .into_iter()
            .map(plugin_host_playlist_item_value)
            .collect::<Vec<_>>();

        let mut response = plugin_host_playlist_value(playlist);
        if let Value::Object(ref mut object) = response {
            object.insert("items".to_string(), Value::Array(stored_items));
        }
        Ok(response)
    }

    pub(super) async fn playlists_update(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let playlist_id = required_string_param(params, "playlist_id")
            .or_else(|_| required_string_param(params, "id"))?;
        let existing = self.load_owned_playlist(user, &playlist_id).await?;

        let title = match string_param(params, "name") {
            Some(name) => name,
            None => existing.title.clone(),
        };
        if title.is_empty() {
            return Err(TingError::InvalidRequest(
                "Playlist name cannot be empty".to_string(),
            ));
        }

        let description = match params.get("description") {
            Some(Value::Null) => None,
            Some(value) => value
                .as_str()
                .map(|text| text.trim().to_string())
                .or_else(|| existing.description.clone()),
            None => existing.description.clone(),
        };

        let updated = Playlist {
            id: existing.id.clone(),
            user_id: existing.user_id.clone(),
            title,
            description,
            created_at: existing.created_at.clone(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        self.playlist_repo.update(&updated).await?;
        Ok(plugin_host_playlist_value(updated))
    }

    pub(super) async fn playlists_delete(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let playlist_id = required_string_param(params, "playlist_id")
            .or_else(|_| required_string_param(params, "id"))?;
        let existing = self.load_owned_playlist(user, &playlist_id).await?;
        self.playlist_repo.delete(&existing.id).await?;

        Ok(serde_json::json!({
            "ok": true,
            "id": existing.id,
        }))
    }

    pub(super) async fn playlists_add_item(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let playlist_id = required_string_param(params, "playlist_id")?;
        let existing = self.load_owned_playlist(user, &playlist_id).await?;

        let (item_type, item_id) = required_playlist_item_pair(params)?;
        let item_order = params
            .get("item_order")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or(0);
        let item = PlaylistItem {
            playlist_id: existing.id.clone(),
            item_type,
            item_id,
            item_order,
        };

        self.playlist_repo
            .add_item(&existing.id, &user.id, user.is_admin(), item.clone())
            .await?;

        Ok(serde_json::json!({
            "ok": true,
            "playlist_id": existing.id,
            "item_type": item.item_type,
            "item_id": item.item_id,
        }))
    }

    pub(super) async fn playlists_remove_item(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let playlist_id = required_string_param(params, "playlist_id")?;
        let existing = self.load_owned_playlist(user, &playlist_id).await?;

        let (item_type, item_id) = required_playlist_item_pair(params)?;
        self.playlist_repo
            .remove_item(&existing.id, &item_type, &item_id)
            .await?;

        Ok(serde_json::json!({
            "ok": true,
            "playlist_id": existing.id,
            "item_type": item_type,
            "item_id": item_id,
        }))
    }

    pub(super) async fn favorites_list(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let favorites = self.favorite_repo.get_by_user(&user.id).await?;
        let total = favorites.len();
        let limit = usize_param(params, "limit")
            .unwrap_or(total.max(1))
            .clamp(1, 500);
        let offset = usize_param(params, "offset").unwrap_or(0);

        let items = favorites
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(plugin_host_favorite_value)
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "items": items,
            "total": total,
            "offset": offset,
            "limit": limit,
        }))
    }

    pub(super) async fn favorites_add(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let book_id = required_string_param(params, "book_id")
            .or_else(|_| required_string_param(params, "id"))?;
        self.ensure_user_can_access_book(user, &book_id).await?;

        let already = self.favorite_repo.is_favorited(&user.id, &book_id).await?;
        if !already {
            let favorite = Favorite {
                id: Uuid::new_v4().to_string(),
                user_id: user.id.clone(),
                book_id: book_id.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            self.favorite_repo.add(&favorite).await?;
        }

        Ok(serde_json::json!({
            "ok": true,
            "book_id": book_id,
            "created": !already,
        }))
    }

    pub(super) async fn favorites_remove(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let book_id = required_string_param(params, "book_id")
            .or_else(|_| required_string_param(params, "id"))?;
        self.favorite_repo.remove(&user.id, &book_id).await?;

        Ok(serde_json::json!({
            "ok": true,
            "book_id": book_id,
        }))
    }

    pub(super) async fn user_settings_get(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let settings = self.settings_repo.get_by_user(&user.id).await?;
        let map = user_settings_map(settings.as_ref());

        match string_param(params, "key") {
            Some(key) => Ok(serde_json::json!({
                "key": key.clone(),
                "value": map.get(&key).cloned().unwrap_or(Value::Null),
            })),
            None => Ok(serde_json::json!({
                "items": map,
            })),
        }
    }

    pub(super) async fn user_settings_set(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let key = required_string_param(params, "key")?;
        if is_reserved_settings_key(&key) {
            return Err(TingError::InvalidRequest(format!(
                "user_settings key '{}' is reserved",
                key
            )));
        }

        let value = params.get("value").cloned().unwrap_or(Value::Null);
        let existing = self.settings_repo.get_by_user(&user.id).await?;

        let mut map = existing_settings_map(existing.as_ref());
        map.insert(key.clone(), value);

        let json_text = serde_json::to_string(&Value::Object(map)).map_err(|e| {
            TingError::SerializationError(format!("Failed to serialize user_settings: {}", e))
        })?;

        let settings = match existing {
            Some(prev) => UserSettings {
                user_id: prev.user_id,
                playback_speed: prev.playback_speed,
                theme: prev.theme,
                auto_play: prev.auto_play,
                skip_intro: prev.skip_intro,
                skip_outro: prev.skip_outro,
                settings_json: Some(json_text),
                updated_at: chrono::Utc::now().to_rfc3339(),
            },
            None => UserSettings {
                user_id: user.id.clone(),
                playback_speed: 1.0,
                theme: "auto".to_string(),
                auto_play: 1,
                skip_intro: 0,
                skip_outro: 0,
                settings_json: Some(json_text),
                updated_at: chrono::Utc::now().to_rfc3339(),
            },
        };

        self.settings_repo.upsert(&settings).await?;

        Ok(serde_json::json!({
            "ok": true,
            "key": key,
        }))
    }

    async fn load_owned_playlist(
        &self,
        user: &PluginHostUser,
        playlist_id: &str,
    ) -> Result<Playlist> {
        let playlist = self
            .playlist_repo
            .find_by_user_and_id(playlist_id, &user.id)
            .await?
            .ok_or_else(|| {
                TingError::NotFound(format!("Playlist with id {} not found", playlist_id))
            })?;
        if playlist.user_id != user.id {
            return Err(TingError::PermissionDenied(
                "Playlist does not belong to current user".to_string(),
            ));
        }
        Ok(playlist)
    }
}

fn plugin_host_playlist_value(playlist: Playlist) -> Value {
    serde_json::json!({
        "id": playlist.id,
        "user_id": playlist.user_id,
        "name": playlist.title,
        "description": playlist.description,
        "created_at": playlist.created_at,
        "updated_at": playlist.updated_at,
    })
}

fn plugin_host_playlist_item_value(item: PlaylistItem) -> Value {
    serde_json::json!({
        "playlist_id": item.playlist_id,
        "item_type": item.item_type,
        "item_id": item.item_id,
        "item_order": item.item_order,
    })
}

fn plugin_host_favorite_value(favorite: Favorite) -> Value {
    serde_json::json!({
        "id": favorite.id,
        "user_id": favorite.user_id,
        "book_id": favorite.book_id,
        "created_at": favorite.created_at,
    })
}

fn required_playlist_name(params: &Value) -> Result<String> {
    let raw = params
        .get("name")
        .or_else(|| params.get("title"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    raw.ok_or_else(|| {
        TingError::InvalidRequest("playlists.create requires a non-empty name".to_string())
    })
}

fn optional_trimmed_string(params: &Value, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_playlist_items(items: &[Value]) -> Result<Vec<PlaylistItem>> {
    let mut normalized = Vec::with_capacity(items.len());
    for (idx, item) in items.iter().enumerate() {
        let (item_type, item_id) = required_playlist_item_pair(item)?;
        let item_order = item
            .get("item_order")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or_else(|| (idx as i32) + 1);
        normalized.push(PlaylistItem {
            playlist_id: String::new(),
            item_type,
            item_id,
            item_order,
        });
    }
    Ok(normalized)
}

fn required_playlist_item_pair(params: &Value) -> Result<(String, String)> {
    let item_type = params
        .get("item_type")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| {
            TingError::InvalidRequest("playlist item requires 'item_type'".to_string())
        })?;
    if !matches!(item_type.as_str(), "book" | "series") {
        return Err(TingError::InvalidRequest(
            "playlist item_type must be 'book' or 'series'".to_string(),
        ));
    }

    let item_id = params
        .get("item_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| TingError::InvalidRequest("playlist item requires 'item_id'".to_string()))?;

    Ok((item_type, item_id))
}

fn user_settings_map(settings: Option<&UserSettings>) -> Map<String, Value> {
    let mut map = existing_settings_map(settings);
    if let Some(settings) = settings {
        map.entry("playback_speed".to_string())
            .or_insert_with(|| serde_json::json!(settings.playback_speed));
        map.entry("theme".to_string())
            .or_insert_with(|| Value::String(settings.theme.clone()));
        map.entry("auto_play".to_string())
            .or_insert_with(|| Value::Bool(settings.auto_play != 0));
        map.entry("skip_intro".to_string())
            .or_insert_with(|| serde_json::json!(settings.skip_intro));
        map.entry("skip_outro".to_string())
            .or_insert_with(|| serde_json::json!(settings.skip_outro));
    }
    map
}

fn existing_settings_map(settings: Option<&UserSettings>) -> Map<String, Value> {
    settings
        .and_then(|settings| settings.settings_json.as_deref())
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

fn is_reserved_settings_key(key: &str) -> bool {
    matches!(key, "user_id" | "updated_at" | "settings_json")
}
