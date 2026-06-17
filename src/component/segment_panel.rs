use super::{info_panel::InfopanelContribution, score_selector::ScoreSelector};
use crate::db::report::Report;
use crate::db::report_comment::ReportComment;
use crate::db::cycleway::{Cycleway, Node};
use crate::db::user::User;
use crate::VeloinfoState;
use axum::body::Body;
use axum::extract::multipart::Multipart;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum_extra::extract::CookieJar;
use geojson::JsonValue;
use image::DynamicImage;
use kamadak_exif::{In, Reader, Tag};
use lazy_static::lazy_static;
use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};
use serde_json::json;
use sqlx::Row;
use std::env;
use uuid::Uuid;

lazy_static! {
    static ref IMAGE_DIR: String = env::var("IMAGE_DIR").unwrap();
}

/// Convertit un GeoJSON (String) en WKT pour PostGIS
fn geojson_to_wkt(geojson: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(geojson)
        .map_err(|e| format!("Erreur parsing GeoJSON: {}", e))?;
    
    let geom_type = value["type"].as_str().ok_or("Type de géométrie manquant")?;
    let coords = &value["coordinates"];
    
    match geom_type {
        "Polygon" => {
            // Polygon: [[[lng,lat],...],...] - on prend le premier anneau (outer ring)
            let rings = coords.as_array().ok_or("Coordinates invalides")?;
            if rings.is_empty() {
                return Err("Polygon vide".to_string());
            }
            let outer_ring = rings[0].as_array().ok_or("Outer ring invalide")?;
            let points: Result<Vec<String>, String> = outer_ring.iter().map(|coord| {
                let arr = coord.as_array().ok_or("Coord invalide")?;
                let lng = arr[0].as_f64().ok_or("Lng invalide")?;
                let lat = arr[1].as_f64().ok_or("Lat invalide")?;
                Ok::<String, String>(format!("{} {}", lng, lat))
            }).collect();
            let points = points?;
            Ok(format!("POLYGON(({}))", points.join(", ")))
        }
        _ => Err(format!("Type de géométrie non supporté: {}", geom_type))
    }
}

