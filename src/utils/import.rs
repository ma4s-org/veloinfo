use crate::{db::edge::Edge, utils::mtl, VeloinfoState};
use axum::extract::State;
use sqlx::PgPool;
use std::process::Command;

pub async fn import(conn: &PgPool) {
    println!("Importing data");
    let output = Command::new("./import.sh")
        .output()
        .expect("failed to execute process");
    println!("status: {}", output.status);

    // clearing cache
    println!("fetching montreal data");
    mtl::fetch_montreal_data(&conn).await;
    println!("fetching montreal data done");
    println!("clearing cache");
    Edge::clear_cache(&conn).await;
}

pub async fn import_mtl(State(state): State<VeloinfoState>) {
    mtl::fetch_montreal_data(&state.conn).await;
}
