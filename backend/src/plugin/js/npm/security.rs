//! npm Dependency Manager - Security types
//!
//! Contains NpmSecurityConfig, VulnerabilitySeverity, NpmAuditResult,
//! and NpmDependency structs.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// npm dependency specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpmDependency {
    pub name: String,
    pub version: String,
}

impl NpmDependency {
    pub fn new(name: String, version: String) -> Self {
        Self { name, version }
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
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "low" => Some(Self::Low),
            "moderate" => Some(Self::Moderate),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Moderate => "moderate",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// Security configuration for npm dependency management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpmSecurityConfig {
    pub whitelist: HashSet<String>,
    pub enforce_version_lock: bool,
    pub enable_audit: bool,
    pub fail_on_audit_vulnerabilities: bool,
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

/// npm audit result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpmAuditResult {
    pub vulnerabilities: HashMap<VulnerabilitySeverity, usize>,
    pub total: usize,
    pub passed: bool,
    pub raw_output: String,
}
