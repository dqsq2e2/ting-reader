//! Cryptographic utilities for encrypting and decrypting sensitive data
//!
//! This module provides encryption and decryption functions for sensitive data
//! such as WebDAV credentials. It uses AES-256-GCM for authenticated encryption.

use crate::core::error::{Result, TingError};
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{Engine as _, engine::general_purpose};

/// Encrypt a string value using AES-256-GCM
///
/// # Arguments
/// * `value` - The plaintext string to encrypt
/// * `key` - 32-byte encryption key
///
/// # Returns
/// Base64-encoded string containing nonce + ciphertext
///
/// # Example
/// ```ignore
/// let key = [0u8; 32];
/// let encrypted = encrypt("my_password", &key)?;
/// ```
pub fn encrypt(value: &str, key: &[u8; 32]) -> Result<String> {
    let cipher = Aes256Gcm::new(key.into());
    
    // Generate random nonce
    let mut nonce_bytes = [0u8; 12];
    use aes_gcm::aead::rand_core::RngCore;
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    // Encrypt
    let ciphertext = cipher.encrypt(nonce, value.as_bytes())
        .map_err(|e| TingError::ConfigError(format!("Encryption failed: {}", e)))?;
    
    // Combine nonce + ciphertext and encode as base64
    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);
    
    Ok(general_purpose::STANDARD.encode(&combined))
}

/// Decrypt a string value using AES-256-GCM
///
/// # Arguments
/// * `encrypted` - Base64-encoded string containing nonce + ciphertext
/// * `key` - 32-byte encryption key (must be the same key used for encryption)
///
/// # Returns
/// Decrypted plaintext string
///
/// # Example
/// ```ignore
/// let key = [0u8; 32];
/// let decrypted = decrypt(&encrypted_value, &key)?;
/// ```
pub fn decrypt(encrypted: &str, key: &[u8; 32]) -> Result<String> {
    let cipher = Aes256Gcm::new(key.into());
    
    // Decode from base64
    let combined = general_purpose::STANDARD.decode(encrypted)
        .map_err(|e| TingError::ConfigError(format!("Invalid encrypted data: {}", e)))?;
    
    if combined.len() < 12 {
        return Err(TingError::ConfigError("Invalid encrypted data length".to_string()));
    }
    
    // Split nonce and ciphertext
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    
    // Decrypt
    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|e| TingError::ConfigError(format!("Decryption failed: {}", e)))?;
    
    String::from_utf8(plaintext)
        .map_err(|e| TingError::ConfigError(format!("Invalid UTF-8 in decrypted data: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        [0x42; 32] // Test key - all bytes set to 0x42
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = test_key();
        let original = "my_secret_password";
        
        let encrypted = encrypt(original, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();
        
        assert_eq!(original, decrypted);
    }

    #[test]
    fn test_encrypt_produces_different_ciphertext() {
        let key = test_key();
        let value = "same_password";
        
        let encrypted1 = encrypt(value, &key).unwrap();
        let encrypted2 = encrypt(value, &key).unwrap();
        
        // Due to random nonce, ciphertexts should be different
        assert_ne!(encrypted1, encrypted2);
        
        // But both should decrypt to the same value
        assert_eq!(decrypt(&encrypted1, &key).unwrap(), value);
        assert_eq!(decrypt(&encrypted2, &key).unwrap(), value);
    }

    #[test]
    fn test_decrypt_with_wrong_key_fails() {
        let key1 = test_key();
        let mut key2 = test_key();
        key2[0] = 0xFF; // Change one byte
        
        let encrypted = encrypt("secret", &key1).unwrap();
        let result = decrypt(&encrypted, &key2);
        
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_invalid_base64_fails() {
        let key = test_key();
        let result = decrypt("not_valid_base64!!!", &key);
        
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_too_short_data_fails() {
        let key = test_key();
        // Create a base64 string that's too short (less than 12 bytes when decoded)
        let short_data = general_purpose::STANDARD.encode(&[0u8; 5]);
        let result = decrypt(&short_data, &key);
        
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypt_empty_string() {
        let key = test_key();
        let encrypted = encrypt("", &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();
        
        assert_eq!("", decrypted);
    }

    #[test]
    fn test_encrypt_unicode_string() {
        let key = test_key();
        let original = "密码123!@#";
        
        let encrypted = encrypt(original, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();
        
        assert_eq!(original, decrypted);
    }
}
