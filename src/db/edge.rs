use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
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
    pub is_conditionally_closed: bool,
    pub in_bicycle_route: bool,
    pub snow: Option<bool>,
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
    pub edge: ARc<Edge>,
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
    pub fn reverse(&self) -> Self {
        let new_direction = match self.direction {
            SourceOrTarget::Source => SourceOrTarget::Target,
            SourceOrTarget::Target => SourceOrTarget::Source,
        };
        EdgePoint {
            edge: self.edge.clone(),
            direction: new_direction,
        }
    }

    pub fn get_node_id(&self) -> i64 {
        match self.direction {
            SourceOrTarget::Source => self.edge.source,
            SourceOrTarget::Target => self.edge.target,
        }
    }

    pub async fn get_neighbors(&self, conn: &sqlx::Pool<Postgres>) -> ARc<Vec<ARc<EdgePoint>>> {
        let node_id = self.get_node_id();
        if let Some(neighbors) = NEIGHBORS_CACHE.lock().await.get(node_id).await {
            return neighbors;
        }

        match sqlx::query_as(
            r#"SELECT
                e.id,
                e.source,
                e.target,
                x1 as lon1,
                y1 as lat1,
                x2 as lon2,
                y2 as lat2,
                e.tags,
                e.way_id,
                e.in_bicycle_route,
                e.is_conditionally_closed,
                e.tags->>'name' as name, 
                st_length(e.geom) as length,
                rw.geom is not null as road_work,
                cs.score,
                case when csnow.city_name is not null then true else false end as snow
            FROM edge e
                left join road_work rw on ST_Intersects(e.geom, rw.geom)
                left join last_cycleway_score cs on cs.way_id = e.way_id
                left join city_snow csnow on csnow.city_name = e.city_name
            WHERE (e.source = $1 or e.target = $1)
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
                                edge: ARc::new(edge),
                                direction: SourceOrTarget::Target,
                            });
                        } else {
                            return ARc::new(EdgePoint {
                                edge: ARc::new(edge),
                                direction: SourceOrTarget::Source,
                            });
                        }
                    })
                    .collect();
                let mut cache = NEIGHBORS_CACHE.lock().await;
                cache.insert(node_id, ARc::new(result)).await
            }
            Err(e) => {
                eprintln!("Error while getting neighbors: {}", e);
                return ARc::new(vec![]);
            }
        }
    }
}

struct EdgePointCache {
    cache: HashMap<i64, ARc<Vec<ARc<EdgePoint>>>>,
    keys: BTreeSet<i64>,
}

impl EdgePointCache {
    pub fn new() -> Self {
        EdgePointCache {
            cache: HashMap::new(),
            keys: BTreeSet::new(),
        }
    }

    pub async fn get(&self, node_id: i64) -> Option<ARc<Vec<ARc<EdgePoint>>>> {
        self.cache.get(&node_id).cloned()
    }

    pub async fn insert(
        &mut self,
        node_id: i64,
        neighbors: ARc<Vec<ARc<EdgePoint>>>,
    ) -> ARc<Vec<ARc<EdgePoint>>> {
        if self.cache.len() > 3_000_000 {
            // Limiter la taille du cache pour éviter une consommation excessive de mémoire
            if let Some(oldest_key) = self.keys.pop_first() {
                self.cache.remove(&oldest_key);
            }
        }
        self.keys.insert(node_id);
        self.cache.insert(node_id, neighbors.clone());
        neighbors
    }

    pub async fn clear(&mut self) {
        self.cache = HashMap::new();
        self.keys = BTreeSet::new();
    }

    pub async fn remove(&mut self, node_id: i64) {
        self.cache.remove(&node_id);
        self.keys.remove(&node_id);
    }
}

lazy_static! {
    static ref NEIGHBORS_CACHE: Mutex<EdgePointCache> = Mutex::new(EdgePointCache::new());
}

