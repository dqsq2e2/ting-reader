//! 主密钥自动派生
//!
//! 基于机器特征自动生成主密钥，无需用户配置。
//! 使用多种机器特征确保密钥的唯一性和稳定性。

use crate::core::error::{Result, TingError};
use sha2::{Sha256, Digest};
use std::path::Path;

/// 主密钥管理器
pub struct MasterKeyManager;

impl MasterKeyManager {
    /// 生成基于机器特征的主密钥
    ///
    /// 使用以下机器特征：
    /// 1. 环境变量 TING_MACHINE_ID（优先级最高，容器友好）
    /// 2. 数据库文件路径（确保不同实例有不同密钥）
    /// 3. 机器标识符（MAC地址/机器ID文件）
    /// 4. 固定盐值（增加安全性）
    /// 
    /// 注意：不包含程序版本，以确保升级后仍能解密数据
    pub fn derive_master_key(db_path: &Path) -> Result<[u8; 32]> {
        let mut hasher = Sha256::new();
        
        // 1. 数据库文件路径（规范化）
        let canonical_path = db_path.canonicalize()
            .unwrap_or_else(|_| db_path.to_path_buf());
        hasher.update(canonical_path.to_string_lossy().as_bytes());
        
        // 2. 机器标识符（支持环境变量覆盖）
        let machine_id = if let Ok(env_machine_id) = std::env::var("TING_MACHINE_ID") {
            tracing::info!("使用环境变量中的机器 ID");
            env_machine_id.into_bytes()
        } else {
            Self::get_machine_id()?
        };
        hasher.update(&machine_id);
        
        // 3. 固定盐值（防止彩虹表攻击）
        hasher.update(b"ting-reader-master-key-salt-v1");
        
        // 4. 应用标识符
        hasher.update(b"ting-reader-encryption-key");
        
        let result = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&result);
        
