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

/// Résumé du kilométrage des routes vertes : total du réseau et portion visible
/// dans l'emprise passée en paramètre, ventilé by `kind` (with_infra / without_infra).
#[axum::debug_handler]
pub async fn route_verte_stats(
    Path((min_lng, min_lat, max_lng, max_lat)): Path<(f64, f64, f64, f64)>,
    State(state): State<VeloinfoState>,
) -> impl IntoResponse {
    let conn = &state.conn;

    // Requête unique : calcule en parallèle le kilométrage total (hors emprise)
    // et le kilométrage visible (intersecté avec l'emprise).
    // On réutilise le même bloc CASE que route_verte_mvt pour rester cohérent
    // avec les couches affichées sur la carte.
    // NB: aw.geom est stocké en SRID 3857 (Web Mercator, voir import.sh),
    // donc on transforme l'emprise 4326 reçue du client vers 3857, et on
    // passe en 4326::geography pour ST_Length (qui attend du 4326).
    let query = r#"
        WITH
        bounds AS (
            SELECT ST_Transform(ST_MakeEnvelope($1, $2, $3, $4, 4326), 3857) AS geom
        ),
        classified AS (
            SELECT
                CASE
                    WHEN
                        aw.tags->>'highway' = 'cycleway' OR
                        aw.tags->>'cyclestreet' = 'yes' OR
                        aw.tags->>'cycleway' = 'track' OR
                        aw.tags->>'cycleway:left' = 'track' OR
                        aw.tags->>'cycleway:right' = 'track' OR
                        aw.tags->>'cycleway:both' = 'track'
                    THEN 'with_infra'
                    WHEN
                        aw.tags->>'cycleway:left' = 'share_busway' OR
                        aw.tags->>'cycleway:right' = 'share_busway' OR
                        aw.tags->>'cycleway:both' = 'share_busway' OR
                        aw.tags->>'cycleway:right' = 'lane' OR
                        aw.tags->>'cycleway:left' = 'lane' OR
                        aw.tags->>'cycleway:both' = 'lane' OR
                        aw.tags->>'cycleway' = 'lane'
                    THEN 'with_infra'
                    WHEN
                        aw.tags->>'cycleway' = 'shared_lane' OR
                        aw.tags->>'cycleway:left' = 'shared_lane' OR
                        aw.tags->>'cycleway:left' = 'opposite_lane' OR
                        aw.tags->>'cycleway:right' = 'shared_lane' OR
                        aw.tags->>'cycleway:right' = 'opposite_lane' OR
                        aw.tags->>'cycleway:both' = 'shared_lane' OR
                        (aw.tags->>'highway' = 'footway' AND aw.tags->>'bicycle' = 'yes')
                    THEN 'with_infra'
                    WHEN aw.tags->>'route' = 'ferry' THEN 'with_infra'
                    WHEN aw.tags->>'highway' = 'service' THEN 'with_infra'
                    ELSE 'without_infra'
                END AS kind,
                aw.geom AS geom
            FROM all_way aw
            WHERE
                aw.in_route_verte = true
                AND COALESCE(aw.tags->>'bicycle', 'yes') <> 'no'
        ),
        total AS (
            SELECT
                kind,
                SUM(ST_Length(ST_Transform(geom, 4326)::geography)) AS length
            FROM classified
            GROUP BY kind
        ),
        visible AS (
            SELECT
                c.kind,
                SUM(ST_Length(ST_Transform(ST_Intersection(c.geom, b.geom), 4326)::geography)) AS length
            FROM classified c, bounds b
            WHERE c.geom && b.geom
            GROUP BY c.kind
        )
        SELECT
            COALESCE((SELECT length FROM total WHERE kind = 'with_infra'), 0),
            COALESCE((SELECT length FROM total WHERE kind = 'without_infra'), 0),
            COALESCE((SELECT length FROM visible WHERE kind = 'with_infra'), 0),
            COALESCE((SELECT length FROM visible WHERE kind = 'without_infra'), 0)
        "#;

    let result = sqlx::query(query)
        .bind(min_lng)
        .bind(min_lat)
        .bind(max_lng)
        .bind(max_lat)
        .fetch_one(conn)
        .await;

    match result {
        Ok(row) => {
            let total_with_infra: f64 = row.try_get(0).unwrap_or(0.0);
            let total_without_infra: f64 = row.try_get(1).unwrap_or(0.0);
            let visible_with_infra: f64 = row.try_get(2).unwrap_or(0.0);
            let visible_without_infra: f64 = row.try_get(3).unwrap_or(0.0);

            // Convertir les mètres en kilomètres (1 décimale)
            let stats = serde_json::json!({
                "total": {
                    "with_infra": (total_with_infra / 1000.0 * 10.0).round() / 10.0,
                    "without_infra": (total_without_infra / 1000.0 * 10.0).round() / 10.0
                },
                "visible": {
                    "with_infra": (visible_with_infra / 1000.0 * 10.0).round() / 10.0,
                    "without_infra": (visible_without_infra / 1000.0 * 10.0).round() / 10.0
                }
            });
            Json(stats).into_response()
        }
        Err(e) => {
            eprintln!("SQL error (route_verte_stats): {}", e);
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