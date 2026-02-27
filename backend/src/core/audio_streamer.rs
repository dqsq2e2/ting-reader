//! Audio Streaming Module
//!
//! This module provides core audio streaming functionality including:
//! - HTTP Range request support for segmented transmission
//! - Audio file streaming with proper Content-Type headers
//! - Audio metadata reading (duration, bitrate, sample rate)
//! - Audio format detection
//! - Breakpoint resume support (resume from any position)
//!
//! This is a CORE MODULE (not a plugin) that provides built-in audio streaming
//! functionality as specified in Requirements 16.1-16.7.

use crate::core::error::{Result, TingError};
use std::fs::File;
use std::io::SeekFrom;
use std::ops::Range;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tracing::{debug, info};

/// Audio format enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Mp3,
    M4a,
    Aac,
    Flac,
    Ogg,
    Opus,
    Wma,
    Unknown,
}

impl AudioFormat {
    /// Get the MIME type for this audio format
    pub fn mime_type(&self) -> &'static str {
        match self {
            AudioFormat::Mp3 => "audio/mpeg",
            AudioFormat::M4a => "audio/mp4",
            AudioFormat::Aac => "audio/aac",
            AudioFormat::Flac => "audio/flac",
            AudioFormat::Ogg => "audio/ogg",
            AudioFormat::Opus => "audio/opus",
            AudioFormat::Wma => "audio/x-ms-wma",
            AudioFormat::Unknown => "application/octet-stream",
        }
    }

    /// Get the file extension for this audio format
    pub fn extension(&self) -> &'static str {
        match self {
            AudioFormat::Mp3 => "mp3",
            AudioFormat::M4a => "m4a",
            AudioFormat::Aac => "aac",
            AudioFormat::Flac => "flac",
            AudioFormat::Ogg => "ogg",
            AudioFormat::Opus => "opus",
            AudioFormat::Wma => "wma",
            AudioFormat::Unknown => "bin",
        }
    }
}

/// Audio metadata structure
#[derive(Debug, Clone)]
pub struct AudioMetadata {
    pub format: AudioFormat,
    pub duration: Duration,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub channels: u8,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub composer: Option<String>,
}

/// Audio stream configuration
#[derive(Debug, Clone)]
pub struct StreamerConfig {
    pub cache_enabled: bool,
    pub cache_size: usize,
    pub buffer_size: usize,
    pub supported_formats: Vec<AudioFormat>,
}

impl Default for StreamerConfig {
    fn default() -> Self {
        Self {
            cache_enabled: true,
            cache_size: 100 * 1024 * 1024, // 100 MB
            buffer_size: 64 * 1024,         // 64 KB
            supported_formats: vec![
                AudioFormat::Mp3,
                AudioFormat::M4a,
                AudioFormat::Aac,
                AudioFormat::Flac,
            ],
        }
    }
}

/// Audio cache entry
#[derive(Debug, Clone)]
struct CacheEntry {
    metadata: AudioMetadata,
    file_size: u64,
    last_accessed: std::time::SystemTime,
}

/// Audio cache
struct AudioCache {
    entries: std::collections::HashMap<String, CacheEntry>,
    total_size: usize,
    max_size: usize,
}

impl AudioCache {
    fn new(max_size: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            total_size: 0,
            max_size,
        }
    }

    fn get(&mut self, key: &str) -> Option<AudioMetadata> {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_accessed = std::time::SystemTime::now();
            Some(entry.metadata.clone())
        } else {
            None
        }
    }

    fn insert(&mut self, key: String, metadata: AudioMetadata, file_size: u64) {
        // Simple LRU eviction if cache is full
        while self.total_size + file_size as usize > self.max_size && !self.entries.is_empty() {
            if let Some(oldest_key) = self.find_oldest_entry() {
                if let Some(entry) = self.entries.remove(&oldest_key) {
                    self.total_size = self.total_size.saturating_sub(entry.file_size as usize);
                }
            } else {
                break;
            }
        }

        self.entries.insert(
            key,
            CacheEntry {
                metadata,
                file_size,
                last_accessed: std::time::SystemTime::now(),
            },
        );
        self.total_size += file_size as usize;
    }

    fn find_oldest_entry(&self) -> Option<String> {
        self.entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(key, _)| key.clone())
    }
}

