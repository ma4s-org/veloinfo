use std::env;

use axum::http::HeaderMap;
use axum::response::IntoResponse;

pub async fn martin_proxy(
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl axum::response::IntoResponse {
    let martin_url = env::var("MARTIN_URL").unwrap();
    let url = format!("{}/{}", martin_url, path);
    // Les travaux ne doivent jamais être mis en cache :
    // - Martin a `cache: disable` pour la source road_work (martin/config.yaml)
    // - ici on empêche aussi le cache navigateur/CDN avec `no-store`.
    let is_road_work = path == "road_work" || path.starts_with("road_work/");
    let max_age = 604800; // 7 jours

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
            // Les requêtes internes (Chromium headless) doivent rester sur
            // 127.0.0.1. Les requêtes publiques gardent l'URL publique.
            let public_url = env::var("VELOINFO_URL").unwrap();
            let origin = headers
                .get("host")
                .and_then(|value| value.to_str().ok())
                .filter(|host| {
                    host.starts_with("127.0.0.1") || host.starts_with("localhost")
                })
                .map(|host| format!("http://{}", host))
                .unwrap_or(public_url);
            let body = if content_type.contains("application/json") {
                String::from_utf8_lossy(&body_bytes)
                    .replace(&martin_url, &format!("{}/martin", origin))
                    .into_bytes()
                    .into()
            } else {
                body_bytes
            };
            (
                status,
                [
                    (axum::http::header::CONTENT_TYPE, content_type),
                    (
                        axum::http::header::CACHE_CONTROL,
                        if is_road_work {
                            "no-store".to_string()
                        } else {
                            format!("public, max-age={max_age}, stale-while-revalidate=15768000")
                        },
                    ),
                ],
                body,
            )
                .into_response()
        }
        Err(_) => axum::http::StatusCode::BAD_GATEWAY.into_response(),
    }
}
