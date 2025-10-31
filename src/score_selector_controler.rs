use axum::extract::{Path, State};
use axum::Json;

use crate::db::cycleway::Cycleway;
use crate::VeloinfoState;

pub async fn score_bounds_controler(
    State(state): State<VeloinfoState>,
    Path(score): Path<i32>,
) -> Json<Vec<Cycleway>> {
    let geom = match Cycleway::get_by_score_id(&score, &state.conn).await {
        Ok(response) => response,
        Err(e) => {
            eprintln!("Error while fetching cycleways: {}", e);
            vec![]
        }
    };
    Json(geom)
}
