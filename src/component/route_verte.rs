use std::env;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, Utc};
use geojson::JsonValue;
use sqlx::Row;

use crate::VeloinfoState;

/// Classifie un way de la Route Verte selon la présence d'infrastructure cyclable.
/// Infra = cycleway / cycleway_crossing / designated / shared_lane (même logique que bike_path.rs).
/// Sans infra = way de la Route Verte sans aucun marquage cyclable dédié.
#[axum::debug_handler]
pub async fn route_verte_mvt(
    State(state): State<VeloinfoState>,
    Path((z, x, y)): Path<(u32, u32, u32)>,
) -> impl IntoResponse {
    let conn = &state.conn;

    let query = r#"
        WITH
        bounds AS (
            SELECT ST_TileEnvelope($1, $2, $3) AS geom
        ),
        mvtgeom AS (
            SELECT
                ST_AsMVTGeom(
                    aw.geom,
                    b.geom
                ) AS geom,
                aw.tags->>'name' AS name,
                CASE
                    -- Infrastructure cyclable dédiée
                    WHEN
                        aw.tags->>'highway' = 'cycleway' OR
                        aw.tags->>'cyclestreet' = 'yes' OR
                        aw.tags->>'cycleway' = 'track' OR
                        aw.tags->>'cycleway:left' = 'track' OR
                        aw.tags->>'cycleway:right' = 'track' OR
                        aw.tags->>'cycleway:both' = 'track'
                    THEN 'with_infra'
                    -- Voies désignées (bandes cyclables, partage bus)
                    WHEN
                        aw.tags->>'cycleway:left' = 'share_busway' OR
                        aw.tags->>'cycleway:right' = 'share_busway' OR
                        aw.tags->>'cycleway:both' = 'share_busway' OR
                        aw.tags->>'cycleway:right' = 'lane' OR
                        aw.tags->>'cycleway:left' = 'lane' OR
                        aw.tags->>'cycleway:both' = 'lane' OR
                        aw.tags->>'cycleway' = 'lane'
                    THEN 'with_infra'
                    -- Voies partagées (shared_lane, opposite_lane, footway bicycle=yes)
                    WHEN
                        aw.tags->>'cycleway' = 'shared_lane' OR
                        aw.tags->>'cycleway:left' = 'shared_lane' OR
                        aw.tags->>'cycleway:left' = 'opposite_lane' OR
                        aw.tags->>'cycleway:right' = 'shared_lane' OR
                        aw.tags->>'cycleway:right' = 'opposite_lane' OR
                        aw.tags->>'cycleway:both' = 'shared_lane' OR
                        (aw.tags->>'highway' = 'footway' AND aw.tags->>'bicycle' = 'yes')
                    THEN 'with_infra'
                    -- Ferry de la Route Verte : considéré comme avec infrastructure
                    WHEN aw.tags->>'route' = 'ferry' THEN 'with_infra'
                    -- highway=service : voie de service à faible circulation, considérée sécuritaire
                    WHEN aw.tags->>'highway' = 'service' THEN 'with_infra'
                    -- Aucune infrastructure cyclable marquée
                    ELSE 'without_infra'
                END AS kind
            FROM
                all_way aw, bounds b
            WHERE
                aw.in_route_verte = true
                AND COALESCE(aw.tags->>'bicycle', 'yes') <> 'no'
                AND aw.geom && b.geom
        )
        SELECT ST_AsMVT(mvtgeom.*, 'route_verte', 4096, 'geom')
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
            match mvt {
                Some(tile) if !tile.is_empty() => {
                    let expires = Utc::now() + Duration::days(1);
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, "application/vnd.mapbox-vector-tile")
                        .header(
                            header::CACHE_CONTROL,
                            "public, max-age=86400, stale-while-revalidate=15768000",
                        )
                        .header(
                            header::EXPIRES,
                            expires.format("%a, %d %b %Y %H:%M:%S GMT").to_string(),
                        )
                        .body(Body::from(tile))
                        .unwrap()
                        .into_response()
                }
                _ => Response::builder()
                    .status(StatusCode::NO_CONTENT)
                    .body(Body::empty())
                    .unwrap()
                    .into_response(),
            }
        }
        Ok(None) => Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .unwrap()
            .into_response(),
        Err(e) => {
            eprintln!("SQL error (route_verte): {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "SQL error").into_response()
        }
    }
}

pub async fn route_verte() -> Json<JsonValue> {
    let tilejson = serde_json::json!({
        "tilejson": "3.0.0",
        "name": "route_verte",
        "tiles": [
            format!("{}/route_verte/{{z}}/{{x}}/{{y}}", env::var("VELOINFO_URL").unwrap())
        ],
        "vector_layers": [
            {
                "id": "route_verte",
                "fields": {
                    "name": "String",
                    "kind": "String"
                },
                "minzoom": 0,
                "maxzoom": 22
            }
        ]
    });
    Json(tilejson)
}