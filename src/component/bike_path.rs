use std::env;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use geojson::JsonValue;
use sqlx::Row;

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
        filtered_ways AS (
            SELECT aw.way_id, aw.geom, aw.tags
            FROM all_way aw, bounds b
            WHERE COALESCE(aw.tags->>'bicycle', 'yes') <> 'no'
              AND aw.geom && b.geom
        ),
        all_bike_paths AS (
            SELECT
                fw.geom,
                fw.tags,
                COALESCE(cscore.score, 1) as score,
                cs.city_name IS NOT NULL as snow,
                CASE
                    -- 1. Ferry
                    WHEN fw.tags->>'route' = 'ferry' THEN 'ferry'
                    
                    -- 2. Pistes dédiées (tracks, etc.)
                    WHEN 
                        fw.tags->>'highway' = 'cycleway' OR
                        fw.tags->>'cyclestreet' = 'yes' OR
                        fw.tags->>'cycleway' = 'track' OR
                        fw.tags->>'cycleway:left' = 'track' OR
                        fw.tags->>'cycleway:right' = 'track' OR
                        fw.tags->>'cycleway:both' = 'track'
                    THEN CASE WHEN fw.tags->>'cycleway' = 'crossing' THEN 'cycleway_crossing' ELSE 'cycleway' END
                    
                    -- 3. Voies désignées (partagées avec bus ou marquées sur le côté)
                    WHEN 
                        fw.tags->>'cycleway:left' = 'share_busway' OR
                        fw.tags->>'cycleway:right' = 'share_busway' OR
                        fw.tags->>'cycleway:both' = 'share_busway' OR
                        fw.tags->>'cycleway:right' = 'lane' OR
                        fw.tags->>'cycleway:left' = 'lane' OR
                        fw.tags->>'cycleway:both' = 'lane' OR
                        fw.tags->>'cycleway' = 'lane'
                    THEN 'designated'
                    
                    -- 4. Voies partagées (cas plus généraux)
                    WHEN 
                        fw.tags->>'cycleway' = 'shared_lane' OR
                        fw.tags->>'cycleway:left' = 'shared_lane' OR
                        fw.tags->>'cycleway:left' = 'opposite_lane' OR
                        fw.tags->>'cycleway:right' = 'shared_lane' OR
                        fw.tags->>'cycleway:right' = 'opposite_lane' OR
                        fw.tags->>'cycleway:both' = 'shared_lane' OR
                        (fw.tags->>'highway' = 'footway' AND fw.tags->>'bicycle' = 'yes')
                    THEN  'shared_lane'
                END as kind
            FROM
                filtered_ways fw
            LEFT JOIN LATERAL (
                SELECT way_ids, score
                FROM cyclability_score
                WHERE fw.way_id = ANY(cyclability_score.way_ids)
                ORDER BY cyclability_score.created_at DESC
                LIMIT 1
            ) cscore ON true
            LEFT JOIN city ON ST_Intersects(fw.geom, city.geom)
            LEFT JOIN city_snow cs ON city.name = cs.city_name
        ),
        mvtgeom AS (
        SELECT
            ST_AsMVTGeom(
                abp.geom,
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
            -- On n'exclut que si c'est explicitement interdit en hiver
            NOT (
                abp.snow = true AND (
                    COALESCE(abp.tags->>'winter_service', 'yes') = 'no'
                    OR (LOWER(abp.tags->>'cycleway:conditional') LIKE '%snow%' AND LOWER(abp.tags->>'cycleway:conditional') LIKE '%no%') IS TRUE
                    OR (LOWER(abp.tags->>'cycleway:left:conditional') LIKE '%snow%' AND LOWER(abp.tags->>'cycleway:left:conditional') LIKE '%no%') IS TRUE
                    OR (LOWER(abp.tags->>'cycleway:right:conditional') LIKE '%snow%' AND LOWER(abp.tags->>'cycleway:right:conditional') LIKE '%no%') IS TRUE
                )
            )
        )
        SELECT ST_AsMVT(mvtgeom.*, 'bike_path', 4096, 'geom')
        FROM mvtgeom;
    "#;

    let result = sqlx::query(query)
        .bind(z as i32)
        .bind(x as i32)
        .bind(y as i32)
        .fetch_optional(conn)
        .await;
    match result {
        Ok(Some(row)) => {
            let mvt: Option<Vec<u8>> = row.try_get(0).ok();
            eprintln!("MVT size: {:?}", mvt.as_ref().map(|v| v.len()));
            match mvt {
                Some(tile) if !tile.is_empty() => Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "application/vnd.mapbox-vector-tile")
                    .body(Body::from(tile))
                    .unwrap()
                    .into_response(),
                _ => {
                    eprintln!("Empty tile returned");
                    Response::builder()
                        .status(StatusCode::NO_CONTENT)
                        .body(Body::empty())
                        .unwrap()
                        .into_response()
                }
            }
        }
        Ok(None) => {
            eprintln!("No row returned");
            Response::builder()
                .status(StatusCode::NO_CONTENT)
                .body(Body::empty())
                .unwrap()
                .into_response()
        }
        Err(e) => {
            eprintln!("SQL error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "SQL error").into_response()
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
