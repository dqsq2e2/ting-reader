//! Native MP4/M4A/M4B metadata reader.
//!
//! Reads the ISO base media container directly instead of shelling out to
//! FFprobe, so the complete iTunes tag set, free-form tags (including the
//! Audible `JSON` payload), Nero `chpl` chapters, chapter text tracks, cover
//! art and the audio stream parameters are available in one pass. Only the
//! atoms that carry this information are read, which keeps a scan cheap even
//! for a multi-gigabyte audiobook whose `moov` sits at the end of the file.

use crate::core::app::error::{InputErrorKind, Result, TingError};
use base64::Engine;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const MAX_CHILDREN: usize = 100_000;
const MAX_CHAPTERS: usize = 2_048;
const MAX_TEXT_BYTES: usize = 1024 * 1024;
const MAX_TABLE_BYTES: usize = 32 * 1024 * 1024;
const MAX_PICTURE_BYTES: usize = 10 * 1024 * 1024;
const MAX_CHAPTER_TITLE_BYTES: usize = 4096;

/// One authored chapter of an audiobook file. Times are milliseconds from the
/// beginning of the media.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Mp4Chapter {
    pub(crate) title: String,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Mp4Picture {
    pub(crate) data: Vec<u8>,
    pub(crate) is_front_cover: bool,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Mp4AudioStream {
    pub(crate) codec: String,
    pub(crate) channels: u16,
    pub(crate) sample_rate: u32,
    pub(crate) bit_depth: u16,
    pub(crate) avg_bitrate: u32,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct Mp4Metadata {
    /// Tag values addressed by their raw atom name (`©nam`, `desc`, `CDEK`…).
    pub(crate) tags: BTreeMap<String, String>,
    /// Free-form `----` tags addressed as `mean:name`.
    pub(crate) freeform: BTreeMap<String, String>,
    pub(crate) chapters: Vec<Mp4Chapter>,
    pub(crate) pictures: Vec<Mp4Picture>,
    pub(crate) duration_ms: i64,
    pub(crate) audio: Option<Mp4AudioStream>,
}

impl Mp4Metadata {
    /// Look up a tag by any of the given atom/free-form names.
    pub(crate) fn tag(&self, names: &[&str]) -> Option<String> {
        names.iter().find_map(|name| {
            self.freeform
                .get(*name)
                .or_else(|| self.tags.get(*name))
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
    }
}

pub(crate) fn is_mp4_family_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("m4b" | "m4a" | "m4p" | "m4r" | "m4v" | "mp4" | "aax")
    )
}

pub(crate) fn read_metadata(path: &Path) -> Result<Mp4Metadata> {
    let mut reader = Reader::open(path)?;
    let top_level = reader.children_in(reader.media_start, reader.len)?;
    let moov = top_level
        .into_iter()
        .find(|atom| atom.kind == *b"moov")
        .ok_or_else(|| invalid_format("MP4 container has no moov box"))?;

    let mut state = State::default();
    for atom in reader.children(&moov)? {
        match &atom.kind {
            b"mvhd" => state.read_mvhd(&mut reader, &atom)?,
            b"trak" => state.read_track(&mut reader, &atom)?,
            b"udta" => state.read_udta(&mut reader, &atom)?,
            b"meta" => state.read_meta(&mut reader, &atom)?,
            b"chpl" => state.read_chpl(&mut reader, &atom)?,
            _ => {}
        }
    }

    state.finish(&mut reader)
}

fn invalid_format(message: impl Into<String>) -> TingError {
    TingError::invalid_request(
        InputErrorKind::InvalidFormat,
        "mp4_metadata::invalid_request",
        message.into(),
    )
}

fn atom_name(kind: [u8; 4]) -> String {
    kind.iter().map(|byte| char::from(*byte)).collect()
}

fn is_known_atom_start(bytes: &[u8]) -> bool {
    if bytes.len() < 4 {
        return false;
    }
    bytes.iter().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(*byte, b' ' | b'-' | b'_' | b'.' | 0xa9 | 0xaa)
    })
}

#[derive(Debug, Clone)]
struct Atom {
    kind: [u8; 4],
    start: u64,
    header: u64,
    size: u64,
}

impl Atom {
    fn payload_start(&self) -> u64 {
        self.start + self.header
    }

    fn end(&self) -> u64 {
        self.start + self.size
    }

    fn payload_len(&self) -> u64 {
        self.size - self.header
    }
}

struct Reader {
    file: File,
    len: u64,
    /// First byte of the MP4 container. Files rewritten by taggers can carry a
    /// leading ID3v2 tag even though the payload is an MP4 stream.
    media_start: u64,
}

