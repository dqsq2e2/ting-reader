use crate::core::error::{Result, TingError};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path};
use tar::Archive;

pub const MAGIC: &[u8; 8] = b"TRPACK\x00\x01";
const FORMAT_NAME: &str = "ting-reader.trpack";
const FORMAT_VERSION: u32 = 1;
const PACKAGE_MANIFEST_PATH: &str = ".trpack/package.json";
const SIGNATURE_MANIFEST_PATH: &str = ".trpack/signature.json";
const SIGNATURE_FORMAT_NAME: &str = "ting-reader.trpack.signature";
const SIGNATURE_FORMAT_VERSION: u32 = 1;
const SIGNATURE_ALGORITHM: &str = "ed25519";

const TRUSTED_PLUGIN_PUBLIC_KEYS: &[(&str, &str)] = &[(
    "ting-reader-local-2026-06",
    "8eb2b1c14d767be72969773ebd1fb1f486f5e314d801c03019a3d86796aa104a",
)];

#[derive(Debug, Deserialize, Serialize, Clone)]
struct PackageManifest {
    format: String,
    format_version: u32,
    plugin_id: String,
    plugin_version: String,
    #[serde(default)]
    runtime: Option<String>,
    entry_point: String,
    files: Vec<PackageFile>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct PackageFile {
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct PackageSignature {
    format: String,
    format_version: u32,
    algorithm: String,
    key_id: String,
    public_key: String,
    signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrPackageSignatureStatus {
    Trusted {
        key_id: String,
    },
    Unsigned,
    Untrusted {
        key_id: String,
    },
    Invalid {
        key_id: Option<String>,
        reason: String,
    },
}

impl TrPackageSignatureStatus {
    pub fn is_trusted(&self) -> bool {
        matches!(self, Self::Trusted { .. })
    }

    pub fn is_installable_with_confirmation(&self) -> bool {
        matches!(self, Self::Untrusted { .. })
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Trusted { .. } => "trusted",
            Self::Unsigned => "unsigned",
            Self::Untrusted { .. } => "untrusted",
            Self::Invalid { .. } => "invalid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrPackageSignatureIdentity {
    Signed { key_id: String, public_key: String },
}

impl TrPackageSignatureIdentity {
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        self.compatibility_key() == other.compatibility_key()
    }

    pub fn label(&self) -> String {
        match self {
            Self::Signed { key_id, public_key } => {
                let prefix_len = public_key.len().min(12);
                format!("signed:{}:{}", key_id, &public_key[..prefix_len])
            }
        }
    }

    fn compatibility_key(&self) -> String {
        match self {
            Self::Signed { key_id, public_key } => {
                format!("signed:{}:{}", key_id, public_key.to_ascii_lowercase())
            }
        }
    }
}

pub fn has_tr_magic(path: &Path) -> Result<bool> {
    if path.is_dir() {
        return Ok(false);
    }
    let mut file = fs::File::open(path)?;
    let mut magic = [0_u8; 8];
    match file.read_exact(&mut magic) {
        Ok(()) => Ok(&magic == MAGIC),
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(TingError::IoError(error)),
    }
}

pub fn read_manifest_file(path: &Path, manifest_name: &str) -> Result<String> {
    let package = read_tr_package(path)?;
    let bytes = package.files.get(manifest_name).ok_or_else(|| {
        TingError::PluginLoadError(format!("{} not found in .tr package", manifest_name))
    })?;
    String::from_utf8(bytes.clone()).map_err(|error| {
        TingError::PluginLoadError(format!("Invalid UTF-8 in {}: {}", manifest_name, error))
    })
}

pub fn extract_tr_package(path: &Path, target: &Path) -> Result<()> {
    let package = read_tr_package(path)?;
    for (relative, bytes) in package.files {
        validate_archive_path(&relative)?;
        let outpath = target.join(&relative);
        if let Some(parent) = outpath.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(&outpath)?;
        file.write_all(&bytes)?;
    }
    Ok(())
}

pub fn write_install_provenance(package_path: &Path, target: &Path) -> Result<()> {
    let package = read_tr_package(package_path)?;
    let trpack_dir = target.join(".trpack");
    fs::create_dir_all(&trpack_dir)?;

    let manifest = serde_json::to_vec_pretty(&package.manifest).map_err(|error| {
        TingError::PluginLoadError(format!("Failed to write .tr install metadata: {}", error))
    })?;
    fs::write(target.join(PACKAGE_MANIFEST_PATH), manifest)?;

    let signature_path = target.join(SIGNATURE_MANIFEST_PATH);
    if let Some(signature) = package.signature {
        let signature = serde_json::to_vec_pretty(&signature).map_err(|error| {
            TingError::PluginLoadError(format!("Failed to write .tr signature metadata: {}", error))
        })?;
        fs::write(signature_path, signature)?;
    } else if signature_path.exists() {
        fs::remove_file(signature_path)?;
    }

    Ok(())
}

pub fn read_package_signature_identity(path: &Path) -> Result<TrPackageSignatureIdentity> {
    let package = read_tr_package(path)?;
    let status = verify_signature_status(&package.manifest, package.signature.as_ref());
    signature_identity_from_status(package.signature.as_ref(), &status)
}

pub fn read_installed_signature_identity(plugin_path: &Path) -> Result<TrPackageSignatureIdentity> {
    let manifest_path = plugin_path.join(PACKAGE_MANIFEST_PATH);
    if !manifest_path.exists() {
        return Err(TingError::PluginLoadError(format!(
            "Installed plugin {} is missing .trpack package metadata",
            plugin_path.display()
        )));
    }

    let manifest_bytes = fs::read(&manifest_path)?;
    let manifest = serde_json::from_slice::<PackageManifest>(&manifest_bytes).map_err(|error| {
        TingError::PluginLoadError(format!(
            "Invalid installed .tr package metadata at {}: {}",
            manifest_path.display(),
            error
        ))
    })?;

    verify_installed_package(&manifest, plugin_path)?;

    let signature_path = plugin_path.join(SIGNATURE_MANIFEST_PATH);
    let signature = if signature_path.exists() {
        let signature_bytes = fs::read(&signature_path)?;
        Some(
            serde_json::from_slice::<PackageSignature>(&signature_bytes).map_err(|error| {
                TingError::PluginLoadError(format!(
                    "Invalid installed .tr signature metadata at {}: {}",
                    signature_path.display(),
                    error
                ))
            })?,
        )
    } else {
        return Err(TingError::PluginLoadError(format!(
            "Installed plugin {} is missing .trpack signature metadata",
            plugin_path.display()
        )));
    };

    let status = verify_signature_status(&manifest, signature.as_ref());
    signature_identity_from_status(signature.as_ref(), &status)
}

pub fn has_installed_signature_metadata(plugin_path: &Path) -> bool {
    plugin_path.join(PACKAGE_MANIFEST_PATH).is_file()
        && plugin_path.join(SIGNATURE_MANIFEST_PATH).is_file()
}

struct ReadPackage {
    manifest: PackageManifest,
    signature: Option<PackageSignature>,
    files: BTreeMap<String, Vec<u8>>,
}

pub fn verify_tr_package_signature(path: &Path) -> Result<TrPackageSignatureStatus> {
    let package = read_tr_package(path)?;
    Ok(verify_signature_status(
        &package.manifest,
        package.signature.as_ref(),
    ))
}

fn read_tr_package(path: &Path) -> Result<ReadPackage> {
    let file = fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    let mut magic = [0_u8; 8];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(TingError::PluginLoadError(format!(
            "{} is not a Ting Reader .tr package",
            path.display()
        )));
    }

    let decoder = zstd::Decoder::new(reader).map_err(|error| {
        TingError::PluginLoadError(format!("Failed to decode .tr package: {}", error))
    })?;
    let mut archive = Archive::new(decoder);
    let mut package_manifest = None;
    let mut package_signature = None;
    let mut files = BTreeMap::new();

    for entry in archive.entries().map_err(|error| {
        TingError::PluginLoadError(format!("Failed to read .tr package: {}", error))
    })? {
        let mut entry = entry.map_err(|error| {
            TingError::PluginLoadError(format!("Failed to read .tr entry: {}", error))
        })?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry
            .path()
            .map_err(|error| {
                TingError::PluginLoadError(format!("Invalid .tr entry path: {}", error))
            })?
            .to_string_lossy()
            .replace('\\', "/");
        validate_archive_path(&path)?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        if path == PACKAGE_MANIFEST_PATH {
            package_manifest = Some(serde_json::from_slice::<PackageManifest>(&bytes).map_err(
                |error| TingError::PluginLoadError(format!("Invalid .tr manifest: {}", error)),
            )?);
        } else if path == SIGNATURE_MANIFEST_PATH {
            package_signature = Some(serde_json::from_slice::<PackageSignature>(&bytes).map_err(
                |error| TingError::PluginLoadError(format!("Invalid .tr signature: {}", error)),
            )?);
        } else {
            files.insert(path, bytes);
        }
    }

    let manifest = package_manifest
        .ok_or_else(|| TingError::PluginLoadError(".tr package metadata not found".to_string()))?;
    verify_package(&manifest, &files)?;
    Ok(ReadPackage {
        manifest,
        signature: package_signature,
        files,
    })
}

fn verify_package(manifest: &PackageManifest, files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    if manifest.format != FORMAT_NAME || manifest.format_version != FORMAT_VERSION {
        return Err(TingError::PluginLoadError(format!(
            "Unsupported .tr package format {} v{}",
            manifest.format, manifest.format_version
        )));
    }
    if manifest.plugin_id.trim().is_empty() || manifest.plugin_version.trim().is_empty() {
        return Err(TingError::PluginLoadError(
            ".tr package metadata missing plugin id/version".to_string(),
        ));
    }
    if !files.contains_key("plugin.yml") {
        return Err(TingError::PluginLoadError(
            ".tr package is missing plugin.yml".to_string(),
        ));
    }
    if !files.contains_key(&manifest.entry_point) {
        return Err(TingError::PluginLoadError(format!(
            ".tr package is missing entry_point {}",
            manifest.entry_point
        )));
    }

    let expected: BTreeSet<_> = manifest
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    let actual: BTreeSet<_> = files.keys().map(String::as_str).collect();
    if expected != actual {
        return Err(TingError::PluginLoadError(
            ".tr package file table does not match payload".to_string(),
        ));
    }

    for file in &manifest.files {
        validate_archive_path(&file.path)?;
        let bytes = files.get(&file.path).ok_or_else(|| {
            TingError::PluginLoadError(format!(".tr package missing file {}", file.path))
        })?;
        if bytes.len() as u64 != file.size {
            return Err(TingError::PluginLoadError(format!(
                ".tr package size mismatch for {}",
                file.path
            )));
        }
        let sha256 = hex_lower(&Sha256::digest(bytes));
        if sha256 != file.sha256 {
            return Err(TingError::PluginLoadError(format!(
                ".tr package checksum mismatch for {}",
                file.path
            )));
        }
    }
    Ok(())
}

fn verify_installed_package(manifest: &PackageManifest, plugin_path: &Path) -> Result<()> {
    if manifest.format != FORMAT_NAME || manifest.format_version != FORMAT_VERSION {
        return Err(TingError::PluginLoadError(format!(
            "Unsupported installed .tr package format {} v{}",
            manifest.format, manifest.format_version
        )));
    }
    if manifest.plugin_id.trim().is_empty() || manifest.plugin_version.trim().is_empty() {
        return Err(TingError::PluginLoadError(
            "Installed .tr package metadata missing plugin id/version".to_string(),
        ));
    }
    if !manifest.files.iter().any(|file| file.path == "plugin.yml") {
        return Err(TingError::PluginLoadError(
            "Installed .tr package metadata is missing plugin.yml".to_string(),
        ));
    }
    if !manifest
        .files
        .iter()
        .any(|file| file.path == manifest.entry_point)
    {
        return Err(TingError::PluginLoadError(format!(
            "Installed .tr package metadata is missing entry_point {}",
            manifest.entry_point
        )));
    }

    for file in &manifest.files {
        validate_archive_path(&file.path)?;
        let full_path = plugin_path.join(&file.path);
        let bytes = fs::read(&full_path).map_err(|error| {
            TingError::PluginLoadError(format!(
                "Installed plugin file {} cannot be read: {}",
                full_path.display(),
                error
            ))
        })?;
        if bytes.len() as u64 != file.size {
            return Err(TingError::PluginLoadError(format!(
                "Installed plugin file size mismatch for {}",
                file.path
            )));
        }
        let sha256 = hex_lower(&Sha256::digest(&bytes));
        if sha256 != file.sha256 {
            return Err(TingError::PluginLoadError(format!(
                "Installed plugin file checksum mismatch for {}",
                file.path
            )));
        }
    }

    Ok(())
}

fn validate_archive_path(path: &str) -> Result<()> {
    if path.is_empty() {
        return Err(TingError::PluginLoadError(
            ".tr package contains empty path".to_string(),
        ));
    }
    let path = Path::new(path);
    if path.is_absolute() {
        return Err(TingError::PluginLoadError(format!(
            ".tr package path must be relative: {}",
            path.display()
        )));
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(TingError::PluginLoadError(format!(
                ".tr package contains unsafe path: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn verify_signature_status(
    manifest: &PackageManifest,
    signature: Option<&PackageSignature>,
) -> TrPackageSignatureStatus {
    let Some(signature) = signature else {
        return TrPackageSignatureStatus::Unsigned;
    };

    if let Err(error) = validate_signature_metadata(signature) {
        return TrPackageSignatureStatus::Invalid {
            key_id: Some(signature.key_id.clone()).filter(|key_id| !key_id.trim().is_empty()),
            reason: error.to_string(),
        };
    }

    let trusted_public_key = trusted_public_key(&signature.key_id);
    let public_key = trusted_public_key.unwrap_or(signature.public_key.as_str());

    if trusted_public_key.is_some() && !signature.public_key.eq_ignore_ascii_case(public_key) {
        return TrPackageSignatureStatus::Invalid {
            key_id: Some(signature.key_id.clone()),
            reason: "signature public key does not match trusted key".to_string(),
        };
    }

    if let Err(error) = verify_signature_with_public_key(manifest, signature, public_key) {
        return TrPackageSignatureStatus::Invalid {
            key_id: Some(signature.key_id.clone()),
            reason: error.to_string(),
        };
    }

    if trusted_public_key.is_some() {
        TrPackageSignatureStatus::Trusted {
            key_id: signature.key_id.clone(),
        }
    } else {
        TrPackageSignatureStatus::Untrusted {
            key_id: signature.key_id.clone(),
        }
    }
}

fn signature_identity_from_status(
    signature: Option<&PackageSignature>,
    status: &TrPackageSignatureStatus,
) -> Result<TrPackageSignatureIdentity> {
    match status {
        TrPackageSignatureStatus::Trusted { key_id } => {
            let signature = signature.ok_or_else(|| {
                TingError::PluginLoadError("Trusted .tr signature metadata missing".to_string())
            })?;
            Ok(TrPackageSignatureIdentity::Signed {
                key_id: key_id.clone(),
                public_key: signature.public_key.clone(),
            })
        }
        TrPackageSignatureStatus::Untrusted { key_id } => {
            let signature = signature.ok_or_else(|| {
                TingError::PluginLoadError("Untrusted .tr signature metadata missing".to_string())
            })?;
            Ok(TrPackageSignatureIdentity::Signed {
                key_id: key_id.clone(),
                public_key: signature.public_key.clone(),
            })
        }
        TrPackageSignatureStatus::Unsigned => Err(TingError::PluginLoadError(
            "Plugin package is not signed".to_string(),
        )),
        TrPackageSignatureStatus::Invalid { reason, .. } => Err(TingError::PluginLoadError(
            format!("Invalid plugin package signature: {}", reason),
        )),
    }
}

fn validate_signature_metadata(signature: &PackageSignature) -> Result<()> {
    if signature.format != SIGNATURE_FORMAT_NAME {
        return Err(TingError::PluginLoadError(format!(
            "Unsupported .tr signature format {}",
            signature.format
        )));
    }
    if signature.format_version != SIGNATURE_FORMAT_VERSION {
        return Err(TingError::PluginLoadError(format!(
            "Unsupported .tr signature format version {}",
            signature.format_version
        )));
    }
    if signature.algorithm != SIGNATURE_ALGORITHM {
        return Err(TingError::PluginLoadError(format!(
            "Unsupported .tr signature algorithm {}",
            signature.algorithm
        )));
    }
    if signature.key_id.trim().is_empty() {
        return Err(TingError::PluginLoadError(
            ".tr signature key_id is required".to_string(),
        ));
    }
    decode_fixed_hex::<32>(&signature.public_key, "public_key")?;
    decode_fixed_hex::<64>(&signature.signature, "signature")?;
    Ok(())
}

fn verify_signature_with_public_key(
    manifest: &PackageManifest,
    signature: &PackageSignature,
    public_key_hex: &str,
) -> Result<()> {
    let public_key = decode_fixed_hex::<32>(public_key_hex, "public_key")?;
    let verifying_key = VerifyingKey::from_bytes(&public_key).map_err(|error| {
        TingError::PluginLoadError(format!("Invalid .tr signature public key: {}", error))
    })?;
    let signature_bytes = decode_fixed_hex::<64>(&signature.signature, "signature")?;
    let signature = Signature::from_bytes(&signature_bytes);
    verifying_key
        .verify(&signature_payload(manifest), &signature)
        .map_err(|error| {
            TingError::PluginLoadError(format!("Invalid .tr package signature: {}", error))
        })
}

fn signature_payload(manifest: &PackageManifest) -> Vec<u8> {
    let mut payload = String::new();
    payload.push_str("ting-reader.trpack.signature.v1\n");
    payload.push_str("format=");
    payload.push_str(&manifest.format);
    payload.push('\n');
    payload.push_str("format_version=");
    payload.push_str(&manifest.format_version.to_string());
    payload.push('\n');
    payload.push_str("plugin_id=");
    payload.push_str(&manifest.plugin_id);
    payload.push('\n');
    payload.push_str("plugin_version=");
    payload.push_str(&manifest.plugin_version);
    payload.push('\n');
    payload.push_str("runtime=");
    payload.push_str(manifest.runtime.as_deref().unwrap_or(""));
    payload.push('\n');
    payload.push_str("entry_point=");
    payload.push_str(&manifest.entry_point);
    payload.push('\n');
    for file in &manifest.files {
        payload.push_str("file=");
        payload.push_str(&file.path);
        payload.push('\0');
        payload.push_str(&file.size.to_string());
        payload.push('\0');
        payload.push_str(&file.sha256);
        payload.push('\n');
    }
    payload.into_bytes()
}

fn trusted_public_key(key_id: &str) -> Option<&'static str> {
    TRUSTED_PLUGIN_PUBLIC_KEYS
        .iter()
        .find_map(|(trusted_key_id, public_key)| (*trusted_key_id == key_id).then_some(*public_key))
}

fn decode_fixed_hex<const N: usize>(value: &str, field: &str) -> Result<[u8; N]> {
    let bytes = hex_decode(value).map_err(|error| {
        TingError::PluginLoadError(format!(".tr signature {} must be hex: {}", field, error))
    })?;
    if bytes.len() != N {
        return Err(TingError::PluginLoadError(format!(
            ".tr signature {} must be {} bytes",
            field, N
        )));
    }
    let mut output = [0_u8; N];
    output.copy_from_slice(&bytes);
    Ok(output)
}

fn hex_decode(value: &str) -> std::result::Result<Vec<u8>, &'static str> {
    let value = value.trim();
    if value.len() % 2 != 0 {
        return Err("odd length");
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let raw = value.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        let high = hex_value(raw[index]).ok_or("invalid character")?;
        let low = hex_value(raw[index + 1]).ok_or("invalid character")?;
        bytes.push((high << 4) | low);
        index += 2;
    }
    Ok(bytes)
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
