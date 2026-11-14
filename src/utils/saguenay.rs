use chrono::{DateTime, NaiveDate, NaiveDateTime};
use serde_json::{json, Value};
use sqlx::postgres::Postgres;
use std::time::Duration;

use super::road_work_import::{local_today, replace_road_works, to_time_date};

// Jeu de données « Chantiers - 511 » de la Ville de Saguenay, publié sur
// Données Québec (CC-BY 4.0).
const GEOJSON_URL: &str = "https://www.donneesquebec.ca/recherche/dataset/221d146e-f285-4eb4-9f47-09b4b36cc429/resource/6df36e69-fdfd-43ea-bcea-ec41525b6c86/download/sag_chantiers511.geojson";

const SOURCE: &str = "saguenay";

fn parse_date(value: &Value) -> Option<NaiveDate> {
    let text = value.as_str()?;

    if let Ok(datetime) = DateTime::parse_from_rfc3339(text) {
        return Some(datetime.date_naive());
    }

    NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S")
        .ok()
        .map(|datetime| datetime.date())
}

fn is_line_geometry(geometry: &Value) -> bool {
    matches!(
        geometry.get("type").and_then(Value::as_str),
        Some("LineString" | "MultiLineString")
    )
}

/// Télécharge les chantiers en cours de Saguenay, les recale sur le réseau
/// routier OSM et remplace les travaux précédemment importés.
pub async fn fetch_saguenay_data(conn: &sqlx::Pool<Postgres>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            println!("Error building saguenay http client: {}", e);
            return;
        }
    };

    let response = match client.get(GEOJSON_URL).send().await {
        Ok(response) => response,
        Err(e) => {
            println!("Error fetching saguenay: {}", e);
            return;
        }
    };

    if !response.status().is_success() {
        println!(
            "Request to saguenay failed with status {}",
            response.status()
        );
        return;
    }

    let body: Value = match response.json().await {
        Ok(body) => body,
        Err(e) => {
            println!("Error parsing saguenay json: {}", e);
            return;
        }
    };

    let raw_features = match body.get("features").and_then(Value::as_array) {
        Some(features) => features,
        None => {
            println!("Unexpected saguenay json: expected a FeatureCollection");
            return;
        }
    };

    let today = local_today();
    let today_sql = match to_time_date(today) {
        Some(date) => date,
        None => {
            println!("Error converting today's date for saguenay");
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
            properties.get("date_debut").and_then(parse_date),
            properties.get("date_fin").and_then(parse_date),
        ) else {
            skipped += 1;
            continue;
        };

        if start > today || end < today {
            skipped += 1;
            continue;
        }

        let geometry = match feature.get("geometry").cloned() {
            Some(geometry) if is_line_geometry(&geometry) => geometry,
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
        println!("No current road works from saguenay; keeping previous import");
        super::import_status::mark_success(SOURCE);
        return;
    }

    let works_count = features.len();
    let works_json = json!({
        "type": "FeatureCollection",
        "features": features,
    })
    .to_string();

    match replace_road_works(conn, SOURCE, &works_json, today_sql).await {
        Some(inserted) => println!(
            "Done fetching saguenay data ({} road works from {} current features, {} skipped)",
            inserted, works_count, skipped
        ),
        None => println!("Error replacing saguenay road works"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepte_les_lignes() {
        assert!(is_line_geometry(&json!({
            "type": "LineString",
            "coordinates": [[-71.06, 48.43], [-71.07, 48.44]]
        })));
    }

    #[test]
    fn parse_une_date_iso() {
        assert_eq!(
            parse_date(&json!("2026-10-05T07:00:00")),
            NaiveDate::from_ymd_opt(2026, 10, 5)
        );
    }
}
