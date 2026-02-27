//! Structured logging system
//!
//! This module provides structured logging with:
//! - JSON and text format support
//! - Configurable log levels
//! - Log rotation with size limits
//! - Integration with tracing ecosystem

use crate::core::config::LoggingConfig;
use anyhow::{Context, Result};
use std::path::Path;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

/// Logger instance that manages the logging system
pub struct Logger {
    _guard: Option<WorkerGuard>,
}

impl Logger {
    /// Initialize the logging system based on configuration
    ///
    /// This sets up the global tracing subscriber with the specified format,
    /// level, and output destination.
    pub fn init(config: &LoggingConfig) -> Result<Self> {
        // Parse log level
        let level = parse_log_level(&config.level)?;
        
        // Create env filter with the configured level
        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(level.as_str()));
        
        // Create the appropriate writer and guard based on output configuration
        let (writer, guard) = match config.output.as_str() {
            "stdout" => {
                let (non_blocking, guard) = tracing_appender::non_blocking(std::io::stdout());
                (non_blocking, Some(guard))
            }
            "file" => {
                let log_file = config.log_file.as_ref()
                    .context("log_file must be specified when output is 'file'")?;
                
                // Create parent directory if it doesn't exist
                if let Some(parent) = log_file.parent() {
                    std::fs::create_dir_all(parent)
                        .context("Failed to create log directory")?;
                }
                
                // Create rolling file appender
                let file_appender = create_rolling_appender(
                    log_file,
                    config.max_file_size,
                    config.max_backups,
                )?;
                
                let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
                (non_blocking, Some(guard))
            }
            _ => {
                anyhow::bail!("Invalid output configuration: {}", config.output);
            }
        };
        
        // Create the formatting layer based on format configuration
        let fmt_layer = match config.format.as_str() {
            "json" => {
                fmt::layer()
                    .json()
                    .with_writer(writer)
                    .with_span_events(FmtSpan::CLOSE)
                    .with_current_span(true)
                    .with_thread_ids(true)
                    .with_thread_names(true)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .boxed()
            }
            "text" => {
                fmt::layer()
                    .with_writer(writer)
                    .with_span_events(FmtSpan::CLOSE)
                    .with_thread_ids(true)
                    .with_thread_names(true)
                    .with_target(true)
                    .with_file(true)
                    .with_line_number(true)
                    .boxed()
            }
            _ => {
                anyhow::bail!("Invalid format configuration: {}", config.format);
            }
        };
        
        // Initialize the global subscriber
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .try_init()
            .context("Failed to initialize tracing subscriber")?;
        
        tracing::info!(
            level = %config.level,
            format = %config.format,
            output = %config.output,
            "Logging system initialized"
        );
        
        Ok(Logger { _guard: guard })
    }
}

/// Parse log level string to tracing Level
fn parse_log_level(level: &str) -> Result<Level> {
    match level.to_lowercase().as_str() {
        "debug" => Ok(Level::DEBUG),
        "info" => Ok(Level::INFO),
        "warn" => Ok(Level::WARN),
        "error" => Ok(Level::ERROR),
        _ => anyhow::bail!("Invalid log level: {}", level),
    }
}

/// Create a rolling file appender with size-based rotation
fn create_rolling_appender(
    log_file: &Path,
    max_file_size: usize,
    max_backups: usize,
) -> Result<RollingFileAppender> {
    let directory = log_file.parent()
        .context("Log file must have a parent directory")?;
    
    let filename = log_file.file_name()
        .context("Log file must have a filename")?
        .to_str()
        .context("Log filename must be valid UTF-8")?;
    
    Ok(RollingFileAppender::new(
        directory.to_path_buf(),
        filename.to_string(),
        max_file_size,
        max_backups,
    ))
}

/// Rolling file appender that rotates based on file size
pub struct RollingFileAppender {
    directory: std::path::PathBuf,
    filename: String,
    max_file_size: usize,
    max_backups: usize,
    current_file: std::sync::Mutex<Option<std::fs::File>>,
    current_size: std::sync::atomic::AtomicUsize,
}

impl RollingFileAppender {
    /// Create a new rolling file appender
    pub fn new(
        directory: std::path::PathBuf,
        filename: String,
        max_file_size: usize,
        max_backups: usize,
    ) -> Self {
        Self {
            directory,
            filename,
            max_file_size,
            max_backups,
            current_file: std::sync::Mutex::new(None),
            current_size: std::sync::atomic::AtomicUsize::new(0),
        }
    }
    
