// Importation des différents modules et composants de l'application
use crate::auth::auth;
use crate::auth::logout;
use crate::component::bike_path::bike_path;
use crate::component::bike_path::bike_path_mvt;
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
use crate::utils::proxy::martin_proxy;
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

// Définition des modules internes
mod auth;
mod component;
mod db;
mod score_selector_controler;
mod utils;

// Variables statiques chargées une seule fois au démarrage depuis l'environnement
lazy_static! {
    static ref IMAGE_DIR: String = env::var("IMAGE_DIR").expect("IMAGE_DIR must be set");
    static ref MATOMO_SERVER: String = env::var("MATOMO_SERVER").expect("MATOMO_SERVER must be set");
}

/// État partagé de l'application (accessible dans les handlers)
#[derive(Clone, Debug)]
struct VeloinfoState {
    conn: PgPool, // Pool de connexion à la base de données PostgreSQL
}

#[tokio::main]
async fn main() {
    // Détection du mode développement
    let dev = env::var("ENV").unwrap_or_else(|_| "prod".into()).as_str().contains("dev");

    // Initialisation du logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_http_proxy=trace,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Connexion à la base de données PostgreSQL
    let conn = PgPool::connect(&env::var("DATABASE_URL").expect("DATABASE_URL must be set"))
        .await
        .unwrap();
    let state = VeloinfoState { conn: conn.clone() };

    // Exécution des migrations SQL au démarrage
    sqlx::migrate!().run(&conn).await.unwrap();

    // Configuration du planificateur de tâches (cron)
    println!("Starting cron scheduler");
    let sched = JobScheduler::new().await.unwrap();

    // En production, on gère l'import automatique des données et le cache
    if !dev {
        let conn = conn.clone();
        tokio::spawn(async move {
            // Si un fichier lock existe, on relance l'import
            if std::path::Path::new("lock/import").exists() {
                std::fs::remove_file("lock/import").unwrap();
                import(&conn).await;
            }
            // Rechargement du cache des segments (edges)
            Edge::clear_cache_and_reload(&conn).await;
            
            // Tâche planifiée : tous les jours à 3h du matin (heure de Montréal)
            // On crée un fichier lock et on quitte l'application (elle sera redémarrée par Docker/Systemd)
            sched
                .add(
                    Job::new_tz(
                        "0 0 0 * * *",
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

    // Définition du routeur Axum
    let mut app = Router::new()
        // Routes principales
        .route("/", get(index))
        .route("/auth", get(auth))
        .route("/logout", get(logout))
        
        // Panels d'information et de segments
        .route("/info_panel/up/{lng1}/{lat1}/{lng2}/{lat2}", get(info_panel_up))
        .route("/segment_panel/id/{id}", get(select_score_id))
        .route("/segment_panel_lng_lat/{lng}/{lat}", get(segment_panel_lng_lat))
        .route("/segment_panel/ways/{way_ids}", get(segment_panel_get))
        .route("/segment_panel/edit/ways/{way_ids}", get(segment_panel_edit))
        .route("/segment_panel", post(segment_panel_post))
        .route("/segment_panel_bigger/{start_lng}/{start_lat}/{end_lng}/{end_lat}", get(segment_panel_bigger_route))
        
        // Gestion de la neige
        .route("/city_snow_edit", post(post_city_snow))
        .route("/city_snow", get(city_snow))
        .route("/city_snow/{z}/{x}/{y}", get(city_snow_mvt))
        
        // Pistes cyclables et tuiles vectorielles (MVT)
        .route("/bike_path", get(bike_path))
        .route("/bike_path/{z}/{x}/{y}", get(bike_path_mvt))
        
        // Recherche et calcul d'itinéraires
        .route("/point_panel_lng_lat/{lng}/{lat}", get(point_panel_lng_lat))
        .route("/search", post(search::post))
        .route("/route/{start_lng}/{start_lat}/{end_lgt}/{end_lat}", get(route))
        .route("/recalculate_route/{route}/{start_lng}/{start_lat}/{end_lgt}/{end_lat}", get(recalculate_route))
        
        // Divers (scores, photos, style mapbox)
        .route("/cyclability_score/geom/{cyclability_score_id}", get(score_bounds_controler))
        .route("/photo_scroll/{photo}/{way_ids}", get(photo_scroll))
        .route("/style.json", get(style))
        .route("/martin/{*path}", get(martin_proxy)) // Proxy pour le serveur de tuiles Martin
        .route("/pub/service-worker.js", get(service_worker_js))
        .route("/health-check", get(|| async { "ok" }))
        .route("/version", get(version))
        
        // Services de fichiers statiques
        .nest_service("/.well-known/", ServeDir::new("well-known"))
        .nest_service("/dist/", ServeDir::new("dist"))
        .nest_service("/pub/", ServeDir::new("pub"))
        .nest_service("/custom-elements/", ServeDir::new("custom-elements"))
        .nest_service("/images/", ServeDir::new(IMAGE_DIR.as_str()))
        .nest_service("/node_modules/", ServeDir::new("node_modules"))
        
        // État et middleware
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(1024 * 1024 * 10)); // Limite de 10Mo pour l'upload d'images

    // Activation du rechargement à chaud en mode dev
    if dev {
        let livereload = LiveReloadLayer::new();
        app = app.layer(livereload.request_predicate(not_htmx_predicate));
    }

    // Lancement du serveur sur le port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Prédicat pour éviter de recharger la page lors de requêtes HTMX
fn not_htmx_predicate<T>(req: &Request<T>) -> bool {
    !req.headers().contains_key("hx-request")
}

/// Structure du template pour la page d'accueil (index.html)
#[derive(Template, WebTemplate)]
#[template(path = "index.html", escape = "none")]
pub struct IndexTemplate {
    pub matomo_server: String,
    pub dev: bool,
}

/// Handler pour la page d'accueil
#[axum::debug_handler]
pub async fn index() -> (HeaderMap, IndexTemplate) {
    let dev = env::var("ENV")
        .unwrap_or_else(|_| "prod".into())
        .as_str()
        .contains("dev");
    let template = IndexTemplate {
        matomo_server: MATOMO_SERVER.clone(),
        dev,
    };
    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    (headers, template)
}

/// Handler pour servir le service worker avec les bons en-têtes
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

/// Handler pour la version
async fn version() -> &'static str {
    "1.0"
}
