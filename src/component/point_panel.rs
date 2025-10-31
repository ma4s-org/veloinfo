use crate::{db::search_db, VeloinfoState};
use axum::{
    extract::{Path, State},
    Json,
};
use geojson::JsonValue;
use serde_json::json;

pub async fn point_panel_lng_lat(
    Path((lng, lat)): Path<(f64, f64)>,
    state: State<VeloinfoState>,
) -> Json<JsonValue> {
    let name = match search_db::get_any(&lng, &lat, &state.conn).await.first() {
        Some(ar) => ar.name.clone(),
        None => "".to_string(),
    };
    Json(json!({
        "name": name
    }))
}
