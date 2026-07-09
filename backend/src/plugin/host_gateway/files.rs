use super::{
    bool_param, required_string_param, string_param, usize_param, PluginHostGateway, PluginHostUser,
};
use crate::core::error::{Result, TingError};
use crate::core::local_paths::resolve_existing_local_library_root;
use crate::core::Config;
use crate::db::models::Library;
use crate::db::repository::Repository;
use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

const MAX_HOST_FILE_READ_BYTES: u64 = 20 * 1024 * 1024;
const MAX_HOST_FILE_WRITE_BYTES: usize = 50 * 1024 * 1024;
const MAX_LIBRARY_FILE_LIST_ENTRIES: usize = 500;

impl PluginHostGateway {
    pub(super) async fn library_file_list(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let (library, root, target, relative_path) = self
            .resolve_library_file_target(user, params, true, false)
            .await?;
        let metadata = tokio::fs::metadata(&target).await?;
        if !metadata.is_dir() {
            return Err(TingError::InvalidRequest(format!(
                "Library file target is not a directory: {}",
                relative_path
            )));
        }

        let limit = usize_param(params, "limit")
            .unwrap_or(200)
            .clamp(1, MAX_LIBRARY_FILE_LIST_ENTRIES);
        let mut entries = Vec::new();
        let mut dir = tokio::fs::read_dir(&target).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            entries.push(library_file_entry_value(&root, &path).await?);
            if entries.len() >= limit {
                break;
            }
        }

