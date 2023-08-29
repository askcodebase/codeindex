use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use redis::cluster::ClusterClient;
use redis::AsyncCommands;
use serde_json::Value as JsonValue;

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

async fn fetch_an_integer() -> String {
    let nodes = vec!["redis://127.0.0.1:6739"];
    let client = ClusterClient::new(nodes).unwrap();
    let mut connection = client.get_async_connection().await.expect(
        format!(
            "\n[Error] {}\n\t{}\n\t{}\n",
            "Failed to connect to redis cluster server. Please check:",
            "1. Whether redis is running and listening on port 6739.",
            "2. Whether redis cluster support is enabled."
        )
        .as_str(),
    );
    let _: () = connection.set("test", "test_data").await.unwrap();
    let rv: String = connection.get("test").await.unwrap();
    return rv;
}

#[tokio::main]
async fn main() {
    let result = fetch_an_integer();
    print!("{:?}", result.await);

    let app = Router::new()
        .route("/", get(root))
        .route("/:file", get(handler));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn root() -> Html<&'static str> {
    Html("<h1>CodeIndex API</h1>")
}
