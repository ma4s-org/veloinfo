use askama_axum::IntoResponse;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use sqlx::prelude::FromRow;

use crate::VeloinfoState;

#[derive(Debug, Serialize, FromRow)]
pub struct CitySnow {
    name: String,
    snow: bool,
}

pub async fn get_city_snow(
    Path((lng, lat)): Path<(f32, f32)>,
    State(state): State<VeloinfoState>,
) -> impl IntoResponse {
    let conn = &state.conn;
    let city_snow: CitySnow = match sqlx::query_as(
        r#"SELECT
                    name,
                    snow
                FROM city_snow cs
                WHERE ST_DWithin(cs.geom, ST_Transform(ST_SetSRID(ST_MakePoint($1, $2), 4326), 3857), 0)
            "#,
    )
    .bind(lng)
    .bind(lat)
    .fetch_one(conn)
    .await
    {
        Ok(ar) => ar,
        Err(e) => {
            eprintln!("Error getting CitySnow: {}", e);
            CitySnow {
                name: "".to_string(),
                snow: false,
            }
        }
    };
    Json(city_snow)
}
