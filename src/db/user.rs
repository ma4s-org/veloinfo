use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct User {
    pub name: String,
}

impl User {
    pub async fn insert(id: &Uuid, name: &String, conn: &PgPool) {
        match sqlx::query(
            r#"
            INSERT INTO users (id, name)
            VALUES ($1, $2)"#,
        )
        .bind(id)
        .bind(name)
        .execute(conn)
        .await
        {
            Ok(_) => (),
            Err(e) => eprintln!("Error inserting user: {}", e),
        };
    }

    pub async fn update(id: &Uuid, name: &String, conn: &PgPool) {
        match sqlx::query(
            r#"
            UPDATE users
            SET name = $2
            WHERE id = $1"#,
        )
        .bind(id)
        .bind(name)
        .execute(conn)
        .await
        {
            Ok(_) => (),
            Err(e) => eprintln!("Error updating user: {}", e),
        };
    }

    pub async fn get(id: &Uuid, conn: &PgPool) -> Option<User> {
        match sqlx::query_as(
            r#"
            SELECT id, name
            FROM users
            WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(conn)
        .await
        {
            Ok(user) => user,
            Err(e) => {
                eprintln!("Error fetching user: {}", e);
                None
            }
        }
    }
}
