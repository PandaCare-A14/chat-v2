mod db;
mod server;
mod types;
mod utils;

use actix_web::{App, HttpServer};
use server::ChatServer;
use std::io::{Error, ErrorKind, Result};
use tokio::{spawn, try_join};

#[actix_web::main]
async fn main() -> Result<()> {
    let db_uri_str = std::env::var("DATABASE_URI")
        .map_err(|err| Error::new(ErrorKind::Other, err.to_string()))?;

    let db_client = mongodb::Client::with_uri_str(db_uri_str)
        .await
        .map_err(|err| Error::new(ErrorKind::Other, err.to_string()))?;

    let (chat_server, server_tx) = ChatServer::new();

    let chat_server_handle = spawn(chat_server.run());

    let http_server = HttpServer::new(move || App::new())
        .workers(4)
        .bind(("0.0.0.0", 8080))?
        .run();

    try_join!(http_server, async move { chat_server_handle.await.unwrap() })?;

    Ok(())
}
