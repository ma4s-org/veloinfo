use chrono::{DateTime, Datelike, NaiveDate, Utc};
use chrono_tz::America::Montreal;
use serde_json::Value;
use sqlx::postgres::Postgres;
use time::Date;

/// Date du jour à Montréal. Les imports nocturnes sont planifiés à 1 h, heure
/// locale, mais un import manuel peut tomber près de minuit.
pub fn local_today() -> NaiveDate {
    Utc::now().with_timezone(&Montreal).date_naive()
}

/// Convertit un horodatage ArcGIS en date civile.
///
/// Les champs de type date sont renvoyés en millisecondes depuis l'époque, à
/// minuit UTC (par exemple `2026-09-13T00:00:00Z` pour le 13 septembre 2026).
pub fn millis_to_date(value: &Value) -> Option<NaiveDate> {
    let millis = value.as_i64()?;
    DateTime::<Utc>::from_timestamp_millis(millis).map(|datetime| datetime.date_naive())
}

/// Convertit une date `chrono` en date `time` pour l'insertion SQL.
pub fn to_time_date(date: NaiveDate) -> Option<Date> {
    let month = time::Month::try_from(date.month() as u8).ok()?;
    Date::from_calendar_date(date.year(), month, date.day() as u8).ok()
}

/// Insère des travaux sans les recaler sur le réseau OSM.
///
/// Utile pour les sources qui fournissent des points (une entrave localisée)
/// ou des lignes déjà alignées sur le réseau routier.
pub const INSERT_RAW_SQL: &str = r#"
INSERT INTO road_work (geom, start_date, end_date, source)
SELECT
    ST_Transform(ST_SetSRID(ST_GeomFromGeoJSON(feature->'geometry'), 4326), 3857),
    COALESCE(
        NULLIF(feature->'properties'->>'veloinfo_start_date', '')::date,
        $2::date
    ),
    COALESCE(
        NULLIF(feature->'properties'->>'veloinfo_end_date', '')::date,
        $2::date
    ),
    $3
FROM json_array_elements(($1::json)->'features') AS feature
WHERE feature->'geometry' IS NOT NULL
"#;

