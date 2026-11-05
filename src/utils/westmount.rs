use geojson::GeoJson;
use sqlx::postgres::Postgres;
use std::str::FromStr;
use std::time::Duration;
use time::OffsetDateTime;

// Couche « Rue en Chantier / Street under construction » du service public de
// Westmount. L'URL est millésimée : à mettre à jour chaque année (le service
// 2025 existe aussi sous InfoTravauxPublic2025).
//
// Attention : cette couche ne contient souvent qu'une ligne droite entre le
// début et la fin de la rue (ex. Mountain n'a que deux points et coupe les
// îlots). On la recale donc sur RoutesLabels, le centreline officiel des rues.
const ROAD_WORK_URL: &str = "https://cartes.westmount.org/server/rest/services/InfoTravauxPublic2026/FeatureServer/7/query";
const ROUTES_URL: &str = "https://cartes.westmount.org/server/rest/services/RoutesLabels/FeatureServer/0/query";

// Etat : 1 = à venir, 2 = en cours, 3 = terminé.
const ACTIVE_WORKS_FILTER: &str = "Etat IN (1,2)";
const SOURCE: &str = "westmount";

/// Insère les lignes de travaux recalées sur le réseau routier officiel.
///
/// Chaque ligne de travaux est associée à la rue de RoutesLabels la plus proche
/// (début + milieu + fin), puis on garde la portion de cette rue comprise entre
/// les extrémités de la ligne. Si le recalage couvre moins de 60 % de la ligne
/// d'origine, on conserve la géométrie brute pour ne pas perdre de tronçon.
const INSERT_SNAPPED_SQL: &str = r#"
WITH work_raw AS (
    SELECT ST_Transform(ST_SetSRID(ST_GeomFromGeoJSON(feature->'geometry'), 4326), 3857) AS geom
    FROM json_array_elements(($1::json)->'features') AS feature
),
work AS (
    SELECT (ST_Dump(ST_LineMerge(geom))).geom AS geom FROM work_raw
),
route AS (
    SELECT (ST_Dump(ST_LineMerge(geom))).geom AS geom
    FROM (
        SELECT ST_Transform(ST_SetSRID(ST_GeomFromGeoJSON(feature->'geometry'), 4326), 3857) AS geom
        FROM json_array_elements(($2::json)->'features') AS feature
    ) AS routes
),
matched AS (
    SELECT
        w.geom AS work_geom,
        r.geom AS route_geom,
        ST_LineLocatePoint(r.geom, ST_StartPoint(w.geom)) AS start_fraction,
        ST_LineLocatePoint(r.geom, ST_EndPoint(w.geom)) AS end_fraction
    FROM work AS w
    CROSS JOIN LATERAL (
        SELECT route.geom
        FROM route
        WHERE GeometryType(route.geom) = 'LINESTRING'
        ORDER BY
            ST_Distance(route.geom, ST_StartPoint(w.geom))
          + ST_Distance(route.geom, ST_EndPoint(w.geom))
          + ST_Distance(route.geom, ST_LineInterpolatePoint(w.geom, 0.5))
        LIMIT 1
    ) AS r
),
snapped AS (
    SELECT
        CASE
            WHEN ST_Length(ST_LineSubstring(route_geom, LEAST(start_fraction, end_fraction), GREATEST(start_fraction, end_fraction))) >= 0.6 * ST_Length(work_geom)
            THEN ST_LineSubstring(route_geom, LEAST(start_fraction, end_fraction), GREATEST(start_fraction, end_fraction))
            ELSE work_geom
        END AS geom
    FROM matched
)
INSERT INTO road_work (geom, start_date, end_date, source)
SELECT geom, $3::date, $3::date, $4
FROM snapped
"#;

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
/// réseau routier officiel et remplace les travaux précédemment importés pour
/// cette source.
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

    let (routes_json, routes_count) = match fetch_feature_collection(
        &client,
        ROUTES_URL,
        &[
            ("where", "1=1"),
            ("outFields", "OBJECTID"),
            ("returnGeometry", "true"),
            ("f", "geojson"),
        ],
    )
    .await
    {
        Some(result) => result,
        None => return,
    };

    // Sans réseau routier, le recalage ne produirait rien : on garde les
    // données existantes plutôt que de vider la table.
    if routes_count == 0 {
        println!("Westmount road network is empty, keeping existing road works");
        return;
    }

    // Les lignes de la couche 7 n'ont pas de dates : on utilise la date du jour,
    // comme l'import de Montréal qui renseigne des dates fixes.
    let today = OffsetDateTime::now_utc().date();

    let mut transaction = match conn.begin().await {
        Ok(transaction) => transaction,
        Err(e) => {
            println!("Error starting westmount transaction: {}", e);
            return;
        }
    };

    if let Err(e) = sqlx::query("DELETE FROM road_work WHERE source = $1")
        .bind(SOURCE)
        .execute(&mut *transaction)
        .await
    {
        println!("Error removing westmount road works: {}", e);
        return;
    }

    match sqlx::query(INSERT_SNAPPED_SQL)
        .bind(&works_json)
        .bind(&routes_json)
        .bind(today)
        .bind(SOURCE)
        .execute(&mut *transaction)
        .await
    {
        Ok(result) => match transaction.commit().await {
            Ok(_) => println!(
                "Done fetching westmount data ({} road works from {} source features)",
                result.rows_affected(),
                works_count
            ),
            Err(e) => println!("Error committing westmount road works: {}", e),
        },
        Err(e) => println!("Error inserting westmount road works: {}", e),
    }
}
