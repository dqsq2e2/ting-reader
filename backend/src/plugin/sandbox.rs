//! Security sandbox implementation
//!
//! This module provides security isolation for plugins through:
//! - Permission-based access control
//! - Resource usage limits
//! - File system and network access restrictions

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use crate::core::error::Result;

/// Security sandbox for plugin execution
/// 
/// Provides isolation and resource limits for plugins to prevent
/// malicious behavior and resource exhaustion.
#[derive(Clone)]
pub struct Sandbox {
    /// Permissions granted to the plugin
    pub permissions: Vec<Permission>,
    
    /// Resource limits for the plugin
    pub resource_limits: ResourceLimits,
    
    /// Virtual file system (restricted paths)
    allowed_paths: Vec<PathBuf>,
    
    /// Network access whitelist (allowed domains)
    allowed_domains: Vec<String>,
}

impl Sandbox {
    /// Create a new sandbox with the given permissions and limits
    pub fn new(permissions: Vec<Permission>, resource_limits: ResourceLimits) -> Self {
        let mut allowed_paths = Vec::new();
        let mut allowed_domains = Vec::new();
        
        // Extract allowed paths and domains from permissions
        for permission in &permissions {
            match permission {
                Permission::FileRead(path) | Permission::FileWrite(path) => {
                    allowed_paths.push(path.clone());
                }
                Permission::NetworkAccess(domain) => {
                    allowed_domains.push(domain.clone());
                }
                _ => {}
            }
        }
        
        Self {
            permissions,
            resource_limits,
            allowed_paths,
            allowed_domains,
        }
    }
    
    /// Check if file access is allowed
    pub fn check_file_access(&self, path: &std::path::Path, access: FileAccess) -> Result<()> {
        // Normalize the path for comparison (handle mixed separators and redundant separators)
        let normalized_path = Self::normalize_path(path);
        
        // Check if the path is within any allowed path
        let is_allowed = self.allowed_paths.iter().any(|allowed_path| {
            let normalized_allowed = Self::normalize_path(allowed_path);
            normalized_path.starts_with(&normalized_allowed)
        });
        
        if !is_allowed {
            return Err(crate::core::error::TingError::PermissionDenied(
                format!("File access denied: {:?}", path)
            ));
        }
        
        // Check if the specific access type is permitted
        let has_permission = match access {
            FileAccess::Read => self.permissions.iter().any(|p| {
                matches!(p, Permission::FileRead(allowed) if {
                    let normalized_allowed = Self::normalize_path(allowed);
                    normalized_path.starts_with(&normalized_allowed)
                })
            }),
            FileAccess::Write => self.permissions.iter().any(|p| {
                matches!(p, Permission::FileWrite(allowed) if {
                    let normalized_allowed = Self::normalize_path(allowed);
                    normalized_path.starts_with(&normalized_allowed)
                })
            }),
            FileAccess::Execute => false, // Execute not currently supported
        };
        
        if !has_permission {
            return Err(crate::core::error::TingError::PermissionDenied(
                format!("File {:?} access denied: {:?}", access, path)
            ));
        }
        
        Ok(())
    }
    
    /// Normalize a path by removing redundant separators and converting to a canonical form
    fn normalize_path(path: &std::path::Path) -> PathBuf {
        use std::path::Component;
        
        // Convert to string and normalize separators first
        let path_str = path.to_string_lossy();
        let normalized_str = path_str.replace('\\', "/");
        
        // Remove redundant slashes
        let mut cleaned = String::new();
        let mut last_was_slash = false;
        for ch in normalized_str.chars() {
            if ch == '/' {
                if !last_was_slash {
                    cleaned.push(ch);
                    last_was_slash = true;
                }
            } else {
                cleaned.push(ch);
                last_was_slash = false;
            }
        }
        
        // Parse the cleaned path
        let cleaned_path = PathBuf::from(cleaned);
        
        let mut components = Vec::new();
        for component in cleaned_path.components() {
            match component {
                Component::RootDir | Component::Prefix(_) => {
                    components.clear();
                    components.push(component);
                }
                Component::CurDir => {
                    // Skip "." components
                }
                Component::ParentDir => {
                    // Handle ".." by popping the last component if it's not a root
                    if let Some(last) = components.last() {
                        if !matches!(last, Component::RootDir | Component::Prefix(_)) {
                            components.pop();
                        }
                    }
                }
                Component::Normal(_) => {
                    components.push(component);
                }
            }
        }
        
        components.iter().collect()
    }
    
