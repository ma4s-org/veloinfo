use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt::Debug,
    hash::Hash,
};

use crate::utils::h::H;
use crate::{db::utils::Score, utils::h::get_h_moyen};
use axum::extract::ws::WebSocket;
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
    pub id: i64,
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
    pub in_bicycle_route: bool,
}

impl Eq for Edge {}
impl Hash for Edge {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialEq for Edge {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Debug, Clone, Eq, Hash)]
pub struct EdgePoint {
    pub edge: Edge,
    pub direction: SourceOrTarget,
}

#[derive(Debug, Clone, Eq, Hash)]
pub enum SourceOrTarget {
    Source,
    Target,
}

impl PartialEq for SourceOrTarget {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SourceOrTarget::Source, SourceOrTarget::Source) => true,
            (SourceOrTarget::Target, SourceOrTarget::Target) => true,
            _ => false,
        }
    }
}

impl PartialEq for EdgePoint {
    fn eq(&self, other: &Self) -> bool {
        self.edge.id == other.edge.id && self.direction == other.direction
    }
}

impl EdgePoint {
    pub fn get_node_id(&self) -> i64 {
        match self.direction {
            SourceOrTarget::Source => self.edge.source,
            SourceOrTarget::Target => self.edge.target,
        }
    }

    pub async fn get_neighbors(&self, conn: &sqlx::Pool<Postgres>) -> Vec<ARc<EdgePoint>> {
        let mut cache = NEIGHBORS_CACHE.lock().await;
        let node_id = self.get_node_id();
        if cache.contains_key(&node_id) {
            return cache.get(&node_id).unwrap().clone();
        }

        match sqlx::query_as(
            r#"SELECT
                e.id,
                source,
                target,
                score,
                x1 as lon1,
                y1 as lat1,
                x2 as lon2,
                y2 as lat2,
                tags,
                way_id,
                in_bicycle_route,
                tags->>'name' as name, 
                st_length(e.geom) as length,
                rw.geom is not null as road_work
            FROM edge e
            left join road_work rw on ST_Intersects(e.geom, rw.geom)
            WHERE (source = $1 or target = $1)
            "#,
        )
        .bind(node_id)
        .fetch_all(conn)
        .await
        {
            Ok(results) => {
                let result: Vec<ARc<EdgePoint>> = results
                    .into_iter()
                    .map(|edge: Edge| {
                        if edge.source == node_id {
                            return ARc::new(EdgePoint {
                                edge,
                                direction: SourceOrTarget::Target,
                            });
                        } else {
                            return ARc::new(EdgePoint {
                                edge,
                                direction: SourceOrTarget::Source,
                            });
                        }
                    })
                    .collect();
                cache.insert(node_id, result.clone());
                result
            }
            Err(e) => {
                eprintln!("Error while getting neighbors: {}", e);
                return vec![];
            }
        }
    }
}

lazy_static! {
    static ref NEIGHBORS_CACHE: Mutex<HashMap<i64, Vec<ARc<EdgePoint>>>> =
        Mutex::new(HashMap::new());
}