pub async fn segment_panel_post(
    State(state): State<VeloinfoState>,
    jar: CookieJar,
    mut multipart: Multipart,
) -> (CookieJar, Json<JsonValue>) {
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
    let mut geom_json = "".to_string();
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
            "geom_json" => geom_json = field.text().await.unwrap_or("".to_string()),
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

    // Convertir GeoJSON en WKT pour PostGIS
    let geom_wkt = match geojson_to_wkt(&geom_json) {
        Ok(wkt) => wkt,
        Err(e) => {
            eprintln!("Erreur conversion GeoJSON -> WKT: {}", e);
            return (
                jar,
                Json(json!({
                    "way_ids": "".to_string(),
                    "score_circle": { "score": score },
                    "segment_name": "".to_string(),
                    "score_selector": ScoreSelector::get_score_selector(score),
                    "comment": "".to_string(),
                    "edit": false,
                    "history": Vec::<InfopanelContribution>::new(),
                    "photo_ids": Vec::<i32>::new(),
                    "geom_json": "".to_string(),
                    "fit_bounds": false,
                    "user_name": user_name,
                    "martin_url": format!("{}/martin", env::var("VELOINFO_URL").unwrap()),
                })),
            );
        }
    };

    let id = match Report::insert(
        &score,
        &geom_wkt,
        &None,
        &None,
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
                Json(json!({
                    "way_ids": "".to_string(),
                    "score_circle": { "score": score },
                    "segment_name": "".to_string(),
                    "score_selector": ScoreSelector::get_score_selector(score),
                    "comment": "".to_string(),
                    "edit": false,
                    "history": Vec::<InfopanelContribution>::new(),
                    "photo_ids": Vec::<i32>::new(),
                    "geom_json": "".to_string(),
                    "fit_bounds": false,
                    "user_name": user_name,
                    "martin_url": format!("{}/martin", env::var("VELOINFO_URL").unwrap()),
                })),
            );
        }
    };

    // Insert comment separately if provided
    if !comment.is_empty() {
        if let Err(e) = Report::insert_comment(id, &comment, None, &user_name, &state.conn).await {
            eprintln!("Error while inserting comment: {}", e);
        }
    }
    // Traiter la photo seulement si elle a du contenu (pas vide)
    if let Some(photo) = photo.as_ref() {
        if photo.is_empty() {
        } else {
            let img = (|| -> Result<DynamicImage, Box<dyn std::error::Error>> {
            // Try to read EXIF orientation first
            let exif_reader = Reader::new();
            let exif = exif_reader
                .read_from_container(&mut std::io::Cursor::new(&photo))
                .ok();

            let mut img = match image::load_from_memory(&photo) {
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

            // Apply EXIF orientation if available
            if let Some(exif_data) = exif {
                if let Some(orientation) = exif_data.get_field(Tag::Orientation, In::PRIMARY) {
                    if let Some(value) = orientation.value.get_uint(0) {
                        // Values: 1=normal, 3=180°, 6=90° clockwise, 8=270° clockwise
                        match value {
                            3 => img = img.rotate180(),
                            6 => img = img.rotate90(),
                            8 => img = img.rotate270(),
                            _ => (), // No rotation needed
                        }
                    }
                }
            }

            Ok(img)
        })();
        match img {
            Ok(mut img) => {
                // Convert the image to RGB if it is in RGBA format
                if img.color().has_alpha() {
                    img = img.to_rgb8().into();
                }
                let full_path = IMAGE_DIR.to_string() + "/" + id.to_string().as_str() + ".jpeg";
                if let Err(e) = img.save(&full_path) {
                    eprintln!("Error while saving image: {}", e);
                } else {
                    let img = img.resize(300, 300, image::imageops::FilterType::Lanczos3);
                    let thumb_path =
                        IMAGE_DIR.to_string() + "/" + id.to_string().as_str() + "_thumbnail.jpeg";
                    if let Err(e) = img.save(&thumb_path) {
                        eprintln!("Error while saving thumbnail: {}", e);
                    }
                }
            }
            Err(e) => eprintln!("Error while processing image: {}", e),
        }
        } // Fermer le else de photo.is_empty()
    }

    // Update photo paths after successful save
    let photo_path = match photo {
        Some(_) => Some(IMAGE_DIR.to_string() + "/" + id.to_string().as_str() + ".jpeg"),
        None => None,
    };

    let photo_path_thumbnail = match photo {
        Some(_) => Some(IMAGE_DIR.to_string() + "/" + id.to_string().as_str() + "_thumbnail.jpeg"),
        None => None,
    };

    // Update the record with photo paths
    if let Err(e) =
        Report::update_photo_paths(id, &photo_path, &photo_path_thumbnail, &state.conn)
            .await
    {
        eprintln!("Error updating photo paths: {}", e);
    }

    // Retourner le segment avec la géométrie sauvegardée
    let geom_json_for_response = geom_json.clone();
    (jar, Json(json!({
        "way_ids": "".to_string(),
        "score_circle": { "score": score },
        "segment_name": "Segment sélectionné".to_string(),
        "score_selector": ScoreSelector::get_score_selector(score),
        "comment": comment,
        "edit": true,
        "history": Vec::<InfopanelContribution>::new(),
        "photo_ids": Vec::<i32>::new(),
        "geom_json": geom_json_for_response,
        "fit_bounds": false,
        "user_name": user_name,
        "martin_url": format!("{}/martin", env::var("VELOINFO_URL").unwrap()),
    })))
}

pub async fn segment_panel_edit_post(
    State(_state): State<VeloinfoState>,
    jar: CookieJar,
    mut multipart: Multipart,
) -> (CookieJar, Json<JsonValue>) {
    let mut geom_json = "".to_string();
    let mut user_name = "".to_string();
    let mut _edit = "".to_string();
    
    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        let value = field.text().await.unwrap_or("".to_string());
        match name.as_str() {
            "geom_json" => geom_json = value,
            "user_name" => user_name = value,
            "edit" => _edit = value,
            _ => {}
        }
    }
    
    
    // Récupérer les données existantes pour ce segment (historique, photos, etc.)
    // Pour l'instant, on retourne une réponse avec edit=true et la géométrie
    // fit_bounds: false pour ne pas bouger la carte en mode édition
    let json = Json(json!({
        "way_ids": "".to_string(),
        "score_circle": { "score": -1. },
        "segment_name": "Segment sélectionné".to_string(),
        "score_selector": ScoreSelector::get_score_selector(-1.),
        "comment": "".to_string(),
        "edit": true,
        "history": Vec::<InfopanelContribution>::new(),
        "photo_ids": Vec::<i32>::new(),
        "geom_json": &geom_json,
        "fit_bounds": false,
        "user_name": user_name,
        "martin_url": format!("{}/martin", env::var("VELOINFO_URL").unwrap()),
    }));
    
    (jar, json)
}

