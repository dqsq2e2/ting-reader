//! API utility helpers

use crate::auth::AuthUser;
use crate::core::error::{Result, TingError};
use axum::http::{header, HeaderMap};
use std::net::{IpAddr, SocketAddr};

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
        .and_then(normalize_ip)
        .or_else(|| header_value(headers, "x-real-ip").and_then(normalize_ip))
        .or_else(|| {
            header_value(headers, "forwarded")
                .and_then(|value| {
                    value.split(';').find_map(|part| {
                        let trimmed = part.trim();
                        trimmed.strip_prefix("for=").map(|ip| ip.trim_matches('"'))
                    })
                })
                .and_then(normalize_ip)
        })
        .or_else(|| peer_addr.map(|addr| normalize_ip_addr(addr.ip()).to_string()))
}

fn normalize_ip(value: &str) -> Option<String> {
    let value = value.trim().trim_matches('"');
    let value = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(value);
    if value.is_empty() {
        return None;
    }
    if let Ok(ip) = value.parse::<IpAddr>() {
        return Some(normalize_ip_addr(ip).to_string());
    }
    if let Ok(addr) = value.parse::<SocketAddr>() {
        return Some(normalize_ip_addr(addr.ip()).to_string());
    }
    Some(truncate_header(value))
}

fn normalize_ip_addr(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(ipv6) => ipv6
            .to_ipv4_mapped()
            .map(IpAddr::V4)
            .unwrap_or(IpAddr::V6(ipv6)),
        ip => ip,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_ipv4_mapped_ipv6_from_forwarded_header() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "::ffff:192.168.1.17".parse().unwrap());

        assert_eq!(
            extract_real_ip(&headers, None).as_deref(),
            Some("192.168.1.17")
        );
    }

    #[test]
    fn normalizes_ipv4_mapped_ipv6_peer_address() {
        let peer = "[::ffff:192.168.1.17]:3000".parse().unwrap();
        assert_eq!(
            extract_real_ip(&HeaderMap::new(), Some(peer)).as_deref(),
            Some("192.168.1.17")
        );
    }
}
