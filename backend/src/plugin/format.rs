//! Format plugin interface
//!
//! This module defines the interface for format plugins that handle audio file
//! format detection, decryption, transcoding, and metadata extraction.
//!
//! Format plugins must implement the `FormatPlugin` trait in addition to the base
//! `Plugin` trait. They provide functionality for:
//! - Detecting supported audio file formats
//! - Decrypting encrypted audio files
//! - Transcoding audio files to different formats
//! - Extracting metadata from audio files
//! - Reporting progress for long-running operations

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use crate::core::error::Result;
use super::Plugin;

/// Format plugin trait
///
/// All format plugins must implement this trait to provide audio file
/// processing functionality.
#[async_trait]
pub trait FormatPlugin: Plugin {
    /// Detect if a file is in the format supported by this plugin
    ///
    /// # Arguments
    /// * `file_path` - Path to the audio file to check
    ///
    /// # Returns
    /// `true` if the file is in a supported format, `false` otherwise
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or accessed
    fn detect(&self, file_path: &Path) -> Result<bool>;

    /// Decrypt an encrypted audio file
    ///
    /// # Arguments
    /// * `input` - Path to the encrypted input file
    /// * `output` - Path where the decrypted output should be written
    /// * `progress` - Callback function to report progress (0.0 to 1.0)
    ///
    /// # Returns
    /// `Ok(())` if decryption succeeds
    ///
    /// # Errors
    /// Returns an error if:
    /// - The input file is corrupted or in an incorrect format
    /// - The output file cannot be written
    /// - Decryption fails for any reason
    async fn decrypt(
        &self,
        input: &Path,
        output: &Path,
        progress: ProgressCallback,
    ) -> Result<()>;

    /// Transcode an audio file to a different format
    ///
    /// # Arguments
    /// * `input` - Path to the input audio file
    /// * `output` - Path where the transcoded output should be written
    /// * `options` - Transcoding options (format, bitrate, etc.)
    /// * `progress` - Callback function to report progress (0.0 to 1.0)
    ///
    /// # Returns
    /// `Ok(())` if transcoding succeeds
    ///
    /// # Errors
    /// Returns an error if:
    /// - The input file format is not supported
    /// - The output format is not supported
    /// - Transcoding fails for any reason
    async fn transcode(
        &self,
        input: &Path,
        output: &Path,
        options: TranscodeOptions,
        progress: ProgressCallback,
    ) -> Result<()>;

    /// Extract metadata from an audio file
    ///
    /// # Arguments
    /// * `file_path` - Path to the audio file
    ///
    /// # Returns
    /// Audio metadata including title, artist, duration, etc.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The file cannot be read
    /// - The file format is not supported
    /// - Metadata extraction fails
    fn extract_metadata(&self, file_path: &Path) -> Result<AudioMetadata>;
}

/// Progress callback function type
///
/// This callback is invoked during long-running operations to report progress.
/// The progress value should be between 0.0 (0%) and 1.0 (100%).
pub type ProgressCallback = Arc<dyn Fn(f32) + Send + Sync>;

/// Transcoding options
///
/// Specifies the desired output format and quality settings for audio transcoding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscodeOptions {
    /// Target output format
    pub output_format: AudioFormat,
    
    /// Target bitrate in bits per second (optional)
    /// If not specified, a default bitrate for the format will be used
    #[serde(default)]
    pub bitrate: Option<u32>,
    
    /// Target sample rate in Hz (optional)
    /// If not specified, the original sample rate will be preserved
    #[serde(default)]
    pub sample_rate: Option<u32>,
    
    /// Number of audio channels (optional)
    /// If not specified, the original channel count will be preserved
    #[serde(default)]
    pub channels: Option<u8>,
}

/// Audio format enumeration
///
/// Represents the supported audio file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    /// MP3 format
    Mp3,
    
    /// M4A/AAC format
    M4a,
    
    /// Ogg Vorbis format
    Ogg,
    
    /// FLAC lossless format
    Flac,
    
    /// WMA format
    Wma,
    
    /// OPUS format
    Opus,
}

