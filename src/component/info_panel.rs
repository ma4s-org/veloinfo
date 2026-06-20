use super::score_circle::ScoreCircle;
use crate::db::report::Report;
use crate::db::report_comment::ReportComment;
use crate::db::user::User;
use crate::VeloinfoState;
use axum::extract::{Path, State};
use axum::Json;
use chrono::Locale;
use chrono_tz::America::Montreal;
use futures::future::join_all;
use serde::Serialize;
use sqlx::types::chrono::Local;
use sqlx::Postgres;
use timeago;
use timeago::languages::french::French;

#[derive(Clone, Serialize)]
pub struct InfoPanelTemplate {
    pub arrow: String,
    pub contributions: Vec<InfopanelContribution>,
}

#[derive(Clone, Serialize)]
pub struct InfopanelContribution {
    id: Option<i32>,
    created_at: String,
    timeago: String,
    score_circle: ScoreCircle,
    name: String,
    comment: String,
    report_id: i32,
    photo_path_thumbnail: Option<String>,
    user_name: String,
    enabled: bool,
    replies: Vec<InfopanelContributionReply>,
}

#[derive(Clone, Serialize)]
pub struct InfopanelContributionReply {
    id: i32,
    created_at: String,
    timeago: String,
    comment: String,
    user_name: String,
    photo_path_thumbnail: Option<String>,
    replies: Vec<InfopanelContributionReply>,
}

impl InfopanelContribution {
    pub async fn get(
        lng1: f64,
        lat1: f64,
        lng2: f64,
        lat2: f64,
        conn: &sqlx::Pool<Postgres>,
    ) -> Vec<InfopanelContribution> {
        let scores = match Report::get_recents(lng1, lat1, lng2, lat2, conn).await {
            Result::Ok(cs) => cs,
            Err(e) => {
                eprintln!("Error getting contributions {:?}", e);
                Vec::new()
            }
        };

        join_all(scores.iter().map(|score| async {
            // Fetch comment separately (premier commentaire - racine)
            let (comment_id, comment) = match Report::get_comment_by_report_id(score.id, conn).await {
                Ok(Some((id, c))) => (Some(id), c),
                _ => (None, "".to_string()),
            };
            
            // Fetch all comments and build hierarchy
            let all_comments = ReportComment::get_by_report_id(score.id, conn).await.unwrap_or_default();
            
            // Convertir Vec<ReportComment> en Vec<InfopanelContributionReply>
            fn convert_replies(replies: &[ReportComment]) -> Vec<InfopanelContributionReply> {
                replies.iter().map(|reply| InfopanelContributionReply {
                    id: reply.id,
                    created_at: reply.created_at.with_timezone(&Montreal).format_localized("%H:%M - %d %B", Locale::fr_CA).to_string(),
                    timeago: timeago::Formatter::with_language(French).convert_chrono(reply.created_at, Local::now()),
                    comment: reply.comment.clone(),
                    user_name: reply.user_name.clone(),
                    photo_path_thumbnail: reply.photo_path_thumbnail.as_ref().map(|p| format!("/images/{}", p)),
                    replies: convert_replies(&reply.replies),
                }).collect()
            }
            
            let replies = convert_replies(&all_comments);

            InfopanelContribution {
                id: comment_id,
                created_at: score
                    .created_at
                    .with_timezone(&Montreal)
                    .format_localized("%H:%M - %d %B", Locale::fr_CA)
                    .to_string(),
                timeago: timeago::Formatter::with_language(French)
                    .convert_chrono(score.created_at, Local::now()),
                score_circle: ScoreCircle { score: score.score },
                name: get_name(&score.name).await,
                comment: comment,
                report_id: score.id,
                photo_path_thumbnail: score.photo_path_thumbnail.clone(),
                user_name: match score.user_id {
                    Some(user_id) => match User::get(&user_id, conn).await {
                        Some(user) => user.name,
                        None => "".to_string(),
                    },
                    None => "".to_string(),
                },
                replies: replies,
                enabled: score.enabled,
            }
        }))
        .await
    }

    #[allow(dead_code)]
    pub async fn get_history(
        _way_ids: &Vec<i64>,
        _conn: &sqlx::Pool<Postgres>,
    ) -> Vec<InfopanelContribution> {
        // TODO: Implémenter get_history par geom pour les segments personnalisés
        // Pour l'instant, retourne un tableau vide
        Vec::new()
    }

    pub async fn get_history_by_way_id(
        _way_id: i64,
        _conn: &sqlx::Pool<Postgres>,
    ) -> Vec<InfopanelContribution> {
        // TODO: Implémenter get_history par geom pour les segments personnalisés
        // Pour l'instant, retourne un tableau vide
        Vec::new()
    }
}

async fn get_name(names: &Option<Vec<Option<String>>>) -> String {
    if let Some(names) = names {
        names.iter().fold("".to_string(), |acc, name| {
            let blank_name = "non inconnu".to_string();
            let name = match name {
                Some(name) => name,
                None => &blank_name,
            };
            if acc.find(name) != None {
                return acc;
            }
            format!("{} {}", acc, name)
        })
    } else {
        "".to_string()
    }
}

pub async fn info_panel_up(
    State(state): State<VeloinfoState>,
    Path((lng1, lat1, lng2, lat2)): Path<(f64, f64, f64, f64)>,
) -> Json<InfoPanelTemplate> {
    let contributions = InfopanelContribution::get(lng1, lat1, lng2, lat2, &state.conn).await;
    InfoPanelTemplate {
        arrow: "▼".to_string(),
        contributions: contributions,
    }
    .into()
}
