use super::*;

fn atom(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(8 + payload.len());
    bytes.extend_from_slice(&((payload.len() + 8) as u32).to_be_bytes());
    bytes.extend_from_slice(kind);
    bytes.extend_from_slice(payload);
    bytes
}

fn container(kind: &[u8; 4], children: &[Vec<u8>]) -> Vec<u8> {
    let payload: Vec<u8> = children.iter().flatten().copied().collect();
    atom(kind, &payload)
}

fn data_atom(data_type: u32, bytes: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(8 + bytes.len());
    payload.extend_from_slice(&data_type.to_be_bytes());
    payload.extend_from_slice(&[0, 0, 0, 0]);
    payload.extend_from_slice(bytes);
    atom(b"data", &payload)
}

fn text_tag(name: &[u8; 4], value: &str) -> Vec<u8> {
    atom(name, &data_atom(1, value.as_bytes()))
}

fn freeform_tag(mean: &str, name: &str, value: &str) -> Vec<u8> {
    let mut mean_payload = vec![0, 0, 0, 0];
    mean_payload.extend_from_slice(mean.as_bytes());
    let mut name_payload = vec![0, 0, 0, 0];
    name_payload.extend_from_slice(name.as_bytes());
    atom(
        b"----",
        &[
            atom(b"mean", &mean_payload),
            atom(b"name", &name_payload),
            data_atom(1, value.as_bytes()),
        ]
        .concat(),
    )
}

fn mvhd(timescale: u32, duration: u32) -> Vec<u8> {
    let mut payload = vec![0_u8; 4];
    payload.extend_from_slice(&0_u32.to_be_bytes()); // creation
    payload.extend_from_slice(&0_u32.to_be_bytes()); // modification
    payload.extend_from_slice(&timescale.to_be_bytes());
    payload.extend_from_slice(&duration.to_be_bytes());
    payload.resize(100, 0);
    atom(b"mvhd", &payload)
}

fn tkhd(track_id: u32) -> Vec<u8> {
    let mut payload = vec![0_u8; 4];
    payload.extend_from_slice(&0_u32.to_be_bytes());
    payload.extend_from_slice(&0_u32.to_be_bytes());
    payload.extend_from_slice(&track_id.to_be_bytes());
    payload.resize(84, 0);
    atom(b"tkhd", &payload)
}

fn mdhd(timescale: u32, duration: u32) -> Vec<u8> {
    let mut payload = vec![0_u8; 4];
    payload.extend_from_slice(&0_u32.to_be_bytes());
    payload.extend_from_slice(&0_u32.to_be_bytes());
    payload.extend_from_slice(&timescale.to_be_bytes());
    payload.extend_from_slice(&duration.to_be_bytes());
    payload.extend_from_slice(&[0x55, 0xc4]); // und language
    atom(b"mdhd", &payload)
}

fn hdlr(handler: &[u8; 4]) -> Vec<u8> {
    let mut payload = vec![0_u8; 8];
    payload.extend_from_slice(handler);
    payload.extend_from_slice(&[0; 12]);
    payload.push(0);
    atom(b"hdlr", &payload)
}

fn esds(avg_bitrate: u32, max_bitrate: u32) -> Vec<u8> {
    let mut descriptor = vec![0x04, 17, 0x40, 0x15];
    descriptor.extend_from_slice(&[0, 0, 0]); // buffer size
    descriptor.extend_from_slice(&max_bitrate.to_be_bytes());
    descriptor.extend_from_slice(&avg_bitrate.to_be_bytes());
    descriptor.extend_from_slice(&[0x05, 2, 0x12, 0x10]);
    let mut payload = vec![0_u8; 4];
    payload.extend_from_slice(&descriptor);
    atom(b"esds", &payload)
}