impl Edge {
    pub async fn a_star_route(
        start_node_id: i64,
        end_node_id: i64,
        h: Box<dyn H>,
        conn: &sqlx::Pool<Postgres>,
        mut socket: Option<&mut WebSocket>,
    ) -> Vec<Point> {
        let end_edge = Edge::get(end_node_id, conn)
            .await
            .expect(format!("the end node should exist: {} ", end_node_id).as_str());
        let start_edge = Edge::get(start_node_id, conn)
            .await
            .expect(format!("the start node should exist: {} ", start_node_id).as_str());
        // open_set is the set of nodes to be evaluated
        let mut open_set = HashSet::new();
        let mut revisited_map = HashMap::new();

        let mut min_in_open_set = BTreeMap::new();
        open_set.insert(start_edge.clone());
        min_in_open_set.insert(Score(0.), start_edge.clone());
        let mut came_from: HashMap<ARc<EdgePoint>, ARc<EdgePoint>> = HashMap::new();
        // g_score is the shortest distance from the start node to the current node
        let mut g_score: HashMap<ARc<EdgePoint>, f64> = HashMap::new();
        g_score.insert(start_edge.clone(), 0.0);
        // f_score is the shortest distance from the start node to the end node
        let mut f_score: HashMap<ARc<EdgePoint>, f64> = HashMap::new();
        f_score.insert(start_edge.clone(), 0.0);

        // a* algorithm
        let mut number_of_nodes = 0;
        while !open_set.is_empty() {
            number_of_nodes += 1;
            if number_of_nodes > h.get_max_point() {
                break;
            }
            let first_min_entry = min_in_open_set
                .first_entry()
                .expect("open set should not be empty");
            let current = first_min_entry.get().clone();
            open_set.remove(&current);
            first_min_entry.remove();
            revisited_map.insert(
                current.clone(),
                match revisited_map.get(&current.clone()) {
                    Some(entry) => entry + 1,
                    None => 1,
                },
            );

            if let Some(ref mut socket) = socket {
                // Send the current edge to the client every 3 iterations to not overload the client
                if revisited_map.len() % 3 == 0 {
                    socket
                        .send(axum::extract::ws::Message::Text(
                            format!(
                                "[[{},{}],[{},{}]]",
                                current.edge.lon1,
                                current.edge.lat1,
                                current.edge.lon2,
                                current.edge.lat2
                            )
                            .into(),
                        ))
                        .await
                        .unwrap();
                }
            }
            // if we are at the end, return the path
            if current == end_edge {
                let mut current = &end_edge;
                let mut path = vec![];
                while current != &start_edge {
                    path.push(current);
                    current = came_from.get(current).unwrap();
                }
                path.push(&start_edge);
                path.reverse();
                let promises = path
                    .iter()
                    .map(|edge| async {
                        match edge.direction {
                            SourceOrTarget::Source => {
                                return Point {
                                    lng: edge.edge.lon1,
                                    lat: edge.edge.lat1,
                                    way_id: edge.edge.way_id,
                                    node_id: edge.edge.source,
                                    length: edge.edge.length,
                                }
                            }
                            SourceOrTarget::Target => {
                                return Point {
                                    lng: edge.edge.lon2,
                                    lat: edge.edge.lat2,
                                    way_id: edge.edge.way_id,
                                    node_id: edge.edge.target,
                                    length: edge.edge.length,
                                }
                            }
                        }
                    })
                    .collect::<Vec<_>>();
                let points = join_all(promises).await;
                return points;
            }
            let neighbors = current.get_neighbors(conn).await;
            for neighbor in neighbors.iter() {
                let tentative_g_score = g_score.get(&current).expect("current should have a score")
                    + neighbor.edge.length * h.get_cost(neighbor);
                let neighbord_g_score = g_score.get(neighbor);
                if neighbord_g_score.is_none() || &tentative_g_score < neighbord_g_score.unwrap() {
                    if let Some(revisited) = revisited_map.get(neighbor) {
                        if *revisited > 3 {
                            // On empêche de revisiter trop souvent le même voisin.
                            // Sinon ça bloque dans les coins (ex: allé à Oka de Montréal)
                            continue;
                        }
                    }
                    came_from.insert(neighbor.clone(), current.clone());
                    g_score.insert(neighbor.clone(), tentative_g_score);
                    f_score.insert(
                        neighbor.clone(),
                        tentative_g_score + h.h(&neighbor, &end_edge),
                    );
                    if !open_set.contains(neighbor) {
                        open_set.insert(neighbor.clone());
                        min_in_open_set
                            .insert(Score(*f_score.get(neighbor).unwrap()), neighbor.clone());
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
                    ST_DWithin(geom, ST_Transform(ST_SetSRID(ST_MakePoint($1, $2), 4326), 3857), 1000) 
                    AND tags->>'highway' is not null
                    AND (tags->>'highway' != 'footway' or
                            (tags->>'highway' = 'footway' AND tags->>'bicycle' IN ('yes', 'designated', 'dismount')))
                    AND (tags->>'highway' != 'track')
                    AND (tags->>'highway' != 'path')
                    AND (tags->>'highway' != 'steps')
                    AND (tags->>'highway' != 'pedestrian' or 
                            (tags->>'highway' = 'pedestrian' AND tags->>'bicycle' IN ('yes', 'designated', 'dismount')))
                    AND (tags->>'highway' != 'motorway')
                    AND (tags->>'footway' IS NULL OR tags->>'footway' != 'sidewalk')
                    AND (tags->>'indoor' IS NULL OR tags->>'indoor' != 'yes')
                    AND (tags->>'access' IS NULL or tags->>'access'  in ('customers'))
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
        Ok((&distance).into())
    }

    pub async fn get(
        node_id: i64,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<ARc<EdgePoint>, sqlx::Error> {
        let edge: Result<Edge, _> = sqlx::query_as(
            r#"SELECT
                e.id,
                source,
                target,
                score,
                x1 as lon1,
                y1 as lat1,
                x2 as lon2,
                y2 as lat2,
                tags,
                way_id,
                tags->>'name' as name, 
                st_length(e.geom) as length,
                rw.geom is not null as road_work,
                in_bicycle_route
            FROM edge e
            left join road_work rw on ST_Intersects(e.geom, rw.geom)
            WHERE source = $1 or target = $1"#,
        )
        .bind(node_id)
        .fetch_one(conn)
        .await;

        match edge {
            Ok(edge) => {
                let point = if edge.source == node_id {
                    SourceOrTarget::Source
                } else {
                    SourceOrTarget::Target
                };
                Ok(ARc::new(EdgePoint {
                    edge,
                    direction: point,
                }))
            }
            Err(e) => Err(e),
        }
    }

    pub async fn clear_nodes_cache(node_ids: Vec<i64>) {
        let mut cache = NEIGHBORS_CACHE.lock().await;
        for node_id in node_ids {
            cache.remove(&node_id);
        }
    }

    pub async fn clear_cache(conn: &sqlx::Pool<Postgres>) {
        // Tenter d'acquérir le verrou avec timeout
        println!("Attempting to acquire cache lock...");
        match tokio::time::timeout(std::time::Duration::from_secs(5), NEIGHBORS_CACHE.lock()).await
        {
            Ok(mut guard) => {
                println!("Cache lock acquired, clearing...");
                guard.clear();
                println!("Cache cleared");
            }
            Err(_) => {
                println!("Failed to acquire cache lock after 5 seconds");
                return;
            }
        };

        // Remplir le cache de manière synchrone
        let conn = conn.clone();
        tokio::spawn(async move {
            println!("Starting cache prefill...");

            let routes = vec![
                (235888032, 177522966, "Sainte-Anne-de-Bellevue to Quebec"),
                (268157240, 177522966, "Alma to Quebec"),
                (1477879177, 177522966, "Matane to Quebec"),
                (26233313, 1870784004, "Montreal to Sherbrooke"),
                (26233313, 2352518821, "Montreal to Mont-Tremblant"),
                (26233313, 555491818, "Montreal to Saint-Anicet"),
                (26233313, 10926929438, "Montreal to Saint-Hyacinthe"),
            ];

            for (source, target, description) in routes {
                println!("Calculating route: {}", description);
                Edge::a_star_route(source, target, get_h_moyen(), &conn, None).await;
            }

            println!("Cache prefill complete");
        });
    }
}

#[cfg(test)]
mod tests {

    use crate::utils::h::get_h_moyen;

    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_fast_route() {
        let conn = sqlx::Pool::connect(&env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();
        let points = Edge::a_star_route(321801851, 1764306722, get_h_moyen(), &conn, None).await;
        assert_eq!(321801851, points.first().unwrap().node_id);
        assert_eq!(1764306722, points.last().unwrap().node_id);
    }
}
