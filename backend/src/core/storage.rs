use crate::core::error::{Result, TingError};
use crate::db::models::Library;
use futures::stream::TryStreamExt;
use reqwest::{Client, Url};
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncSeekExt};
use tokio_util::io::StreamReader;
use std::io::SeekFrom;

#[derive(Clone)]
pub struct StorageService {
    client: Client,
}

impl StorageService {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Get a reader for a local file
    pub async fn get_local_reader(&self, path: &Path, range: Option<(u64, u64)>) -> Result<(File, u64)> {
        let mut file = File::open(path).await.map_err(TingError::IoError)?;
        let metadata = file.metadata().await.map_err(TingError::IoError)?;
        let file_size = metadata.len();

        if let Some((start, _)) = range {
            file.seek(SeekFrom::Start(start)).await.map_err(TingError::IoError)?;
        }
        
        Ok((file, file_size))
    }

    /// Get a reader for a WebDAV file
    pub async fn get_webdav_reader(
        &self, 
        library: &Library, 
        relative_path: &str, 
        range: Option<(u64, u64)>,
        decryption_key: &[u8; 32]
    ) -> Result<(Box<dyn AsyncRead + Send + Unpin>, u64)> {
        // Handle case where relative_path is actually an absolute URL (legacy/bug fix)
        // If it starts with the library URL, we strip it to get the relative path
        // OR we just use it as is if it matches the base.
        
        let base_url = Url::parse(&library.url).map_err(|e| TingError::ValidationError(e.to_string()))?;
        let mut url = base_url.clone();

        if relative_path.starts_with("http://") || relative_path.starts_with("https://") {
            // It's a full URL. Check if it belongs to this library.
            // We blindly trust it for now but ideally should verify host.
            // If it's a full URL, we parse it directly.
            url = Url::parse(relative_path).map_err(|e| TingError::ValidationError(e.to_string()))?;
        } else {
            // Construct from components
            let root = library.root_path.as_str();
            let root = if root.is_empty() { "/" } else { root };
            
            // Ensure paths are joined correctly without double slashes
            let root_trimmed = root.trim_matches('/');
            let rel_trimmed = relative_path.trim_matches('/');
            let full_path_str = if root_trimmed.is_empty() {
                 rel_trimmed.to_string()
            } else {
                 format!("{}/{}", root_trimmed, rel_trimmed)
            };
            
            // Mutate the URL to append path segments
            // IMPORTANT: We must NOT manually encode segments if we use push().
            // Url::push() automatically percent-encodes the segment.
            // If `full_path_str` is already encoded (e.g. "foo%20bar"), push() will make it "foo%2520bar".
            // So we must decode first if it is encoded.
            // BUT, detecting if it is encoded is hard. 
            // Assumption: `relative_path` coming from internal logic (like scanner) might be encoded or not.
            // If it comes from DB `path` column, and we store decoded path in DB, we are good.
            // If we store encoded path in DB (current state), we need to decode here.
            
            // Try to decode percent-encoded string
            let decoded_path = urlencoding::decode(&full_path_str).map_err(|e| TingError::ValidationError(e.to_string()))?;
            
            {
                let mut segments = url.path_segments_mut().map_err(|_| TingError::ValidationError("Invalid URL".to_string()))?;
                for segment in decoded_path.split('/') {
                    let s: &str = segment;
                    if !s.is_empty() {
                        segments.push(s);
                    }
                }
            }
        }

        let mut req = self.client.get(url.clone());
        
        if let (Some(u), Some(p)) = (&library.username, &library.password) {
            match crate::core::crypto::decrypt(p, decryption_key) {
                Ok(decrypted) => {
                    req = req.basic_auth(u, Some(decrypted));
                },
                Err(_) => {
                    req = req.basic_auth(u, Some(p));
                }
            }
        }
        
        if let Some((start, end)) = range {
            let end_byte = if end > 0 { end.saturating_sub(1) } else { 0 };
            if end > start {
                req = req.header("Range", format!("bytes={}-{}", start, end_byte));
            } else {
                 req = req.header("Range", format!("bytes={}-", start));
            }
        }

        let res = req.send().await.map_err(|e| TingError::NetworkError(e.to_string()))?;
        
        if !res.status().is_success() {
             return Err(TingError::NotFound(format!("WebDAV file not found: {} at {}", res.status(), url)));
        }

        let mut total_size = res.content_length().unwrap_or(0);
        
        // Try to parse Content-Range header to get total size if this is a partial content response
        if let Some(range_header) = res.headers().get("Content-Range") {
            if let Ok(range_str) = range_header.to_str() {
                // Format: bytes start-end/total
                if let Some(slash_pos) = range_str.rfind('/') {
                    if let Ok(total) = range_str[slash_pos + 1..].parse::<u64>() {
                        total_size = total;
                    }
                }
            }
        }
        
        // Convert stream to AsyncRead
        let stream = res.bytes_stream().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
        let reader = StreamReader::new(stream);
        
        Ok((Box::new(reader), total_size))
    }
}
