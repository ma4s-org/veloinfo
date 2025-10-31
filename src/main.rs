use crate::auth::auth;
use crate::auth::logout;
use crate::component::bike_path::bike_path;
use crate::component::bike_path::bike_path_mvt;
use crate::component::info_panel::info_panel_down;
use crate::component::info_panel::info_panel_up;
use crate::component::photo_scroll::photo_scroll;
use crate::component::point_panel::point_panel_lng_lat;
use crate::component::route_panel::recalculate_route;
use crate::component::search;
use crate::component::segment_panel::segment_panel_bigger_route;
use crate::component::segment_panel::segment_panel_edit;
use crate::component::segment_panel::segment_panel_get;
use crate::component::segment_panel::segment_panel_lng_lat;
use crate::component::segment_panel::segment_panel_post;
use crate::component::segment_panel::select_score_id;
use crate::score_selector_controler::score_bounds_controler;
use askama::Template;
use askama_web::WebTemplate;
use axum::extract::DefaultBodyLimit;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::Request;
use axum::routing::post;
use axum::routing::{get, Router};
use component::layers;
use component::route_panel::route;
use component::style::style;
use db::city_snow::{city_snow_geojson, post_city_snow};
use db::edge::Edge;
use lazy_static::lazy_static;
use sqlx::PgPool;
use std::env;
use tokio_cron_scheduler::{Job, JobScheduler};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tower_livereload::LiveReloadLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use utils::import::import;

mod auth;
mod component;
mod db;
mod score_selector_controler;
mod utils;

lazy_static! {
    static ref IMAGE_DIR: String = env::var("IMAGE_DIR").unwrap();
    static ref MATOMO_SERVER: String = env::var("MATOMO_SERVER").unwrap();
}

#[derive(Clone, Debug)]
struct VeloinfoState {
    conn: PgPool,
}

#[tokio::main]
async fn main() {
    let dev = env::var("ENV").unwrap().as_str().contains("dev");
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_http_proxy=trace,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let conn = PgPool::connect(format!("{}", env::var("DATABASE_URL").unwrap()).as_str())
        .await
        .unwrap();
    let state = VeloinfoState { conn: conn.clone() };
    sqlx::migrate!().run(&conn).await.unwrap();

    if !dev {
        Edge::clear_cache_and_reload(&conn).await;
    }

    println!("Starting cron scheduler");
    let sched = JobScheduler::new().await.unwrap();

    if !dev {
        sched
            .add(
                Job::new("0 0 7 * * *", move |_uuid, _l| {
                    // tokio spawn
                    let conn = conn.clone();
                    tokio::spawn(async move {
                        import(&conn).await;
                    });
                })
                .unwrap(),
            )
            .await
            .unwrap();
        sched.start().await.unwrap();
    }

    let mut app = Router::new()
        .route("/", get(index))
        .route("/auth", get(auth))
        .route("/logout", get(logout))
        .route("/info_panel/down", get(info_panel_down))
        .route(
            "/info_panel/up/{lng1}/{lat1}/{lng2}/{lat2}",
            get(info_panel_up),
        )
        .route("/segment_panel/id/{id}", get(select_score_id))
        .route(
            "/segment_panel_lng_lat/{lng}/{lat}",
            get(segment_panel_lng_lat),
        )
        .route("/segment_panel/ways/{way_ids}", get(segment_panel_get))
        .route(
            "/segment_panel/edit/ways/{way_ids}",
            get(segment_panel_edit),
        )
        .route("/segment_panel", post(segment_panel_post))
        .route(
            "/segment_panel_bigger/{start_lng}/{start_lat}/{end_lng}/{end_lat}",
            get(segment_panel_bigger_route),
        )
        .route("/city_snow", post(post_city_snow))
        .route("/city_snow_geojson", get(city_snow_geojson))
        .route("/bike_path", get(bike_path))
        .route("/bike_path/{z}/{x}/{y}", get(bike_path_mvt))
        .route("/point_panel_lng_lat/{lng}/{lat}", get(point_panel_lng_lat))
        .route("/search", post(search::post))
        .route(
            "/route/{start_lng}/{start_lat}/{end_lgt}/{end_lat}",
            get(route),
        )
        .route(
            "/recalculate_route/{route}/{start_lng}/{start_lat}/{end_lgt}/{end_lat}",
            get(recalculate_route),
        )
        .route(
            "/cyclability_score/geom/{cyclability_score_id}",
            get(score_bounds_controler),
        )
        .route("/photo_scroll/{photo}/{way_ids}", get(photo_scroll))
        .route("/style.json", get(style))
        .route("/layers", get(layers::layers))
        .nest_service("/dist/", ServeDir::new("dist"))
        .nest_service("/pub/", ServeDir::new("pub"))
        .nest_service("/images/", ServeDir::new(IMAGE_DIR.as_str()))
        .nest_service("/node_modules/", ServeDir::new("node_modules"))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(1024 * 1024 * 10));

    if dev {
        let livereload = LiveReloadLayer::new();
        app = app.layer(livereload.request_predicate(not_htmx_predicate));
    }

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn not_htmx_predicate<T>(req: &Request<T>) -> bool {
    !req.headers().contains_key("hx-request")
}

#[derive(Template, WebTemplate)]
#[template(path = "index.html", escape = "none")]
pub struct IndexTemplate {
    pub matomo_server: String,
}

#[axum::debug_handler]
pub async fn index() -> (HeaderMap, IndexTemplate) {
    let template = IndexTemplate {
        matomo_server: MATOMO_SERVER.clone(),
    };
    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    (headers, template)
}
