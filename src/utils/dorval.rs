use serde_json::{json, Value};
use sqlx::postgres::Postgres;
use std::time::Duration;

use super::road_work_import::{
    local_today, millis_to_date, replace_raw_road_works, to_time_date,
};

// Carte Info-travaux de la Cité de Dorval :
// https://www.ville.dorval.qc.ca/fr/environnement-et-voirie/infrastructures-urbaines/info-travaux
//
// La carte est une application ArcGIS Experience qui utilise une couche
// ponctuelle publique. Les entraves sont des points : elles sont insérées
// telles quelles dans road_work.
const QUERY_URL: &str = "https://services2.arcgis.com/UfBk83iw7IIXzPRW/arcgis/rest/services/Entraves2410_Vue/FeatureServer/34/query";

const SOURCE: &str = "dorval";

/// Télécharge les entraves actives de Dorval et remplace les travaux
/// précédemment importés pour cette source.
pub async fn fetch_dorval_data(conn: &sqlx::Pool<Postgres>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            println!("Error building dorval http client: {}", e);
            return;
        }
    };

    let response = match client
        .get(QUERY_URL)
        .query(&[
            ("where", "StatusEntr='Actif'"),
            ("outFields", "*"),
            ("returnGeometry", "true"),
            ("f", "geojson"),
        ])
        .send()
        .await
    {
        Ok(response) => response,
        Err(e) => {
            println!("Error fetching {}: {}", QUERY_URL, e);
            return;
        }
    };

    if !response.status().is_success() {
        println!(
            "Request to {} failed with status {}",
            QUERY_URL,
            response.status()
        );
        return;
    }

    let body: Value = match response.json().await {
        Ok(body) => body,
        Err(e) => {
            println!("Error parsing json from {}: {}", QUERY_URL, e);
            return;
        }
    };

    let raw_features = match body.get("features").and_then(Value::as_array) {
        Some(features) => features,
        None => {
            println!("Unexpected json from {}: expected a FeatureCollection", QUERY_URL);
            return;
        }
    };

    let today = local_today();
    let today_sql = match to_time_date(today) {
        Some(date) => date,
        None => {
            println!("Error converting today's date for dorval");
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

        let (start, end) = match (
            properties.get("DateDebut").and_then(millis_to_date),
            properties.get("DateFin").and_then(millis_to_date),
        ) {
            (Some(start), Some(end)) if start <= today && today <= end => (start, end),
            (None, None) => (today, today),
            _ => {
                skipped += 1;
                continue;
            }
        };

        let geometry = match feature.get("geometry").cloned() {
            Some(geometry)
                if geometry.get("type").and_then(Value::as_str) == Some("Point") =>
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
        println!("No current road works from dorval; keeping previous import");
        super::import_status::mark_success(SOURCE);
        return;
    }

    let works_count = features.len();
    let works_json = json!({
        "type": "FeatureCollection",
        "features": features,
    })
    .to_string();

    match replace_raw_road_works(conn, SOURCE, &works_json, today_sql).await {
        Some(inserted) => println!(
            "Done fetching dorval data ({} road works from {} active features, {} skipped)",
            inserted, works_count, skipped
        ),
        None => println!("Error replacing dorval road works"),
    }
}
