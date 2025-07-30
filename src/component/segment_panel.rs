use super::{
    info_panel::InfopanelContribution, score_circle::ScoreCircle, score_selector::ScoreSelector,
};
use crate::db::cycleway::{Cycleway, Node};
use crate::db::edge::Edge;
use crate::db::user::User;
use crate::utils::h::get_h_bigger_selection;
use crate::{db::cyclability_score::CyclabilityScore, VeloinfoState};
use askama::Template;
use askama_web::WebTemplate;
use axum::extract::multipart::Multipart;
use axum::extract::{Path, State};
use axum_extra::extract::cookie::Cookie;
use axum_extra::extract::CookieJar;
use futures::future::join_all;
use image::DynamicImage;
use lazy_static::lazy_static;
use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::Postgres;
use std::env;
use uuid::Uuid;

#[derive(Template, WebTemplate)]
#[template(path = "segment_panel.html", escape = "none")]
pub struct SegmentPanel {
    way_ids: String,
    score_circle: ScoreCircle,
    segment_name: String,
    score_selector: ScoreSelector,
    comment: String,
    edit: bool,
    history: Vec<InfopanelContribution>,
    photo_ids: Vec<i32>,
    geom_json: String,
    fit_bounds: bool,
    user_name: String,
    martin_url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostValue {
    pub score: f64,
    pub comment: String,
    pub way_ids: String,
    pub photo: Option<i64>,
}

lazy_static! {
    static ref RE_NUMBER: Regex = Regex::new(r"\d+").unwrap();
}

lazy_static! {
    static ref IMAGE_DIR: String = env::var("IMAGE_DIR").unwrap();
}

pub async fn segment_panel_post(
    State(state): State<VeloinfoState>,
    jar: CookieJar,
    mut multipart: Multipart,
) -> (CookieJar, SegmentPanel) {
    let user_id = match jar.get("uuid") {
        Some(uuid) => {
            let uuid = match Uuid::parse_str(uuid.value().to_string().as_str()) {
                Ok(uuid) => {
                    let user = User::get(&uuid, &state.conn).await;
                    if let None = user {
                        User::insert(&uuid, &"".to_string(), &state.conn).await;
                    }
                    Some(uuid)
                }
                Err(e) => {
                    eprintln!("Error while parsing uuid: {}", e);
                    None
                }
            };
            uuid
        }
        None => None,
    };

    let mut score = -1.;
    let mut comment = "".to_string();
    let mut way_ids = "".to_string();
    let mut photo = None;
    let mut user_name = "".to_string();
    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap();
        match name {
            "score" => {
                score = field
                    .text()
                    .await
                    .unwrap_or("0".to_string())
                    .parse::<f64>()
                    .unwrap()
            }
            "comment" => comment = field.text().await.unwrap_or("".to_string()),
            "way_ids" => way_ids = field.text().await.unwrap_or("".to_string()),
            "photo" => {
                photo = match field.bytes().await {
                    Ok(b) => Some(b),
                    Err(e) => {
                        println!("Error getting bytes {:?}", e);
                        None
                    }
                }
            }
            "user_name" => user_name = field.text().await.unwrap_or("".to_string()),
            _ => (),
        }
    }
    if let Some(user_id) = user_id {
        User::update(&user_id, &user_name, &state.conn).await;
    }
    let way_ids_i64 = RE_NUMBER
        .find_iter(way_ids.as_str())
        .map(|m| m.as_str().parse::<i64>().unwrap())
        .collect::<Vec<i64>>();