impl Reader {
    fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path)?;
        let len = file.metadata()?.len();
        if len < 8 {
            return Err(invalid_format("File is too small to be an MP4 container"));
        }
        let mut media_start = 0_u64;
        if len >= 10 {
            let mut header = [0_u8; 10];
            file.seek(SeekFrom::Start(0))?;
            if file.read_exact(&mut header).is_ok() && &header[..3] == b"ID3" {
                if header[6..10].iter().any(|byte| byte & 0x80 != 0) {
                    return Err(invalid_format("Invalid leading ID3 tag"));
                }
                media_start = 10
                    + ((u64::from(header[6]) & 0x7f) << 21)
                    + ((u64::from(header[7]) & 0x7f) << 14)
                    + ((u64::from(header[8]) & 0x7f) << 7)
                    + (u64::from(header[9]) & 0x7f)
                    + if header[3] == 4 && header[5] & 0x10 != 0 {
                        10
                    } else {
                        0
                    };
                if media_start + 8 > len {
                    return Err(invalid_format("Leading ID3 tag leaves no MP4 container"));
                }
            }
        }
        Ok(Self {
            file,
            len,
            media_start,
        })
    }

    fn read_at(&mut self, offset: u64, length: usize) -> Result<Vec<u8>> {
        if length == 0 {
            return Ok(Vec::new());
        }
        if offset.saturating_add(length as u64) > self.len {
            return Err(invalid_format("MP4 atom extends past the end of the file"));
        }
        self.file.seek(SeekFrom::Start(offset))?;
        let mut buffer = vec![0_u8; length];
        self.file.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    fn atom_at(&mut self, offset: u64, limit: u64) -> Result<Option<Atom>> {
        if offset.saturating_add(8) > limit {
            return Ok(None);
        }
        let header = self.read_at(offset, 8)?;
        let mut size = u64::from(u32::from_be_bytes([
            header[0], header[1], header[2], header[3],
        ]));
        let mut kind = [0_u8; 4];
        kind.copy_from_slice(&header[4..8]);
        let mut header_len = 8_u64;
        if size == 1 {
            if offset.saturating_add(16) > limit {
                return Err(invalid_format("Truncated 64-bit MP4 atom header"));
            }
            let large = self.read_at(offset + 8, 8)?;
            size = u64::from_be_bytes(
                large
                    .try_into()
                    .map_err(|_| invalid_format("Invalid 64-bit MP4 atom size"))?,
            );
            header_len = 16;
        } else if size == 0 {
            size = limit - offset;
        }
        // `uuid` atoms carry a 16-byte extended type before their payload.
        if kind == *b"uuid" {
            header_len += 16;
        }
        if size < header_len || offset.saturating_add(size) > limit {
            return Err(invalid_format(format!(
                "Invalid MP4 atom size for '{}'",
                atom_name(kind)
            )));
        }
        Ok(Some(Atom {
            kind,
            start: offset,
            header: header_len,
            size,
        }))
    }

    fn children_in(&mut self, start: u64, end: u64) -> Result<Vec<Atom>> {
        let mut atoms = Vec::new();
        let mut cursor = start;
        while cursor < end {
            match self.atom_at(cursor, end)? {
                Some(atom) => {
                    let next = atom.end();
                    if next <= cursor {
                        return Err(invalid_format("MP4 atom does not advance"));
                    }
                    cursor = next;
                    atoms.push(atom);
                    if atoms.len() > MAX_CHILDREN {
                        return Err(invalid_format("MP4 container has too many atoms"));
                    }
                }
                None => break,
            }
        }
        Ok(atoms)
    }

    fn children(&mut self, atom: &Atom) -> Result<Vec<Atom>> {
        self.children_in(atom.payload_start(), atom.end())
    }

    fn payload(&mut self, atom: &Atom, cap: usize) -> Result<Vec<u8>> {
        let length = atom.payload_len();
        if length > cap as u64 {
            return Err(invalid_format(format!(
                "MP4 atom '{}' is larger than expected",
                atom_name(atom.kind)
            )));
        }
        self.read_at(atom.payload_start(), length as usize)
    }
}

#[derive(Debug, Default)]
struct Track {
    id: u32,
    handler: [u8; 4],
    timescale: u32,
    duration: u64,
    chapter_track_ids: Vec<u32>,
}

#[derive(Default)]
struct State {
    movie_timescale: u32,
    movie_duration: u64,
    tracks: Vec<Track>,
    tags: BTreeMap<String, String>,
    freeform: BTreeMap<String, String>,
    chapters: Vec<Mp4Chapter>,
    pictures: Vec<Mp4Picture>,
    audio: Option<Mp4AudioStream>,
}

impl State {
    fn duration_ms(&self) -> i64 {
        let from_movie = duration_to_ms(self.movie_duration, self.movie_timescale);
        if from_movie > 0 {
            return from_movie;
        }
        self.tracks
            .iter()
            .filter(|track| track.handler == *b"soun")
            .map(|track| duration_to_ms(track.duration, track.timescale))
            .max()
            .unwrap_or(0)
    }

    fn read_mvhd(&mut self, reader: &mut Reader, atom: &Atom) -> Result<()> {
        let payload = reader.payload(atom, 128)?;
        if payload.len() < 20 {
            return Ok(());
        }
        match payload[0] {
            1 if payload.len() >= 32 => {
                self.movie_timescale = u32::from_be_bytes(payload[20..24].try_into().unwrap());
                self.movie_duration = u64::from_be_bytes(payload[24..32].try_into().unwrap());
            }
            _ if payload.len() >= 20 => {
                self.movie_timescale = u32::from_be_bytes(payload[12..16].try_into().unwrap());
                self.movie_duration =
                    u32::from_be_bytes(payload[16..20].try_into().unwrap()) as u64;
            }
            _ => {}
        }
        Ok(())
    }

