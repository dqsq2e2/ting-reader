use super::*;

#[test]
fn reads_case_insensitive_tags_and_audio_stream_fallbacks() {
    let bytes = br#"{
        "format":{"duration":"68.219","tags":{"TITLE":"Chapter","ALBUM":"Book","AUTHOR":"Narrator","WM/AlbumArtist":"Author"}},
        "streams":[{"index":0,"codec_type":"audio","tags":{"description":"Introduction"}},
                   {"index":1,"codec_type":"video","codec_name":"png","disposition":{"attached_pic":1}}]
    }"#;
    let (metadata, cover) = parse_probe(bytes).unwrap();
    assert_eq!(metadata.title.as_deref(), Some("Chapter"));
    assert_eq!(metadata.album.as_deref(), Some("Book"));
    assert_eq!(metadata.description.as_deref(), Some("Introduction"));
    assert_eq!(
        metadata.author_narrator(),
        (Some("Author".into()), Some("Narrator".into()))
    );
    assert_eq!(metadata.duration, 68.219);
    assert_eq!(cover, Some(1));
}

#[test]
fn ignores_empty_tags_invalid_duration_and_noncover_video() {
    let bytes = br#"{"format":{"duration":"NaN","tags":{"album_artist":" ","artist":"Reader"}},"streams":[{"index":1,"codec_type":"video","codec_name":"h264"}]}"#;
    let (metadata, cover) = parse_probe(bytes).unwrap();
    assert_eq!(metadata.author_narrator(), (Some("Reader".into()), None));
    assert_eq!(metadata.duration, 0.0);
    assert_eq!(cover, None);
}

#[test]
fn rejects_invalid_probe_output() {
    assert!(parse_probe(b"not JSON").is_err());
}

fn update() -> AudioMetadataUpdate {
    AudioMetadataUpdate {
        title: "New chapter".into(),
        album: "New book".into(),
        artist: "Narrator".into(),
        album_artist: "Author".into(),
        composer: "Narrator".into(),
        genre: "Audiobook".into(),
        description: "Introduction".into(),
    }
}

fn asf_fixture() -> Vec<u8> {
    let mut header = vec![
        0x30, 0x26, 0xb2, 0x75, 0x8e, 0x66, 0xcf, 0x11, 0xa6, 0xd9, 0, 0xaa, 0, 0x62, 0xce, 0x6c,
    ];
    header.extend_from_slice(&54_u64.to_le_bytes());
    header.extend_from_slice(&1_u32.to_le_bytes());
    header.extend_from_slice(&[1, 2]);
    header.extend_from_slice(&[0x42; 16]);
    header.extend_from_slice(&24_u64.to_le_bytes());
    header.extend_from_slice(b"UNCHANGED AUDIO PAYLOAD");
    header
}

#[test]
fn wma_write_preserves_audio_payload_and_unknown_objects() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.wma");
    let target = directory.path().join("output.wma");
    let original = asf_fixture();
    std::fs::write(&source, &original).unwrap();
    super::super::asf::write_metadata(
        &source,
        &target,
        &update(),
        None,
        &tokio_util::sync::CancellationToken::new(),
    )
    .unwrap();
    let bytes = std::fs::read(&target).unwrap();
    let size = u64::from_le_bytes(bytes[16..24].try_into().unwrap()) as usize;
    assert_eq!(&bytes[size..], b"UNCHANGED AUDIO PAYLOAD");
    assert!(bytes.windows(16).any(|window| window == [0x42; 16]));
    for text in [
        "New chapter",
        "New book",
        "Narrator",
        "Author",
        "Audiobook",
        "Introduction",
    ] {
        let encoded: Vec<_> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert!(bytes.windows(encoded.len()).any(|window| window == encoded));
    }
    assert_eq!(std::fs::read(source).unwrap(), original);
}

#[test]
fn repeated_wma_cover_write_replaces_instead_of_appending() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.wma");
    let second = directory.path().join("second.wma");
    let third = directory.path().join("third.wma");
    std::fs::write(&first, asf_fixture()).unwrap();
    let cancel = tokio_util::sync::CancellationToken::new();
    super::super::asf::write_metadata(&first, &second, &update(), Some(b"old JPEG"), &cancel)
        .unwrap();
    super::super::asf::write_metadata(&second, &third, &update(), Some(b"new JPEG"), &cancel)
        .unwrap();
    let bytes = std::fs::read(third).unwrap();
    let picture: Vec<_> = "WM/Picture"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert_eq!(
        bytes
            .windows(picture.len())
            .filter(|window| *window == picture)
            .count(),
        1
    );
    assert!(bytes.windows(8).any(|window| window == b"new JPEG"));
    assert!(!bytes.windows(8).any(|window| window == b"old JPEG"));
}