    let id = match CyclabilityScore::insert(
        &score,
        &Some(comment),
        &way_ids_i64,
        &match photo.as_ref() {
            Some(_photo) => Some(IMAGE_DIR.to_string() + "/{}.jpeg"),
            None => None,
        },
        &match photo.as_ref() {
            Some(_photo) => Some(IMAGE_DIR.to_string() + "/{}_thumbnail.jpeg"),
            None => None,
        },
        user_id,
        &state.conn,
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Error while inserting score: {}", e);
            return (
                jar,
                SegmentPanel {
                    way_ids: way_ids.clone(),
                    score_circle: ScoreCircle { score },
                    segment_name: "".to_string(),
                    score_selector: ScoreSelector::get_score_selector(score),
                    comment: "".to_string(),
                    edit: false,
                    history: vec![],
                    photo_ids: vec![],
                    geom_json: "".to_string(),
                    fit_bounds: false,
                    user_name,
                    martin_url: env::var("MARTIN_URL").unwrap(),
                },
            );
        }
    };
    if let Some(photo) = photo {
        let img = (|| -> Result<DynamicImage, Box<dyn std::error::Error>> {
            let img = match image::load_from_memory(&photo) {
                Ok(img) => img,
                Err(_) => {
                    let lib_heif = LibHeif::new();
                    let context = HeifContext::read_from_bytes(&photo)?;
                    let handle = context.primary_image_handle()?;
                    let decoded_image =
                        lib_heif.decode(&handle, ColorSpace::Rgb(RgbChroma::Rgb), None)?;
                    let rgb_data = decoded_image
                        .planes()
                        .interleaved
                        .ok_or("Error accessing image data")?
                        .data;
                    let width = decoded_image.width();
                    let height = decoded_image.height();
                    DynamicImage::ImageRgb8(
                        image::RgbImage::from_raw(width, height, rgb_data.into())
                            .ok_or("Error creating image RGB")?,
                    )
                }
            };
            Ok(img)
        })();
        match img {
            Ok(mut img) => {
                // Convert the image to RGB if it is in RGBA format
                if img.color().has_alpha() {
                    img = img.to_rgb8().into();
                }
                if let Err(e) =
                    img.save(IMAGE_DIR.to_string() + "/" + id.to_string().as_str() + ".jpeg")
                {
                    eprintln!("Error while saving image: {}", e);
                } else {
                    let img = img.resize(300, 300, image::imageops::FilterType::Lanczos3);
                    if let Err(e) = img.save(
                        IMAGE_DIR.to_string() + "/" + id.to_string().as_str() + "_thumbnail.jpeg",
                    ) {
                        eprintln!("Error while saving thumbnail: {}", e);
                    }
                }
            }
            Err(e) => eprintln!("Error while processing image: {}", e),
        }
    }

    (jar, segment_panel(state, way_ids).await)
}

pub async fn segment_panel_edit(
    State(state): State<VeloinfoState>,
    Path(way_ids): Path<String>,
    mut jar: CookieJar,
) -> (CookieJar, SegmentPanel) {
    let user_name = match jar.get("uuid") {
        Some(uuid) => {
            let uuid = match Uuid::parse_str(uuid.value().to_string().as_str()) {
                Ok(uuid) => uuid,
                Err(e) => {
                    eprintln!("Error while parsing uuid: {}", e);
                    let uuid = Uuid::now_v7();
                    jar = jar.add(
                        Cookie::build(("uuid", uuid.to_string()))
                            .path("/")
                            .permanent(),
                    );
                    uuid
                }
            };
            match User::get(&uuid, &state.conn).await {
                Some(user) => user.name,
                None => "".to_string(),
            }
        }
        None => {
            let uuid = Uuid::now_v7();
            jar = jar.add(
                Cookie::build(("uuid", uuid.to_string()))
                    .path("/")
                    .permanent(),
            );
            "".to_string()
        }
    };
    let way_ids_i64 = RE_NUMBER
        .find_iter(way_ids.as_str())
        .map(|m| m.as_str().parse::<i64>().unwrap())
        .collect::<Vec<i64>>();

    let ways = join_all(
        way_ids_i64
            .iter()
            .map(|way_id| async { Cycleway::get(way_id, &state.conn).await }),
    )
    .await;
    let cycleways: Vec<&Cycleway> = ways
        .iter()
        .filter(|r| r.is_ok())
        .map(|r| r.as_ref().unwrap())
        .collect();
    let all_same_score = cycleways.iter().all(|way| way.score == cycleways[0].score);
    let mut way = cycleways[0].clone();
    if !all_same_score {
        way.score = Some(-1.);
    }
    let segment_name = cycleways
        .iter()
        .fold("".to_string(), |acc, way| match way.name.as_ref() {
            Some(name) => {
                if acc.find(name) != None {
                    return acc;
                }
                format!("{} {}", acc, name)
            }
            None => acc,
        });

    let geom_json = cycleways.iter().fold(vec![], |acc, way| {
        acc.iter().chain(way.geom.iter()).cloned().collect()
    });
    let geom_json = serde_json::to_string(&geom_json).unwrap_or("".to_string());
    let photo_ids = CyclabilityScore::get_photo_by_way_ids(&way_ids_i64, &state.conn).await;
    let history = InfopanelContribution::get_history(&way_ids_i64, &state.conn).await;
    let segment_panel = SegmentPanel {
        way_ids: way_ids.clone(),
        score_circle: ScoreCircle {
            score: way.score.unwrap_or(-1.),
        },
        segment_name,
        score_selector: ScoreSelector::get_score_selector(way.score.unwrap_or(-1.)),
        comment: "".to_string(),
        edit: true,
        history,
        photo_ids,
        geom_json,
        fit_bounds: false,
        user_name,
        martin_url: env::var("MARTIN_URL").unwrap(),
    };

    (jar, segment_panel)
}

