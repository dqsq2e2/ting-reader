use base64::Engine;
use sha2::{Digest, Sha256};

pub const DEFAULT_PLUGIN_ROUTE_SIGNATURE_TTL_SECONDS: u64 = 60 * 60;
pub const MAX_PLUGIN_ROUTE_SIGNATURE_TTL_SECONDS: u64 = 30 * 24 * 60 * 60;
pub const DEFAULT_MEDIA_SIGNATURE_TTL_SECONDS: u64 = 30 * 24 * 60 * 60;
pub const MAX_MEDIA_SIGNATURE_TTL_SECONDS: u64 = 365 * 24 * 60 * 60;

pub fn signature_expires_from_ttl(
    ttl_seconds: Option<u64>,
    default_ttl_seconds: u64,
    max_ttl_seconds: u64,
) -> i64 {
    match ttl_seconds {
        Some(0) => 0,
        Some(ttl) => chrono::Utc::now().timestamp() + ttl.clamp(1, max_ttl_seconds) as i64,
        None => chrono::Utc::now().timestamp() + default_ttl_seconds as i64,
    }
}

pub fn signature_has_expired(expires: i64) -> bool {
    expires > 0 && expires < chrono::Utc::now().timestamp()
}

pub fn normalize_plugin_route_sign_path(path: &str) -> String {
    let trimmed = path.trim().split('?').next().unwrap_or("").trim();
    let normalized = if trimmed.is_empty() {
        "/".to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    };

    [
        "/api/v1/public/plugin-routes",
        "/api/public/plugin-routes",
        "/api/v1/plugin-routes",
        "/api/plugin-routes",
    ]
    .iter()
    .find_map(|prefix| normalized.strip_prefix(prefix))
    .map(|path| {
        if path.is_empty() {
            "/".to_string()
        } else {
            path.to_string()
        }
    })
    .unwrap_or(normalized)
}

pub fn sign_plugin_route_request(
    signing_key: &[u8; 32],
    method: &str,
    route_path: &str,
    expires: i64,
    user_id: Option<&str>,
) -> String {
    let payload = plugin_route_signature_payload(method, route_path, expires, user_id);
    hmac_sha256_base64_url(signing_key, payload.as_bytes())
}

pub fn plugin_route_signature_payload(
    method: &str,
    route_path: &str,
    expires: i64,
    user_id: Option<&str>,
) -> String {
    let base = format!("{}\n{}\n{}", method.to_uppercase(), route_path, expires);
    match user_id {
        Some(user_id) => format!("{}\nuser:{}", base, user_id),
        None => base,
    }
}

pub fn sign_media_stream_request(
    signing_key: &[u8; 32],
    chapter_id: &str,
    expires: i64,
    user_id: &str,
    transcode: Option<&str>,
    seek: Option<&str>,
    download: bool,
) -> String {
    let payload =
        media_stream_signature_payload(chapter_id, expires, user_id, transcode, seek, download);
    hmac_sha256_base64_url(signing_key, payload.as_bytes())
}

pub fn media_stream_signature_payload(
    chapter_id: &str,
    expires: i64,
    user_id: &str,
    transcode: Option<&str>,
    seek: Option<&str>,
    download: bool,
) -> String {
    format!(
        "media-stream\nchapter:{}\nexpires:{}\nuser:{}\ntranscode:{}\nseek:{}\ndownload:{}",
        chapter_id,
        expires,
        user_id,
        transcode.unwrap_or(""),
        seek.unwrap_or(""),
        if download { "1" } else { "0" }
    )
}

pub fn hmac_sha256_base64_url(key: &[u8], payload: &[u8]) -> String {
    let signature = hmac_sha256(key, payload);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(signature)
}

pub fn hmac_sha256(key: &[u8], payload: &[u8]) -> [u8; 32] {
    const BLOCK_SIZE: usize = 64;

    let mut key_block = [0_u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let digest = Sha256::digest(key);
        key_block[..digest.len()].copy_from_slice(&digest);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut inner_pad = [0x36_u8; BLOCK_SIZE];
    let mut outer_pad = [0x5c_u8; BLOCK_SIZE];
    for index in 0..BLOCK_SIZE {
        inner_pad[index] ^= key_block[index];
        outer_pad[index] ^= key_block[index];
    }

    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(payload);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_hash);
    let signature = outer.finalize();

    let mut out = [0_u8; 32];
    out.copy_from_slice(&signature);
    out
}

pub fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    left.iter()
        .zip(right.iter())
        .fold(0_u8, |diff, (left, right)| diff | (*left ^ *right))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_signature_expiry_never_expires() {
        assert!(!signature_has_expired(0));
    }

    #[test]
    fn past_positive_signature_expiry_expires() {
        assert!(signature_has_expired(chrono::Utc::now().timestamp() - 60));
    }
}