/// Audio streamer - provides HTTP Range request support and audio streaming
pub struct AudioStreamer {
    cache: Arc<RwLock<AudioCache>>,
    config: StreamerConfig,
}

impl AudioStreamer {
    /// Create a new audio streamer with the given configuration
    pub fn new(config: StreamerConfig) -> Self {
        let cache_size = if config.cache_enabled {
            config.cache_size
        } else {
            0
        };

        Self {
            cache: Arc::new(RwLock::new(AudioCache::new(cache_size))),
            config,
        }
    }

    /// Detect the audio format of a file
    pub fn detect_format(&self, file_path: &Path) -> Result<AudioFormat> {
        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let format = match extension.as_str() {
            "mp3" => AudioFormat::Mp3,
            "m4a" | "mp4" => AudioFormat::M4a,
            "aac" => AudioFormat::Aac,
            "flac" => AudioFormat::Flac,
            "ogg" => AudioFormat::Ogg,
            "opus" => AudioFormat::Opus,
            "wma" => AudioFormat::Wma,
            _ => AudioFormat::Unknown,
        };

        debug!("Detected format {:?} for file: {:?}", format, file_path);
        Ok(format)
    }

    /// Validate that an audio file exists and is readable
    pub fn validate_audio(&self, file_path: &Path) -> Result<()> {
        if !file_path.exists() {
            return Err(TingError::NotFound(format!(
                "Audio file not found: {:?}",
                file_path
            )));
        }

        if !file_path.is_file() {
            return Err(TingError::InvalidRequest(format!(
                "Path is not a file: {:?}",
                file_path
            )));
        }

        let format = self.detect_format(file_path)?;
        if !self.config.supported_formats.contains(&format) {
            return Err(TingError::InvalidRequest(format!(
                "Unsupported audio format: {:?}",
                format
            )));
        }

        Ok(())
    }

    /// Read audio metadata from a file
    pub fn read_metadata(&self, file_path: &Path) -> Result<AudioMetadata> {
        // Check cache first
        if self.config.cache_enabled {
            let cache_key = file_path.to_string_lossy().to_string();
            if let Ok(mut cache) = self.cache.write() {
                if let Some(metadata) = cache.get(&cache_key) {
                    debug!("Cache hit for metadata: {:?}", file_path);
                    return Ok(metadata);
                }
            }
        }

        // Validate file
        self.validate_audio(file_path)?;

        // Open file and probe format
        let file = File::open(file_path).map_err(|e| {
            TingError::IoError(std::io::Error::new(
                e.kind(),
                format!("Failed to open audio file {:?}: {}", file_path, e),
            ))
        })?;

        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| {
                TingError::InvalidRequest(format!("Failed to probe audio format: {}", e))
            })?;

        let mut format_reader = probed.format;
        let track = format_reader
            .default_track()
            .ok_or_else(|| TingError::InvalidRequest("No audio track found".to_string()))?;

        let codec_params = &track.codec_params;

        // Extract metadata
        let format = self.detect_format(file_path)?;
        let sample_rate = codec_params.sample_rate.unwrap_or(44100);
        let channels = codec_params.channels.map(|c| c.count()).unwrap_or(2) as u8;

        // Calculate duration
        let duration = if let Some(n_frames) = codec_params.n_frames {
            Duration::from_secs_f64(n_frames as f64 / sample_rate as f64)
        } else {
            Duration::from_secs(0)
        };

