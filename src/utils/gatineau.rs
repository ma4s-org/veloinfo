use chrono::{Duration, NaiveDate};
use serde_json::{json, Value};
use sqlx::postgres::Postgres;
use std::f64::consts::PI;
use std::time::Duration as StdDuration;

use super::road_work_import::{
    local_today, millis_to_date, replace_road_works, to_time_date,
};

// Plateforme OPA de la Ville de Gatineau.
const PROJECTS_URL: &str = "https://gatineau.opatech.ca/public/get_projects";

const SOURCE: &str = "gatineau";

const EARTH_RADIUS: f64 = 6_378_137.0;

fn web_mercator_to_wgs84(x: f64, y: f64) -> [f64; 2] {
    let longitude = x / EARTH_RADIUS * 180.0 / PI;
    let latitude = (2.0 * (y / EARTH_RADIUS).exp().atan() - PI / 2.0) * 180.0 / PI;
    [longitude, latitude]
}

fn point_to_wgs84(point: &Value) -> Option<[f64; 2]> {
    let point = point.as_array()?;
    let x = point.first()?.as_f64()?;
    let y = point.get(1)?.as_f64()?;
    Some(web_mercator_to_wgs84(x, y))
}

fn add_selected_paths(selected: &Value, lines: &mut Vec<Vec<[f64; 2]>>) {
    let Some(paths) = selected.get("path").and_then(Value::as_array) else {
        return;
    };

    for path in paths {
        let Some(geometry_paths) = path
            .pointer("/geometry/paths")
            .and_then(Value::as_array)
        else {
            continue;
        };

        for geometry_path in geometry_paths {
            let coordinates: Option<Vec<[f64; 2]>> = geometry_path
                .as_array()
                .map(|points| points.iter().map(point_to_wgs84).collect())
                .unwrap_or_default();

            if let Some(coordinates) = coordinates {
                if coordinates.len() >= 2 {
                    lines.push(coordinates);
                }
            }
        }
    }
}

fn entrave_paths(entrave: &Value) -> Vec<Vec<[f64; 2]>> {
    let mut lines = Vec::new();

    match entrave.get("selected") {
        Some(Value::Array(items)) => {
            for item in items {
                add_selected_paths(item, &mut lines);
            }
        }
        Some(item) => add_selected_paths(item, &mut lines),
        None => {}
    }

    lines
}

fn lines_to_geometry(lines: Vec<Vec<[f64; 2]>>) -> Option<Value> {
    match lines.len() {
        0 => None,
        1 => Some(json!({
            "type": "LineString",
            "coordinates": lines[0],
        })),
        _ => Some(json!({
            "type": "MultiLineString",
            "coordinates": lines,
        })),
    }
}

fn date_range_millis(today: NaiveDate) -> (i64, i64) {
    let start = today
        .and_hms_opt(0, 0, 0)
        .expect("minuit valide")
        .and_utc()
        .timestamp_millis();
    let end = (today + Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .expect("minuit valide")
        .and_utc()
        .timestamp_millis();

    (start, end)
}

/// Télécharge les travaux en cours de Gatineau, les recale sur le réseau
/// routier OSM et remplace les travaux précédemment importés.
pub async fn fetch_gatineau_data(conn: &sqlx::Pool<Postgres>) {
    let client = match reqwest::Client::builder()
        .timeout(StdDuration::from_secs(60))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            println!("Error building gatineau http client: {}", e);
            return;
        }
    };

    let today = local_today();
    let today_sql = match to_time_date(today) {
        Some(date) => date,
        None => {
            println!("Error converting today's date for gatineau");
            return;
        }
    };

    let (start_ms, end_ms) = date_range_millis(today);

    let filters = json!({
        "city": "gatineau",
        "type": "osm",
        "modeMap": "0",
        "lang": "fr",
        "long": -75.7,
        "lat": 45.45,
        "publish": true,
        "nopublish": true,
        "ranges": [start_ms, end_ms],
        "tags": {},
        "step1": true,
        "step2": true,
        "emergencyProject": true,
        "auto": false,
        "pieton": false,
        "cycliste": false,
        "year": false,
    });

    let response = match client.post(PROJECTS_URL).json(&filters).send().await {
        Ok(response) => response,
        Err(e) => {
            println!("Error fetching gatineau: {}", e);
            return;
        }
    };

    if !response.status().is_success() {
        println!(
            "Request to gatineau failed with status {}",
            response.status()
        );
        return;
    }

    let body: Value = match response.json().await {
        Ok(body) => body,
        Err(e) => {
            println!("Error parsing gatineau json: {}", e);
            return;
        }
    };

    let projects = match body.get("data").and_then(Value::as_array) {
        Some(projects) => projects,
        None => {
            println!("Unexpected gatineau json: expected a data array");
            return;
        }
    };

    let mut features = Vec::new();
    let mut skipped = 0usize;

    for project in projects {
        let project_data = match project.get("data") {
            Some(data) => data,
            None => continue,
        };
        let entraves = match project_data.get("entraves") {
            Some(entraves) => entraves,
            None => {
                skipped += 1;
                continue;
            }
        };

        let (Some(start), Some(end)) = (
            entraves.get("date").and_then(Value::as_array).and_then(|range| {
                range.first().and_then(millis_to_date)
            }),
            entraves.get("date").and_then(Value::as_array).and_then(|range| {
                range.get(1).and_then(millis_to_date)
            }),
        ) else {
            skipped += 1;
            continue;
        };

        if start > today || end < today {
            skipped += 1;
            continue;
        }

        let raw_entraves = match entraves.get("entraves").and_then(Value::as_array) {
            Some(entraves) => entraves,
            None => {
                skipped += 1;
                continue;
            }
        };

        let mut lines = Vec::new();
        for entrave in raw_entraves {
            lines.extend(entrave_paths(entrave));
        }

        let geometry = match lines_to_geometry(lines) {
            Some(geometry) => geometry,
            None => {
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
        println!("No current road works from gatineau; keeping previous import");
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
            "Done fetching gatineau data ({} road works from {} current projects, {} skipped)",
            inserted, works_count, skipped
        ),
        None => println!("Error replacing gatineau road works"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convertit_le_web_mercator_en_wgs84() {
        let [longitude, latitude] = web_mercator_to_wgs84(-8_421_000.0, 5_687_000.0);

        assert!((longitude + 75.65).abs() < 0.2);
        assert!((latitude - 45.50).abs() < 0.2);
    }
}
