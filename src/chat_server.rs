use std::collections::HashMap;

use futures::TryStreamExt;
use mongodb::bson::oid::ObjectId;
use mongodb::Client;
use mongodb::bson::doc;
use mongodb::bson::{DateTime, Uuid};
use serde::{Deserialize, Serialize};
use tokio::io;
use tokio::sync::{mpsc, oneshot};

use crate::db::get_message_collection;

pub type UserId = Uuid;

#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    #[serde(skip_serializing_if = "Option::is_none")]
    _id: Option<ObjectId>,
    content: String,
    delivered: bool,
    recipient_id: Uuid,
    sender_id: Uuid,
    timestamp: DateTime,
    last_updated: DateTime,
}

enum Command {
    Connect {
        user_id: UserId,
        message_tx: mpsc::Sender<Message>,
    },
    SendMessage {
        content: String,
        sender_id: UserId,
        recipient_id: UserId,
        res_tx: oneshot::Sender<String>,
    },
    Disconnect {
        user_id: UserId,
    },
}

pub struct ChatServer {
    connections: HashMap<UserId, mpsc::Sender<Message>>,
    cmd_rx: mpsc::UnboundedReceiver<Command>,
}

impl ChatServer {
    pub fn new() -> (Self, ChatServerHandle) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Command>();

        (
            Self {
                connections: HashMap::new(),
                cmd_rx,
            },
            ChatServerHandle { cmd_tx },
        )
    }

    pub async fn run(mut self, db_client: Client) -> io::Result<()> {
        while let Some(command) = self.cmd_rx.recv().await {
            match command {
                Command::Connect {
                    user_id,
                    message_tx,
                } => {
                    println!("User connected: {}", user_id);
                    self.connections.insert(user_id, message_tx.clone());

                    // Fetch undelivered messages from MongoDB
                    let messages = get_message_collection(&db_client);
                    
                    // Find messages where this user is the recipient and not yet delivered
                    let filter = doc! {
                        "recipient_id": user_id.to_string(),
                        "delivered": false
                    };
                    
                    // Execute query
                    match messages.find(filter).await {
                        Ok(mut cursor) => {
                            // Process each undelivered message
                            while let Some(message_result) = cursor.try_next().await
                                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))? 
                            {
                                // Send to the user's connection
                                if let Err(e) = message_tx.send(message_result.clone()).await {
                                    println!("Failed to send undelivered message: {}", e);
                                    continue;
                                }
                                
                                // Update message as delivered in MongoDB
                                let update = doc! {
                                    "$set": {
                                        "delivered": true,
                                        "last_updated": DateTime::now()
                                    }
                                };
                                
                                if let Err(e) = messages
                                    .update_one(
                                        doc! { "_id": message_result._id.unwrap() },
                                        update
                                    )
                                    .await
                                {
                                    println!("Failed to update message status: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            println!("Error fetching undelivered messages: {}", e);
                        }
                    }
                }
                Command::Disconnect { user_id } => {
                    println!("User disconnected: {}", user_id);
                    self.connections.remove(&user_id);
                }
                Command::SendMessage {
                    content,
                    sender_id,
                    recipient_id,
                    res_tx,
                } => {
                    let messages = get_message_collection(&db_client);
                    let now = DateTime::now();

                    // Check if recipient is connected
                    let delivered = self.connections.contains_key(&recipient_id);

                    // Create message DTO for MongoDB
                    let message = Message {
                        _id: None,
                        content: content.clone(),
                        delivered,
                        recipient_id,
                        sender_id,
                        timestamp: now,
                        last_updated: now,
                    };

                    // Insert into MongoDB
                    match messages.insert_one(message.clone()).await {
                        Ok(_result) => {
                            // If recipient is connected, deliver the message
                            if delivered {
                                if let Some(tx) = self.connections.get(&recipient_id) {
                                    if let Err(e) = tx.send(message).await {
                                        println!("Failed to deliver message: {}", e);
                                    }
                                }
                            }

                            // Send success response
                            let _ = res_tx.send("Message sent successfully".to_string());
                        }
                        Err(e) => {
                            println!("Failed to save message: {}", e);
                            let _ = res_tx.send(format!("Failed to send message: {}", e));
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct ChatServerHandle {
    cmd_tx: mpsc::UnboundedSender<Command>,
}

impl ChatServerHandle {
    pub async fn connect(
        &self,
        user_id: UserId,
        message_tx: mpsc::Sender<Message>,
    ) -> Result<(), String> {
        self.cmd_tx
            .send(Command::Connect {
                user_id,
                message_tx,
            })
            .map_err(|_| "Failed to send connect command".to_string())
    }

    pub async fn disconnect(&self, user_id: UserId) -> Result<(), String> {
        self.cmd_tx
            .send(Command::Disconnect { user_id })
            .map_err(|_| "Failed to send disconnect command".to_string())
    }

    pub async fn send_message(
        &self,
        content: String,
        sender_id: UserId,
        recipient_id: UserId,
    ) -> Result<String, String> {
        let (res_tx, res_rx) = oneshot::channel();

        self.cmd_tx
            .send(Command::SendMessage {
                content,
                sender_id,
                recipient_id,
                res_tx,
            })
            .map_err(|_| "Failed to transmit send message command".to_string())?;

        res_rx
            .await
            .map_err(|_| "Failed to receive response".to_string())
    }
}
