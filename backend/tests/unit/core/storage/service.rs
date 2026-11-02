use super::*;

#[test]
fn media_urls_join_encoded_segments_once_and_keep_credentials_inside_library() {
    let mut library = metadata_library();
    library.username = Some("user".into());
    library.password = Some("p@ss:word".into());
    for path in ["Book%20One/chapter%201.mp3", "Book One/chapter 1.mp3"] {
        let url = StorageService::media_url(&library, path).unwrap();
        assert_eq!(
            url.as_str(),
            "https://example.test/dav/books/Book%20One/chapter%201.mp3"
        );
        let authenticated =
            Url::parse(&StorageService::authenticated_media_url(&library, path, None).unwrap())
                .unwrap();
        assert_eq!(authenticated.username(), "user");
        assert_eq!(authenticated.host_str(), Some("example.test"));
    }
    for path in [
        "https://outside.test/dav/books/a.mp3",
        "https://example.test/dav/other/a.mp3",
        "../outside.mp3",
        "%2e%2e/outside.mp3",
    ] {
        assert!(
            StorageService::authenticated_media_url(&library, path, None).is_err(),
            "{path}"
        );
    }
    assert_eq!(
        StorageService::media_url(&library, "file%2Fpart.mp3")
            .unwrap()
            .path(),
        "/dav/books/file%2Fpart.mp3"
    );
}
use axum::{Router, body::Body, http::StatusCode, response::IntoResponse, routing::get};
use tokio::io::AsyncReadExt;

fn metadata_library() -> Library {
    Library {
        id: "source".into(),
        name: "source".into(),
        library_type: "webdav".into(),
        url: "https://example.test/dav/".into(),
        username: None,
        password: None,
        root_path: "/books".into(),
        last_scanned_at: None,
        created_at: String::new(),
        scraper_config: None,
    }
}

#[test]
fn metadata_probe_input_enforces_webdav_library_boundary() {
    let service = StorageService::new();
    let library = metadata_library();
    for path in [
        "https://example.test/dav/books/book/001.m4a",
        "book/001.m4a",
        "book/Chapter%20One.wma",
    ] {
        assert!(
            service
                .webdav_metadata_input(&library, path, &[0; 32])
                .is_ok()
        );
    }
    for path in [
        "https://other.test/dav/books/book/001.m4a",
        "https://example.test/dav/outside/001.m4a",
        "file:///outside/001.m4a",
    ] {
        assert!(
            service
                .webdav_metadata_input(&library, path, &[0; 32])
                .is_err()
        );
    }
}

#[test]
fn sidecar_upload_keeps_encoded_book_names_and_library_root() {
    let library = metadata_library();
    for book in [
        "https://example.test/dav/books/Book%20%E4%B8%80%26Two",
        "https://example.test/dav/books/Book%20%E4%B8%80%26Two/",
    ] {
        assert_eq!(
            webdav_sidecar_url(&library, book, "metadata.json")
                .unwrap()
                .as_str(),
            "https://example.test/dav/books/Book%20%E4%B8%80%26Two/metadata.json"
        );
    }
}

#[test]
fn sidecar_upload_rejects_outside_folders_and_unapproved_filenames() {
    let library = metadata_library();
    for book in [
        "https://other.test/dav/books/book",
        "https://example.test/dav/books-other/book",
        "https://example.test/dav/books/../outside",
        "https://user:password@example.test/dav/books/book",
        "https://example.test/dav/books/book?redirect=other",
    ] {
        assert!(webdav_sidecar_url(&library, book, "metadata.json").is_err());
    }
    assert!(
        webdav_sidecar_url(
            &library,
            "https://example.test/dav/books/book",
            "../metadata.json"
        )
        .is_err()
    );
    assert!(
        webdav_sidecar_url(&library, "https://example.test/dav/books/book", "001.mp3").is_err()
    );
}

