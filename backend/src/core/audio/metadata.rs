//! Standard-container tags, embedded covers and transactional metadata writes.

use super::AudioService;
use crate::core::app::error::{AccessErrorKind, InputErrorKind, TaskErrorKind};
use crate::core::app::error::{Result, TingError};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

pub(crate) const MAX_COVER_BYTES: usize = 10 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct AudioInput {
    source: OsString,
    local_path: Option<PathBuf>,
    authorization: Option<String>,
}

impl AudioInput {
    pub(crate) fn local(path: &Path) -> Self {
        Self {
            source: path.as_os_str().into(),
            local_path: Some(path.to_path_buf()),
            authorization: None,
        }
    }

    pub(crate) fn remote(url: url::Url, authorization: Option<String>) -> Self {
        Self {
            source: url.to_string().into(),
            local_path: None,
            authorization,
        }
    }

    fn configure(&self, command: &mut Command) {
        if let Some(authorization) = &self.authorization {
            command.args(["-headers", authorization]);
        }
        command.args(["-rw_timeout", "15000000"]);
    }
}

/// One authored chapter inside an audio file. Times are milliseconds.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct AudioChapter {
    pub(crate) title: String,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct AudioFileMetadata {
    pub(crate) title: Option<String>,
    pub(crate) album: Option<String>,
    pub(crate) artist: Option<String>,
    pub(crate) album_artist: Option<String>,
    pub(crate) composer: Option<String>,
    pub(crate) genre: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) subtitle: Option<String>,
    pub(crate) publisher: Option<String>,
    pub(crate) published_date: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) asin: Option<String>,
    pub(crate) isbn: Option<String>,
    pub(crate) explicit: bool,
    pub(crate) codec: Option<String>,
    pub(crate) bit_depth: Option<u16>,
    pub(crate) bitrate_kbps: Option<u32>,
    pub(crate) sample_rate: Option<u32>,
    pub(crate) channels: Option<u16>,
    pub(crate) duration: f64,
    pub(crate) cover_url: Option<String>,
    pub(crate) chapters: Vec<AudioChapter>,
}

