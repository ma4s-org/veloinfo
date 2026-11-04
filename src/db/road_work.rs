use geo::{
    Geometry as GeoGeometry, LineString, MultiLineString, MultiPoint, MultiPolygon, Point, Polygon,
};
use geojson::{Geometry as GeoJsonGeometry, Value as GeoJsonValue};
use geozero::wkb;
use sqlx::types::time::Date;
use sqlx::PgPool;

#[derive(sqlx::FromRow)]
pub struct Roadwork {
    pub geom: GeoJsonGeometry,
    pub start_date: Date,
    pub end_date: Date,
    pub source: String,
}

impl Roadwork {
    /// Insère une géométrie déjà exprimée en Web Mercator (3857).
    pub async fn insert(&self, conn: &PgPool) {
        self.insert_with_srid(conn, 3857).await;
    }

    /// Insère une géométrie exprimée en WGS84 (4326) et la reprojette en 3857.
    pub async fn insert_wgs84(&self, conn: &PgPool) {
        self.insert_with_srid(conn, 4326).await;
    }

    async fn insert_with_srid(&self, conn: &PgPool, srid: i32) {
        let geo_geometry: GeoGeometry<f64> = match convert_geojson_to_geo(&self.geom) {
            Ok(geo) => geo,
            Err(e) => {
                println!("Error converting GeoJSON to Geo: {}", e);
                return;
            }
        };

        let geom_sql = if srid == 4326 {
            "ST_Transform(ST_SetSRID($1::geometry, 4326), 3857)"
        } else {
            "$1"
        };
        let query = format!(
            r#"
        INSERT INTO road_work (geom, start_date, end_date, source)
        VALUES ({}, $2, $3, $4)
        "#,
            geom_sql
        );

        match sqlx::query(&query)
            .bind(wkb::Encode(geo_geometry))
            .bind(&self.start_date)
            .bind(&self.end_date)
            .bind(&self.source)
            .execute(conn)
            .await
        {
            Ok(_) => (),
            Err(e) => {
                println!("Error inserting roadwork: {}", e);
            }
        }
    }

    /// Efface les travaux d'une seule source, sans toucher aux autres.
    pub async fn remove_source(conn: &PgPool, source: &str) {
        if let Err(e) = sqlx::query("DELETE FROM road_work WHERE source = $1")
            .bind(source)
            .execute(conn)
            .await
        {
            println!("Error removing roadworks for source {}: {}", source, e);
        }
    }
}

fn convert_geojson_to_geo(geometry: &GeoJsonGeometry) -> Result<GeoGeometry<f64>, &'static str> {
    match &geometry.value {
        GeoJsonValue::Point(coords) => Ok(GeoGeometry::Point(Point::new(coords[0], coords[1]))),
        GeoJsonValue::LineString(coords) => {
            let points: Vec<_> = coords.iter().map(|c| (c[0], c[1])).collect();
            Ok(GeoGeometry::LineString(LineString::from(points)))
        }
        GeoJsonValue::Polygon(coords) => {
            let rings: Vec<_> = coords
                .iter()
                .map(|ring| {
                    ring.iter()
                        .map(|c| (c[0], c[1]))
                        .collect::<Vec<(f64, f64)>>()
                })
                .collect();
            Ok(GeoGeometry::Polygon(Polygon::new(
                LineString::from(rings[0].clone()),
                rings[1..]
                    .iter()
                    .map(|r| LineString::from(r.clone()))
                    .collect(),
            )))
        }
        GeoJsonValue::MultiPolygon(polygons) => {
            let multipolygons: Vec<Polygon<f64>> = polygons
                .iter()
                .map(|coords| {
                    let rings: Vec<_> = coords
                        .iter()
                        .map(|ring| {
                            ring.iter()
                                .map(|c| (c[0], c[1]))
                                .collect::<Vec<(f64, f64)>>()
                        })
                        .collect();
                    Polygon::new(
                        LineString::from(rings[0].clone()),
                        rings[1..]
                            .iter()
                            .map(|r| LineString::from(r.clone()))
                            .collect(),
                    )
                })
                .collect();
            Ok(GeoGeometry::MultiPolygon(MultiPolygon::from(multipolygons)))
        }
        GeoJsonValue::MultiPoint(points) => {
            let points: Vec<Point<f64>> = points
                .iter()
                .map(|c| Point::new(c[0], c[1]))
                .collect();
            Ok(GeoGeometry::MultiPoint(MultiPoint::from(points)))
        }
        GeoJsonValue::MultiLineString(lines) => {
            let lines: Vec<LineString<f64>> = lines
                .iter()
                .map(|line| {
                    LineString::from(
                        line.iter()
                            .map(|c| (c[0], c[1]))
                            .collect::<Vec<(f64, f64)>>(),
                    )
                })
                .collect();
            Ok(GeoGeometry::MultiLineString(MultiLineString(lines)))
        }
        GeoJsonValue::GeometryCollection(_) => Err("Unsupported geometry type GeometryCollection"),
    }
}