        Ok(serde_json::json!({
            "library_id": library.id,
            "path": relative_path,
            "entries": entries,
            "limit": limit,
        }))
    }

    pub(super) async fn library_file_stat(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let (library, root, target, relative_path) = self
            .resolve_library_file_target(user, params, true, false)
            .await?;
        let entry = library_file_entry_value(&root, &target).await?;

        Ok(serde_json::json!({
            "library_id": library.id,
            "path": relative_path,
            "entry": entry,
        }))
    }

    pub(super) async fn library_file_read(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        let (library, root, target, relative_path) =
            if string_param(params, "relative_to").as_deref() == Some("book") {
                self.resolve_book_file_target(user, params).await?
            } else {
                self.resolve_library_file_target(user, params, false, false)
                    .await?
            };
        let metadata = tokio::fs::metadata(&target).await?;
        if !metadata.is_file() {
            return Err(TingError::InvalidRequest(format!(
                "Library file target is not a file: {}",
                relative_path
            )));
        }
        if metadata.len() > MAX_HOST_FILE_READ_BYTES {
            return Err(TingError::ResourceLimitExceeded(format!(
                "Library file read is limited to {} bytes",
                MAX_HOST_FILE_READ_BYTES
            )));
        }

        let bytes = tokio::fs::read(&target).await?;
        let mut response = serde_json::json!({
            "library_id": library.id,
            "path": relative_path,
            "size": bytes.len(),
            "data_base64": general_purpose::STANDARD.encode(&bytes),
        });
        if bool_param(params, "as_text").unwrap_or(false) {
            let text = String::from_utf8(bytes).map_err(|e| {
                TingError::InvalidRequest(format!("Library file is not valid UTF-8: {}", e))
            })?;
            response["text"] = Value::String(text);
        }
        response["entry"] = library_file_entry_value(&root, &target).await?;

        Ok(response)
    }

    pub(super) async fn library_file_write(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<Value> {
        if !user.is_admin() {
            return Err(TingError::PermissionDenied(
                "Admin access required for library.file.write".to_string(),
            ));
        }

        let (library, root, target, relative_path) =
            if string_param(params, "relative_to").as_deref() == Some("book") {
                self.resolve_book_file_target(user, params).await?
            } else {
                self.resolve_library_file_target(user, params, false, true)
                    .await?
            };
        let bytes = library_file_write_bytes(params)?;
        if bytes.len() > MAX_HOST_FILE_WRITE_BYTES {
            return Err(TingError::ResourceLimitExceeded(format!(
                "Library file write is limited to {} bytes",
                MAX_HOST_FILE_WRITE_BYTES
            )));
        }

        let overwrite = bool_param(params, "overwrite").unwrap_or(false);
        if !overwrite && tokio::fs::try_exists(&target).await? {
            return Err(TingError::InvalidRequest(format!(
                "Library file already exists: {}",
                relative_path
            )));
        }

        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
            ensure_canonical_child(&root, parent)?;
        }
        if tokio::fs::try_exists(&target).await? {
            let canonical = std::fs::canonicalize(&target)?;
            ensure_path_inside(&root, &canonical)?;
        }

        tokio::fs::write(&target, &bytes).await?;

        Ok(serde_json::json!({
            "library_id": library.id,
            "path": relative_path,
            "size": bytes.len(),
            "entry": library_file_entry_value(&root, &target).await?,
        }))
    }

    async fn resolve_library_file_target(
        &self,
        user: &PluginHostUser,
        params: &Value,
        allow_root_path: bool,
        for_write: bool,
    ) -> Result<(Library, PathBuf, PathBuf, String)> {
        let library_id = required_string_param(params, "library_id")?;
        self.ensure_user_can_access_library(user, &library_id)
            .await?;
        let library = self
            .library_repo
            .find_by_id(&library_id)
            .await?
            .ok_or_else(|| {
                TingError::NotFound(format!("Library with id {} not found", library_id))
            })?;
        let root = self.local_library_root(&library)?;
        let requested_path = string_param(params, "path")
            .or_else(|| string_param(params, "relative_path"))
            .unwrap_or_default();
        let relative = normalize_library_relative_path(&requested_path, allow_root_path)?;
        let target = root.join(&relative);

        if for_write {
            if let Some(parent) = target.parent() {
                if parent.exists() {
                    ensure_canonical_child(&root, parent)?;
                }
            }
        } else {
            let canonical = std::fs::canonicalize(&target)?;
            ensure_path_inside(&root, &canonical)?;
        }

        Ok((
            library,
            root,
            target,
            relative.to_string_lossy().replace('\\', "/"),
        ))
    }

    async fn resolve_book_file_target(
        &self,
        user: &PluginHostUser,
        params: &Value,
    ) -> Result<(Library, PathBuf, PathBuf, String)> {
        let library_id = required_string_param(params, "library_id")?;
        let book_id = required_string_param(params, "book_id")?;
        self.ensure_user_can_access_library(user, &library_id)
            .await?;
        self.ensure_user_can_access_book(user, &book_id).await?;

        let library = self
            .library_repo
            .find_by_id(&library_id)
            .await?
            .ok_or_else(|| {
                TingError::NotFound(format!("Library with id {} not found", library_id))
            })?;
        let book =
            self.book_repo.find_by_id(&book_id).await?.ok_or_else(|| {
                TingError::NotFound(format!("Book with id {} not found", book_id))
            })?;
        if book.library_id != library.id {
            return Err(TingError::InvalidRequest(format!(
                "Book {} does not belong to library {}",
                book_id, library_id
            )));
        }

        let requested_path = string_param(params, "path")
            .or_else(|| string_param(params, "relative_path"))
            .unwrap_or_default();
        let relative = normalize_library_relative_path(&requested_path, false)?;
        let (root, book_dir) = self.book_file_base_dir(&library, &book.path)?;
        let target = book_dir.join(&relative);
        let relative_path = target.to_string_lossy().replace('\\', "/");

        if let Some(parent) = target.parent() {
            if parent.exists() {
                ensure_canonical_child(&root, parent)?;
            }
        }

        Ok((library, root, target, relative_path))
    }
}

