use std::io::{self, Error, ErrorKind};

use actix_web::{HttpRequest, http::header};
use jsonwebtoken::{
    Algorithm, DecodingKey, TokenData, Validation, decode,
    jwk::{Jwk, JwkSet},
};
use mongodb::{Client, bson::Uuid};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct User {
    user_id: Uuid,
}

impl User {
    pub fn user_id(&self) -> Uuid {
        self.user_id
    }
}

pub fn get_user_details(
    token: &str,
    verifying_key: &DecodingKey,
) -> Result<User, jsonwebtoken::errors::Error> {
    let token_data: TokenData<User> =
        decode(token, verifying_key, &Validation::new(Algorithm::RS256))?;

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

pub async fn get_jwk(url: &str) -> std::io::Result<Jwk> {
    let response = reqwest::get(url)
        .await
        .unwrap()
        .json::<JwkSet>()
        .await
        .unwrap();
    let jwk: &Jwk = response.keys.first().unwrap();
    Ok(jwk.clone())
}

pub async fn get_db_client() -> Result<Client, io::Error> {
    let db_uri_str = std::env::var("DATABASE_URI")
        .map_err(|err| Error::new(ErrorKind::Other, err.to_string()))?;

    let db_client = mongodb::Client::with_uri_str(db_uri_str)
        .await
        .map_err(|err| Error::new(ErrorKind::Other, err.to_string()))?;

    Ok(db_client)
}
