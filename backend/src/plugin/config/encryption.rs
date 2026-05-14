//! AES-256-GCM encryption for sensitive plugin configuration values

use crate::core::error::{Result, TingError};
use serde_json::Value;

/// Encrypt a value using AES-256-GCM
pub fn encrypt_value(encryption_key: &[u8; 32], value: &str) -> Result<String> {
    use aes_gcm::{
        aead::{Aead, KeyInit, OsRng},
        Aes256Gcm, Nonce,
    };
    use base64::{engine::general_purpose, Engine as _};

    let cipher = Aes256Gcm::new(encryption_key.into());

    let mut nonce_bytes = [0u8; 12];
    use aes_gcm::aead::rand_core::RngCore;
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, value.as_bytes())
        .map_err(|e| TingError::ConfigError(format!("Encryption failed: {}", e)))?;

    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);

    Ok(general_purpose::STANDARD.encode(&combined))
}

/// Decrypt a value using AES-256-GCM
pub fn decrypt_value(encryption_key: &[u8; 32], encrypted: &str) -> Result<String> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };
    use base64::{engine::general_purpose, Engine as _};

    let cipher = Aes256Gcm::new(encryption_key.into());

    let combined = general_purpose::STANDARD
        .decode(encrypted)
        .map_err(|e| TingError::ConfigError(format!("Invalid encrypted data: {}", e)))?;

    if combined.len() < 12 {
        return Err(TingError::ConfigError("Invalid encrypted data length".to_string()));
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| TingError::ConfigError(format!("Decryption failed: {}", e)))?;

    String::from_utf8(plaintext)
        .map_err(|e| TingError::ConfigError(format!("Invalid UTF-8 in decrypted data: {}", e)))
}

/// Extract encrypted field paths from schema (fields with "x-encrypted": true)
pub fn extract_encrypted_fields(schema: &Value) -> Vec<String> {
    let mut encrypted_fields = Vec::new();
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        for (field_name, field_schema) in properties {
            if let Some(true) = field_schema.get("x-encrypted").and_then(|v| v.as_bool()) {
                encrypted_fields.push(field_name.clone());
            }
        }
    }
    encrypted_fields
}

/// Encrypt sensitive fields in a configuration value
pub fn encrypt_sensitive_fields(
    encryption_key: &[u8; 32],
    config: &Value,
    encrypted_fields: &[String],
) -> Result<Value> {
    if encrypted_fields.is_empty() {
        return Ok(config.clone());
    }

    let mut encrypted_config = config.clone();
    if let Some(obj) = encrypted_config.as_object_mut() {
        for field_name in encrypted_fields {
            if let Some(field_value) = obj.get(field_name) {
                let value_str = if field_value.is_string() {
                    field_value.as_str().unwrap().to_string()
                } else {
                    field_value.to_string()
                };
                let encrypted = encrypt_value(encryption_key, &value_str)?;
                obj.insert(field_name.clone(), Value::String(format!("encrypted:{}", encrypted)));
            }
        }
    }

    Ok(encrypted_config)
}

/// Decrypt sensitive fields in a configuration value
pub fn decrypt_sensitive_fields(
    encryption_key: &[u8; 32],
    config: &Value,
    encrypted_fields: &[String],
) -> Result<Value> {
    if encrypted_fields.is_empty() {
        return Ok(config.clone());
    }

    let mut decrypted_config = config.clone();
    if let Some(obj) = decrypted_config.as_object_mut() {
        for field_name in encrypted_fields {
            if let Some(field_value) = obj.get(field_name) {
                if let Some(encrypted_str) = field_value.as_str() {
                    if let Some(encrypted_data) = encrypted_str.strip_prefix("encrypted:") {
                        let decrypted = decrypt_value(encryption_key, encrypted_data)?;
                        obj.insert(field_name.clone(), Value::String(decrypted));
                    }
                }
            }
        }
    }

    Ok(decrypted_config)
}

/// Validate a configuration against a JSON Schema
pub fn validate_config(schema: &Value, config: &Value) -> Result<()> {
    let compiled_schema = jsonschema::JSONSchema::compile(schema).map_err(|e| {
        TingError::ConfigError(format!("Invalid configuration schema: {}", e))
    })?;

    if let Err(errors) = compiled_schema.validate(config) {
        let error_messages: Vec<String> = errors.map(|e| format!("{}", e)).collect();
        return Err(TingError::ConfigError(format!(
            "Configuration validation failed: {}",
            error_messages.join(", ")
        )));
    }

    Ok(())
}
