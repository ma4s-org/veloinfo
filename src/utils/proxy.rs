use std::env;
use std::sync::OnceLock;
use std::time::Duration;

use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;

/// Client HTTP partagé vers Martin.
///
/// On réutilise les connexions (keep-alive) et on borne l'attente : sans
/// timeout, une tuile bloquée côté Martin finit par saturer les connexions de
/// Veloinfo.
static MARTIN_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn martin_client() -> &'static reqwest::Client {
    MARTIN_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("client HTTP Martin")
    })
}

/// Une requête de tuile se termine par `{z}/{x}/{y}` (éventuellement `.pbf`).
///
/// Les autres chemins (`road_work`, `all_way`, ...) renvoient du TileJSON :
/// on ne les compresse pas pour pouvoir réécrire l'URL interne de Martin.
fn is_tile_path(path: &str) -> bool {
    let mut segments = path.rsplit('/');
    let y = segments.next().map(|s| s.trim_end_matches(".pbf"));
    let x = segments.next().map(|s| s.trim_end_matches(".pbf"));
    let z = segments.next().map(|s| s.trim_end_matches(".pbf"));
    match (z, x, y) {
        (Some(z), Some(x), Some(y)) => {
            z.parse::<u32>().is_ok() && x.parse::<u32>().is_ok() && y.parse::<u32>().is_ok()
        }
        _ => false,
    }
}

pub async fn martin_proxy(
    headers: HeaderMap,
    axum::extract::Path(path): axum::extract::Path<String>,
    axum::extract::RawQuery(query): axum::extract::RawQuery,
) -> impl IntoResponse {
    let martin_url = env::var("MARTIN_URL").unwrap();
    // On transmet la query (`?v=<version>`) à Martin. Le navigateur et le CDN
    // l'utilisent comme clé de cache : un nouvel import change la version et
    // les anciennes tuiles ne sont plus demandées.
    let url = match query {
        Some(query) if !query.is_empty() => format!("{}/{}?{}", martin_url, path, query),
        _ => format!("{}/{}", martin_url, path),
    };
    let is_road_work = path == "road_work" || path.starts_with("road_work/");
    let is_tile = is_tile_path(&path);

    let mut request = martin_client().get(&url);

    // Sans ce transfert, Martin renvoie les tuiles non compressées et le proxy
    // les transmet telles quelles : une tuile de travaux de Montréal passe de
    // ~50 Ko (gzip) à ~120 Ko.
    if is_tile {
        if let Some(value) = headers.get(header::ACCEPT_ENCODING) {
            request = request.header(header::ACCEPT_ENCODING, value.clone());
        }
    }
    if let Some(value) = headers.get(header::IF_NONE_MATCH) {
        request = request.header(header::IF_NONE_MATCH, value.clone());
    }

    match request.send().await {
        Ok(response) => {
            let status =
                StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_string();
            let content_encoding = response
                .headers()
                .get(header::CONTENT_ENCODING)
                .cloned();
            let etag = response.headers().get(header::ETAG).cloned();
            let body_bytes = response.bytes().await.unwrap_or_default();

            // Les requêtes internes (Chromium headless) doivent rester sur
            // 127.0.0.1. Les requêtes publiques gardent l'URL publique.
            let public_url = env::var("VELOINFO_URL").unwrap();
            let origin = headers
                .get("host")
                .and_then(|value| value.to_str().ok())
                .filter(|host| host.starts_with("127.0.0.1") || host.starts_with("localhost"))
                .map(|host| format!("http://{}", host))
                .unwrap_or(public_url);
            let body = if content_type.contains("application/json") && content_encoding.is_none() {
                String::from_utf8_lossy(&body_bytes)
                    .replace(&martin_url, &format!("{}/martin", origin))
                    .into_bytes()
                    .into()
            } else {
                body_bytes
            };

            // Les URL de tuiles de travaux contiennent `?v=<version>` : un
            // nouvel import change l'URL, donc on peut laisser le navigateur
            // et le CDN garder les tuiles une journée. Sans cela, chaque
            // déplacement de carte redemandait toutes les tuiles à PostgreSQL.
            let cache_control = if is_road_work {
                if is_tile {
                    "public, max-age=86400, stale-while-revalidate=604800"
                } else {
                    "no-store"
                }
            } else {
                "public, max-age=604800, stale-while-revalidate=15768000"
            };

            let mut response_headers = HeaderMap::new();
            response_headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(&content_type)
                    .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
            );
            response_headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache_control));
            if is_tile {
                response_headers.insert(
                    header::VARY,
                    HeaderValue::from_static("Accept-Encoding"),
                );
            }
            if let Some(value) = content_encoding {
                response_headers.insert(header::CONTENT_ENCODING, value);
            }
            if let Some(value) = etag {
                response_headers.insert(header::ETAG, value);
            }

            (status, response_headers, body).into_response()
        }
        Err(_) => StatusCode::BAD_GATEWAY.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::is_tile_path;

    #[test]
    fn reconnait_les_tuiles() {
        assert!(is_tile_path("road_work/11/605/732"));
        assert!(is_tile_path("all_way/14/4800/5865.pbf"));
        assert!(!is_tile_path("road_work"));
        assert!(!is_tile_path("road_work/11/605"));
    }
}
