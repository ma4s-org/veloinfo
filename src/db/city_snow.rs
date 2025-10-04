use axum::response::IntoResponse;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

use crate::VeloinfoState;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct CitySnow {
    name: String,
    snow: bool,
}

#[axum::debug_handler]
pub async fn post_city_snow(
    State(state): State<VeloinfoState>,
    Json(payload): Json<CitySnow>,
) -> impl IntoResponse {
    let conn = &state.conn;
    match sqlx::query(
        r#"UPDATE city 
            SET snow = $2 
            WHERE name = $1;
            "#,
    )
    .bind(&payload.name)
    .bind(&payload.snow)
    .execute(conn)
    .await
    {
        Ok(_) => (),
        Err(e) => eprintln!("Error updating CitySnow: {}", e),
    };
    Json(payload)
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
                FROM city cs
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

pub async fn city_snow_geojson(State(state): State<VeloinfoState>) -> impl IntoResponse {
    let conn = &state.conn;
    let geojson: serde_json::Value = match sqlx::query_scalar(
        r#"SELECT
                    jsonb_build_object(
                        'type', 'FeatureCollection',
                        'features', jsonb_agg(feature)
                    )
                FROM (
                    SELECT
                        jsonb_build_object(
                            'type', 'Feature',
                            'geometry', ST_AsGeoJSON(ST_Transform(c.geom, 4326))::jsonb,
                            'properties', to_jsonb(c) - 'geom'
                        ) AS feature
                    FROM city c
                    WHERE c.snow = true
                ) AS features;
            "#,
    )
    .fetch_one(conn)
    .await
    {
        Ok(geo) => geo,
        Err(e) => {
            eprintln!("Error getting CitySnow GeoJSON: {}", e);
            serde_json::json!({"type": "FeatureCollection", "features": []})
        }
    };
    Json(geojson)
}
