use chrono::{DateTime, Utc};
use sqlx::PgPool;

/// Informations nécessaires pour construire une Note ActivityPub à partir
/// d'un commentaire local ou distant.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommentFederation {
    pub id: i32,
    pub report_id: i32,
    pub parent_comment_id: Option<i32>,
    pub comment: String,
    pub created_at: DateTime<Utc>,
    /// URL ActivityPub de la Note lorsqu'il s'agit d'un commentaire distant.
    /// NULL pour les commentaires locaux : leur URL est alors calculée.
    pub ap_id: Option<String>,
    pub ap_remote_actor_id: Option<String>,
    pub parent_ap_id: Option<String>,
    pub parent_remote_actor_id: Option<String>,
    /// Photo du commentaire, ou du report pour le commentaire racine.
    pub photo_path_thumbnail: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RemoteActor {
    pub id: String,
    pub inbox: String,
    pub shared_inbox: Option<String>,
    pub public_key_id: Option<String>,
    pub public_key_pem: String,
    pub display_name: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Delivery {
    pub id: i64,
    pub inbox: String,
    pub payload: String,
    pub attempts: i32,
    pub account: String,
}

/// Note ActivityPub publiée lorsqu'un import de travaux échoue.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ImportErrorNote {
    pub id: i32,
    pub failures: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

impl ImportErrorNote {
    pub async fn last(conn: &PgPool) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as(
            "SELECT id, failures, content, created_at
             FROM ap_import_error
             ORDER BY created_at DESC, id DESC
             LIMIT 1",
        )
        .fetch_optional(conn)
        .await
    }

    pub async fn get(id: i32, conn: &PgPool) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as(
            "SELECT id, failures, content, created_at
             FROM ap_import_error
             WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(conn)
        .await
    }

    pub async fn insert(
        failures: &str,
        content: &str,
        conn: &PgPool,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as(
            "INSERT INTO ap_import_error (failures, content)
             VALUES ($1, $2)
             RETURNING id, failures, content, created_at",
        )
        .bind(failures)
        .bind(content)
        .fetch_one(conn)
        .await
    }
}

const COMMENT_FEDERATION_SELECT: &str = r#"
    SELECT
        rc.id,
        rc.report_id,
        rc.parent_comment_id,
        rc.comment,
        rc.created_at,
        rc.ap_id,
        rc.ap_remote_actor_id,
        p.ap_id AS parent_ap_id,
        p.ap_remote_actor_id AS parent_remote_actor_id,
        CASE
            WHEN rc.parent_comment_id IS NULL THEN rp.photo_path_thumbnail
            ELSE rc.photo_path_thumbnail
        END AS photo_path_thumbnail
    FROM report_comment rc
    LEFT JOIN report_comment p ON p.id = rc.parent_comment_id
    LEFT JOIN report rp ON rp.id = rc.report_id
"#;

impl CommentFederation {
    pub async fn get(id: i32, conn: &PgPool) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as(&format!("{} WHERE rc.id = $1", COMMENT_FEDERATION_SELECT))
            .bind(id)
            .fetch_optional(conn)
            .await
    }

    pub async fn find_by_ap_id(ap_id: &str, conn: &PgPool) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as(&format!("{} WHERE rc.ap_id = $1", COMMENT_FEDERATION_SELECT))
            .bind(ap_id)
            .fetch_optional(conn)
            .await
    }

    /// Commentaires locaux, du plus récent au plus ancien (pour l'outbox).
    pub async fn list_local(
        limit: i64,
        offset: i64,
        conn: &PgPool,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as(&format!(
            "{} WHERE rc.ap_remote_actor_id IS NULL
             ORDER BY rc.created_at DESC, rc.id DESC
             LIMIT $1 OFFSET $2",
            COMMENT_FEDERATION_SELECT
        ))
        .bind(limit)
        .bind(offset)
        .fetch_all(conn)
        .await
    }

    pub async fn count_local(conn: &PgPool) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM report_comment WHERE ap_remote_actor_id IS NULL",
        )
        .fetch_one(conn)
        .await
    }

    /// Insère une réponse reçue du Fédivers. Retourne l'id local, ou None si
    /// la Note a déjà été enregistrée (doublon de livraison).
    pub async fn insert_remote(
        report_id: i32,
        parent_comment_id: i32,
        comment: &str,
        ap_id: &str,
        actor_id: &str,
        actor_name: &str,
        conn: &PgPool,
    ) -> Result<Option<i32>, sqlx::Error> {
        sqlx::query_scalar(
            r#"INSERT INTO report_comment
                   (report_id, comment, parent_comment_id, ap_id,
                    ap_remote_actor_id, ap_remote_actor_name)
               VALUES ($1, $2, $3, $4, $5, $6)
               ON CONFLICT (ap_id) WHERE ap_id IS NOT NULL DO NOTHING
               RETURNING id"#,
        )
        .bind(report_id)
        .bind(comment)
        .bind(parent_comment_id)
        .bind(ap_id)
        .bind(actor_id)
        .bind(actor_name)
        .fetch_optional(conn)
        .await
    }

    pub async fn update_remote(
        ap_id: &str,
        actor_id: &str,
        comment: &str,
        conn: &PgPool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE report_comment
               SET comment = $3
               WHERE ap_id = $1 AND ap_remote_actor_id = $2"#,
        )
        .bind(ap_id)
        .bind(actor_id)
        .bind(comment)
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn delete_remote(
        ap_id: &str,
        actor_id: &str,
        conn: &PgPool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"DELETE FROM report_comment
               WHERE ap_id = $1 AND ap_remote_actor_id = $2"#,
        )
        .bind(ap_id)
        .bind(actor_id)
        .execute(conn)
        .await?;
        Ok(())
    }

    /// Supprime tous les commentaires d'un acteur distant qui supprime son
    /// compte (activité `Delete` ayant pour objet l'acteur lui-même).
    pub async fn delete_remote_actor_comments(
        actor_id: &str,
        conn: &PgPool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM report_comment WHERE ap_remote_actor_id = $1")
            .bind(actor_id)
            .execute(conn)
            .await?;
        Ok(())
    }
}

