use crate::db::cyclability_score::CyclabilityScore;
use crate::VeloinfoState;
use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};
use lazy_static::lazy_static;
use regex::Regex;

#[derive(Template, WebTemplate)]
#[template(path = "photo_scroll.html")]
pub struct PhotoScroll {
    pub photo: String,
    pub next: Option<String>,
    pub previous: Option<String>,
    pub way_ids: String,
}

lazy_static! {
    static ref INT_REGEX: Regex = Regex::new(r"\d+").unwrap();
}

pub async fn photo_scroll(
    State(state): State<VeloinfoState>,
    Path((photo, way_ids)): Path<(String, String)>,
) -> PhotoScroll {
    let way_ids_i64 = INT_REGEX
        .find_iter(&way_ids)
        .map(|m| m.as_str().parse::<i64>().unwrap())
        .collect();
    let scores = CyclabilityScore::get_photo_by_way_ids(&way_ids_i64, &state.conn).await;
    let mut next = None;
    let mut previous = None;
    for (i, score) in scores.iter().enumerate() {
        if score == &photo.parse::<i32>().unwrap() {
            if i > 0 {
                previous = Some(scores[i - 1].to_string());
            }
            if i < scores.len() - 1 {
                next = Some(scores[i + 1].to_string());
            }
        }
    }
    PhotoScroll {
        photo,
        next,
        previous,
        way_ids,
    }
}
