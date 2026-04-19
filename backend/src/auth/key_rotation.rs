//! JWT 密钥自动生成和轮换
//!
//! 维护两个活跃密钥：
//! - 当前密钥：用于签发新 token
//! - 上一个密钥：用于验证旧 token（7天内有效）
//!
//! 安全特性：
//! - 密钥在数据库中加密存储
//! - 使用系统派生的主密钥加密
//! - 支持密钥轮换和平滑过渡

use crate::core::error::{Result, TingError};
use crate::core::crypto::{encrypt, decrypt};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// JWT 密钥对（内存中的明文形式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtKeyPair {
    /// 当前密钥（用于签发）
    pub current: String,
    /// 当前密钥创建时间
    pub current_created_at: i64,
    /// 上一个密钥（用于验证）
    pub previous: Option<String>,
    /// 上一个密钥创建时间
    pub previous_created_at: Option<i64>,
}

/// JWT 密钥对（数据库中的加密形式）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedJwtKeyPair {
    /// 加密的当前密钥
    current_encrypted: String,
    /// 当前密钥创建时间
    current_created_at: i64,
    /// 加密的上一个密钥
    previous_encrypted: Option<String>,
    /// 上一个密钥创建时间
    previous_created_at: Option<i64>,
}

impl JwtKeyPair {
    /// 生成新的密钥对
    pub fn generate() -> Self {
        let current = Self::generate_secret();
        let now = chrono::Utc::now().timestamp();
        
        Self {
            current,
            current_created_at: now,
            previous: None,
            previous_created_at: None,
        }
    }

    /// 生成随机密钥
    fn generate_secret() -> String {
        let mut rng = rand::thread_rng();
        let bytes: Vec<u8> = (0..64).map(|_| rng.gen()).collect();
        use base64::{Engine as _, engine::general_purpose};
        general_purpose::STANDARD.encode(&bytes)
    }

    /// 轮换密钥
    pub fn rotate(&mut self) {
        info!("正在轮换 JWT 密钥");
        
        // 将当前密钥移到 previous
        self.previous = Some(self.current.clone());
        self.previous_created_at = Some(self.current_created_at);
        
        // 生成新的当前密钥
        self.current = Self::generate_secret();
        self.current_created_at = chrono::Utc::now().timestamp();
        
        info!("JWT 密钥轮换完成");
    }

    /// 检查是否需要轮换（超过7天）
    pub fn should_rotate(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        let age_days = (now - self.current_created_at) / 86400;
        age_days >= 7
    }

    /// 获取所有有效密钥（用于验证）
    pub fn get_valid_secrets(&self) -> Vec<String> {
        let mut secrets = vec![self.current.clone()];
        if let Some(prev) = &self.previous {
            secrets.push(prev.clone());
        }
        secrets
    }

    /// 加密密钥对用于存储
    fn encrypt(&self, encryption_key: &[u8; 32]) -> Result<EncryptedJwtKeyPair> {
        let current_encrypted = encrypt(&self.current, encryption_key)?;
        let previous_encrypted = if let Some(prev) = &self.previous {
            Some(encrypt(prev, encryption_key)?)
        } else {
            None
        };

        Ok(EncryptedJwtKeyPair {
            current_encrypted,
            current_created_at: self.current_created_at,
            previous_encrypted,
            previous_created_at: self.previous_created_at,
        })
    }

    /// 从加密的密钥对解密
    fn decrypt(encrypted: &EncryptedJwtKeyPair, encryption_key: &[u8; 32]) -> Result<Self> {
        let current = decrypt(&encrypted.current_encrypted, encryption_key)?;
        let previous = if let Some(prev_enc) = &encrypted.previous_encrypted {
            Some(decrypt(prev_enc, encryption_key)?)
        } else {
            None
        };

        Ok(Self {
            current,
            current_created_at: encrypted.current_created_at,
            previous,
            previous_created_at: encrypted.previous_created_at,
        })
    }
}

/// JWT 密钥管理器
pub struct JwtKeyManager {
    keys: Arc<RwLock<JwtKeyPair>>,
    db: Arc<r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>>,
    encryption_key: Arc<[u8; 32]>,
}

