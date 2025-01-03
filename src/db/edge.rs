use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt::Debug,
    hash::Hash,
};

use crate::utils::h::H;
use crate::{db::utils::Score, utils::h::get_h_moyen};
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
pub enum Neighbour {
    EdgePoint(EdgePoint),
    Shortcut(Shortcut),
}

impl Neighbour {
    pub fn length(&self) -> f64 {
        match self {
            Neighbour::EdgePoint(edge_point) => edge_point.edge.length,
            Neighbour::Shortcut(shortcut) => shortcut.length,
        }
    }

    pub fn get_cost(&self, h: &Box<dyn H>) -> f64 {
        match self {
            Neighbour::EdgePoint(edge_point) => h.get_cost(edge_point),
            Neighbour::Shortcut(shortcut) => shortcut.cost,
        }
    }

    pub fn get_name(&self) -> String {
        match self {
            Neighbour::EdgePoint(edge_point) => edge_point
                .edge
                .tags
                .get("name")
                .unwrap_or(&"".to_string())
                .clone(),
            Neighbour::Shortcut(shortcut) => shortcut.name.clone(),
        }
    }

    pub fn get_points(&self) -> Vec<Point> {
        match self {
            Neighbour::EdgePoint(edge_point) => match edge_point.point {
                SourceOrTarget::Source => vec![Point {
                    lng: edge_point.edge.lon1,
                    lat: edge_point.edge.lat1,
                    way_id: edge_point.edge.way_id,
                    node_id: edge_point.edge.source,
                    length: edge_point.edge.length,
                }],
                SourceOrTarget::Target => vec![Point {
                    lng: edge_point.edge.lon2,
                    lat: edge_point.edge.lat2,
                    way_id: edge_point.edge.way_id,
                    node_id: edge_point.edge.target,
                    length: edge_point.edge.length,
                }],
            },
            Neighbour::Shortcut(shortcut) => shortcut.path.clone(),
        }
    }

    pub async fn get_neighbors(&self, conn: &sqlx::Pool<Postgres>) -> Vec<ARc<Neighbour>> {
        match self {
            Neighbour::EdgePoint(edge_point) => edge_point.get_neighbors(conn).await,
            Neighbour::Shortcut(shortcut) => shortcut.target.get_neighbors(conn).await,
        }
    }

    pub fn h(&self, h: &Box<dyn H>, end_edge: &Neighbour) -> f64 {
        let end_edge = match end_edge {
            Neighbour::EdgePoint(edge_point) => edge_point,
            Neighbour::Shortcut(shortcut) => &shortcut.target,
        };
        match self {
            Neighbour::EdgePoint(edge_point) => h.h(edge_point, end_edge),
            Neighbour::Shortcut(shortcut) => h.h(&shortcut.target, end_edge),
        }
    }

    pub fn get_begining_point(&self) -> ARc<EdgePoint> {
        match self {
            Neighbour::EdgePoint(edge_point) => ARc::new(edge_point.clone()),
            Neighbour::Shortcut(shortcut) => shortcut.source.clone(),
        }
    }

    pub fn get_ending_point(&self) -> ARc<EdgePoint> {
        match self {
            Neighbour::EdgePoint(edge_point) => ARc::new(EdgePoint {
                edge: edge_point.edge.clone(),
                cost: edge_point.cost,
                point: match edge_point.point {
                    SourceOrTarget::Source => SourceOrTarget::Target,
                    SourceOrTarget::Target => SourceOrTarget::Source,
                },
            }),
            Neighbour::Shortcut(shortcut) => shortcut.target.clone(),
        }
    }
}

impl PartialEq for Neighbour {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Neighbour::EdgePoint(edge_point1), Neighbour::EdgePoint(edge_point2)) => {
                edge_point1 == edge_point2
            }
            (Neighbour::Shortcut(shortcut1), Neighbour::Shortcut(shortcut2)) => {
                shortcut1 == shortcut2
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EdgePoint {
    pub edge: Edge,
    pub point: SourceOrTarget,
    pub cost: Option<f64>,
}

impl Hash for EdgePoint {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.edge.hash(state);
        self.point.hash(state);
    }
}

#[derive(Debug, Clone, Eq, Hash)]
pub enum SourceOrTarget {
    Source,
    Target,
}

#[derive(Debug, Clone)]
pub struct Shortcut {
    pub source: ARc<EdgePoint>,
    pub target: ARc<EdgePoint>,
    pub name: String,
    pub path: Vec<Point>,
    pub length: f64,
    pub cost: f64,
}

impl PartialEq for Shortcut {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.target == other.target
    }
}

impl Eq for Shortcut {}
impl Hash for Shortcut {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.source.hash(state);
        self.target.hash(state);
    }
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
        self.edge.id == other.edge.id && self.point == other.point
    }
}

impl Eq for EdgePoint {}

impl EdgePoint {
    pub fn get_node_id(&self) -> i64 {
        match self.point {
            SourceOrTarget::Source => self.edge.source,
            SourceOrTarget::Target => self.edge.target,
        }
    }

