use crate::VeloinfoState;
use askama::Template;
use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use chrono::{Duration, Utc};
use chrono_tz::America::Montreal;
use sqlx::PgPool;
use std::env;

#[derive(Template)]
#[template(path = "style.json", escape = "none")]
pub struct Style {
    veloinfo_url: String,
    road_work_cache_key: String,
}

/// Partie datée de la clé de cache des tuiles de travaux.
///
/// Les travaux sont importés chaque nuit à 1 h (heure de Montréal). On décale
/// la date de deux heures pour que la clé change après l'import, même si une
/// page a été ouverte juste avant 1 h.
fn road_work_date_key() -> String {
    (Utc::now() - Duration::hours(2))
        .with_timezone(&Montreal)
        .format("%Y-%m-%d")
        .to_string()
}

/// Clé de cache qui change à chaque import réussi.
///
/// La date seule ne suffit pas : un import manuel ou un redémarrage en cours
/// de journée ne change pas la date, et le navigateur peut alors conserver les
/// tuiles d'avant l'import. Le couple `(MAX(id), COUNT(*))` change dès que la
/// table `road_work` est remplacée, ce qui force le rechargement des tuiles.
async fn road_work_cache_key(conn: &PgPool) -> String {
    let date = road_work_date_key();

    match sqlx::query_as::<_, (i64, i64)>(
        "SELECT COALESCE(MAX(id), 0)::bigint, COUNT(*)::bigint FROM road_work",
    )
    .fetch_one(conn)
    .await
    {
        Ok((max_id, count)) => format!("{}-{}-{}", date, max_id, count),
        Err(error) => {
            eprintln!("Road work cache key: {:#}", error);
            date
        }
    }
}

pub async fn style(State(state): State<VeloinfoState>) -> impl IntoResponse {
    let veloinfo_url = env::var("VELOINFO_URL").unwrap();

    let body = Style {
        veloinfo_url,
        road_work_cache_key: road_work_cache_key(&state.conn).await,
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
        let key = road_work_date_key();
        assert_eq!(key.len(), 10);
        assert_eq!(key.chars().filter(|c| *c == '-').count(), 2);
    }
}