    /// Get list of allowed file paths
    pub fn get_allowed_paths(&self) -> &[PathBuf] {
        &self.allowed_paths
    }
    
    /// Check if network access is allowed
    pub fn check_network_access(&self, url: &str) -> Result<()> {
        // Parse domain from URL
        let domain = Self::extract_domain(url)?;
        
        // Check if domain matches any allowed pattern
        let is_allowed = self.allowed_domains.iter().any(|pattern| {
            Self::domain_matches(&domain, pattern)
        });
        
        if !is_allowed {
            return Err(crate::core::error::TingError::PermissionDenied(
                format!("Network access denied: {}", url)
            ));
        }
        
        Ok(())
    }
    
    /// Get list of allowed domains
    pub fn get_allowed_domains(&self) -> &[String] {
        &self.allowed_domains
    }
    
    /// Check if memory usage is within limits
    pub fn check_memory_limit(&self, current_bytes: usize) -> Result<()> {
        if current_bytes > self.resource_limits.max_memory_bytes {
            return Err(crate::core::error::TingError::ResourceLimitExceeded(
                format!(
                    "Memory limit exceeded: {} bytes (limit: {} bytes)",
                    current_bytes, self.resource_limits.max_memory_bytes
                )
            ));
        }
        Ok(())
    }
    
    /// Check if CPU time is within limits
    pub fn check_cpu_time(&self, elapsed: Duration) -> Result<()> {
        if elapsed > self.resource_limits.max_cpu_time {
            return Err(crate::core::error::TingError::Timeout(
                format!(
                    "CPU time limit exceeded: {:?} (limit: {:?})",
                    elapsed, self.resource_limits.max_cpu_time
                )
            ));
        }
        Ok(())
    }
    
    /// Extract domain from URL
    fn extract_domain(url: &str) -> Result<String> {
        // Simple domain extraction (in production, use url crate)
        let url = url.trim_start_matches("http://").trim_start_matches("https://");
        let domain = url.split('/').next().unwrap_or(url);
        let domain = domain.split(':').next().unwrap_or(domain);
        Ok(domain.to_string())
    }
    
    /// Check if domain matches pattern (supports wildcards)
    fn domain_matches(domain: &str, pattern: &str) -> bool {
        if pattern.starts_with("*.") {
            // Wildcard subdomain match
            let base = &pattern[2..];
            domain.ends_with(base) || domain == base
        } else {
            // Exact match
            domain == pattern
        }
    }
}

/// Permission types for plugin access control
/// 
/// Defines what resources a plugin is allowed to access.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value")]
pub enum Permission {
    /// Read access to a file or directory
    #[serde(rename = "file_read")]
    FileRead(PathBuf),
    
    /// Write access to a file or directory
    #[serde(rename = "file_write")]
    FileWrite(PathBuf),
    
    /// Network access to a domain or URL pattern
    /// 
    /// Supports wildcards: "*.example.com" matches all subdomains
    #[serde(rename = "network_access")]
    NetworkAccess(String),
    
    /// Read access to the database
    #[serde(rename = "database_read")]
    DatabaseRead,
    
    /// Write access to the database
    #[serde(rename = "database_write")]
    DatabaseWrite,
    
    /// Permission to publish events
    #[serde(rename = "event_publish")]
    EventPublish,
    
