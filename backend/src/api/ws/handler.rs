//! WebSocket handler for real-time progress sync

use crate::api::handlers::AppState;
use crate::api::ws::manager::WsSessionManager;
use crate::auth::jwt;
use crate::core::error::TingError;
use crate::db::models::Progress;
use crate::db::repository::Repository;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, warn};

/// WebSocket query parameters (token auth)
#[derive(Deserialize)]
pub struct WsQuery {
    #[allow(dead_code)]
    token: Option<String>,
}

/// Client → Server message
#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
enum ClientMessage {
    #[serde(rename = "progress_update")]
    ProgressUpdate {
        book_id: String,
        chapter_id: Option<String>,
        position: f64,
    },
    #[serde(rename = "ping")]
    Ping,
}

/// Server → Client message
#[derive(Serialize, Debug)]
#[serde(tag = "type")]
enum ServerMessage {
    #[serde(rename = "progress_updated")]
    ProgressUpdated {
        book_id: String,
        chapter_id: Option<String>,
        position: f64,
        updated_at: String,
    },
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "error")]
    Error { message: String },
}

/// Handle WebSocket upgrade request
pub async fn ws_handler(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let token = params.get("token").cloned();

    // Authenticate via token
    let user_id = match authenticate_ws_token(&state, token).await {
        Ok(id) => id,
        Err(e) => return e.into_response(),
    };

    // Upgrade and pass state + user_id to the handler
    ws.on_upgrade(move |socket| handle_ws_connection(socket, state, user_id))
}

/// Validate the WebSocket token and return the user_id
async fn authenticate_ws_token(
    state: &AppState,
    token: Option<String>,
) -> Result<String, axum::response::Response> {
    let token = token.ok_or_else(|| {
        TingError::AuthenticationError("Missing token parameter".to_string()).into_response()
    })?;

    let claims = if let Some(key_manager) = &state.jwt_key_manager {
        let secrets = key_manager.get_validation_secrets().await;
        match jwt::validate_token_with_secrets(&token, &secrets) {
            Ok(c) => c,
            Err(e) => return Err(e.into_response()),
        }
    } else {
        match jwt::validate_token(&token, &state.jwt_secret) {
            Ok(c) => c,
            Err(e) => return Err(e.into_response()),
        }
    };

    // Verify user exists in DB
    let user = state.user_repo.find_by_id(&claims.user_id).await
        .map_err(|e| e.into_response())?;

    match user {
        Some(_) => Ok(claims.user_id),
        None => Err(TingError::AuthenticationError("User not found".to_string()).into_response()),
    }
}

/// Handle an established WebSocket connection
async fn handle_ws_connection(socket: WebSocket, state: AppState, user_id: String) {
    debug!("WebSocket 连接已建立: user={}", &user_id);
    let (mut sender, mut receiver) = socket.split();
    let ws_manager = state.ws_manager.clone();

    // Subscribe to broadcast channel for this user's progress updates
    let mut broadcast_rx = ws_manager.subscribe(&user_id).await;

    let user_id_send = user_id.clone();
    let user_id_recv = user_id.clone();

    // Task: forward broadcast messages to the WebSocket client
    let send_task: tokio::task::JoinHandle<()> = tokio::spawn(async move {
        loop {
            match broadcast_rx.recv().await {
                Ok(msg) => {
                    if sender
                        .send(Message::Text(msg.into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(RecvError::Lagged(n)) => {
                    debug!(
                        "WS broadcast lagged by {} messages for user {}",
                        n, &user_id_send
                    );
                }
                Err(RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    // Task: handle incoming messages from the WebSocket client
    let recv_task: tokio::task::JoinHandle<()> = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    handle_client_message(&state, &ws_manager, &user_id_recv, &text).await;
                }
                Ok(Message::Close(_)) => {
                    break;
                }
                Err(e) => {
                    debug!("WS receive error for user {}: {}", &user_id_recv, e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    debug!("WebSocket 连接已断开: user={}", &user_id);
}

/// Process an incoming message from the client
async fn handle_client_message(
    state: &AppState,
    ws_manager: &Arc<WsSessionManager>,
    user_id: &str,
    text: &str,
) {
    let msg: ClientMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            warn!("Invalid WS message from user {}: {}", user_id, e);
            let error = serde_json::to_string(&ServerMessage::Error {
                message: format!("Invalid message: {}", e),
            })
            .unwrap_or_default();
            ws_manager.broadcast(user_id, &error).await;
            return;
        }
    };

    match msg {
        ClientMessage::ProgressUpdate {
            book_id,
            chapter_id,
            position,
        } => {
            // Save progress to database
            let progress = Progress {
                id: uuid::Uuid::new_v4().to_string(),
                user_id: user_id.to_string(),
                book_id: book_id.clone(),
                chapter_id: chapter_id.clone(),
                position,
                duration: None,
                updated_at: chrono::Utc::now().to_rfc3339(),
            };

            debug!("WS 收到进度: user={} book={} ch={} pos={}s", &user_id, &book_id, chapter_id.as_deref().unwrap_or("-"), position);

            if let Err(e) = state.progress_repo.upsert(&progress).await {
                warn!("WS 进度保存失败: user={} err={}", &user_id, e);
                return;
            }

            // Broadcast to all sessions of this user (including other devices)
            let broadcast_msg = serde_json::to_string(&ServerMessage::ProgressUpdated {
                book_id,
                chapter_id,
                position,
                updated_at: progress.updated_at,
            })
            .unwrap_or_default();

            ws_manager.broadcast(user_id, &broadcast_msg).await;
        }
        ClientMessage::Ping => {
            let pong = serde_json::to_string(&ServerMessage::Pong).unwrap_or_default();
            ws_manager.broadcast(user_id, &pong).await;
        }
    }
}