        // Calculate bitrate
        let file_size = std::fs::metadata(file_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let bitrate = if duration.as_secs() > 0 {
            ((file_size * 8) / duration.as_secs()) as u32
        } else {
            0
        };

        // Extract tags
        let mut title = None;
        let mut artist = None;
        let mut album = None;
        let mut album_artist = None;
        let mut composer = None;

        if let Some(metadata_rev) = format_reader.metadata().current() {
            for tag in metadata_rev.tags() {
                match tag.std_key {
                    Some(symphonia::core::meta::StandardTagKey::TrackTitle) => {
                        title = Some(tag.value.to_string());
                    }
                    Some(symphonia::core::meta::StandardTagKey::Artist) => {
                        artist = Some(tag.value.to_string());
                    }
                    Some(symphonia::core::meta::StandardTagKey::Album) => {
                        album = Some(tag.value.to_string());
                    }
                    Some(symphonia::core::meta::StandardTagKey::AlbumArtist) => {
                        album_artist = Some(tag.value.to_string());
                    }
                    Some(symphonia::core::meta::StandardTagKey::Composer) => {
                        composer = Some(tag.value.to_string());
                    }
                    _ => {}
                }
            }
        }

        let metadata = AudioMetadata {
            format,
            duration,
            bitrate,
            sample_rate,
            channels,
            title,
            artist,
            album,
            album_artist,
            composer,
        };

        // Cache the metadata
        if self.config.cache_enabled {
            let cache_key = file_path.to_string_lossy().to_string();
            if let Ok(mut cache) = self.cache.write() {
                cache.insert(cache_key, metadata.clone(), file_size);
            }
        }

        debug!("Read metadata for {:?}: {:?}", file_path, metadata);
        Ok(metadata)
    }

    /// Get the duration of an audio file
    pub fn get_duration(&self, file_path: &Path) -> Result<Duration> {
        let metadata = self.read_metadata(file_path)?;
        Ok(metadata.duration)
    }

    /// Parse HTTP Range header
    /// Supports formats: "bytes=0-499", "bytes=500-", "bytes=-500"
    pub fn parse_range_header(&self, header: &str, file_size: u64) -> Result<Range<u64>> {
        if !header.starts_with("bytes=") {
            return Err(TingError::InvalidRequest(
                "Invalid Range header format".to_string(),
            ));
        }

        let range_str = &header[6..]; // Skip "bytes="
        let parts: Vec<&str> = range_str.split('-').collect();

        if parts.len() != 2 {
            return Err(TingError::InvalidRequest(
                "Invalid Range header format".to_string(),
            ));
        }

        let (start, end) = if parts[0].is_empty() {
            // Suffix range: bytes=-500
            let suffix_len: u64 = parts[1].parse().map_err(|_| {
                TingError::InvalidRequest("Invalid Range header: invalid suffix length".to_string())
            })?;
            let start = file_size.saturating_sub(suffix_len);
            (start, file_size)
        } else {
            let start: u64 = parts[0].parse().map_err(|_| {
                TingError::InvalidRequest("Invalid Range header: invalid start".to_string())
            })?;
            
            let end = if parts[1].is_empty() {
                // Open-ended range: bytes=500-
                file_size
            } else {
                let end_val: u64 = parts[1].parse().map_err(|_| {
                    TingError::InvalidRequest("Invalid Range header: invalid end".to_string())
                })?;
                (end_val + 1).min(file_size) // Range is inclusive, so add 1
            };
            
            (start, end)
        };

        if start >= file_size {
            return Err(TingError::InvalidRequest(format!(
                "Range start {} exceeds file size {}",
                start, file_size
            )));
        }

        if start >= end {
            return Err(TingError::InvalidRequest(format!(
                "Invalid range: start {} >= end {}",
                start, end
            )));
        }

        Ok(start..end)
    }