impl AudioFormat {
    /// Get the file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            AudioFormat::Mp3 => "mp3",
            AudioFormat::M4a => "m4a",
            AudioFormat::Ogg => "ogg",
            AudioFormat::Flac => "flac",
            AudioFormat::Wma => "wma",
            AudioFormat::Opus => "opus",
        }
    }

    /// Get the MIME type for this format
    pub fn mime_type(&self) -> &'static str {
        match self {
            AudioFormat::Mp3 => "audio/mpeg",
            AudioFormat::M4a => "audio/mp4",
            AudioFormat::Ogg => "audio/ogg",
            AudioFormat::Flac => "audio/flac",
            AudioFormat::Wma => "audio/x-ms-wma",
            AudioFormat::Opus => "audio/opus",
        }
    }
}

/// Audio metadata
///
/// Contains metadata information extracted from an audio file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioMetadata {
    /// Track title (optional)
    #[serde(default)]
    pub title: Option<String>,
    
    /// Artist name (optional)
    #[serde(default)]
    pub artist: Option<String>,
    
    /// Album name (optional)
    #[serde(default)]
    pub album: Option<String>,
    
    /// Duration in seconds
    pub duration: u64,
    
    /// Bitrate in bits per second
    pub bitrate: u32,
    
    /// Sample rate in Hz
    pub sample_rate: u32,
    
    /// Number of audio channels (1 for mono, 2 for stereo, etc.)
    pub channels: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_format_extension() {
        assert_eq!(AudioFormat::Mp3.extension(), "mp3");
        assert_eq!(AudioFormat::M4a.extension(), "m4a");
        assert_eq!(AudioFormat::Ogg.extension(), "ogg");
        assert_eq!(AudioFormat::Flac.extension(), "flac");
        assert_eq!(AudioFormat::Wma.extension(), "wma");
        assert_eq!(AudioFormat::Opus.extension(), "opus");
    }

    #[test]
    fn test_audio_format_mime_type() {
        assert_eq!(AudioFormat::Mp3.mime_type(), "audio/mpeg");
        assert_eq!(AudioFormat::M4a.mime_type(), "audio/mp4");
        assert_eq!(AudioFormat::Ogg.mime_type(), "audio/ogg");
        assert_eq!(AudioFormat::Flac.mime_type(), "audio/flac");
        assert_eq!(AudioFormat::Wma.mime_type(), "audio/x-ms-wma");
        assert_eq!(AudioFormat::Opus.mime_type(), "audio/opus");
    }

    #[test]
    fn test_transcode_options_serialization() {
        let options = TranscodeOptions {
            output_format: AudioFormat::Mp3,
            bitrate: Some(192000),
            sample_rate: Some(44100),
            channels: Some(2),
        };

        let json = serde_json::to_string(&options).unwrap();
        let deserialized: TranscodeOptions = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.output_format, AudioFormat::Mp3);
        assert_eq!(deserialized.bitrate, Some(192000));
        assert_eq!(deserialized.sample_rate, Some(44100));
        assert_eq!(deserialized.channels, Some(2));
    }

    #[test]
    fn test_transcode_options_optional_fields() {
        let json = r#"{"output_format":"mp3"}"#;
        let options: TranscodeOptions = serde_json::from_str(json).unwrap();

        assert_eq!(options.output_format, AudioFormat::Mp3);
        assert_eq!(options.bitrate, None);
        assert_eq!(options.sample_rate, None);
        assert_eq!(options.channels, None);
    }

    #[test]
    fn test_audio_metadata_serialization() {
        let metadata = AudioMetadata {
            title: Some("Test Track".to_string()),
            artist: Some("Test Artist".to_string()),
            album: Some("Test Album".to_string()),
            duration: 180,
            bitrate: 320000,
            sample_rate: 44100,
            channels: 2,
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: AudioMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.title, Some("Test Track".to_string()));
        assert_eq!(deserialized.duration, 180);
        assert_eq!(deserialized.bitrate, 320000);
        assert_eq!(deserialized.channels, 2);
    }

    #[test]
    fn test_audio_metadata_optional_fields() {
        let json = r#"{"duration":180,"bitrate":320000,"sample_rate":44100,"channels":2}"#;
        let metadata: AudioMetadata = serde_json::from_str(json).unwrap();

        assert_eq!(metadata.title, None);
        assert_eq!(metadata.artist, None);
        assert_eq!(metadata.album, None);
        assert_eq!(metadata.duration, 180);
    }

    #[test]
    fn test_audio_format_serialization() {
        let format = AudioFormat::Mp3;
        let json = serde_json::to_string(&format).unwrap();
        assert_eq!(json, r#""mp3""#);

        let deserialized: AudioFormat = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, AudioFormat::Mp3);
    }
}
