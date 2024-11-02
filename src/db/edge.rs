use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt::Debug,
};

use crate::db::utils::Score;
use crate::utils::h::H;
use futures::future::join_all;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use sqlx::Postgres;
use std::sync::Arc as ARc;
use tokio::sync::Mutex;

use super::cycleway::{Node, NodeDb};

lazy_static! {
    pub static ref HOST: String = std::env::var("HOST").unwrap();
    pub static ref USER: String = std::env::var("USER").unwrap();
    pub static ref PASSWORD: String = std::env::var("PASSWORD").unwrap();
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize, Clone)]
pub struct Point {
    pub lng: f64,
    pub lat: f64,
    pub way_id: i64,
    pub node_id: i64,
    pub length: f64,
}

impl std::fmt::Display for Point {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{},{}]", self.lng, self.lat)
    }
}

#[derive(Debug, sqlx::FromRow, Clone)]
pub struct Edge {
    pub source: i64,
    pub target: i64,
    pub lon1: f64,
    pub lat1: f64,
    pub lon2: f64,
    pub lat2: f64,
    pub score: Option<f64>,
    pub way_id: i64,
    pub length: f64,
    pub tags: sqlx::types::Json<HashMap<String, String>>,
    pub road_work: bool,
}

lazy_static! {
    pub static ref NEIGHBORS_CACHE: Mutex<HashMap<i64, ARc<Vec<Edge>>>> =
        Mutex::new(HashMap::new());
}

impl Edge {
    pub async fn fast_route(
        start_node_id: i64,
        end_node_id: i64,
        h: Box<dyn H>,
        conn: &sqlx::Pool<Postgres>,
    ) -> Vec<Point> {
        let end_node = Edge::get(end_node_id, conn)
            .await
            .expect(format!("the end node should exist: {} ", end_node_id).as_str());
        // open_set is the set of nodes to be evaluated
        let mut open_set = HashSet::new();
        let mut min_in_open_set = BTreeMap::new();
        open_set.insert(start_node_id);
        min_in_open_set.insert(Score(0.), start_node_id);
        let mut came_from: HashMap<i64, i64> = HashMap::new();
        // g_score is the shortest distance from the start node to the current node
        let mut g_score: HashMap<i64, f64> = HashMap::new();
        g_score.insert(start_node_id, 0.0);
        // f_score is the shortest distance from the start node to the end node
        let mut f_score: HashMap<i64, f64> = HashMap::new();
        f_score.insert(start_node_id, 0.0);

        // a* algorithm
        while !open_set.is_empty() {
            let first_min_entry = min_in_open_set
                .first_entry()
                .expect("open set should not be empty");
            let current = first_min_entry.get().clone();
            // if we are at the end, return the path
            if current == end_node_id {
                let mut current = end_node_id;
                let mut path = vec![];
                while current != start_node_id {
                    path.push(current);
                    current = *came_from.get(&current).unwrap();
                }
                path.push(start_node_id);
                path.reverse();
                let promises = path
                    .iter()
                    .map(|edge_id| async {
                        let edge = Edge::get(*edge_id, conn).await.unwrap();
                        if *edge_id == edge.source {
                            return Point {
                                lng: edge.lon1,
                                lat: edge.lat1,
                                way_id: edge.way_id,
                                node_id: edge.source,
                                length: edge.length,
                            };
                        } else {
                            return Point {
                                lng: edge.lon2,
                                lat: edge.lat2,
                                way_id: edge.way_id,
                                node_id: edge.target,
                                length: edge.length,
                            };
                        }
                    })
                    .collect::<Vec<_>>();
                let points = join_all(promises).await;
                return points;
            }
            open_set.remove(&current);
            first_min_entry.remove();
            let neighbors = Edge::get_neighbors(current, conn).await;
            for neighbor in neighbors.iter() {
                let neighbor_id = if current == neighbor.source {
                    neighbor.target
                } else {
                    neighbor.source
                };
                let tentative_g_score = g_score.get(&current).expect("current should have a score")
                    + neighbor.length * h.get_cost(neighbor, neighbor_id);
                let neignbourd_g_score = g_score.get(&neighbor_id);
                if neignbourd_g_score.is_none() || &tentative_g_score < neignbourd_g_score.unwrap()
                {
                    came_from.insert(neighbor_id, current);
                    g_score.insert(neighbor_id, tentative_g_score);
                    f_score.insert(
                        neighbor_id,
                        tentative_g_score + h.h(&neighbor, neighbor_id, &end_node, end_node_id),
                    );
                    if !open_set.contains(&neighbor_id) {
                        open_set.insert(neighbor_id);
                        min_in_open_set
                            .insert(Score(*f_score.get(&neighbor_id).unwrap()), neighbor_id);
                    }
                }
            }
        }

        vec![]
    }

