use geo::{Geometry as GeoGeometry, LineString, MultiPolygon, Point, Polygon};
use geojson::{Geometry as GeoJsonGeometry, Value as GeoJsonValue};
use geozero::wkb;
use sqlx::types::time::Date;
use sqlx::PgPool;

#[derive(sqlx::FromRow)]
pub struct Roadwork {
    pub geom: GeoJsonGeometry,
    pub start_date: Date,
    pub end_date: Date,
}

impl Roadwork {
    pub async fn insert(&self, conn: &PgPool) {
        let geo_geometry: GeoGeometry<f64> = match convert_geojson_to_geo(&self.geom) {
            Ok(geo) => geo,
            Err(e) => {
                println!("Error converting GeoJSON to Geo: {}", e);
                return;
            }
        };

        match sqlx::query(
            r#"
        INSERT INTO road_work (geom, start_date, end_date)
        VALUES ($1, $2, $3)
        "#,
        )
        .bind(wkb::Encode(geo_geometry))
        .bind(&self.start_date)
        .bind(&self.end_date)
        .execute(conn)
        .await
        {
            Ok(_) => (),
            Err(e) => {
                println!("Error inserting roadwork: {}", e);
            }
        }
    }

    pub async fn remove_all(conn: &PgPool) {
        sqlx::query("DELETE FROM road_work")
            .execute(conn)
            .await
            .unwrap();
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
        GeoJsonValue::MultiPoint(_) => Err("Unsupported geometry type MultiPoint"),
        GeoJsonValue::MultiLineString(_) => Err("Unsupported geometry type MultiLineString"),
        GeoJsonValue::GeometryCollection(_) => Err("Unsupported geometry type GeometryCollection"),
    }
}