    fn read_track(&mut self, reader: &mut Reader, atom: &Atom) -> Result<()> {
        let mut track = Track::default();
        for child in reader.children(atom)? {
            match &child.kind {
                b"tkhd" => {
                    let payload = reader.payload(&child, 128)?;
                    track.id = read_tkhd_track_id(&payload).unwrap_or_default();
                }
                b"tref" => {
                    for reference in reader.children(&child)? {
                        if reference.kind == *b"chap" {
                            let payload = reader.payload(&reference, 4096)?;
                            track.chapter_track_ids.extend(
                                payload
                                    .chunks_exact(4)
                                    .map(|chunk| u32::from_be_bytes(chunk.try_into().unwrap())),
                            );
                        }
                    }
                }
                b"mdia" => read_mdia(reader, &child, &mut track)?,
                _ => {}
            }
        }
        self.tracks.push(track);
        Ok(())
    }

    fn read_udta(&mut self, reader: &mut Reader, atom: &Atom) -> Result<()> {
        for child in reader.children(atom)? {
            match &child.kind {
                b"meta" => self.read_meta(reader, &child)?,
                b"chpl" => self.read_chpl(reader, &child)?,
                _ => {}
            }
        }
        Ok(())
    }

    fn read_meta(&mut self, reader: &mut Reader, atom: &Atom) -> Result<()> {
        let start = meta_children_start(reader, atom)?;
        for child in reader.children_in(start, atom.end())? {
            match &child.kind {
                b"ilst" => self.read_ilst(reader, &child)?,
                b"keys" => {}
                _ => {}
            }
        }
        Ok(())
    }

    fn read_ilst(&mut self, reader: &mut Reader, atom: &Atom) -> Result<()> {
        for item in reader.children(atom)? {
            if item.kind == *b"----" {
                self.read_freeform_item(reader, &item)?;
            } else {
                self.read_tag_item(reader, &item)?;
            }
        }
        Ok(())
    }

    fn read_tag_item(&mut self, reader: &mut Reader, item: &Atom) -> Result<()> {
        let key = atom_name(item.kind);
        let mut values = Vec::new();
        for data in reader.children(item)? {
            if data.kind != *b"data" {
                continue;
            }
            let Ok(payload) = reader.payload(&data, MAX_PICTURE_BYTES) else {
                continue;
            };
            if key == "covr" {
                self.push_picture(&payload);
            } else if let Some(value) = decode_data_value(&key, &payload) {
                values.push(value);
            }
        }
        if values.is_empty() {
            return Ok(());
        }
        if key == "trkn" || key == "disk" {
            self.tags.insert(key.clone(), values[0].clone());
            if values.len() > 1 && !values[1].trim().is_empty() {
                self.tags.insert(format!("{key}_total"), values[1].clone());
            }
        } else if !values[0].trim().is_empty() {
            self.tags.insert(key, values[0].clone());
        }
        Ok(())
    }

    fn read_freeform_item(&mut self, reader: &mut Reader, item: &Atom) -> Result<()> {
        let mut mean = String::new();
        let mut name = String::new();
        let mut values = Vec::new();
        for child in reader.children(item)? {
            match &child.kind {
                b"mean" => mean = read_full_box_text(reader, &child)?,
                b"name" => name = read_full_box_text(reader, &child)?,
                b"data" => {
                    let payload = reader.payload(&child, MAX_PICTURE_BYTES)?;
                    if let Some(value) = decode_data_value(&name, &payload) {
                        values.push(value);
                    }
                }
                _ => {}
            }
        }
        if name.is_empty() || values.is_empty() {
            return Ok(());
        }
        let value = values[0].clone();
        if value.trim().is_empty() {
            return Ok(());
        }
        let key = if mean.is_empty() {
            name
        } else {
            format!("{mean}:{name}")
        };
        self.freeform.insert(key, value);
        Ok(())
    }

    fn push_picture(&mut self, payload: &[u8]) {
        if payload.len() < 8 {
            return;
        }
        let data_type = u32::from_be_bytes(payload[0..4].try_into().unwrap()) & 0x00FF_FFFF;
        // 13 = JPEG, 14 = PNG, 12 = GIF, 27 = BMP; other payloads are ignored.
        if !matches!(data_type, 12 | 13 | 14 | 27) {
            return;
        }
        let data = payload[8..].to_vec();
        if data.is_empty() || data.len() > MAX_PICTURE_BYTES {
            return;
        }
        let is_front_cover = self.pictures.is_empty();
        self.pictures.push(Mp4Picture {
            data,
            is_front_cover,
        });
    }

    fn read_chpl(&mut self, reader: &mut Reader, atom: &Atom) -> Result<()> {
        let payload = reader.payload(atom, MAX_TABLE_BYTES)?;
        if payload.len() < 5 {
            return Ok(());
        }
        let version = payload[0];
        let mut cursor = 4_usize;
        if version >= 1 {
            cursor += 4;
        }
        if cursor >= payload.len() {
            return Ok(());
        }
        let count = payload[cursor] as usize;
        cursor += 1;
        let duration_ms = self.duration_ms();
        let mut raw: Vec<(u64, String)> = Vec::new();
        for _ in 0..count.min(MAX_CHAPTERS) {
            if cursor + 9 > payload.len() {
                break;
            }
            let start = u64::from_be_bytes(payload[cursor..cursor + 8].try_into().unwrap());
            let title_len = payload[cursor + 8] as usize;
            cursor += 9;
            if cursor + title_len > payload.len() {
                break;
            }
            let title = String::from_utf8_lossy(&payload[cursor..cursor + title_len])
                .trim()
                .to_string();
            cursor += title_len;
            raw.push((start, title));
        }
        if raw.is_empty() {
            return Ok(());
        }
        // Nero stores start times in 100-nanosecond units. A few writers use
        // milliseconds instead; pick the interpretation that fits the movie.
        let scale = chpl_time_scale(&raw, duration_ms);
        let chapters = normalize_chapters(
            raw.into_iter()
                .map(|(start, title)| Mp4Chapter {
                    title,
                    start_ms: (start / scale) as i64,
                    end_ms: 0,
                })
                .collect(),
            duration_ms,
        );
        if !chapters.is_empty() {
            self.chapters = chapters;
        }
        Ok(())
    }

