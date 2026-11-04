use geojson::GeoJson;
use sqlx::postgres::Postgres;
use std::str::FromStr;
use std::time::Duration;
use time::OffsetDateTime;

use crate::db::road_work::Roadwork;

// Couche « Rue en Chantier / Street under construction » du service public de
// Westmount. L'URL est millésimée : à mettre à jour chaque année (le service
// 2025 existe aussi sous InfoTravauxPublic2025).
const ROAD_WORK_URL: &str = "https://cartes.westmount.org/server/rest/services/InfoTravauxPublic2026/FeatureServer/7/query";

// Etat : 1 = à venir, 2 = en cours, 3 = terminé.
const ACTIVE_WORKS_FILTER: &str = "Etat IN (1,2)";
const SOURCE: &str = "westmount";

/// Télécharge les rues en chantier actives de Westmount et remplace les
/// travaux précédemment importés pour cette source.
pub async fn fetch_westmount_data(conn: &sqlx::Pool<Postgres>) {
    let response = match reqwest::Client::new()
        .get(ROAD_WORK_URL)
        .query(&[
            ("where", ACTIVE_WORKS_FILTER),
            ("outFields", "*"),
            ("returnGeometry", "true"),
            ("f", "geojson"),
        ])
        .timeout(Duration::from_secs(30))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            println!("Error fetching westmount road works: {}", e);
            return;
        }
    };

    if !response.status().is_success() {
        println!(
            "Westmount road works request failed with status {}",
            response.status()
        );
        return;
    }

    let body = match response.text().await {
        Ok(body) => body,
        Err(e) => {
            println!("Error reading westmount road works response: {}", e);
            return;
        }
    };

    let features = match GeoJson::from_str(&body) {
        Ok(GeoJson::FeatureCollection(collection)) => collection.features,
        Ok(_) => {
            println!("Unexpected westmount road works geojson: expected a FeatureCollection");
            return;
        }
        Err(e) => {
            println!("Error parsing westmount road works geojson: {}", e);
            return;
        }
    };

    // Ne supprimer les anciennes données qu'une fois le téléchargement réussi.
    Roadwork::remove_source(conn, SOURCE).await;

    // Les lignes de la couche 7 n'ont pas de dates : on utilise la date du jour,
    // comme l'import de Montréal qui renseigne des dates fixes.
    let today = OffsetDateTime::now_utc().date();
    let mut inserted = 0;
    for feature in features {
        if let Some(geometry) = feature.geometry {
            let road_work = Roadwork {
                geom: geometry,
                start_date: today,
                end_date: today,
                source: SOURCE.to_string(),
            };
            road_work.insert_wgs84(conn).await;
            inserted += 1;
        }
    }

    println!("Done fetching westmount data ({} road works)", inserted);
}