impl AudioFileMetadata {
    pub(crate) fn author_narrator(&self) -> (Option<String>, Option<String>) {
        let author = self.album_artist.clone().or_else(|| self.artist.clone());
        let narrator = self
            .artist
            .clone()
            .filter(|artist| Some(artist) != author.as_ref())
            .or_else(|| self.composer.clone());
        (author, narrator)
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct AudioMetadataUpdate {
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) album: String,
    pub(crate) album_artist: String,
    pub(crate) composer: String,
    pub(crate) genre: String,
    pub(crate) description: String,
}

#[derive(Default, Deserialize)]
struct ProbeSection {
    #[serde(default)]
    tags: BTreeMap<String, String>,
    duration: Option<String>,
}

#[derive(Default, Deserialize)]
struct ProbeStream {
    index: usize,
    codec_name: Option<String>,
    codec_type: Option<String>,
    #[serde(default)]
    disposition: BTreeMap<String, i32>,
    #[serde(default)]
    tags: BTreeMap<String, String>,
}

#[derive(Default, Deserialize)]
struct ProbeChapter {
    start_time: Option<String>,
    end_time: Option<String>,
    #[serde(default)]
    tags: BTreeMap<String, String>,
}

#[derive(Default, Deserialize)]
struct ProbeOutput {
    #[serde(default)]
    format: ProbeSection,
    #[serde(default)]
    streams: Vec<ProbeStream>,
    #[serde(default)]
    chapters: Vec<ProbeChapter>,
}

fn parse_probe(bytes: &[u8]) -> Result<(AudioFileMetadata, Option<usize>)> {
    let probe: ProbeOutput = serde_json::from_slice(bytes).map_err(|error| {
        let message = format!("Invalid FFprobe metadata: {error}");
        TingError::invalid_request(
            InputErrorKind::InvalidValue,
            "parse_probe::invalid_request",
            message,
        )
        .with_source(anyhow::anyhow!(error))
    })?;
    let mut tags = BTreeMap::new();
    for section in std::iter::once(&probe.format.tags).chain(
        probe
            .streams
            .iter()
            .filter(|stream| stream.codec_type.as_deref() == Some("audio"))
            .map(|stream| &stream.tags),
    ) {
        for (key, value) in section {
            if !value.trim().is_empty() {
                tags.entry(key.to_ascii_lowercase())
                    .or_insert_with(|| value.clone());
            }
        }
    }
    let tag = |keys: &[&str]| keys.iter().find_map(|key| tags.get(*key).cloned());
    let metadata = AudioFileMetadata {
        title: tag(&["title", "name", "nam"]),
        artist: tag(&["artist", "author", "art"]),
        album: tag(&["album", "wm/albumtitle", "alb"]),
        album_artist: tag(&["album_artist", "albumartist", "wm/albumartist"]),
        composer: tag(&["composer", "wm/composer"]),
        genre: tag(&["genre", "wm/genre"]),
        description: tag(&["description", "synopsis", "comment", "desc"]),
        subtitle: tag(&["subtitle"]),
        publisher: tag(&["publisher", "wm/publisher"]),
        published_date: tag(&["date", "year", "wm/year"]),
        language: tag(&["language", "lang"]),
        asin: tag(&["asin", "audible_asin"]),
        isbn: tag(&["isbn"]),
        explicit: tag(&["parental_advisory", "rating"]).is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "explicit" | "true" | "yes"
            )
        }),
        duration: probe
            .format
            .duration
            .as_deref()
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or_default(),
        chapters: probe
            .chapters
            .iter()
            .map(|chapter| {
                let start_ms = chapter
                    .start_time
                    .as_deref()
                    .and_then(|value| value.parse::<f64>().ok())
                    .filter(|value| value.is_finite())
                    .map(|value| (value * 1000.0).round() as i64)
                    .unwrap_or(0);
                let end_ms = chapter
                    .end_time
                    .as_deref()
                    .and_then(|value| value.parse::<f64>().ok())
                    .filter(|value| value.is_finite())
                    .map(|value| (value * 1000.0).round() as i64)
                    .unwrap_or(0);
                let title = chapter
                    .tags
                    .iter()
                    .find(|(key, _)| {
                        matches!(
                            key.to_ascii_lowercase().as_str(),
                            "title" | "name" | "chapter_title"
                        )
                    })
                    .map(|(_, value)| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| format!("Chapter {}", start_ms / 1000));
                AudioChapter {
                    title,
                    start_ms,
                    end_ms: end_ms.max(start_ms),
                }
            })
            .collect(),
        cover_url: None,
        ..Default::default()
    };
    let cover = probe
        .streams
        .iter()
        .find(|stream| stream.disposition.get("attached_pic") == Some(&1))
        .or_else(|| {
            probe.streams.iter().find(|stream| {
                stream.codec_type.as_deref() == Some("video")
                    && matches!(
                        stream.codec_name.as_deref(),
                        Some("mjpeg" | "png" | "webp" | "gif")
                    )
            })
        })
        .map(|stream| stream.index);
    Ok((metadata, cover))
}

pub(crate) struct TemporaryAudioFile(pub(crate) PathBuf);

impl Drop for TemporaryAudioFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

