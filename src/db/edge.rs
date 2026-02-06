use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt::Debug,
    hash::Hash,
};

use crate::utils::cost::H;
use crate::{db::utils::Score, utils::cost::get_h_moyen};
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
    pub snow: bool,
    pub elevation_start: Option<f64>,
    pub elevation_end: Option<f64>,
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

#[derive(Debug, Clone)]
pub struct EdgePoint {
    pub id: i64,
    pub lon1: f64,
    pub lat1: f64,
    pub lon2: f64,
    pub lat2: f64,
    pub length: f64,
    pub way_id: i64,
    pub source: i64,
    pub target: i64,
    pub in_bicycle_route: bool,
    pub road_work: bool,
    pub snow: bool,
    pub winter_service_no: bool,
    pub abandoned: bool,
    pub score: Option<f64>,
    pub direction: SourceOrTarget,
    pub cycleway: Option<Cycleway>,
    pub cycleway_left: Option<Cycleway>,
    pub cycleway_right: Option<Cycleway>,
    pub cycleway_both: Option<Cycleway>,
    pub highway: Option<Highway>,
    pub bicycle: Option<Bicycle>,
    pub surface: Option<Surface>,
    pub smoothness: Option<Smoothness>,
    pub access: Option<Access>,
    pub cyclestreet: bool,
    pub footway: Option<Footway>,
    pub tunnel: Option<Tunnel>,
    pub oneway: Option<Oneway>,
    pub oneway_bicycle: Option<Oneway>,
    pub cycleway_left_oneway: Option<Oneway>,
    pub cycleway_right_oneway: Option<Oneway>,
    pub informal: bool,
    pub routing_bicycle_use_sidepath: bool,
    pub elevation_start: Option<f64>,
    pub elevation_end: Option<f64>,
}

impl Eq for EdgePoint {}

