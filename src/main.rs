// Importation des différents modules et composants de l'application
use crate::auth::auth;
use crate::auth::logout;
use crate::component::bike_path::bike_path;
use crate::component::bike_path::bike_path_mvt;
use crate::component::info_panel::info_panel_up;
use crate::component::photo_scroll::photo_scroll;
use crate::component::point_panel::point_panel_lng_lat;
use crate::component::route_panel::recalculate_route;
use crate::component::route_verte::route_verte;
use crate::component::route_verte::route_verte_mvt;
use crate::component::route_verte::route_verte_stats;
use crate::component::search;
use crate::component::segment_panel::segment_between;
use crate::component::segment_panel::segment_panel_edit_post;
use crate::component::segment_panel::segment_panel_lng_lat;
use crate::component::segment_panel::segment_panel_post;
use crate::component::segment_panel::select_report_id;
use crate::component::segment_panel::get_user_name_endpoint;
use crate::component::segment_panel::toggle_report;
use crate::component::segment_panel::report_mvt;
use crate::component::segment_panel::report;
use crate::component::segment_panel::report_reply_post;
use crate::db::city_snow::city_snow;
use crate::score_selector_controler::report_bounds_controler;
use crate::utils::ddo;
use crate::utils::dorval;
use crate::utils::drummondville;
use crate::utils::gatineau;
use crate::utils::hampstead;
use crate::utils::import_status;
use crate::utils::laval;
use crate::utils::levis;
use crate::utils::longueuil;
use crate::utils::mont_royal;
use crate::utils::mtl;
use crate::utils::quebec;
use crate::utils::repentigny;
use crate::utils::saguenay;
use crate::utils::sherbrooke;
use crate::utils::trois_rivieres;
use crate::utils::westmount;
use crate::utils::proxy::martin_proxy;
use askama::Template;
use askama_web::WebTemplate;
use axum::extract::DefaultBodyLimit;
use axum::http::HeaderMap;
use axum::http::HeaderValue;

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
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use utils::import::import;

// Définition des modules internes
mod activity_pub;
mod auth;
mod component;
mod db;
mod score_selector_controler;
mod utils;

// Variables statiques chargées une seule fois au démarrage depuis l'environnement
lazy_static! {
    static ref IMAGE_DIR: String = env::var("IMAGE_DIR").expect("IMAGE_DIR must be set");
    static ref MATOMO_SERVER: String =
        env::var("MATOMO_SERVER").expect("MATOMO_SERVER must be set");
}

/// État partagé de l'application (accessible dans les handlers)
#[derive(Clone, Debug)]
struct VeloinfoState {
    conn: PgPool, // Pool de connexion à la base de données PostgreSQL
}

/// Sources suivies par le rapport d'erreurs d'import.
const ROAD_WORK_SOURCES: &[&str] = &[
    "montreal",
    "westmount",
    "quebec",
    "laval",
    "levis",
    "dorval",
    "hampstead",
    "mont-royal",
    "ddo",
    "gatineau",
    "sherbrooke",
    "longueuil",
    "trois-rivieres",
    "saguenay",
    "repentigny",
    "drummondville",
];

/// Rafraîchit toutes les sources de travaux routiers puis recharge le cache
/// des segments, qui mémorise l'état « travaux » de chaque rue.
async fn fetch_all_road_works(conn: &PgPool) {
    import_status::reset();

    mtl::fetch_montreal_data(conn).await;
    westmount::fetch_westmount_data(conn).await;
    quebec::fetch_quebec_data(conn).await;
    laval::fetch_laval_data(conn).await;
    levis::fetch_levis_data(conn).await;
    dorval::fetch_dorval_data(conn).await;
    hampstead::fetch_hampstead_data(conn).await;
    mont_royal::fetch_mont_royal_data(conn).await;
    ddo::fetch_ddo_data(conn).await;
    gatineau::fetch_gatineau_data(conn).await;
    sherbrooke::fetch_sherbrooke_data(conn).await;
    longueuil::fetch_longueuil_data(conn).await;
    trois_rivieres::fetch_trois_rivieres_data(conn).await;
    saguenay::fetch_saguenay_data(conn).await;
    repentigny::fetch_repentigny_data(conn).await;
    drummondville::fetch_drummondville_data(conn).await;

    let failures = import_status::failures(ROAD_WORK_SOURCES);
    if !failures.is_empty() {
        println!("Road work import failures: {}", failures.join(", "));
        if let Err(error) = activity_pub::publish_import_failures(conn, &failures).await {
            eprintln!("ActivityPub: publication des erreurs d'import: {:#}", error);
        }
    }

    Edge::clear_cache_and_reload(conn).await;
}