    /// Permission to subscribe to specific event types
    #[serde(rename = "event_subscribe")]
    EventSubscribe(String),
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Permission::FileRead(path) => write!(f, "FileRead({:?})", path),
            Permission::FileWrite(path) => write!(f, "FileWrite({:?})", path),
            Permission::NetworkAccess(domain) => write!(f, "NetworkAccess({})", domain),
            Permission::DatabaseRead => write!(f, "DatabaseRead"),
            Permission::DatabaseWrite => write!(f, "DatabaseWrite"),
            Permission::EventPublish => write!(f, "EventPublish"),
            Permission::EventSubscribe(event_type) => write!(f, "EventSubscribe({})", event_type),
        }
    }
}

/// Resource limits for plugin execution
/// 
/// Defines the maximum resources a plugin can consume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum memory usage in bytes
    pub max_memory_bytes: usize,
    
    /// Maximum CPU time per single execution
    pub max_cpu_time: Duration,
    
    /// Maximum size of a single file operation in bytes
    pub max_file_size_bytes: u64,
    
    /// Maximum network bandwidth in bytes per second
    pub max_network_bandwidth_bps: u64,
}

impl ResourceLimits {
    /// Create new resource limits with default values
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Create resource limits with custom values
    pub fn custom(
        max_memory_bytes: usize,
        max_cpu_time: Duration,
        max_file_size_bytes: u64,
        max_network_bandwidth_bps: u64,
    ) -> Self {
        Self {
            max_memory_bytes,
            max_cpu_time,
            max_file_size_bytes,
            max_network_bandwidth_bps,
        }
    }
    
    /// Create permissive limits for trusted plugins
    pub fn permissive() -> Self {
        Self {
            max_memory_bytes: 1024 * 1024 * 1024, // 1 GB
            max_cpu_time: Duration::from_secs(600), // 10 minutes
            max_file_size_bytes: 1024 * 1024 * 1024, // 1 GB
            max_network_bandwidth_bps: 100 * 1024 * 1024, // 100 MB/s
        }
    }
    
    /// Create restrictive limits for untrusted plugins
    pub fn restrictive() -> Self {
        Self {
            max_memory_bytes: 128 * 1024 * 1024, // 128 MB
            max_cpu_time: Duration::from_secs(30), // 30 seconds
            max_file_size_bytes: 10 * 1024 * 1024, // 10 MB
            max_network_bandwidth_bps: 1024 * 1024, // 1 MB/s
        }
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 512 * 1024 * 1024, // 512 MB
            max_cpu_time: Duration::from_secs(300), // 5 minutes
            max_file_size_bytes: 100 * 1024 * 1024, // 100 MB
            max_network_bandwidth_bps: 10 * 1024 * 1024, // 10 MB/s
        }
    }
}

/// File access type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAccess {
    /// Read access
    Read,
    
    /// Write access
    Write,
    
    /// Execute access
    Execute,
}

impl std::fmt::Display for FileAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileAccess::Read => write!(f, "Read"),
            FileAccess::Write => write!(f, "Write"),
            FileAccess::Execute => write!(f, "Execute"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_domain_matches_exact() {
        assert!(Sandbox::domain_matches("example.com", "example.com"));
        assert!(!Sandbox::domain_matches("example.com", "other.com"));
    }
    
    #[test]
    fn test_domain_matches_wildcard() {
        assert!(Sandbox::domain_matches("sub.example.com", "*.example.com"));
        assert!(Sandbox::domain_matches("example.com", "*.example.com"));
        assert!(!Sandbox::domain_matches("example.org", "*.example.com"));
    }
    
    #[test]
    fn test_extract_domain() {
        assert_eq!(
            Sandbox::extract_domain("https://example.com/path").unwrap(),
            "example.com"
        );
        assert_eq!(
            Sandbox::extract_domain("http://example.com:8080/path").unwrap(),
            "example.com"
        );
    }
}