    pub async fn find_closest_node(
        lng: &f64,
        lat: &f64,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<Node, sqlx::Error> {
        let distance: NodeDb = match sqlx::query_as(
            r#"        
            SELECT
                way_id,
                ST_AsText(ST_Transform(st_centroid(geom), 4326)) as geom,
                node_id,
                ST_X(st_transform(st_centroid(geom), 4326)) as lng,
                ST_Y(st_transform(st_centroid(geom), 4326)) as lat
            FROM (  
                SELECT 
                        way_id,
                        source as node_id,
                        geom
                FROM edge e
	            WHERE 
                    ST_DWithin(geom, ST_Transform(ST_SetSRID(ST_MakePoint($1, $2), 4326), 3857), 1000) and
                    tags->>'highway' != 'footway' and
                    cost_road < 20
            ) as subquery
            ORDER BY geom <-> ST_Transform(ST_SetSRID(ST_MakePoint($1, $2), 4326), 3857)
            LIMIT 1"#,
        )
        .bind(lng)
        .bind(lat)
        .fetch_one(conn)
        .await
        {
            Ok(distance) => distance,
            Err(e) => return Err(e),
        };
        Ok(distance.into())
    }

    pub async fn get(edge_id: i64, conn: &sqlx::Pool<Postgres>) -> Result<Edge, sqlx::Error> {
        sqlx::query_as(
            r#"SELECT
                source,
                target,
                e.id as edge,
                score,
                x1 as lon1,
                y1 as lat1,
                x2 as lon2,
                y2 as lat2,
                tags,
                way_id,
                tags->>'name' as name, 
                st_length(e.geom) as length,
                rw.geom is not null as road_work
            FROM edge e
            left join road_work rw on ST_Intersects(e.geom, rw.geom)
            WHERE source = $1 or target = $1"#,
        )
        .bind(edge_id)
        .fetch_one(conn)
        .await
    }

    pub async fn get_neighbors(edge_id: i64, conn: &sqlx::Pool<Postgres>) -> ARc<Vec<Edge>> {
        let mut cache = NEIGHBORS_CACHE.lock().await;
        if cache.contains_key(&edge_id) {
            return cache.get(&edge_id).unwrap().clone();
        }

        match sqlx::query_as(
            r#"SELECT
                source,
                target,
                e.id as edge,
                score,
                x1 as lon1,
                y1 as lat1,
                x2 as lon2,
                y2 as lat2,
                tags,
                way_id,
                tags->>'name' as name, 
                st_length(e.geom) as length,
                rw.geom is not null as road_work
            FROM edge e
            left join road_work rw on ST_Intersects(e.geom, rw.geom)
            WHERE (source = $1 or target = $1)"#,
        )
        .bind(edge_id)
        .fetch_all(conn)
        .await
        {
            Ok(distance) => {
                let distance = ARc::new(distance);
                cache.insert(edge_id, distance.clone());
                distance
            }
            Err(e) => {
                eprintln!("Error while getting neighbors: {}", e);
                ARc::new(vec![])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::h::HMoyen;

    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_fast_route() {
        let conn = sqlx::Pool::connect(&env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();
        let points = Edge::fast_route(321801851, 1764306722, Box::new(HMoyen {}), &conn).await;
        assert_eq!(321801851, points.first().unwrap().node_id);
        assert_eq!(1764306722, points.last().unwrap().node_id);
    }
}
