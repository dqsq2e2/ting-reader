//! npm Dependency Manager
//!
//! Handles npm dependency resolution and installation for JavaScript plugins.
//! - Parse npm dependencies from plugin.json
//! - Generate package.json files
//! - Execute npm install commands
//! - Manage node_modules paths
//! - Cache dependencies across plugins

mod cache;
mod package_json;
mod security;
#[cfg(test)]
mod tests;

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, RwLock};
use tracing::{debug, error, info, warn};

use crate::core::error::TingError;

pub use cache::{CacheEntry, CacheStatistics};
pub use package_json::PackageJson;
pub use security::{NpmAuditResult, NpmDependency, NpmSecurityConfig, VulnerabilitySeverity};

/// Dependency installation log entry
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DependencyInstallLog {
    pub timestamp: String,
    pub plugin_name: String,
    pub dependencies: Vec<NpmDependency>,
    pub success: bool,
    pub error: Option<String>,
    pub audit_result: Option<NpmAuditResult>,
}

/// npm dependency manager
pub struct NpmManager {
    npm_path: PathBuf,
    cache_dir: Option<PathBuf>,
    security_config: NpmSecurityConfig,
    log_dir: Option<PathBuf>,
    cache_registry: cache::CacheRegistry,
    cache_stats: cache::CacheStatsLock,
}

