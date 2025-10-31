use serde::Serialize;

#[derive(PartialEq, Serialize)]
pub enum Category {
    Good,
    Problems,
    Dangerous,
    Closed,
}

#[derive(Serialize)]
pub struct ScoreSelector {
    category: Category,
    score: f64,
}

impl ScoreSelector {
    pub fn get_score_selector(score: f64) -> ScoreSelector {
        let category = if score == 0.0 {
            Category::Closed
        } else if score <= 0.35 && score > 0.0 {
            Category::Dangerous
        } else if score <= 0.68 && score > 0.35 {
            Category::Problems
        } else {
            Category::Good
        };
        ScoreSelector { score, category }
    }
}
