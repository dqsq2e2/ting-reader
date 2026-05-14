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

use std::time::Duration;

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
    pub genre: Option<String>,
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
                AudioFormat::Wma, // Add Wma support explicitly
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
pub(crate) struct AudioCache {
    entries: std::collections::HashMap<String, CacheEntry>,
    total_size: usize,
    max_size: usize,
}

impl AudioCache {
    pub(crate) fn new(max_size: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            total_size: 0,
            max_size,
        }
    }

    pub(crate) fn get(&mut self, key: &str) -> Option<AudioMetadata> {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_accessed = std::time::SystemTime::now();
            Some(entry.metadata.clone())
        } else {
            None
        }
    }

    pub(crate) fn insert(&mut self, key: String, metadata: AudioMetadata, file_size: u64) {
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
