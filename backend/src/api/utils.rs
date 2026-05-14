//! API utility helpers

use crate::auth::AuthUser;
use crate::core::error::{Result, TingError};

/// Require admin role, returning PermissionDenied if not admin
pub fn require_admin(user: &AuthUser) -> Result<()> {
    if user.role != "admin" {
        return Err(TingError::PermissionDenied("Admin access required".to_string()));
    }
    Ok(())
}