impl RemoteActor {
    pub async fn get(id: &str, conn: &PgPool) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as(
            r#"SELECT id, inbox, shared_inbox, public_key_id, public_key_pem,
                      display_name, username
               FROM ap_remote_actors
               WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(conn)
        .await
    }

    pub async fn upsert(actor: &RemoteActor, conn: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO ap_remote_actors
                   (id, inbox, shared_inbox, public_key_id, public_key_pem,
                    display_name, username, fetched_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
               ON CONFLICT (id) DO UPDATE SET
                   inbox = EXCLUDED.inbox,
                   shared_inbox = EXCLUDED.shared_inbox,
                   public_key_id = EXCLUDED.public_key_id,
                   public_key_pem = EXCLUDED.public_key_pem,
                   display_name = EXCLUDED.display_name,
                   username = EXCLUDED.username,
                   fetched_at = NOW()"#,
        )
        .bind(&actor.id)
        .bind(&actor.inbox)
        .bind(&actor.shared_inbox)
        .bind(&actor.public_key_id)
        .bind(&actor.public_key_pem)
        .bind(&actor.display_name)
        .bind(&actor.username)
        .execute(conn)
        .await?;
        Ok(())
    }
}

pub async fn upsert_follower(
    actor_id: &str,
    inbox: &str,
    shared_inbox: Option<&str>,
    display_name: Option<&str>,
    follow_activity_id: Option<&str>,
    conn: &PgPool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO ap_followers
               (actor_id, inbox, shared_inbox, display_name, follow_activity_id)
           VALUES ($1, $2, $3, $4, $5)
           ON CONFLICT (actor_id) DO UPDATE SET
               inbox = EXCLUDED.inbox,
               shared_inbox = EXCLUDED.shared_inbox,
               display_name = EXCLUDED.display_name,
               follow_activity_id = EXCLUDED.follow_activity_id"#,
    )
    .bind(actor_id)
    .bind(inbox)
    .bind(shared_inbox)
    .bind(display_name)
    .bind(follow_activity_id)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn remove_follower(actor_id: &str, conn: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM ap_followers WHERE actor_id = $1")
        .bind(actor_id)
        .execute(conn)
        .await?;
    Ok(())
}

pub async fn remove_follower_by_activity(
    follow_activity_id: &str,
    actor_id: &str,
    conn: &PgPool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM ap_followers WHERE actor_id = $1 AND follow_activity_id = $2",
    )
    .bind(actor_id)
    .bind(follow_activity_id)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn follower_inboxes(conn: &PgPool) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT COALESCE(shared_inbox, inbox) FROM ap_followers ORDER BY actor_id",
    )
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().map(|(inbox,)| inbox).collect())
}

pub async fn count_followers(conn: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM ap_followers")
        .fetch_one(conn)
        .await
}