pub async fn segment_panel_get(
    State(state): State<VeloinfoState>,
    Path(way_ids): Path<String>,
) -> SegmentPanel {
    segment_panel(state, way_ids).await
}

pub async fn segment_panel(state: VeloinfoState, way_ids: String) -> SegmentPanel {
    println!("segment_panel {}", way_ids);
    let re = Regex::new(r"\d+").unwrap();
    let way_ids_i64 = re
        .find_iter(way_ids.as_str())
        .map(|cap| cap.as_str().parse::<i64>().unwrap())
        .collect::<Vec<i64>>();

    let cycleways: Vec<Cycleway> = join_all(way_ids_i64.iter().map(|way_id| async {
        match Cycleway::get(way_id, &state.conn).await {
            Ok(way) => way,
            Err(e) => {
                eprintln!("Error while fetching way: {}", e);
                Cycleway {
                    way_id: 0,
                    name: Some("".to_string()),
                    score: None,
                    geom: vec![],
                    source: 0,
                    target: 0,
                }
            }
        }
    }))
    .await;
    let all_same_score = cycleways.iter().all(|way| way.score == cycleways[0].score);
    let segment_name = cycleways
        .iter()
        .fold("".to_string(), |acc, way| match way.name.as_ref() {
            Some(name) => {
                if acc.find(name) != None {
                    return acc;
                }
                format!("{} {}", acc, name)
            }
            None => acc,
        });
    let geom: Vec<Vec<[f64; 2]>> = cycleways.iter().map(|way| way.geom.clone()).collect();

    let history = InfopanelContribution::get_history(&way_ids_i64, &state.conn).await;
    let photo_ids = CyclabilityScore::get_photo_by_way_ids(&way_ids_i64, &state.conn).await;
    SegmentPanel {
        way_ids: way_ids.clone(),
        score_circle: ScoreCircle {
            score: match cycleways.first() {
                Some(way) => way.score.unwrap_or(-1.),
                None => -1.,
            },
        },
        segment_name,
        score_selector: ScoreSelector::get_score_selector(if all_same_score {
            match cycleways.first() {
                Some(way) => way.score.unwrap_or(-1.),
                None => -1.,
            }
        } else {
            -1.
        }),
        comment: "".to_string(),
        edit: false,
        history,
        photo_ids,
        geom_json: serde_json::to_string(&geom).unwrap_or("".to_string()),
        fit_bounds: false,
        user_name: "".to_string(),
        martin_url: env::var("MARTIN_URL").unwrap(),
    }
}

pub async fn segment_panel_bigger_route(
    State(state): State<VeloinfoState>,
    Path((lng1, lat1, lng2, lat2)): Path<(f64, f64, f64, f64)>,
) -> SegmentPanel {
    let node1 = match Cycleway::find(&lng1, &lat1, &state.conn).await {
        Ok(node) => node,
        Err(e) => {
            eprintln!("Error while fetching node1: {}", e);
            Node {
                way_id: 0,
                geom: vec![],
                node_id: 0,
                lng: 0.,
                lat: 0.,
            }
        }
    };
    let node2 = match Cycleway::find(&lng2, &lat2, &state.conn).await {
        Ok(node) => node,
        Err(e) => {
            eprintln!("Error while fetching node2: {}", e);
            Node {
                way_id: 0,
                geom: vec![],
                node_id: 0,
                lng: 0.,
                lat: 0.,
            }
        }
    };

    let edges = Edge::a_star_route(
        node1.node_id,
        node2.node_id,
        get_h_bigger_selection(),
        &state.conn,
        None,
    )
    .await;

    let ways = edges.iter().fold("".to_string(), |acc, edge| {
        println!("edge {}", edge.way_id);
        match acc.contains(&edge.way_id.to_string()) {
            true => return acc,
            false => format!("{} {}", acc, edge.way_id),
        }
    });
    segment_panel(state, ways).await
}

