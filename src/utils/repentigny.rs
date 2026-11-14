use chrono::{DateTime, NaiveDate, NaiveDateTime};
use serde_json::{json, Value};
use sqlx::postgres::Postgres;
use std::time::Duration;

use super::road_work_import::{local_today, replace_road_works, to_time_date};

// API Open511 de la Ville de Repentigny.
//
// Le certificat TLS de ce domaine est incomplet (chaîne non vérifiable par
// rustls/OpenSSL), mais l'API est publique et en lecture seule.
const EVENTS_URL: &str = "https://info-travaux.ville.repentigny.qc.ca/api/events/?format=json";

const SOURCE: &str = "repentigny";

fn parse_datetime(text: &str) -> Option<NaiveDate> {
    if let Ok(datetime) = DateTime::parse_from_rfc3339(text) {
        return Some(datetime.date_naive());
    }

    NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M")
        .ok()
        .map(|datetime| datetime.date())
}

fn parse_interval(value: &Value) -> Option<(NaiveDate, NaiveDate)> {
    let interval = value.as_str()?;
    let (start, end) = interval.split_once('/')?;

    Some((parse_datetime(start)?, parse_datetime(end)?))
}

/// Télécharge les événements actifs de Repentigny, les recale sur le réseau
/// routier OSM et remplace les travaux précédemment importés.
pub async fn fetch_repentigny_data(conn: &sqlx::Pool<Postgres>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .danger_accept_invalid_certs(true)
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            println!("Error building repentigny http client: {}", e);
            return;
        }
    };

    let response = match client.get(EVENTS_URL).send().await {
        Ok(response) => response,
        Err(e) => {
            println!("Error fetching repentigny: {}", e);
            return;
        }
    };

    if !response.status().is_success() {
        println!(
            "Request to repentigny failed with status {}",
            response.status()
        );
        return;
    }

    let body: Value = match response.json().await {
        Ok(body) => body,
        Err(e) => {
            println!("Error parsing repentigny json: {}", e);
            return;
        }
    };

    let events = match body.get("events").and_then(Value::as_array) {
        Some(events) => events,
        None => {
            println!("Unexpected repentigny json: expected an events array");
            return;
        }
    };

    let today = local_today();
    let today_sql = match to_time_date(today) {
        Some(date) => date,
        None => {
            println!("Error converting today's date for repentigny");
            return;
        }
    };

    let mut features = Vec::new();
    let mut skipped = 0usize;

    for event in events {
        if event.get("status").and_then(Value::as_str) != Some("ACTIVE") {
            skipped += 1;
            continue;
        }

        let schedule = event
            .pointer("/schedule/intervals")
            .and_then(Value::as_array)
            .and_then(|intervals| intervals.first())
            .and_then(parse_interval);

        let (start, end) = match schedule {
            Some((start, end)) => (start, end),
            None => (today, today),
        };

        if start > today || end < today {
            skipped += 1;
            continue;
        }

        let geometry = match event.get("geography").cloned() {
            Some(geometry)
                if matches!(
                    geometry.get("type").and_then(Value::as_str),
                    Some("LineString" | "MultiLineString")
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
        println!("No current road works from repentigny; keeping previous import");
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
            "Done fetching repentigny data ({} road works from {} active events, {} skipped)",
            inserted, works_count, skipped
        ),
        None => println!("Error replacing repentigny road works"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_un_intervalle_open511() {
        let value = json!("2026-10-05T07:00/2026-11-20T17:00");
        let (start, end) = parse_interval(&value).unwrap();

        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 10, 5).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
    }
}
