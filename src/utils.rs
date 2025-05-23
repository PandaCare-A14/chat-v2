use actix_web::{HttpRequest, http::header};
use jsonwebtoken::{Algorithm, DecodingKey, TokenData, Validation, decode};
use mongodb::bson::Uuid;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct User {
    user_id: Uuid,
}

impl User {
    pub fn user_id(self: User) -> Uuid {
        self.user_id
    }
}

pub fn get_user_details(
    token: &str,
    verifying_key: &DecodingKey,
) -> Result<User, jsonwebtoken::errors::Error> {
    let token_data: TokenData<User> =
        decode(token, verifying_key, &Validation::new(Algorithm::HS256))?;

    Ok(token_data.claims)
}

pub fn get_access_token_from_auth_header(req: HttpRequest) -> Option<String> {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|header_value| header_value.to_str().ok())
        .and_then(|header| {
            if header.starts_with("Bearer ") {
                header.split_whitespace().nth(1)
            } else {
                None
            }
        })
        .map(|header| header.to_string());

    token
}
