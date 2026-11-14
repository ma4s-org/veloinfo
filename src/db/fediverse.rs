use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Application OAuth enregistrée sur une instance du Fédivers.
#[derive(Clone, sqlx::FromRow)]
pub struct FediverseApp {
    pub client_id: String,
    pub client_secret: String,
}

impl FediverseApp {
    pub async fn get(instance: &str, conn: &PgPool) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as(
            "SELECT client_id, client_secret
             FROM fediverse_apps
             WHERE instance = $1",
        )
        .bind(instance)
        .fetch_optional(conn)
        .await
    }

    pub async fn upsert(
        instance: &str,
        client_id: &str,
        client_secret: &str,
        conn: &PgPool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO fediverse_apps (instance, client_id, client_secret)
               VALUES ($1, $2, $3)
               ON CONFLICT (instance) DO UPDATE SET
                   client_id = EXCLUDED.client_id,
                   client_secret = EXCLUDED.client_secret"#,
        )
        .bind(instance)
        .bind(client_id)
        .bind(client_secret)
        .execute(conn)
        .await?;
        Ok(())
    }

    /// Oublie l'application enregistrée, par exemple si l'instance a révoqué
    /// ses identifiants. Le prochain parcours OAuth la réenregistrera.
    pub async fn delete(instance: &str, conn: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM fediverse_apps WHERE instance = $1")
            .bind(instance)
            .execute(conn)
            .await?;
        Ok(())
    }
}

/// Compte Fédivers relié à un utilisateur local.
#[allow(dead_code)]
#[derive(Clone, sqlx::FromRow)]
pub struct FediverseAccount {
    pub user_id: Uuid,
    pub instance: String,
    pub account_id: String,
    pub acct: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub profile_url: Option<String>,
    pub access_token: String,
    pub scope: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FediverseAccount {
    pub async fn get_by_user_id(
        user_id: Uuid,
        conn: &PgPool,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as(
            r#"SELECT user_id, instance, account_id, acct, username, display_name,
                      avatar_url, profile_url, access_token, scope,
                      created_at, updated_at
               FROM fediverse_accounts
               WHERE user_id = $1"#,
        )
        .bind(user_id)
        .fetch_optional(conn)
        .await
    }

    /// Relie (ou met à jour) le compte Fédivers d'un utilisateur local.
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert(
        user_id: Uuid,
        instance: &str,
        account_id: &str,
        acct: &str,
        username: Option<&str>,
        display_name: Option<&str>,
        avatar_url: Option<&str>,
        profile_url: Option<&str>,
        access_token: &str,
        scope: Option<&str>,
        conn: &PgPool,
    ) -> Result<(), sqlx::Error> {
        let mut tx = conn.begin().await?;

        // Si le même compte distant était relié à un autre utilisateur local
        // (par exemple après l'effacement des cookies), on remplace le lien.
        sqlx::query(
            "DELETE FROM fediverse_accounts
             WHERE instance = $1 AND account_id = $2 AND user_id <> $3",
        )
        .bind(instance)
        .bind(account_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"INSERT INTO fediverse_accounts
                   (user_id, instance, account_id, acct, username, display_name,
                    avatar_url, profile_url, access_token, scope)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               ON CONFLICT (user_id) DO UPDATE SET
                   instance = EXCLUDED.instance,
                   account_id = EXCLUDED.account_id,
                   acct = EXCLUDED.acct,
                   username = EXCLUDED.username,
                   display_name = EXCLUDED.display_name,
                   avatar_url = EXCLUDED.avatar_url,
                   profile_url = EXCLUDED.profile_url,
                   access_token = EXCLUDED.access_token,
                   scope = EXCLUDED.scope,
                   updated_at = NOW()"#,
        )
        .bind(user_id)
        .bind(instance)
        .bind(account_id)
        .bind(acct)
        .bind(username)
        .bind(display_name)
        .bind(avatar_url)
        .bind(profile_url)
        .bind(access_token)
        .bind(scope)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_for_user(user_id: Uuid, conn: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM fediverse_accounts WHERE user_id = $1")
            .bind(user_id)
            .execute(conn)
            .await?;
        Ok(())
    }
}

/// Trace d'une publication de signalement sur un compte personnel.
#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FediversePublication {
    pub id: i64,
    pub report_id: i32,
    pub user_id: Uuid,
    pub remote_status_id: Option<String>,
    pub remote_status_url: Option<String>,
    pub state: String,
    pub attempts: i32,
    pub error: Option<String>,
}

impl FediversePublication {
    pub async fn exists(
        report_id: i32,
        user_id: Uuid,
        conn: &PgPool,
    ) -> Result<bool, sqlx::Error> {
        let value: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM fediverse_publications WHERE report_id = $1 AND user_id = $2",
        )
        .bind(report_id)
        .bind(user_id)
        .fetch_optional(conn)
        .await?;
        Ok(value.is_some())
    }

    pub async fn insert_pending(
        report_id: i32,
        user_id: Uuid,
        conn: &PgPool,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar(
            r#"INSERT INTO fediverse_publications (report_id, user_id, state)
               VALUES ($1, $2, 'pending')
               ON CONFLICT (report_id, user_id) DO UPDATE SET
                   state = 'pending',
                   updated_at = NOW()
               RETURNING id"#,
        )
        .bind(report_id)
        .bind(user_id)
        .fetch_one(conn)
        .await
    }

    pub async fn mark_published(
        id: i64,
        remote_status_id: &str,
        remote_status_url: &str,
        conn: &PgPool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE fediverse_publications
               SET state = 'published',
                   remote_status_id = $2,
                   remote_status_url = $3,
                   error = NULL,
                   attempts = attempts + 1,
                   updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(id)
        .bind(remote_status_id)
        .bind(remote_status_url)
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn mark_failed(
        id: i64,
        error: &str,
        conn: &PgPool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE fediverse_publications
               SET state = 'failed',
                   error = $2,
                   attempts = attempts + 1,
                   updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(id)
        .bind(error)
        .execute(conn)
        .await?;
        Ok(())
    }
}
