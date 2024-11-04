use chrono::{DateTime, Local};
use regex::Regex;
use sqlx::{Postgres, Row};
use uuid::Uuid;

use super::edge::Edge;

#[derive(sqlx::FromRow, Debug)]
pub struct CyclabilityScore {
    pub id: i32,
    pub name: Option<Vec<Option<String>>>,
    pub score: f64,
    pub comment: Option<String>,
    pub way_ids: Vec<i64>,
    pub created_at: DateTime<Local>,
    pub photo_path_thumbnail: Option<String>,
    pub geom: Vec<[f64; 2]>,
    pub user_id: Option<Uuid>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct CyclabilityScoreDb {
    pub id: i32,
    pub name: Option<Vec<Option<String>>>,
    pub score: f64,
    pub comment: Option<String>,
    pub way_ids: Vec<i64>,
    pub created_at: DateTime<Local>,
    pub photo_path_thumbnail: Option<String>,
    pub geom: String,
    pub user_id: Option<Uuid>,
}

impl CyclabilityScore {
    pub async fn get_recents(
        lng1: f64,
        lat1: f64,
        lng2: f64,
        lat2: f64,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<Vec<CyclabilityScore>, sqlx::Error> {
        let cs: Vec<CyclabilityScoreDb> = sqlx::query_as(
            r#"select DISTINCT ON (cs.created_at) cs.id, 
                        cs.name,
                        cs.score, 
                        cs.comment, 
                        cs.way_ids, 
                        cs.created_at, 
                        cs.photo_path, 
                        cs.photo_path_thumbnail,
                        ST_AsText(ST_Transform(geom, 4326)) as geom,
                        cs.user_id
               from cyclability_score cs
               where geom && ST_Transform(st_makeenvelope($1, $2, $3, $4, 4326), 3857)
               order by cs.created_at desc
               limit 100"#,
        )
        .bind(lng1)
        .bind(lat1)
        .bind(lng2)
        .bind(lat2)
        .fetch_all(conn)
        .await?;

        Ok(cs.iter().map(|c| c.into()).collect())
    }

    pub async fn get_history(
        way_ids: &Vec<i64>,
        conn: &sqlx::Pool<Postgres>,
    ) -> Vec<CyclabilityScore> {
        let cs: Vec<CyclabilityScoreDb> = match sqlx::query_as(
            r#"select id, 
                      name,
                      ST_AsText(ST_Transform(geom, 4326)) as geom,
                      score, 
                      comment, 
                      way_ids, 
                      created_at, 
                      photo_path, 
                      photo_path_thumbnail,
                      user_id
               from cyclability_score
               where way_ids = $1
               order by created_at desc
               limit 100"#,
        )
        .bind(way_ids)
        .fetch_all(conn)
        .await
        {
            Ok(cs) => cs,
            Err(e) => {
                eprintln!("Error while fetching history: {}", e);
                vec![]
            }
        };

        cs.iter().map(|c| c.into()).collect()
    }

    pub async fn get_by_id(
        id: i32,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<CyclabilityScore, sqlx::Error> {
        let cs: CyclabilityScoreDb = sqlx::query_as(
            r#"select id, 
                      s.name,
                      ST_AsText(ST_Transform(s.geom, 4326)) as geom, 
                      score, 
                      comment, 
                      way_ids, 
                      created_at, 
                      photo_path, 
                      photo_path_thumbnail,
                      user_id
               from cyclability_score s
               join cycleway_way on way_id = any(way_ids)
               where id = $1"#,
        )
        .bind(id)
        .fetch_one(conn)
        .await?;
        Ok(cs.into())
    }

    pub async fn get_by_way_ids(
        way_ids: &Vec<i64>,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<Vec<CyclabilityScore>, sqlx::Error> {
        let cs: Vec<CyclabilityScoreDb> = sqlx::query_as(
            r#"select id, 
                    s.name,
                    ST_AsText(ST_Transform(s.geom, 4326)) as geom, 
                    score, 
                    comment, 
                    way_ids, 
                    created_at, 
                    photo_path, 
                    photo_path_thumbnail,
                    user_id
               from cyclability_score s
               where way_ids = $1
               order by created_at desc"#,
        )
        .bind(way_ids)
        .fetch_all(conn)
        .await?;

        Ok(cs.iter().map(|c| c.into()).collect())
    }

    pub async fn get_photo_by_way_ids(way_ids: &Vec<i64>, conn: &sqlx::Pool<Postgres>) -> Vec<i32> {
        let result = sqlx::query(
            r#"select id
               from cyclability_score
               where way_ids && $1
               and photo_path_thumbnail is not null
               order by created_at desc"#,
        )
        .bind(way_ids)
        .fetch_all(conn)
        .await
        .unwrap();

        result.iter().map(|photo| photo.get(0)).collect()
    }

    pub async fn insert(
        score: &f64,
        comment: &Option<String>,
        way_ids: &Vec<i64>,
        photo_path: &Option<String>,
        photo_path_thumbnail: &Option<String>,
        user_id: Option<Uuid>,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<i32, sqlx::Error> {
        let id: i32 = sqlx::query(
            r#"INSERT INTO cyclability_score 
                    (way_ids, score, comment, photo_path, photo_path_thumbnail, name, geom, user_id) 
                    SELECT $1, $2, $3, $4, $5, array_agg(cw.name), ST_Union(cw.geom), $6
                    from cycleway_way cw
                    where cw.way_id = any($1)
                    group by $1, $2, $3, $4, $5, $6
                    RETURNING id"#,
        )
        .bind(way_ids)
        .bind(score)
        .bind(comment)
        .bind(&photo_path)
        .bind(&photo_path_thumbnail)
        .bind(&user_id)
        .fetch_one(conn)
        .await?
        .get(0);

        if let Some(photo_path) = photo_path {
            sqlx::query(
                r#"UPDATE cyclability_score 
                        SET photo_path = $1,
                        photo_path_thumbnail = $2
                        WHERE id = $3"#,
            )
            .bind(photo_path.replace("{}", id.to_string().as_str()))
            .bind(match photo_path_thumbnail {
                Some(p) => Some(p.replace("{}", id.to_string().as_str())),
                None => None,
            })
            .bind(id)
            .execute(conn)
            .await?;
        };

        let conn = conn.clone();
        tokio::spawn(async move {
            match sqlx::query(r#"REFRESH MATERIALIZED VIEW bike_path"#)
                .execute(&conn)
                .await
            {
                Ok(_) => (),
                Err(e) => eprintln!("Error while refreshing bike_path: {}", e),
            };
            match sqlx::query(r#"REFRESH MATERIALIZED VIEW CONCURRENTLY last_cycleway_score"#)
                .execute(&conn)
                .await
            {
                Ok(_) => (),
                Err(e) => eprintln!("Error while refreshing last_cycleway_score: {}", e),
            };
            match sqlx::query(r#"REFRESH MATERIALIZED VIEW CONCURRENTLY edge"#)
                .execute(&conn)
                .await
            {
                Ok(_) => (),
                Err(e) => eprintln!("Error while refreshing edge: {}", e),
            };
            Edge::clear_cache(&conn).await;
        });

        Ok(id)
    }
}

impl From<&CyclabilityScoreDb> for CyclabilityScore {
    fn from(response: &CyclabilityScoreDb) -> Self {
        let re = Regex::new(r"(-?\d+\.*\d*) (-?\d+\.*\d*)").unwrap();
        let points = re
            .captures_iter(response.geom.as_str())
            .map(|cap| {
                let x = cap[1].parse::<f64>().unwrap();
                let y = cap[2].parse::<f64>().unwrap();

                [x, y]
            })
            .collect::<Vec<[f64; 2]>>();
        CyclabilityScore {
            id: response.id,
            name: response.name.clone(),
            score: response.score,
            comment: response.comment.clone(),
            way_ids: response.way_ids.clone(),
            created_at: response.created_at,
            photo_path_thumbnail: response.photo_path_thumbnail.clone(),
            geom: points,
            user_id: response.user_id,
        }
    }
}

impl From<CyclabilityScoreDb> for CyclabilityScore {
    fn from(response: CyclabilityScoreDb) -> Self {
        CyclabilityScore::from(&response)
    }
}
