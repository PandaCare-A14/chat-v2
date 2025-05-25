use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_ws::{Message as WsMessage, MessageStream, Session};
use futures::{FutureExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::{sync::{mpsc, oneshot}, time::interval};

use crate::chat_server::{ChatServerHandle, Message, UserId};
use crate::utils::{get_access_token_from_auth_header, get_user_details};

// WebSocket connection constants
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    content: String,
    recipient_id: UserId,
}

#[derive(Serialize, Deserialize)]
struct WebSocketMessage {
    #[serde(default)]
    message_type: String,
    #[serde(flatten)]
    data: serde_json::Value,
}

// WebSocket connection handler endpoint
pub async fn ws_connect(
    req: HttpRequest,
    body: web::Payload,
    chat_handle: web::Data<ChatServerHandle>,
    verifying_key: web::Data<jsonwebtoken::DecodingKey>,
) -> Result<HttpResponse, Error> {
    // Extract and verify token
    let token = match get_access_token_from_auth_header(req.clone()) {
        Some(token) => token,
        None => return Ok(HttpResponse::Unauthorized().body("No authorization token provided")),
    };

    // Get user details from token
    let user = match get_user_details(&token, verifying_key.get_ref()) {
        Ok(user) => user,
        Err(_) => return Ok(HttpResponse::Unauthorized().body("Invalid token")),
    };

    let user_id = user.user_id();

    // Create a WebSocket session
    let (response, session, msg_stream) = actix_ws::handle(&req, body)?;
    
    // Spawn the WebSocket handler
    actix_web::rt::spawn(websocket_handler(session, msg_stream, chat_handle.get_ref().clone(), user_id));

    // Return the response
    Ok(response)
}

// Main WebSocket handler function
async fn websocket_handler(
    mut session: Session,
    mut msg_stream: MessageStream,
    chat_handle: ChatServerHandle,
    user_id: UserId,
) {
    // Create a channel for receiving messages from chat server
    let (msg_tx, mut msg_rx) = mpsc::channel::<Message>(100);
    
    // Connect user to chat server
    match chat_handle.connect(user_id, msg_tx).await {
        Ok(_) => println!("User {} connected to chat", user_id),
        Err(e) => {
            println!("Failed to connect to chat server: {}", e);
            let _ = session.close(Some(actix_ws::CloseReason {
                code: actix_ws::CloseCode::Error,
                description: Some(e),
            })).await;
            return;
        }
    };

    // Create a channel for sending shutdown signals
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let mut shutdown_rx = shutdown_rx.fuse();
    
    // Set up heartbeat
    let mut heartbeat_interval = interval(HEARTBEAT_INTERVAL);

    let last_heartbeat = Arc::new(Mutex::new(Instant::now()));
    let last_heartbeat_clone = Arc::clone(&last_heartbeat);

    // Task for forwarding chat messages to WebSocket
    let chat_to_ws = {
        let mut session = session.clone();
        
        async move {
            loop {
                tokio::select! {
                    // New message from chat server
                    Some(msg) = msg_rx.recv() => {
                        // Convert the chat message to a WebSocket message
                        let ws_msg = WebSocketMessage {
                            message_type: "message".to_string(),
                            data: serde_json::to_value(msg).unwrap_or_default(),
                        };
                        
                        if let Ok(json) = serde_json::to_string(&ws_msg) {
                            if session.text(json).await.is_err() {
                                break;
                            }
                        }
                    }
                    
                    // Heartbeat tick
                    _ = heartbeat_interval.tick() => {
                        // Check client heartbeat
                        if Instant::now().duration_since(*last_heartbeat.lock().unwrap()) > CLIENT_TIMEOUT {
                            println!("Client heartbeat timeout, disconnecting!");
                            let _ = session.close(None).await;
                            break;
                        }
                        
                        // Send ping
                        if session.ping(b"").await.is_err() {
                            break;
                        }
                    }
                    
                    // Shutdown signal
                    _ = &mut shutdown_rx => {
                        break;
                    }
                }
            }
        }
    };
    
    // Spawn the message forwarding task
    let chat_task = tokio::spawn(chat_to_ws);

    // Process incoming WebSocket messages
    while let Some(msg) = msg_stream.next().await {
        match msg {
            Ok(WsMessage::Text(text)) => {
                // Try to parse the WebSocket message
                if let Ok(ws_message) = serde_json::from_str::<WebSocketMessage>(&text) {
                    match ws_message.message_type.as_str() {
                        "message" => {
                            // Parse the chat message
                            if let Ok(chat_msg) = serde_json::from_value::<ChatMessage>(ws_message.data) {
                                // Send the message
                                match chat_handle.send_message(
                                    chat_msg.content,
                                    user_id,
                                    chat_msg.recipient_id,
                                ).await {
                                    Ok(response) => {
                                        if session.text(response).await.is_err() {
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        let error_msg = format!("{{\"message_type\":\"error\",\"message\":\"{}\"}}", e);
                                        if session.text(error_msg).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        "ping" => {
                            // Send pong response
                            if session.text("{\"message_type\":\"pong\"}").await.is_err() {
                                break;
                            }
                        }
                        _ => {
                            // Unknown message type
                            if session.text("{\"message_type\":\"error\",\"message\":\"Unknown message type\"}").await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }
            Ok(WsMessage::Ping(bytes)) => {
                let mut last_heartbeat = last_heartbeat_clone.lock().unwrap();
                *last_heartbeat = Instant::now();
                if session.pong(&bytes).await.is_err() {
                    break;
                }
            }
            Ok(WsMessage::Pong(_)) => {
                let mut last_heartbeat = last_heartbeat_clone.lock().unwrap();
                *last_heartbeat = Instant::now();
            }
            Ok(WsMessage::Close(reason)) => {
                let _ = session.close(reason).await;
                break;
            }
            Ok(WsMessage::Binary(_)) => {
                // Ignore binary messages or handle them if needed
            }
            Ok(WsMessage::Continuation(_)) => {
                // Ignore continuation messages
            }
            Ok(WsMessage::Nop) => {
                // No-op, do nothing
            }
            Err(e) => {
                println!("Error receiving message: {:?}", e);
                break;
            }
        }
    }

    // Signal the chat task to stop
    let _ = shutdown_tx.send(());
    
    // Wait for the chat task to finish
    let _ = chat_task.await;
    
    // Disconnect from chat server
    let _ = chat_handle.disconnect(user_id).await;
    
    println!("WebSocket connection closed for user {}", user_id);
}