use chrono::{DateTime, Local};
use regex::Regex;
use sqlx::{Postgres, Row};
use uuid::Uuid;

#[derive(sqlx::FromRow, Debug)]
pub struct Report {
    pub id: i32,
    pub name: Option<Vec<Option<String>>>,
    pub score: f64,
    pub created_at: DateTime<Local>,
    pub photo_path_thumbnail: Option<String>,
    pub geom: Vec<Vec<[f64; 2]>>,
    pub user_id: Option<Uuid>,
    pub enabled: bool,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ReportDb {
    pub id: i32,
    pub name: Option<Vec<Option<String>>>,
    pub score: f64,
    pub created_at: DateTime<Local>,
    pub photo_path_thumbnail: Option<String>,
    pub geom: String,
    pub user_id: Option<Uuid>,
    pub enabled: bool,
}

impl Report {
    pub async fn get_recents(
        lng1: f64,
        lat1: f64,
        lng2: f64,
        lat2: f64,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<Vec<Report>, sqlx::Error> {
        let cs: Vec<ReportDb> = sqlx::query_as(
            r#"select r.id, 
                        r.name,
                        r.score, 
                        r.created_at, 
                        r.photo_path, 
                        r.photo_path_thumbnail,
                        ST_AsText(ST_Transform(geom, 4326)) as geom,
                        r.user_id,
                        r.enabled
               from report r
               where geom && ST_Transform(st_makeenvelope($1, $2, $3, $4, 4326), 3857)
               order by r.created_at desc
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

    #[allow(dead_code)]
    pub async fn get_history(
        geom: &str,
        conn: &sqlx::Pool<Postgres>,
    ) -> Vec<Report> {
        let cs: Vec<ReportDb> = match sqlx::query_as(
            r#"select id, 
                      name,
                      ST_AsText(ST_Transform(geom, 4326)) as geom,
                      score, 
                      created_at, 
                      photo_path, 
                      photo_path_thumbnail,
                      user_id,
                      enabled
               from report
               where ST_Equals(geom, ST_GeomFromText($1, 4326))
               order by created_at desc
               limit 100"#,
        )
        .bind(geom)
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
    ) -> Result<Report, sqlx::Error> {
        let cs: ReportDb = sqlx::query_as(
            r#"select id, 
                      r.name,
                      ST_AsText(geom) as geom, 
                      score, 
                      created_at, 
                      photo_path, 
                      photo_path_thumbnail,
                      user_id,
                      enabled
               from report r
               where id = $1"#,
        )
        .bind(id)
        .fetch_one(conn)
        .await?;
        Ok(cs.into())
    }

    pub async fn get_comment_by_report_id(
        report_id: i32,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<Option<(i32, String)>, sqlx::Error> {
        let row: Option<(i32, String)> = sqlx::query_as(
            r#"select id, comment
               from report_comment
               where report_id = $1
               and parent_comment_id is null
               order by created_at asc
               limit 1"#,
        )
        .bind(report_id)
        .fetch_optional(conn)
        .await?;

        Ok(row)
    }

    #[allow(dead_code)]
    pub async fn get_by_geom(
        geom_wkt: &str,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<Vec<Report>, sqlx::Error> {
        let cs: Vec<ReportDb> = sqlx::query_as(
            r#"select id, 
                    r.name,
                    ST_AsText(ST_Transform(r.geom, 4326)) as geom, 
                    score, 
                    created_at, 
                    photo_path, 
                    photo_path_thumbnail,
                    user_id,
                    enabled
               from report r
               where ST_Equals(r.geom, ST_GeomFromText($1, 4326))
               order by created_at desc"#,
        )
        .bind(geom_wkt)
        .fetch_all(conn)
        .await?;

        Ok(cs.iter().map(|c| c.into()).collect())
    }

    #[allow(dead_code)]
    pub async fn get_photo_by_geom(geom_wkt: &str, conn: &sqlx::Pool<Postgres>) -> Vec<i32> {
        let result = sqlx::query(
            r#"select id
               from report
               where ST_Intersects(geom, ST_GeomFromText($1, 4326))
               and photo_path_thumbnail is not null
               order by created_at desc"#,
        )
        .bind(geom_wkt)
        .fetch_all(conn)
        .await
        .unwrap();

        result.iter().map(|photo| photo.get(0)).collect()
    }

    pub async fn insert(
        score: &f64,
        geom_wkt: &str,
        photo_path: &Option<String>,
        photo_path_thumbnail: &Option<String>,
        user_id: Option<Uuid>,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<i32, sqlx::Error> {
        let id: i32 = sqlx::query(
            r#"INSERT INTO report
                    (score, photo_path, photo_path_thumbnail, geom, user_id)
                    VALUES ($1, $2, $3, ST_Transform(ST_GeomFromText($4, 4326), 3857), $5)
                    RETURNING id"#,
        )
        .bind(score)
        .bind(&photo_path)
        .bind(&photo_path_thumbnail)
        .bind(geom_wkt)
        .bind(&user_id)
        .fetch_one(conn)
        .await?
        .get(0);

        if let Some(photo_path) = photo_path {
            sqlx::query(
                r#"UPDATE report 
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

        Ok(id)
    }

    pub async fn insert_comment(
        report_id: i32,
        comment: &str,
        parent_comment_id: Option<i32>,
        user_name: &str,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<i32, sqlx::Error> {
        let id: i32 = sqlx::query(
            r#"INSERT INTO report_comment (report_id, comment, parent_comment_id, user_name)
               VALUES ($1, $2, $3, $4)
               RETURNING id"#,
        )
        .bind(report_id)
        .bind(comment)
        .bind(parent_comment_id)
        .bind(user_name)
        .fetch_one(conn)
        .await?
        .get(0);

        Ok(id)
    }

    pub async fn update_photo_paths(
        id: i32,
        photo_path: &Option<String>,
        photo_path_thumbnail: &Option<String>,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE report
               SET photo_path = $1,
                   photo_path_thumbnail = $2
               WHERE id = $3"#,
        )
        .bind(photo_path)
        .bind(photo_path_thumbnail)
        .bind(id)
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn set_enabled(
        id: i32,
        enabled: bool,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE report SET enabled = $1 WHERE id = $2")
            .bind(enabled)
            .bind(id)
            .execute(conn)
            .await?;
        Ok(())
    }

    /// Récupère les node_ids des edges qui intersectent le geom d'un report
    pub async fn get_intersecting_nodes(
        id: i32,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<Vec<i64>, sqlx::Error> {
        let rows: Vec<(i64,)> = sqlx::query_as(
            r#"SELECT DISTINCT e.source FROM edge e JOIN report r ON ST_Intersects(e.geom, r.geom) WHERE r.id = $1
               UNION
               SELECT DISTINCT e.target FROM edge e JOIN report r ON ST_Intersects(e.geom, r.geom) WHERE r.id = $1"#,
        )
        .bind(id)
        .fetch_all(conn)
        .await?;
        Ok(rows.into_iter().map(|(n,)| n).collect())
    }
}

impl From<&ReportDb> for Report {
    fn from(response: &ReportDb) -> Self {
        let re = Regex::new(r"(-?\d+\.\d+) (-?\d+\.\d+)").unwrap();
        let mut points = Vec::new();

        for line in response.geom.split("),(") {
            let line_points = re
                .captures_iter(line)
                .map(|cap| {
                    let lng = cap[1].parse::<f64>().unwrap();
                    let lat = cap[2].parse::<f64>().unwrap();
                    [lng, lat]  // GeoJSON expects [longitude, latitude]
                })
                .collect::<Vec<[f64; 2]>>();
            points.push(line_points);
        }

        Report {
            id: response.id,
            name: response.name.clone(),
            score: response.score,
            created_at: response.created_at,
            photo_path_thumbnail: response.photo_path_thumbnail.clone(),
            geom: points,
            user_id: response.user_id,
            enabled: response.enabled,
        }
    }
}
impl From<ReportDb> for Report {
    fn from(response: ReportDb) -> Self {
        Report::from(&response)
    }
}