        tracing::info!("主密钥已基于机器特征自动派生（版本无关）");
        Ok(key)
    }
    
    /// 获取机器标识符
    ///
    /// 尝试多种方法获取稳定的机器标识：
    /// 1. 环境变量 TING_MACHINE_ID（最高优先级，容器推荐）
    /// 2. 机器 ID 文件（容器友好）
    /// 3. MAC 地址（物理机稳定）
    /// 4. 系统信息组合（最后备选）
    fn get_machine_id() -> Result<Vec<u8>> {
        // 方法 1: 环境变量（容器部署推荐）
        if let Ok(env_id) = std::env::var("TING_MACHINE_ID") {
            tracing::info!("使用环境变量中的机器 ID");
            return Ok(env_id.into_bytes());
        }
        
        // 方法 2: 尝试读取或创建机器 ID 文件（容器友好）
        if let Ok(file_id) = Self::get_or_create_machine_id_file() {
            return Ok(file_id);
        }
        
        // 方法 3: 尝试获取 MAC 地址（物理机稳定）
        if let Ok(mac_id) = Self::get_mac_address() {
            return Ok(mac_id);
        }
        
        // 方法 4: 使用系统信息组合
        Self::get_system_info_id()
    }
    
    /// 获取 MAC 地址作为机器标识
    fn get_mac_address() -> Result<Vec<u8>> {
        use std::process::Command;
        
        #[cfg(target_os = "windows")]
        {
            // Windows: 使用 getmac 命令
            let output = Command::new("getmac")
                .arg("/fo")
                .arg("csv")
                .arg("/nh")
                .output()
                .map_err(|e| TingError::ConfigError(format!("获取 MAC 地址失败: {}", e)))?;
            
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = stdout.lines().next() {
                    if let Some(mac) = line.split(',').next() {
                        let mac_clean = mac.trim_matches('"').replace("-", "");
                        return Ok(mac_clean.into_bytes());
                    }
                }
            }
        }
        
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            // Linux/macOS: 读取网络接口信息
            let interfaces = [
                "/sys/class/net/eth0/address",
                "/sys/class/net/en0/address", 
                "/sys/class/net/wlan0/address",
                "/sys/class/net/enp0s3/address",
            ];
            
            for interface in &interfaces {
                if let Ok(mac) = std::fs::read_to_string(interface) {
                    let mac_clean = mac.trim().replace(":", "");
                    if !mac_clean.is_empty() && mac_clean != "00:00:00:00:00:00" {
                        return Ok(mac_clean.into_bytes());
                    }
                }
            }
            
            // 尝试使用 ip 命令
            let output = Command::new("ip")
                .args(&["link", "show"])
                .output()
                .map_err(|e| TingError::ConfigError(format!("获取网络接口失败: {}", e)))?;
            
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.contains("link/ether") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if let Some(mac_pos) = parts.iter().position(|&x| x == "link/ether") {
                            if let Some(mac) = parts.get(mac_pos + 1) {
                                let mac_clean = mac.replace(":", "");
                                if mac_clean != "000000000000" {
                                    return Ok(mac_clean.into_bytes());
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Err(TingError::ConfigError("无法获取 MAC 地址".to_string()))
    }
    
    /// 获取或创建机器 ID 文件
    fn get_or_create_machine_id_file() -> Result<Vec<u8>> {
        // 优先使用数据目录中的机器ID（容器友好）
        let data_machine_id_path = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("data")
            .join(".machine-id");
        
        // 备选：用户目录中的机器ID
        let user_machine_id_path = dirs::data_dir()
            .or_else(|| dirs::home_dir())
            .map(|dir| dir.join(".ting-reader").join("machine-id"));
        
        // 按优先级尝试读取现有文件
        for path in [Some(&data_machine_id_path), user_machine_id_path.as_ref()].into_iter().flatten() {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let id = content.trim();
                    if !id.is_empty() {
                        tracing::info!("使用现有机器 ID: {}", path.display());
                        return Ok(id.as_bytes().to_vec());
                    }
                }
            }
        }
        
        // 创建新的机器 ID，优先保存到数据目录（容器持久化友好）
        let machine_id = uuid::Uuid::new_v4().to_string();
        
        // 尝试保存到数据目录
        if let Some(parent) = data_machine_id_path.parent() {
            if std::fs::create_dir_all(parent).is_ok() {
                if std::fs::write(&data_machine_id_path, &machine_id).is_ok() {
                    tracing::info!("已创建新的机器 ID（数据目录）: {}", data_machine_id_path.display());
                    return Ok(machine_id.as_bytes().to_vec());
                }
            }
        }
        
        // 备选：保存到用户目录
        if let Some(user_path) = user_machine_id_path {
            if let Some(parent) = user_path.parent() {
                if std::fs::create_dir_all(parent).is_ok() {
                    if std::fs::write(&user_path, &machine_id).is_ok() {
                        tracing::info!("已创建新的机器 ID（用户目录）: {}", user_path.display());
                        return Ok(machine_id.as_bytes().to_vec());
                    }
                }
            }
        }
        
        // 如果都失败了，返回基于UUID的临时ID（不推荐，但至少能工作）
        tracing::warn!("无法持久化机器 ID，使用临时标识符");
        Ok(machine_id.as_bytes().to_vec())
    }
    
    /// 使用系统信息组合生成机器标识
    fn get_system_info_id() -> Result<Vec<u8>> {
        let mut hasher = Sha256::new();
        
        // 主机名
        if let Ok(hostname) = hostname::get() {
            hasher.update(hostname.to_string_lossy().as_bytes());
        }
        
        // 用户名
        if let Ok(username) = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .or_else(|_| std::env::var("LOGNAME")) {
            hasher.update(username.as_bytes());
        }
        
        // 操作系统信息
        hasher.update(std::env::consts::OS.as_bytes());
        hasher.update(std::env::consts::ARCH.as_bytes());
        
        // 当前可执行文件路径
        if let Ok(exe_path) = std::env::current_exe() {
            hasher.update(exe_path.to_string_lossy().as_bytes());
        }
        
        // 工作目录
        if let Ok(cwd) = std::env::current_dir() {
            hasher.update(cwd.to_string_lossy().as_bytes());
        }
        
        // 添加随机种子（基于进程 ID 和时间）
        hasher.update(&std::process::id().to_le_bytes());
        
        let result = hasher.finalize();
        tracing::warn!("使用系统信息组合生成机器标识（不够稳定）");
        Ok(result.to_vec())
    }
    
    /// 验证主密钥是否有效
    pub fn validate_master_key(key: &[u8; 32]) -> bool {
        // 检查密钥不全为零
        !key.iter().all(|&b| b == 0)
    }
    
    /// 获取密钥信息（用于调试）
    pub fn get_key_info(db_path: &Path) -> String {
        let canonical_path = db_path.canonicalize()
            .unwrap_or_else(|_| db_path.to_path_buf());
        
        let machine_info = if let Ok(env_id) = std::env::var("TING_MACHINE_ID") {
            format!("环境变量机器ID: {} (长度: {} 字节)", 
                   &env_id[..std::cmp::min(8, env_id.len())], env_id.len())
        } else {
            match Self::get_machine_id() {
                Ok(id) => format!("自动获取机器ID长度: {} 字节", id.len()),
                Err(_) => "机器ID获取失败".to_string(),
            }
        };
        
        format!(
            "主密钥派生信息:\n- 数据库路径: {}\n- {}\n- 容器友好: {}\n- 版本无关: 是（升级安全）",
            canonical_path.display(),
            machine_info,
            std::env::var("TING_MACHINE_ID").is_ok()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    
    #[test]
    fn test_derive_master_key() {
        let db_path = PathBuf::from("./test.db");
        let key1 = MasterKeyManager::derive_master_key(&db_path).unwrap();
        let key2 = MasterKeyManager::derive_master_key(&db_path).unwrap();
        
        // 相同路径应该生成相同密钥
        assert_eq!(key1, key2);
        
        // 密钥不应该全为零
        assert!(MasterKeyManager::validate_master_key(&key1));
    }
    
    #[test]
    fn test_different_paths_different_keys() {
        let key1 = MasterKeyManager::derive_master_key(&PathBuf::from("./test1.db")).unwrap();
        let key2 = MasterKeyManager::derive_master_key(&PathBuf::from("./test2.db")).unwrap();
        
        // 不同路径应该生成不同密钥
        assert_ne!(key1, key2);
    }
    
    #[test]
    fn test_get_machine_id() {
        let id1 = MasterKeyManager::get_machine_id().unwrap();
        let id2 = MasterKeyManager::get_machine_id().unwrap();
        
        // 机器 ID 应该稳定
        assert_eq!(id1, id2);
        assert!(!id1.is_empty());
    }
}