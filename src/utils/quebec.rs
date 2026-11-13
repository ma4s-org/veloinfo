use chrono::DateTime;
use serde_json::Value;
use sqlx::postgres::Postgres;
use std::time::Duration;
use time::OffsetDateTime;

use super::road_work_import::replace_road_works;

// Flux WZDX officiel de la Ville de Québec, publié sur Données Québec :
// https://www.donneesquebec.ca/recherche/dataset/entraves-a-la-circulation-en-temps-reel-de-la-ville-de-quebec
//
// Le flux contient les entraves (event_type = "work-zone") et les détours
// recommandés (event_type = "detour"). On n'importe que les entraves : les
// détours ne sont pas des travaux et n'ont pas leur place dans road_work.
const ROAD_WORK_URL: &str = "https://quebec.gewi.com/wzdx4waze/pull";
const SOURCE: &str = "quebec";

/// Convertit une date ISO 8601 du flux (UTC) en date locale de Montréal.
///
/// Le flux encode par exemple une fin de travaux le 11 décembre à 23 h 59 par
/// `2026-12-12T04:59:00Z` : sans conversion, on obtiendrait le 12 décembre.
fn local_date(properties: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    let raw = properties.get(key)?.as_str()?;
    let parsed = DateTime::parse_from_rfc3339(raw).ok()?;
    Some(
        parsed
            .with_timezone(&chrono_tz::America::Montreal)
            .format("%Y-%m-%d")
            .to_string(),
    )
}

/// Télécharge les entraves en cours de la Ville de Québec, les recale sur le
/// réseau routier OSM et remplace les travaux précédemment importés pour cette
/// source.
pub async fn fetch_quebec_data(conn: &sqlx::Pool<Postgres>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            println!("Error building quebec http client: {}", e);
            return;
        }
    };

    let response = match client.get(ROAD_WORK_URL).send().await {
        Ok(response) => response,
        Err(e) => {
            println!("Error fetching {}: {}", ROAD_WORK_URL, e);
            return;
        }
    };

    if !response.status().is_success() {
        println!(
            "Request to {} failed with status {}",
            ROAD_WORK_URL,
            response.status()
        );
        return;
    }

    let body = match response.text().await {
        Ok(body) => body,
        Err(e) => {
            println!("Error reading response from {}: {}", ROAD_WORK_URL, e);
            return;
        }
    };

    // On travaille directement sur le JSON : certaines entrées du flux n'ont
    // pas de clé `geometry`, ce que le désérialiseur GeoJSON strict refuse.
    let mut root: Value = match serde_json::from_str(&body) {
        Ok(root) => root,
        Err(e) => {
            println!("Error parsing json from {}: {}", ROAD_WORK_URL, e);
            return;
        }
    };

    let features = match root.get_mut("features").and_then(Value::as_array_mut) {
        Some(features) => features,
        None => {
            println!(
                "Unexpected json from {}: expected a FeatureCollection",
                ROAD_WORK_URL
            );
            return;
        }
    };

    // On ne garde que les travaux en cours (`event_status = "active"`). Les
    // travaux à venir (`planned`) et en attente (`pending`) sont ignorés.
    features.retain(|feature| {
        feature
            .get("geometry")
            .map(|geometry| !geometry.is_null())
            .unwrap_or(false)
            && feature
                .pointer("/properties/event_type")
                .and_then(Value::as_str)
                == Some("work-zone")
            && feature
                .pointer("/properties/event_status")
                .and_then(Value::as_str)
                == Some("active")
    });

    for feature in features.iter_mut() {
        if let Some(properties) = feature.get_mut("properties").and_then(Value::as_object_mut) {
            if let Some(start_date) = local_date(properties, "start_date") {
                properties.insert("veloinfo_start_date".to_string(), Value::String(start_date));
            }
            if let Some(end_date) = local_date(properties, "end_date") {
                properties.insert("veloinfo_end_date".to_string(), Value::String(end_date));
            }
        }
    }

    let works_count = features.len();
    let works_json = match serde_json::to_string(&root) {
        Ok(works_json) => works_json,
        Err(e) => {
            println!("Error serializing quebec road works: {}", e);
            return;
        }
    };

    // Les dates sont lues dans les propriétés ; cette valeur ne sert que de
    // repli si une entrave n'a pas de date.
    let today = OffsetDateTime::now_utc().date();

    match replace_road_works(conn, SOURCE, &works_json, today).await {
        Some(inserted) => println!(
            "Done fetching quebec data ({} road works from {} source features)",
            inserted, works_count
        ),
        None => println!("Error replacing quebec road works"),
    }
}
