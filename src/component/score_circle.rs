use askama::Template;
use serde::Serialize;

#[derive(Template, Clone, Serialize)]
#[template(path = "score_circle.html")]
pub struct ScoreCircle {
    pub score: f64,
}
