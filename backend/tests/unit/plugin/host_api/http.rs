use super::*;

#[test]
fn http_network_permission_denies_by_default() {
    assert!(!is_network_allowed(&[], "https://example.com"));
    assert!(is_network_allowed(
        &["example.com".to_string()],
        "https://example.com/path"
    ));
    assert!(is_network_allowed(
        &["*.example.com".to_string()],
        "https://api.example.com/path"
    ));
    assert!(is_network_allowed(
        &["*".to_string()],
        "https://plugins.example.net/path"
    ));
    assert!(!is_network_allowed(
        &["example.com".to_string()],
        "https://evil.example.net/path"
    ));
    assert!(!is_network_allowed(
        &["example.com".to_string()],
        "ftp://example.com/archive"
    ));
    assert!(!is_network_allowed(
        &["example.com".to_string()],
        "https://user:secret@example.com/private"
    ));
}

#[test]
fn http_urls_are_redacted_before_logging() {
    let url =
        Url::parse("https://user:secret@[2606:4700:4700::1111]:8443/plugin?token=secret#part")
            .unwrap();
    assert_eq!(
        redacted_http_url(&url),
        "https://[2606:4700:4700::1111]:8443/plugin"
    );
    assert_eq!(
        redacted_http_url_str("not a URL?token=secret"),
        "<invalid-url>"
    );
}

#[tokio::test]
async fn http_target_validation_allows_private_addresses_with_permission() {
    let allowed_domains = vec!["*".to_string()];
    for (raw_url, expected_address) in [
        ("http://127.0.0.1/private", "127.0.0.1:80"),
        (
            "http://169.254.169.254/latest/meta-data",
            "169.254.169.254:80",
        ),
        ("http://[::1]/private", "[::1]:80"),
        ("https://[fd00::1]:8443/private", "[fd00::1]:8443"),
    ] {
        let resolved = validate_http_target(&allowed_domains, &Url::parse(raw_url).unwrap())
            .await
            .unwrap_or_else(|error| panic!("{raw_url}: {error}"));
        assert_eq!(
            resolved,
            vec![expected_address.parse::<SocketAddr>().unwrap()],
            "{raw_url}"
        );
    }

    validate_http_target(
        &allowed_domains,
        &Url::parse("https://8.8.8.8/resource").unwrap(),
    )
    .await
    .unwrap();
}

#[test]
fn http_redirects_drop_sensitive_headers_and_post_bodies() {
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret"));
    headers.insert(COOKIE, HeaderValue::from_static("session=secret"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(CONTENT_LENGTH, HeaderValue::from_static("2"));
    strip_cross_origin_fetch_headers(&mut headers);
    assert!(!headers.contains_key(AUTHORIZATION));
    assert!(!headers.contains_key(COOKIE));

    let mut method = Method::POST;
    let mut body = Some("{}".to_string());
    apply_http_redirect_semantics(StatusCode::FOUND, &mut method, &mut headers, &mut body);
    assert_eq!(method, Method::GET);
    assert!(body.is_none());
    assert!(!headers.contains_key(CONTENT_TYPE));
    assert!(!headers.contains_key(CONTENT_LENGTH));
}

#[test]
fn http_response_limit_is_enforced_without_overflow() {
    assert_eq!(
        checked_http_body_len(MAX_HTTP_RESPONSE_BYTES - 1, 1).unwrap(),
        MAX_HTTP_RESPONSE_BYTES
    );
    assert!(checked_http_body_len(MAX_HTTP_RESPONSE_BYTES, 1).is_err());
    assert!(checked_http_body_len(usize::MAX, 1).is_err());
}

#[tokio::test]
async fn http_client_pins_dns_and_does_not_follow_redirects_automatically() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 1024];
        let _ = socket.read(&mut request).await.unwrap();
        socket
            .write_all(
                b"HTTP/1.1 302 Found\r\nLocation: http://example.com/next\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await
            .unwrap();
    });

    let url = Url::parse(&format!(
        "http://plugin-fetch.test:{}/start?token=secret",
        address.port()
    ))
    .unwrap();
    let client = build_http_client(&url, &[address]).unwrap();
    let response = client.get(url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(response.url().host_str(), Some("plugin-fetch.test"));
    server.await.unwrap();
}

