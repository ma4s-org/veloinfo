use std::env;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::VeloinfoState;

#[axum::debug_handler]
pub async fn bike_path_mvt(
    State(state): State<VeloinfoState>,
    Path((z, x, y)): Path<(u32, u32, u32)>,
) -> impl IntoResponse {
    let conn = &state.conn;
    let query = r#"
        WITH
        bounds AS (
            SELECT ST_TileEnvelope($1, $2, $3) AS geom
        ),
        all_bike_paths AS (
            SELECT
                t.geom,
                t.tags,
                COALESCE(s.score, 1) as score,
                CASE
                    -- 1. Pistes dédiées (tracks, etc.)
                    WHEN 
                        t.tags->>'highway' = 'cycleway' OR
                        t.tags->>'cyclestreet' = 'yes' OR
                        t.tags->>'cycleway' = 'track' OR
                        t.tags->>'cycleway:left' = 'track' OR
                        t.tags->>'cycleway:right' = 'track' OR
                        t.tags->>'cycleway:both' = 'track'
                    THEN CASE WHEN t.tags->>'cycleway' = 'crossing' THEN 'cycleway_crossing' ELSE 'cycleway' END
                    
                    -- 2. Voies désignées (partagées avec bus ou marquées sur le côté)
                    WHEN 
                        t.tags->>'cycleway:left' = 'share_busway' OR
                        t.tags->>'cycleway:right' = 'share_busway' OR
                        t.tags->>'cycleway:both' = 'share_busway' OR
                        t.tags->>'cycleway:right' = 'lane' OR
                        t.tags->>'cycleway:left' = 'lane' OR
                        t.tags->>'cycleway:both' = 'lane'
                    THEN 'designated'
                    
                    -- 3. Voies partagées (cas plus généraux)
                    WHEN 
                        t.tags->>'cycleway' = 'shared_lane' OR
                        (t.tags->>'cycleway' = 'lane' AND t.tags->>'cycleway:left' IS NULL AND t.tags->>'cycleway:right' IS NULL AND t.tags->>'cycleway:both' IS NULL) OR
                        t.tags->>'cycleway:left' = 'shared_lane' OR
                        t.tags->>'cycleway:left' = 'opposite_lane' OR
                        t.tags->>'cycleway:right' = 'shared_lane' OR
                        t.tags->>'cycleway:right' = 'opposite_lane' OR
                        t.tags->>'cycleway:both' = 'shared_lane' OR
                        (t.tags->>'highway' = 'footway' AND t.tags->>'bicycle' = 'yes')
                    THEN  'shared_lane'
                END as kind
            FROM
                all_way t
            LEFT JOIN cyclability_score s ON t.way_id = ANY(s.way_ids)
        ),
        mvtgeom AS (
            SELECT
                ST_AsMVTGeom(
                    ST_Transform(t.geom, 3857),
                    bounds.geom
                ) AS geom,
                t.tags,
                t.score,
                t.kind
            FROM
                all_bike_paths t, bounds
            WHERE
                t.kind IS NOT NULL AND
                ST_Intersects(t.geom, ST_Transform(bounds.geom, 3857))
        )
        SELECT ST_AsMVT(mvtgeom.*, 'bike_path', 4096, 'geom')
        FROM mvtgeom;
    "#;

    let tile: Result<Option<Vec<u8>>, sqlx::Error> = sqlx::query_scalar(query)
        .bind(z as i32)
        .bind(x as i32)
        .bind(y as i32)
        .fetch_one(conn)
        .await;
    match tile {
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

pub async fn bike_path() -> impl IntoResponse {
    let tilejson = serde_json::json!({
        "tilejson": "3.0.0",
        "name": "bike_path",
        "tiles": [
            format!("{}/bike_path/{{z}}/{{x}}/{{y}}", env::var("VELOINFO_URL").unwrap())
        ],
        "vector_layers": [
            {
                "id": "bike_path",
                "fields": {
                    "tags": "String",
                    "score": "Number",
                    "kind": "String"
                },
                "minzoom": 0,
                "maxzoom": 22
            }
        ]
    });
    Json(tilejson)
}
