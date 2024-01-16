use axum::{extract::Path, http::StatusCode, Json};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres};
use std::{borrow::Borrow, env};

#[derive(Debug, sqlx::FromRow)]
struct ResponseDb {
    geom: Option<String>,
    source: Option<i64>,
    target: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Segment {
    geom: Option<Vec<[f64; 2]>>,
    source: Option<i64>,
    target: Option<i64>,
}

impl Segment {
    async fn get(way_id: i64, conn: sqlx::Pool<Postgres>) -> Segment {
        let response: ResponseDb = sqlx::query_as(
            r#"select  
                source,
                target,
                ST_AsText(ST_Transform(geom, 4326)) as geom  
               from cycleway where way_id = $1"#,
        )
        .bind(way_id)
        .fetch_one(&conn)
        .await
        .unwrap();
        response.borrow().into()
    }

    async fn route(source: i64, target: i64, conn: sqlx::Pool<Postgres>) -> Segment {
        let responses: Vec<ResponseDb> = sqlx::query_as(
            r#"select $1 as source,
                        $2 as target, 
                        ST_AsText(ST_Transform(geom, 4326)) as geom 
                from pgr_bdastar(
                'select  way_id as id, 
                        source, 
                        target, 
                        st_length(geom) as cost, 
                        st_length(geom) as reverse_cost, 
                        st_x(st_startpoint(geom)) as x1,
                        st_y(st_startpoint(geom)) as y1,
                        st_x(st_endpoint(geom)) as x2,
                        st_y(st_endpoint(geom)) as y2
                FROM cycleway ', 
                $1, 
                $2
                ) as pa join cycleway c on pa.edge = c.way_id"#,
        )
        .bind(source)
        .bind(target)
        .fetch_all(&conn)
        .await
        .unwrap();
        responses.iter().fold(
            Segment {
                geom: Some(vec![]),
                source: Some(source),
                target: Some(target),
            },
            |mut acc, response| {
                let this_segement: Segment = response.into();
                acc.geom
                    .as_mut()
                    .unwrap()
                    .extend(this_segement.geom.unwrap());
                acc
            },
        )
    }
}

impl From<&ResponseDb> for Segment {
    fn from(response: &ResponseDb) -> Self {
        match response.geom.clone() {
            Some(str) => {
                let re = Regex::new(r"(-?\d+\.*\d*) (-?\d+\.*\d*)").unwrap();
                let points = re
                    .captures_iter(str.as_str())
                    .map(|cap| {
                        let x = cap[1].parse::<f64>().unwrap();
                        let y = cap[2].parse::<f64>().unwrap();

                        [x, y]
                    })
                    .collect::<Vec<[f64; 2]>>();
                Segment {
                    geom: Some(points),
                    source: response.source,
                    target: response.target,
                }
            }
            None => Segment {
                geom: None,
                source: None,
                target: None,
            },
        }
    }
}

pub async fn segment(
    Path(way_id): Path<i64>,
    Json(start_segment): Json<Segment>,
) -> Result<Json<Segment>, StatusCode> {
    let conn = PgPool::connect(format!("{}", env::var("DATABASE_URL").unwrap()).as_str())
        .await
        .unwrap();

    // We get the segment from the database
    let searched_segment: Segment = Segment::get(way_id, conn.clone()).await;

    if start_segment.geom.is_none() {
        Ok(Json(searched_segment))
    } else {
        let mut segments: Vec<Segment> = vec![];
        segments.push(
            Segment::route(
                start_segment.source.unwrap(),
                searched_segment.target.unwrap(),
                conn.clone(),
            )
            .await,
        );
        segments.push(
            Segment::route(
                start_segment.target.unwrap(),
                searched_segment.source.unwrap(),
                conn.clone(),
            )
            .await,
        );
        segments.push(
            Segment::route(
                start_segment.source.unwrap(),
                searched_segment.source.unwrap(),
                conn.clone(),
            )
            .await,
        );
        segments.push(
            Segment::route(
                start_segment.target.unwrap(),
                searched_segment.target.unwrap(),
                conn.clone(),
            )
            .await,
        );
        segments.iter().for_each(|segment| {
            println!("{:?}", segment);
        });
        let segment= segments
        .iter()
        .max_by(|x, y| {
            x.geom
                .as_ref()
                .unwrap()
                .len()
                .cmp(&y.geom.as_ref().unwrap().len())
        })
        .unwrap()
        .clone();
        println!("{:?}", segment);
        Ok(Json(
            segment
        ))
    }
}
