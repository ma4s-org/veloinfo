use axum::extract::{Path, State};
use axum::Json;

use crate::db::cycleway::Cycleway;
use crate::VeloinfoState;

pub async fn report_bounds_controler(
    State(state): State<VeloinfoState>,
    Path(report_id): Path<i32>,
) -> Json<Vec<Cycleway>> {
    let geom = match Cycleway::get_by_report_id(&report_id, &state.conn).await {
        Ok(response) => response,
        Err(e) => {
            eprintln!("Error while fetching cycleways: {}", e);
            vec![]
        }
    };
    Json(geom)
}