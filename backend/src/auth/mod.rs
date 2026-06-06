//! Authentication module
//!
//! This module provides authentication functionality including:
//! - User registration and login
//! - JWT token generation and validation
//! - Password hashing and verification
//! - Authentication middleware
//! - Automatic JWT key rotation

pub mod handlers;
pub mod jwt;
pub mod key_rotation;
pub mod middleware;
pub mod models;
pub mod password;

pub use handlers::{get_me, login, register, update_me};
pub use jwt::{generate_token, validate_token, validate_token_with_secrets, Claims};
pub use key_rotation::{JwtKeyManager, JwtKeyPair};
pub use middleware::{authenticate, AuthUser};
pub use password::{hash_password, verify_password};
