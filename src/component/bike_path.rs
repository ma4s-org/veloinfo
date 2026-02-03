use std::env;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use geojson::JsonValue;

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
                aw.geom,
                aw.tags,
                COALESCE(cscore.score, 1) as score,
                cs.city_name IS NOT NULL as snow,
                CASE
                    -- 1. Pistes dédiées (tracks, etc.)
                    WHEN 
                        aw.tags->>'highway' = 'cycleway' OR
                        aw.tags->>'cyclestreet' = 'yes' OR
                        aw.tags->>'cycleway' = 'track' OR
                        aw.tags->>'cycleway:left' = 'track' OR
                        aw.tags->>'cycleway:right' = 'track' OR
                        aw.tags->>'cycleway:both' = 'track'
                    THEN CASE WHEN aw.tags->>'cycleway' = 'crossing' THEN 'cycleway_crossing' ELSE 'cycleway' END
                    
                    -- 2. Voies désignées (partagées avec bus ou marquées sur le côté)
                    WHEN 
                        aw.tags->>'cycleway:left' = 'share_busway' OR
                        aw.tags->>'cycleway:right' = 'share_busway' OR
                        aw.tags->>'cycleway:both' = 'share_busway' OR
                        aw.tags->>'cycleway:right' = 'lane' OR
                        aw.tags->>'cycleway:left' = 'lane' OR
                        aw.tags->>'cycleway:both' = 'lane'
                    THEN 'designated'
                    
                    -- 3. Voies partagées (cas plus généraux)
                    WHEN 
                        aw.tags->>'cycleway' = 'shared_lane' OR
                        (aw.tags->>'cycleway' = 'lane' AND aw.tags->>'cycleway:left' IS NULL AND aw.tags->>'cycleway:right' IS NULL AND aw.tags->>'cycleway:both' IS NULL) OR
                        aw.tags->>'cycleway:left' = 'shared_lane' OR
                        aw.tags->>'cycleway:left' = 'opposite_lane' OR
                        aw.tags->>'cycleway:right' = 'shared_lane' OR
                        aw.tags->>'cycleway:right' = 'opposite_lane' OR
                        aw.tags->>'cycleway:both' = 'shared_lane' OR
                        (aw.tags->>'highway' = 'footway' AND aw.tags->>'bicycle' = 'yes')
                    THEN  'shared_lane'
                END as kind
            FROM
                all_way aw
            LEFT JOIN cyclability_score cscore ON aw.way_id = ANY(cscore.way_ids)
            LEFT JOIN city ON ST_Intersects(aw.geom, city.geom)
            LEFT JOIN city_snow cs ON city.name = cs.city_name
            WHERE COALESCE(aw.tags->>'bicycle', 'yes') <> 'no'
        ),
        mvtgeom AS (
            SELECT
                ST_AsMVTGeom(
                    ST_Transform(abp.geom, 3857),
                    bounds.geom
                ) AS geom,
                abp.tags,
                abp.score,
                abp.kind,
                abp.snow
            FROM
                all_bike_paths abp, bounds
            WHERE
                    abp.kind IS NOT NULL AND
                    ST_Intersects(abp.geom, ST_Transform(bounds.geom, 3857)) AND
                    NOT (
                        abp.snow AND (
                            COALESCE(abp.tags->>'winter_service', 'yes') = 'no'
                            OR (
                                (
                                    (abp.tags->>'cycleway:conditional') IS NOT NULL
                                    AND (abp.tags->>'cycleway:conditional') LIKE '%@snow%'
                                    AND (abp.tags->>'cycleway:conditional') LIKE '%no%'
                                )
                                OR (
                                    (abp.tags->>'cycleway:left:conditional') IS NOT NULL
                                    AND (abp.tags->>'cycleway:left:conditional') LIKE '%@snow%'
                                    AND (abp.tags->>'cycleway:left:conditional') LIKE '%no%'
                                )
                                OR (
                                    (abp.tags->>'cycleway:right:conditional') IS NOT NULL
                                    AND (abp.tags->>'cycleway:right:conditional') LIKE '%@snow%'
                                    AND (abp.tags->>'cycleway:right:conditional') LIKE '%no%'
                                )
                            )
                        )
                    )
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

pub async fn bike_path() -> Json<JsonValue> {
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
