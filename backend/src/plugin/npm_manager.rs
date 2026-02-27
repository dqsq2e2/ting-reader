//! npm Dependency Manager
//!
//! This module handles npm dependency resolution and installation for JavaScript plugins.
//! It provides functionality to:
//! - Parse npm dependencies from plugin.json
//! - Generate package.json files
//! - Execute npm install commands
//! - Manage node_modules paths

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, RwLock};
use tracing::{debug, error, info, warn};

use crate::core::error::TingError;

/// npm dependency specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpmDependency {
    /// Package name
    pub name: String,
    
    /// Version requirement (e.g., "^1.0.0", ">=2.0.0", "latest")
    pub version: String,
}

impl NpmDependency {
    /// Create a new npm dependency
    pub fn new(name: String, version: String) -> Self {
        Self { name, version }
    }
}

/// Security configuration for npm dependency management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpmSecurityConfig {
    /// Whitelist of allowed npm packages (empty means all allowed)
    pub whitelist: HashSet<String>,
    
    /// Whether to enforce version locking via package-lock.json
    pub enforce_version_lock: bool,
    
    /// Whether to run npm audit for security scanning
    pub enable_audit: bool,
    
    /// Whether to fail installation on audit vulnerabilities
    pub fail_on_audit_vulnerabilities: bool,
    
    /// Maximum allowed severity level for vulnerabilities (low, moderate, high, critical)
    pub max_vulnerability_severity: VulnerabilitySeverity,
}

impl Default for NpmSecurityConfig {
    fn default() -> Self {
        Self {
            whitelist: HashSet::new(),
            enforce_version_lock: true,
            enable_audit: false,
            fail_on_audit_vulnerabilities: false,
            max_vulnerability_severity: VulnerabilitySeverity::High,
        }
    }
}

/// Vulnerability severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum VulnerabilitySeverity {
    Low,
    Moderate,
    High,
    Critical,
}

impl VulnerabilitySeverity {
    /// Parse severity from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "low" => Some(Self::Low),
            "moderate" => Some(Self::Moderate),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
    
    /// Convert to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Moderate => "moderate",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// npm audit result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpmAuditResult {
    /// Number of vulnerabilities by severity
    pub vulnerabilities: HashMap<VulnerabilitySeverity, usize>,
    
    /// Total number of vulnerabilities
    pub total: usize,
    
    /// Whether the audit passed (no vulnerabilities above threshold)
    pub passed: bool,
    
    /// Raw audit output
    pub raw_output: String,
}

/// Dependency installation log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInstallLog {
    /// Timestamp of installation
    pub timestamp: String,
    
    /// Plugin name
    pub plugin_name: String,
    
    /// Dependencies installed
    pub dependencies: Vec<NpmDependency>,
    
    /// Whether installation succeeded
    pub success: bool,
    
    /// Error message if failed
    pub error: Option<String>,
    
    /// Audit result if enabled
    pub audit_result: Option<NpmAuditResult>,
}

/// Cache entry for a dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Package name
    pub package_name: String,
    
    /// Package version
    pub version: String,
    
    /// Path to cached package
    pub cache_path: PathBuf,
    
    /// Plugins using this dependency
    pub used_by: HashSet<String>,
    
    /// Last accessed timestamp
    pub last_accessed: String,
    
    /// Size in bytes
    pub size_bytes: u64,
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStatistics {
    /// Total number of cached packages
    pub total_packages: usize,
    
    /// Total cache size in bytes
    pub total_size_bytes: u64,
    
    /// Number of cache hits
    pub cache_hits: usize,
    
    /// Number of cache misses
    pub cache_misses: usize,
    
    /// Cache hit rate (0.0 to 1.0)
    pub hit_rate: f64,
    
    /// Number of plugins using cache
    pub plugins_count: usize,
    
    /// Timestamp of last cleanup
    pub last_cleanup: Option<String>,
}

/// package.json structure for JavaScript plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageJson {
    /// Package name
    pub name: String,
    
    /// Package version
    pub version: String,
    
    /// Package description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// Package author
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    
    /// Package license
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    
    /// Dependencies
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub dependencies: HashMap<String, String>,
    
    /// Private flag (always true for plugins)
    pub private: bool,
}

impl PackageJson {
    /// Create a new package.json from plugin metadata
    pub fn from_plugin_metadata(
        name: &str,
        version: &str,
        description: Option<&str>,
        author: Option<&str>,
        license: Option<&str>,
        npm_dependencies: &[NpmDependency],
    ) -> Self {
        let mut dependencies = HashMap::new();
        for dep in npm_dependencies {
            dependencies.insert(dep.name.clone(), dep.version.clone());
        }
        
        Self {
            name: name.to_string(),
            version: version.to_string(),
            description: description.map(|s| s.to_string()),
            author: author.map(|s| s.to_string()),
            license: license.map(|s| s.to_string()),
            dependencies,
            private: true,
        }
    }
    
    /// Write package.json to a file
    pub fn write_to_file(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize package.json")?;
        
        std::fs::write(path, json)
            .with_context(|| format!("Failed to write package.json to {}", path.display()))?;
        
        info!("Generated package.json at: {}", path.display());
        Ok(())
    }
    
    /// Read package.json from a file
    pub fn read_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read package.json from {}", path.display()))?;
        
        let package_json: PackageJson = serde_json::from_str(&content)
            .context("Failed to parse package.json")?;
        
        Ok(package_json)
    }
}

/// npm dependency manager
pub struct NpmManager {
    /// Path to npm executable
    npm_path: PathBuf,
    
    /// Global node_modules cache directory
    cache_dir: Option<PathBuf>,
    
    /// Security configuration
    security_config: NpmSecurityConfig,
    
    /// Installation log directory
    log_dir: Option<PathBuf>,
    
    /// Cache registry (package_name@version -> CacheEntry)
    cache_registry: Arc<RwLock<HashMap<String, CacheEntry>>>,
    
    /// Cache statistics
    cache_stats: Arc<RwLock<CacheStatistics>>,
}

impl NpmManager {
    /// Create a new npm manager
    ///
    /// # Arguments
    /// * `npm_path` - Optional path to npm executable (defaults to "npm" in PATH)
    /// * `cache_dir` - Optional global cache directory for node_modules
    pub fn new(npm_path: Option<PathBuf>, cache_dir: Option<PathBuf>) -> Self {
        let npm_path = npm_path.unwrap_or_else(|| PathBuf::from("npm"));
        
        Self {
            npm_path,
            cache_dir,
            security_config: NpmSecurityConfig::default(),
            log_dir: None,
            cache_registry: Arc::new(RwLock::new(HashMap::new())),
            cache_stats: Arc::new(RwLock::new(CacheStatistics {
                total_packages: 0,
                total_size_bytes: 0,
                cache_hits: 0,
                cache_misses: 0,
                hit_rate: 0.0,
                plugins_count: 0,
                last_cleanup: None,
            })),
        }
    }
    