async fn segment_panel_score_id(conn: &sqlx::Pool<Postgres>, id: i32, edit: bool) -> SegmentPanel {
    let score = match CyclabilityScore::get_by_id(id, &conn).await {
        Ok(score) => score,
        Err(e) => {
            eprintln!("Error while fetching score: {}", e);
            CyclabilityScore {
                id: 0,
                name: Some(vec![]),
                score: -1.,
                comment: None,
                way_ids: vec![],
                created_at: chrono::DateTime::from_timestamp(0, 0).unwrap().into(),
                photo_path_thumbnail: None,
                geom: vec![],
                user_id: None,
            }
        }
    };
    let (segment_name, way_ids) = join_all(score.way_ids.iter().map(|way_id| async {
        let conn = conn.clone();
        Cycleway::get(way_id, &conn).await
    }))
    .await
    .iter()
    .fold(("".to_string(), "".to_string()), |(names, ways), way| {
        let way = match way {
            Ok(way) => way,
            _ => return (names, ways),
        };
        match way.name.as_ref() {
            Some(name) => {
                if names.find(name) != None {
                    return (names, format!("{} {}", ways, way.way_id));
                }
                (
                    format!("{} {}", names, name),
                    format!("{} {}", ways, way.way_id),
                )
            }
            None => (names, format!("{} {}", ways, way.way_id)),
        }
    });

    let geom_json = match serde_json::to_string(&score.geom.clone()) {
        Ok(geom) => geom,
        Err(e) => {
            eprintln!("Error while serializing geom: {}", e);
            "".to_string()
        }
    };

    let history = InfopanelContribution::get_history(&score.way_ids, conn).await;
    let photo_ids = CyclabilityScore::get_photo_by_way_ids(&score.way_ids, &conn).await;

    SegmentPanel {
        way_ids,
        score_circle: ScoreCircle { score: score.score },
        segment_name,
        score_selector: ScoreSelector::get_score_selector(score.score),
        comment: score.comment.unwrap_or("".to_string()),
        edit,
        history,
        photo_ids,
        geom_json,
        fit_bounds: true,
        user_name: "".to_string(),
        martin_url: env::var("MARTIN_URL").unwrap(),
    }
}

pub async fn segment_panel_lng_lat(
    State(state): State<VeloinfoState>,
    Path((lng, lat)): Path<(f64, f64)>,
) -> SegmentPanel {
    let conn = state.clone().conn;
    let node: Node = match Cycleway::find(&lng, &lat, &conn).await {
        Ok(response) => response,
        Err(e) => {
            eprintln!("Error while fetching node: {}", e);
            Node {
                way_id: 0,
                geom: vec![],
                node_id: 0,
                lng: 0.,
                lat: 0.,
            }
        }
    };

    let way: Cycleway = match Cycleway::get(&node.way_id, &state.conn).await {
        Ok(way) => way,
        Err(e) => {
            eprintln!("Error while fetching way segment_panel_lng_lat: {}", e);
            Cycleway {
                way_id: 0,
                name: None,
                score: None,
                geom: vec![],
                source: 0,
                target: 0,
            }
        }
    };

    let segment_name = match way.name.as_ref() {
        Some(name) => name.clone(),
        None => "Inconnu".to_string(),
    };
    let history = InfopanelContribution::get_history_by_way_id(node.way_id, &state.conn).await;
    let photo_ids = CyclabilityScore::get_photo_by_way_ids(&vec![node.way_id], &state.conn).await;
    let info_panel = SegmentPanel {
        way_ids: node.way_id.to_string(),
        score_circle: ScoreCircle {
            score: way.score.unwrap_or(-1.),
        },
        segment_name,
        score_selector: ScoreSelector::get_score_selector(way.score.unwrap_or(-1.)),
        comment: "".to_string(),
        edit: false,
        history,
        photo_ids,
        geom_json: serde_json::to_string(&vec![node.geom]).unwrap_or("".to_string()),
        fit_bounds: false,
        user_name: "".to_string(),
        martin_url: env::var("MARTIN_URL").unwrap(),
    };

    info_panel
}

pub async fn select_score_id(
    State(state): State<VeloinfoState>,
    Path(id): Path<i32>,
) -> SegmentPanel {
    segment_panel_score_id(&state.conn, id, false).await
}
