use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::Postgres;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Cycleway {
    pub name: Option<String>,
    pub way_id: i64,
    pub geom: Vec<[f64; 2]>,
    pub source: i64,
    pub target: i64,
    pub score: Option<f64>,
}

#[derive(Debug, sqlx::FromRow)]
struct CyclewayDb {
    name: Option<String>,
    way_id: i64,
    geom: String,
    source: i64,
    target: i64,
    score: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct NodeDb {
    pub way_id: i64,
    pub geom: String,
    pub node_id: i64,
    pub lng: f64,
    pub lat: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Node {
    pub way_id: i64,
    pub geom: Vec<[f64; 2]>,
    pub node_id: i64,
    pub lng: f64,
    pub lat: f64,
}

impl Eq for Node {}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.node_id == other.node_id
    }
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize, Clone)]
pub struct RouteDB {
    seq: i32,
    path_seq: i32,
    node: i64,
    edge: i64,
    cost: f64,
    agg_cost: f64,
    pub x1: f64,
    pub y1: f64,
    pub way_id: i64,
}

impl Cycleway {
    pub async fn get(way_id: &i64, conn: &sqlx::Pool<Postgres>) -> Result<Cycleway> {
        let response: Result<CyclewayDb, sqlx::Error> = sqlx::query_as(
            r#"select
                    c.name,  
                    c.way_id,
                    c.source,
                    c.target,
                    ST_AsText(ST_Transform(c.geom, 4326)) as geom,  
                    cs.score
                from cycleway_way c 
                left join last_cycleway_score cs on c.way_id = cs.way_id
                where 
                c.way_id = $1"#,
        )
        .bind(way_id)
        .fetch_one(conn)
        .await;

        match response {
            Ok(response) => Ok(response.into()),
            Err(e) => {
                eprintln!("Error getting cycleway {:?} {:?}", way_id, e);
                Err(e.into())
            }
        }
    }

    pub async fn get_by_score_id(
        score_id: &i32,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<Vec<Cycleway>> {
        let responses: Vec<CyclewayDb> = sqlx::query_as(
            r#"select
                c.name,  
                c.way_id,
                c.source,
                c.target,
                ST_AsText(ST_Transform(c.geom, 4326)) as geom,  
                score,
                cs.id as score_id
               from cycleway_way c
               join cyclability_score cs on c.way_id = any(cs.way_ids)
               where cs.id = $1
               "#,
        )
        .bind(score_id)
        .fetch_all(conn)
        .await?;
        Ok(responses.iter().map(|response| response.into()).collect())
    }

    pub async fn find(
        lng: &f64,
        lat: &f64,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<Node, sqlx::Error> {
        let response: NodeDb = match sqlx::query_as(
            r#"        
            SELECT
                way_id,
                geom,
                name,
                nodes as node_id,
                ST_X(st_transform((dp).geom, 4326)) as lng,
                ST_Y(st_transform((dp).geom, 4326)) as lat
            FROM (  
                SELECT (ST_DumpPoints(geom)) as dp, 
                        name,
                        way_id,
                        ST_AsText(ST_Transform(geom, 4326)) as geom,
                        unnest(nodes) as nodes
                FROM cycleway_way
                WHERE ST_DWithin(geom, ST_Transform(ST_SetSRID(ST_MakePoint($1, $2), 4326), 3857), 1000)
            ) as subquery
            ORDER BY (dp).geom <-> ST_Transform(ST_SetSRID(ST_MakePoint($1, $2), 4326), 3857)
            LIMIT 1"#,
        )
        .bind(lng)
        .bind(lat)
        .fetch_one(conn)
        .await
        {
            Ok(response) => response,
            Err(e) => return Err(e),
        };
        Ok((&response).into())
    }
}

impl From<CyclewayDb> for Cycleway {
    fn from(response: CyclewayDb) -> Self {
        Cycleway::from(&response)
    }
}

impl From<&CyclewayDb> for Cycleway {
    fn from(response: &CyclewayDb) -> Self {
        let re = Regex::new(r"(-?\d+\.*\d*) (-?\d+\.*\d*)").unwrap();
        let points = re
            .captures_iter(response.geom.as_str())
            .map(|cap| {
                let x = cap[1].parse::<f64>().unwrap();
                let y = cap[2].parse::<f64>().unwrap();

                [x, y]
            })
            .collect::<Vec<[f64; 2]>>();
        Cycleway {
            name: response.name.clone(),
            way_id: response.way_id,
            geom: points,
            source: response.source,
            target: response.target,
            score: response.score,
        }
    }
}

impl From<&NodeDb> for Node {
    fn from(response: &NodeDb) -> Self {
        let re = Regex::new(r"(-?\d+\.*\d*) (-?\d+\.*\d*)").unwrap();
        let points = re
            .captures_iter(response.geom.as_str())
            .map(|cap| {
                let x = cap[1].parse::<f64>().unwrap();
                let y = cap[2].parse::<f64>().unwrap();

                [x, y]
            })
            .collect::<Vec<[f64; 2]>>();
        Node {
            node_id: response.node_id,
            way_id: response.way_id,
            geom: points,
            lng: response.lng,
            lat: response.lat,
        }
    }
}
