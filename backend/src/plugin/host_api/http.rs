//! Host HTTP transport and request limits shared by all plugin runtimes.

use anyhow::{Context, Result};
use futures::StreamExt;
use reqwest::header::{
    AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, COOKIE, HeaderMap, HeaderName, HeaderValue,
    LOCATION, PROXY_AUTHORIZATION, TRANSFER_ENCODING,
};
use reqwest::{Method, StatusCode, Url};
use serde_json::Value;
use std::net::SocketAddr;
use std::time::Duration;
use tracing::{debug, info, warn};

const MAX_HTTP_REDIRECTS: usize = 5;
const MAX_HTTP_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_HTTP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
struct HttpOptions {
    method: Method,
    headers: HeaderMap,
    body: Option<String>,
    timeout: Duration,
}

impl Default for HttpOptions {
    fn default() -> Self {
        Self {
            method: Method::GET,
            headers: HeaderMap::new(),
            body: None,
            timeout: DEFAULT_HTTP_TIMEOUT,
        }
    }
}

fn build_http_client(url: &Url, resolved: &[SocketAddr]) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy();
    if matches!(url.host(), Some(url::Host::Domain(_))) {
        let host = url
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("Plugin fetch URL is missing a host"))?;
        builder = builder.resolve_to_addrs(host, resolved);
    }
    builder
        .build()
        .context("Failed to build plugin fetch client")
}

fn parse_http_options(options: Option<&Value>) -> Result<HttpOptions> {
    let Some(options) = options else {
        return Ok(HttpOptions::default());
    };

    let mut parsed = HttpOptions::default();
    if let Some(method) = options.get("method").and_then(Value::as_str) {
        parsed.method = match method.to_ascii_uppercase().as_str() {
            "POST" => Method::POST,
            "PUT" => Method::PUT,
            "DELETE" => Method::DELETE,
            "GET" => Method::GET,
            "HEAD" => Method::HEAD,
            "PATCH" => Method::PATCH,
            "OPTIONS" => Method::OPTIONS,
            _ => anyhow::bail!("Unsupported HTTP method"),
        };
    }

    if let Some(headers) = options.get("headers").and_then(Value::as_object) {
        for (name, value) in headers {
            let Some(value) = value.as_str() else {
                continue;
            };
            let name = HeaderName::from_bytes(name.as_bytes())
                .context("Plugin fetch contains an invalid header name")?;
            let value = HeaderValue::from_str(value)
                .context("Plugin fetch contains an invalid header value")?;
            parsed.headers.insert(name, value);
        }
    }

    parsed.body = options
        .get("body")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(timeout_ms) = options.get("timeout_ms").and_then(Value::as_u64) {
        parsed.timeout = Duration::from_millis(timeout_ms.clamp(1, 30_000));
    }

    Ok(parsed)
}

fn redacted_http_url(url: &Url) -> String {
    let mut redacted = url.clone();
    if redacted.set_username("").is_err() || redacted.set_password(None).is_err() {
        return "<invalid-url>".to_string();
    }
    redacted.set_query(None);
    redacted.set_fragment(None);
    redacted.to_string()
}

fn redacted_http_url_str(url: &str) -> String {
    Url::parse(url)
        .map(|parsed| redacted_http_url(&parsed))
        .unwrap_or_else(|_| "<invalid-url>".to_string())
}

async fn validate_http_target(allowed_domains: &[String], url: &Url) -> Result<Vec<SocketAddr>> {
    let redacted_url = redacted_http_url(url);
    if !matches!(url.scheme(), "http" | "https") {
        anyhow::bail!("Network access denied: unsupported URL scheme for {redacted_url}");
    }
    if !url.username().is_empty() || url.password().is_some() {
        anyhow::bail!("Network access denied: URL credentials are not allowed");
    }
    if !is_network_allowed(allowed_domains, url.as_str()) {
        anyhow::bail!("Network access denied for {redacted_url}");
    }

    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("Network access denied: URL is missing a host"))?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let port = url
        .port_or_known_default()
        .ok_or_else(|| anyhow::anyhow!("Network access denied: URL uses an unknown port"))?;
    if port == 0 {
        anyhow::bail!("Network access denied: port 0 is not allowed");
    }

    match url.host() {
        Some(url::Host::Ipv4(ip)) => return Ok(vec![SocketAddr::new(ip.into(), port)]),
        Some(url::Host::Ipv6(ip)) => return Ok(vec![SocketAddr::new(ip.into(), port)]),
        _ => {}
    }

    let resolved = tokio::net::lookup_host((host.as_str(), port))
        .await
        .with_context(|| format!("Failed to resolve plugin fetch host {host}"))?
        .collect::<Vec<_>>();
    if resolved.is_empty() {
        anyhow::bail!("Network access denied: host did not resolve to an address");
    }

    Ok(resolved)
}

