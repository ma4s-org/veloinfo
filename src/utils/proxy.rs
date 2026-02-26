use std::env;

use axum::response::IntoResponse;

pub async fn martin_proxy(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl axum::response::IntoResponse {
    let martin_url = env::var("MARTIN_URL").unwrap();
    let url = format!("{}/{}", martin_url, path);

    match reqwest::get(&url).await {
        Ok(response) => {
            let status = axum::http::StatusCode::from_u16(response.status().as_u16())
                .unwrap_or(axum::http::StatusCode::BAD_GATEWAY);
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_string();
            let body_bytes = response.bytes().await.unwrap_or_default();
            let veloinfo_url = env::var("VELOINFO_URL").unwrap();
            let body = if content_type.contains("application/json") {
                String::from_utf8_lossy(&body_bytes)
                    .replace(&martin_url, &format!("{}/martin", veloinfo_url))
                    .into_bytes()
                    .into()
            } else {
                body_bytes
            };
            (
                status,
                [(axum::http::header::CONTENT_TYPE, content_type)],
                body,
            )
                .into_response()
        }
        Err(_) => axum::http::StatusCode::BAD_GATEWAY.into_response(),
    }
}
