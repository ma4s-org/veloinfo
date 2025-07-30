use crate::{db::search_db, VeloinfoState};
use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, State};

#[derive(Template, WebTemplate)]
#[template(path = "point_panel.html", escape = "none")]
pub struct PointPanel {
    name: String,
}

pub async fn point_panel_lng_lat(
    Path((lng, lat)): Path<(f64, f64)>,
    state: State<VeloinfoState>,
) -> PointPanel {
    let name = match search_db::get_any(&lng, &lat, &state.conn).await.first() {
        Some(ar) => ar.name.clone(),
        None => "".to_string(),
    };
    PointPanel { name }
}
