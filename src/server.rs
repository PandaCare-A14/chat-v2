use actix_web::{
    HttpRequest, HttpResponse, Responder,
    web::{self},
};
use futures::TryStreamExt;
use jsonwebtoken::DecodingKey;
use mongodb::{
    Client,
    bson::{self, Binary, doc},
};

use crate::{
    chat_server::Message,
    db::get_message_collection,
    utils::{get_access_token_from_auth_header, get_user_details},
};

pub fn rest_scope(cfg: &mut web::ServiceConfig) {
    cfg.service(get_rooms);
}

#[actix_web::get("/chat/rooms")]
async fn get_rooms(
    req: HttpRequest,
    client: web::Data<Client>,
    verifying_key: web::Data<DecodingKey>,
) -> impl Responder {
    let client = client.get_ref().clone();

    let token_str = match get_access_token_from_auth_header(req) {
        Some(token) => token,
        None => return HttpResponse::Unauthorized().body("User is invalid"),
    };

    let user = match get_user_details(&token_str, verifying_key.get_ref()) {
        Ok(user) => user,
        Err(_err) => return HttpResponse::Unauthorized().body("User is invalid"),
    };

    let messages = get_message_collection(&client);

    let query_pipeline = vec![
        doc! {
            "$match": {
                "$or": [
                    { "sender_id": &user.user_id() },
                    { "recipient_id": &user.user_id() }
                ]
            }
        },
        doc! {
            "$addFields": {
                "chat_partner_id": {
                    "$cond": [
                        { "$eq": ["$sender_id", &user.user_id()] },
                        "$recipient_id",
                        "$sender_id"
                    ]
                }
            }
        },
        doc! {
            "$sort": { "timestamp": 1 } // chronological order
        },
        doc! {
            "$group": {
                "_id": "$chat_partner_id",
                "messages": { "$push": "$$ROOT" }
            }
        },
    ];

    let mut rooms = match messages.aggregate(query_pipeline).await {
        Ok(rooms) => rooms,
        Err(err) => return HttpResponse::InternalServerError().body(err.to_string()),
    };

    let mut room_vec: Vec<(Binary, Vec<Message>)> = Vec::new();

    while let Some(room) = rooms.try_next().await.unwrap() {
        let partner_id = match room.get("_id") {
            Some(bson::Bson::Binary(binary)) if binary.subtype == bson::spec::BinarySubtype::Uuid => binary.bytes.clone(),
            _ => return HttpResponse::InternalServerError().body("Invalid or missing '_id' field"),
        };

        if let Ok(msgs) = room.get_array("messages") {
            let messages: Vec<Message> = msgs
                .iter()
                .filter_map(|val| bson::from_bson::<Message>(val.clone()).ok())
                .collect();

            room_vec.push((
                Binary {
                    subtype: bson::spec::BinarySubtype::Uuid,
                    bytes: partner_id,
                },
                messages,
            ))
        }
    }

    HttpResponse::Ok().json(room_vec)
}