    pub async fn get_neighbors(&self, conn: &sqlx::Pool<Postgres>) -> Vec<ARc<Neighbour>> {
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
                let result: Vec<ARc<Neighbour>> = results
                    .into_iter()
                    .map(|edge: Edge| {
                        if edge.source == node_id {
                            return ARc::new(Neighbour::EdgePoint(EdgePoint {
                                cost: None,
                                edge,
                                point: SourceOrTarget::Target,
                            }));
                        } else {
                            return ARc::new(Neighbour::EdgePoint(EdgePoint {
                                cost: None,
                                edge,
                                point: SourceOrTarget::Source,
                            }));
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
    static ref NEIGHBORS_CACHE: Mutex<HashMap<i64, Vec<ARc<Neighbour>>>> =
        Mutex::new(HashMap::new());
}

impl Edge {
    pub async fn a_star_route(
        start_node_id: i64,
        end_node_id: i64,
        h: Box<dyn H>,
        conn: &sqlx::Pool<Postgres>,
    ) -> Vec<Point> {
        let end_edge = Edge::get(end_node_id, conn)
            .await
            .expect(format!("the end node should exist: {} ", end_node_id).as_str());
        let start_edge = Edge::get(start_node_id, conn)
            .await
            .expect(format!("the start node should exist: {} ", start_node_id).as_str());
        // open_set is the set of nodes to be evaluated
        let mut open_set = HashSet::new();
        let mut min_in_open_set = BTreeMap::new();
        open_set.insert(start_edge.clone());
        min_in_open_set.insert(Score(0.), start_edge.clone());
        let mut came_from: HashMap<ARc<Neighbour>, ARc<Neighbour>> = HashMap::new();
        // g_score is the shortest distance from the start node to the current node
        let mut g_score: HashMap<ARc<Neighbour>, f64> = HashMap::new();
        g_score.insert(start_edge.clone(), 0.0);
        // f_score is the shortest distance from the start node to the end node
        let mut f_score: HashMap<ARc<Neighbour>, f64> = HashMap::new();
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
            // if we are at the end, return the path
            if current == end_edge {
                return Edge::extract_end_path(end_edge, start_edge, came_from, g_score).await;
            }
            open_set.remove(&current);
            first_min_entry.remove();
            let neighbors = current.get_neighbors(conn).await;
            for neighbor in neighbors.iter() {
                let tentative_g_score = g_score.get(&current).expect("current should have a score")
                    + neighbor.length() * neighbor.get_cost(&h);
                let neighbord_g_score = g_score.get(neighbor);
                if neighbord_g_score.is_none() || &tentative_g_score < neighbord_g_score.unwrap() {
                    came_from.insert(neighbor.clone(), current.clone());
                    g_score.insert(neighbor.clone(), tentative_g_score);
                    f_score.insert(
                        neighbor.clone(),
                        tentative_g_score + neighbor.h(&h, &end_edge),
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

    async fn extract_end_path(
        end_edge: ARc<Neighbour>,
        start_edge: ARc<Neighbour>,
        came_from: HashMap<ARc<Neighbour>, ARc<Neighbour>>,
        g_score: HashMap<ARc<Neighbour>, f64>,
    ) -> Vec<Point> {
        let mut current = &end_edge;
        let mut path = vec![];
        let mut current_shortcut = Shortcut {
            source: start_edge.get_begining_point(),
            target: end_edge.get_ending_point(),
            name: current.get_name(),
            cost: *g_score.get(&end_edge).unwrap(),
            length: start_edge.length(),
            path: current.get_points(),
        };
        println!("Curent shortcut: {:?}", current_shortcut);
        while current != &start_edge {
            path.push(current);
            let next = came_from.get(current).unwrap();
            if let Neighbour::Shortcut(shortcut) = &**next {
                println!("Shortcut: {:?}", shortcut);
            }
            if current_shortcut.name == next.get_name() {
                current_shortcut.length += next.length();
                current_shortcut.path.extend(next.get_points());
                current_shortcut.cost = *g_score.get(next).unwrap();
                current_shortcut.target = next.get_ending_point();
            } else {
                // add shortcut to cash
                NEIGHBORS_CACHE.lock().await.insert(
                    current_shortcut.source.get_node_id(),
                    vec![ARc::new(Neighbour::Shortcut(current_shortcut.clone()))],
                );
                current_shortcut = Shortcut {
                    source: start_edge.get_begining_point(),
                    target: end_edge.get_ending_point(),
                    name: next.get_name(),
                    cost: *g_score.get(&end_edge).unwrap(),
                    length: start_edge.length(),
                    path: next.get_points(),
                };
            }
            current = next;
        }
        path.push(&start_edge);
        path.reverse();
        let points = path
            .iter()
            .map(|destination| {
                let points = destination.get_points();
                points
            })
            .flatten()
            .collect::<Vec<_>>();
        return points;
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
                    AND (tags->>'indoor' IS NULL OR tags->>'indoor' != 'yes')
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
    ) -> Result<ARc<Neighbour>, sqlx::Error> {
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
                rw.geom is not null as road_work
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
                Ok(ARc::new(Neighbour::EdgePoint(EdgePoint {
                    edge,
                    point,
                    cost: None,
                })))
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
                (26233313, 1870784004, "Montreal to Sherbrooke"),
                (26233313, 2352518821, "Montreal to Mont-Tremblant"),
                (26233313, 305805771, "Montreal to Trois-Rivières"),
                (305805771, 177522966, "Trois-Rivières to Québec"),
            ];

            for (source, target, description) in routes {
                println!("Calculating route: {}", description);
                Edge::a_star_route(source, target, get_h_moyen(), &conn).await;
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
        let points = Edge::a_star_route(321801851, 1764306722, get_h_moyen(), &conn).await;
        assert_eq!(321801851, points.first().unwrap().node_id);
        assert_eq!(1764306722, points.last().unwrap().node_id);
    }
}