impl NpmManager {
    pub fn new(npm_path: Option<PathBuf>, cache_dir: Option<PathBuf>) -> Self {
        let npm_path = npm_path.unwrap_or_else(|| PathBuf::from("npm"));
        Self {
            npm_path,
            cache_dir,
            security_config: NpmSecurityConfig::default(),
            log_dir: None,
            cache_registry: Arc::new(RwLock::new(std::collections::HashMap::new())),
            cache_stats: Arc::new(RwLock::new(CacheStatistics::default())),
        }
    }

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
            cache_registry: Arc::new(RwLock::new(std::collections::HashMap::new())),
            cache_stats: Arc::new(RwLock::new(CacheStatistics::default())),
        }
    }

    pub fn set_security_config(&mut self, config: NpmSecurityConfig) {
        self.security_config = config;
    }

    pub fn set_log_dir(&mut self, log_dir: PathBuf) {
        self.log_dir = Some(log_dir);
    }

    /// Parse npm dependencies from plugin.json (static method)
    pub fn parse_dependencies(plugin_json: &Value) -> Vec<NpmDependency> {
        let mut dependencies = Vec::new();

        if let Some(npm_deps) = plugin_json.get("npm_dependencies") {
            if let Some(deps_obj) = npm_deps.as_object() {
                for (name, version) in deps_obj {
                    if let Some(version_str) = version.as_str() {
                        dependencies.push(NpmDependency::new(name.clone(), version_str.to_string()));
                    } else {
                        warn!("npm dependency version format invalid {}: {:?}", name, version);
                    }
                }
            } else if let Some(deps_array) = npm_deps.as_array() {
                for dep in deps_array {
                    if let Some(dep_obj) = dep.as_object() {
                        if let (Some(name), Some(version)) = (
                            dep_obj.get("name").and_then(|v| v.as_str()),
                            dep_obj.get("version").and_then(|v| v.as_str()),
                        ) {
                            dependencies.push(NpmDependency::new(name.to_string(), version.to_string()));
                        } else {
                            warn!("npm dependency missing name or version: {:?}", dep);
                        }
                    } else {
                        warn!("npm dependency array element is not an object: {:?}", dep);
                    }
                }
            } else {
                warn!("npm_dependencies field has invalid format, expected object or array");
            }
        }

        debug!("Parsed {} npm dependencies", dependencies.len());
        dependencies
    }

    /// Generate package.json for a plugin
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
        package_json::generate_package_json(
            plugin_dir, plugin_name, plugin_version, description, author, license, npm_dependencies,
        )
    }

    /// Install npm dependencies for a plugin
    pub fn install_dependencies(&self, plugin_dir: &Path) -> Result<()> {
        self.install_dependencies_with_name(plugin_dir, "unknown-plugin")
    }

    /// Install npm dependencies for a plugin with logging
    pub fn install_dependencies_with_name(&self, plugin_dir: &Path, plugin_name: &str) -> Result<()> {
        info!("Installing npm dependencies for plugin '{}' in: {}", plugin_name, plugin_dir.display());
        let start_time = std::time::Instant::now();

        let package_json_path = plugin_dir.join("package.json");
        if !package_json_path.exists() {
            let error_msg = format!("package.json not found in {}", plugin_dir.display());
            self.log_installation(plugin_name, &[], false, Some(&error_msg), None)?;
            return Err(TingError::PluginLoadError(error_msg).into());
        }

        let package_json = PackageJson::read_from_file(&package_json_path)?;
        let dependencies: Vec<NpmDependency> = package_json
            .dependencies.iter()
            .map(|(name, version)| NpmDependency::new(name.clone(), version.clone()))
            .collect();

        if let Err(e) = self.validate_dependencies(&dependencies) {
            let error_msg = format!("Dependency validation failed: {}", e);
            error!("{}", error_msg);
            self.log_installation(plugin_name, &dependencies, false, Some(&error_msg), None)?;
            return Err(e);
        }

        self.check_npm_available()?;

        if self.security_config.enforce_version_lock {
            let package_lock_path = plugin_dir.join("package-lock.json");
            if !package_lock_path.exists() {
                warn!("package-lock.json not found, version locking cannot be enforced");
            } else {
                info!("Using existing package-lock.json for version locking");
            }
        }

        debug!("Executing: npm install in {}", plugin_dir.display());
        let mut cmd = Command::new(&self.npm_path);
        cmd.arg("install").arg("--production").arg("--no-fund").current_dir(plugin_dir);
        if !self.security_config.enable_audit {
            cmd.arg("--no-audit");
        }

        let output = cmd.output()
            .with_context(|| format!("Failed to execute npm install in {}", plugin_dir.display()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let error_msg = format!("npm install failed: {}", stderr);
            error!("{}", error_msg);
            self.log_installation(plugin_name, &dependencies, false, Some(&error_msg), None)?;
            return Err(TingError::PluginLoadError(error_msg).into());
        }

        debug!("npm install output: {}", String::from_utf8_lossy(&output.stdout));

        let audit_result = if self.security_config.enable_audit {
            match self.run_npm_audit(plugin_dir) {
                Ok(result) => {
                    info!("npm audit completed: {} total vulnerabilities", result.total);
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
        self.log_installation(plugin_name, &dependencies, true, None, audit_result)?;
        Ok(())
    }

    /// Install dependencies with caching support
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

        if self.cache_dir.is_none() {
            debug!("Cache not enabled, falling back to regular installation");
            return self.install_dependencies_with_name(plugin_dir, plugin_name);
        }

        let node_modules_path = self.get_node_modules_path(plugin_dir);
        if !node_modules_path.exists() {
            std::fs::create_dir_all(&node_modules_path)?;
        }

        let mut uncached_deps = Vec::new();
        for dep in dependencies {
            if cache::is_cached(&self.cache_dir, &self.cache_registry, &dep.name, &dep.version) {
                let target_path = node_modules_path.join(&dep.name);
                match cache::link_from_cache(
                    &self.cache_registry, &self.cache_stats,
                    &dep.name, &dep.version, plugin_name, &target_path,
                ) {
                    Ok(_) => info!("Linked cached dependency: {}@{}", dep.name, dep.version),
                    Err(e) => {
                        warn!("Failed to link cached dependency {}@{}: {}", dep.name, dep.version, e);
                        uncached_deps.push(dep.clone());
                    }
                }
            } else {
                uncached_deps.push(dep.clone());
            }
        }

        if !uncached_deps.is_empty() {
            info!("Installing {} uncached dependencies", uncached_deps.len());
            let temp_package_json = PackageJson::from_plugin_metadata(
                plugin_name, "1.0.0", None, None, None, &uncached_deps,
            );
            temp_package_json.write_to_file(&plugin_dir.join("package.json"))?;
            self.install_dependencies_with_name(plugin_dir, plugin_name)?;

            for dep in &uncached_deps {
                let installed_path = node_modules_path.join(&dep.name);
                if installed_path.exists() {
                    if let Err(e) = cache::add_to_cache(
                        &self.cache_dir, &self.cache_registry, &self.cache_stats,
                        &dep.name, &dep.version, plugin_name, &installed_path,
                    ) {
                        warn!("Failed to cache dependency {}: {}", dep.name, e);
                    }
                }
            }
        }

        info!("All dependencies installed successfully for plugin: {}", plugin_name);
        Ok(())
    }

    // ── node_modules helpers ──

    pub fn get_node_modules_path(&self, plugin_dir: &Path) -> PathBuf {
        plugin_dir.join("node_modules")
    }

    pub fn has_node_modules(&self, plugin_dir: &Path) -> bool {
        self.get_node_modules_path(plugin_dir).exists()
    }

    pub fn clean_node_modules(&self, plugin_dir: &Path) -> Result<()> {
        let node_modules_path = self.get_node_modules_path(plugin_dir);
        if node_modules_path.exists() {
            info!("Cleaning node_modules in: {}", plugin_dir.display());
            std::fs::remove_dir_all(&node_modules_path)
                .with_context(|| format!("Failed to remove node_modules at {}", node_modules_path.display()))?;
        }
        Ok(())
    }

    // ── Cache delegation ──

    pub fn is_cached(&self, package_name: &str, version: &str) -> bool {
        cache::is_cached(&self.cache_dir, &self.cache_registry, package_name, version)
    }

    pub fn cleanup_cache_for_plugin(&self, plugin_name: &str) -> Result<usize> {
        cache::cleanup_cache_for_plugin(&self.cache_dir, &self.cache_registry, &self.cache_stats, plugin_name)
    }

    pub fn cleanup_all_unused(&self) -> Result<usize> {
        cache::cleanup_all_unused(&self.cache_dir, &self.cache_registry, &self.cache_stats)
    }

    pub fn get_cache_statistics(&self) -> CacheStatistics {
        cache::get_cache_statistics(&self.cache_registry, &self.cache_stats)
    }

    pub fn clear_cache(&self) -> Result<()> {
        cache::clear_cache(&self.cache_dir, &self.cache_registry, &self.cache_stats)
    }

    // ── Private helpers ──

    fn check_npm_available(&self) -> Result<()> {
        let output = Command::new(&self.npm_path).arg("--version").output()
            .context("Failed to execute npm --version")?;
        if !output.status.success() {
            return Err(TingError::PluginLoadError("npm is not available or not in PATH".to_string()).into());
        }
        info!("npm version: {}", String::from_utf8_lossy(&output.stdout).trim());
        Ok(())
    }

    fn validate_dependencies(&self, dependencies: &[NpmDependency]) -> Result<()> {
        if self.security_config.whitelist.is_empty() {
            return Ok(());
        }

        let mut blocked = Vec::new();
        for dep in dependencies {
            if !self.security_config.whitelist.contains(&dep.name) {
                warn!("Dependency '{}' is not in whitelist", dep.name);
                blocked.push(dep.name.clone());
            }
        }

        if !blocked.is_empty() {
            return Err(TingError::PluginLoadError(format!(
                "The following dependencies are not whitelisted: {}", blocked.join(", ")
            )).into());
        }

        Ok(())
    }

    fn run_npm_audit(&self, plugin_dir: &Path) -> Result<NpmAuditResult> {
        info!("Running npm audit in: {}", plugin_dir.display());
        let output = Command::new(&self.npm_path)
            .arg("audit").arg("--json").current_dir(plugin_dir)
            .output()
            .context("Failed to execute npm audit")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let audit_json: Value = serde_json::from_str(&stdout)
            .context("Failed to parse npm audit output")?;

        let mut vulnerabilities = std::collections::HashMap::new();
        let mut total = 0;

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

        let passed = vulnerabilities.iter()
            .filter(|(sev, count)| **sev > self.security_config.max_vulnerability_severity && **count > 0)
            .count() == 0;

        Ok(NpmAuditResult { vulnerabilities, total, passed, raw_output: stdout.to_string() })
    }

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

        if success {
            info!(plugin = plugin_name, dep_count = dependencies.len(), "Dependency installation succeeded");
        } else {
            error!(plugin = plugin_name, dep_count = dependencies.len(), error = error.unwrap_or("unknown"), "Dependency installation failed");
        }

        if let Some(log_dir) = &self.log_dir {
            if !log_dir.exists() {
                std::fs::create_dir_all(log_dir).context("Failed to create log directory")?;
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
}

impl Default for NpmManager {
    fn default() -> Self {
        Self::new(None, None)
    }
}