fn audio_trak(sample_rate: u32, channels: u16, bitrate: u32) -> Vec<u8> {
    let mut entry = Vec::new();
    entry.extend_from_slice(&[0; 6]);
    entry.extend_from_slice(&1_u16.to_be_bytes()); // data reference index
    entry.extend_from_slice(&0_u16.to_be_bytes()); // version
    entry.extend_from_slice(&0_u16.to_be_bytes()); // revision
    entry.extend_from_slice(&0_u32.to_be_bytes()); // vendor
    entry.extend_from_slice(&channels.to_be_bytes());
    entry.extend_from_slice(&16_u16.to_be_bytes());
    entry.extend_from_slice(&0_u16.to_be_bytes());
    entry.extend_from_slice(&0_u16.to_be_bytes());
    entry.extend_from_slice(&(sample_rate << 16).to_be_bytes());
    entry.extend_from_slice(&esds(bitrate, bitrate + 1000));
    let mut sample_entry = Vec::new();
    sample_entry.extend_from_slice(&((entry.len() + 8) as u32).to_be_bytes());
    sample_entry.extend_from_slice(b"mp4a");
    sample_entry.extend_from_slice(&entry);
    let mut stsd = vec![0_u8; 4];
    stsd.extend_from_slice(&1_u32.to_be_bytes());
    stsd.extend_from_slice(&sample_entry);
    container(
        b"trak",
        &[
            tkhd(1),
            container(
                b"mdia",
                &[
                    mdhd(44_100, 44_100 * 10),
                    hdlr(b"soun"),
                    container(b"minf", &[container(b"stbl", &[atom(b"stsd", &stsd)])]),
                ],
            ),
        ],
    )
}

fn chpl(chapters: &[(u32, &str)]) -> Vec<u8> {
    let mut payload = vec![1, 0, 0, 0, 0, 0, 0, 0, chapters.len() as u8];
    for (start_ms, title) in chapters {
        payload.extend_from_slice(&((*start_ms as u64) * 10_000).to_be_bytes());
        payload.push(title.len() as u8);
        payload.extend_from_slice(title.as_bytes());
    }
    atom(b"chpl", &payload)
}

fn ilst(children: Vec<Vec<u8>>) -> Vec<u8> {
    let hdlr = {
        let mut payload = vec![0_u8; 8];
        payload.extend_from_slice(b"mdir");
        payload.extend_from_slice(&[0; 12]);
        payload.push(0);
        atom(b"hdlr", &payload)
    };
    container(b"meta", &[hdlr, container(b"ilst", &children)])
}

fn sample_file(extra_tags: Vec<Vec<u8>>, chapters: Option<Vec<u8>>) -> Vec<u8> {
    let mut ilst_items = vec![
        text_tag(b"\xa9nam", "The Worlds I See"),
        text_tag(b"\xa9ART", "Fei-Fei Li"),
        text_tag(b"aART", "Fei-Fei Li"),
        text_tag(b"\xa9wrt", "Cindy Kay"),
        text_tag(b"\xa9alb", "The Worlds I See"),
        text_tag(b"\xa9gen", "Biographies & Memoirs"),
        text_tag(b"desc", "A memoir."),
        text_tag(b"\xa9nrt", "Cindy Kay"),
        text_tag(b"\xa9pub", "Macmillan Audio"),
        text_tag(b"\xa9day", "2023-11-07"),
        text_tag(b"LANG", "english"),
        text_tag(b"CDEK", "B0BSP3QDZH"),
        text_tag(b"cprt", "\u{a9}2023 Fei-Fei Li"),
        atom(b"cpil", &data_atom(21, &[1])),
        atom(b"covr", &data_atom(13, b"fake-jpeg-bytes")),
    ];
    ilst_items.extend(extra_tags);
    let mut udta_children = Vec::new();
    if let Some(chapters) = chapters {
        udta_children.push(chapters);
    }
    udta_children.push(ilst(ilst_items));
    let mut file = atom(b"ftyp", b"M4B \0\0\0\0M4B mp42");
    file.extend(atom(b"mdat", &[0u8; 16]));
    file.extend(container(
        b"moov",
        &[
            mvhd(1000, 10_000),
            audio_trak(44_100, 2, 32_000),
            container(b"udta", &udta_children),
        ],
    ));
    file
}

fn write_temp(bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("book.m4b");
    std::fs::write(&path, bytes).unwrap();
    (directory, path)
}

