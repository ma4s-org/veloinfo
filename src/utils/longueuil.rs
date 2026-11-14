use serde_json::{json, Value};
use sqlx::postgres::Postgres;
use std::time::Duration;

use super::road_work_import::{
    local_today, millis_to_date, replace_polygon_road_works, to_time_date,
};

// Couche « Gestion des entraves - Surfaces » du GéoHub de Longueuil.
const QUERY_URL: &str = "https://geomatique.longueuil.quebec/public/rest/services/Communication/Gestion_des_entraves_Diffusion/FeatureServer/1/query";

const SOURCE: &str = "longueuil";

/// Télécharge les zones d'entraves en cours de Longueuil, les découpe sur le
/// réseau routier OSM et remplace les travaux précédemment importés.
pub async fn fetch_longueuil_data(conn: &sqlx::Pool<Postgres>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            println!("Error building longueuil http client: {}", e);
            return;
        }
    };

    let response = match client
        .get(QUERY_URL)
        .query(&[
            ("where", "DATE_FIN >= CURRENT_TIMESTAMP"),
            ("outFields", "*"),
            ("returnGeometry", "true"),
            ("f", "geojson"),
        ])
        .send()
        .await
    {
        Ok(response) => response,
        Err(e) => {
            println!("Error fetching longueuil: {}", e);
            return;
        }
    };

    if !response.status().is_success() {
        println!(
            "Request to longueuil failed with status {}",
            response.status()
        );
        return;
    }

    let body: Value = match response.json().await {
        Ok(body) => body,
        Err(e) => {
            println!("Error parsing longueuil json: {}", e);
            return;
        }
    };

    let raw_features = match body.get("features").and_then(Value::as_array) {
        Some(features) => features,
        None => {
            println!("Unexpected longueuil json: expected a FeatureCollection");
            return;
        }
    };

    let today = local_today();
    let today_sql = match to_time_date(today) {
        Some(date) => date,
        None => {
            println!("Error converting today's date for longueuil");
            return;
        }
    };

    let mut features = Vec::new();
    let mut skipped = 0usize;

    for feature in raw_features {
        let properties = match feature.get("properties") {
            Some(properties) => properties,
            None => continue,
        };

        let (Some(start), Some(end)) = (
            properties.get("DATE_DEBUT").and_then(millis_to_date),
            properties.get("DATE_FIN").and_then(millis_to_date),
        ) else {
            skipped += 1;
            continue;
        };

        if start > today || end < today {
            skipped += 1;
            continue;
        }

        let geometry = match feature.get("geometry").cloned() {
            Some(geometry)
                if matches!(
                    geometry.get("type").and_then(Value::as_str),
                    Some("Polygon" | "MultiPolygon")
                ) =>
            {
                geometry
            }
            _ => {
                skipped += 1;
                continue;
            }
        };

        features.push(json!({
            "type": "Feature",
            "geometry": geometry,
            "properties": {
                "veloinfo_start_date": start.format("%Y-%m-%d").to_string(),
                "veloinfo_end_date": end.format("%Y-%m-%d").to_string(),
            },
        }));
    }

    if features.is_empty() {
        println!("No current road works from longueuil; keeping previous import");
        super::import_status::mark_success(SOURCE);
        return;
    }

    let works_count = features.len();
    let works_json = json!({
        "type": "FeatureCollection",
        "features": features,
    })
    .to_string();

    match replace_polygon_road_works(conn, SOURCE, &works_json, today_sql).await {
        Some(_) => println!(
            "Done fetching longueuil data ({} current zones, {} skipped)",
            works_count, skipped
        ),
        None => println!("Error replacing longueuil road works"),
    }
}
