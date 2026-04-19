//! JWT token generation and validation

use crate::core::error::{Result, TingError};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub user_id: String,
    pub exp: usize,
}

/// Generate a JWT token for a user
pub fn generate_token(user_id: &str, secret: &str) -> Result<String> {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::days(7))
        .ok_or_else(|| TingError::AuthenticationError("无法计算令牌过期时间".to_string()))?
        .timestamp() as usize;

    let claims = Claims {
        user_id: user_id.to_string(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| TingError::AuthenticationError(format!("生成令牌失败: {}", e)))
}

/// Validate a JWT token and extract claims (single secret)
pub fn validate_token(token: &str, secret: &str) -> Result<Claims> {
    validate_token_with_secrets(token, &[secret.to_string()])
}

/// Validate a JWT token with multiple secrets (for key rotation)
pub fn validate_token_with_secrets(token: &str, secrets: &[String]) -> Result<Claims> {
    let mut last_error = None;
    
    // 尝试用每个密钥验证
    for secret in secrets {
        match decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        ) {
            Ok(token_data) => return Ok(token_data.claims),
            Err(e) => last_error = Some(e),
        }
    }
    
    // 所有密钥都失败，返回错误
    if let Some(e) = last_error {
        let error_msg = e.to_string();
        if error_msg.contains("ExpiredSignature") {
            return Err(TingError::AuthenticationError("令牌已过期".to_string()));
        } else if error_msg.contains("InvalidSignature") {
            return Err(TingError::AuthenticationError("令牌签名无效".to_string()));
        } else if error_msg.contains("InvalidToken") {
            return Err(TingError::AuthenticationError("令牌格式无效".to_string()));
        } else {
            return Err(TingError::AuthenticationError(format!("令牌验证失败: {}", e)));
        }
    }
    
    Err(TingError::AuthenticationError("令牌验证失败".to_string()))
}