pub async fn upsert_alert_follower(
    actor_id: &str,
    inbox: &str,
    shared_inbox: Option<&str>,
    display_name: Option<&str>,
    follow_activity_id: Option<&str>,
    conn: &PgPool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO ap_alert_followers
               (actor_id, inbox, shared_inbox, display_name, follow_activity_id)
           VALUES ($1, $2, $3, $4, $5)
           ON CONFLICT (actor_id) DO UPDATE SET
               inbox = EXCLUDED.inbox,
               shared_inbox = EXCLUDED.shared_inbox,
               display_name = EXCLUDED.display_name,
               follow_activity_id = EXCLUDED.follow_activity_id"#,
    )
    .bind(actor_id)
    .bind(inbox)
    .bind(shared_inbox)
    .bind(display_name)
    .bind(follow_activity_id)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn remove_alert_follower(
    actor_id: &str,
    conn: &PgPool,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM ap_alert_followers WHERE actor_id = $1")
        .bind(actor_id)
        .execute(conn)
        .await?;
    Ok(())
}

pub async fn remove_alert_follower_by_activity(
    follow_activity_id: &str,
    actor_id: &str,
    conn: &PgPool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM ap_alert_followers
         WHERE actor_id = $1 AND follow_activity_id = $2",
    )
    .bind(actor_id)
    .bind(follow_activity_id)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn alert_follower_inboxes(conn: &PgPool) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT COALESCE(shared_inbox, inbox) FROM ap_alert_followers ORDER BY actor_id",
    )
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().map(|(inbox,)| inbox).collect())
}

pub async fn count_alert_followers(conn: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM ap_alert_followers")
        .fetch_one(conn)
        .await
}

pub async fn get_keypair(
    conn: &PgPool,
) -> Result<Option<(String, String)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT private_key_pem, public_key_pem FROM ap_keys WHERE id = 1",
    )
    .fetch_optional(conn)
    .await
}

pub async fn insert_keypair(
    private_key_pem: &str,
    public_key_pem: &str,
    conn: &PgPool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO ap_keys (id, private_key_pem, public_key_pem)
           VALUES (1, $1, $2)
           ON CONFLICT (id) DO NOTHING"#,
    )
    .bind(private_key_pem)
    .bind(public_key_pem)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn get_alert_keypair(
    conn: &PgPool,
) -> Result<Option<(String, String)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT private_key_pem, public_key_pem FROM ap_alert_keys WHERE id = 1",
    )
    .fetch_optional(conn)
    .await
}

pub async fn insert_alert_keypair(
    private_key_pem: &str,
    public_key_pem: &str,
    conn: &PgPool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO ap_alert_keys (id, private_key_pem, public_key_pem)
           VALUES (1, $1, $2)
           ON CONFLICT (id) DO NOTHING"#,
    )
    .bind(private_key_pem)
    .bind(public_key_pem)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn enqueue_delivery(
    activity_id: &str,
    inbox: &str,
    payload: &str,
    conn: &PgPool,
) -> Result<(), sqlx::Error> {
    enqueue_delivery_for("main", activity_id, inbox, payload, conn).await
}

pub async fn enqueue_delivery_for(
    account: &str,
    activity_id: &str,
    inbox: &str,
    payload: &str,
    conn: &PgPool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO ap_delivery_queue (account, activity_id, inbox, payload)
           VALUES ($1, $2, $3, $4)
           ON CONFLICT (activity_id, inbox) DO NOTHING"#,
    )
    .bind(account)
    .bind(activity_id)
    .bind(inbox)
    .bind(payload)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn due_deliveries(
    limit: i64,
    conn: &PgPool,
) -> Result<Vec<Delivery>, sqlx::Error> {
    sqlx::query_as(
        r#"SELECT id, inbox, payload, attempts, account
           FROM ap_delivery_queue
           WHERE next_attempt_at <= NOW()
           ORDER BY id
           LIMIT $1"#,
    )
    .bind(limit)
    .fetch_all(conn)
    .await
}

pub async fn delivery_succeeded(id: i64, conn: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM ap_delivery_queue WHERE id = $1")
        .bind(id)
        .execute(conn)
        .await?;
    Ok(())
}

pub async fn delivery_failed(
    id: i64,
    attempts: i32,
    error: &str,
    conn: &PgPool,
) -> Result<(), sqlx::Error> {
    // Backoff exponentiel : 1 min, 2 min, 4 min, ... plafonné à 6 h.
    // Après 10 tentatives, on abandonne la livraison.
    if attempts >= 10 {
        sqlx::query("DELETE FROM ap_delivery_queue WHERE id = $1")
            .bind(id)
            .execute(conn)
            .await?;
    } else {
        let delay_seconds = 60i64 * (1i64 << attempts.min(9));
        sqlx::query(
            r#"UPDATE ap_delivery_queue
               SET attempts = $2,
                   last_error = $3,
                   next_attempt_at = NOW() + ($4::bigint * INTERVAL '1 second')
               WHERE id = $1"#,
        )
        .bind(id)
        .bind(attempts)
        .bind(error)
        .bind(delay_seconds)
        .execute(conn)
        .await?;
    }
    Ok(())
}
