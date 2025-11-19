use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct ScoreCircle {
    pub score: f64,
}