    /// Stream audio file with optional range support
    /// Returns the content type, content length, range, and the file data
    pub async fn stream_audio(
        &self,
        file_path: &Path,
        range: Option<Range<u64>>,
    ) -> Result<(String, u64, Option<Range<u64>>, Vec<u8>)> {
        // Validate file
        self.validate_audio(file_path)?;

        // Get file size
        let file_size = std::fs::metadata(file_path)
            .map(|m| m.len())
            .map_err(|e| TingError::IoError(e))?;

        // Determine content type
        let format = self.detect_format(file_path)?;
        let content_type = format.mime_type().to_string();

        // Determine range to read
        let (start, end) = if let Some(ref r) = range {
            (r.start, r.end)
        } else {
            (0, file_size)
        };

        let content_length = end - start;

        // Read file data
        let mut file = tokio::fs::File::open(file_path)
            .await
            .map_err(|e| TingError::IoError(e))?;

        // Seek to start position
        file.seek(SeekFrom::Start(start))
            .await
            .map_err(|e| TingError::IoError(e))?;

        // Read the requested range
        let mut buffer = vec![0u8; content_length as usize];
        file.read_exact(&mut buffer)
            .await
            .map_err(|e| TingError::IoError(e))?;

        info!(
            "Streaming audio: {:?}, range: {:?}, size: {}",
            file_path, range, content_length
        );

        Ok((content_type, content_length, range, buffer))
    }

    /// Build HTTP response headers for range request
    /// Returns (status_code, content_type, content_length, content_range, accept_ranges)
    pub fn build_range_response(
        &self,
        content_type: String,
        range: Option<Range<u64>>,
        total_size: u64,
    ) -> (u16, String, u64, Option<String>, String) {
        if let Some(r) = range {
            let content_length = r.end - r.start;
            let content_range = format!("bytes {}-{}/{}", r.start, r.end - 1, total_size);
            (
                206, // 206 Partial Content
                content_type,
                content_length,
                Some(content_range),
                "bytes".to_string(),
            )
        } else {
            (
                200, // 200 OK
                content_type,
                total_size,
                None,
                "bytes".to_string(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_format_mime_types() {
        assert_eq!(AudioFormat::Mp3.mime_type(), "audio/mpeg");
        assert_eq!(AudioFormat::M4a.mime_type(), "audio/mp4");
        assert_eq!(AudioFormat::Aac.mime_type(), "audio/aac");
        assert_eq!(AudioFormat::Flac.mime_type(), "audio/flac");
    }

    #[test]
    fn test_parse_range_header() {
        let streamer = AudioStreamer::new(StreamerConfig::default());
        let file_size = 1000u64;

        // Test normal range
        let range = streamer
            .parse_range_header("bytes=0-499", file_size)
            .unwrap();
        assert_eq!(range, 0..500);

        // Test open-ended range
        let range = streamer
            .parse_range_header("bytes=500-", file_size)
            .unwrap();
        assert_eq!(range, 500..1000);

        // Test suffix range
        let range = streamer
            .parse_range_header("bytes=-500", file_size)
            .unwrap();
        assert_eq!(range, 500..1000);

        // Test invalid range
        assert!(streamer
            .parse_range_header("bytes=1000-", file_size)
            .is_err());
        assert!(streamer
            .parse_range_header("bytes=500-400", file_size)
            .is_err());
    }

    #[test]
    fn test_build_range_response() {
        let streamer = AudioStreamer::new(StreamerConfig::default());

        // Test partial content response
        let (status, content_type, content_length, content_range, accept_ranges) =
            streamer.build_range_response("audio/mpeg".to_string(), Some(0..500), 1000);

        assert_eq!(status, 206);
        assert_eq!(content_type, "audio/mpeg");
        assert_eq!(content_length, 500);
        assert_eq!(content_range, Some("bytes 0-499/1000".to_string()));
        assert_eq!(accept_ranges, "bytes");

        // Test full content response
        let (status, content_type, content_length, content_range, accept_ranges) =
            streamer.build_range_response("audio/mpeg".to_string(), None, 1000);

        assert_eq!(status, 200);
        assert_eq!(content_type, "audio/mpeg");
        assert_eq!(content_length, 1000);
        assert_eq!(content_range, None);
        assert_eq!(accept_ranges, "bytes");
    }
}
