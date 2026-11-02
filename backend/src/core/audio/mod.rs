//! Core audio services for metadata, local file ranges and FFmpeg processes.

mod asf;
pub mod ffmpeg;
pub mod metadata;
pub(crate) mod mp4;
pub mod streamer;

pub use ffmpeg::AudioService;
pub use streamer::{AudioFormat, AudioMetadata, AudioStreamer, StreamerConfig};