/// Insère des travaux fournis sous forme de polygones en les découpant sur le
/// réseau routier OSM.
///
/// Les polygones de Montréal représentent des zones de chantier. Pour avoir un
/// affichage cohérent avec les autres villes, on garde les tronçons de rue qui
/// traversent la zone. Si un polygone ne touche aucune rue (parc, ruelle, etc.),
/// on insère un point à son centroïde.
pub const INSERT_POLYGON_ROADS_SQL: &str = r#"
WITH work_raw AS (
    SELECT
        row_number() OVER () AS oid,
        COALESCE(
            NULLIF(feature->'properties'->>'veloinfo_start_date', '')::date,
            $2::date
        ) AS start_date,
        COALESCE(
            NULLIF(feature->'properties'->>'veloinfo_end_date', '')::date,
            $2::date
        ) AS end_date,
        ST_Transform(ST_SetSRID(ST_GeomFromGeoJSON(feature->'geometry'), 4326), 3857) AS geom
    FROM json_array_elements(($1::json)->'features') AS feature
    WHERE feature->'geometry' IS NOT NULL
),
work_valid AS (
    SELECT
        oid,
        start_date,
        end_date,
        ST_CollectionExtract(ST_MakeValid(geom), 3) AS geom
    FROM work_raw
),
work_poly AS (
    SELECT oid, start_date, end_date, (ST_Dump(geom)).geom AS geom
    FROM work_valid
    WHERE NOT ST_IsEmpty(geom)
),
work_extent AS (
    SELECT ST_Expand(ST_Extent(geom), 200) AS geom FROM work_poly
),
roads AS (
    SELECT DISTINCT ON (way_id, geom)
        way_id,
        geom
    FROM (
        SELECT
            t.way_id,
            t.geom
        FROM transportation AS t, work_extent AS e
        WHERE t.highway IN (
            'residential', 'tertiary', 'secondary', 'primary', 'unclassified',
            'service', 'living_street', 'pedestrian', 'trunk', 'motorway',
            'trunk_link', 'motorway_link', 'primary_link', 'secondary_link',
            'tertiary_link'
        )
          AND t.geom && e.geom

        UNION ALL

        -- Les pistes cyclables et aménagements cyclables sont dans all_way,
        -- pas dans transportation. Un chantier qui couvre une piste doit
        -- aussi être découpé sur cette piste.
        SELECT
            aw.way_id,
            aw.geom
        FROM all_way AS aw, work_extent AS e
        WHERE aw.geom && e.geom
          AND (
            aw.tags->>'highway' = 'cycleway'
            OR aw.tags->>'cycleway' IN ('track', 'lane', 'shared_lane')
            OR aw.tags->>'cycleway:left' IN ('track', 'lane', 'shared_lane')
            OR aw.tags->>'cycleway:right' IN ('track', 'lane', 'shared_lane')
            OR aw.tags->>'cycleway:both' IN ('track', 'lane', 'shared_lane')
            OR aw.tags->>'bicycle' IN ('designated', 'yes')
            OR aw.tags->>'cyclestreet' = 'yes'
          )
    ) AS candidates
),
clipped AS (
    SELECT
        w.oid,
        w.start_date,
        w.end_date,
        (ST_Dump(ST_CollectionExtract(
            ST_MakeValid(ST_Intersection(
                ST_MakeValid(r.geom),
                ST_Buffer(w.geom, 8)
            )),
            2
        ))).geom AS geom
    FROM work_poly AS w
    JOIN roads AS r
        ON r.geom && ST_Expand(w.geom, 8)
        AND ST_DWithin(r.geom, w.geom, 8)
),
lines AS (
    SELECT DISTINCT ON (oid, geom)
        oid,
        start_date,
        end_date,
        geom
    FROM clipped
    WHERE GeometryType(geom) = 'LINESTRING'
      AND ST_Length(geom) > 1
),
line_oids AS (
    SELECT DISTINCT oid FROM lines
),
fallbacks AS (
    SELECT
        w.oid,
        w.start_date,
        w.end_date,
        w.geom
    FROM work_poly AS w
    WHERE NOT EXISTS (SELECT 1 FROM line_oids AS l WHERE l.oid = w.oid)
)
INSERT INTO road_work (geom, start_date, end_date, source)
SELECT geom, start_date, end_date, $3 FROM lines
UNION ALL
SELECT geom, start_date, end_date, $3 FROM fallbacks
"#;

/// Remplace les travaux d'une source par ceux de la FeatureCollection fournie,
/// en découpant les polygones sur le réseau OSM.
pub async fn replace_polygon_road_works(
    conn: &sqlx::Pool<Postgres>,
    source: &str,
    works_json: &str,
    default_date: Date,
) -> Option<u64> {
    let mut transaction = match conn.begin().await {
        Ok(transaction) => transaction,
        Err(e) => {
            println!("Error starting {} transaction: {}", source, e);
            return None;
        }
    };

    if let Err(e) = sqlx::query("DELETE FROM road_work WHERE source = $1")
        .bind(source)
        .execute(&mut *transaction)
        .await
    {
        println!("Error removing {} road works: {}", source, e);
        return None;
    }

    let result = match sqlx::query(INSERT_POLYGON_ROADS_SQL)
        .bind(works_json)
        .bind(default_date)
        .bind(source)
        .execute(&mut *transaction)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            println!("Error inserting {} road works: {}", source, e);
            return None;
        }
    };

    match transaction.commit().await {
        Ok(_) => {
            super::import_status::mark_success(source);
            Some(result.rows_affected())
        }
        Err(e) => {
            println!("Error committing {} road works: {}", source, e);
            None
        }
    }
}

