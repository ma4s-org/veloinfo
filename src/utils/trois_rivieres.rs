use chrono::{NaiveDate, NaiveDateTime};
use serde_json::{json, Value};
use sqlx::postgres::Postgres;
use std::time::Duration;

use super::road_work_import::{
    local_today, replace_polygon_road_works, to_time_date,
};

// Jeu de données « Avis de grands travaux » de la Ville de Trois-Rivières,
// publié sur Données Québec (CC-BY 4.0).
const GEOJSON_URL: &str = "https://www.donneesquebec.ca/recherche/dataset/bf40546c-0f04-45be-b039-c6eaa51d3ab5/resource/7e3ffb7a-93bb-44d1-80f2-b74b9d051b3a/download/avis-grands-travaux-v3r.json";

const SOURCE: &str = "trois-rivieres";

fn parse_date(value: &Value) -> Option<NaiveDate> {
    let text = value.as_str()?;
    NaiveDateTime::parse_from_str(text, "%Y%m%d%H%M%S")
        .ok()
        .map(|datetime| datetime.date())
}

fn is_polygon_geometry(geometry: &Value) -> bool {
    matches!(
        geometry.get("type").and_then(Value::as_str),
        Some("Polygon" | "MultiPolygon")
    )
}

/// Télécharge les grands travaux en cours de Trois-Rivières, les découpe sur
/// le réseau routier OSM et remplace les travaux précédemment importés.
pub async fn fetch_trois_rivieres_data(conn: &sqlx::Pool<Postgres>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            println!("Error building trois-rivieres http client: {}", e);
            return;
        }
    };

    let response = match client.get(GEOJSON_URL).send().await {
        Ok(response) => response,
        Err(e) => {
            println!("Error fetching trois-rivieres: {}", e);
            return;
        }
    };

    if !response.status().is_success() {
        println!(
            "Request to trois-rivieres failed with status {}",
            response.status()
        );
        return;
    }

    let body: Value = match response.json().await {
        Ok(body) => body,
        Err(e) => {
            println!("Error parsing trois-rivieres json: {}", e);
            return;
        }
    };

    let raw_features = match body.get("features").and_then(Value::as_array) {
        Some(features) => features,
        None => {
            println!("Unexpected trois-rivieres json: expected a FeatureCollection");
            return;
        }
    };

    let today = local_today();
    let today_sql = match to_time_date(today) {
        Some(date) => date,
        None => {
            println!("Error converting today's date for trois-rivieres");
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
            properties.get("DATE_DEBUT").and_then(parse_date),
            properties.get("DATE_FIN").and_then(parse_date),
        ) else {
            skipped += 1;
            continue;
        };

        if start > today || end < today {
            skipped += 1;
            continue;
        }

        let geometry = match feature.get("geometry").cloned() {
            Some(geometry) if is_polygon_geometry(&geometry) => geometry,
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
        println!("No current road works from trois-rivieres; keeping previous import");
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
            "Done fetching trois-rivieres data ({} current zones, {} skipped)",
            works_count, skipped
        ),
        None => println!("Error replacing trois-rivieres road works"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_une_date_arcgis() {
        assert_eq!(
            parse_date(&json!("20260914040000")),
            NaiveDate::from_ymd_opt(2026, 9, 14)
        );
    }

    #[test]
    fn accepte_les_polygones() {
        assert!(is_polygon_geometry(&json!({
            "type": "Polygon",
            "coordinates": [[[-72.56, 46.36], [-72.57, 46.37], [-72.56, 46.36]]]
        })));
    }
}
