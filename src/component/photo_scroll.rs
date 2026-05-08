use crate::VeloinfoState;
use axum::{
    debug_handler,
    extract::{Path, State},
    Json,
};
use lazy_static::lazy_static;
use regex::Regex;
use serde::Serialize;

#[derive(Serialize)]
pub struct PhotoScroll {
    pub photo: String,
    pub next: Option<String>,
    pub previous: Option<String>,
    pub way_ids: String,
}

lazy_static! {
    static ref INT_REGEX: Regex = Regex::new(r"\d+").unwrap();
}

#[debug_handler]
pub async fn photo_scroll(
    State(_state): State<VeloinfoState>,
    Path((photo, _way_ids)): Path<(String, String)>,
) -> Json<PhotoScroll> {
    // TODO: Implémenter photo_scroll par geom pour les segments personnalisés
    // Pour l'instant, retourne une réponse vide
    Json(PhotoScroll {
        photo,
        next: None,
        previous: None,
        way_ids: _way_ids,
    })
}
