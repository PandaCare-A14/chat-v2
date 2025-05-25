mod chat_server;
mod db;
mod server;
mod utils;
mod handler;

use actix_web::{App, HttpServer, web};
use chat_server::ChatServer;
use dotenvy::dotenv;
use handler::ws_connect;
use jsonwebtoken::DecodingKey;
use server::rest_scope;
use std::io::{Error, ErrorKind, Result};
use tokio::spawn;
use tokio::signal::unix::{signal, SignalKind};
use utils::{get_db_client, get_jwk};

#[actix_web::main]
async fn main() -> Result<()> {
    use actix_web::middleware::Logger;
    use env_logger;
    use log;

    match dotenv() {
        Ok(_) => {}
        Err(_err) => {}
    };

    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let jwk = get_jwk(
        &std::env::var("JWK_SET_URI")
            .map_err(|err| Error::new(ErrorKind::Other, err.to_string()))?,
    )
    .await
    .map_err(|err| Error::new(ErrorKind::Other, err.to_string()))?;

    let db_client = get_db_client().await?;

    let verifying_key =
        DecodingKey::from_jwk(&jwk).map_err(|err| Error::new(ErrorKind::Other, err.to_string()))?;

    let (chat_server, chat_handle) = ChatServer::new();

    let chat_server_handle = spawn(chat_server.run(db_client.clone()));

    let http_server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(db_client.clone()))
            .app_data(web::Data::new(verifying_key.clone()))
            .app_data(web::Data::new(chat_handle.clone()))
            .service(web::scope("/api").route("/ws", web::get().to(ws_connect)).service(web::scope("/rest").configure(rest_scope)))
            .wrap(Logger::default())
    })
    .workers(4)
    .bind(("0.0.0.0", 8080))?
    .run();

    let mut term_signal = signal(SignalKind::terminate())?;
    let mut int_signal = signal(SignalKind::interrupt())?;

    tokio::select! {
        _ = http_server => println!("HTTP server stopped"),
        _ = chat_server_handle => println!("Chat server stopped"),
        _ = term_signal.recv() => println!("Received SIGTERM"),
        _ = int_signal.recv() => println!("Received SIGINT"),
    }

    println!("Shutting down...");
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    Ok(())
}