#[test]
fn reads_tags_freeform_chapters_cover_and_audio_parameters() {
    let (directory, path) = write_temp(&sample_file(
        vec![
            freeform_tag("com.apple.iTunes", "SUBTITLE", "A Memoir"),
            freeform_tag("", "JSON", "eyJjaGFwdGVycyI6W119"),
        ],
        Some(chpl(&[(0, "Opening Credits"), (5_000, "Chapter 1")])),
    ));
    let metadata = read_metadata(&path).unwrap();
    assert_eq!(
        metadata.tag(&["\u{a9}nam"]).as_deref(),
        Some("The Worlds I See")
    );
    assert_eq!(metadata.tag(&["\u{a9}wrt"]).as_deref(), Some("Cindy Kay"));
    assert_eq!(metadata.tag(&["\u{a9}nrt"]).as_deref(), Some("Cindy Kay"));
    assert_eq!(
        metadata.tag(&["\u{a9}pub"]).as_deref(),
        Some("Macmillan Audio")
    );
    assert_eq!(metadata.tag(&["CDEK"]).as_deref(), Some("B0BSP3QDZH"));
    assert_eq!(metadata.tag(&["cpil"]).as_deref(), Some("1"));
    assert_eq!(
        metadata.tag(&["com.apple.iTunes:SUBTITLE"]).as_deref(),
        Some("A Memoir")
    );
    assert_eq!(metadata.duration_ms, 10_000);
    assert_eq!(metadata.chapters.len(), 2);
    assert_eq!(metadata.chapters[0].title, "Opening Credits");
    assert_eq!(metadata.chapters[0].start_ms, 0);
    assert_eq!(metadata.chapters[0].end_ms, 5_000);
    assert_eq!(metadata.chapters[1].start_ms, 5_000);
    assert_eq!(metadata.chapters[1].end_ms, 10_000);
    assert_eq!(metadata.pictures.len(), 1);
    assert!(metadata.pictures[0].is_front_cover);
    assert_eq!(metadata.pictures[0].data, b"fake-jpeg-bytes");
    let audio = metadata.audio.expect("audio stream");
    assert_eq!(audio.codec, "mp4a");
    assert_eq!(audio.channels, 2);
    assert_eq!(audio.sample_rate, 44_100);
    assert_eq!(audio.avg_bitrate, 32_000);
    drop(directory);
}

#[test]
fn falls_back_to_audible_json_chapters_when_no_chpl_exists() {
    let payload = br#"{"chapters":[{"start_offset_ms":0,"length_ms":1000,"title":"Intro"},{"start_offset_ms":1000,"length_ms":9000,"title":"Body"}]}"#;
    let encoded = base64::engine::general_purpose::STANDARD.encode(payload);
    let (directory, path) =
        write_temp(&sample_file(vec![freeform_tag("", "JSON", &encoded)], None));
    let metadata = read_metadata(&path).unwrap();
    assert_eq!(metadata.chapters.len(), 2);
    assert_eq!(metadata.chapters[0].title, "Intro");
    assert_eq!(metadata.chapters[1].end_ms, 10_000);
    drop(directory);
}

