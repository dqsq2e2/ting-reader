//! Password hashing and verification using bcrypt

use crate::core::error::{Result, TingError};

/// Hash a password using bcrypt
pub fn hash_password(password: &str) -> Result<String> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST)
        .map_err(|e| TingError::AuthenticationError(format!("Failed to hash password: {}", e)))
}

/// Verify a password against a hash
pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    bcrypt::verify(password, hash)
        .map_err(|e| TingError::AuthenticationError(format!("Failed to verify password: {}", e)))
}
