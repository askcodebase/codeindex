use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
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

#[tokio::main]
async fn main() {
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
