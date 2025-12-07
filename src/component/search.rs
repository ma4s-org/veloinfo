use axum::{extract::State, Form, Json};
use lazy_static::lazy_static;
use regex::Regex;
use serde::Serialize;

use crate::{
    db::search_db::{get, get_with_adress},
    VeloinfoState,
};

#[derive(Debug, Serialize)]
pub struct SearchResults {
    query: String,
    search_results: Vec<SearchResult>,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub name: String,
    pub lat: f64,
    pub lng: f64,
}

#[derive(serde::Deserialize, Debug)]
pub struct QueryParams {
    pub query: String,
    pub lat: f64,
    pub lng: f64,
}

lazy_static! {
    static ref ADDRESS_RE: Regex = Regex::new(r"(\d+) (.*)").unwrap();
}

pub async fn post(
    State(state): State<VeloinfoState>,
    Form(query): Form<QueryParams>,
) -> Json<SearchResults> {
    match ADDRESS_RE.captures(&query.query) {
        Some(caps) => {
            let number = caps.get(1).unwrap().as_str().parse::<i64>().unwrap();
            let original_sub_query = caps.get(2).unwrap().as_str().to_string();

            // On essaie la requête complète puis on retire le dernier mot successivement
            // jusqu'à obtenir des résultats ou arriver à une requête d'un seul mot.
            let mut attempt = original_sub_query.trim().to_string();
            let mut rows = Vec::new();

            while !attempt.is_empty() {
                rows =
                    get_with_adress(&number, &attempt, &query.lng, &query.lat, &state.conn).await;

                if !rows.is_empty() {
                    break;
                }

                // Si il ne reste qu'un mot, on s'arrête (on a déjà essayé ce mot)
                let count = attempt.split_whitespace().count();
                if count <= 1 {
                    break;
                }

                // Supprimer le dernier mot
                if let Some(pos) = attempt.rfind(' ') {
                    attempt.truncate(pos);
                    attempt = attempt.trim_end().to_string();
                } else {
                    // Pas d'espace trouvé, on vide pour sortir de la boucle
                    attempt.clear();
                }
            }

            let search_results = rows
                .into_iter()
                .map(|ar| SearchResult {
                    name: ar.name,
                    lat: ar.lat,
                    lng: ar.lng,
                })
                .collect();

            SearchResults {
                query: query.query,
                search_results,
            }
            .into()
        }
        None => {
            let search_results = get(&query.query, &query.lng, &query.lat, &state.conn)
                .await
                .into_iter()
                .map(|ar| SearchResult {
                    name: ar.name,
                    lat: ar.lat,
                    lng: ar.lng,
                })
                .collect();
            SearchResults {
                query: query.query,
                search_results,
            }
            .into()
        }
    }
}
