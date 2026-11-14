use chrono::NaiveDate;
use serde_json::{json, Value};
use sqlx::postgres::Postgres;
use std::time::Duration;

use super::road_work_import::{
    local_today, millis_to_date, replace_road_works, to_time_date,
};

// Carte Info-Travaux de Hampstead :
// https://experience.arcgis.com/experience/e484763e01614cb4a68ae7d7a9334bb5/
//
// La couche publique « Réseau routier - Info-travaux » contient les tronçons
// de rue touchés par des travaux. La colonne `Travaux` vaut 0 quand il n'y a
// rien, 1 ou 3 pour une entrave en cours.
const QUERY_URL: &str = "https://services1.arcgis.com/IP2j0oTRjMlb9KsM/arcgis/rest/services/Reseau_Infotravaux_2023_Hampstead_L/FeatureServer/2/query";

const SOURCE: &str = "hampstead";

fn parse_date(value: Option<&Value>) -> Option<NaiveDate> {
    let value = value?;

    if let Some(date) = millis_to_date(value) {
        return Some(date);
    }

    value
        .as_str()
        .and_then(|text| NaiveDate::parse_from_str(text, "%Y-%m-%d").ok())
}

fn is_line_geometry(geometry: &Value) -> bool {
    matches!(
        geometry.get("type").and_then(Value::as_str),
        Some("LineString" | "MultiLineString")
    )
}

/// Télécharge les travaux en cours de Hampstead, les recale sur le réseau
/// routier OSM et remplace les travaux précédemment importés pour cette source.
pub async fn fetch_hampstead_data(conn: &sqlx::Pool<Postgres>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            println!("Error building hampstead http client: {}", e);
            return;
        }
    };

    let response = match client
        .get(QUERY_URL)
        .query(&[
            ("where", "Travaux <> 0"),
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
            println!("Error converting today's date for hampstead");
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

        // Certaines entraves n'ont pas de date de fin (travaux en cours sans
        // échéance). On les considère actives jusqu'à aujourd'hui.
        let start = parse_date(properties.get("Date_debut")).unwrap_or(today);
        let end = parse_date(properties.get("Date_fin")).unwrap_or(today);
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
        println!("No current road works from hampstead; keeping previous import");
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
            "Done fetching hampstead data ({} road works from {} current features, {} skipped)",
            inserted, works_count, skipped
        ),
        None => println!("Error replacing hampstead road works"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepte_les_lignes() {
        assert!(is_line_geometry(&json!({
            "type": "LineString",
            "coordinates": [[-73.65, 45.48], [-73.66, 45.49]]
        })));
    }

    #[test]
    fn refuse_les_points() {
        assert!(!is_line_geometry(&json!({
            "type": "Point",
            "coordinates": [-73.65, 45.48]
        })));
    }
}