impl AudioService {
    pub(crate) async fn read_file_metadata(
        input: &AudioInput,
        cover_dir: Option<&Path>,
    ) -> Result<AudioFileMetadata> {
        if let Some(path) = input.local_path.clone()
            && is_mp4_file(&path)
        {
            let cover_dir = cover_dir.map(Path::to_path_buf);
            let native = tokio::task::spawn_blocking(move || {
                let metadata = super::mp4::read_metadata(&path)?;
                let mut audio = audio_metadata_from_mp4(&metadata);
                let cover = metadata
                    .pictures
                    .iter()
                    .find(|picture| picture.is_front_cover)
                    .or_else(|| metadata.pictures.first());
                if let (Some(dir), Some(picture)) = (cover_dir.as_deref(), cover) {
                    audio.cover_url = Some(save_cover_bytes(dir, &picture.data)?);
                } else if let Some(dir) = cover_dir.as_deref() {
                    audio.cover_url = existing_cover_path(dir);
                }
                Ok::<_, TingError>(audio)
            })
            .await
            .map_err(|error| {
                let message = error.to_string();
                TingError::task(TaskErrorKind::Join, "read_file_metadata::task", message)
                    .with_source(anyhow::anyhow!(error))
            })?;
            match native {
                Ok(metadata) => return Ok(metadata),
                Err(error) => tracing::warn!(
                    path = %input.source.to_string_lossy(),
                    error = %error,
                    "Native MP4 metadata read failed; falling back to FFprobe"
                ),
            }
        }

        let mut command = Self::ffprobe_command()?;
        command.args(["-v", "error"]);
        input.configure(&mut command);
        command.args([
            "-show_entries",
            "format=duration:format_tags:stream=index,codec_name,codec_type:stream_tags:stream_disposition=attached_pic:chapter=start_time,end_time:chapter_tags=title",
            "-of", "json",
        ]).arg(&input.source);
        let bytes = Self::bounded_output(command, 4 * 1024 * 1024, Duration::from_secs(30)).await?;
        let (mut metadata, cover) = parse_probe(&bytes)?;
        if let (Some(dir), Some(index)) = (cover_dir, cover) {
            match Self::extract_cover(input, index, dir).await {
                Ok(cover) => metadata.cover_url = Some(cover),
                Err(error) => {
                    tracing::warn!(error = %error, "Could not extract embedded audio cover")
                }
            }
        }
        Ok(metadata)
    }

