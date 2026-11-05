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
// îlots). On la recale sur le réseau routier OSM (table transportation), qui
// est aussi celui du fond de carte, pour que la ligne reste alignée avec la
// chaussée.
const ROAD_WORK_URL: &str = "https://cartes.westmount.org/server/rest/services/InfoTravauxPublic2026/FeatureServer/7/query";

// Etat : 1 = à venir, 2 = en cours, 3 = terminé.
const ACTIVE_WORKS_FILTER: &str = "Etat IN (1,2)";
const SOURCE: &str = "westmount";

/// Insère les lignes de travaux recalées sur le réseau routier OSM.
///
/// Pour chaque ligne de travaux, on considère les rues OSM les plus proches du
/// début, du milieu et de la fin. On garde ensuite, pour chaque rue, la portion
/// effectivement à proximité de la ligne (buffer de 30 m), puis on assemble les
/// morceaux. Si le recalage couvre moins de 60 % de la ligne d'origine, on
/// conserve la géométrie brute pour ne pas perdre de tronçon.
const INSERT_SNAPPED_SQL: &str = r#"
WITH work_raw AS (
    SELECT
        row_number() OVER () AS oid,
        ST_Transform(ST_SetSRID(ST_GeomFromGeoJSON(feature->'geometry'), 4326), 3857) AS geom
    FROM json_array_elements(($1::json)->'features') AS feature
),
work AS (
    SELECT oid, (ST_Dump(ST_LineMerge(geom))).geom AS geom
    FROM work_raw
),
work_extent AS (
    SELECT ST_Expand(ST_Extent(geom), 200) AS geom FROM work
),
route AS (
    SELECT rid, (ST_Dump(ST_LineMerge(ST_Collect(geom)))).geom AS geom
    FROM (
        SELECT
            DENSE_RANK() OVER (ORDER BY COALESCE(t.name, t.way_id::text)) AS rid,
            COALESCE(t.name, t.way_id::text) AS name,
            t.geom
        FROM transportation AS t, work_extent AS e
        WHERE t.highway IN (
            'residential', 'tertiary', 'secondary', 'primary', 'unclassified',
            'service', 'living_street', 'pedestrian', 'trunk', 'motorway',
            'trunk_link', 'motorway_link', 'primary_link', 'secondary_link',
            'tertiary_link'
        )
          AND t.geom && e.geom
    ) AS roads
    GROUP BY rid, name
),
points AS (
    SELECT oid, geom, ST_StartPoint(geom) AS pt FROM work
    UNION ALL
    SELECT oid, geom, ST_LineInterpolatePoint(geom, 0.5) FROM work
    UNION ALL
    SELECT oid, geom, ST_EndPoint(geom) FROM work
),
nearest AS (
    SELECT DISTINCT ON (p.oid, p.geom, r.geom)
        p.oid,
        p.geom AS work_geom,
        r.geom AS route_geom
    FROM points AS p
    CROSS JOIN LATERAL (
        SELECT r.rid, r.geom
        FROM route AS r
        WHERE GeometryType(r.geom) = 'LINESTRING'
        ORDER BY ST_Distance(r.geom, p.pt)
        LIMIT 1
    ) AS r
    ORDER BY p.oid, p.geom, r.geom, ST_Distance(r.geom, p.pt)
),
near AS (
    SELECT
        n.oid,
        n.work_geom,
        n.route_geom,
        (ST_Dump(ST_Intersection(n.work_geom, ST_Buffer(n.route_geom, 30)))).geom AS near_geom
    FROM nearest AS n
),
near_lines AS (
    SELECT *
    FROM near
    WHERE GeometryType(near_geom) = 'LINESTRING'
      AND ST_Length(near_geom) > 1
),
fractions AS (
    SELECT
        oid,
        work_geom,
        route_geom,
        MIN(LEAST(f1, f2)) AS fmin,
        MAX(GREATEST(f1, f2)) AS fmax
    FROM (
        SELECT
            oid,
            work_geom,
            route_geom,
            ST_LineLocatePoint(route_geom, ST_StartPoint(near_geom)) AS f1,
            ST_LineLocatePoint(route_geom, ST_EndPoint(near_geom)) AS f2
        FROM near_lines
    ) AS projected
    GROUP BY oid, work_geom, route_geom
),
clips AS (
    SELECT
        oid,
        work_geom,
        ST_LineSubstring(route_geom, fmin, fmax) AS clip
    FROM fractions
),
filtered AS (
    SELECT *
    FROM clips
    WHERE ST_Length(clip) >= GREATEST(10, 0.1 * ST_Length(work_geom))
),
unioned AS (
    SELECT oid, work_geom, ST_LineMerge(ST_Union(clip)) AS geom
    FROM filtered
    GROUP BY oid, work_geom
),
final AS (
    SELECT
        w.oid,
        w.geom AS work_geom,
        u.geom AS snapped_geom
    FROM work AS w
    LEFT JOIN unioned AS u
        ON u.oid = w.oid AND ST_Equals(u.work_geom, w.geom)
)
INSERT INTO road_work (geom, start_date, end_date, source)
SELECT
    CASE
        WHEN snapped_geom IS NULL OR ST_Length(snapped_geom) < 0.6 * ST_Length(work_geom)
        THEN work_geom
        ELSE snapped_geom
    END,
    $2::date,
    $2::date,
    $3
FROM final
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
