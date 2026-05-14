//! Installation backup with automatic rollback on failure
//!
//! **Validates: Requirement 26.8**

use crate::core::error::Result;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, error, warn};

/// Installation backup for rollback support
pub(crate) struct InstallationBackup {
    target_path: PathBuf,
    backup_path: Option<PathBuf>,
    committed: bool,
}

impl InstallationBackup {
    pub(super) fn new(target_path: &Path) -> Result<Self> {
        let backup_path = if target_path.exists() {
            let backup = target_path.with_extension("backup");
            debug!("Creating backup: {} -> {}", target_path.display(), backup.display());

            if backup.exists() {
                fs::remove_dir_all(&backup)?;
            }

            fs::rename(target_path, &backup)?;
            Some(backup)
        } else {
            None
        };

        Ok(Self { target_path: target_path.to_path_buf(), backup_path, committed: false })
    }

    pub(super) fn commit(mut self) -> Result<()> {
        self.committed = true;
        if let Some(backup) = &self.backup_path {
            debug!("Committing installation, removing backup: {}", backup.display());
            fs::remove_dir_all(backup)?;
        }
        Ok(())
    }

    pub(super) fn rollback(&self) -> Result<()> {
        warn!("Rolling back installation: {}", self.target_path.display());

        if self.target_path.exists() {
            fs::remove_dir_all(&self.target_path)?;
        }

        if let Some(backup) = &self.backup_path {
            debug!("Restoring backup: {} -> {}", backup.display(), self.target_path.display());
            fs::rename(backup, &self.target_path)?;
        }

        Ok(())
    }
}

impl Drop for InstallationBackup {
    fn drop(&mut self) {
        if !self.committed {
            if let Err(e) = self.rollback() {
                error!("Failed to rollback installation: {}", e);
            }
        }
    }
}
