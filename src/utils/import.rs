use crate::db::edge::Edge;
use sqlx::PgPool;
use tokio::process::Command;

pub async fn import(conn: &PgPool) {
    println!("Importing data");
    match Command::new("./import.sh").output().await {
        Ok(output) => {
            if !output.status.success() {
                println!(
                    "Error1 importing: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            println!("{}", String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => {
            println!("Error2 importing: {:?}", e);
        }
    }
    println!("clearing cache");
    Edge::clear_cache_and_reload(&conn).await;
}
