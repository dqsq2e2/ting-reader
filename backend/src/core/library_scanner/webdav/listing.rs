use super::super::{LibraryScanner, ScanMode};
use crate::core::app::error::NetworkErrorKind;
use crate::core::app::error::{Result, TingError};
use crate::db::repository::LibraryScanState;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::{HashMap, HashSet};
use tracing::{debug, warn};

#[derive(Debug)]
struct WebDavSyncItem {
    url: String,
    is_dir: bool,
    deleted: bool,
    last_modified: Option<String>,
    etag: Option<String>,
}

#[derive(Debug, Default)]
struct WebDavSyncReport {
    next_token: Option<String>,
    items: Vec<WebDavSyncItem>,
    truncated: bool,
    requires_fallback: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebDavSyncResponseDisposition {
    Apply,
    Truncated,
    Fallback,
}

/// Everything one RFC 6578 `sync-collection` run needs: the endpoint and
/// credentials plus the cached snapshot the delta is applied to.
struct WebDavSyncRequest<'a> {
    library: &'a crate::db::models::Library,
    root_url: &'a str,
    previous_token: &'a str,
    client: &'a reqwest::Client,
    username: Option<&'a str>,
    password: Option<&'a str>,
    cached_dirs: &'a HashMap<String, LibraryScanState>,
    cached_files: &'a HashMap<String, LibraryScanState>,
}

impl LibraryScanner {
    /// List all files in a WebDAV library recursively
    pub(super) async fn list_webdav_files(
        &self,
        library: &crate::db::models::Library,
        task_id: Option<&str>,
        mode: ScanMode,
    ) -> Result<
        Vec<(
            String,
            Option<chrono::DateTime<chrono::Utc>>,
            Option<String>,
        )>,
    > {
        // Simple BFS or recursive traversal
        // Start from root
        let base = library.url.trim_end_matches('/');
        let path = library.root_path.trim_matches('/');
        let root_url = if path.is_empty() {
            base.to_string()
        } else {
            format!("{base}/{path}")
        };

        let mut files = HashMap::new(); // URL -> (LastModified, ETag/LastModified validator)
        let mut cache_states = Vec::new();
        // Full scans still traverse every collection, but loading the previous
        // snapshot lets the persistence phase write only the state delta.
        let cached_dirs = self
            .scan_state_repo
            .find_by_library_kind(&library.id, "webdav_dir")
            .await
            .unwrap_or_default();
        let cached_files = self
            .scan_state_repo
            .find_by_library_kind(&library.id, "webdav_file")
            .await
            .unwrap_or_default();
        let mut queue = std::collections::VecDeque::new();
        let mut visited_dirs = HashSet::new(); // Track visited directories to prevent cycles/re-visits

        queue.push_back(root_url.clone());
        visited_dirs.insert(root_url.clone());

        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| { let message = e.to_string(); TingError::network(NetworkErrorKind::Transport, "list_webdav_files::network", message).with_source(anyhow::anyhow!(e)) })?;

        let username = library.username.as_deref();

        // Decrypt password
        let password = if let Some(ref enc_pass) = library.password {
            if let Some(key) = &self.encryption_key {
                match crate::core::security::crypto::decrypt(enc_pass, key) {
                    Ok(p) => Some(p),
                    Err(_) => Some(enc_pass.clone()), // Fallback to raw if decrypt fails
                }
            } else {
                Some(enc_pass.clone())
            }
        } else {
            None
        };

        if !mode.is_full()
            && let Some(sync_state) = self
                .scan_state_repo
                .find(&library.id, &root_url, "webdav_sync")
                .await?
            && let Some(synced_files) = self
                .try_webdav_sync_collection(WebDavSyncRequest {
                    library,
                    root_url: &root_url,
                    previous_token: &sync_state.fingerprint,
                    client: &client,
                    username,
                    password: password.as_deref(),
                    cached_dirs: &cached_dirs,
                    cached_files: &cached_files,
                })
                .await?
        {
            return Ok(synced_files);
        }

        // Capture the collection token before the fallback/full traversal.
        // Saving a token obtained after traversal could miss a change that
        // happened between visiting its directory and fetching that token.
        let baseline_sync_token = match self
            .fetch_webdav_sync_token(&root_url, &client, username, password.as_deref())
            .await
        {
            Ok(token) => token,
            Err(e) => {
                warn!(
                    root_url = %root_url,
                    error = %e,
                    "Failed to capture WebDAV sync token before directory scan"
                );
                None
            }
        };

