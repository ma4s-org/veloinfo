use askama::Template;
use axum::{extract::State, Form};
use axum_macros::debug_handler;
use lazy_static::lazy_static;
use regex::Regex;

use crate::{
    db::search_db::{get, get_with_adress},
    VeloinfoState,
};

#[derive(Template, Debug)]
#[template(path = "search_result.html")]
pub struct SearchResults {
    query: String,
    search_results: Vec<SearchResult>,
}

#[derive(Debug)]
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

#[debug_handler]
pub async fn post(
    State(state): State<VeloinfoState>,
    Form(query): Form<QueryParams>,
) -> SearchResults {
    match ADDRESS_RE.captures(&query.query) {
        Some(caps) => {
            let number = caps.get(1).unwrap().as_str().parse::<i64>().unwrap();
            let sub_query = caps.get(2).unwrap().as_str().to_string();
            let search_results =
                get_with_adress(&number, &sub_query, &query.lng, &query.lat, &state.conn)
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
        }
    }
}