/// Récupère le nom utilisateur depuis le cookie uuid.
/// Utilisé par les handlers qui n'ont pas de score.user_id (nouveau segment, pas de report en DB).
async fn get_user_name(jar: &CookieJar, conn: &sqlx::Pool<sqlx::Postgres>) -> String {
    match jar.get("uuid") {
        Some(uuid_cookie) => {
            match Uuid::parse_str(uuid_cookie.value().to_string().as_str()) {
                Ok(uuid) => match User::get(&uuid, conn).await {
                    Some(user) => user.name,
                    None => "".to_string(),
                },
                Err(_) => "".to_string(),
            }
        }
        None => "".to_string(),
    }
}

async fn segment_panel_report_id(
    conn: &sqlx::Pool<sqlx::Postgres>,
    id: i32,
    edit: bool,
) -> Json<JsonValue> {
    let score = match Report::get_by_id(id, &conn).await {
        Ok(score) => score,
        Err(e) => {
            eprintln!("Error while fetching score: {}", e);
            Report {
                id: 0,
                name: Some(vec![]),
                score: -1.,
                created_at: chrono::DateTime::from_timestamp(0, 0).unwrap().into(),
                photo_path_thumbnail: None,
                geom: vec![],
                user_id: None,
            }
        }
    };

    // Fetch comment separately
    let comment = match Report::get_comment_by_report_id(id, &conn).await {
        Ok(Some((_, c))) => c,
        _ => "".to_string(),
    };

    let segment_name = score.name.as_ref().map(|n| n.iter().filter_map(|s| s.clone()).collect::<Vec<_>>().join(" ")).unwrap_or_default();
    let geom_json = match serde_json::to_string(&score.geom) {
        Ok(geom) => geom,
        Err(e) => {
            eprintln!("Error while serializing geom: {}", e);
            "".to_string()
        }
    };

    // TODO: Implémenter get_history par geom
    let history = Vec::<InfopanelContribution>::new();
    // TODO: Implémenter get_photo_by_geom
    let photo_ids = Vec::<i32>::new();
    
    let user_name = match score.user_id {
        Some(uid) => match User::get(&uid, conn).await {
            Some(user) => user.name,
            None => "".to_string(),
        },
        None => "".to_string(),
    };

    Json(json!({
        "way_ids": "".to_string(),
        "score_circle": {
            "score": score.score,
        },
        "segment_name": segment_name,
        "score_selector": ScoreSelector::get_score_selector(score.score),
        "comment": comment,
        "edit": edit,
        "history": history,
        "photo_ids": photo_ids,
        "geom_json": geom_json,
        "fit_bounds": true,
        "user_name": user_name,
        "martin_url": format!("{}/martin", env::var("VELOINFO_URL").unwrap()),
    }))
}

pub async fn segment_panel_lng_lat(
    State(state): State<VeloinfoState>,
    jar: CookieJar,
    Path((lng, lat)): Path<(f64, f64)>,
) -> impl IntoResponse {
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
    let photo_ids = Vec::<i32>::new();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&serde_json::json!({
            "way_ids": node.way_id.to_string(),
            "score_circle": {
                "score": way.score.unwrap_or(-1.0),
            },
            "segment_name": segment_name,
            "score_selector": ScoreSelector::get_score_selector(way.score.unwrap_or(-1.0)),
            "comment": "".to_string(),
            "edit": false,
            "history": history,
            "photo_ids": photo_ids,
            "geom_json": serde_json::to_string(&vec![node.geom]).unwrap_or("".to_string()),
            "fit_bounds": false,
            "user_name": get_user_name(&jar, &state.conn).await,
            "martin_url": format!("{}/martin", env::var("VELOINFO_URL").unwrap()),
        })).unwrap_or_default(),
    )
}

