use mongodb::bson::{Array, Timestamp, oid::ObjectId};
use serde::{Deserialize, Serialize};

use crate::server::UserId;

#[derive(Deserialize, Serialize)]
pub struct Message {
    id: ObjectId,
    user_id: UserId,
    time_sent: Timestamp,
    content: String,
}

#[derive(Deserialize, Serialize)]
pub struct ChatRoom {
    id: ObjectId,
    whitelist: Vec<UserId>,
    created_at: Timestamp,
    chat_history: Vec<Message>,
}

#[derive(Deserialize, Serialize)]
pub struct ChatRoomCreationParams {
    pub whitelist: Vec<UserId>,
    pub created_at: Timestamp,
    pub chat_history: Array,
}