impl Hash for EdgePoint {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.direction.hash(state);
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Oneway {
    Yes,
    No,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Cycleway {
    Track,
    Lane,
    Crossing,
    SharedLane,
    ShareBusway,
    Snow,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Highway {
    Cycleway,
    Unclassified,
    Footway,
    Residential,
    Service,
    Tertiary,
    Secondary,
    SecondaryLink,
    Primary,
    Trunk,
    Path,
    Steps,
    Pedestrian,
    Motorway,
    Proposed,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Bicycle {
    Yes,
    No,
    Designated,
    Dismount,
    Discouraged,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Surface {
    Sett,
    Cobblestone,
    Gravel,
    FineGravel,
    Chipseal,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Smoothness {
    Bad,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Access {
    Private,
    No,
    Customers,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Footway {
    Crossing,
    Sidewalk,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Tunnel {
    Yes,
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
        self.id == other.id && self.direction == other.direction
    }
}

impl From<(ARc<Edge>, SourceOrTarget)> for EdgePoint {
    fn from((edge, direction): (ARc<Edge>, SourceOrTarget)) -> Self {
        let tags = &edge.tags.0;

        let get = |k: &str| -> Option<&String> { tags.get(k) };

        let parse_oneway = |v: Option<&String>| -> Option<Oneway> {
            match v {
                Some(s) => match s.as_str() {
                    "yes" => Some(Oneway::Yes),
                    "no" => Some(Oneway::No),
                    _ => None,
                },
                None => None,
            }
        };

        let parse_cycleway = |v: Option<&String>| -> Option<Cycleway> {
            match v {
                Some(s) => match s.as_str() {
                    "track" => Some(Cycleway::Track),
                    "lane" => Some(Cycleway::Lane),
                    "crossing" => Some(Cycleway::Crossing),
                    "shared_lane" => Some(Cycleway::SharedLane),
                    "share_busway" => Some(Cycleway::ShareBusway),
                    _ => None,
                },
                None => None,
            }
        };

        let parse_highway = |v: Option<&String>| -> Option<Highway> {
            match v {
                Some(s) => match s.as_str() {
                    "cycleway" => Some(Highway::Cycleway),
                    "unclassified" => Some(Highway::Unclassified),
                    "footway" => Some(Highway::Footway),
                    "residential" => Some(Highway::Residential),
                    "service" => Some(Highway::Service),
                    "tertiary" => Some(Highway::Tertiary),
                    "secondary" => Some(Highway::Secondary),
                    "secondary_link" => Some(Highway::SecondaryLink),
                    "primary" => Some(Highway::Primary),
                    "trunk" => Some(Highway::Trunk),
                    "path" => Some(Highway::Path),
                    "steps" => Some(Highway::Steps),
                    "pedestrian" => Some(Highway::Pedestrian),
                    "motorway" => Some(Highway::Motorway),
                    "proposed" => Some(Highway::Proposed),
                    _ => None,
                },
                None => None,
            }
        };

        let parse_bicycle = |v: Option<&String>| -> Option<Bicycle> {
            match v {
                Some(s) => match s.as_str() {
                    "yes" => Some(Bicycle::Yes),
                    "no" => Some(Bicycle::No),
                    "designated" => Some(Bicycle::Designated),
                    "dismount" => Some(Bicycle::Dismount),
                    "discouraged" => Some(Bicycle::Discouraged),
                    _ => None,
                },
                None => None,
            }
        };

        let parse_surface = |v: Option<&String>| -> Option<Surface> {
            match v {
                Some(s) => match s.as_str() {
                    "sett" => Some(Surface::Sett),
                    "cobblestone" => Some(Surface::Cobblestone),
                    "gravel" => Some(Surface::Gravel),
                    "fine_gravel" => Some(Surface::FineGravel),
                    "chipseal" => Some(Surface::Chipseal),
                    _ => None,
                },
                None => None,
            }
        };

        let parse_smoothness = |v: Option<&String>| -> Option<Smoothness> {
            match v {
                Some(s) => match s.as_str() {
                    "bad" => Some(Smoothness::Bad),
                    _ => None,
                },
                None => None,
            }
        };

        let parse_access = |v: Option<&String>| -> Option<Access> {
            match v {
                Some(s) => match s.as_str() {
                    "private" => Some(Access::Private),
                    "no" => Some(Access::No),
                    "customers" => Some(Access::Customers),
                    _ => None,
                },
                None => None,
            }
        };

        let parse_footway = |v: Option<&String>| -> Option<Footway> {
            match v {
                Some(s) => match s.as_str() {
                    "crossing" => Some(Footway::Crossing),
                    "sidewalk" => Some(Footway::Sidewalk),
                    _ => None,
                },
                None => None,
            }
        };

        let parse_tunnel = |v: Option<&String>| -> Option<Tunnel> {
            match v {
                Some(s) => match s.as_str() {
                    "yes" => Some(Tunnel::Yes),
                    _ => None,
                },
                None => None,
            }
        };

        let normalize = |s: &str| -> String {
            s.replace(' ', "")
                .replace('(', "")
                .replace(')', "")
                .to_lowercase()
        };

        let cycleway_conditional_no_snow = match get("cycleway:conditional") {
            Some(s) => {
                let cleaned = normalize(s);
                cleaned.contains("no") && cleaned.contains("@snow")
            }
            None => false,
        };

        let cycleway_left_conditional_no_snow = match get("cycleway:left:conditional") {
            Some(s) => {
                let cleaned = normalize(s);
                cleaned.contains("no") && cleaned.contains("@snow")
            }
            None => false,
        };

        let cycleway_right_conditional_no_snow = match get("cycleway:right:conditional") {
            Some(s) => {
                let cleaned = normalize(s);
                cleaned.contains("no") && cleaned.contains("@snow")
            }
            None => false,
        };

        let ep = EdgePoint {
            id: edge.id,
            lon1: edge.lon1,
            lat1: edge.lat1,
            lon2: edge.lon2,
            lat2: edge.lat2,
            length: edge.length,
            way_id: edge.way_id,
            source: edge.source,
            target: edge.target,
            in_bicycle_route: edge.in_bicycle_route,
            road_work: edge.road_work,
            snow: edge.snow,
            score: edge.score,
            direction,
            cycleway: if edge.snow && cycleway_conditional_no_snow {
                Some(Cycleway::Snow)
            } else {
                parse_cycleway(get("cycleway"))
            },
            cycleway_left: if edge.snow && cycleway_left_conditional_no_snow {
                Some(Cycleway::Snow)
            } else {
                parse_cycleway(get("cycleway:left"))
            },
            cycleway_right: if edge.snow && cycleway_right_conditional_no_snow {
                Some(Cycleway::Snow)
            } else {
                parse_cycleway(get("cycleway:right"))
            },
            cycleway_both: parse_cycleway(get("cycleway:both")),
            highway: parse_highway(get("highway")),
            bicycle: parse_bicycle(get("bicycle")),
            surface: parse_surface(get("surface")).or_else(|| parse_surface(get("suface"))),
            smoothness: parse_smoothness(get("smoothness")),
            access: parse_access(get("access")),
            cyclestreet: get("cyclestreet") == Some(&"yes".to_string()),
            footway: parse_footway(get("footway")),
            tunnel: parse_tunnel(get("tunnel")),
            oneway: parse_oneway(get("oneway")),
            oneway_bicycle: parse_oneway(get("oneway:bicycle")),
            cycleway_left_oneway: parse_oneway(get("cycleway:left:oneway")),
            cycleway_right_oneway: parse_oneway(get("cycleway:right:oneway")),
            informal: get("informal") == Some(&"yes".to_string()),
            routing_bicycle_use_sidepath: get("routing:bicycle")
                == Some(&"use_sidepath".to_string()),
            winter_service_no: get("winter_service") == Some(&"no".to_string()),
            abandoned: get("abandoned") == Some(&"yes".to_string()),
            elevation_start: edge.elevation_start,
            elevation_end: edge.elevation_end,
        };

        ep
    }
}

impl EdgePoint {
    pub fn reverse(&self) -> Self {
        let new_direction = match self.direction {
            SourceOrTarget::Source => SourceOrTarget::Target,
            SourceOrTarget::Target => SourceOrTarget::Source,
        };
        let mut e = self.clone();
        e.direction = new_direction;
        e
    }

    pub fn get_node_id(&self) -> i64 {
        match self.direction {
            SourceOrTarget::Source => self.source,
            SourceOrTarget::Target => self.target,
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
                e.tags->>'name' as name, 
                st_length(e.geom) as length,
                rw.geom is not null as road_work,
                cs.score,
                case when csnow.city_name is not null then true else false end as snow,
                e.elevation_start,
                e.elevation_end
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
                        let direction = if edge.source == node_id {
                            SourceOrTarget::Target
                        } else {
                            SourceOrTarget::Source
                        };
                        ARc::new((ARc::new(edge), direction).into())
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
                                current_fwd.lon1,
                                current_fwd.lat1,
                                current_fwd.lon2,
                                current_fwd.lat2
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
                        g_score_fwd[&current_fwd] + neighbor.length * h.get_cost(&neighbor);
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
                                current_bwd.lon1,
                                current_bwd.lat1,
                                current_bwd.lon2,
                                current_bwd.lat2
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
                        + neighbor.length * h.get_cost(&neighbor.reverse());
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
                            lng: edge.lon1,
                            lat: edge.lat1,
                            way_id: edge.way_id,
                            node_id: edge.source,
                            length: edge.length,
                        },
                        SourceOrTarget::Target => Point {
                            lng: edge.lon2,
                            lat: edge.lat2,
                            way_id: edge.way_id,
                            node_id: edge.target,
                            length: edge.length,
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
                in_bicycle_route,
                cs.score,
                case when csnow.city_name is not null then true else false end as snow,
                e.elevation_start,
                e.elevation_end
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
                Ok(ARc::new((ARc::new(edge), point).into()))
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
                in_bicycle_route,
                cs.score,
                case when csnow.city_name is not null then true else false end as snow,
                e.elevation_start,
                e.elevation_end
            FROM edge e
            left join road_work rw on ST_Intersects(e.geom, rw.geom)
            left join last_cycleway_score cs on cs.way_id = e.way_id
            left join city_snow csnow on csnow.city_name = e.city_name
            WHERE e.city_name = $1"#,
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
            sqlx::query(r#"REFRESH MATERIALIZED VIEW CONCURRENTLY last_cycleway_score"#)
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
                (1419279436, 177522966, "Montréal to Quebec"),
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

    use crate::utils::cost::get_h_moyen;

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
