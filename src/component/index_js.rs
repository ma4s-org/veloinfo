use askama::Template;
use askama_axum::IntoResponse;
use axum::http::{HeaderMap, HeaderValue};

#[derive(Template)]
#[template(path = "index.js", escape = "none")]
struct IndexJs {}

pub async fn indexjs() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        HeaderValue::from_static("application/javascript"),
    );
    let resp = IndexJs {};
    (headers, resp)
}