impl JwtKeyManager {
    /// 创建新的密钥管理器
    pub async fn new(
        db: Arc<r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>>,
        encryption_key: [u8; 32],
    ) -> Result<Self> {
        let encryption_key = Arc::new(encryption_key);
        
        // 从数据库加载或生成新密钥
        let keys = Self::load_or_generate(&db, &encryption_key).await?;
        
        let manager = Self {
            keys: Arc::new(RwLock::new(keys)),
            db,
            encryption_key,
        };
        
        Ok(manager)
    }

    /// 从数据库加载或生成新密钥
    async fn load_or_generate(
        db: &r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>,
        encryption_key: &[u8; 32],
    ) -> Result<JwtKeyPair> {
        // 尝试从数据库加载
        match Self::load_from_db(db, encryption_key) {
            Ok(keys) => {
                info!("从数据库加载 JWT 密钥（已加密存储）");
                Ok(keys)
            }
            Err(_) => {
                info!("生成新的 JWT 密钥对（将加密存储）");
                let keys = JwtKeyPair::generate();
                Self::save_to_db(db, &keys, encryption_key)?;
                Ok(keys)
            }
        }
    }

    /// 从数据库加载密钥（解密）
    fn load_from_db(
        db: &r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>,
        encryption_key: &[u8; 32],
    ) -> Result<JwtKeyPair> {
        let conn = db.get().map_err(|e| TingError::DatabaseError(
            rusqlite::Error::ToSqlConversionFailure(Box::new(e))
        ))?;
        
        let encrypted_json: String = conn
            .query_row(
                "SELECT value FROM system_settings WHERE key = 'jwt_keys'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| TingError::DatabaseError(e))?;
        
        // 反序列化加密的密钥对
        let encrypted: EncryptedJwtKeyPair = serde_json::from_str(&encrypted_json)
            .map_err(|e| TingError::ConfigError(format!("解析加密的 JWT 密钥失败: {}", e)))?;
        
        // 解密密钥对
        JwtKeyPair::decrypt(&encrypted, encryption_key)
    }

    /// 保存密钥到数据库（加密）
    fn save_to_db(
        db: &r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>,
        keys: &JwtKeyPair,
        encryption_key: &[u8; 32],
    ) -> Result<()> {
        let conn = db.get().map_err(|e| TingError::DatabaseError(
            rusqlite::Error::ToSqlConversionFailure(Box::new(e))
        ))?;
        
        // 加密密钥对
        let encrypted = keys.encrypt(encryption_key)?;
        
        // 序列化加密后的数据
        let encrypted_json = serde_json::to_string(&encrypted)
            .map_err(|e| TingError::ConfigError(format!("序列化加密的 JWT 密钥失败: {}", e)))?;
        
        conn.execute(
            "INSERT OR REPLACE INTO system_settings (key, value, updated_at) VALUES ('jwt_keys', ?, CURRENT_TIMESTAMP)",
            [encrypted_json],
        )
        .map_err(|e| TingError::DatabaseError(e))?;
        
        Ok(())
    }

    /// 获取当前签发密钥
    pub async fn get_signing_secret(&self) -> String {
        let keys = self.keys.read().await;
        keys.current.clone()
    }

    /// 获取所有验证密钥
    pub async fn get_validation_secrets(&self) -> Vec<String> {
        let keys = self.keys.read().await;
        keys.get_valid_secrets()
    }

    /// 检查并执行密钥轮换
    pub async fn check_and_rotate(&self) -> Result<()> {
        let should_rotate = {
            let keys = self.keys.read().await;
            keys.should_rotate()
        };

        if should_rotate {
            let mut keys = self.keys.write().await;
            keys.rotate();
            Self::save_to_db(&self.db, &keys, &self.encryption_key)?;
            info!("JWT 密钥已自动轮换并加密存储");
        }

        Ok(())
    }

    /// 启动后台轮换任务
    pub fn start_rotation_task(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600)); // 每小时检查一次
            
            loop {
                interval.tick().await;
                
                if let Err(e) = self.check_and_rotate().await {
                    warn!("JWT 密钥轮换检查失败: {}", e);
                }
            }
        });
    }
}
