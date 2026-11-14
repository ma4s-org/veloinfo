use serde_json::{json, Value};
use sqlx::postgres::Postgres;
use std::time::Duration;

use super::road_work_import::{
    local_today, millis_to_date, replace_road_works, to_time_date,
};

// Carte interactive de la Ville de Lévis :
// https://levis.ca/fr/carte-interactive?map=travaux
//
// Le service contient plusieurs couches : le pavage, les travaux
// d'infrastructure en cours (ponctuels et linéaires) et les travaux à venir.
// On ne garde que les couches linéaires de travaux en cours.
const SERVICE_URL: &str =
    "https://cartes.ville.levis.qc.ca/arcgis/rest/services/TravauxInfra/MapServer";

// 0 = pavage, 7 = infrastructure linéaire en cours. Les couches 5 et 6 sont
// les travaux à venir, et les couches 3 et 4 sont des points (travaux
// ponctuels) que road_work ne sait pas encore afficher.
const CURRENT_LAYERS: &[u32] = &[0, 7];

const SOURCE: &str = "levis";

/// Vérifie qu'une géométrie GeoJSON est bien une ligne.
fn is_line_geometry(geometry: &Value) -> bool {
    matches!(
        geometry.get("type").and_then(Value::as_str),
        Some("LineString" | "MultiLineString")
    )
}

/// Télécharge une couche ArcGIS au format GeoJSON.
async fn fetch_layer(client: &reqwest::Client, layer: u32) -> Option<Vec<Value>> {
    let url = format!("{SERVICE_URL}/{layer}/query");

    let response = match client
        .get(&url)
        .query(&[
            ("where", "1=1"),
            ("outFields", "*"),
            ("returnGeometry", "true"),
            ("f", "geojson"),
        ])
        .send()
        .await
    {
        Ok(response) => response,
        Err(e) => {
            println!("Error fetching {}: {}", url, e);
            return None;
        }
    };

    if !response.status().is_success() {
        println!("Request to {} failed with status {}", url, response.status());
        return None;
    }

    let body: Value = match response.json().await {
        Ok(body) => body,
        Err(e) => {
            println!("Error parsing json from {}: {}", url, e);
            return None;
        }
    };

    match body.get("features").and_then(Value::as_array) {
        Some(features) => Some(features.clone()),
        None => {
            println!("Unexpected json from {}: expected a FeatureCollection", url);
            None
        }
    }
}

/// Télécharge les travaux en cours de la Ville de Lévis, les recale sur le
/// réseau routier OSM et remplace les travaux précédemment importés pour cette
/// source.
pub async fn fetch_levis_data(conn: &sqlx::Pool<Postgres>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            println!("Error building levis http client: {}", e);
            return;
        }
    };

    let mut raw_features = Vec::new();
    for &layer in CURRENT_LAYERS {
        match fetch_layer(&client, layer).await {
            Some(features) => raw_features.extend(features),
            None => {
                println!(
                    "Error fetching levis layer {}; keeping previous import",
                    layer
                );
                return;
            }
        }
    }

    let today = local_today();
    let today_sql = match to_time_date(today) {
        Some(date) => date,
        None => {
            println!("Error converting today's date for levis");
            return;
        }
    };

    let mut features = Vec::new();
    let mut skipped = 0usize;

    for mut feature in raw_features {
        let properties = feature.get("properties").cloned().unwrap_or(Value::Null);

        // La couche de pavage n'a pas de dates : elle représente les travaux
        // de la saison en cours, on la considère donc active aujourd'hui.
        let (start, end) = match (
            properties.get("Date_debut").and_then(millis_to_date),
            properties.get("Date_fin").and_then(millis_to_date),
        ) {
            (Some(start), Some(end)) if start <= today && today <= end => (start, end),
            (None, None) => (today, today),
            _ => {
                skipped += 1;
                continue;
            }
        };

        let geometry = match feature
            .as_object_mut()
            .and_then(|object| object.remove("geometry"))
        {
            Some(geometry) if !geometry.is_null() => geometry,
            _ => {
                skipped += 1;
                continue;
            }
        };

        if !is_line_geometry(&geometry) {
            skipped += 1;
            continue;
        }

        features.push(json!({
            "type": "Feature",
            "geometry": geometry,
            "properties": {
                "veloinfo_start_date": start.format("%Y-%m-%d").to_string(),
                "veloinfo_end_date": end.format("%Y-%m-%d").to_string(),
            },
        }));
    }

    // Comme pour les autres sources, on garde l'import précédent si le service
    // renvoie une réponse vide ou inattendue.
    if features.is_empty() {
        println!("No current road works from levis; keeping previous import");
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
            "Done fetching levis data ({} road works from {} current features, {} skipped)",
            inserted, works_count, skipped
        ),
        None => println!("Error replacing levis road works"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepte_les_lignes() {
        assert!(is_line_geometry(&json!({
            "type": "LineString",
            "coordinates": [[-71.2, 46.8], [-71.21, 46.81]]
        })));
        assert!(is_line_geometry(&json!({
            "type": "MultiLineString",
            "coordinates": [[[-71.2, 46.8], [-71.21, 46.81]]]
        })));
    }

    #[test]
    fn refuse_les_points() {
        assert!(!is_line_geometry(&json!({
            "type": "Point",
            "coordinates": [-71.2, 46.8]
        })));
    }
}
