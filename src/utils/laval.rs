use serde_json::{json, Value};
use sqlx::postgres::Postgres;
use std::time::Duration;

use super::road_work_import::{
    local_today, millis_to_date, replace_road_works, to_time_date,
};

// Carte « Info-travaux » de la Ville de Laval :
// https://vl.maps.arcgis.com/apps/instant/sidebar/index.html?appid=729ff9eeb851437b9a4cf365efadfe8f
//
// L'opération `query` du service renvoie bien les attributs mais pas les
// géométries (`geometry: null`). L'opération `identify`, elle, les renvoie :
// on l'appelle avec l'emprise complète de la ville pour récupérer d'un coup
// toutes les entraves en cours, avec leurs lignes.
const IDENTIFY_URL: &str =
    "https://gis.laval.ca/arcgis/rest/services/ing/Obstruction_14_jours/MapServer/identify";

// Emprise large autour de Laval, en WGS84. Elle doit contenir toute la ville
// pour que l'identification retourne toutes les entraves.
const LAVAL_EXTENT: &str = "-74.1,45.4,-73.4,45.85";

// Couches 0 (rues barrées) et 2 (entraves partielles). Les couches 1 et 4 sont
// des détours, et la couche 3 est la planification à 14 jours : on ne garde que
// les travaux en cours.
const CURRENT_LAYERS: &str = "all:0,2";

const SOURCE: &str = "laval";

/// Convertit les `paths` d'une géométrie Esri JSON en géométrie GeoJSON.
fn paths_to_geometry(paths: &Value) -> Option<Value> {
    let mut lines: Vec<Vec<[f64; 2]>> = Vec::new();

    for path in paths.as_array()? {
        let coordinates: Option<Vec<[f64; 2]>> = path
            .as_array()?
            .iter()
            .map(|point| {
                let point = point.as_array()?;
                Some([point.first()?.as_f64()?, point.get(1)?.as_f64()?])
            })
            .collect();

        let coordinates = coordinates?;
        if coordinates.len() >= 2 {
            lines.push(coordinates);
        }
    }

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

/// Télécharge les entraves en cours de la Ville de Laval, les recale sur le
/// réseau routier OSM et remplace les travaux précédemment importés pour cette
/// source.
pub async fn fetch_laval_data(conn: &sqlx::Pool<Postgres>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            println!("Error building laval http client: {}", e);
            return;
        }
    };

    let response = match client
        .get(IDENTIFY_URL)
        .query(&[
            ("geometry", LAVAL_EXTENT),
            ("geometryType", "esriGeometryEnvelope"),
            ("sr", "4326"),
            ("layers", CURRENT_LAYERS),
            ("tolerance", "0"),
            ("mapExtent", LAVAL_EXTENT),
            ("imageDisplay", "1200,900,96"),
            ("returnGeometry", "true"),
            ("returnFieldName", "true"),
            ("returnUnformattedValues", "true"),
            ("f", "json"),
        ])
        .send()
        .await
    {
        Ok(response) => response,
        Err(e) => {
            println!("Error fetching {}: {}", IDENTIFY_URL, e);
            return;
        }
    };

    if !response.status().is_success() {
        println!(
            "Request to {} failed with status {}",
            IDENTIFY_URL,
            response.status()
        );
        return;
    }

    let body: Value = match response.json().await {
        Ok(body) => body,
        Err(e) => {
            println!("Error parsing json from {}: {}", IDENTIFY_URL, e);
            return;
        }
    };

    let results = match body.get("results").and_then(Value::as_array) {
        Some(results) => results,
        None => {
            println!(
                "Unexpected json from {}: expected a results array",
                IDENTIFY_URL
            );
            return;
        }
    };

    let today = local_today();
    let today_sql = match to_time_date(today) {
        Some(date) => date,
        None => {
            println!("Error converting today's date for laval");
            return;
        }
    };

    let mut features = Vec::new();
    let mut skipped = 0usize;

    for result in results {
        let attributes = match result.get("attributes") {
            Some(attributes) => attributes,
            None => continue,
        };

        // On ne garde que les entraves actives aujourd'hui : la couche 2
        // contient parfois des permis dont la date de fin est dépassée.
        let (start, end) = match (
            attributes.get("DATE_DEBUT").and_then(millis_to_date),
            attributes.get("DATE_FIN").and_then(millis_to_date),
        ) {
            (Some(start), Some(end)) if start <= today && today <= end => (start, end),
            _ => {
                skipped += 1;
                continue;
            }
        };

        let geometry = match result
            .pointer("/geometry/paths")
            .and_then(paths_to_geometry)
        {
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

    // Une réponse vide peut venir d'une erreur passagère du service : on
    // conserve alors l'import précédent plutôt que d'effacer les travaux.
    if features.is_empty() {
        println!("No current road works from laval; keeping previous import");
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
            "Done fetching laval data ({} road works from {} current features, {} skipped)",
            inserted, works_count, skipped
        ),
        None => println!("Error replacing laval road works"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convertit_un_seul_trace_en_ligne() {
        let paths = json!([[[-73.7, 45.6], [-73.71, 45.61]]]);
        let geometry = paths_to_geometry(&paths).unwrap();

        assert_eq!(geometry["type"], "LineString");
        assert_eq!(geometry["coordinates"][0][0], -73.7);
    }

    #[test]
    fn convertit_plusieurs_traces_en_multiligne() {
        let paths = json!([
            [[-73.7, 45.6], [-73.71, 45.61]],
            [[-73.72, 45.62], [-73.73, 45.63]]
        ]);
        let geometry = paths_to_geometry(&paths).unwrap();

        assert_eq!(geometry["type"], "MultiLineString");
        assert_eq!(geometry["coordinates"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn ignore_les_traces_trop_courts() {
        let paths = json!([[[-73.7, 45.6]]]);

        assert!(paths_to_geometry(&paths).is_none());
    }
}