#[test]
fn reads_chapter_text_track_referenced_by_audio_track() {
    let chapter_text = b"\x00\x05Intro";
    let mut text_sample = vec![0_u8; 0];
    text_sample.extend_from_slice(chapter_text);
    let chapter_payload = text_sample.clone();
    let mut stbl_children = vec![atom(b"stsd", &{
        let mut stsd = vec![0_u8; 4];
        stsd.extend_from_slice(&1_u32.to_be_bytes());
        let mut entry = vec![0_u8; 8];
        entry.extend_from_slice(&0_u16.to_be_bytes());
        entry.resize(8 + 8, 0);
        let mut sample_entry = Vec::new();
        sample_entry.extend_from_slice(&((entry.len() + 8) as u32).to_be_bytes());
        sample_entry.extend_from_slice(b"text");
        sample_entry.extend_from_slice(&entry);
        stsd.extend_from_slice(&sample_entry);
        stsd
    })];
    // stts: one sample of 10_000 ticks, timescale 1000 -> 10s
    let mut stts = vec![0_u8; 4];
    stts.extend_from_slice(&1_u32.to_be_bytes());
    stts.extend_from_slice(&1_u32.to_be_bytes());
    stts.extend_from_slice(&10_000_u32.to_be_bytes());
    stbl_children.push(atom(b"stts", &stts));
    // stsz: one sample with the text size
    let mut stsz = vec![0_u8; 4];
    stsz.extend_from_slice(&0_u32.to_be_bytes());
    stsz.extend_from_slice(&1_u32.to_be_bytes());
    stsz.extend_from_slice(&(chapter_payload.len() as u32).to_be_bytes());
    stbl_children.push(atom(b"stsz", &stsz));
    // stsc: one sample per chunk
    let mut stsc = vec![0_u8; 4];
    stsc.extend_from_slice(&1_u32.to_be_bytes());
    stsc.extend_from_slice(&1_u32.to_be_bytes());
    stsc.extend_from_slice(&1_u32.to_be_bytes());
    stsc.extend_from_slice(&1_u32.to_be_bytes());
    stbl_children.push(atom(b"stsc", &stsc));

    let mut file = atom(b"ftyp", b"M4B \0\0\0\0M4B mp42");
    file.extend(atom(b"mdat", &chapter_payload));
    let text_offset = 8 + 8 + 8 + 8 + (chapter_text.len() as u32); // ftyp + mdat payload offset
    // Compute the absolute offset of the chapter bytes: ftyp(24) + mdat header(8).
    let sample_offset = 24_u32 + 8;
    let _ = text_offset;
    let mut stco = vec![0_u8; 4];
    stco.extend_from_slice(&1_u32.to_be_bytes());
    stco.extend_from_slice(&sample_offset.to_be_bytes());
    stbl_children.push(atom(b"stco", &stco));

    let text_trak = container(
        b"trak",
        &[
            tkhd(2),
            container(
                b"mdia",
                &[
                    mdhd(1000, 10_000),
                    hdlr(b"text"),
                    container(b"minf", &[container(b"stbl", &stbl_children)]),
                ],
            ),
        ],
    );
    let audio_trak = container(
        b"trak",
        &[
            tkhd(1),
            container(
                b"tref",
                &[{
                    let mut payload = Vec::new();
                    payload.extend_from_slice(&2_u32.to_be_bytes());
                    atom(b"chap", &payload)
                }],
            ),
            container(
                b"mdia",
                &[
                    mdhd(44_100, 441_000),
                    hdlr(b"soun"),
                    container(b"minf", &[container(b"stbl", &[])]),
                ],
            ),
        ],
    );
    file.extend(container(
        b"moov",
        &[mvhd(1000, 10_000), audio_trak, text_trak],
    ));
    let (directory, path) = write_temp(&file);
    let metadata = read_metadata(&path).unwrap();
    assert_eq!(metadata.chapters.len(), 1);
    assert_eq!(metadata.chapters[0].title, "Intro");
    assert_eq!(metadata.chapters[0].start_ms, 0);
    assert_eq!(metadata.chapters[0].end_ms, 10_000);
    drop(directory);
}

#[test]
fn skips_a_leading_id3_tag_before_the_mp4_boxes() {
    let mut file = b"ID3\x03\x00\x00\x00\x00\x00\x00".to_vec();
    file.extend(sample_file(vec![], Some(chpl(&[(0, "Intro")]))));
    let (directory, path) = write_temp(&file);
    let metadata = read_metadata(&path).unwrap();
    assert_eq!(metadata.chapters.len(), 1);
    assert_eq!(
        metadata.tag(&["\u{a9}nam"]).as_deref(),
        Some("The Worlds I See")
    );
    drop(directory);
}

#[test]
fn rejects_broken_atom_sizes() {
    let mut file = atom(b"ftyp", b"M4B \0\0\0\0M4B mp42");
    // A moov whose child claims to be larger than the container.
    file.extend(atom(
        b"moov",
        &[0xff, 0xff, 0xff, 0xf0, b't', b'r', b'a', b'k'],
    ));
    let (directory, path) = write_temp(&file);
    assert!(read_metadata(&path).is_err());
    drop(directory);
}

#[test]
fn reads_real_m4b_fixture_when_available() {
    let Some(path) = std::env::var_os("TING_TEST_M4B_PATH") else {
        return;
    };
    let metadata = read_metadata(std::path::Path::new(&path)).unwrap();
    let title = metadata.tag(&["\u{a9}nam"]).unwrap_or_default();
    assert!(title.contains("The Worlds I See"), "title was {title}");
    assert_eq!(metadata.chapters.len(), 16);
    assert_eq!(metadata.chapters[0].title, "Opening Credits");
    assert_eq!(metadata.chapters[0].start_ms, 0);
    assert_eq!(metadata.chapters[0].end_ms, 38_173);
    assert_eq!(metadata.chapters[15].title, "End Credits");
    assert_eq!(metadata.chapters[15].start_ms, 43_793_917);
    assert_eq!(metadata.chapters[15].end_ms, 43_862_995);
    let audio = metadata.audio.expect("audio stream");
    assert_eq!(audio.channels, 2);
    assert_eq!(audio.sample_rate, 44_100);
    assert!(
        metadata
            .pictures
            .iter()
            .any(|picture| picture.data.len() > 10_000)
    );
}
