use crate::db::edge::Edge;
use axum::response::IntoResponse;
use axum::{extract::State, Json};
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
    let city_name = payload.name.clone();
    if payload.snow {
        match sqlx::query(
            r#"insert into city_snow (city_name)
                select $1
                where not exists (
                    select 1 from city_snow where city_name = $1
                )
        "#,
        )
        .bind(&city_name)
        .execute(conn)
        .await
        {
            Ok(_) => (),
            Err(e) => eprintln!("Error updating CitySnow: {}", e),
        };
    } else {
        match sqlx::query(
            r#"delete from city_snow where city_name = $1
        "#,
        )
        .bind(&payload.name)
        .execute(conn)
        .await
        {
            Ok(_) => (),
            Err(e) => eprintln!("Error updating CitySnow: {}", e),
        };
    }

    let conn = conn.clone();
    match Edge::get_edge_by_city(&city_name, &conn).await {
        Ok(edge_ids) => {
            let node_ids: Vec<i64> = edge_ids.into_iter().fold(Vec::new(), |mut acc, e| {
                acc.push(e.source);
                acc.push(e.target);
                acc
            });
            Edge::clear_nodes_cache(node_ids, &conn).await;
        }
        Err(e) => {
            eprintln!("Error clearing edge cache for city {}: {}", city_name, e);
        }
    }

    Json(payload)
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
                            'properties', to_jsonb(c) - 'geom' || jsonb_build_object('snow', true)
                        ) AS feature
                    FROM city c
                    INNER JOIN city_snow cs ON c.name = cs.city_name
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
