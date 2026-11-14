use serde_json::{json, Value};
use sqlx::postgres::Postgres;
use std::time::Duration;

use super::road_work_import::{local_today, replace_raw_road_works, to_time_date};

// Carte interactive de Dollard-des-Ormeaux :
// https://ville.ddo.qc.ca/carte-interactive/
//
// La carte est un module WordPress maison qui embarque ses points dans une
// variable JavaScript `actMapData`. On récupère donc la page HTML puis on
// extrait cette variable, plutôt que de dépendre d'une API interne.
const MAP_URL: &str = "https://ville.ddo.qc.ca/carte-interactive/";

const SOURCE: &str = "ddo";

/// Extrait le tableau JavaScript `actMapData` de la page de la carte.
fn extract_act_map_data(html: &str) -> Option<Vec<Value>> {
    let marker = "const actMapData = ";
    let start = html.find(marker)? + marker.len();
    let bytes = html.as_bytes();

    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, byte) in bytes[start..].iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    let end = start + offset + 1;
                    return serde_json::from_str(&html[start..end]).ok();
                }
            }
            _ => {}
        }
    }

    None
}

fn as_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

fn is_road_work(item: &Value) -> bool {
    let title = item
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();
    let street = item
        .get("rue")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();

    !(title.contains("parc ")
        || title.contains("terrain de")
        || street.starts_with("parc ")
        || street.contains("parc "))
}

/// Télécharge les travaux en cours de Dollard-des-Ormeaux et remplace les
/// travaux précédemment importés pour cette source.
pub async fn fetch_ddo_data(conn: &sqlx::Pool<Postgres>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            println!("Error building ddo http client: {}", e);
            return;
        }
    };

    let response = match client.get(MAP_URL).send().await {
        Ok(response) => response,
        Err(e) => {
            println!("Error fetching {}: {}", MAP_URL, e);
            return;
        }
    };

    if !response.status().is_success() {
        println!("Request to {} failed with status {}", MAP_URL, response.status());
        return;
    }

    let html = match response.text().await {
        Ok(html) => html,
        Err(e) => {
            println!("Error reading response from {}: {}", MAP_URL, e);
            return;
        }
    };

    let items = match extract_act_map_data(&html) {
        Some(items) => items,
        None => {
            println!("Could not find actMapData in {}", MAP_URL);
            return;
        }
    };

    let today = local_today();
    let today_sql = match to_time_date(today) {
        Some(date) => date,
        None => {
            println!("Error converting today's date for ddo");
            return;
        }
    };

    let mut features = Vec::new();
    let mut skipped = 0usize;

    for item in items {
        let slugs = item
            .get("categslugs")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !slugs.split('|').any(|slug| slug == "en-cours") || !is_road_work(&item) {
            continue;
        }

        let (Some(longitude), Some(latitude)) = (
            item.get("longitude").and_then(as_f64),
            item.get("latitude").and_then(as_f64),
        ) else {
            skipped += 1;
            continue;
        };

        features.push(json!({
            "type": "Feature",
            "geometry": {
                "type": "Point",
                "coordinates": [longitude, latitude],
            },
            "properties": {
                "veloinfo_start_date": today.format("%Y-%m-%d").to_string(),
                "veloinfo_end_date": today.format("%Y-%m-%d").to_string(),
            },
        }));
    }

    if features.is_empty() {
        println!("No current road works from ddo; keeping previous import");
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
            "Done fetching ddo data ({} road works from {} current features, {} skipped)",
            inserted, works_count, skipped
        ),
        None => println!("Error replacing ddo road works"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrait_le_tableau_act_map_data() {
        let html = r#"<script>const actMapData = [{"id":1,"categslugs":"en-cours|construction"}];</script>"#;
        let items = extract_act_map_data(html).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], 1);
    }

    #[test]
    fn ignore_les_travaux_de_parc() {
        assert!(!is_road_work(&json!({
            "title": "Installation d'éclairage - Terrain de soccer au parc Lake Road",
            "rue": "Terrain de soccer au parc Lake Road"
        })));
        assert!(is_road_work(&json!({
            "title": "Réparation d'égout",
            "rue": "Huron à l'intersection de Westminster"
        })));
    }
}
