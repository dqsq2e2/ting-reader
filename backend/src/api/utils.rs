//! API utility helpers

use crate::auth::AuthUser;
use crate::core::error::{Result, TingError};
use axum::http::{header, HeaderMap};
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct RequestInfo {
    pub real_ip: String,
    pub user_agent: String,
    pub device: String,
}

/// Require admin role, returning PermissionDenied if not admin
pub fn require_admin(user: &AuthUser) -> Result<()> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied(
            "Admin access required".to_string(),
        ));
    }
    Ok(())
}

pub fn request_info_from_headers(
    headers: &HeaderMap,
    peer_addr: Option<SocketAddr>,
) -> RequestInfo {
    let user_agent = header_value(headers, header::USER_AGENT.as_str()).unwrap_or("unknown");
    let device = header_value(headers, "x-ting-device")
        .map(truncate_header)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| summarize_device(user_agent));
    RequestInfo {
        real_ip: extract_real_ip(headers, peer_addr).unwrap_or_else(|| "unknown".to_string()),
        device,
        user_agent: truncate_header(user_agent),
    }
}

fn extract_real_ip(headers: &HeaderMap, peer_addr: Option<SocketAddr>) -> Option<String> {
    header_value(headers, "x-forwarded-for")
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(truncate_header)
        .or_else(|| {
            header_value(headers, "x-real-ip")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(truncate_header)
        })
        .or_else(|| {
            header_value(headers, "forwarded")
                .and_then(|value| {
                    value.split(';').find_map(|part| {
                        let trimmed = part.trim();
                        trimmed
                            .strip_prefix("for=")
                            .map(|ip| ip.trim_matches('"').trim_matches('[').trim_matches(']'))
                    })
                })
                .filter(|value| !value.is_empty())
                .map(truncate_header)
        })
        .or_else(|| peer_addr.map(|addr| addr.ip().to_string()))
}

fn header_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn truncate_header(value: &str) -> String {
    const MAX_HEADER_LEN: usize = 256;
    let trimmed = value.trim();
    if trimmed.chars().count() <= MAX_HEADER_LEN {
        return trimmed.to_string();
    }
    trimmed.chars().take(MAX_HEADER_LEN).collect()
}

fn summarize_device(user_agent: &str) -> String {
    let ua = user_agent.to_lowercase();
    let os = if ua.contains("windows") {
        "Windows"
    } else if ua.contains("android") {
        "Android"
    } else if ua.contains("iphone") {
        "iPhone"
    } else if ua.contains("ipad") {
        "iPad"
    } else if ua.contains("mac os") || ua.contains("macintosh") {
        "macOS"
    } else if ua.contains("linux") {
        "Linux"
    } else {
        "Unknown OS"
    };

    let browser = if ua.contains("edg/") || ua.contains("edge/") {
        "Edge"
    } else if ua.contains("firefox/") {
        "Firefox"
    } else if ua.contains("tingreaderflutter/") {
        "Flutter Client"
    } else if ua.contains("chrome/") || ua.contains("chromium/") {
        "Chrome"
    } else if ua.contains("safari/") {
        "Safari"
    } else {
        "Unknown Browser"
    };

    format!("{} / {}", os, browser)
}