        // Limit depth/count to prevent infinite loops
        let mut processed_dirs = 0;
        let max_dirs = 1000;
        let mut listing_complete = true;
        let mut last_request_time = std::time::Instant::now();
        let min_request_interval = std::time::Duration::from_millis(200); // 200ms between requests

        while let Some(current_url) = queue.pop_front() {
            // Check cancellation
            self.check_cancellation(task_id).await?;

            if processed_dirs >= max_dirs {
                warn!("Max WebDAV directories limit reached");
                listing_complete = false;
                break;
            }
            processed_dirs += 1;

            // Rate limiting: ensure minimum interval between requests
            let elapsed = last_request_time.elapsed();
            if elapsed < min_request_interval {
                let sleep_time = min_request_interval - elapsed;
                tokio::time::sleep(sleep_time).await;
            }

            // PROPFIND request with browser-like headers
            let mut req = client
                .request(
                    reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
                    &current_url,
                )
                .header("Depth", "1")
                .header(
                    "Accept",
                    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                )
                .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
                .header("Accept-Encoding", "gzip, deflate, br")
                .header("Connection", "keep-alive");

            if let (Some(u), Some(p)) = (username, &password) {
                req = req.basic_auth(u, Some(p));
            }

            last_request_time = std::time::Instant::now();

            match req.send().await {
                Ok(res) => {
                    if res.status().is_success() || res.status().as_u16() == 207 {
                        let xml = res.text().await.unwrap_or_default();
                        let items = self.parse_webdav_response(&xml, &current_url);

                        for (item_url, is_dir, last_mod, etag) in items {
                            // Avoid re-processing current_url (PROPFIND returns self)
                            // We need to handle trailing slashes carefully
                            let item_norm = item_url.trim_end_matches('/');
                            let current_norm = current_url.trim_end_matches('/');

                            if item_norm == current_norm {
                                continue;
                            }

                            if is_dir {
                                // Only collection ETags are used to reuse a subtree.
                                // Directory modification times commonly describe the
                                // collection object itself and may not change when a
                                // descendant is updated.
                                let validator = etag.clone();
                                let cached = if !mode.is_full() {
                                    validator.as_ref().and_then(|validator| {
                                        cached_dirs
                                            .get(&item_url)
                                            .filter(|state| state.fingerprint == *validator)
                                    })
                                } else {
                                    None
                                };

                                if cached.is_some() {
                                    let normalized_dir = item_url.trim_end_matches('/');
                                    let nested_prefix = format!("{}/", normalized_dir);
                                    cache_states.extend(
                                        cached_dirs
                                            .values()
                                            .filter(|state| {
                                                state
                                                    .entry_path
                                                    .trim_end_matches('/')
                                                    .starts_with(&nested_prefix)
                                            })
                                            .cloned(),
                                    );
                                    for cached_file in cached_files.values().filter(|state| {
                                        state.parent_path.as_deref().is_some_and(|parent| {
                                            let normalized_parent = parent.trim_end_matches('/');
                                            normalized_parent == normalized_dir
                                                || normalized_parent.starts_with(&nested_prefix)
                                        })
                                    }) {
                                        cache_states.push(cached_file.clone());
                                        let modified =
                                            cached_file.modified_at.as_deref().and_then(|value| {
                                                chrono::DateTime::parse_from_rfc2822(value)
                                                    .map(|dt| dt.with_timezone(&chrono::Utc))
                                                    .ok()
                                                    .or_else(|| {
                                                        chrono::DateTime::parse_from_rfc3339(value)
                                                            .map(|dt| {
                                                                dt.with_timezone(&chrono::Utc)
                                                            })
                                                            .ok()
                                                    })
                                            });
                                        let validator = cached_file
                                            .etag
                                            .clone()
                                            .or_else(|| cached_file.modified_at.clone());
                                        files.insert(
                                            cached_file.entry_path.clone(),
                                            (modified, validator),
                                        );
                                    }
                                } else if !visited_dirs.contains(&item_url) {
                                    visited_dirs.insert(item_url.clone());
                                    queue.push_back(item_url.clone());
                                }

                                if let Some(validator) = validator {
                                    let mut state = LibraryScanState::new(
                                        &library.id,
                                        &item_url,
                                        "webdav_dir",
                                        validator,
                                    );
                                    state.modified_at = last_mod;
                                    state.etag = etag;
                                    state.parent_path = Some(current_url.clone());
                                    cache_states.push(state);
                                }
                            } else {
                                // Parse last modified - try multiple formats
                                let raw_last_mod = last_mod.clone();
                                let dt = if let Some(lm) = raw_last_mod.as_deref() {
                                    // Try RFC 2822 first (e.g., "Mon, 15 Aug 2005 15:52:01 +0000")
                                    chrono::DateTime::parse_from_rfc2822(lm)
                                        .map(|dt| dt.with_timezone(&chrono::Utc))
                                        .ok()
                                        .or_else(|| {
                                            // Try RFC 3339 / ISO 8601 (e.g., "2005-08-15T15:52:01Z")
                                            chrono::DateTime::parse_from_rfc3339(lm)
                                                .map(|dt| dt.with_timezone(&chrono::Utc))
                                                .ok()
                                        })
                                        .or_else(|| {
                                            // Log parsing failure for debugging
                                            debug!(
                                                "Failed to parse WebDAV last modified time: {}",
                                                lm
                                            );
                                            None
                                        })
                                } else {
                                    None
                                };
                                let stable_validator = etag.clone().or_else(|| last_mod.clone());
                                files.insert(item_url.clone(), (dt, stable_validator.clone()));
                                let validator =
                                    stable_validator.clone().unwrap_or_else(|| item_url.clone());
                                let mut state = LibraryScanState::new(
                                    &library.id,
                                    &item_url,
                                    "webdav_file",
                                    validator,
                                );
                                state.modified_at = raw_last_mod;
                                state.etag = etag;
                                state.parent_path = Some(current_url.clone());
                                cache_states.push(state);
                            }
                        }
                    } else {
                        listing_complete = false;
                        warn!(
                            "WebDAV PROPFIND failed for {}: {}",
                            current_url,
                            res.status()
                        );
                    }
                }
                Err(e) => {
                    listing_complete = false;
                    warn!("WebDAV request failed for {}: {}", current_url, e);
                }
            }
        }

