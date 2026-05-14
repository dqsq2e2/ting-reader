//! Package.json generation for JavaScript plugins

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::info;

use super::security::NpmDependency;

/// package.json structure for JavaScript plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageJson {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub dependencies: HashMap<String, String>,
    pub private: bool,
}

impl PackageJson {
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

    pub fn write_to_file(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize package.json")?;
        std::fs::write(path, json)
            .with_context(|| format!("Failed to write package.json to {}", path.display()))?;
        info!("Generated package.json at: {}", path.display());
        Ok(())
    }

    pub fn read_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read package.json from {}", path.display()))?;
        let package_json: PackageJson = serde_json::from_str(&content)
            .context("Failed to parse package.json")?;
        Ok(package_json)
    }
}

/// Generates a package.json for a plugin at the given directory
pub fn generate_package_json(
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
        plugin_name, plugin_version, description, author, license, npm_dependencies,
    );
    let path = plugin_dir.join("package.json");
    package_json.write_to_file(&path)?;
    Ok(path)
}