fn is_followable_http_redirect(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::SEE_OTHER
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT
    )
}

fn same_url_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str().map(str::to_ascii_lowercase)
            == right.host_str().map(str::to_ascii_lowercase)
        && left.port_or_known_default() == right.port_or_known_default()
}

fn strip_cross_origin_fetch_headers(headers: &mut HeaderMap) {
    headers.remove(AUTHORIZATION);
    headers.remove(COOKIE);
    headers.remove(PROXY_AUTHORIZATION);
}

fn apply_http_redirect_semantics(
    status: StatusCode,
    method: &mut Method,
    headers: &mut HeaderMap,
    body: &mut Option<String>,
) {
    let switch_to_get = status == StatusCode::SEE_OTHER
        || (matches!(status, StatusCode::MOVED_PERMANENTLY | StatusCode::FOUND)
            && *method == Method::POST);
    if switch_to_get {
        *method = Method::GET;
        *body = None;
        headers.remove(CONTENT_LENGTH);
        headers.remove(CONTENT_TYPE);
        headers.remove(TRANSFER_ENCODING);
    }
}

fn checked_http_body_len(current: usize, additional: usize) -> Result<usize> {
    let next = current
        .checked_add(additional)
        .ok_or_else(|| anyhow::anyhow!("Plugin fetch response is too large"))?;
    if next > MAX_HTTP_RESPONSE_BYTES {
        anyhow::bail!(
            "Plugin fetch response exceeds the {} MiB limit",
            MAX_HTTP_RESPONSE_BYTES / (1024 * 1024)
        );
    }
    Ok(next)
}

#[derive(Debug, PartialEq)]
pub(crate) struct HttpResponse {
    pub status: u16,
    pub headers: std::collections::BTreeMap<String, String>,
    pub body: Vec<u8>,
}

pub(crate) async fn request(
    raw_url: &str,
    options: Option<&Value>,
    allowed_domains: &[String],
) -> Result<HttpResponse> {
    let response = send_response(
        raw_url,
        parse_http_options(options)?,
        allowed_domains,
        false,
    )
    .await?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_HTTP_RESPONSE_BYTES as u64)
    {
        anyhow::bail!("Plugin fetch response exceeds the 8 MiB limit");
    }
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.to_string(), value.to_string()))
        })
        .collect();
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| {
            anyhow::anyhow!("Plugin fetch response read failed: {}", error.without_url())
        })?;
        checked_http_body_len(body.len(), chunk.len())?;
        body.extend_from_slice(&chunk);
    }
    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

/// Media stays streamed; the same domain, DNS pinning and redirect policy
/// applies, without buffering it in the plugin's bounded JSON HTTP response.
pub(crate) async fn media_response(
    raw_url: &str,
    headers: HeaderMap,
    allowed_domains: &[String],
) -> Result<reqwest::Response> {
    send_response(
        raw_url,
        HttpOptions {
            headers,
            ..Default::default()
        },
        allowed_domains,
        true,
    )
    .await
}

