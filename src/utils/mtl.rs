use chrono::{DateTime, NaiveDate};
use chrono_tz::America::Montreal;
use serde_json::{json, Value};
use sqlx::postgres::Postgres;
use std::time::Duration;

use super::road_work_import::{
    local_today, replace_polygon_road_works, to_time_date,
};

// Carte des entraves et travaux de la Ville de Montréal :
// https://services.montreal.ca/cartes/entraves
//
// Le WFS public expose la couche `entraves-polygonales` en GeoJSON. Une seule
// requête remplace l'ancien parcours de 27 000 tuiles vectorielles à z17.
const WFS_URL: &str = "https://api.montreal.ca/api/it-platforms/geomatic/wfs-maps/montreal/ows?service=WFS&version=1.0.0&request=GetFeature&typeName=montreal:entraves-polygonales&outputFormat=application/json";

const SOURCE: &str = "montreal";

fn montreal_date(value: &Value) -> Option<NaiveDate> {
    let value = value.as_str()?;
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|datetime| datetime.with_timezone(&Montreal).date_naive())
}

/// Télécharge les zones de travaux en cours de la Ville de Montréal, les
/// découpe sur le réseau routier OSM et remplace les travaux précédemment
/// importés pour cette source.
pub async fn fetch_montreal_data(conn: &sqlx::Pool<Postgres>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            println!("Error building montreal http client: {}", e);
            return;
        }
    };

    let response = match client.get(WFS_URL).send().await {
        Ok(response) => response,
        Err(e) => {
            println!("Error fetching montreal WFS: {}", e);
            return;
        }
    };

    if !response.status().is_success() {
        println!(
            "Request to montreal WFS failed with status {}",
            response.status()
        );
        return;
    }

    let body: Value = match response.json().await {
        Ok(body) => body,
        Err(e) => {
            println!("Error parsing json from montreal WFS: {}", e);
            return;
        }
    };

    let raw_features = match body.get("features").and_then(Value::as_array) {
        Some(features) => features,
        None => {
            println!("Unexpected json from montreal WFS: expected a FeatureCollection");
            return;
        }
    };

    let today = local_today();
    let today_sql = match to_time_date(today) {
        Some(date) => date,
        None => {
            println!("Error converting today's date for montreal");
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
            properties.get("durationStartDate").and_then(montreal_date),
            properties.get("durationEndDate").and_then(montreal_date),
        ) else {
            skipped += 1;
            continue;
        };

        if start > today || end < today {
            skipped += 1;
            continue;
        }

        let geometry = match feature.get("geometry").cloned() {
            Some(geometry) if !geometry.is_null() => geometry,
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
        println!("No current road works from montreal; keeping previous import");
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
            "Done fetching montreal data ({} current zones, {} skipped)",
            works_count, skipped
        ),
        None => println!("Error replacing montreal road works"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convertit_une_date_wfs_en_date_locale() {
        assert_eq!(
            montreal_date(&json!("2026-04-24T04:00:00Z")),
            NaiveDate::from_ymd_opt(2026, 4, 24)
        );
    }

    #[test]
    fn ignore_une_date_invalide() {
        assert_eq!(montreal_date(&json!("pas une date")), None);
        assert_eq!(montreal_date(&json!(null)), None);
    }
}