#[tokio::main]
async fn main() {
    // Détection du mode développement
    let dev = env::var("ENV")
        .unwrap_or_else(|_| "prod".into())
        .as_str()
        .contains("dev");

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

    // Livraison des activités ActivityPub en arrière-plan
    tokio::spawn(activity_pub::run_delivery_worker(conn.clone()));

    // Import ponctuel des travaux routiers, utile en développement où le
    // planificateur de production n'est pas actif :
    //   cargo run -- --fetch-road-work [quebec|westmount|montreal|laval|levis|dorval|hampstead|montroyal|ddo|gatineau|sherbrooke|longueuil|trois-rivieres|saguenay|repentigny|drummondville]
    // Sans argument de source, toutes les sources sont importées.
    if let Some(position) = env::args().position(|arg| arg == "--fetch-road-work") {
        match env::args().nth(position + 1).as_deref() {
            Some("quebec") => quebec::fetch_quebec_data(&conn).await,
            Some("westmount") => westmount::fetch_westmount_data(&conn).await,
            Some("montreal") => mtl::fetch_montreal_data(&conn).await,
            Some("laval") => laval::fetch_laval_data(&conn).await,
            Some("levis") => levis::fetch_levis_data(&conn).await,
            Some("dorval") => dorval::fetch_dorval_data(&conn).await,
            Some("hampstead") => hampstead::fetch_hampstead_data(&conn).await,
            Some("montroyal") => mont_royal::fetch_mont_royal_data(&conn).await,
            Some("ddo") => ddo::fetch_ddo_data(&conn).await,
            Some("gatineau") => gatineau::fetch_gatineau_data(&conn).await,
            Some("sherbrooke") => sherbrooke::fetch_sherbrooke_data(&conn).await,
            Some("longueuil") => longueuil::fetch_longueuil_data(&conn).await,
            Some("trois-rivieres") => trois_rivieres::fetch_trois_rivieres_data(&conn).await,
            Some("saguenay") => saguenay::fetch_saguenay_data(&conn).await,
            Some("repentigny") => repentigny::fetch_repentigny_data(&conn).await,
            Some("drummondville") => drummondville::fetch_drummondville_data(&conn).await,
            _ => {
                import_status::reset();
                mtl::fetch_montreal_data(&conn).await;
                westmount::fetch_westmount_data(&conn).await;
                quebec::fetch_quebec_data(&conn).await;
                laval::fetch_laval_data(&conn).await;
                levis::fetch_levis_data(&conn).await;
                dorval::fetch_dorval_data(&conn).await;
                hampstead::fetch_hampstead_data(&conn).await;
                mont_royal::fetch_mont_royal_data(&conn).await;
                ddo::fetch_ddo_data(&conn).await;
                gatineau::fetch_gatineau_data(&conn).await;
                sherbrooke::fetch_sherbrooke_data(&conn).await;
                longueuil::fetch_longueuil_data(&conn).await;
                trois_rivieres::fetch_trois_rivieres_data(&conn).await;
                saguenay::fetch_saguenay_data(&conn).await;
                repentigny::fetch_repentigny_data(&conn).await;
                drummondville::fetch_drummondville_data(&conn).await;

                let failures = import_status::failures(ROAD_WORK_SOURCES);
                if !failures.is_empty() {
                    println!("Sources en échec : {}", failures.join(", "));
                }
            }
        }
        return;
    }

    // Configuration du planificateur de tâches (cron)
    println!("Starting cron scheduler");

    // Copie conservée pour le rafraîchissement des travaux au démarrage.
    let conn_startup = conn.clone();

    // En production, on gère l'import automatique des données et le cache
    if !dev {
        let sched_restart = JobScheduler::new().await.unwrap();
        sched_restart
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
        sched_restart.start().await.unwrap();

        let sched_road_work = JobScheduler::new().await.unwrap();
        sched_road_work
            .add(
                Job::new_tz(
                    "0 0 1 * * *",
                    chrono_tz::America::Montreal,
                    move |_uuid, _lock| {
                        let conn_clone = conn.clone();
                        tokio::spawn(async move {
                            fetch_all_road_works(&conn_clone).await;
                        });
                    },
                )
                .unwrap(),
            )
            .await
            .unwrap();
        sched_road_work.start().await.unwrap();
    }

    // Rafraîchit les travaux dès le démarrage, sans attendre le cron de 1 h.
    // En production, si un import OSM est demandé (fichier lock), on attend
    // qu'il soit terminé avant de recaler les travaux sur le réseau routier.
    tokio::spawn(async move {
        if !dev && std::path::Path::new("lock/import").exists() {
            std::fs::remove_file("lock/import").unwrap();
            import(&conn_startup).await;
        }
        fetch_all_road_works(&conn_startup).await;
    });

    // Définition du routeur Axum
    let app = Router::new()
        // Routes principales
        .route("/", get(index))
        .route("/auth", get(auth))
        .route("/logout", get(logout))
        // Panels d'information et de segments
        .route(
            "/info_panel/up/{lng1}/{lat1}/{lng2}/{lat2}",
            get(info_panel_up),
        )
        .route("/segment_panel/id/{id}", get(select_report_id))
        .route("/user_name", get(get_user_name_endpoint))
        .route("/report/toggle/{id}", post(toggle_report))
        .route(
            "/segment_panel_lng_lat/{lng}/{lat}",
            get(segment_panel_lng_lat),
        )
        .route("/segment_panel", post(segment_panel_post))
        .route("/segment_panel/edit", post(segment_panel_edit_post))
        .route("/report/reply", post(report_reply_post))
        .route(
            "/segment_between/{start_lng}/{start_lat}/{end_lng}/{end_lat}",
            get(segment_between),
        )
        // Gestion de la neige
        .route("/city_snow_edit", post(post_city_snow))
        .route("/city_snow", get(city_snow))
        .route("/city_snow/{z}/{x}/{y}", get(city_snow_mvt))
        // Pistes cyclables et tuiles vectorielles (MVT)
        .route("/bike_path", get(bike_path))
        .route("/bike_path/{z}/{x}/{y}", get(bike_path_mvt))
        // Routes vertes (réseau officiel Route Verte du Québec)
        .route("/route_verte", get(route_verte))
        .route("/route_verte/{z}/{x}/{y}", get(route_verte_mvt))
        .route(
            "/route_verte/stats/{min_lng}/{min_lat}/{max_lng}/{max_lat}",
            get(route_verte_stats),
        )
        // Recherche et calcul d'itinéraires
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
        // Divers (scores, photos, style mapbox)
        .route(
            "/report/geom/{report_id}",
            get(report_bounds_controler),
        )
        .route("/photo_scroll/{photo}/{way_ids}", get(photo_scroll))
        .route("/style.json", get(style))
        .route("/martin/{*path}", get(martin_proxy)) // Proxy pour le serveur de tuiles Martin
        // Tuiles vectorielles report
        .route("/report", get(report))
        .route("/report/{z}/{x}/{y}", get(report_mvt))
        .route("/pub/service-worker.js", get(service_worker_js))
        .route("/health-check", get(|| async { "ok" }))
        .route("/version", get(version))
        // ActivityPub / Fédivers
        .route("/.well-known/webfinger", get(activity_pub::webfinger))
        .route("/ap/actor", get(activity_pub::actor))
        .route("/ap/actor/inbox", post(activity_pub::inbox))
        .route("/ap/actor/followers", get(activity_pub::followers))
        .route("/ap/actor/outbox", get(activity_pub::outbox))
        .route("/ap/alert-actor", get(activity_pub::alert_actor))
        .route("/ap/alert-actor/inbox", post(activity_pub::alert_inbox))
        .route("/ap/alert-actor/followers", get(activity_pub::alert_followers))
        .route("/ap/alert-actor/outbox", get(activity_pub::alert_outbox))
        .route("/ap/inbox", post(activity_pub::inbox))
        .route("/ap/comment/{id}", get(activity_pub::note))
        .route(
            "/ap/import-error/{id}",
            get(activity_pub::import_error_note),
        )
        .route("/ap/report/{id}", get(activity_pub::report_context))
        .route("/report/{id}", get(activity_pub::report_redirect))
        .route("/report/{id}/map.png", get(activity_pub::report_map))
        .route("/report/{id}/map-render", get(activity_pub::report_render_page))
        .route("/fediverse", get(activity_pub::fediverse_page))
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

    // Lancement du serveur sur le port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Structure du template pour la page d'accueil (index.html)
#[derive(Template, WebTemplate)]
#[template(path = "index.html", escape = "none")]
pub struct IndexTemplate {
    pub matomo_server: String,
}

/// Handler pour la page d'accueil
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

async fn version() -> &'static str {
    "1.2"
}
