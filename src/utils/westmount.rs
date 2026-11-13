use geojson::GeoJson;
use sqlx::postgres::Postgres;
use std::str::FromStr;
use std::time::Duration;
use time::OffsetDateTime;

use super::road_work_import::replace_road_works;

// Couche « Rue en Chantier / Street under construction » du service public de
// Westmount. L'URL est millésimée : à mettre à jour chaque année (le service
// 2025 existe aussi sous InfoTravauxPublic2025).
//
// Attention : cette couche ne contient souvent qu'une ligne droite entre le
// début et la fin de la rue (ex. Mountain n'a que deux points et coupe les
// îlots). On la recale sur le réseau routier OSM (table transportation), qui
// est aussi celui du fond de carte, pour que la ligne reste alignée avec la
// chaussée.
const ROAD_WORK_URL: &str = "https://cartes.westmount.org/server/rest/services/InfoTravauxPublic2026/FeatureServer/7/query";

// Etat : 1 = à venir, 2 = en cours, 3 = terminé.
const ACTIVE_WORKS_FILTER: &str = "Etat IN (1,2)";
const SOURCE: &str = "westmount";

/// Télécharge une FeatureCollection GeoJSON et renvoie le corps brut ainsi que
/// le nombre d'éléments, pour pouvoir l'injecter directement dans PostGIS.
async fn fetch_feature_collection(
    client: &reqwest::Client,
    url: &str,
    params: &[(&str, &str)],
) -> Option<(String, usize)> {
    let response = match client.get(url).query(params).send().await {
        Ok(r) => r,
        Err(e) => {
            println!("Error fetching {}: {}", url, e);
            return None;
        }
    };

    if !response.status().is_success() {
        println!("Request to {} failed with status {}", url, response.status());
        return None;
    }

    let body = match response.text().await {
        Ok(body) => body,
        Err(e) => {
            println!("Error reading response from {}: {}", url, e);
            return None;
        }
    };

    match GeoJson::from_str(&body) {
        Ok(GeoJson::FeatureCollection(collection)) => Some((body, collection.features.len())),
        Ok(_) => {
            println!("Unexpected geojson from {}: expected a FeatureCollection", url);
            None
        }
        Err(e) => {
            println!("Error parsing geojson from {}: {}", url, e);
            None
        }
    }
}

/// Télécharge les rues en chantier actives de Westmount, les recale sur le
/// réseau routier OSM et remplace les travaux précédemment importés pour cette
/// source.
pub async fn fetch_westmount_data(conn: &sqlx::Pool<Postgres>) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            println!("Error building westmount http client: {}", e);
            return;
        }
    };

    let (works_json, works_count) = match fetch_feature_collection(
        &client,
        ROAD_WORK_URL,
        &[
            ("where", ACTIVE_WORKS_FILTER),
            ("outFields", "*"),
            ("returnGeometry", "true"),
            ("f", "geojson"),
        ],
    )
    .await
    {
        Some(result) => result,
        None => return,
    };

    // Les lignes de la couche 7 n'ont pas de dates : on utilise la date du jour,
    // comme l'import de Montréal qui renseigne des dates fixes.
    let today = OffsetDateTime::now_utc().date();

    match replace_road_works(conn, SOURCE, &works_json, today).await {
        Some(inserted) => println!(
            "Done fetching westmount data ({} road works from {} source features)",
            inserted, works_count
        ),
        None => println!("Error replacing westmount road works"),
    }
}
