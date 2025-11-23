use std::env;

use crate::db::edge::Edge;
use axum::body::Body;
use axum::extract::Path;
use axum::http::header;
use axum::http::Response;
use axum::response::IntoResponse;
use axum::{extract::State, Json};
use reqwest::StatusCode;
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
    tokio::spawn(async move {
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
    });
    Json(payload)
}

pub async fn city_snow_mvt(
    State(state): State<VeloinfoState>,
    Path((z, x, y)): Path<(i32, i32, i32)>,
) -> impl IntoResponse {
    let conn = &state.conn;
    let tiles: Result<Option<Vec<u8>>, sqlx::Error> = sqlx::query_scalar(
        r#"        
        WITH
        bounds AS (
            SELECT ST_TileEnvelope($1, $2, $3) AS geom
        ),
        city_snow_data AS (
            SELECT
                city.name AS city_name,
                city.geom,
                (cs.city_name IS NOT NULL) AS snow
            FROM
                city
            LEFT JOIN city_snow cs ON city.name = cs.city_name
        ),
        mvtgeom AS (
            SELECT
                ST_AsMVTGeom(
                    ST_Transform(csd.geom, 3857),
                    bounds.geom
                ) AS geom,
                csd.snow
            FROM
                city_snow_data csd, bounds
            WHERE
                csd.snow
        )
        SELECT ST_AsMVT(mvtgeom.*, 'city_snow', 4096, 'geom')
        FROM mvtgeom;"#,
    )
    .bind(z)
    .bind(x)
    .bind(y)
    .fetch_one(conn)
    .await;
    match tiles {
        Ok(Some(mvt)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/vnd.mapbox-vector-tile")
            .body(Body::from(mvt))
            .unwrap()
            .into_response(),
        Ok(None) => Response::builder() // No tile found, return empty response
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .unwrap()
            .into_response(),
        Err(e) => {
            eprintln!("Error fetching tile: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Error fetching tile").into_response()
        }
    }
}

pub async fn city_snow() -> impl IntoResponse {
    let tilejson = serde_json::json!({
        "tilejson": "3.0.0",
        "name": "city_snow",
        "tiles": [
            format!("{}/city_snow/{{z}}/{{x}}/{{y}}", env::var("VELOINFO_URL").unwrap())
        ],
        "vector_layers": [
            {
                "id": "city_snow",
                "fields": {
                    "tags": "String",
                    "score": "Number",
                    "kind": "String",
                    "snow": "Boolean"
                },
                "minzoom": 0,
                "maxzoom": 22
            }
        ]
    });
    Json(tilejson)
}