    /// Read only the embedded chapter table of a local MP4/M4B file.
    pub(crate) async fn read_embedded_chapters(path: &Path) -> Result<Vec<AudioChapter>> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let metadata = super::mp4::read_metadata(&path)?;
            let duration_ms = metadata.duration_ms;
            Ok(metadata
                .chapters
                .into_iter()
                .filter(|chapter| chapter.end_ms > chapter.start_ms)
                .map(|chapter| AudioChapter {
                    title: chapter.title,
                    start_ms: chapter.start_ms,
                    end_ms: chapter.end_ms.min(duration_ms.max(chapter.start_ms)),
                })
                .collect())
        })
        .await
        .map_err(|error| {
            let message = error.to_string();
            TingError::task(TaskErrorKind::Join, "read_embedded_chapters::task", message)
                .with_source(anyhow::anyhow!(error))
        })?
    }

    async fn extract_cover(input: &AudioInput, index: usize, dir: &Path) -> Result<String> {
        if let Some(existing) = existing_cover_path(dir) {
            return Ok(existing);
        }
        let mut command = Self::ffmpeg_command()?;
        command.args(["-v", "error", "-nostdin"]);
        input.configure(&mut command);
        command.arg("-i").arg(&input.source).args([
            "-map",
            &format!("0:{index}"),
            "-frames:v",
            "1",
            "-c:v",
            "copy",
            "-f",
            "image2",
            "-update",
            "1",
            "pipe:1",
        ]);
        let bytes = Self::bounded_output(command, MAX_COVER_BYTES, Duration::from_secs(30)).await?;
        let dir = dir.to_path_buf();
        tokio::task::spawn_blocking(move || save_cover_bytes(&dir, &bytes))
            .await
            .map_err(|error| {
                let message = error.to_string();
                TingError::task(TaskErrorKind::Join, "extract_cover::task", message)
                    .with_source(anyhow::anyhow!(error))
            })?
    }

    pub(crate) async fn write_file_metadata(
        path: &Path,
        update: AudioMetadataUpdate,
        cover_path: Option<&Path>,
    ) -> Result<()> {
        let original = tokio::fs::metadata(path).await?;
        if original.permissions().readonly() {
            return Err(TingError::permission(
                AccessErrorKind::ResourceDenied,
                "write_file_metadata::permission",
                "Audio file is read-only".into(),
            ));
        }
        let format = detect_audio_format(path)
            .or_else(|| {
                path.extension()
                    .and_then(|value| value.to_str())
                    .map(str::to_ascii_lowercase)
            })
            .unwrap_or_default();
        if !matches!(
            format.as_str(),
            "mp3" | "m4a" | "m4b" | "mp4" | "flac" | "ogg" | "opus" | "wav" | "aac" | "wma"
        ) {
            return Err(TingError::invalid_request(
                InputErrorKind::UnsupportedValue,
                "write_file_metadata::invalid_request",
                format!("Unsupported audio container: {format}"),
            ));
        }
        let parent = path.parent().ok_or_else(|| {
            TingError::invalid_request(
                InputErrorKind::MissingField,
                "write_file_metadata::invalid_request",
                "Missing audio folder".into(),
            )
        })?;
        let staging = TemporaryAudioFile(parent.join(format!(
            ".metadata-{}.{}",
            uuid::Uuid::new_v4(),
            format
        )));
        let is_wma = format == "wma";
        let cover = if let Some(path) = cover_path {
            let path = path.to_path_buf();
            Some(
                tokio::task::spawn_blocking(move || normalized_cover(&path, is_wma))
                    .await
                    .map_err(|error| {
                        let message = error.to_string();
                        TingError::task(TaskErrorKind::Join, "write_file_metadata::task", message)
                            .with_source(anyhow::anyhow!(error))
                    })??,
            )
        } else {
            None
        };
        let source = path.to_path_buf();
        let cancel = tokio_util::sync::CancellationToken::new();
        let _cancel_on_drop = cancel.clone().drop_guard();
        let staging = tokio::task::spawn_blocking(move || {
            if is_wma {
                super::asf::write_metadata(
                    &source,
                    &staging.0,
                    &update,
                    cover.as_deref(),
                    &cancel,
                )?;
            } else {
                copy_audio(&source, &staging.0, &format, &cancel)?;
                write_lofty_metadata(&staging.0, &format, &update, cover)?;
            }
            Ok::<_, TingError>(staging)
        })
        .await
        .map_err(|error| {
            let message = error.to_string();
            TingError::task(TaskErrorKind::Join, "write_file_metadata::task", message)
                .with_source(anyhow::anyhow!(error))
        })??;
        if tokio::fs::metadata(&staging.0).await?.len() == 0 {
            return Err(TingError::invalid_request(
                InputErrorKind::InvalidValue,
                "write_file_metadata::invalid_request",
                "Metadata writer created an empty audio file".into(),
            ));
        }
        let current = tokio::fs::metadata(path).await?;
        if current.len() != original.len() || current.modified()? != original.modified()? {
            return Err(TingError::invalid_request(
                InputErrorKind::InvalidValue,
                "write_file_metadata::invalid_request",
                "Audio file changed during metadata writing".into(),
            ));
        }
        tokio::fs::set_permissions(&staging.0, original.permissions()).await?;
        tokio::fs::rename(&staging.0, path).await?;
        Ok(())
    }
}