#[test]
fn rejects_oversized_and_malformed_asf_headers() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.wma");
    let target = directory.path().join("output.wma");
    for size in [0_u64, 9 * 1024 * 1024, u64::MAX] {
        let mut bytes = asf_fixture();
        bytes[16..24].copy_from_slice(&size.to_le_bytes());
        std::fs::write(&source, &bytes).unwrap();
        assert!(
            super::super::asf::write_metadata(
                &source,
                &target,
                &update(),
                None,
                &tokio_util::sync::CancellationToken::new()
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&source).unwrap(), bytes);
        assert!(!target.exists());
    }
}

#[test]
fn copy_repairs_prefixed_mp4_without_changing_original() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.mp3");
    let target = directory.path().join("output.m4a");
    for damaged in [false, true] {
        let mut bytes = b"ID3\x03\0\0\0\0\0\0".to_vec();
        let mp4 = b"\0\0\0\x1cftypM4A ";
        bytes.extend_from_slice(if damaged { &mp4[3..] } else { mp4 });
        std::fs::write(&source, &bytes).unwrap();
        copy_audio(
            &source,
            &target,
            "m4a",
            &tokio_util::sync::CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), mp4);
        assert_eq!(std::fs::read(&source).unwrap(), bytes);
    }
}

#[test]
fn audio_copy_obeys_cancellation() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.mp3");
    let target = directory.path().join("output.mp3");
    std::fs::write(&source, b"audio").unwrap();
    let cancel = tokio_util::sync::CancellationToken::new();
    cancel.cancel();
    assert!(copy_audio(&source, &target, "mp3", &cancel).is_err());
    assert_eq!(std::fs::read(&source).unwrap(), b"audio");
}

#[test]
fn cover_normalization_never_enlarges_small_images() {
    let directory = tempfile::tempdir().unwrap();
    let cover = directory.path().join("small.jpg");
    image::RgbImage::from_pixel(24, 16, image::Rgb([10, 70, 130]))
        .save(&cover)
        .unwrap();
    for small in [false, true] {
        let bytes = normalized_cover(&cover, small).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (24, 16));
        assert!(bytes.len() < 4096);
    }
}

#[tokio::test]
async fn invalid_cover_cannot_modify_original_or_leave_staging_files() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.mp3");
    let cover = directory.path().join("cover.jpg");
    std::fs::write(&source, b"\xff\xfb\x50\x00audio").unwrap();
    std::fs::write(&cover, b"not an image").unwrap();
    let original = std::fs::read(&source).unwrap();
    assert!(
        AudioService::write_file_metadata(&source, update(), Some(&cover))
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(source).unwrap(), original);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[tokio::test]
async fn invalid_audio_write_keeps_original_and_cleans_worker_staging() {
    for (extension, original) in [
        ("wma", {
            let mut bytes = asf_fixture();
            bytes[16..24].copy_from_slice(&0_u64.to_le_bytes());
            bytes
        }),
        ("m4a", b"\0\0\0\x1cftypM4A ".to_vec()),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("source.{extension}"));
        std::fs::write(&source, &original).unwrap();
        assert!(
            AudioService::write_file_metadata(&source, update(), None)
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(source).unwrap(), original);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

#[test]
fn maps_real_m4b_metadata_onto_book_fields() {
    let Some(path) = std::env::var_os("TING_TEST_M4B_PATH") else {
        return;
    };
    let mp4 = super::super::mp4::read_metadata(std::path::Path::new(&path)).unwrap();
    let metadata = audio_metadata_from_mp4(&mp4);
    assert_eq!(
        metadata.title.as_deref(),
        Some("The Worlds I See - Curiosity, Exploration, and Discovery at the Dawn of AI")
    );
    assert_eq!(metadata.album.as_deref(), Some("The Worlds I See"));
    assert_eq!(
        metadata.genre.as_deref(),
        Some("Biographies & Memoirs:Professionals & Academics:Science & Technology")
    );
    assert_eq!(metadata.publisher.as_deref(), Some("Macmillan Audio"));
    assert_eq!(metadata.asin.as_deref(), Some("B0BSP3QDZH"));
    assert_eq!(metadata.language.as_deref(), Some("english"));
    assert_eq!(metadata.published_date.as_deref(), Some("2023-11-07"));
    assert_eq!(
        metadata.author_narrator(),
        (Some("Fei-Fei Li".into()), Some("Cindy Kay".into()))
    );
    assert_eq!(metadata.chapters.len(), 16);
    assert_eq!(metadata.chapters[0].start_ms, 0);
    assert_eq!(metadata.chapters[0].end_ms, 38_173);
    assert_eq!(metadata.duration.round() as i64, 43_863);
    assert_eq!(metadata.sample_rate, Some(44_100));
    assert_eq!(metadata.channels, Some(2));
    assert_eq!(metadata.codec.as_deref(), Some("mp4a"));
    // The reference metadata reports 28 for this file, but the container's
    // decoder config and the file size agree on roughly 125 kbps.
    assert!(
        metadata.bitrate_kbps.unwrap_or_default() > 50,
        "{:?}",
        metadata.bitrate_kbps
    );
}
