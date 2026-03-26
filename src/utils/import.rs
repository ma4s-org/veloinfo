use crate::{db::edge::Edge, utils::mtl, VeloinfoState};
use axum::extract::State;
use sqlx::PgPool;
use tokio::process::Command;

pub async fn import(conn: &PgPool) {
    println!("Importing data");
    let conn_clone = conn.clone();
    tokio::spawn(async move {
        println!("fetching montreal data");
        mtl::fetch_montreal_data(&conn_clone).await;
        println!("fetching montreal data done");
    });
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

#[allow(dead_code)]
pub async fn import_mtl(State(state): State<VeloinfoState>) {
    import(&state.conn).await;
}
