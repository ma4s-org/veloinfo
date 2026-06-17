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

    pub async fn get_by_report_id(
        _report_id: &i32,
        _conn: &sqlx::Pool<Postgres>,
    ) -> Result<Vec<Cycleway>> {
        // Pour les segments personnalisés (cercle/capsule), il n'y a pas de cycleway_way associé
        // Retourne un tableau vide
        Ok(vec![])
    }

    pub async fn find(
        lng: &f64,
        lat: &f64,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<Node, sqlx::Error> {
        let response: NodeDb = match sqlx::query_as::<_, NodeDb>(
            r#"WITH dumped AS (
                    SELECT (ST_DumpPoints(cw.geom)).geom as dp_geom,
                           cw.name,
                           cw.way_id,
                           unnest(cw.nodes) as nodes
                    FROM cycleway_way cw
                    JOIN edge e ON cw.way_id = e.way_id
                    WHERE ST_DWithin(cw.geom, ST_Transform(ST_SetSRID(ST_MakePoint($1, $2), 4326), 3857), 1000)
                    AND e.tags->>'highway' NOT IN ('motorway', 'trunk', 'motorway_link', 'trunk_link')
                )
                SELECT ST_AsText(dp_geom) as geom,
                       name,
                       way_id,
                       nodes as node_id,
                       ST_X(ST_Transform(dp_geom, 4326)) as lng,
                       ST_Y(ST_Transform(dp_geom, 4326)) as lat
                FROM dumped
                ORDER BY dp_geom <-> ST_Transform(ST_SetSRID(ST_MakePoint($1, $2), 4326), 3857)
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
