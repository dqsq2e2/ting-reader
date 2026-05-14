//! WebSocket session manager
//!
//! Tracks connected WebSocket sessions per user and broadcasts progress updates.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Maximum number of queued messages per user's broadcast channel
const BROADCAST_CAPACITY: usize = 64;

/// Manages WebSocket sessions grouped by user ID
pub struct WsSessionManager {
    /// Map of user_id → broadcast sender for progress updates
    channels: RwLock<HashMap<String, broadcast::Sender<String>>>,
}

impl WsSessionManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            channels: RwLock::new(HashMap::new()),
        })
    }

    /// Subscribe to progress updates for a user.
    /// Returns a receiver that will get all progress update messages for this user.
    pub async fn subscribe(&self, user_id: &str) -> broadcast::Receiver<String> {
        let channels = self.channels.read().await;
        if let Some(sender) = channels.get(user_id) {
            sender.subscribe()
        } else {
            drop(channels);
            let mut channels = self.channels.write().await;
            let (tx, rx) = broadcast::channel(BROADCAST_CAPACITY);
            channels.insert(user_id.to_string(), tx);
            rx
        }
    }

    /// Broadcast a progress update to all sessions of a user.
    /// The message is a JSON string that will be sent to all connected clients.
    pub async fn broadcast(&self, user_id: &str, message: &str) {
        let channels = self.channels.read().await;
        if let Some(sender) = channels.get(user_id) {
            let _ = sender.send(message.to_string());
        }
    }

    /// Remove a user's channel (e.g., when all sessions disconnect).
    /// In practice we keep channels alive since they're cheap and re-subscription is simpler.
    #[allow(dead_code)]
    pub async fn unsubscribe(&self, user_id: &str) {
        self.channels.write().await.remove(user_id);
    }
}
