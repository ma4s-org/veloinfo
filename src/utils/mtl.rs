use futures::future::join_all;
use geojson::GeoJson;
use reqwest;
use sqlx::postgres::Postgres;
use std::fs::File;
use std::io::Write;
use std::process::Command;
use std::str::FromStr;
use time::Date;

use crate::db::road_work::{self, Roadwork};

pub struct SphericalMercator {
    zoom: u32,
    x: i32,
    y: i32,
}

pub async fn fetch_montreal_data(conn: &sqlx::Pool<Postgres>) {
    Roadwork::remove_all(conn).await;
    match std::fs::create_dir("tiles") {
        Ok(_) => {}
        Err(_) => {}
    }
    let x_range = 38611..38784;
    let y_range = 46786..46945;

    let sms: Vec<SphericalMercator> = x_range
        .flat_map(|x| {
            y_range
                .clone()
                .map(move |y| SphericalMercator { zoom: 17, x, y })
        })
        .collect();

    let tasks = sms
        .into_iter()
        .map(|sm| async move {
            read_tile(&sm, &conn).await;
        })
        .collect::<Vec<_>>();

    join_all(tasks).await;

    match std::fs::remove_dir_all("tiles") {
        Ok(_) => {}
        Err(e) => {
            println!("Error removing tiles directory: {}", e);
        }
    }
}

pub async fn read_tile(sm: &SphericalMercator, conn: &sqlx::Pool<Postgres>) {
    let response = match reqwest::get(
            format!("https://api.montreal.ca/api/it-platforms/geomatic/vector-tiles/maps/v1/entraves-polygonales/{}/{}/{}.pbf", 
            sm.zoom, 
            sm.x, 
            sm.y)).await {
        Ok(r) => r,
        Err(e) => {
            println!("Error fetching tile: {}", e);
            return;
    }};
    if response.status() == 404 {
        return;
    }
    let bytes = response.bytes().await.unwrap();
    if bytes.len() == 0 {
        return;
    }

    let mut file = File::create(format!("tiles/{}_{}_{}.pbf", sm.zoom, sm.x, sm.y)).unwrap();
    file.write_all(&bytes).unwrap();

    match Command::new("ogr2ogr")
        .arg("-f")
        .arg("GeoJSON")
        .arg(format!("tiles/{}_{}_{}.geojson", sm.zoom, sm.x, sm.y))
        .arg(format!("tiles/{}_{}_{}.pbf", sm.zoom, sm.x, sm.y))
        .output()
    {
        Ok(_) => {}
        Err(e) => {
            println!("Error executing ogr2ogr: {}", e);
        }
    };

    let geojson_path = format!("tiles/{}_{}_{}.geojson", sm.zoom, sm.x, sm.y);
    let geojson_str = match std::fs::read_to_string(&geojson_path) {
        Ok(s) => s,
        Err(e) => {
            println!("Error reading geojson file: {}", e);
            return;
        }
    };
    let geojson = match GeoJson::from_str(&geojson_str) {
        Ok(g) => g,
        Err(e) => {
            println!("Error parsing geojson: {}", e);
            return;
        }
    };

    if let GeoJson::FeatureCollection(features) = geojson {
        for feature in features {
            if let Some(geometry) = feature.geometry {
                let road_work = road_work::Roadwork {
                    geom: geometry,
                    start_date: match Date::from_calendar_date(2024, time::Month::April, 1) {
                        Ok(d) => d,
                        Err(e) => {
                            println!("Error creating date: {}", e);
                            return;
                        }
                    },
                    end_date: match Date::from_calendar_date(2024, time::Month::April, 1) {
                        Ok(d) => d,
                        Err(e) => {
                            println!("Error creating date: {}", e);
                            return;
                        }
                    },
                };
                road_work.insert(conn).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    #[tokio::test]
    async fn read_one_tile() {
        let conn = sqlx::Pool::connect(&env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();

        super::fetch_montreal_data(&conn).await;
    }
}