fn book_file_base_dir_with_root(
    config: &Config,
    library: &Library,
    book_path: &str,
) -> Result<(PathBuf, PathBuf)> {
    if library.library_type == "local" {
        let root = local_library_root_with_config(config, library)?;
        let book_dir = PathBuf::from(book_path);
        let canonical_book_dir = std::fs::canonicalize(&book_dir)?;
        ensure_path_inside(&root, &canonical_book_dir)?;
        return Ok((root, canonical_book_dir));
    }

    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut hasher = Sha256::new();
    hasher.update(book_path.as_bytes());
    let book_hash = format!("{:x}", hasher.finalize());
    Ok((root.clone(), root.join("temp").join(book_hash)))
}

fn local_library_root_with_config(config: &Config, library: &Library) -> Result<PathBuf> {
    if library.library_type == "local" {
        let root = resolve_existing_local_library_root(library, config)?;
        if !root.is_dir() {
            return Err(TingError::InvalidRequest(format!(
                "Library {} root path is not a directory",
                library.id
            )));
        }
        return Ok(root);
    }

    let root = library.root_path.trim();
    if root.is_empty() {
        return Err(TingError::InvalidRequest(format!(
            "Library {} does not expose a local root path",
            library.id
        )));
    }

    let root = std::fs::canonicalize(PathBuf::from(root))?;
    if !root.is_dir() {
        return Err(TingError::InvalidRequest(format!(
            "Library {} root path is not a directory",
            library.id
        )));
    }
    Ok(root)
}

impl PluginHostGateway {
    fn local_library_root(&self, library: &Library) -> Result<PathBuf> {
        local_library_root_with_config(&self.config, library)
    }

    fn book_file_base_dir(&self, library: &Library, book_path: &str) -> Result<(PathBuf, PathBuf)> {
        book_file_base_dir_with_root(&self.config, library, book_path)
    }
}

fn normalize_library_relative_path(value: &str, allow_empty: bool) -> Result<PathBuf> {
    let raw = Path::new(value.trim());
    if raw.is_absolute() {
        return Err(TingError::SecurityViolation(
            "Library file path must be relative".to_string(),
        ));
    }

    let mut normalized = PathBuf::new();
    for component in raw.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(TingError::SecurityViolation(
                    "Library file path cannot escape the library root".to_string(),
                ));
            }
        }
    }

    if !allow_empty && normalized.as_os_str().is_empty() {
        return Err(TingError::InvalidRequest(
            "Library file path is required".to_string(),
        ));
    }
    Ok(normalized)
}

fn ensure_canonical_child(root: &Path, path: &Path) -> Result<()> {
    let canonical = std::fs::canonicalize(path)?;
    ensure_path_inside(root, &canonical)
}

fn ensure_path_inside(root: &Path, path: &Path) -> Result<()> {
    if path == root || path.starts_with(root) {
        return Ok(());
    }
    Err(TingError::SecurityViolation(
        "Library file path escapes the library root".to_string(),
    ))
}

async fn library_file_entry_value(root: &Path, path: &Path) -> Result<Value> {
    let metadata = tokio::fs::metadata(path).await?;
    let canonical = std::fs::canonicalize(path)?;
    ensure_path_inside(root, &canonical)?;
    let relative_path = canonical
        .strip_prefix(root)
        .unwrap_or(&canonical)
        .to_string_lossy()
        .replace('\\', "/");
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs());

    Ok(serde_json::json!({
        "name": file_name,
        "path": relative_path,
        "is_file": metadata.is_file(),
        "is_dir": metadata.is_dir(),
        "size": metadata.len(),
        "modified_unix": modified_at,
    }))
}

fn library_file_write_bytes(params: &Value) -> Result<Vec<u8>> {
    if let Some(data) = params.get("data_base64").and_then(Value::as_str) {
        return general_purpose::STANDARD.decode(data).map_err(|e| {
            TingError::InvalidRequest(format!("Invalid data_base64 for library.file.write: {}", e))
        });
    }
    if let Some(text) = params.get("text").and_then(Value::as_str) {
        return Ok(text.as_bytes().to_vec());
    }
    Err(TingError::InvalidRequest(
        "library.file.write requires data_base64 or text".to_string(),
    ))
}
