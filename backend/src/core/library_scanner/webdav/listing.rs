use super::super::LibraryScanner;
use crate::core::error::{Result, TingError};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::{HashMap, HashSet};
use tracing::{debug, warn};

impl LibraryScanner {
    /// List all files in a WebDAV library recursively
    pub(super) async fn list_webdav_files(
        &self,
        library: &crate::db::models::Library,
        task_id: Option<&str>,
    ) -> Result<Vec<(String, Option<chrono::DateTime<chrono::Utc>>)>> {
        // Simple BFS or recursive traversal
        // Start from root
        let root_url = if library.root_path.starts_with('/') {
            // Combine library.url + root_path
            let base = library.url.trim_end_matches('/');
            let path = library.root_path.trim_start_matches('/');
            if path.is_empty() {
                base.to_string()
            } else {
                format!("{}/{}", base, path)
            }
        } else {
            library.url.clone()
        };

        let mut files = HashMap::new(); // Use HashMap to store URL -> LastModified
        let mut queue = std::collections::VecDeque::new();
        let mut visited_dirs = HashSet::new(); // Track visited directories to prevent cycles/re-visits

        queue.push_back(root_url.clone());
        visited_dirs.insert(root_url);

        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| TingError::NetworkError(e.to_string()))?;

        let username = library.username.as_deref();

        // Decrypt password
        let password = if let Some(ref enc_pass) = library.password {
            if let Some(key) = &self.encryption_key {
                match crate::core::crypto::decrypt(enc_pass, key) {
                    Ok(p) => Some(p),
                    Err(_) => Some(enc_pass.clone()), // Fallback to raw if decrypt fails
                }
            } else {
                Some(enc_pass.clone())
            }
        } else {
            None
        };

        // Limit depth/count to prevent infinite loops
        let mut processed_dirs = 0;
        let max_dirs = 1000;
        let mut last_request_time = std::time::Instant::now();
        let min_request_interval = std::time::Duration::from_millis(200); // 200ms between requests

        while let Some(current_url) = queue.pop_front() {
            // Check cancellation
            self.check_cancellation(task_id).await?;

            if processed_dirs >= max_dirs {
                warn!("Max WebDAV directories limit reached");
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

                        for (item_url, is_dir, last_mod) in items {
                            // Avoid re-processing current_url (PROPFIND returns self)
                            // We need to handle trailing slashes carefully
                            let item_norm = item_url.trim_end_matches('/');
                            let current_norm = current_url.trim_end_matches('/');

                            if item_norm == current_norm {
                                continue;
                            }

                            if is_dir {
                                if !visited_dirs.contains(&item_url) {
                                    visited_dirs.insert(item_url.clone());
                                    queue.push_back(item_url);
                                }
                            } else {
                                // Parse last modified - try multiple formats
                                let dt = if let Some(lm) = last_mod {
                                    // Try RFC 2822 first (e.g., "Mon, 15 Aug 2005 15:52:01 +0000")
                                    chrono::DateTime::parse_from_rfc2822(&lm)
                                        .map(|dt| dt.with_timezone(&chrono::Utc))
                                        .ok()
                                        .or_else(|| {
                                            // Try RFC 3339 / ISO 8601 (e.g., "2005-08-15T15:52:01Z")
                                            chrono::DateTime::parse_from_rfc3339(&lm)
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
                                files.insert(item_url, dt);
                            }
                        }
                    } else {
                        warn!(
                            "WebDAV PROPFIND failed for {}: {}",
                            current_url,
                            res.status()
                        );
                    }
                }
                Err(e) => {
                    warn!("WebDAV request failed for {}: {}", current_url, e);
                }
            }
        }

        Ok(files.into_iter().collect())
    }

    fn parse_webdav_response(
        &self,
        xml: &str,
        base_url: &str,
    ) -> Vec<(String, bool, Option<String>)> {
        let mut items = Vec::new();
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);

        let mut in_response = false;
        let mut current_href = String::new();
        let mut is_collection = false;
        let mut current_last_mod = None;
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
                    }
                    b"D:href" | b"d:href" | b"href" => {
                        if in_response {
                            if let Ok(txt) = reader.read_text(e.name()) {
                                current_href = txt.to_string();
                            }
                        }
                    }
                    b"D:collection" | b"d:collection" | b"collection" => {
                        if in_response {
                            is_collection = true;
                        }
                    }
                    b"D:getlastmodified" | b"d:getlastmodified" | b"getlastmodified" => {
                        if in_response {
                            if let Ok(txt) = reader.read_text(e.name()) {
                                current_last_mod = Some(txt.to_string());
                            }
                        }
                    }
                    _ => {}
                },
                Ok(Event::Empty(e)) => match e.name().as_ref() {
                    b"D:collection" | b"d:collection" | b"collection" => {
                        if in_response {
                            is_collection = true;
                        }
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
                                items.push((full_url, is_collection, current_last_mod.clone()));
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
        if let Ok(base) = url::Url::parse(base_request_url) {
            if let Ok(joined) = base.join(href) {
                return joined.to_string();
            }
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
                    if input_bytes[i] == b'%' && i + 2 < input_bytes.len() {
                        if let Ok(slice) = std::str::from_utf8(&input_bytes[i + 1..i + 3]) {
                            if let Ok(b) = u8::from_str_radix(slice, 16) {
                                bytes.push(b);
                                i += 3;
                                continue;
                            }
                        }
                    }
                    bytes.push(input_bytes[i]);
                    i += 1;
                }
                String::from_utf8_lossy(&bytes).into_owned()
            }
        }
    }
}
