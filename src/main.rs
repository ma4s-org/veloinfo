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
use crate::db::city_snow::city_snow;
use crate::score_selector_controler::score_bounds_controler;
use askama::Template;
use askama_web::WebTemplate;
use axum::extract::DefaultBodyLimit;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::Request;
use axum::routing::post;
use axum::routing::{get, Router};
use component::route_panel::route;
use component::style::style;
use db::city_snow::{city_snow_mvt, post_city_snow};
use db::edge::Edge;
use lazy_static::lazy_static;
use sqlx::PgPool;
use std::env;
use std::process::exit;
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

async fn service_worker_js() -> impl axum::response::IntoResponse {
    let content = tokio::fs::read_to_string("pub/service-worker.js")
        .await
        .unwrap_or_default();
    (
        [
            ("Content-Type", "application/javascript; charset=utf-8"),
            ("Service-Worker-Allowed", "/"),
        ],
        content,
    )
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

    println!("Starting cron scheduler");
    let sched = JobScheduler::new().await.unwrap();

    if !dev {
        let conn = conn.clone();
        tokio::spawn(async move {
            if std::path::Path::new("lock/import").exists() {
                std::fs::remove_file("lock/import").unwrap();
                import(&conn).await;
            }
            Edge::clear_cache_and_reload(&conn).await;
            sched
                .add(
                    Job::new_tz(
                        "0 0 3 * * *",
                        chrono_tz::America::Montreal,
                        move |_uuid, _l| {
                            std::fs::File::create("lock/import").unwrap();
                            exit(0);
                        },
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
            sched.start().await.unwrap();
        });
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
        .route("/city_snow_edit", post(post_city_snow))
        .route("/city_snow", get(city_snow))
        .route("/city_snow/{z}/{x}/{y}", get(city_snow_mvt))
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
        .route("/pub/service-worker.js", get(service_worker_js))
        .nest_service("/dist/", ServeDir::new("dist"))
        .nest_service("/pub/", ServeDir::new("pub"))
        .nest_service("/custom-elements/", ServeDir::new("custom-elements"))
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
