use std::collections::{HashMap, HashSet};

use mongodb::bson::{Timestamp, Uuid, oid::ObjectId};
use tokio::io;
use tokio::sync::mpsc::{self};

pub type ConnId = Uuid;
pub type RoomId = Uuid;
pub type UserId = Uuid;

struct Message {
    message_id: ObjectId,
    timestamp: Timestamp,
    last_updated: Timestamp,
    content: String,
}

enum Command {
    JoinRoom {
        user_id: UserId,
    },
    SendMessage {
        message: Message,
        conn_id: ConnId,
        room_id: RoomId,
    },
}

pub struct ChatServer {
    rooms: HashMap<RoomId, HashSet<ConnId>>,
    cmd_rx: mpsc::UnboundedReceiver<Command>,
}

impl ChatServer {
    pub fn new() -> (Self, ChatServerHandle) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Command>();

        (
            Self {
                rooms: HashMap::new(),
                cmd_rx,
            },
            ChatServerHandle { cmd_tx },
        )
    }

    pub async fn run(mut self) -> io::Result<()> {
        match &self.cmd_rx.recv().await.unwrap() {
            Command::SendMessage {
                message,
                conn_id,
                room_id,
            } => {
                self.rooms.get(&room_id);
            }
            Command::JoinRoom { user_id } => {
                user_id;
            }
        };

        Ok(())
    }
}

pub struct ChatServerHandle {
    cmd_tx: mpsc::UnboundedSender<Command>,
}