pub async fn segment_between(
    State(state): State<VeloinfoState>,
    jar: CookieJar,
    Path((start_lng, start_lat, end_lng, end_lat)): Path<(f64, f64, f64, f64)>,
) -> Json<JsonValue> {
    // Pour les segments personnalisés, on utilise directement les points de début et fin
    // sans passer par le routage A* sur les ways OSM
    
    // Créer une LineString entre les deux points
    let coords = vec![
        [start_lng, start_lat],
        [end_lng, end_lat]
    ];
    
    // Créer une LineString GeoJSON
    let line_string_json = serde_json::json!({
        "type": "LineString",
        "coordinates": coords
    });

    // Bufferiser la LineString à 10m en utilisant PostGIS
    // On passe la LineString à PostGIS, il fait le buffer et retourne un Polygon
    let buffer_query = r#"
        WITH line AS (
            SELECT ST_GeomFromGeoJSON($1) AS geom
        ),
        transformed AS (
            SELECT ST_Transform(geom, 3857) AS geom FROM line
        ),
        buffered AS (
            SELECT ST_Buffer(geom, 10) AS geom FROM transformed
        ),
        final AS (
            SELECT ST_Transform(geom, 4326) AS geom FROM buffered
        )
        SELECT ST_AsGeoJSON(geom) FROM final;
    "#;

    let geom_json: String = match sqlx::query_scalar(buffer_query)
        .bind(serde_json::to_string(&line_string_json).unwrap_or_default())
        .fetch_optional(&state.conn)
        .await
    {
        Ok(Some(geom)) => {
            geom
        },
        Ok(None) => {
            // Fallback: retourner la LineString brute
            serde_json::to_string(&line_string_json).unwrap_or_default()
        },
        Err(e) => {
            eprintln!("Buffer SQL error: {}", e);
            // Fallback: retourner la LineString brute
            serde_json::to_string(&line_string_json).unwrap_or_default()
        }
    };

    // Récupérer l'historique et les photos en utilisant la geom
    // Pour l'instant, on retourne des tableaux vides pour les segments personnalisés
    // L'historique par geom sera implémenté avec une comparaison spatiale
    let history = Vec::<InfopanelContribution>::new();
    let photo_ids = Vec::<i32>::new();

    let json = json!({
        "way_ids": "".to_string(),
        "score_circle": {
            "score": -1.,
        },
        "segment_name": "Segment sélectionné".to_string(),
        "score_selector": ScoreSelector::get_score_selector(-1.),
        "comment": "".to_string(),
        "edit": false,
        "history": history,
        "photo_ids": photo_ids,
        "geom_json": geom_json,
        "fit_bounds": false,
        "user_name": get_user_name(&jar, &state.conn).await,
        "martin_url": format!("{}/martin", env::var("VELOINFO_URL").unwrap()),
    });
    Json(json)
}

pub async fn select_report_id(
    State(state): State<VeloinfoState>,
    Path(id): Path<i32>,
) -> Json<JsonValue> {
    segment_panel_report_id(&state.conn, id, false).await
}

/// Endpoint léger pour récupérer le nom utilisateur depuis le cookie uuid.
/// Utilisé par le frontend quand un panel est créé client-side (mode "signaler").
pub async fn get_user_name_endpoint(
    State(state): State<VeloinfoState>,
    jar: CookieJar,
) -> Json<JsonValue> {
    Json(json!({ "user_name": get_user_name(&jar, &state.conn).await }))
}

