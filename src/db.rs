use std::time::{SystemTime, UNIX_EPOCH};

use futures::TryStreamExt;
use mongodb::{
    bson::{doc, Bson, Timestamp}, error::Error, Client, Collection, Cursor
};

use crate::{server::UserId, types::{ChatRoom, ChatRoomCreationParams}};

const DB_NAME: &str = "public";
const COLL_NAME: &str = "chat_rooms";

pub async fn create_chat_room(client: Client, issuer_user_id:UserId, target_user_id: UserId) -> Result<Bson, Error> {
    let collection = client.database(DB_NAME).collection(COLL_NAME);

    let whitelist = vec![target_user_id, issuer_user_id];

    let chat_room = ChatRoomCreationParams {
        whitelist,
        created_at: Timestamp { time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as u32, increment: 1 },
        chat_history: vec![],
    };

    let result = collection.insert_one(chat_room).await?;

    Ok(result.inserted_id)
}

pub async fn get_chat_rooms(client: Client, user_id: UserId) -> Result<Vec<ChatRoom>, Error> {
    let collection: Collection<ChatRoom> = client.database(DB_NAME).collection(COLL_NAME);

    let result: Cursor<ChatRoom> = collection
        .find(doc! {"whitelist": user_id.to_string()})
        .await?;

    let results = result.try_collect().await?;
    
    Ok(results)
}

