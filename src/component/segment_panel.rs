use super::info_panel::InfoPanelTemplate;
use crate::{db::cyclability_score::CyclabilityScore, VeloInfoError, VeloinfoState};
use anyhow::Result;
use askama::Template;
use axum::{
    extract::{Path, State},
    Form,
};
use futures::future::join_all;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::Postgres;

#[derive(Template)]
#[template(path = "segment_panel.html", escape = "none")]
pub struct SegmentPanel {
    way_ids: String,
    status: String,
    segment_name: String,
    options: String,
    comment: String,
    info_panel_template: InfoPanelTemplate,
}

#[derive(Debug, sqlx::FromRow, Clone)]
struct WayInfo {
    name: Option<String>,
    score: Option<f64>,
}

impl WayInfo {
    pub async fn get(way_id: i64, conn: sqlx::Pool<Postgres>) -> Result<WayInfo, sqlx::Error> {
        sqlx::query_as(
            r#"select  
                c.name,
                cs.score,
                cs.comment
               from cycleway c
               left join cyclability_score cs on c.way_id = any(cs.way_ids)
               where c.way_id = $1
               order by created_at desc"#,
        )
        .bind(way_id)
        .fetch_one(&conn)
        .await
    }
}

pub async fn get_empty_segment_panel() -> String {
    let info_panel_template = InfoPanelTemplate {
        arrow: "▲".to_string(),
        direction: "up".to_string(),
        contributions: Vec::new(),
    };

    SegmentPanel {
        way_ids: "".to_string(),
        status: "none".to_string(),
        segment_name: "".to_string(),
        options: "".to_string(),
        comment: "".to_string(),
        info_panel_template,
    }
    .render()
    .unwrap()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostValue {
    pub score: Option<f64>,
    pub comment: Option<String>,
}

pub async fn segment_panel_post(
    State(state): State<VeloinfoState>,
    Path(way_ids): Path<String>,
    Form(post): Form<PostValue>,
) -> Result<String, VeloInfoError> {
    let re = Regex::new(r"\d*").unwrap();
    let way_ids_i64 = re
        .find_iter(way_ids.as_str())
        .map(|m| m.as_str().parse::<i64>().unwrap())
        .collect::<Vec<i64>>();
    let conn = state.conn.clone();

    sqlx::query(
        r#"INSERT INTO cyclability_score 
                    (way_ids, score, comment) 
                    VALUES ($1, $2, $3)"#,
    )
    .bind(way_ids_i64)
    .bind(post.score)
    .bind(post.comment)
    .execute(&conn)
    .await?;

    segment_panel(State(state), Path(way_ids)).await
}

fn get_options(score: f64) -> String {
    let s = vec![
        (-1., "⚪ inconnu", "disabled"),
        (0.2, "🔴 Impossible", ""),
        (0.4, "🟠 mauvais", ""),
        (0.6, "🟡 difficile", ""),
        (0.8, "🟢 bon", ""),
        (1., "🔵 excellent", ""),
    ];
    s.iter()
        .map(|(s, color, disabled)| {
            ScoreOption {
                score: *s,
                selected: if *s == score {
                    "selected".to_string()
                } else {
                    "".to_string()
                },
                color: color.to_string(),
                disabled: disabled.to_string(),
            }
            .render()
            .unwrap()
        })
        .collect::<Vec<String>>()
        .join(" ")
}

pub async fn segment_panel(
    State(state): State<VeloinfoState>,
    Path(way_ids): Path<String>,
) -> Result<String, VeloInfoError> {
    let re = Regex::new(r"\d+").unwrap();
    let way_ids_i64 = re
        .find_iter(way_ids.as_str())
        .map(|cap| cap.as_str().parse::<i64>().unwrap())
        .collect::<Vec<i64>>();
    let ways = join_all(way_ids_i64.iter().map(|way_id| async {
        let conn = state.conn.clone();
        WayInfo::get(*way_id, conn).await.unwrap()
    }))
    .await;
    let all_same_score = ways.iter().all(|way| way.score == ways[0].score);
    let mut way = ways[0].clone();
    if !all_same_score {
        way.score = Some(-1.);
    }
    let way_score = match way.score {
        Some(score) => score,
        None => -1.,
    };
    let options = get_options(way_score);
    let segment_name = ways
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
    let info_panel = SegmentPanel {
        way_ids,
        status: "segment".to_string(),
        segment_name,
        options,
        comment: "".to_string(),
        info_panel_template: InfoPanelTemplate {
            arrow: "▲".to_string(),
            direction: "up".to_string(),
            contributions: Vec::new(),
        },
    }
    .render()
    .unwrap()
    .to_string();

    Ok(info_panel)
}

pub async fn select_score_id(
    State(state): State<VeloinfoState>,
    Path(id): Path<i32>,
) -> Result<String, VeloInfoError> {
    let score = CyclabilityScore::get_by_id(id, state.conn.clone())
        .await
        .unwrap();
    let segment_name = join_all(score.way_ids.iter().map(|way_id| async {
        let conn = state.conn.clone();
        WayInfo::get(*way_id, conn).await.unwrap()
    }))
    .await
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
    let panel = SegmentPanel {
        way_ids: score
            .way_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<String>>()
            .join(" "),
        status: "segment".to_string(),
        segment_name,
        options: get_options(score.score),
        comment: score.comment.unwrap_or("".to_string()),
        info_panel_template: InfoPanelTemplate {
            arrow: "▲".to_string(),
            direction: "up".to_string(),
            contributions: Vec::new(),
        },
    };

    Ok(panel.render().unwrap())
}

#[derive(Template)]
#[template(
    source = r#"<option value="{{score}}" {{selected}} {{disabled}}>{{color}}</option>"#,
    escape = "none",
    ext = "txt"
)]
struct ScoreOption {
    score: f64,
    selected: String,
    color: String,
    disabled: String,
}