    /// Create a new npm manager with security configuration
    ///
    /// # Arguments
    /// * `npm_path` - Optional path to npm executable (defaults to "npm" in PATH)
    /// * `cache_dir` - Optional global cache directory for node_modules
    /// * `security_config` - Security configuration
    /// * `log_dir` - Optional directory for installation logs
    pub fn with_security(
        npm_path: Option<PathBuf>,
        cache_dir: Option<PathBuf>,
        security_config: NpmSecurityConfig,
        log_dir: Option<PathBuf>,
    ) -> Self {
        let npm_path = npm_path.unwrap_or_else(|| PathBuf::from("npm"));
        
        Self {
            npm_path,
            cache_dir,
            security_config,
            log_dir,
            cache_registry: Arc::new(RwLock::new(HashMap::new())),
            cache_stats: Arc::new(RwLock::new(CacheStatistics {
                total_packages: 0,
                total_size_bytes: 0,
                cache_hits: 0,
                cache_misses: 0,
                hit_rate: 0.0,
                plugins_count: 0,
                last_cleanup: None,
            })),
        }
    }
    
    /// Set security configuration
    pub fn set_security_config(&mut self, config: NpmSecurityConfig) {
        self.security_config = config;
    }
    
    /// Set log directory
    pub fn set_log_dir(&mut self, log_dir: PathBuf) {
        self.log_dir = Some(log_dir);
    }
    
    /// Parse npm dependencies from plugin.json
    ///
    /// # Arguments
    /// * `plugin_json` - The parsed plugin.json content
    ///
    /// # Returns
    /// A vector of npm dependencies, or empty if none specified
    pub fn parse_dependencies(plugin_json: &Value) -> Vec<NpmDependency> {
        let mut dependencies = Vec::new();
        
        // Check for "npm_dependencies" field
        if let Some(npm_deps) = plugin_json.get("npm_dependencies") {
            if let Some(deps_obj) = npm_deps.as_object() {
                for (name, version) in deps_obj {
                    if let Some(version_str) = version.as_str() {
                        dependencies.push(NpmDependency::new(
                            name.clone(),
                            version_str.to_string(),
                        ));
                    } else {
                        warn!("Invalid npm dependency version for {}: {:?}", name, version);
                    }
                }
            } else {
                warn!("npm_dependencies field is not an object");
            }
        }
        
        debug!("Parsed {} npm dependencies", dependencies.len());
        dependencies
    }
    
    /// Generate package.json for a plugin
    ///
    /// # Arguments
    /// * `plugin_dir` - Plugin directory path
    /// * `plugin_name` - Plugin name
    /// * `plugin_version` - Plugin version
    /// * `description` - Plugin description
    /// * `author` - Plugin author
    /// * `license` - Plugin license
    /// * `npm_dependencies` - npm dependencies to include
    ///
    /// # Returns
    /// Path to the generated package.json file
    pub fn generate_package_json(
        &self,
        plugin_dir: &Path,
        plugin_name: &str,
        plugin_version: &str,
        description: Option<&str>,
        author: Option<&str>,
        license: Option<&str>,
        npm_dependencies: &[NpmDependency],
    ) -> Result<PathBuf> {
        info!("Generating package.json for plugin: {}", plugin_name);
        
        let package_json = PackageJson::from_plugin_metadata(
            plugin_name,
            plugin_version,
            description,
            author,
            license,
            npm_dependencies,
        );
        
        let package_json_path = plugin_dir.join("package.json");
        package_json.write_to_file(&package_json_path)?;
        
        Ok(package_json_path)
    }
    
    /// Install npm dependencies for a plugin
    ///
    /// # Arguments
    /// * `plugin_dir` - Plugin directory containing package.json
    ///
    /// # Returns
    /// Result indicating success or failure
    pub fn install_dependencies(&self, plugin_dir: &Path) -> Result<()> {
        self.install_dependencies_with_name(plugin_dir, "unknown-plugin")
    }
    
