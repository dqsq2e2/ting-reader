use crate::core::error::Result;
use crate::core::task_queue::TaskQueue;
use crate::db::repository::LibraryRepository;
use notify::event::ModifyKind;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};

pub struct LibraryWatcher {
    library_repo: Arc<LibraryRepository>,
    task_queue: Arc<TaskQueue>,
    storage_root: PathBuf,
    // Map of library_id -> notify::RecommendedWatcher
    watchers: RwLock<HashMap<String, notify::RecommendedWatcher>>,
    // Map of library_id -> mpsc::Sender for debounce
    debounce_senders: RwLock<HashMap<String, mpsc::Sender<()>>>,
}

impl LibraryWatcher {
    pub fn new(
        library_repo: Arc<LibraryRepository>,
        task_queue: Arc<TaskQueue>,
        storage_root: PathBuf,
    ) -> Self {
        Self {
            library_repo,
            task_queue,
            storage_root,
            watchers: RwLock::new(HashMap::new()),
            debounce_senders: RwLock::new(HashMap::new()),
        }
    }

    pub async fn start_all(&self) -> Result<()> {
        let libraries = self.library_repo.find_all().await?;
        for library in libraries {
            if library.library_type == "local" {
                // Check if watcher is disabled in config
                let scraper_config: crate::db::models::ScraperConfig = library
                    .scraper_config
                    .as_ref()
                    .and_then(|json| serde_json::from_str(json).ok())
                    .unwrap_or_default();

                if !scraper_config.disable_watcher {
                    let full_path = self.storage_root.join(&library.url);
                    if let Err(e) = self
                        .watch_library(&library.id, &full_path.to_string_lossy())
                        .await
                    {
                        warn!("Failed to start watcher for library {}: {}", library.id, e);
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn watch_library(&self, library_id: &str, path: &str) -> Result<()> {
        let path_buf = PathBuf::from(path);
        if !path_buf.exists() || !path_buf.is_dir() {
            return Err(crate::core::error::TingError::NotFound(format!(
                "Directory not found: {}",
                path
            )));
        }

        let lib_id = library_id.to_string();
        let path_clone = path.to_string();

        let (tx, mut rx) = mpsc::channel(100);

        // Debounce logic task
        let task_queue = self.task_queue.clone();
        let lib_id_clone = lib_id.clone();

        tokio::spawn(async move {
            loop {
                // Wait for an event
                if rx.recv().await.is_none() {
                    break;
                }

                // Debounce: Wait 10 seconds. If more events come, reset timer.
                let timeout = tokio::time::sleep(Duration::from_secs(10));
                tokio::pin!(timeout);
                loop {
                    tokio::select! {
                        _ = &mut timeout => {
                            // Timeout expired, enqueue scan task
                            info!("Library watcher triggered scan for library {}", lib_id_clone);

                            if let Err(e) = task_queue.enqueue_scan_library(&lib_id_clone, &path_clone).await {
                                warn!("Failed to enqueue auto-scan task: {}", e);
                            }
                            break;
                        }
                        opt = rx.recv() => {
                            if opt.is_none() {
                                return; // Channel closed
                            }
                            // Reset timer
                            timeout.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(10));
                        }
                    }
                }
            }
        });

        let tx_clone = tx.clone();

        let mut watcher =
            notify::recommended_watcher(move |res: std::result::Result<Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        // Only trigger on creations, modifications or deletions
                        match event.kind {
                            EventKind::Create(_)
                            | EventKind::Modify(ModifyKind::Data(_))
                            | EventKind::Modify(ModifyKind::Name(_))
                            | EventKind::Remove(_) => {
                                // Filter out events caused by our own metadata generation
                                let should_ignore = event.paths.iter().any(|p| {
                                    if let Some(file_name) = p.file_name().and_then(|n| n.to_str())
                                    {
                                        file_name == "metadata.json"
                                            || file_name.ends_with(".nfo")
                                            || file_name.starts_with("cover.")
                                            || file_name.starts_with("folder.")
                                    } else {
                                        false
                                    }
                                });

                                if !should_ignore {
                                    let _ = tx_clone.blocking_send(());
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(e) => error!("Watch error: {:?}", e),
                }
            })
            .map_err(|e| {
                crate::core::error::TingError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;

        watcher
            .watch(&path_buf, RecursiveMode::Recursive)
            .map_err(|e| {
                crate::core::error::TingError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;

        self.watchers.write().await.insert(lib_id.clone(), watcher);
        self.debounce_senders.write().await.insert(lib_id, tx);

        info!("开始监视库 {} at {:?}", library_id, path_buf);

        Ok(())
    }

    pub async fn stop_watching(&self, library_id: &str) {
        let mut watchers = self.watchers.write().await;
        if watchers.remove(library_id).is_some() {
            info!("Stopped watching library {}", library_id);
        }
        let mut senders = self.debounce_senders.write().await;
        senders.remove(library_id);
    }
}
