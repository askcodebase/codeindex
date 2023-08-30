use std::panic;

use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use colored::*;
use redis::cluster::ClusterClient;
use redis::AsyncCommands;
use serde_json::Value as JsonValue;
use std::error::Error;

#[derive(serde::Serialize)]
struct CodeIndexResponse {
    errcode: u16,
    data: JsonValue,
}

enum CodeIndexError {
    SomethingWentWrong,
}

impl IntoResponse for CodeIndexError {
    fn into_response(self) -> Response {
        let body = match self {
            CodeIndexError::SomethingWentWrong => "Something went wrong",
        };

        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}

async fn handler() -> Result<(), CodeIndexError> {
    Err(CodeIndexError::SomethingWentWrong)
}

const ERROR_MESSAGE_REDIS_CONNECTION: &str =
    "Failed to connect to redis cluster server. Please check: 
        1. Make sure redis is running and listening on port 6739. 
        2. Make sure redis cluster support is enabled.";
const SERVER_ENDPOINT: &'static str = "0.0.0.0:3000";

async fn fetch_an_integer() -> Result<String, Box<dyn Error>> {
    let nodes = vec!["redis://127.0.0.1:6739"];
    let client = ClusterClient::new(nodes).map_err(|_| ERROR_MESSAGE_REDIS_CONNECTION)?;

    let mut connection = client
        .get_async_connection()
        .await
        .map_err(|_| ERROR_MESSAGE_REDIS_CONNECTION)?;
    let _: () = connection.set("test", "test_data").await?;
    let rv: String = connection.get("test").await?;
    Ok(rv)
}

#[tokio::main]
async fn main() {
    panic::set_hook(Box::new(|panic_info| {
        match panic_info.payload().downcast_ref::<&str>() {
            Some(s) => {
                eprintln!("Caught panic with message: {}", s);
            }
            None => match panic_info.payload().downcast_ref::<String>() {
                Some(s) => {
                    eprintln!("Caught panic with message: {}", s);
                }
                None => {
                    eprintln!("Caught panic with unknown message");
                }
            },
        }
    }));

    match fetch_an_integer().await {
        Ok(value) => print!("value: {:?}", &value),
        Err(error) => eprintln!("{}", format!("Error: {}", error).red()),
    }

    let app = Router::new()
        .route("/", get(root))
        .route("/:file", get(handler));

    println!("Server started at: {}", &SERVER_ENDPOINT);
    axum::Server::bind(&SERVER_ENDPOINT.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn root() -> Html<&'static str> {
    Html("<h1>CodeIndex API</h1>")
}