#[tokio::test]
async fn shared_http_preserves_status_and_rejects_undeclared_redirect_target() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        for response in [
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 4\r\nConnection: close\r\n\r\nnope".as_slice(),
            b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.2:1234/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_slice(),
            b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.2:1234/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_slice(),
        ] {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 512];
            let _ = socket.read(&mut request).await.unwrap();
            socket.write_all(response).await.unwrap();
        }
    });
    let allowed = vec!["127.0.0.1".to_string()];
    let url = format!("http://127.0.0.1:{port}/data");
    let result = request(&url, None, &allowed).await.unwrap();
    assert_eq!(result.status, 404);
    assert_eq!(result.body, b"nope");
    let error = request(&url, None, &allowed).await.unwrap_err();
    assert!(
        error.to_string().contains("Network access denied"),
        "{error}"
    );
    assert!(
        media_response(&url, HeaderMap::new(), &allowed)
            .await
            .is_err()
    );
    server.await.unwrap();
}

#[tokio::test]
async fn media_stream_exceeds_json_limit_and_disconnect_releases_upstream() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 1024];
        let _ = socket.read(&mut request).await.unwrap();
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 67108864\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let chunk = [b'x'; 8192];
        let mut sent = 0;
        while sent < 64 * 1024 * 1024 {
            if socket.write_all(&chunk).await.is_err() {
                break;
            }
            sent += chunk.len();
        }
        sent
    });
    let response = media_response(
        &format!("http://{address}/audio"),
        HeaderMap::new(),
        &["127.0.0.1".into()],
    )
    .await
    .unwrap();
    let mut stream = response.bytes_stream();
    let mut received = 0;
    while received <= MAX_HTTP_RESPONSE_BYTES {
        received += stream.next().await.unwrap().unwrap().len();
    }
    drop(stream);
    let sent = tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
    assert!(received > MAX_HTTP_RESPONSE_BYTES);
    assert!(
        sent < 64 * 1024 * 1024,
        "Dropping the reader must cancel the upstream response"
    );
}

#[tokio::test]
async fn media_cross_origin_redirect_strips_credentials_but_preserves_range() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_address = target.local_addr().unwrap();
    let upstream = tokio::spawn(async move {
        let (mut socket, _) = target.accept().await.unwrap();
        let mut bytes = [0; 4096];
        let length = socket.read(&mut bytes).await.unwrap();
        let request = String::from_utf8_lossy(&bytes[..length]).to_ascii_lowercase();
        assert!(!request.contains("authorization:"));
        assert!(!request.contains("cookie:"));
        assert!(request.contains("range: bytes=3-5"));
        socket.write_all(b"HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nContent-Range: bytes 3-5/9\r\nConnection: close\r\n\r\ndef").await.unwrap();
    });
    let origin = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin_address = origin.local_addr().unwrap();
    let redirect = tokio::spawn(async move {
        let (mut socket, _) = origin.accept().await.unwrap();
        let mut bytes = [0; 4096];
        let _ = socket.read(&mut bytes).await.unwrap();
        let reply = format!(
            "HTTP/1.1 302 Found\r\nLocation: http://{target_address}/audio\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        socket.write_all(reply.as_bytes()).await.unwrap();
    });
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_static("Bearer source-only"),
    );
    headers.insert(COOKIE, HeaderValue::from_static("source=only"));
    headers.insert("Range", HeaderValue::from_static("bytes=3-5"));
    let response = media_response(
        &format!("http://{origin_address}/audio"),
        headers,
        &["127.0.0.1".into()],
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.bytes().await.unwrap().as_ref(), b"def");
    upstream.await.unwrap();
    redirect.await.unwrap();
}