    /// Get the current log file path
    fn current_path(&self) -> std::path::PathBuf {
        self.directory.join(&self.filename)
    }
    
    /// Get the backup file path for a given index
    fn backup_path(&self, index: usize) -> std::path::PathBuf {
        self.directory.join(format!("{}.{}", self.filename, index))
    }
    
    /// Rotate log files
    fn rotate(&self) -> std::io::Result<()> {
        // Close current file
        let mut file_guard = self.current_file.lock().unwrap();
        *file_guard = None;
        drop(file_guard);
        
        // Rotate existing backups
        for i in (1..self.max_backups).rev() {
            let from = self.backup_path(i);
            let to = self.backup_path(i + 1);
            
            if from.exists() {
                if to.exists() {
                    std::fs::remove_file(&to)?;
                }
                std::fs::rename(&from, &to)?;
            }
        }
        
        // Move current file to backup.1
        let current = self.current_path();
        if current.exists() {
            let backup = self.backup_path(1);
            if backup.exists() {
                std::fs::remove_file(&backup)?;
            }
            std::fs::rename(&current, &backup)?;
        }
        
        // Reset size counter
        self.current_size.store(0, std::sync::atomic::Ordering::SeqCst);
        
        Ok(())
    }
    
    /// Get or create the current log file
    fn get_file(&self) -> std::io::Result<std::sync::MutexGuard<'_, Option<std::fs::File>>> {
        let mut file_guard = self.current_file.lock().unwrap();
        
        if file_guard.is_none() {
            let path = self.current_path();
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)?;
            
            // Get current file size
            let metadata = file.metadata()?;
            self.current_size.store(
                metadata.len() as usize,
                std::sync::atomic::Ordering::SeqCst
            );
            
            *file_guard = Some(file);
        }
        
        Ok(file_guard)
    }
}

impl std::io::Write for RollingFileAppender {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // Check if rotation is needed
        let current_size = self.current_size.load(std::sync::atomic::Ordering::SeqCst);
        if current_size + buf.len() > self.max_file_size {
            self.rotate()?;
        }
        
        // Write to file
        let mut file_guard = self.get_file()?;
        let file = file_guard.as_mut().unwrap();
        let written = file.write(buf)?;
        
        // Update size counter
        self.current_size.fetch_add(written, std::sync::atomic::Ordering::SeqCst);
        
        Ok(written)
    }
    
    fn flush(&mut self) -> std::io::Result<()> {
        let mut file_guard = self.get_file()?;
        if let Some(file) = file_guard.as_mut() {
            file.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    
    #[allow(dead_code)]
    fn test_config_stdout() -> LoggingConfig {
        LoggingConfig {
            level: "info".to_string(),
            format: "json".to_string(),
            output: "stdout".to_string(),
            log_file: None,
            max_file_size: 10485760,
            max_backups: 5,
        }
    }
    
    #[allow(dead_code)]
    fn test_config_file() -> LoggingConfig {
        LoggingConfig {
            level: "debug".to_string(),
            format: "text".to_string(),
            output: "file".to_string(),
            log_file: Some(PathBuf::from("./test_logs/test.log")),
            max_file_size: 1024,
            max_backups: 3,
        }
    }
    
    #[test]
    fn test_parse_log_level() {
        assert!(matches!(parse_log_level("debug"), Ok(Level::DEBUG)));
        assert!(matches!(parse_log_level("info"), Ok(Level::INFO)));
        assert!(matches!(parse_log_level("warn"), Ok(Level::WARN)));
        assert!(matches!(parse_log_level("error"), Ok(Level::ERROR)));
        assert!(parse_log_level("invalid").is_err());
    }
    
    #[test]
    fn test_rolling_appender_paths() {
        let appender = RollingFileAppender::new(
            PathBuf::from("/tmp/logs"),
            "test.log".to_string(),
            1024,
            5,
        );
        
        assert_eq!(appender.current_path(), PathBuf::from("/tmp/logs/test.log"));
        assert_eq!(appender.backup_path(1), PathBuf::from("/tmp/logs/test.log.1"));
        assert_eq!(appender.backup_path(2), PathBuf::from("/tmp/logs/test.log.2"));
    }
}