    /// Install npm dependencies for a plugin with logging
    ///
    /// # Arguments
    /// * `plugin_dir` - Plugin directory containing package.json
    /// * `plugin_name` - Plugin name for logging
    ///
    /// # Returns
    /// Result indicating success or failure
    pub fn install_dependencies_with_name(&self, plugin_dir: &Path, plugin_name: &str) -> Result<()> {
        info!("Installing npm dependencies for plugin '{}' in: {}", plugin_name, plugin_dir.display());
        
        let start_time = std::time::Instant::now();
        
        // Check if package.json exists
        let package_json_path = plugin_dir.join("package.json");
        if !package_json_path.exists() {
            let error_msg = format!("package.json not found in {}", plugin_dir.display());
            self.log_installation(plugin_name, &[], false, Some(&error_msg), None)?;
            return Err(TingError::PluginLoadError(error_msg).into());
        }
        
        // Read package.json to get dependencies
        let package_json = PackageJson::read_from_file(&package_json_path)?;
        let dependencies: Vec<NpmDependency> = package_json
            .dependencies
            .iter()
            .map(|(name, version)| NpmDependency::new(name.clone(), version.clone()))
            .collect();
        
        // Validate dependencies against whitelist
        if let Err(e) = self.validate_dependencies(&dependencies) {
            let error_msg = format!("Dependency validation failed: {}", e);
            error!("{}", error_msg);
            self.log_installation(plugin_name, &dependencies, false, Some(&error_msg), None)?;
            return Err(e);
        }
        
        // Check if npm is available
        self.check_npm_available()?;
        
        // Check for package-lock.json if version locking is enforced
        if self.security_config.enforce_version_lock {
            let package_lock_path = plugin_dir.join("package-lock.json");
            if !package_lock_path.exists() {
                warn!("package-lock.json not found, version locking cannot be enforced");
                info!("Generating package-lock.json during installation");
            } else {
                info!("Using existing package-lock.json for version locking");
            }
        }
        
        // Run npm install
        debug!("Executing: npm install in {}", plugin_dir.display());
        
        let mut cmd = Command::new(&self.npm_path);
        cmd.arg("install")
            .arg("--production") // Only install production dependencies
            .arg("--no-fund") // Skip funding messages
            .current_dir(plugin_dir);
        
        // Add audit flag if disabled (default is to run audit)
        if !self.security_config.enable_audit {
            cmd.arg("--no-audit");
        }
        
        let output = cmd.output().with_context(|| {
            format!("Failed to execute npm install in {}", plugin_dir.display())
        })?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let error_msg = format!("npm install failed: {}", stderr);
            error!("{}", error_msg);
            self.log_installation(plugin_name, &dependencies, false, Some(&error_msg), None)?;
            return Err(TingError::PluginLoadError(error_msg).into());
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("npm install output: {}", stdout);
        
        // Run npm audit if enabled
        let audit_result = if self.security_config.enable_audit {
            match self.run_npm_audit(plugin_dir) {
                Ok(result) => {
                    info!("npm audit completed: {} total vulnerabilities", result.total);
                    
                    // Check if we should fail on vulnerabilities
                    if self.security_config.fail_on_audit_vulnerabilities && !result.passed {
                        let error_msg = format!(
                            "npm audit found vulnerabilities above threshold ({}): {} total",
                            self.security_config.max_vulnerability_severity.as_str(),
                            result.total
                        );
                        error!("{}", error_msg);
                        self.log_installation(plugin_name, &dependencies, false, Some(&error_msg), Some(result))?;
                        return Err(TingError::PluginLoadError(error_msg).into());
                    }
                    
                    Some(result)
                }
                Err(e) => {
                    warn!("npm audit failed: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        let elapsed = start_time.elapsed();
        info!("npm dependencies installed successfully in {:?}", elapsed);
        
        // Log successful installation
        self.log_installation(plugin_name, &dependencies, true, None, audit_result)?;
        
        Ok(())
    }
    
    /// Validate dependencies against whitelist
    ///
    /// # Arguments
    /// * `dependencies` - Dependencies to validate
    ///
    /// # Returns
    /// Result indicating success or failure
    fn validate_dependencies(&self, dependencies: &[NpmDependency]) -> Result<()> {
        // If whitelist is empty, all dependencies are allowed
        if self.security_config.whitelist.is_empty() {
            debug!("No whitelist configured, all dependencies allowed");
            return Ok(());
        }
        
        info!("Validating {} dependencies against whitelist", dependencies.len());
        
        let mut blocked_deps = Vec::new();
        
        for dep in dependencies {
            if !self.security_config.whitelist.contains(&dep.name) {
                warn!("Dependency '{}' is not in whitelist", dep.name);
                blocked_deps.push(dep.name.clone());
            }
        }
        
        if !blocked_deps.is_empty() {
            return Err(TingError::PluginLoadError(format!(
                "The following dependencies are not whitelisted: {}",
                blocked_deps.join(", ")
            ))
            .into());
        }
        
        info!("All dependencies passed whitelist validation");
        Ok(())
    }
    
    /// Run npm audit for security scanning
    ///
    /// # Arguments
    /// * `plugin_dir` - Plugin directory
    ///
    /// # Returns
    /// Audit result
    fn run_npm_audit(&self, plugin_dir: &Path) -> Result<NpmAuditResult> {
        info!("Running npm audit in: {}", plugin_dir.display());
        
        let output = Command::new(&self.npm_path)
            .arg("audit")
            .arg("--json")
            .current_dir(plugin_dir)
            .output()
            .context("Failed to execute npm audit")?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Parse audit output
        let audit_json: Value = serde_json::from_str(&stdout)
            .context("Failed to parse npm audit output")?;
        
        let mut vulnerabilities = HashMap::new();
        let mut total = 0;
        
        // Parse vulnerabilities by severity
        if let Some(metadata) = audit_json.get("metadata") {
            if let Some(vulns) = metadata.get("vulnerabilities") {
                for severity in &["low", "moderate", "high", "critical"] {
                    if let Some(count) = vulns.get(*severity).and_then(|v| v.as_u64()) {
                        let sev = VulnerabilitySeverity::from_str(severity).unwrap();
                        vulnerabilities.insert(sev, count as usize);
                        total += count as usize;
                    }
                }
            }
        }
        
        // Check if audit passed based on max severity threshold
        let passed = vulnerabilities
            .iter()
            .filter(|(sev, count)| **sev > self.security_config.max_vulnerability_severity && **count > 0)
            .count() == 0;
        
        Ok(NpmAuditResult {
            vulnerabilities,
            total,
            passed,
            raw_output: stdout.to_string(),
        })
    }
    
    /// Log dependency installation
    ///
    /// # Arguments
    /// * `plugin_name` - Plugin name
    /// * `dependencies` - Dependencies installed
    /// * `success` - Whether installation succeeded
    /// * `error` - Error message if failed
    /// * `audit_result` - Audit result if enabled
    ///
    /// # Returns
    /// Result indicating success or failure
    fn log_installation(
        &self,
        plugin_name: &str,
        dependencies: &[NpmDependency],
        success: bool,
        error: Option<&str>,
        audit_result: Option<NpmAuditResult>,
    ) -> Result<()> {
        let log_entry = DependencyInstallLog {
            timestamp: chrono::Utc::now().to_rfc3339(),
            plugin_name: plugin_name.to_string(),
            dependencies: dependencies.to_vec(),
            success,
            error: error.map(|s| s.to_string()),
            audit_result,
        };
        
        // Log to tracing
        if success {
            info!(
                plugin = plugin_name,
                dep_count = dependencies.len(),
                "Dependency installation succeeded"
            );
        } else {
            error!(
                plugin = plugin_name,
                dep_count = dependencies.len(),
                error = error.unwrap_or("unknown"),
                "Dependency installation failed"
            );
        }
        
        // Write to log file if log directory is configured
        if let Some(log_dir) = &self.log_dir {
            if !log_dir.exists() {
                std::fs::create_dir_all(log_dir)
                    .context("Failed to create log directory")?;
            }
            
            let log_file = log_dir.join(format!(
                "npm_install_{}_{}.json",
                plugin_name,
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            ));
            
            let log_json = serde_json::to_string_pretty(&log_entry)
                .context("Failed to serialize log entry")?;
            
            std::fs::write(&log_file, log_json)
                .with_context(|| format!("Failed to write log file: {}", log_file.display()))?;
            
            debug!("Installation log written to: {}", log_file.display());
        }
        
        Ok(())
    }
    
    /// Get the node_modules path for a plugin
    ///
    /// # Arguments
    /// * `plugin_dir` - Plugin directory path
    ///
    /// # Returns
    /// Path to the node_modules directory
    pub fn get_node_modules_path(&self, plugin_dir: &Path) -> PathBuf {
        plugin_dir.join("node_modules")
    }
    
    /// Check if node_modules exists for a plugin
    ///
    /// # Arguments
    /// * `plugin_dir` - Plugin directory path
    ///
    /// # Returns
    /// true if node_modules directory exists
    pub fn has_node_modules(&self, plugin_dir: &Path) -> bool {
        self.get_node_modules_path(plugin_dir).exists()
    }
    
    /// Clean node_modules directory
    ///
    /// # Arguments
    /// * `plugin_dir` - Plugin directory path
    ///
    /// # Returns
    /// Result indicating success or failure
    pub fn clean_node_modules(&self, plugin_dir: &Path) -> Result<()> {
        let node_modules_path = self.get_node_modules_path(plugin_dir);
        
        if node_modules_path.exists() {
            info!("Cleaning node_modules in: {}", plugin_dir.display());
            std::fs::remove_dir_all(&node_modules_path)
                .with_context(|| {
                    format!(
                        "Failed to remove node_modules at {}",
                        node_modules_path.display()
                    )
                })?;
            info!("node_modules cleaned successfully");
        } else {
            debug!("node_modules does not exist, nothing to clean");
        }
        
        Ok(())
    }
    
    /// Check if npm is available
    fn check_npm_available(&self) -> Result<()> {
        debug!("Checking if npm is available");
        
        let output = Command::new(&self.npm_path)
            .arg("--version")
            .output()
            .context("Failed to execute npm --version")?;
        
        if !output.status.success() {
            return Err(TingError::PluginLoadError(
                "npm is not available or not in PATH".to_string()
            )
            .into());
        }
        
        let version = String::from_utf8_lossy(&output.stdout);
        info!("npm version: {}", version.trim());
        
        Ok(())
    }
    
    // ========== Cache Management Methods ==========
    
    /// Get cache key for a dependency
    fn get_cache_key(package_name: &str, version: &str) -> String {
        format!("{}@{}", package_name, version)
    }
    
    /// Check if a dependency is cached
    ///
    /// # Arguments
    /// * `package_name` - Package name
    /// * `version` - Package version
    ///
    /// # Returns
    /// true if the dependency is cached
    pub fn is_cached(&self, package_name: &str, version: &str) -> bool {
        if self.cache_dir.is_none() {
            return false;
        }
        
        let cache_key = Self::get_cache_key(package_name, version);
        let registry = self.cache_registry.read().unwrap();
        registry.contains_key(&cache_key)
    }
    
    /// Add a dependency to cache
    ///
    /// # Arguments
    /// * `package_name` - Package name
    /// * `version` - Package version
    /// * `plugin_name` - Plugin using this dependency
    /// * `source_path` - Path to the installed package
    ///
    /// # Returns
    /// Result indicating success or failure
    fn add_to_cache(
        &self,
        package_name: &str,
        version: &str,
        plugin_name: &str,
        source_path: &Path,
    ) -> Result<()> {
        let cache_dir = match &self.cache_dir {
            Some(dir) => dir,
            None => {
                debug!("Cache directory not configured, skipping cache");
                return Ok(());
            }
        };
        
        // Create cache directory if it doesn't exist
        if !cache_dir.exists() {
            std::fs::create_dir_all(cache_dir)
                .context("Failed to create cache directory")?;
        }
        
        let cache_key = Self::get_cache_key(package_name, version);
        let cache_path = cache_dir.join(&cache_key);
        
        // Copy package to cache if not already there
        if !cache_path.exists() {
            info!("Caching dependency: {}", cache_key);
            
            // Copy the package directory
            self.copy_dir_recursive(source_path, &cache_path)?;
            
            // Calculate size
            let size_bytes = self.calculate_dir_size(&cache_path)?;
            
            // Create cache entry
            let mut used_by = HashSet::new();
            used_by.insert(plugin_name.to_string());
            
            let entry = CacheEntry {
                package_name: package_name.to_string(),
                version: version.to_string(),
                cache_path: cache_path.clone(),
                used_by,
                last_accessed: chrono::Utc::now().to_rfc3339(),
                size_bytes,
            };
            
            // Add to registry
            let mut registry = self.cache_registry.write().unwrap();
            registry.insert(cache_key.clone(), entry);
            
            // Update statistics
            let mut stats = self.cache_stats.write().unwrap();
            stats.total_packages += 1;
            stats.total_size_bytes += size_bytes;
            stats.cache_misses += 1;
            self.update_hit_rate(&mut stats);
            
            info!("Dependency cached successfully: {}", cache_key);
        } else {
            // Update existing cache entry
            let mut registry = self.cache_registry.write().unwrap();
            if let Some(entry) = registry.get_mut(&cache_key) {
                entry.used_by.insert(plugin_name.to_string());
                entry.last_accessed = chrono::Utc::now().to_rfc3339();
                
                // Update statistics
                let mut stats = self.cache_stats.write().unwrap();
                stats.cache_hits += 1;
                self.update_hit_rate(&mut stats);
                
                info!("Using cached dependency: {}", cache_key);
            }
        }
        
        Ok(())
    }
    
    /// Link cached dependency to plugin directory
    ///
    /// # Arguments
    /// * `package_name` - Package name
    /// * `version` - Package version
    /// * `plugin_name` - Plugin name
    /// * `target_path` - Target path in plugin's node_modules
    ///
    /// # Returns
    /// Result indicating success or failure
    fn link_from_cache(
        &self,
        package_name: &str,
        version: &str,
        plugin_name: &str,
        target_path: &Path,
    ) -> Result<()> {
        let cache_key = Self::get_cache_key(package_name, version);
        
        let registry = self.cache_registry.read().unwrap();
        let entry = registry.get(&cache_key).ok_or_else(|| {
            TingError::PluginLoadError(format!("Dependency not found in cache: {}", cache_key))
        })?;
        
        info!("Linking cached dependency {} to {}", cache_key, target_path.display());
        
        // Create parent directory if needed
        if let Some(parent) = target_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }
        
        // Copy from cache to target
        self.copy_dir_recursive(&entry.cache_path, target_path)?;
        
        // Update last accessed time
        drop(registry);
        let mut registry = self.cache_registry.write().unwrap();
        if let Some(entry) = registry.get_mut(&cache_key) {
            entry.used_by.insert(plugin_name.to_string());
            entry.last_accessed = chrono::Utc::now().to_rfc3339();
        }
        
        // Update statistics
        let mut stats = self.cache_stats.write().unwrap();
        stats.cache_hits += 1;
        self.update_hit_rate(&mut stats);
        
        Ok(())
    }
    
    /// Install dependencies with caching support
    ///
    /// # Arguments
    /// * `plugin_dir` - Plugin directory
    /// * `plugin_name` - Plugin name
    /// * `dependencies` - Dependencies to install
    ///
    /// # Returns
    /// Result indicating success or failure
    pub fn install_dependencies_with_cache(
        &self,
        plugin_dir: &Path,
        plugin_name: &str,
        dependencies: &[NpmDependency],
    ) -> Result<()> {
        if dependencies.is_empty() {
            debug!("No dependencies to install for plugin: {}", plugin_name);
            return Ok(());
        }
        
        info!("Installing {} dependencies for plugin '{}' with caching", dependencies.len(), plugin_name);
        
        // Check if cache is enabled
        if self.cache_dir.is_none() {
            debug!("Cache not enabled, falling back to regular installation");
            return self.install_dependencies_with_name(plugin_dir, plugin_name);
        }
        
        // Create node_modules directory
        let node_modules_path = self.get_node_modules_path(plugin_dir);
        if !node_modules_path.exists() {
            std::fs::create_dir_all(&node_modules_path)?;
        }
        
        // Try to use cached dependencies first
        let mut uncached_deps = Vec::new();
        
        for dep in dependencies {
            let cache_key = Self::get_cache_key(&dep.name, &dep.version);
            
            if self.is_cached(&dep.name, &dep.version) {
                // Link from cache
                let target_path = node_modules_path.join(&dep.name);
                match self.link_from_cache(&dep.name, &dep.version, plugin_name, &target_path) {
                    Ok(_) => {
                        info!("Linked cached dependency: {}", cache_key);
                    }
                    Err(e) => {
                        warn!("Failed to link cached dependency {}: {}", cache_key, e);
                        uncached_deps.push(dep.clone());
                    }
                }
            } else {
                uncached_deps.push(dep.clone());
            }
        }
        
        // Install uncached dependencies
        if !uncached_deps.is_empty() {
            info!("Installing {} uncached dependencies", uncached_deps.len());
            
            // Generate temporary package.json for uncached deps
            let temp_package_json = PackageJson::from_plugin_metadata(
                plugin_name,
                "1.0.0",
                None,
                None,
                None,
                &uncached_deps,
            );
            
            let package_json_path = plugin_dir.join("package.json");
            temp_package_json.write_to_file(&package_json_path)?;
            
            // Install using npm
            self.install_dependencies_with_name(plugin_dir, plugin_name)?;
            
            // Add newly installed packages to cache
            for dep in &uncached_deps {
                let installed_path = node_modules_path.join(&dep.name);
                if installed_path.exists() {
                    if let Err(e) = self.add_to_cache(&dep.name, &dep.version, plugin_name, &installed_path) {
                        warn!("Failed to cache dependency {}: {}", dep.name, e);
                    }
                }
            }
        }
        
        info!("All dependencies installed successfully for plugin: {}", plugin_name);
        Ok(())
    }
    
    /// Clean unused dependencies from cache
    ///
    /// # Arguments
    /// * `plugin_name` - Plugin name to remove from cache usage tracking
    ///
    /// # Returns
    /// Number of packages removed from cache
    pub fn cleanup_cache_for_plugin(&self, plugin_name: &str) -> Result<usize> {
        if self.cache_dir.is_none() {
            return Ok(0);
        }
        
        info!("Cleaning up cache for plugin: {}", plugin_name);
        
        let mut removed_count = 0;
        let mut packages_to_remove = Vec::new();
        
        // Find packages only used by this plugin
        {
            let mut registry = self.cache_registry.write().unwrap();
            
            for (cache_key, entry) in registry.iter_mut() {
                entry.used_by.remove(plugin_name);
                
                if entry.used_by.is_empty() {
                    packages_to_remove.push((cache_key.clone(), entry.cache_path.clone(), entry.size_bytes));
                }
            }
        }
        
        // Remove unused packages
        for (cache_key, cache_path, size_bytes) in packages_to_remove {
            info!("Removing unused cached package: {}", cache_key);
            
            if cache_path.exists() {
                std::fs::remove_dir_all(&cache_path)
                    .with_context(|| format!("Failed to remove cached package at {}", cache_path.display()))?;
            }
            
            // Remove from registry
            let mut registry = self.cache_registry.write().unwrap();
            registry.remove(&cache_key);
            
            // Update statistics
            let mut stats = self.cache_stats.write().unwrap();
            stats.total_packages = stats.total_packages.saturating_sub(1);
            stats.total_size_bytes = stats.total_size_bytes.saturating_sub(size_bytes);
            
            removed_count += 1;
        }
        
        if removed_count > 0 {
            let mut stats = self.cache_stats.write().unwrap();
            stats.last_cleanup = Some(chrono::Utc::now().to_rfc3339());
            info!("Removed {} unused packages from cache", removed_count);
        } else {
            debug!("No unused packages found for plugin: {}", plugin_name);
        }
        
        Ok(removed_count)
    }
    
    /// Clean all unused dependencies from cache
    ///
    /// # Returns
    /// Number of packages removed from cache
    pub fn cleanup_all_unused(&self) -> Result<usize> {
        if self.cache_dir.is_none() {
            return Ok(0);
        }
        
        info!("Cleaning up all unused cached packages");
        
        let mut removed_count = 0;
        let mut packages_to_remove = Vec::new();
        
        // Find packages with no users
        {
            let registry = self.cache_registry.read().unwrap();
            
            for (cache_key, entry) in registry.iter() {
                if entry.used_by.is_empty() {
                    packages_to_remove.push((cache_key.clone(), entry.cache_path.clone(), entry.size_bytes));
                }
            }
        }
        
        // Remove unused packages
        for (cache_key, cache_path, size_bytes) in packages_to_remove {
            info!("Removing unused cached package: {}", cache_key);
            
            if cache_path.exists() {
                std::fs::remove_dir_all(&cache_path)
                    .with_context(|| format!("Failed to remove cached package at {}", cache_path.display()))?;
            }
            
            // Remove from registry
            let mut registry = self.cache_registry.write().unwrap();
            registry.remove(&cache_key);
            
            // Update statistics
            let mut stats = self.cache_stats.write().unwrap();
            stats.total_packages = stats.total_packages.saturating_sub(1);
            stats.total_size_bytes = stats.total_size_bytes.saturating_sub(size_bytes);
            
            removed_count += 1;
        }
        
        if removed_count > 0 {
            let mut stats = self.cache_stats.write().unwrap();
            stats.last_cleanup = Some(chrono::Utc::now().to_rfc3339());
            info!("Removed {} unused packages from cache", removed_count);
        } else {
            info!("No unused packages found in cache");
        }
        
        Ok(removed_count)
    }
    
    /// Get cache statistics
    ///
    /// # Returns
    /// Cache statistics
    pub fn get_cache_statistics(&self) -> CacheStatistics {
        let stats = self.cache_stats.read().unwrap();
        
        // Count unique plugins using cache
        let registry = self.cache_registry.read().unwrap();
        let mut all_plugins = HashSet::new();
        for entry in registry.values() {
            all_plugins.extend(entry.used_by.iter().cloned());
        }
        
        CacheStatistics {
            total_packages: stats.total_packages,
            total_size_bytes: stats.total_size_bytes,
            cache_hits: stats.cache_hits,
            cache_misses: stats.cache_misses,
            hit_rate: stats.hit_rate,
            plugins_count: all_plugins.len(),
            last_cleanup: stats.last_cleanup.clone(),
        }
    }
    
    /// Clear all cache
    ///
    /// # Returns
    /// Result indicating success or failure
    pub fn clear_cache(&self) -> Result<()> {
        let cache_dir = match &self.cache_dir {
            Some(dir) => dir,
            None => return Ok(()),
        };
        
        info!("Clearing all cache");
        
        if cache_dir.exists() {
            std::fs::remove_dir_all(cache_dir)
                .context("Failed to remove cache directory")?;
            std::fs::create_dir_all(cache_dir)
                .context("Failed to recreate cache directory")?;
        }
        
        // Clear registry and statistics
        {
            let mut registry = self.cache_registry.write().unwrap();
            registry.clear();
        }
        
        {
            let mut stats = self.cache_stats.write().unwrap();
            stats.total_packages = 0;
            stats.total_size_bytes = 0;
            stats.cache_hits = 0;
            stats.cache_misses = 0;
            stats.hit_rate = 0.0;
            stats.plugins_count = 0;
            stats.last_cleanup = Some(chrono::Utc::now().to_rfc3339());
        }
        
        info!("Cache cleared successfully");
        Ok(())
    }
    
    // ========== Helper Methods ==========
    
    /// Update hit rate in statistics
    fn update_hit_rate(&self, stats: &mut CacheStatistics) {
        let total = stats.cache_hits + stats.cache_misses;
        if total > 0 {
            stats.hit_rate = stats.cache_hits as f64 / total as f64;
        } else {
            stats.hit_rate = 0.0;
        }
    }
    
    /// Copy directory recursively
    fn copy_dir_recursive(&self, src: &Path, dst: &Path) -> Result<()> {
        if !dst.exists() {
            std::fs::create_dir_all(dst)?;
        }
        
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            
            if file_type.is_dir() {
                self.copy_dir_recursive(&src_path, &dst_path)?;
            } else {
                std::fs::copy(&src_path, &dst_path)?;
            }
        }
        
        Ok(())
    }
    
    /// Calculate directory size recursively
    fn calculate_dir_size(&self, path: &Path) -> Result<u64> {
        let mut total_size = 0u64;
        
        if path.is_file() {
            return Ok(std::fs::metadata(path)?.len());
        }
        
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();
            
            if entry_path.is_dir() {
                total_size += self.calculate_dir_size(&entry_path)?;
            } else {
                total_size += std::fs::metadata(&entry_path)?.len();
            }
        }
        
        Ok(total_size)
    }
}

impl Default for NpmManager {
    fn default() -> Self {
        Self::new(None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_npm_dependency_creation() {
        let dep = NpmDependency::new("axios".to_string(), "^1.6.0".to_string());
        assert_eq!(dep.name, "axios");
        assert_eq!(dep.version, "^1.6.0");
    }
    
    #[test]
    fn test_vulnerability_severity_ordering() {
        assert!(VulnerabilitySeverity::Low < VulnerabilitySeverity::Moderate);
        assert!(VulnerabilitySeverity::Moderate < VulnerabilitySeverity::High);
        assert!(VulnerabilitySeverity::High < VulnerabilitySeverity::Critical);
    }
    
    #[test]
    fn test_vulnerability_severity_from_str() {
        assert_eq!(VulnerabilitySeverity::from_str("low"), Some(VulnerabilitySeverity::Low));
        assert_eq!(VulnerabilitySeverity::from_str("moderate"), Some(VulnerabilitySeverity::Moderate));
        assert_eq!(VulnerabilitySeverity::from_str("high"), Some(VulnerabilitySeverity::High));
        assert_eq!(VulnerabilitySeverity::from_str("critical"), Some(VulnerabilitySeverity::Critical));
        assert_eq!(VulnerabilitySeverity::from_str("invalid"), None);
    }
    
    #[test]
    fn test_security_config_default() {
        let config = NpmSecurityConfig::default();
        assert!(config.whitelist.is_empty());
        assert!(config.enforce_version_lock);
        assert!(!config.enable_audit);
        assert!(!config.fail_on_audit_vulnerabilities);
        assert_eq!(config.max_vulnerability_severity, VulnerabilitySeverity::High);
    }
    
    #[test]
    fn test_security_config_with_whitelist() {
        let mut whitelist = HashSet::new();
        whitelist.insert("axios".to_string());
        whitelist.insert("cheerio".to_string());
        
        let config = NpmSecurityConfig {
            whitelist,
            enforce_version_lock: true,
            enable_audit: true,
            fail_on_audit_vulnerabilities: true,
            max_vulnerability_severity: VulnerabilitySeverity::Moderate,
        };
        
        assert_eq!(config.whitelist.len(), 2);
        assert!(config.whitelist.contains("axios"));
        assert!(config.whitelist.contains("cheerio"));
    }
    
    #[test]
    fn test_parse_dependencies_from_json() {
        let plugin_json = serde_json::json!({
            "name": "test-plugin",
            "npm_dependencies": {
                "axios": "^1.6.0",
                "cheerio": "^1.0.0"
            }
        });
        
        let deps = NpmManager::parse_dependencies(&plugin_json);
        assert_eq!(deps.len(), 2);
        
        // Dependencies might be in any order
        let dep_names: Vec<String> = deps.iter().map(|d| d.name.clone()).collect();
        assert!(dep_names.contains(&"axios".to_string()));
        assert!(dep_names.contains(&"cheerio".to_string()));
    }
    
    #[test]
    fn test_parse_dependencies_empty() {
        let plugin_json = serde_json::json!({
            "name": "test-plugin"
        });
        
        let deps = NpmManager::parse_dependencies(&plugin_json);
        assert_eq!(deps.len(), 0);
    }
    
    #[test]
    fn test_package_json_creation() {
        let deps = vec![
            NpmDependency::new("axios".to_string(), "^1.6.0".to_string()),
            NpmDependency::new("cheerio".to_string(), "^1.0.0".to_string()),
        ];
        
        let package_json = PackageJson::from_plugin_metadata(
            "test-plugin",
            "1.0.0",
            Some("Test plugin"),
            Some("Test Author"),
            Some("MIT"),
            &deps,
        );
        
        assert_eq!(package_json.name, "test-plugin");
        assert_eq!(package_json.version, "1.0.0");
        assert_eq!(package_json.description, Some("Test plugin".to_string()));
        assert_eq!(package_json.author, Some("Test Author".to_string()));
        assert_eq!(package_json.license, Some("MIT".to_string()));
        assert_eq!(package_json.dependencies.len(), 2);
        assert_eq!(package_json.dependencies.get("axios"), Some(&"^1.6.0".to_string()));
        assert_eq!(package_json.dependencies.get("cheerio"), Some(&"^1.0.0".to_string()));
        assert!(package_json.private);
    }
    
    #[test]
    fn test_package_json_write_and_read() {
        let temp_dir = TempDir::new().unwrap();
        let package_json_path = temp_dir.path().join("package.json");
        
        let deps = vec![
            NpmDependency::new("axios".to_string(), "^1.6.0".to_string()),
        ];
        
        let package_json = PackageJson::from_plugin_metadata(
            "test-plugin",
            "1.0.0",
            Some("Test plugin"),
            Some("Test Author"),
            Some("MIT"),
            &deps,
        );
        
        // Write
        package_json.write_to_file(&package_json_path).unwrap();
        assert!(package_json_path.exists());
        
        // Read
        let read_package_json = PackageJson::read_from_file(&package_json_path).unwrap();
        assert_eq!(read_package_json.name, "test-plugin");
        assert_eq!(read_package_json.version, "1.0.0");
        assert_eq!(read_package_json.dependencies.len(), 1);
    }
    
    #[test]
    fn test_npm_manager_creation() {
        let manager = NpmManager::default();
        assert_eq!(manager.npm_path, PathBuf::from("npm"));
        assert!(manager.cache_dir.is_none());
        assert!(manager.log_dir.is_none());
    }
    
    #[test]
    fn test_npm_manager_with_custom_path() {
        let custom_path = PathBuf::from("/usr/local/bin/npm");
        let manager = NpmManager::new(Some(custom_path.clone()), None);
        assert_eq!(manager.npm_path, custom_path);
    }
    
    #[test]
    fn test_npm_manager_with_security() {
        let mut whitelist = HashSet::new();
        whitelist.insert("axios".to_string());
        
        let security_config = NpmSecurityConfig {
            whitelist,
            enforce_version_lock: true,
            enable_audit: true,
            fail_on_audit_vulnerabilities: false,
            max_vulnerability_severity: VulnerabilitySeverity::High,
        };
        
        let log_dir = PathBuf::from("/tmp/npm_logs");
        let manager = NpmManager::with_security(None, None, security_config.clone(), Some(log_dir.clone()));
        
        assert_eq!(manager.security_config.whitelist.len(), 1);
        assert!(manager.security_config.enable_audit);
        assert_eq!(manager.log_dir, Some(log_dir));
    }
    
    #[test]
    fn test_validate_dependencies_no_whitelist() {
        let manager = NpmManager::default();
        
        let deps = vec![
            NpmDependency::new("axios".to_string(), "^1.6.0".to_string()),
            NpmDependency::new("cheerio".to_string(), "^1.0.0".to_string()),
        ];
        
        // Should pass when whitelist is empty
        let result = manager.validate_dependencies(&deps);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_validate_dependencies_with_whitelist_pass() {
        let mut whitelist = HashSet::new();
        whitelist.insert("axios".to_string());
        whitelist.insert("cheerio".to_string());
        
        let security_config = NpmSecurityConfig {
            whitelist,
            ..Default::default()
        };
        
        let mut manager = NpmManager::default();
        manager.set_security_config(security_config);
        
        let deps = vec![
            NpmDependency::new("axios".to_string(), "^1.6.0".to_string()),
            NpmDependency::new("cheerio".to_string(), "^1.0.0".to_string()),
        ];
        
        // Should pass when all deps are whitelisted
        let result = manager.validate_dependencies(&deps);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_validate_dependencies_with_whitelist_fail() {
        let mut whitelist = HashSet::new();
        whitelist.insert("axios".to_string());
        
        let security_config = NpmSecurityConfig {
            whitelist,
            ..Default::default()
        };
        
        let mut manager = NpmManager::default();
        manager.set_security_config(security_config);
        
        let deps = vec![
            NpmDependency::new("axios".to_string(), "^1.6.0".to_string()),
            NpmDependency::new("malicious-package".to_string(), "^1.0.0".to_string()),
        ];
        
        // Should fail when a dep is not whitelisted
        let result = manager.validate_dependencies(&deps);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not whitelisted"));
    }
    
    #[test]
    fn test_generate_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let manager = NpmManager::default();
        
        let deps = vec![
            NpmDependency::new("axios".to_string(), "^1.6.0".to_string()),
        ];
        
        let result = manager.generate_package_json(
            temp_dir.path(),
            "test-plugin",
            "1.0.0",
            Some("Test plugin"),
            Some("Test Author"),
            Some("MIT"),
            &deps,
        );
        
        assert!(result.is_ok());
        let package_json_path = result.unwrap();
        assert!(package_json_path.exists());
        assert_eq!(package_json_path, temp_dir.path().join("package.json"));
    }
    
    #[test]
    fn test_get_node_modules_path() {
        let manager = NpmManager::default();
        let plugin_dir = PathBuf::from("/path/to/plugin");
        let node_modules_path = manager.get_node_modules_path(&plugin_dir);
        assert_eq!(node_modules_path, PathBuf::from("/path/to/plugin/node_modules"));
    }
    
    #[test]
    fn test_has_node_modules() {
        let temp_dir = TempDir::new().unwrap();
        let manager = NpmManager::default();
        
        // Initially no node_modules
        assert!(!manager.has_node_modules(temp_dir.path()));
        
        // Create node_modules directory
        let node_modules_path = temp_dir.path().join("node_modules");
        std::fs::create_dir(&node_modules_path).unwrap();
        
        // Now it should exist
        assert!(manager.has_node_modules(temp_dir.path()));
    }
    
    #[test]
    fn test_clean_node_modules() {
        let temp_dir = TempDir::new().unwrap();
        let manager = NpmManager::default();
        
        // Create node_modules directory with a file
        let node_modules_path = temp_dir.path().join("node_modules");
        std::fs::create_dir(&node_modules_path).unwrap();
        std::fs::write(node_modules_path.join("test.txt"), "test").unwrap();
        
        // Clean
        let result = manager.clean_node_modules(temp_dir.path());
        assert!(result.is_ok());
        assert!(!manager.has_node_modules(temp_dir.path()));
    }
    
    #[test]
    fn test_clean_node_modules_not_exists() {
        let temp_dir = TempDir::new().unwrap();
        let manager = NpmManager::default();
        
        // Clean when node_modules doesn't exist (should not error)
        let result = manager.clean_node_modules(temp_dir.path());
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_dependency_install_log_serialization() {
        let deps = vec![
            NpmDependency::new("axios".to_string(), "^1.6.0".to_string()),
        ];
        
        let log = DependencyInstallLog {
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            plugin_name: "test-plugin".to_string(),
            dependencies: deps,
            success: true,
            error: None,
            audit_result: None,
        };
        
        let json = serde_json::to_string(&log).unwrap();
        assert!(json.contains("test-plugin"));
        assert!(json.contains("axios"));
        
        let deserialized: DependencyInstallLog = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.plugin_name, "test-plugin");
        assert!(deserialized.success);
    }
    
    // ========== Cache Tests ==========
    
    #[test]
    fn test_cache_key_generation() {
        let key = NpmManager::get_cache_key("axios", "1.6.0");
        assert_eq!(key, "axios@1.6.0");
    }
    
    #[test]
    fn test_cache_entry_creation() {
        let mut used_by = HashSet::new();
        used_by.insert("plugin1".to_string());
        
        let entry = CacheEntry {
            package_name: "axios".to_string(),
            version: "1.6.0".to_string(),
            cache_path: PathBuf::from("/cache/axios@1.6.0"),
            used_by,
            last_accessed: "2024-01-01T00:00:00Z".to_string(),
            size_bytes: 1024,
        };
        
        assert_eq!(entry.package_name, "axios");
        assert_eq!(entry.version, "1.6.0");
        assert_eq!(entry.used_by.len(), 1);
        assert!(entry.used_by.contains("plugin1"));
    }
    
    #[test]
    fn test_cache_statistics_default() {
        let stats = CacheStatistics {
            total_packages: 0,
            total_size_bytes: 0,
            cache_hits: 0,
            cache_misses: 0,
            hit_rate: 0.0,
            plugins_count: 0,
            last_cleanup: None,
        };
        
        assert_eq!(stats.total_packages, 0);
        assert_eq!(stats.hit_rate, 0.0);
    }
    
    #[test]
    fn test_cache_statistics_hit_rate() {
        let stats = CacheStatistics {
            total_packages: 5,
            total_size_bytes: 5120,
            cache_hits: 8,
            cache_misses: 2,
            hit_rate: 0.8,
            plugins_count: 3,
            last_cleanup: None,
        };
        
        assert_eq!(stats.cache_hits, 8);
        assert_eq!(stats.cache_misses, 2);
        assert_eq!(stats.hit_rate, 0.8);
    }
    
    #[test]
    fn test_is_cached_no_cache_dir() {
        let manager = NpmManager::new(None, None);
        assert!(!manager.is_cached("axios", "1.6.0"));
    }
    
    #[test]
    fn test_is_cached_with_cache_dir() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("cache");
        
        let manager = NpmManager::new(None, Some(cache_dir));
        
        // Initially not cached
        assert!(!manager.is_cached("axios", "1.6.0"));
        
        // Add to cache registry manually for testing
        let cache_key = NpmManager::get_cache_key("axios", "1.6.0");
        let mut used_by = HashSet::new();
        used_by.insert("plugin1".to_string());
        
        let entry = CacheEntry {
            package_name: "axios".to_string(),
            version: "1.6.0".to_string(),
            cache_path: temp_dir.path().join("axios@1.6.0"),
            used_by,
            last_accessed: chrono::Utc::now().to_rfc3339(),
            size_bytes: 1024,
        };
        
        {
            let mut registry = manager.cache_registry.write().unwrap();
            registry.insert(cache_key, entry);
        }
        
        // Now it should be cached
        assert!(manager.is_cached("axios", "1.6.0"));
    }
    
    #[test]
    fn test_get_cache_statistics() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("cache");
        
        let manager = NpmManager::new(None, Some(cache_dir));
        
        // Initial statistics
        let stats = manager.get_cache_statistics();
        assert_eq!(stats.total_packages, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.cache_misses, 0);
        assert_eq!(stats.hit_rate, 0.0);
        
        // Add some cache entries
        let cache_key1 = NpmManager::get_cache_key("axios", "1.6.0");
        let cache_key2 = NpmManager::get_cache_key("cheerio", "1.0.0");
        
        let mut used_by1 = HashSet::new();
        used_by1.insert("plugin1".to_string());
        
        let mut used_by2 = HashSet::new();
        used_by2.insert("plugin1".to_string());
        used_by2.insert("plugin2".to_string());
        
        {
            let mut registry = manager.cache_registry.write().unwrap();
            registry.insert(cache_key1, CacheEntry {
                package_name: "axios".to_string(),
                version: "1.6.0".to_string(),
                cache_path: temp_dir.path().join("axios@1.6.0"),
                used_by: used_by1,
                last_accessed: chrono::Utc::now().to_rfc3339(),
                size_bytes: 1024,
            });
            registry.insert(cache_key2, CacheEntry {
                package_name: "cheerio".to_string(),
                version: "1.0.0".to_string(),
                cache_path: temp_dir.path().join("cheerio@1.0.0"),
                used_by: used_by2,
                last_accessed: chrono::Utc::now().to_rfc3339(),
                size_bytes: 2048,
            });
        }
        
        {
            let mut stats = manager.cache_stats.write().unwrap();
            stats.total_packages = 2;
            stats.total_size_bytes = 3072;
            stats.cache_hits = 3;
            stats.cache_misses = 1;
            stats.hit_rate = 0.75;
        }
        
        // Get statistics
        let stats = manager.get_cache_statistics();
        assert_eq!(stats.total_packages, 2);
        assert_eq!(stats.total_size_bytes, 3072);
        assert_eq!(stats.cache_hits, 3);
        assert_eq!(stats.cache_misses, 1);
        assert_eq!(stats.hit_rate, 0.75);
        assert_eq!(stats.plugins_count, 2); // plugin1 and plugin2
    }
    
    #[test]
    fn test_clear_cache() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("cache");
        std::fs::create_dir_all(&cache_dir).unwrap();
        
        let manager = NpmManager::new(None, Some(cache_dir.clone()));
        
        // Add some cache entries
        {
            let mut registry = manager.cache_registry.write().unwrap();
            let mut used_by = HashSet::new();
            used_by.insert("plugin1".to_string());
            
            registry.insert("axios@1.6.0".to_string(), CacheEntry {
                package_name: "axios".to_string(),
                version: "1.6.0".to_string(),
                cache_path: cache_dir.join("axios@1.6.0"),
                used_by,
                last_accessed: chrono::Utc::now().to_rfc3339(),
                size_bytes: 1024,
            });
        }
        
        {
            let mut stats = manager.cache_stats.write().unwrap();
            stats.total_packages = 1;
            stats.total_size_bytes = 1024;
        }
        
        // Clear cache
        let result = manager.clear_cache();
        assert!(result.is_ok());
        
        // Verify cache is cleared
        let registry = manager.cache_registry.read().unwrap();
        assert_eq!(registry.len(), 0);
        
        let stats = manager.cache_stats.read().unwrap();
        assert_eq!(stats.total_packages, 0);
        assert_eq!(stats.total_size_bytes, 0);
        assert!(stats.last_cleanup.is_some());
    }
    