        if listing_complete {
            let mut current_dirs = HashMap::new();
            let mut current_files = HashMap::new();
            for state in cache_states {
                match state.entry_kind.as_str() {
                    "webdav_dir" => {
                        current_dirs.insert(state.entry_path.clone(), state);
                    }
                    "webdav_file" => {
                        current_files.insert(state.entry_path.clone(), state);
                    }
                    _ => {}
                }
            }

            let deleted_dirs = cached_dirs
                .keys()
                .filter(|path| !current_dirs.contains_key(*path))
                .cloned()
                .collect();
            let deleted_files = cached_files
                .keys()
                .filter(|path| !current_files.contains_key(*path))
                .cloned()
                .collect();
            let changed_states = current_dirs
                .into_values()
                .chain(current_files.into_values())
                .filter(|state| {
                    let cached = match state.entry_kind.as_str() {
                        "webdav_dir" => cached_dirs.get(&state.entry_path),
                        "webdav_file" => cached_files.get(&state.entry_path),
                        _ => None,
                    };
                    cached.is_none_or(|cached| !scan_state_payload_matches(cached, state))
                })
                .collect();

            self.scan_state_repo
                .delete_many(&library.id, "webdav_dir", deleted_dirs)
                .await?;
            self.scan_state_repo
                .delete_many(&library.id, "webdav_file", deleted_files)
                .await?;
            self.scan_state_repo.upsert_many(changed_states).await?;
        } else {
            // Keep old entries after a transient/partial listing so a failed
            // request cannot be interpreted as a remote deletion.
            self.scan_state_repo.upsert_many(cache_states).await?;
        }

        if listing_complete {
            if let Some(sync_token) = baseline_sync_token {
                let sync_state =
                    LibraryScanState::new(&library.id, &root_url, "webdav_sync", sync_token);
                self.scan_state_repo.upsert_many(vec![sync_state]).await?;
            } else {
                self.scan_state_repo
                    .delete_many(&library.id, "webdav_sync", vec![root_url.clone()])
                    .await?;
            }
        }

