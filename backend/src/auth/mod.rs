//! Authentication module
//!
//! This module provides authentication functionality including:
//! - User registration and login
//! - JWT token generation and validation
//! - Password hashing and verification
//! - Authentication middleware
//! - Automatic JWT key rotation

pub mod jwt;
pub mod password;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod key_rotation;

pub use jwt::{generate_token, validate_token, validate_token_with_secrets, Claims};
pub use password::{hash_password, verify_password};
pub use middleware::{authenticate, AuthUser};
pub use handlers::{register, login, get_me, update_me};
pub use key_rotation::{JwtKeyManager, JwtKeyPair};