impl Edge {
    pub async fn a_star_bidirectional(
        start_node_id: i64,
        end_node_id: i64,
        h: Box<dyn H>,
        conn: &sqlx::Pool<Postgres>,
        mut socket: Option<&mut WebSocket>,
    ) -> Vec<Point> {
        let start_node = Edge::get(start_node_id, conn).await.unwrap();
        let end_node = Edge::get(end_node_id, conn).await.unwrap();

        // --- Structures pour la recherche AVANT (start -> end) ---
        let mut open_set_fwd = BTreeMap::new();
        let mut came_from_fwd: HashMap<ARc<EdgePoint>, ARc<EdgePoint>> = HashMap::new();
        let mut g_score_fwd: HashMap<ARc<EdgePoint>, f64> = HashMap::new();
        g_score_fwd.insert(start_node.clone(), 0.0);
        open_set_fwd.insert(Score(h.h(&start_node, &end_node)), start_node.clone());

        // --- Structures pour la recherche ARRIÈRE (end -> start) ---
        let mut open_set_bwd = BTreeMap::new();
        let mut came_from_bwd: HashMap<ARc<EdgePoint>, ARc<EdgePoint>> = HashMap::new();
        let mut g_score_bwd: HashMap<ARc<EdgePoint>, f64> = HashMap::new();
        g_score_bwd.insert(end_node.clone(), 0.0);
        open_set_bwd.insert(Score(h.h(&end_node, &start_node)), end_node.clone());

        // --- Variables pour la rencontre ---
        let mut meeting_point: Option<ARc<EdgePoint>> = None;
        let mut best_path_score = f64::INFINITY;

        let mut number_of_nodes = 0;

        while !open_set_fwd.is_empty() && !open_set_bwd.is_empty() {
            // Condition d'arrêt optimisée
            if let (Some((f_score_fwd, _)), Some((f_score_bwd, _))) = (
                open_set_fwd.first_key_value(),
                open_set_bwd.first_key_value(),
            ) {
                if f_score_fwd.0 + f_score_bwd.0 >= best_path_score {
                    break; // On a trouvé le chemin optimal
                }
            }

            number_of_nodes += 1;
            if number_of_nodes > h.get_max_point() {
                // Augmenté la limite pour être sûr
                break;
            }

            // --- Étape de la recherche AVANT ---
            if let Some((_, current_fwd)) = open_set_fwd.pop_first() {
                if let Some(ref mut s) = socket {
                    if number_of_nodes % 10 == 0 {
                        s.send(axum::extract::ws::Message::Text(
                            format!(
                                "[[{},{}],[{},{}]]",
                                current_fwd.edge.lon1,
                                current_fwd.edge.lat1,
                                current_fwd.edge.lon2,
                                current_fwd.edge.lat2
                            )
                            .into(),
                        ))
                        .await
                        .ok();
                    }
                }

                if g_score_bwd.contains_key(&current_fwd) {
                    let current_score = g_score_fwd[&current_fwd] + g_score_bwd[&current_fwd];
                    if current_score < best_path_score {
                        best_path_score = current_score;
                        meeting_point = Some(current_fwd.clone());
                    }
                }

                for neighbor in current_fwd.get_neighbors(conn).await.iter() {
                    let tentative_g_score =
                        g_score_fwd[&current_fwd] + neighbor.edge.length * h.get_cost(&neighbor);
                    if tentative_g_score < *g_score_fwd.get(neighbor).unwrap_or(&f64::INFINITY) {
                        came_from_fwd.insert(neighbor.clone(), current_fwd.clone());
                        g_score_fwd.insert(neighbor.clone(), tentative_g_score);
                        let f_score = tentative_g_score + h.h(&neighbor, &end_node);
                        open_set_fwd.insert(Score(f_score), neighbor.clone());
                    }
                }
            }

            // --- Étape de la recherche ARRIÈRE ---
            if let Some((_, current_bwd)) = open_set_bwd.pop_first() {
                if let Some(ref mut s) = socket {
                    if number_of_nodes % 10 == 0 {
                        s.send(axum::extract::ws::Message::Text(
                            format!(
                                "[[{},{}],[{},{}]]",
                                current_bwd.edge.lon1,
                                current_bwd.edge.lat1,
                                current_bwd.edge.lon2,
                                current_bwd.edge.lat2
                            )
                            .into(),
                        ))
                        .await
                        .ok();
                    }
                }

                if g_score_fwd.contains_key(&current_bwd) {
                    let current_score = g_score_fwd[&current_bwd] + g_score_bwd[&current_bwd];
                    if current_score < best_path_score {
                        best_path_score = current_score;
                        meeting_point = Some(current_bwd.clone());
                    }
                }

                for neighbor in current_bwd.get_neighbors(conn).await.iter() {
                    let tentative_g_score = g_score_bwd[&current_bwd]
                        + neighbor.edge.length * h.get_cost(&neighbor.reverse());
                    if tentative_g_score < *g_score_bwd.get(neighbor).unwrap_or(&f64::INFINITY) {
                        came_from_bwd.insert(neighbor.clone(), current_bwd.clone());
                        g_score_bwd.insert(neighbor.clone(), tentative_g_score);
                        let f_score = tentative_g_score + h.h(&neighbor, &start_node);
                        open_set_bwd.insert(Score(f_score), neighbor.clone());
                    }
                }
            }
        }

        if let Some(meeting_node) = meeting_point {
            // --- Reconstruction du chemin AVANT (start -> meeting_node) ---
            let mut path_fwd = vec![];
            let mut current_fwd = meeting_node.clone();
            while let Some(prev) = came_from_fwd.get(&current_fwd) {
                path_fwd.push(current_fwd.clone());
                current_fwd = prev.clone();
            }
            path_fwd.push(start_node);
            path_fwd.reverse();

            // --- Reconstruction du chemin ARRIÈRE (meeting_node -> end) ---
            let mut path_bwd = vec![];
            let mut current_bwd = meeting_node.clone();
            // On suit les "prédécesseurs" de la recherche arrière, qui sont en fait les "successeurs" sur le chemin final.
            while let Some(next_node) = came_from_bwd.get(&current_bwd) {
                // La recherche arrière stocke le chemin de end -> meeting.
                // Pour obtenir le chemin meeting -> end, nous devons l'inverser.
                // came_from_bwd[current] = predecessor_in_backward_search
                // Pour le chemin final, le segment est (predecessor, current)
                path_bwd.push(current_bwd.clone());
                current_bwd = next_node.clone();
                if current_bwd == end_node {
                    path_bwd.push(current_bwd.clone());
                    break;
                }
            }
            // Le chemin a été collecté dans l'ordre meeting -> end, donc pas besoin de reverse.

            // Combiner les deux chemins, en retirant le meeting_node dupliqué
            let mut path = path_fwd;
            path.extend(path_bwd.into_iter().skip(1));

            let promises = path
                .iter()
                .map(|edge| async {
                    match edge.direction {
                        SourceOrTarget::Source => Point {
                            lng: edge.edge.lon1,
                            lat: edge.edge.lat1,
                            way_id: edge.edge.way_id,
                            node_id: edge.edge.source,
                            length: edge.edge.length,
                        },
                        SourceOrTarget::Target => Point {
                            lng: edge.edge.lon2,
                            lat: edge.edge.lat2,
                            way_id: edge.edge.way_id,
                            node_id: edge.edge.target,
                            length: edge.edge.length,
                        },
                    }
                })
                .collect::<Vec<_>>();
            return join_all(promises).await;
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
                e.source,
                e.target,
                x1 as lon1,
                y1 as lat1,
                x2 as lon2,
                y2 as lat2,
                e.tags,
                e.way_id,
                e.tags->>'name' as name, 
                st_length(e.geom) as length,
                rw.geom is not null as road_work,
                e.is_conditionally_closed,
                in_bicycle_route,
                cs.score,
                case when csnow.city_name is not null then true else false end as snow
            FROM edge e
                left join road_work rw on ST_Intersects(e.geom, rw.geom)
                left join  last_cycleway_score cs on cs.way_id = e.way_id
                left join city_snow csnow on csnow.city_name = e.city_name
            WHERE e.source = $1 or e.target = $1"#,
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
                    edge: ARc::new(edge),
                    direction: point,
                }))
            }
            Err(e) => Err(e),
        }
    }

    pub async fn get_edge_by_city(
        city_name: &str,
        conn: &sqlx::Pool<Postgres>,
    ) -> Result<Vec<Edge>, sqlx::Error> {
        let edges: Vec<Edge> = sqlx::query_as(
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
                is_conditionally_closed,
                in_bicycle_route,
                snow
            FROM edge e
            left join road_work rw on ST_Intersects(e.geom, rw.geom)
            WHERE city_name = $1"#,
        )
        .bind(city_name)
        .fetch_all(conn)
        .await?;

        Ok(edges)
    }

    pub async fn clear_nodes_cache(node_ids: Vec<i64>, conn: &sqlx::Pool<Postgres>) {
        for node_id in node_ids {
            NEIGHBORS_CACHE.lock().await.remove(node_id).await;
        }
        let conn = conn.clone();
        tokio::spawn(async move {
            sqlx::query(r#"REFRESH MATERIALIZED VIEW last_cycleway_score"#)
                .execute(&conn)
                .await
                .unwrap();
        });
    }

    pub async fn clear_all_cache() {
        NEIGHBORS_CACHE.lock().await.clear().await;
    }

    pub async fn clear_cache_and_reload(conn: &sqlx::Pool<Postgres>) {
        Edge::clear_all_cache().await;
        let conn = conn.clone();
        tokio::spawn(async move {
            println!("Starting cache prefill...");

            let routes = vec![
                (235888032, 177522966, "Sainte-Anne-de-Bellevue to Quebec"),
                (2352518821, 1870784004, "Mont-Tremblant to Sherbrooke"),
            ];

            for (source, target, description) in routes {
                println!("Calculating route: {}", description);
                Edge::a_star_bidirectional(source, target, get_h_moyen(), &conn, None).await;
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
        let points =
            Edge::a_star_bidirectional(321801851, 1764306722, get_h_moyen(), &conn, None).await;
        assert_eq!(321801851, points.first().unwrap().node_id);
        assert_eq!(1764306722, points.last().unwrap().node_id);
    }
}