        if !listing_complete {
            return Err(TingError::network(
                NetworkErrorKind::Transport,
                "list_webdav_files::network",
                "WebDAV directory listing was incomplete; existing scan state was preserved"
                    .to_string(),
            ));
        }

        Ok(files
            .into_iter()
            .map(|(url, (modified, validator))| (url, modified, validator))
            .collect())
    }

    async fn try_webdav_sync_collection(
        &self,
        request: WebDavSyncRequest<'_>,
    ) -> Result<
        Option<
            Vec<(
                String,
                Option<chrono::DateTime<chrono::Utc>>,
                Option<String>,
            )>,
        >,
    > {
        let WebDavSyncRequest {
            library,
            root_url,
            previous_token,
            client,
            username,
            password,
            cached_dirs,
            cached_files,
        } = request;
        let mut current_dirs = cached_dirs.clone();
        let mut current_files = cached_files.clone();
        let mut request_token = previous_token.to_string();
        let mut final_token = None;

        // RFC 6578 allows a server to truncate a large change set with a
        // response-level 507. Follow the returned token until the server has
        // delivered the complete delta, keeping all mutations in memory until
        // the final page is accepted.
        for _ in 0..128 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let escaped_token = quick_xml::escape::escape(&request_token);
            let body = format!(
                r#"<?xml version="1.0" encoding="utf-8" ?>
<D:sync-collection xmlns:D="DAV:">
  <D:sync-token>{}</D:sync-token>
  <D:sync-level>infinite</D:sync-level>
  <D:prop>
    <D:getetag />
    <D:getlastmodified />
    <D:resourcetype />
  </D:prop>
</D:sync-collection>"#,
                escaped_token
            );
            let mut request = client
                .request(
                    reqwest::Method::from_bytes(b"REPORT").expect("REPORT is a valid HTTP method"),
                    root_url,
                )
                .header("Depth", "0")
                .header("Content-Type", "application/xml; charset=utf-8")
                .body(body);
            if let (Some(username), Some(password)) = (username, password) {
                request = request.basic_auth(username, Some(password));
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(e) => {
                    warn!(
                        root_url = %root_url,
                        error = %e,
                        "WebDAV sync-collection request failed; falling back to directory scan"
                    );
                    return Ok(None);
                }
            };
            if response.status().as_u16() != 207 {
                warn!(
                    root_url = %root_url,
                    status = %response.status(),
                    "WebDAV sync-collection is unavailable or the token is invalid; falling back"
                );
                return Ok(None);
            }
            let xml = response.text().await.map_err(|e| {
                let message = e.to_string();
                TingError::network(
                    NetworkErrorKind::Transport,
                    "try_webdav_sync_collection::network",
                    message,
                )
                .with_source(anyhow::anyhow!(e))
            })?;
            let report = self.parse_webdav_sync_report(&xml, root_url);
            if report.requires_fallback {
                warn!(
                    root_url = %root_url,
                    "WebDAV sync report contains an unsupported or incomplete subtree; falling back"
                );
                return Ok(None);
            }
            let Some(next_token) = report.next_token.filter(|token| !token.trim().is_empty())
            else {
                warn!(
                    root_url = %root_url,
                    "WebDAV sync-collection response omitted the next token; falling back"
                );
                return Ok(None);
            };

            for item in report.items {
                let normalized_path = item.url.trim_end_matches('/').to_string();
                let nested_prefix = format!("{}/", normalized_path);
                if item.deleted {
                    current_dirs.retain(|path, _| {
                        let normalized = path.trim_end_matches('/');
                        normalized != normalized_path && !normalized.starts_with(&nested_prefix)
                    });
                    current_files.retain(|path, state| {
                        let normalized = path.trim_end_matches('/');
                        let parent = state
                            .parent_path
                            .as_deref()
                            .unwrap_or("")
                            .trim_end_matches('/');
                        normalized != normalized_path
                            && !normalized.starts_with(&nested_prefix)
                            && parent != normalized_path
                            && !parent.starts_with(&nested_prefix)
                    });
                    continue;
                }

                let is_dir = item.is_dir || item.url.ends_with('/');
                let validator = item
                    .etag
                    .clone()
                    .or_else(|| item.last_modified.clone())
                    .unwrap_or_else(|| item.url.clone());
                let parent_path = webdav_parent_url(&item.url);
                let mut state = LibraryScanState::new(
                    &library.id,
                    &item.url,
                    if is_dir { "webdav_dir" } else { "webdav_file" },
                    validator,
                );
                state.modified_at = item.last_modified;
                state.etag = item.etag;
                state.parent_path = parent_path;
                if is_dir {
                    current_dirs.insert(item.url, state);
                } else {
                    current_files.insert(item.url, state);
                }
            }

            if report.truncated {
                if next_token == request_token {
                    warn!(
                        root_url = %root_url,
                        "WebDAV sync report was truncated without advancing its token; falling back"
                    );
                    return Ok(None);
                }
                request_token = next_token;
                continue;
            }

            final_token = Some(next_token);
            break;
        }

        let Some(final_token) = final_token else {
            warn!(
                root_url = %root_url,
                "WebDAV sync report exceeded the pagination limit; falling back"
            );
            return Ok(None);
        };

        let deleted_dirs = cached_dirs
            .keys()
            .filter(|path| !current_dirs.contains_key(*path))
            .cloned()
            .collect();
        let deleted_files = cached_files
            .keys()
            .filter(|path| !current_files.contains_key(*path))
            .cloned()
            .collect();
        let changed_states = current_dirs
            .values()
            .chain(current_files.values())
            .filter(|state| {
                let cached = match state.entry_kind.as_str() {
                    "webdav_dir" => cached_dirs.get(&state.entry_path),
                    "webdav_file" => cached_files.get(&state.entry_path),
                    _ => None,
                };
                cached.is_none_or(|cached| !scan_state_payload_matches(cached, state))
            })
            .cloned()
            .collect();

        self.scan_state_repo
            .delete_many(&library.id, "webdav_dir", deleted_dirs)
            .await?;
        self.scan_state_repo
            .delete_many(&library.id, "webdav_file", deleted_files)
            .await?;
        self.scan_state_repo.upsert_many(changed_states).await?;
        let sync_state = LibraryScanState::new(&library.id, root_url, "webdav_sync", final_token);
        self.scan_state_repo.upsert_many(vec![sync_state]).await?;

        Ok(Some(
            current_files
                .into_values()
                .map(|state| {
                    let modified = parse_webdav_datetime(state.modified_at.as_deref());
                    let validator = state.etag.clone().or(state.modified_at.clone());
                    (state.entry_path, modified, validator)
                })
                .collect(),
        ))
    }

    async fn fetch_webdav_sync_token(
        &self,
        root_url: &str,
        client: &reqwest::Client,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<Option<String>> {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let body = r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:">
  <D:prop><D:sync-token /></D:prop>
</D:propfind>"#;
        let mut request = client
            .request(
                reqwest::Method::from_bytes(b"PROPFIND").expect("PROPFIND is a valid HTTP method"),
                root_url,
            )
            .header("Depth", "0")
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(body);
        if let (Some(username), Some(password)) = (username, password) {
            request = request.basic_auth(username, Some(password));
        }
        let response = request.send().await.map_err(|e| {
            let message = e.to_string();
            TingError::network(
                NetworkErrorKind::Transport,
                "fetch_webdav_sync_token::network",
                message,
            )
            .with_source(anyhow::anyhow!(e))
        })?;
        if response.status().as_u16() != 207 && !response.status().is_success() {
            return Ok(None);
        }
        let xml = response.text().await.map_err(|e| {
            let message = e.to_string();
            TingError::network(
                NetworkErrorKind::Transport,
                "fetch_webdav_sync_token::network",
                message,
            )
            .with_source(anyhow::anyhow!(e))
        })?;
        Ok(parse_webdav_sync_token(&xml))
    }

    fn parse_webdav_sync_report(&self, xml: &str, base_url: &str) -> WebDavSyncReport {
        let mut report = WebDavSyncReport::default();
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut buf = Vec::new();
        let mut in_response = false;
        let mut in_propstat = false;
        let mut current_href = String::new();
        let mut current_is_collection = false;
        let mut current_deleted = false;
        let mut current_last_modified = None;
        let mut current_etag = None;
        let mut current_response_status = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(event)) => match event.local_name().as_ref() {
                    b"response" => {
                        in_response = true;
                        in_propstat = false;
                        current_href.clear();
                        current_is_collection = false;
                        current_deleted = false;
                        current_last_modified = None;
                        current_etag = None;
                        current_response_status = None;
                    }
                    b"propstat" if in_response => in_propstat = true,
                    b"href" if in_response => {
                        if let Ok(value) = reader.read_text(event.name()) {
                            current_href = value.to_string();
                        }
                    }
                    b"collection" if in_response => current_is_collection = true,
                    b"getlastmodified" if in_response => {
                        if let Ok(value) = reader.read_text(event.name()) {
                            current_last_modified = Some(value.to_string());
                        }
                    }
                    b"getetag" if in_response => {
                        if let Ok(value) = reader.read_text(event.name()) {
                            current_etag = Some(value.to_string());
                        }
                    }
                    b"status" if in_response && !in_propstat => {
                        if let Ok(value) = reader.read_text(event.name()) {
                            current_response_status = webdav_status_code(&value);
                            current_deleted = current_response_status == Some(404);
                        }
                    }
                    b"sync-token" => {
                        if let Ok(value) = reader.read_text(event.name())
                            && !in_response
                            && !value.trim().is_empty()
                        {
                            report.next_token = Some(value.to_string());
                        }
                    }
                    _ => {}
                },
                Ok(Event::Empty(event)) => {
                    if in_response && event.local_name().as_ref() == b"collection" {
                        current_is_collection = true;
                    }
                }
                Ok(Event::End(event)) => match event.local_name().as_ref() {
                    b"propstat" => in_propstat = false,
                    b"response" => {
                        if in_response && !current_href.is_empty() {
                            match webdav_sync_response_disposition(current_response_status) {
                                WebDavSyncResponseDisposition::Truncated => {
                                    report.truncated = true;
                                }
                                WebDavSyncResponseDisposition::Fallback => {
                                    report.requires_fallback = true;
                                }
                                WebDavSyncResponseDisposition::Apply => {
                                    report.items.push(WebDavSyncItem {
                                        url: self.resolve_webdav_url(base_url, &current_href),
                                        is_dir: current_is_collection,
                                        deleted: current_deleted,
                                        last_modified: current_last_modified.clone(),
                                        etag: current_etag.clone(),
                                    });
                                }
                            }
                        }
                        in_response = false;
                        in_propstat = false;
                    }
                    _ => {}
                },
                Ok(Event::Eof) => break,
                Err(e) => {
                    warn!(error = %e, "Failed to parse WebDAV sync-collection response");
                    break;
                }
                _ => {}
            }
            buf.clear();
        }
        report
    }

    fn parse_webdav_response(
        &self,
        xml: &str,
        base_url: &str,
    ) -> Vec<(String, bool, Option<String>, Option<String>)> {
        let mut items = Vec::new();
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let mut in_response = false;
        let mut current_href = String::new();
        let mut is_collection = false;
        let mut current_last_mod = None;
        let mut current_etag = None;
        let mut buf = Vec::new();

        // Simple state machine
        // Structure: <response> <href>...</href> ... <resourcetype><collection/></resourcetype> <getlastmodified>...</getlastmodified> ... </response>

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => match e.name().as_ref() {
                    b"D:response" | b"d:response" | b"response" => {
                        in_response = true;
                        current_href.clear();
                        is_collection = false;
                        current_last_mod = None;
                        current_etag = None;
                    }
                    b"D:href" | b"d:href" | b"href" => {
                        if in_response && let Ok(txt) = reader.read_text(e.name()) {
                            current_href = txt.to_string();
                        }
                    }
                    b"D:collection" | b"d:collection" | b"collection" => {
                        if in_response {
                            is_collection = true;
                        }
                    }
                    b"D:getlastmodified" | b"d:getlastmodified" | b"getlastmodified" => {
                        if in_response && let Ok(txt) = reader.read_text(e.name()) {
                            current_last_mod = Some(txt.to_string());
                        }
                    }
                    b"D:getetag" | b"d:getetag" | b"getetag" => {
                        if in_response && let Ok(txt) = reader.read_text(e.name()) {
                            current_etag = Some(txt.to_string());
                        }
                    }
                    _ => {}
                },
                Ok(Event::Empty(e)) => match e.name().as_ref() {
                    b"D:collection" | b"d:collection" | b"collection" if in_response => {
                        is_collection = true;
                    }
                    _ => {}
                },
                Ok(Event::End(e)) => {
                    match e.name().as_ref() {
                        b"D:response" | b"d:response" | b"response" => {
                            if in_response && !current_href.is_empty() {
                                // Resolve href to full URL
                                // href might be relative or absolute path
                                let full_url = self.resolve_webdav_url(base_url, &current_href);
                                items.push((
                                    full_url,
                                    is_collection,
                                    current_last_mod.clone(),
                                    current_etag.clone(),
                                ));
                            }
                            in_response = false;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        items
    }

    fn resolve_webdav_url(&self, base_request_url: &str, href: &str) -> String {
        // href typically looks like "/remote.php/webdav/folder/file.mp3"
        // base_request_url looks like "https://host/remote.php/webdav/folder"

        // We need to construct the full URL.
        // If href is already a full URL, return it.
        if href.starts_with("http") {
            return href.to_string();
        }

        // Parse base URL to get scheme and host
        if let Ok(base) = url::Url::parse(base_request_url)
            && let Ok(joined) = base.join(href)
        {
            return joined.to_string();
        }

        // Fallback simple join
        href.to_string()
    }

    pub(crate) fn decode_url_path(&self, url: &str) -> String {
        match urlencoding::decode(url) {
            Ok(s) => s.into_owned(),
            Err(_) => {
                // If standard decode fails (e.g. invalid UTF-8 from GBK),
                // we try to decode manually to bytes and then use lossy conversion.
                let mut bytes = Vec::new();
                let input_bytes = url.as_bytes();
                let mut i = 0;

                while i < input_bytes.len() {
                    if input_bytes[i] == b'%'
                        && i + 2 < input_bytes.len()
                        && let Ok(slice) = std::str::from_utf8(&input_bytes[i + 1..i + 3])
                        && let Ok(b) = u8::from_str_radix(slice, 16)
                    {
                        bytes.push(b);
                        i += 3;
                        continue;
                    }
                    bytes.push(input_bytes[i]);
                    i += 1;
                }
                String::from_utf8_lossy(&bytes).into_owned()
            }
        }
    }
}

fn parse_webdav_sync_token(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) if event.local_name().as_ref() == b"sync-token" => {
                if let Ok(value) = reader.read_text(event.name()) {
                    let value = value.trim();
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

fn webdav_status_code(status: &str) -> Option<u16> {
    status
        .split_ascii_whitespace()
        .find_map(|part| part.parse::<u16>().ok().filter(|code| *code >= 100))
}

fn webdav_sync_response_disposition(status: Option<u16>) -> WebDavSyncResponseDisposition {
    match status {
        Some(507) => WebDavSyncResponseDisposition::Truncated,
        Some(status) if status >= 400 && status != 404 => WebDavSyncResponseDisposition::Fallback,
        _ => WebDavSyncResponseDisposition::Apply,
    }
}

fn parse_webdav_datetime(value: Option<&str>) -> Option<chrono::DateTime<chrono::Utc>> {
    value.and_then(|value| {
        chrono::DateTime::parse_from_rfc2822(value)
            .map(|date| date.with_timezone(&chrono::Utc))
            .ok()
            .or_else(|| {
                chrono::DateTime::parse_from_rfc3339(value)
                    .map(|date| date.with_timezone(&chrono::Utc))
                    .ok()
            })
    })
}

fn webdav_parent_url(value: &str) -> Option<String> {
    let mut url = url::Url::parse(value).ok()?;
    let path = url.path().trim_end_matches('/').to_string();
    let parent = path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    let parent = if parent.is_empty() { "/" } else { parent }.to_string();
    url.set_path(&parent);
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string().trim_end_matches('/').to_string())
}

fn scan_state_payload_matches(left: &LibraryScanState, right: &LibraryScanState) -> bool {
    left.fingerprint == right.fingerprint
        && left.config_fingerprint == right.config_fingerprint
        && left.modified_at == right.modified_at
        && left.etag == right.etag
        && left.size == right.size
        && left.parent_path == right.parent_path
}

#[cfg(test)]
#[path = "../../../../tests/unit/core/library_scanner/webdav/listing.rs"]
mod tests;