    #[test]
    fn test_calculate_dir_size() {
        let temp_dir = TempDir::new().unwrap();
        let manager = NpmManager::default();
        
        // Create some files
        std::fs::write(temp_dir.path().join("file1.txt"), "hello").unwrap();
        std::fs::write(temp_dir.path().join("file2.txt"), "world").unwrap();
        
        let subdir = temp_dir.path().join("subdir");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(subdir.join("file3.txt"), "test").unwrap();
        
        let size = manager.calculate_dir_size(temp_dir.path()).unwrap();
        assert_eq!(size, 14); // "hello" (5) + "world" (5) + "test" (4)
    }
    
    #[test]
    fn test_copy_dir_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let manager = NpmManager::default();
        
        // Create source directory structure
        let src_dir = temp_dir.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();
        std::fs::write(src_dir.join("file1.txt"), "content1").unwrap();
        
        let src_subdir = src_dir.join("subdir");
        std::fs::create_dir(&src_subdir).unwrap();
        std::fs::write(src_subdir.join("file2.txt"), "content2").unwrap();
        
        // Copy to destination
        let dst_dir = temp_dir.path().join("dst");
        let result = manager.copy_dir_recursive(&src_dir, &dst_dir);
        assert!(result.is_ok());
        
        // Verify files were copied
        assert!(dst_dir.join("file1.txt").exists());
        assert!(dst_dir.join("subdir").join("file2.txt").exists());
        
