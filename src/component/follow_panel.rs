use askama::Template;
use askama_web::WebTemplate;
use axum::{debug_handler, extract::State};

use crate::VeloinfoState;

#[derive(Template, WebTemplate)]
#[template(path = "follow_panel.html")]
pub struct FollowPanel {
    pub error: String,
}

#[debug_handler]
pub async fn follow(State(_state): State<VeloinfoState>) -> FollowPanel {
    FollowPanel {
        error: "".to_string(),
    }
}