    fn finish(&mut self, reader: &mut Reader) -> Result<Mp4Metadata> {
        let audio_track = self
            .tracks
            .iter()
            .position(|track| track.handler == *b"soun");
        if let Some(index) = audio_track {
            self.read_audio_stream(reader, index)?;
        }
        if self.chapters.is_empty() {
            let chapter_ids: Vec<u32> = audio_track
                .and_then(|index| self.tracks.get(index))
                .map(|track| track.chapter_track_ids.clone())
                .unwrap_or_default();
            for id in chapter_ids {
                if let Some(index) = self.tracks.iter().position(|track| track.id == id) {
                    let chapters = self.read_chapter_track(reader, index)?;
                    if !chapters.is_empty() {
                        self.chapters = chapters;
                        break;
                    }
                }
            }
        }
        let duration_ms = self.duration_ms();
        if self.chapters.is_empty() {
            self.chapters = self.audible_json_chapters(duration_ms);
        }
        self.chapters = normalize_chapters(std::mem::take(&mut self.chapters), duration_ms);
        Ok(Mp4Metadata {
            tags: std::mem::take(&mut self.tags),
            freeform: std::mem::take(&mut self.freeform),
            chapters: std::mem::take(&mut self.chapters),
            pictures: std::mem::take(&mut self.pictures),
            duration_ms,
            audio: self.audio.take(),
        })
    }

    fn read_audio_stream(&mut self, reader: &mut Reader, track_index: usize) -> Result<()> {
        let track_id = self.tracks[track_index].id;
        let track_atom = find_track_atom(reader, track_id)?;
        let Some(track_atom) = track_atom else {
            return Ok(());
        };
        let Some(stbl) = find_descendant(reader, &track_atom, &[b"mdia", b"minf", b"stbl"])? else {
            return Ok(());
        };
        for child in reader.children(&stbl)? {
            if child.kind != *b"stsd" {
                continue;
            }
            let payload = reader.payload(&child, MAX_TABLE_BYTES)?;
            if let Some(stream) = parse_stsd(&payload)? {
                self.audio = Some(stream);
            }
        }
        Ok(())
    }

    fn read_chapter_track(
        &mut self,
        reader: &mut Reader,
        track_index: usize,
    ) -> Result<Vec<Mp4Chapter>> {
        let track = &self.tracks[track_index];
        let (track_id, timescale, track_duration) = (track.id, track.timescale, track.duration);
        let Some(track_atom) = find_track_atom(reader, track_id)? else {
            return Ok(Vec::new());
        };
        let Some(stbl) = find_descendant(reader, &track_atom, &[b"mdia", b"minf", b"stbl"])? else {
            return Ok(Vec::new());
        };
        let samples = read_sample_table(reader, &stbl)?;
        if samples.is_empty() || timescale == 0 {
            return Ok(Vec::new());
        }
        let mut ticks = 0_u64;
        let mut chapters = Vec::new();
        let duration_ms = duration_to_ms(track_duration, timescale).max(self.duration_ms());
        for sample in samples.iter().take(MAX_CHAPTERS) {
            let title = read_sample_text(reader, sample)?;
            let start_ms = (ticks.saturating_mul(1000) / u64::from(timescale)) as i64;
            ticks = ticks.saturating_add(sample.duration_ticks);
            chapters.push(Mp4Chapter {
                title,
                start_ms,
                end_ms: 0,
            });
        }
        Ok(normalize_chapters(chapters, duration_ms))
    }

    fn audible_json_chapters(&self, duration_ms: i64) -> Vec<Mp4Chapter> {
        let Some(value) = self
            .freeform
            .get("JSON")
            .or_else(|| self.tags.get("JSON"))
            .or_else(|| self.freeform.get("com.apple.iTunes:JSON"))
        else {
            return Vec::new();
        };
        let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(value) else {
            return Vec::new();
        };
        let Ok(json) = serde_json::from_slice::<serde_json::Value>(&decoded) else {
            return Vec::new();
        };
        let Some(chapters) = json.get("chapters").and_then(|value| value.as_array()) else {
            return Vec::new();
        };
        let chapters = chapters
            .iter()
            .filter_map(|chapter| {
                let start = chapter.get("start_offset_ms")?.as_i64()?;
                let length = chapter.get("length_ms").and_then(|value| value.as_i64())?;
                let title = chapter
                    .get("title")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                Some(Mp4Chapter {
                    title,
                    start_ms: start,
                    end_ms: start + length,
                })
            })
            .take(MAX_CHAPTERS)
            .collect::<Vec<_>>();
        normalize_chapters(chapters, duration_ms)
    }
}