/// Remplace les travaux d'une source par ceux de la FeatureCollection fournie,
/// sans recalage sur le réseau OSM.
pub async fn replace_raw_road_works(
    conn: &sqlx::Pool<Postgres>,
    source: &str,
    works_json: &str,
    default_date: Date,
) -> Option<u64> {
    let mut transaction = match conn.begin().await {
        Ok(transaction) => transaction,
        Err(e) => {
            println!("Error starting {} transaction: {}", source, e);
            return None;
        }
    };

    if let Err(e) = sqlx::query("DELETE FROM road_work WHERE source = $1")
        .bind(source)
        .execute(&mut *transaction)
        .await
    {
        println!("Error removing {} road works: {}", source, e);
        return None;
    }

    let result = match sqlx::query(INSERT_RAW_SQL)
        .bind(works_json)
        .bind(default_date)
        .bind(source)
        .execute(&mut *transaction)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            println!("Error inserting {} road works: {}", source, e);
            return None;
        }
    };

    match transaction.commit().await {
        Ok(_) => {
            super::import_status::mark_success(source);
            Some(result.rows_affected())
        }
        Err(e) => {
            println!("Error committing {} road works: {}", source, e);
            None
        }
    }
}

/// Insère les travaux d'une source à partir d'une FeatureCollection GeoJSON
/// (WGS84), en recalant les lignes sur le réseau routier OSM.
///
/// Pour chaque ligne de travaux, on considère les rues OSM les plus proches du
/// début, du milieu et de la fin. On garde ensuite, pour chaque rue, la portion
/// effectivement à proximité de la ligne (buffer de 30 m), puis on assemble les
/// morceaux. Si le recalage couvre moins de 60 % de la ligne d'origine, on
/// conserve la géométrie brute pour ne pas perdre de tronçon.
///
/// Les dates de début et de fin sont lues dans les propriétés
/// `veloinfo_start_date` et `veloinfo_end_date` (format `AAAA-MM-JJ`). Si elles
/// sont absentes, la date passée en paramètre est utilisée.
pub const INSERT_SNAPPED_SQL: &str = r#"
WITH work_raw AS (
    SELECT
        row_number() OVER () AS oid,
        COALESCE(
            NULLIF(feature->'properties'->>'veloinfo_start_date', '')::date,
            $2::date
        ) AS start_date,
        COALESCE(
            NULLIF(feature->'properties'->>'veloinfo_end_date', '')::date,
            $2::date
        ) AS end_date,
        ST_Transform(ST_SetSRID(ST_GeomFromGeoJSON(feature->'geometry'), 4326), 3857) AS geom
    FROM json_array_elements(($1::json)->'features') AS feature
    WHERE feature->'geometry' IS NOT NULL
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
    r.start_date,
    r.end_date,
    $3
FROM final AS f
JOIN work_raw AS r ON r.oid = f.oid
"#;

/// Remplace les travaux d'une source par ceux de la FeatureCollection fournie.
///
/// Les données existantes ne sont supprimées qu'une fois la nouvelle requête
/// prête : si le téléchargement ou l'insertion échoue, l'import précédent est
/// conservé. Renvoie le nombre de lignes insérées.
pub async fn replace_road_works(
    conn: &sqlx::Pool<Postgres>,
    source: &str,
    works_json: &str,
    default_date: Date,
) -> Option<u64> {
    let mut transaction = match conn.begin().await {
        Ok(transaction) => transaction,
        Err(e) => {
            println!("Error starting {} transaction: {}", source, e);
            return None;
        }
    };

    if let Err(e) = sqlx::query("DELETE FROM road_work WHERE source = $1")
        .bind(source)
        .execute(&mut *transaction)
        .await
    {
        println!("Error removing {} road works: {}", source, e);
        return None;
    }

    let result = match sqlx::query(INSERT_SNAPPED_SQL)
        .bind(works_json)
        .bind(default_date)
        .bind(source)
        .execute(&mut *transaction)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            println!("Error inserting {} road works: {}", source, e);
            return None;
        }
    };

    match transaction.commit().await {
        Ok(_) => {
            super::import_status::mark_success(source);
            Some(result.rows_affected())
        }
        Err(e) => {
            println!("Error committing {} road works: {}", source, e);
            None
        }
    }
}
