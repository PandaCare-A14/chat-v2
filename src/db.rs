use crate::chat_server::Message;
use mongodb::{Client, Collection};

pub fn get_message_collection(client: &Client) -> Collection<Message> {
    client.database("public").collection("messages")
}