async fn send_response(
    raw_url: &str,
    mut options: HttpOptions,
    allowed_domains: &[String],
    streaming: bool,
) -> Result<reqwest::Response> {
    let mut current_url = Url::parse(raw_url).map_err(|error| {
        anyhow::anyhow!(
            "Invalid plugin fetch URL {}: {error}",
            redacted_http_url_str(raw_url)
        )
    })?;
    let deadline = tokio::time::Instant::now() + options.timeout;
    let mut redirect_count = 0_usize;

    loop {
        let redacted_url = redacted_http_url(&current_url);
        let remaining = deadline
            .checked_duration_since(tokio::time::Instant::now())
            .ok_or_else(|| anyhow::anyhow!("Plugin fetch timed out"))?;
        let resolved = match tokio::time::timeout(
            remaining,
            validate_http_target(allowed_domains, &current_url),
        )
        .await
        {
            Ok(Ok(resolved)) => resolved,
            Ok(Err(error)) => {
                warn!(
                    url = %redacted_url,
                    error = %error,
                    message_key = "plugin.fetch.target_rejected",
                    "Plugin fetch target rejected"
                );
                return Err(error);
            }
            Err(_) => return Err(anyhow::anyhow!("Plugin fetch timed out")),
        };

        let remaining = deadline
            .checked_duration_since(tokio::time::Instant::now())
            .ok_or_else(|| anyhow::anyhow!("Plugin fetch timed out"))?;
        info!(url = %redacted_url, "Plugin fetch request started");
        let client = build_http_client(&current_url, &resolved)?;
        let mut request = client
            .request(options.method.clone(), current_url.clone())
            .headers(options.headers.clone());
        if !streaming {
            request = request.timeout(remaining);
        }
        if let Some(body) = &options.body {
            request = request.body(body.clone());
        }

        let response = match tokio::time::timeout(remaining, request.send())
            .await
            .map_err(|_| anyhow::anyhow!("Plugin fetch timed out"))?
        {
            Ok(response) => response,
            Err(error) => {
                let error = error.without_url().to_string();
                debug!(
                    url = %redacted_url,
                    error = %error,
                    message_key = "plugin.fetch.request_failed",
                    message_params = %serde_json::json!({ "error": &error }),
                    "Plugin fetch request failed"
                );
                return Err(anyhow::anyhow!("Plugin fetch request failed: {error}"));
            }
        };

        let remote = response.remote_addr().ok_or_else(|| {
            anyhow::anyhow!("Plugin fetch response did not expose its remote address")
        })?;
        if !resolved.iter().any(|address| address.ip() == remote.ip()) {
            warn!(
                url = %redacted_url,
                "Plugin fetch connected to an unexpected address"
            );
            anyhow::bail!("Network access denied: connected to an unexpected address");
        }

        let status = response.status();
        if is_followable_http_redirect(status) {
            if redirect_count >= MAX_HTTP_REDIRECTS {
                anyhow::bail!(
                    "Plugin fetch exceeded the {} redirect limit",
                    MAX_HTTP_REDIRECTS
                );
            }
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| anyhow::anyhow!("Plugin fetch redirect omitted Location"))?;
            let next_url = current_url
                .join(location)
                .map_err(|_| anyhow::anyhow!("Plugin fetch returned an invalid redirect target"))?;
            if !same_url_origin(&current_url, &next_url) {
                strip_cross_origin_fetch_headers(&mut options.headers);
            }
            apply_http_redirect_semantics(
                status,
                &mut options.method,
                &mut options.headers,
                &mut options.body,
            );
            info!(
                from = %redacted_url,
                to = %redacted_http_url(&next_url),
                status = %status,
                "Plugin fetch following redirect"
            );
            current_url = next_url;
            redirect_count += 1;
            continue;
        }

        return Ok(response);
    }
}

fn is_network_allowed(allowed_domains: &[String], url: &str) -> bool {
    if allowed_domains.is_empty() {
        return false;
    }

    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return false;
    }
    let Some(host) = parsed.host_str() else {
        return false;
    };

    allowed_domains
        .iter()
        .any(|domain| domain_matches(host, domain))
}

fn domain_matches(host: &str, pattern: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let pattern = pattern.to_ascii_lowercase();

    if pattern == "*" {
        true
    } else if let Some(base) = pattern.strip_prefix("*.") {
        host == base || host.ends_with(&format!(".{}", base))
    } else {
        host == pattern
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/plugin/host_api/http.rs"]
mod tests;