        let content1 = std::fs::read_to_string(dst_dir.join("file1.txt")).unwrap();
        assert_eq!(content1, "content1");
        
        let content2 = std::fs::read_to_string(dst_dir.join("subdir").join("file2.txt")).unwrap();
        assert_eq!(content2, "content2");
    }
    
    #[test]
    fn test_cleanup_all_unused() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("cache");
        std::fs::create_dir_all(&cache_dir).unwrap();
        
        let manager = NpmManager::new(None, Some(cache_dir.clone()));
        
        // Add cache entries - one with users, one without
        let cache_path1 = cache_dir.join("axios@1.6.0");
        let cache_path2 = cache_dir.join("cheerio@1.0.0");
        std::fs::create_dir_all(&cache_path1).unwrap();
        std::fs::create_dir_all(&cache_path2).unwrap();
        
        {
            let mut registry = manager.cache_registry.write().unwrap();
            
            let mut used_by1 = HashSet::new();
            used_by1.insert("plugin1".to_string());
            
            registry.insert("axios@1.6.0".to_string(), CacheEntry {
                package_name: "axios".to_string(),
                version: "1.6.0".to_string(),
                cache_path: cache_path1.clone(),
                used_by: used_by1,
                last_accessed: chrono::Utc::now().to_rfc3339(),
                size_bytes: 1024,
            });
            
            registry.insert("cheerio@1.0.0".to_string(), CacheEntry {
                package_name: "cheerio".to_string(),
                version: "1.0.0".to_string(),
                cache_path: cache_path2.clone(),
                used_by: HashSet::new(), // No users
                last_accessed: chrono::Utc::now().to_rfc3339(),
                size_bytes: 2048,
            });
        }
        
        {
            let mut stats = manager.cache_stats.write().unwrap();
            stats.total_packages = 2;
            stats.total_size_bytes = 3072;
        }
        
        // Cleanup unused
        let removed = manager.cleanup_all_unused().unwrap();
        assert_eq!(removed, 1);
        
        // Verify only unused package was removed
        let registry = manager.cache_registry.read().unwrap();
        assert_eq!(registry.len(), 1);
        assert!(registry.contains_key("axios@1.6.0"));
        assert!(!registry.contains_key("cheerio@1.0.0"));
        
        assert!(cache_path1.exists());
        assert!(!cache_path2.exists());
        
        let stats = manager.cache_stats.read().unwrap();
        assert_eq!(stats.total_packages, 1);
        assert_eq!(stats.total_size_bytes, 1024);
        assert!(stats.last_cleanup.is_some());
    }
}