async fn source(
    status: StatusCode,
    content_range: Option<&'static str>,
    data: &'static [u8],
    chunked: bool,
) -> (String, tokio::task::JoinHandle<()>) {
    let router = Router::new().route(
        "/chapter",
        get(move |headers: axum::http::HeaderMap| async move {
            assert_eq!(headers["accept-encoding"], "identity");
            assert_eq!(headers["range"], "bytes=3-5");
            let body = if chunked {
                Body::from_stream(futures::stream::once(async {
                    Ok::<_, std::io::Error>(bytes::Bytes::from_static(data))
                }))
            } else {
                Body::from(data)
            };
            let mut response = (status, body).into_response();
            if let Some(value) = content_range {
                response
                    .headers_mut()
                    .insert("content-range", value.parse().unwrap());
            }
            response
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (format!("http://{address}/chapter"), task)
}

#[tokio::test]
async fn remote_range_reader_handles_partial_and_ignored_ranges() {
    for (status, content_range, body, chunked, expected_size) in [
        (
            StatusCode::PARTIAL_CONTENT,
            Some("bytes 3-5/9"),
            b"def".as_slice(),
            false,
            9,
        ),
        (StatusCode::OK, None, b"abcdefghi".as_slice(), false, 9),
        (StatusCode::OK, None, b"abcdefghi".as_slice(), true, 0),
    ] {
        let (url, task) = source(status, content_range, body, chunked).await;
        let service = StorageService::new();
        for webdav in [false, true] {
            let (mut reader, size) = if webdav {
                let library = Library {
                    id: "source".into(),
                    name: "source".into(),
                    library_type: "webdav".into(),
                    url: url.clone(),
                    username: None,
                    password: None,
                    root_path: String::new(),
                    last_scanned_at: None,
                    created_at: String::new(),
                    scraper_config: None,
                };
                service
                    .get_webdav_reader(&library, &url, Some((3, 6)), &[0; 32])
                    .await
                    .unwrap()
            } else {
                service
                    .get_http_reader(&url, Some((3, 6)), None, None)
                    .await
                    .unwrap()
            };
            assert_eq!(size, expected_size);
            let mut data = Vec::new();
            reader.read_to_end(&mut data).await.unwrap();
            assert_eq!(data, b"def");
        }
        task.abort();
    }
}

#[tokio::test]
async fn source_media_preserves_headers_and_range_and_enforces_network_permissions() {
    use axum::{Router, response::IntoResponse, routing::get};
    let router = Router::new().route(
        "/chapter",
        get(|headers: axum::http::HeaderMap| async move {
            assert_eq!(headers["referer"], "https://source.example/");
            assert_eq!(headers["authorization"], "Bearer source-only");
            assert_eq!(headers["range"], "bytes=3-5");
            assert_eq!(headers["accept-encoding"], "identity");
            (
                StatusCode::PARTIAL_CONTENT,
                [("content-range", "bytes 3-5/9")],
                "def",
            )
                .into_response()
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let headers = std::collections::BTreeMap::from([
        ("Referer".into(), "https://source.example/".into()),
        ("Authorization".into(), "Bearer source-only".into()),
    ]);
    let service = StorageService::new();
    let url = format!("http://{address}/chapter");
    assert!(
        service
            .get_http_reader(&url, Some((3, 6)), Some(&headers), Some(&[]))
            .await
            .is_err()
    );
    let (mut reader, total) = service
        .get_http_reader(
            &url,
            Some((3, 6)),
            Some(&headers),
            Some(&["127.0.0.1".into()]),
        )
        .await
        .unwrap();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await.unwrap();
    assert_eq!(total, 9);
    assert_eq!(bytes, b"def");
    server.abort();
}

#[tokio::test]
async fn remote_range_reader_rejects_wrong_offsets_and_short_ignored_ranges() {
    for (status, content_range, data) in [
        (
            StatusCode::PARTIAL_CONTENT,
            Some("bytes 0-2/9"),
            b"abc".as_slice(),
        ),
        (StatusCode::PARTIAL_CONTENT, None, b"def".as_slice()),
        (
            StatusCode::PARTIAL_CONTENT,
            Some("bytes 3-5/5"),
            b"def".as_slice(),
        ),
        (StatusCode::OK, None, b"ab".as_slice()),
    ] {
        let (url, task) = source(status, content_range, data, false).await;
        assert!(
            StorageService::new()
                .get_http_reader(&url, Some((3, 6)), None, None)
                .await
                .is_err()
        );
        task.abort();
    }
}

#[tokio::test]
async fn remote_range_at_eof_is_an_empty_remainder() {
    let (url, task) = source(
        StatusCode::RANGE_NOT_SATISFIABLE,
        Some("bytes */3"),
        b"",
        false,
    )
    .await;
    let (mut reader, size) = StorageService::new()
        .get_http_reader(&url, Some((3, 6)), None, None)
        .await
        .unwrap();
    assert_eq!(size, 3);
    let mut data = Vec::new();
    reader.read_to_end(&mut data).await.unwrap();
    assert!(data.is_empty());
    task.abort();
}

#[test]
fn chapters_must_stay_in_their_own_book_folder() {
    let library = Library {
        id: "lib".into(),
        name: "lib".into(),
        library_type: "webdav".into(),
        url: "https://example.test/dav".into(),
        root_path: "/books".into(),
        username: None,
        password: None,
        last_scanned_at: None,
        created_at: String::new(),
        scraper_config: None,
    };
    let book = "https://example.test/dav/books/Book%20One";
    assert!(
        chapter_url(
            &library,
            book,
            "https://example.test/dav/books/Book%20One/part/001.mp3"
        )
        .is_ok()
    );
    for chapter in [
        "https://example.test/dav/books/Book%20Other/001.mp3",
        "https://example.test/dav/books/Book%20One/../Other/001.mp3",
        "https://other.test/dav/books/Book%20One/001.mp3",
        "https://example.test/dav/books/Book%20One/",
    ] {
        assert!(chapter_url(&library, book, chapter).is_err());
    }
}