fn read_mdia(reader: &mut Reader, atom: &Atom, track: &mut Track) -> Result<()> {
    for child in reader.children(atom)? {
        match &child.kind {
            b"mdhd" => {
                let payload = reader.payload(&child, 128)?;
                if let Some((timescale, duration)) = read_mdhd(&payload) {
                    track.timescale = timescale;
                    track.duration = duration;
                }
            }
            b"hdlr" => {
                let payload = reader.payload(&child, 256)?;
                if payload.len() >= 12 {
                    track.handler.copy_from_slice(&payload[8..12]);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn read_tkhd_track_id(payload: &[u8]) -> Option<u32> {
    match payload.first().copied() {
        Some(1) if payload.len() >= 24 => {
            Some(u32::from_be_bytes(payload[20..24].try_into().ok()?))
        }
        _ if payload.len() >= 16 => Some(u32::from_be_bytes(payload[12..16].try_into().ok()?)),
        _ => None,
    }
}

fn read_mdhd(payload: &[u8]) -> Option<(u32, u64)> {
    match payload.first().copied() {
        Some(1) if payload.len() >= 32 => {
            let timescale = u32::from_be_bytes(payload[20..24].try_into().ok()?);
            let duration = u64::from_be_bytes(payload[24..32].try_into().ok()?);
            Some((timescale, duration))
        }
        _ if payload.len() >= 20 => {
            let timescale = u32::from_be_bytes(payload[12..16].try_into().ok()?);
            let duration = u32::from_be_bytes(payload[16..20].try_into().ok()?) as u64;
            Some((timescale, duration))
        }
        _ => None,
    }
}

fn duration_to_ms(duration: u64, timescale: u32) -> i64 {
    if timescale == 0 || duration == 0 {
        return 0;
    }
    ((duration as u128 * 1000) / timescale as u128) as i64
}

fn meta_children_start(reader: &mut Reader, atom: &Atom) -> Result<u64> {
    let start = atom.payload_start();
    if start + 8 > atom.end() {
        return Ok(start);
    }
    let header = reader.read_at(start, 8)?;
    let size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as u64;
    let kind = [header[4], header[5], header[6], header[7]];
    if size >= 8 && start + size <= atom.end() && is_known_atom_start(&kind) {
        // Plain container: the first child starts immediately.
        Ok(start)
    } else {
        // FullBox `meta` with a version/flags prefix.
        Ok(start + 4)
    }
}

fn read_full_box_text(reader: &mut Reader, atom: &Atom) -> Result<String> {
    let payload = reader.payload(atom, MAX_TEXT_BYTES)?;
    if payload.len() <= 4 {
        return Ok(String::new());
    }
    Ok(String::from_utf8_lossy(&payload[4..]).trim().to_string())
}

fn decode_data_value(key: &str, payload: &[u8]) -> Option<String> {
    if payload.len() < 8 {
        return None;
    }
    let data_type = u32::from_be_bytes(payload[0..4].try_into().ok()?) & 0x00FF_FFFF;
    let data = &payload[8..];
    match data_type {
        1 | 0 => {
            if key == "trkn" || key == "disk" {
                if data.len() >= 6 {
                    let current = u16::from_be_bytes(data[2..4].try_into().ok()?);
                    let total = u16::from_be_bytes(data[4..6].try_into().ok()?);
                    return Some(if total > 0 {
                        format!("{current}/{total}")
                    } else {
                        current.to_string()
                    });
                }
                return None;
            }
            let text = String::from_utf8_lossy(data).trim().to_string();
            if text.is_empty() { None } else { Some(text) }
        }
        2 => {
            if !data.len().is_multiple_of(2) {
                return None;
            }
            let units = data
                .chunks_exact(2)
                .map(|chunk| u16::from_be_bytes(chunk.try_into().unwrap()))
                .collect::<Vec<_>>();
            let text = String::from_utf16_lossy(&units).trim().to_string();
            if text.is_empty() { None } else { Some(text) }
        }
        21 => Some(read_signed_integer(data).to_string()),
        22 => Some(read_unsigned_integer(data).to_string()),
        23 if data.len() >= 4 => Some(f32::from_be_bytes(data[..4].try_into().ok()?).to_string()),
        24 if data.len() >= 8 => Some(f64::from_be_bytes(data[..8].try_into().ok()?).to_string()),
        13 | 14 | 12 | 27 => None,
        _ => {
            // Some writers store UTF-8 text under an unknown type marker.
            let text = String::from_utf8_lossy(data).trim().to_string();
            if text.is_empty() { None } else { Some(text) }
        }
    }
}

fn read_signed_integer(data: &[u8]) -> i64 {
    match data.len() {
        1 => i64::from(data[0] as i8),
        2 => i64::from(i16::from_be_bytes([data[0], data[1]])),
        4 => i64::from(i32::from_be_bytes(data[..4].try_into().unwrap())),
        8 => i64::from_be_bytes(data[..8].try_into().unwrap()),
        _ => 0,
    }
}

fn read_unsigned_integer(data: &[u8]) -> u64 {
    match data.len() {
        1 => u64::from(data[0]),
        2 => u64::from(u16::from_be_bytes([data[0], data[1]])),
        4 => u64::from(u32::from_be_bytes(data[..4].try_into().unwrap())),
        8 => u64::from_be_bytes(data[..8].try_into().unwrap()),
        _ => 0,
    }
}

fn find_track_atom(reader: &mut Reader, track_id: u32) -> Result<Option<Atom>> {
    let top_level = reader.children_in(reader.media_start, reader.len)?;
    let Some(moov) = top_level.into_iter().find(|atom| atom.kind == *b"moov") else {
        return Ok(None);
    };
    for atom in reader.children(&moov)? {
        if atom.kind != *b"trak" {
            continue;
        }
        for child in reader.children(&atom)? {
            if child.kind != *b"tkhd" {
                continue;
            }
            let payload = reader.payload(&child, 128)?;
            if read_tkhd_track_id(&payload) == Some(track_id) {
                return Ok(Some(atom));
            }
        }
    }
    Ok(None)
}

fn find_descendant(reader: &mut Reader, atom: &Atom, path: &[&[u8; 4]]) -> Result<Option<Atom>> {
    let Some((head, tail)) = path.split_first() else {
        return Ok(Some(atom.clone()));
    };
    for child in reader.children(atom)? {
        if &child.kind == *head {
            return find_descendant(reader, &child, tail);
        }
    }
    Ok(None)
}

struct DateTable {
    sample_count: u32,
    delta: u32,
}

struct Sample {
    offset: u64,
    size: u32,
    duration_ticks: u64,
}

fn read_sample_table(reader: &mut Reader, stbl: &Atom) -> Result<Vec<Sample>> {
    let mut time_entries: Vec<DateTable> = Vec::new();
    let mut sample_sizes: Vec<u32> = Vec::new();
    let mut uniform_size = 0_u32;
    let mut chunk_offsets: Vec<u64> = Vec::new();
    let mut chunk_entries: Vec<(u32, u32)> = Vec::new();

    for child in reader.children(stbl)? {
        match &child.kind {
            b"stts" => {
                let payload = reader.payload(&child, MAX_TABLE_BYTES)?;
                if payload.len() >= 8 {
                    let count = u32::from_be_bytes(payload[4..8].try_into().unwrap()) as usize;
                    for index in 0..count {
                        let base = 8 + index * 8;
                        if base + 8 > payload.len() {
                            break;
                        }
                        time_entries.push(DateTable {
                            sample_count: u32::from_be_bytes(
                                payload[base..base + 4].try_into().unwrap(),
                            ),
                            delta: u32::from_be_bytes(
                                payload[base + 4..base + 8].try_into().unwrap(),
                            ),
                        });
                    }
                }
            }
            b"stsz" => {
                let payload = reader.payload(&child, MAX_TABLE_BYTES)?;
                if payload.len() >= 12 {
                    uniform_size = u32::from_be_bytes(payload[4..8].try_into().unwrap());
                    let count = u32::from_be_bytes(payload[8..12].try_into().unwrap()) as usize;
                    if uniform_size == 0 {
                        for index in 0..count {
                            let base = 12 + index * 4;
                            if base + 4 > payload.len() {
                                break;
                            }
                            sample_sizes.push(u32::from_be_bytes(
                                payload[base..base + 4].try_into().unwrap(),
                            ));
                        }
                    }
                }
            }
            b"stco" => {
                let payload = reader.payload(&child, MAX_TABLE_BYTES)?;
                if payload.len() >= 8 {
                    let count = u32::from_be_bytes(payload[4..8].try_into().unwrap()) as usize;
                    for index in 0..count {
                        let base = 8 + index * 4;
                        if base + 4 > payload.len() {
                            break;
                        }
                        chunk_offsets.push(u32::from_be_bytes(
                            payload[base..base + 4].try_into().unwrap(),
                        ) as u64);
                    }
                }
            }
            b"co64" => {
                let payload = reader.payload(&child, MAX_TABLE_BYTES)?;
                if payload.len() >= 8 {
                    let count = u32::from_be_bytes(payload[4..8].try_into().unwrap()) as usize;
                    for index in 0..count {
                        let base = 8 + index * 8;
                        if base + 8 > payload.len() {
                            break;
                        }
                        chunk_offsets.push(u64::from_be_bytes(
                            payload[base..base + 8].try_into().unwrap(),
                        ));
                    }
                }
            }
            b"stsc" => {
                let payload = reader.payload(&child, MAX_TABLE_BYTES)?;
                if payload.len() >= 8 {
                    let count = u32::from_be_bytes(payload[4..8].try_into().unwrap()) as usize;
                    for index in 0..count {
                        let base = 8 + index * 12;
                        if base + 12 > payload.len() {
                            break;
                        }
                        chunk_entries.push((
                            u32::from_be_bytes(payload[base..base + 4].try_into().unwrap()),
                            u32::from_be_bytes(payload[base + 4..base + 8].try_into().unwrap()),
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    if chunk_offsets.is_empty() || chunk_entries.is_empty() {
        return Ok(Vec::new());
    }

    let mut durations = Vec::new();
    'entries: for entry in &time_entries {
        for _ in 0..entry.sample_count {
            durations.push(u64::from(entry.delta));
            if durations.len() >= MAX_CHAPTERS {
                break 'entries;
            }
        }
    }

    let sample_total = if uniform_size > 0 {
        durations.len()
    } else {
        sample_sizes.len()
    };
    let mut samples = Vec::with_capacity(sample_total.min(MAX_CHAPTERS));
    let mut sample_index = 0_usize;
    for (chunk_index, offset) in chunk_offsets.iter().enumerate() {
        let samples_per_chunk = samples_per_chunk_for(chunk_index as u32 + 1, &chunk_entries);
        let mut cursor = *offset;
        for _ in 0..samples_per_chunk {
            if sample_index >= sample_total || samples.len() >= MAX_CHAPTERS {
                break;
            }
            let size = if uniform_size > 0 {
                uniform_size
            } else {
                sample_sizes.get(sample_index).copied().unwrap_or(0)
            };
            if size == 0 {
                break;
            }
            samples.push(Sample {
                offset: cursor,
                size,
                duration_ticks: durations.get(sample_index).copied().unwrap_or(0),
            });
            cursor = cursor.saturating_add(u64::from(size));
            sample_index += 1;
        }
        if samples.len() >= MAX_CHAPTERS {
            break;
        }
    }
    Ok(samples)
}

fn samples_per_chunk_for(chunk_number: u32, entries: &[(u32, u32)]) -> u32 {
    let mut current = 0_u32;
    for (first_chunk, samples_per_chunk) in entries {
        if *first_chunk <= chunk_number {
            current = *samples_per_chunk;
        } else {
            break;
        }
    }
    current
}

fn read_sample_text(reader: &mut Reader, sample: &Sample) -> Result<String> {
    let length = (sample.size as usize).min(MAX_CHAPTER_TITLE_BYTES);
    if length < 2 {
        return Ok(String::new());
    }
    let payload = reader.read_at(sample.offset, length)?;
    let text_len = u16::from_be_bytes([payload[0], payload[1]]) as usize;
    if text_len == 0 || text_len + 2 > payload.len() {
        let raw = String::from_utf8_lossy(&payload).trim().to_string();
        return Ok(if raw.len() > 2 { raw } else { String::new() });
    }
    Ok(String::from_utf8_lossy(&payload[2..2 + text_len])
        .trim()
        .to_string())
}

fn parse_stsd(payload: &[u8]) -> Result<Option<Mp4AudioStream>> {
    if payload.len() < 16 {
        return Ok(None);
    }
    let entry_count = u32::from_be_bytes(payload[4..8].try_into().unwrap());
    if entry_count == 0 {
        return Ok(None);
    }
    let entry_size = u32::from_be_bytes(payload[8..12].try_into().unwrap()) as usize;
    let format = [payload[12], payload[13], payload[14], payload[15]];
    if entry_size < 16 || 8 + entry_size > payload.len() {
        return Ok(None);
    }
    let entry = &payload[8..8 + entry_size];
    let mut stream = Mp4AudioStream {
        codec: atom_name(format).trim().to_string(),
        ..Default::default()
    };
    // Audio sample entry: 8 bytes reserved/reference index, then version,
    // revision, vendor, channel count, sample size, compression id, packet
    // size, sample rate (16.16 fixed point).
    if entry.len() >= 36 {
        let version = u16::from_be_bytes([entry[16], entry[17]]);
        stream.channels = u16::from_be_bytes([entry[24], entry[25]]);
        stream.bit_depth = u16::from_be_bytes([entry[26], entry[27]]);
        stream.sample_rate = u32::from_be_bytes(entry[32..36].try_into().unwrap()) >> 16;
        let mut child_start = 36;
        if version == 1 {
            child_start += 16;
        } else if version == 2 {
            child_start += 36;
        }
        if entry.len() > child_start {
            parse_sample_entry_children(&entry[child_start..], &mut stream)?;
        }
    }
    Ok(Some(stream))
}

fn parse_sample_entry_children(data: &[u8], stream: &mut Mp4AudioStream) -> Result<()> {
    let mut cursor = 0_usize;
    while cursor + 8 <= data.len() {
        let size = u32::from_be_bytes(data[cursor..cursor + 4].try_into().unwrap()) as usize;
        let kind = [
            data[cursor + 4],
            data[cursor + 5],
            data[cursor + 6],
            data[cursor + 7],
        ];
        if size < 8 || cursor + size > data.len() {
            break;
        }
        let payload = &data[cursor + 8..cursor + size];
        match &kind {
            b"esds" => parse_esds(payload, stream),
            b"alac" => parse_alac(payload, stream),
            _ => {}
        }
        cursor += size;
    }
    Ok(())
}

fn parse_esds(payload: &[u8], stream: &mut Mp4AudioStream) {
    let mut cursor = 4_usize; // version/flags
    while cursor < payload.len() {
        let tag = payload[cursor];
        cursor += 1;
        let Some(length) = read_descriptor_length(payload, &mut cursor) else {
            return;
        };
        let end = cursor.saturating_add(length).min(payload.len());
        match tag {
            0x03 => {
                // ES_Descriptor: ES_ID (2) + flags (1) then nested descriptors.
                if cursor + 3 <= end {
                    let flags = payload[cursor + 2];
                    cursor += 3;
                    if flags & 0x80 != 0 {
                        cursor += 2;
                    }
                    if flags & 0x40 != 0 && cursor < end {
                        let url_len = payload[cursor] as usize;
                        cursor += 1 + url_len;
                    }
                    if flags & 0x20 != 0 {
                        cursor += 2;
                    }
                }
            }
            0x04 => {
                if cursor + 13 <= end {
                    // Skip objectType/streamType/bufferSize/maxBitrate and keep
                    // the average bitrate the player surfaces as "bitrate".
                    stream.avg_bitrate =
                        u32::from_be_bytes(payload[cursor + 9..cursor + 13].try_into().unwrap());
                    cursor += 13;
                }
            }
            0x05 => {
                let config = &payload[cursor..end];
                parse_audio_specific_config(config, stream);
                cursor = end;
            }
            _ => {
                cursor = end;
            }
        }
    }
}

fn read_descriptor_length(payload: &[u8], cursor: &mut usize) -> Option<usize> {
    let mut length = 0_usize;
    for _ in 0..4 {
        let byte = *payload.get(*cursor)?;
        *cursor += 1;
        length = (length << 7) | usize::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return Some(length);
        }
    }
    Some(length)
}

fn parse_audio_specific_config(config: &[u8], stream: &mut Mp4AudioStream) {
    const SAMPLE_RATES: [u32; 13] = [
        96_000, 88_200, 64_000, 48_000, 44_100, 32_000, 24_000, 22_050, 16_000, 12_000, 11_025,
        8_000, 7_350,
    ];
    let mut reader = BitReader::new(config);
    let Some(mut object_type) = reader.read(5) else {
        return;
    };
    if object_type == 31 {
        match reader.read(6) {
            Some(extension) => object_type = 32 + extension,
            None => return,
        }
    }
    let Some(frequency_index) = reader.read(4).map(|value| value as usize) else {
        return;
    };
    let mut sample_rate = SAMPLE_RATES.get(frequency_index).copied();
    if frequency_index == 15 {
        sample_rate = reader.read(24);
    }
    let Some(channel_config) = reader.read(4) else {
        return;
    };
    if let Some(sample_rate) = sample_rate.filter(|value| *value > 0) {
        stream.sample_rate = sample_rate;
    }
    if channel_config > 0 {
        stream.channels = channel_config as u16;
    }
    let _ = object_type;
}

/// MSB-first bit reader for the MPEG-4 AudioSpecificConfig.
struct BitReader<'a> {
    data: &'a [u8],
    bit: usize,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, bit: 0 }
    }

    fn read(&mut self, count: u32) -> Option<u32> {
        if count == 0 || count > 32 {
            return None;
        }
        let end = self.bit.checked_add(count as usize)?;
        if end > self.data.len() * 8 {
            return None;
        }
        let mut value = 0_u32;
        for _ in 0..count {
            let byte = self.data[self.bit / 8];
            let bit = (byte >> (7 - (self.bit % 8))) & 1;
            value = (value << 1) | u32::from(bit);
            self.bit += 1;
        }
        Some(value)
    }
}

fn parse_alac(payload: &[u8], stream: &mut Mp4AudioStream) {
    let cookie = if payload.len() >= 4 && payload[0..4] == [0, 0, 0, 0] {
        &payload[4..]
    } else {
        payload
    };
    if cookie.len() < 24 {
        return;
    }
    stream.bit_depth = u16::from(cookie[5]);
    stream.channels = u16::from(cookie[9]);
    stream.avg_bitrate =
        u32::from_be_bytes(cookie[16..20].try_into().unwrap()).max(stream.avg_bitrate);
    stream.sample_rate = u32::from_be_bytes(cookie[20..24].try_into().unwrap());
}

fn chpl_time_scale(raw: &[(u64, String)], duration_ms: i64) -> u64 {
    let max_start = raw.iter().map(|(start, _)| *start).max().unwrap_or(0);
    if max_start == 0 {
        return 1;
    }
    // Nero writes 100-nanosecond units; tolerate millisecond writers.
    if duration_ms > 0 && max_start <= duration_ms as u64 * 2 {
        1
    } else {
        10_000
    }
}

fn normalize_chapters(mut chapters: Vec<Mp4Chapter>, duration_ms: i64) -> Vec<Mp4Chapter> {
    chapters.retain(|chapter| {
        chapter.start_ms >= 0
            && (!chapter.title.trim().is_empty() || chapter.end_ms > chapter.start_ms)
    });
    chapters.sort_by_key(|chapter| chapter.start_ms);
    chapters.dedup_by_key(|chapter| chapter.start_ms);
    if chapters.len() > MAX_CHAPTERS {
        chapters.truncate(MAX_CHAPTERS);
    }
    for index in 0..chapters.len() {
        let next_start = chapters.get(index + 1).map(|chapter| chapter.start_ms);
        let end = match (chapters[index].end_ms, next_start) {
            (end, Some(next)) if end <= 0 || end > next => next,
            (end, Some(_)) if end > 0 => end,
            (end, None) if end > 0 => end,
            (_, None) => duration_ms,
            _ => next_start.unwrap_or(duration_ms),
        };
        chapters[index].end_ms = end.max(chapters[index].start_ms);
        chapters[index].title = chapters[index].title.trim().to_string();
        if chapters[index].title.is_empty() {
            chapters[index].title = format!("Chapter {}", index + 1);
        }
    }
    chapters
}

#[cfg(test)]
#[path = "../../../tests/unit/core/audio/mp4.rs"]
mod tests;
