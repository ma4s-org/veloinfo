use crate::{db::edge::Edge, utils::mtl, VeloinfoState};
use axum::extract::State;
use sqlx::PgPool;
use std::process::Command;

pub async fn import(conn: &PgPool) {
    Edge::clear_all_cache().await;
    println!("Importing data");
    match Command::new("./import.sh").output() {
        Ok(_) => {}
        Err(e) => {
            println!("Error importing: {:?}", e);
        }
    }

    // clearing cache
    println!("fetching montreal data");
    mtl::fetch_montreal_data(&conn).await;
    println!("fetching montreal data done");
    println!("clearing cache");
    Edge::clear_cache_and_reload(&conn).await;
}

#[allow(dead_code)]
pub async fn import_mtl(State(state): State<VeloinfoState>) {
    import(&state.conn).await;
}
