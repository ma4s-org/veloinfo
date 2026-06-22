use chrono::{DateTime, Local};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Debug)]
pub struct ReportCommentDb {
    pub id: i32,
    pub comment: String,
    pub parent_comment_id: Option<i32>,
    pub created_at: DateTime<Local>,
    #[allow(dead_code)]
    pub user_id: Option<Uuid>,
    pub user_name: Option<String>,
    pub photo_path_thumbnail: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReportComment {
    pub id: i32,
    pub comment: String,
    pub parent_comment_id: Option<i32>,
    pub created_at: DateTime<Local>,
    pub user_name: String,
    pub photo_path_thumbnail: Option<String>,
    pub replies: Vec<ReportComment>,
}

impl ReportComment {
    pub async fn get_by_report_id(
        report_id: i32,
        conn: &sqlx::Pool<sqlx::Postgres>,
    ) -> Result<Vec<ReportComment>, sqlx::Error> {
        let rows: Vec<ReportCommentDb> = sqlx::query_as(
            r#"SELECT 
                rc.id,
                rc.comment,
                rc.parent_comment_id,
                rc.created_at,
                rc.user_id,
                u.name AS user_name,
                rc.photo_path_thumbnail
            FROM report_comment rc
            LEFT JOIN users u ON rc.user_id = u.id
            WHERE rc.report_id = $1
            ORDER BY rc.created_at ASC"#,
        )
        .bind(report_id)
        .fetch_all(conn)
        .await?;

        // Convertir en ReportComment
        let comments: Vec<ReportComment> = rows
            .into_iter()
            .map(|db| ReportComment {
                id: db.id,
                comment: db.comment,
                parent_comment_id: db.parent_comment_id,
                created_at: db.created_at,
                user_name: db.user_name.unwrap_or_default(),
                photo_path_thumbnail: db.photo_path_thumbnail,
                replies: Vec::new(),
            })
            .collect();

        // Construire la hiérarchie : ne retourner que les réponses (parent_comment_id.is_some())
        let all_comments = build_hierarchy(&comments, None);
        
        // Filtrer pour ne garder que les réponses (pas les commentaires racines)
        let mut replies = Vec::new();
        for comment in all_comments {
            if comment.parent_comment_id.is_some() {
                replies.push(comment);
            } else {
                // Ajouter les réponses de ce commentaire racine
                replies.extend(comment.replies);
            }
        }
        
        Ok(replies)
    }

    pub async fn update_photo_thumbnail(
        id: i32,
        photo_path_thumbnail: &str,
        conn: &sqlx::Pool<sqlx::Postgres>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE report_comment SET photo_path_thumbnail = $1 WHERE id = $2"#,
        )
        .bind(photo_path_thumbnail)
        .bind(id)
        .execute(conn)
        .await?;
        Ok(())
    }
}

fn build_hierarchy(comments: &[ReportComment], parent_id: Option<i32>) -> Vec<ReportComment> {
    let mut result = Vec::new();
    
    for comment in comments {
        if comment.parent_comment_id == parent_id {
            let mut new_comment = comment.clone();
            // Trouver récursivement les réponses à ce commentaire
            new_comment.replies = build_hierarchy(comments, Some(comment.id));
            result.push(new_comment);
        }
    }
    
    result
}