/// Endpoint MVT pour afficher les segments report sur la carte
#[axum::debug_handler]
pub async fn report_mvt(
    State(state): State<VeloinfoState>,
    Path((z, x, y)): Path<(u32, u32, u32)>,
) -> impl IntoResponse {
    let conn = &state.conn;

    // Requête MVT - toutes les géométries sont en 4326, on transforme pour l'affichage
    let query = r#"
    WITH
    bounds AS (
        SELECT 
            ((2 * $2::double precision / pow(2, $1)) - 1) * 20037508.34 as xmin,
            ((1 - (2 * ($3 + 1)::double precision / pow(2, $1)))) * 20037508.34 as ymin,
            ((2 * ($2 + 1)::double precision / pow(2, $1)) - 1) * 20037508.34 as xmax,
            ((1 - (2 * $3::double precision / pow(2, $1)))) * 20037508.34 as ymax
    ),
    envelope AS (
        SELECT ST_MakeEnvelope(xmin, ymin, xmax, ymax, 3857) as geom FROM bounds
    ),
    mvtgeom AS (
        SELECT
            ST_AsMVTGeom(
                ST_Transform(r.geom, 3857),
                e.geom
            ) AS geom,
            r.score,
            r.created_at,
            r.user_id,
            rc.comment
        FROM
            report r, envelope e
        LEFT JOIN LATERAL (
            SELECT comment
            FROM report_comment
            WHERE report_id = r.id
            ORDER BY created_at DESC
            LIMIT 1
        ) rc ON true
        WHERE
            ST_Transform(r.geom, 3857) && e.geom
    )
    SELECT ST_AsMVT(mvtgeom.*, 'report', 4096, 'geom')
    FROM mvtgeom;
    "#;

    let result = match sqlx::query(query)
        .bind(z as i64)
        .bind(x as i64)
        .bind(y as i64)
        .fetch_one(conn)
        .await
    {
        Ok(row) => {
            let mvt_data: Vec<u8> = row.get(0);
            mvt_data
        },
        Err(e) => {
            eprintln!("Error fetching report MVT for z={}, x={}, y={}: {}", z, x, y, e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Error fetching MVT").into_response();
        }
    };

    Response::builder()
        .header(header::CONTENT_TYPE, "application/x-protobuf")
        .header(header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .header(header::EXPIRES, "0")
        .body(Body::from(result))
        .unwrap()
        .into_response()
}

/// Endpoint TileJSON pour report
pub async fn report() -> Json<JsonValue> {
    let tilejson = json!({
        "tilejson": "3.0.0",
        "name": "report",
        "tiles": [
            format!("{}/report/{{z}}/{{x}}/{{y}}", env::var("VELOINFO_URL").unwrap())
        ],
        "vector_layers": [
            {
                "id": "report",
                "fields": {
                    "score": "Number",
                    "comment": "String",
                    "created_at": "String",
                    "user_id": "Number"
                },
                "minzoom": 10,
                "maxzoom": 22
            }
        ]
    });
    Json(tilejson)
}

/// Endpoint pour répondre à un commentaire existant
pub async fn report_reply_post(
    State(state): State<VeloinfoState>,
    jar: CookieJar,
    mut multipart: Multipart,
) -> (CookieJar, Json<JsonValue>) {
    
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

    let mut report_id = 0;
    let mut parent_comment_id: Option<i32> = None;
    let mut user_name = "".to_string();
    let mut comment = "".to_string();
    let mut photo = None;
    
    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap();
        match name {
            "report_id" => {
                report_id = field.text().await.unwrap_or("0".to_string()).parse::<i32>().unwrap_or(0)
            }
            "parent_comment_id" => {
                let val = field.text().await.unwrap_or("".to_string());
                parent_comment_id = if val.is_empty() { None } else { val.parse::<i32>().ok() };
            }
            "user_name" => user_name = field.text().await.unwrap_or("".to_string()),
            "comment" => comment = field.text().await.unwrap_or("".to_string()),
            "photo" => {
                photo = match field.bytes().await {
                    Ok(b) if !b.is_empty() => Some(b),
                    _ => None,
                }
            }
            _ => (),
        }
    }
    
    if let Some(user_id) = user_id {
        User::update(&user_id, &user_name, &state.conn).await;
    }
    
    // Insérer le commentaire de réponse
    let new_comment_id = match Report::insert_comment(report_id, &comment, parent_comment_id, &user_name, &state.conn).await {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Error inserting reply comment: {}", e);
            return (
                jar,
                Json(json!({
                    "success": false,
                    "error": "Erreur d'insertion du commentaire"
                })),
            );
        }
    };
    
    // Traiter la photo si présente (optionnel pour les réponses)
    let photo_path_thumbnail = if let Some(photo_bytes) = photo {
        match process_and_save_photo(new_comment_id, &photo_bytes) {
            Ok(path) => Some(path),
            Err(e) => {
                eprintln!("Error processing photo: {}", e);
                None
            }
        }
    } else {
        None
    };
    
    // Mettre à jour le chemin de la photo si traitée
    if let Some(ref path) = photo_path_thumbnail {
        if let Err(e) = ReportComment::update_photo_thumbnail(new_comment_id, path, &state.conn).await {
            eprintln!("Error updating comment photo: {}", e);
        }
    }
    
    
    (
        jar,
        Json(json!({
            "success": true,
            "comment_id": new_comment_id,
            "report_id": report_id
        })),
    )
}

/// Helper pour traiter et sauver une photo
fn process_and_save_photo(_report_id: i32, photo_bytes: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let img = image::load_from_memory(photo_bytes)?;
    let thumbnail = img.thumbnail(100, 100);
    
    let uuid = Uuid::new_v4();
    let photo_filename = format!("{}_thumbnail.jpeg", uuid);
    let photo_path = format!("{}/{}", *IMAGE_DIR, photo_filename);
    
    thumbnail.save(&photo_path)?;
    
    Ok(photo_filename)
}
