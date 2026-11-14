use askama::Template;
use axum::http::header;
use axum::response::IntoResponse;
use chrono::{Duration, Utc};
use chrono_tz::America::Montreal;
use std::env;

#[derive(Template)]
#[template(path = "style.json", escape = "none")]
pub struct Style {
    veloinfo_url: String,
    road_work_cache_key: String,
}

/// Clé de cache des tuiles de travaux.
///
/// Les travaux sont importés chaque nuit à 1 h (heure de Montréal). On décale
/// la date de deux heures pour que la clé change après l'import, même si une
/// page a été ouverte juste avant 1 h. L'URL des tuiles contient cette date,
/// ce qui empêche un cache navigateur/CDN de servir des travaux de la veille.
fn road_work_cache_key() -> String {
    (Utc::now() - Duration::hours(2))
        .with_timezone(&Montreal)
        .format("%Y-%m-%d")
        .to_string()
}

pub async fn style() -> impl IntoResponse {
    let veloinfo_url = env::var("VELOINFO_URL").unwrap();

    let body = Style {
        veloinfo_url,
        road_work_cache_key: road_work_cache_key(),
    }
    .render()
    .unwrap();

    (
        [
            (
                header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            ),
            (
                header::CACHE_CONTROL,
                "no-store, max-age=0, must-revalidate",
            ),
        ],
        body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_cle_de_cache_est_une_date() {
        let key = road_work_cache_key();
        assert_eq!(key.len(), 10);
        assert_eq!(key.chars().filter(|c| *c == '-').count(), 2);
    }
}