/// Map the native MP4 tag set onto the normalized book metadata fields.
fn audio_metadata_from_mp4(mp4: &super::mp4::Mp4Metadata) -> AudioFileMetadata {
    let date = mp4.tag(&["\u{a9}day", "rldt"]);
    AudioFileMetadata {
        title: mp4.tag(&["\u{a9}nam"]),
        album: mp4.tag(&["\u{a9}alb"]),
        artist: mp4.tag(&["\u{a9}ART"]),
        album_artist: mp4.tag(&["aART"]),
        composer: mp4.tag(&["\u{a9}wrt"]),
        genre: mp4.tag(&["\u{a9}gen"]).or_else(|| mp4.tag(&["gnre"])),
        description: mp4.tag(&["desc", "ldes", "\u{a9}des", "\u{a9}cmt"]),
        subtitle: mp4.tag(&[
            "\u{a9}st3",
            "st3",
            "subtitle",
            "com.apple.iTunes:SUBTITLE",
            "com.apple.iTunes:subtitle",
        ]),
        publisher: mp4.tag(&["\u{a9}pub"]),
        published_date: date.clone(),
        language: mp4.tag(&["LANG", "\u{a9}lan", "language"]),
        asin: mp4.tag(&["CDEK", "prID", "ASIN", "asin"]).or_else(|| {
            // Audible files often only carry the product page URL.
            mp4.tag(&["\u{a9}WWW"]).and_then(|url| {
                url.trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
        }),
        isbn: mp4.tag(&["ISBN", "isbn"]),
        explicit: mp4
            .tag(&["cpil"])
            .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true")),
        codec: mp4
            .audio
            .as_ref()
            .map(|audio| audio.codec.clone())
            .filter(|value| !value.is_empty()),
        bit_depth: mp4
            .audio
            .as_ref()
            .map(|audio| audio.bit_depth)
            .filter(|value| *value > 0),
        bitrate_kbps: mp4
            .audio
            .as_ref()
            .and_then(|audio| (audio.avg_bitrate > 0).then_some(audio.avg_bitrate / 1000)),
        sample_rate: mp4
            .audio
            .as_ref()
            .map(|audio| audio.sample_rate)
            .filter(|value| *value > 0),
        channels: mp4
            .audio
            .as_ref()
            .map(|audio| audio.channels)
            .filter(|value| *value > 0),
        duration: (mp4.duration_ms.max(0) as f64) / 1000.0,
        cover_url: None,
        chapters: mp4
            .chapters
            .iter()
            .filter(|chapter| chapter.end_ms > chapter.start_ms)
            .map(|chapter| AudioChapter {
                title: chapter.title.clone(),
                start_ms: chapter.start_ms,
                end_ms: chapter.end_ms,
            })
            .collect(),
    }
}

fn is_mp4_file(path: &Path) -> bool {
    super::mp4::is_mp4_family_extension(path) || detect_audio_format(path).as_deref() == Some("m4a")
}

fn existing_cover_path(dir: &Path) -> Option<String> {
    ["jpg", "jpeg", "png", "webp", "gif"]
        .iter()
        .map(|extension| dir.join(format!("cover.{extension}")))
        .find(|path| path.is_file())
        .map(|path| path.to_string_lossy().replace('\\', "/"))
}

fn save_cover_bytes(dir: &Path, bytes: &[u8]) -> Result<String> {
    let extension = match image::guess_format(bytes) {
        Ok(image::ImageFormat::Jpeg) => "jpg",
        Ok(image::ImageFormat::Png) => "png",
        Ok(image::ImageFormat::WebP) => "webp",
        Ok(image::ImageFormat::Gif) => "gif",
        _ => {
            return Err(TingError::invalid_request(
                InputErrorKind::InvalidValue,
                "save_cover_bytes::invalid_request",
                "Invalid embedded cover image".into(),
            ));
        }
    };
    std::fs::create_dir_all(dir)?;
    let target = dir.join(format!("cover.{extension}"));
    let staging = TemporaryAudioFile(dir.join(format!(".cover-{}.tmp", uuid::Uuid::new_v4())));
    std::fs::write(&staging.0, bytes)?;
    std::fs::rename(&staging.0, &target)?;
    Ok(target.to_string_lossy().replace('\\', "/"))
}

pub(super) fn copy_audio(
    source: &Path,
    target: &Path,
    format: &str,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<()> {
    use std::io::Write;
    let mut source = std::fs::File::open(source)?;
    let mut target = std::fs::File::create(target)?;
    if matches!(format, "m4a" | "m4b" | "mp4") && prepare_mp4_source(&mut source)? {
        target.write_all(&[0, 0, 0])?;
    }
    let mut buffer = [0; 64 * 1024];
    loop {
        if cancel.is_cancelled() {
            return Err(TingError::task(
                TaskErrorKind::InvalidState,
                "copy_audio::task",
                "Audio metadata writing was cancelled".into(),
            ));
        }
        let length = source.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        target.write_all(&buffer[..length])?;
    }
    target.sync_all()?;
    Ok(())
}

fn prepare_mp4_source(source: &mut std::fs::File) -> Result<bool> {
    let mut header = [0; 10];
    let length = source.read(&mut header)?;
    if length < 10 || &header[..3] != b"ID3" {
        source.seek(SeekFrom::Start(0))?;
        return Ok(false);
    }
    if header[6..10].iter().any(|byte| byte & 0x80 != 0) {
        return Err(TingError::invalid_request(
            InputErrorKind::InvalidValue,
            "prepare_mp4_source::invalid_request",
            "Invalid leading ID3 tag".into(),
        ));
    }
    let offset = 10
        + ((u64::from(header[6]) & 0x7f) << 21)
        + ((u64::from(header[7]) & 0x7f) << 14)
        + ((u64::from(header[8]) & 0x7f) << 7)
        + (u64::from(header[9]) & 0x7f)
        + if header[3] == 4 && header[5] & 0x10 != 0 {
            10
        } else {
            0
        };
    source.seek(SeekFrom::Start(offset))?;
    let mut media = [0; 8];
    source.read_exact(&mut media)?;
    let repair = &media[1..5] == b"ftyp";
    if &media[4..8] != b"ftyp" && !repair {
        return Err(TingError::invalid_request(
            InputErrorKind::InvalidValue,
            "prepare_mp4_source::invalid_request",
            "Leading ID3 tag does not contain MP4 audio".into(),
        ));
    }
    source.seek(SeekFrom::Start(offset))?;
    Ok(repair)
}

fn write_lofty_metadata(
    path: &Path,
    format: &str,
    update: &AudioMetadataUpdate,
    cover: Option<Vec<u8>>,
) -> Result<()> {
    use lofty::config::{ParseOptions, ParsingMode, WriteOptions};
    use lofty::prelude::*;
    let options = ParseOptions::new()
        .read_properties(false)
        .parsing_mode(ParsingMode::Relaxed);
    let mut probe = lofty::probe::Probe::open(path)
        .map_err(|error| {
            let message = error.to_string();
            TingError::invalid_request(
                InputErrorKind::InvalidValue,
                "write_lofty_metadata::invalid_request",
                message,
            )
            .with_source(anyhow::anyhow!(error))
        })?
        .guess_file_type()?;
    if matches!(format, "m4a" | "mp4" | "m4b") {
        probe = probe.set_file_type(lofty::file::FileType::Mp4);
    }
    let mut file = probe.options(options).read().map_err(|error| {
        let message = error.to_string();
        TingError::invalid_request(
            InputErrorKind::InvalidValue,
            "write_lofty_metadata::invalid_request",
            message,
        )
        .with_source(anyhow::anyhow!(error))
    })?;
    let kind = file.file_type().primary_tag_type();
    if file.tag(kind).is_none() {
        file.insert_tag(lofty::tag::Tag::new(kind));
    }
    let tag = file.tag_mut(kind).ok_or_else(|| {
        TingError::invalid_request(
            InputErrorKind::MissingField,
            "write_lofty_metadata::invalid_request",
            "Missing audio tag".into(),
        )
    })?;
    tag.set_title(update.title.clone());
    tag.set_artist(update.artist.clone());
    tag.set_album(update.album.clone());
    tag.set_genre(update.genre.clone());
    tag.set_comment(update.description.clone());
    tag.insert_text(ItemKey::AlbumArtist, update.album_artist.clone());
    tag.insert_text(ItemKey::Composer, update.composer.clone());
    if let Some(bytes) = cover {
        while !tag.pictures().is_empty() {
            tag.remove_picture(0);
        }
        let mut picture = lofty::picture::Picture::from_reader(&mut std::io::Cursor::new(bytes))
            .map_err(|error| {
                let message = error.to_string();
                TingError::invalid_request(
                    InputErrorKind::InvalidValue,
                    "write_lofty_metadata::invalid_request",
                    message,
                )
                .with_source(anyhow::anyhow!(error))
            })?;
        picture.set_pic_type(lofty::picture::PictureType::CoverFront);
        tag.push_picture(picture);
    }
    file.save_to_path(path, WriteOptions::default())
        .map_err(|error| {
            let message = error.to_string();
            TingError::invalid_request(
                InputErrorKind::InvalidValue,
                "write_lofty_metadata::invalid_request",
                message,
            )
            .with_source(anyhow::anyhow!(error))
        })?;
    Ok(())
}

fn normalized_cover(path: &Path, small: bool) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_COVER_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_COVER_BYTES {
        return Err(TingError::invalid_request(
            InputErrorKind::OutOfRange,
            "normalized_cover::invalid_request",
            "Cover image is too large".into(),
        ));
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().map_err(|error| {
        let message = error.to_string();
        TingError::invalid_request(
            InputErrorKind::InvalidValue,
            "normalized_cover::invalid_request",
            message,
        )
        .with_source(anyhow::anyhow!(error))
    })?;
    for size in if small {
        &[600, 500, 400, 300, 240][..]
    } else {
        &[8192][..]
    } {
        let rgb = if image.width() > *size || image.height() > *size {
            image.thumbnail(*size, *size).to_rgb8()
        } else {
            image.to_rgb8()
        };
        for quality in if small {
            &[88, 80, 72, 64, 56][..]
        } else {
            &[90][..]
        } {
            let mut jpeg = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, *quality)
                .encode_image(&rgb)
                .map_err(|error| {
                    let message = error.to_string();
                    TingError::invalid_request(
                        InputErrorKind::InvalidValue,
                        "normalized_cover::invalid_request",
                        message,
                    )
                    .with_source(anyhow::anyhow!(error))
                })?;
            if !small || jpeg.len() < 65_490 {
                return Ok(jpeg);
            }
        }
    }
    Err(TingError::invalid_request(
        InputErrorKind::OutOfRange,
        "normalized_cover::invalid_request",
        "Cannot fit cover into the ASF attribute limit".into(),
    ))
}

pub(crate) fn detect_audio_format(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut header = [0_u8; 12];
    if file.read(&mut header).ok()? < 4 {
        return None;
    }
    let mut media_offset = 0_u64;
    if &header[..3] == b"ID3" {
        media_offset = 10
            + ((u64::from(header[6]) & 0x7f) << 21)
            + ((u64::from(header[7]) & 0x7f) << 14)
            + ((u64::from(header[8]) & 0x7f) << 7)
            + (u64::from(header[9]) & 0x7f);
        file.seek(SeekFrom::Start(media_offset)).ok()?;
        header.fill(0);
        if file.read(&mut header).ok()? < 4 {
            return None;
        }
    }
    let format = if &header[4..8] == b"ftyp" || (media_offset > 0 && &header[1..5] == b"ftyp") {
        "m4a"
    } else if header.starts_with(b"fLaC") {
        "flac"
    } else if header.starts_with(b"OggS") {
        "ogg"
    } else if header.starts_with(b"RIFF") && &header[8..12] == b"WAVE" {
        "wav"
    } else if header
        == [
            0x30, 0x26, 0xb2, 0x75, 0x8e, 0x66, 0xcf, 0x11, 0xa6, 0xd9, 0x00, 0xaa,
        ]
    {
        "wma"
    } else if header[0] == 0xff && header[1] & 0xf6 == 0xf0 {
        "aac"
    } else if header[0] == 0xff && header[1] & 0xe0 == 0xe0 {
        "mp3"
    } else {
        return None;
    };
    Some(format.into())
}

#[cfg(test)]
#[path = "../../../tests/unit/core/audio/metadata.rs"]
mod tests;
