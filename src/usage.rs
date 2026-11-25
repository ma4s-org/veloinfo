//! Statistiques d'usage internes, en remplacement de Matomo.
//!
//! Le suivi est volontairement minimal et sans donnée personnelle : on
//! conserve la zone de la carte consultée (centre et zoom), le type d'appareil,
//! la langue et la durée de la session. Ni l'adresse IP, ni la position GPS,
//! ni un identifiant persistant entre les visites ne sont enregistrés.

use crate::auth;
use crate::db::edge::Edge;
use crate::db::fediverse::FediverseAccount;
use crate::utils::cost::{get_h_moyen, get_h_rapid};
use crate::VeloinfoState;
use askama::Template;
use askama_web::WebTemplate;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum_extra::extract::cookie::CookieJar;
use chrono::{DateTime, Duration, NaiveDateTime, SecondsFormat, Utc};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::HashMap;
use std::env;
use std::sync::Mutex;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, BufReader};
use uuid::Uuid;

/// Nombre de géométries d'itinéraires conservées en mémoire pour l'aperçu.
const ROUTE_GEOMETRY_CACHE_LIMIT: usize = 128;

lazy_static! {
    /// Le calcul d'itinéraire est coûteux : on réutilise la géométrie entre
    /// deux survols et entre deux chargements du tableau de bord.
    static ref ROUTE_GEOMETRY_CACHE: Mutex<HashMap<(Uuid, bool), (bool, Vec<[f64; 2]>)>> =
        Mutex::new(HashMap::new());
}

/// Une session est considérée « en direct » si elle a envoyé une activité
/// dans les cinq dernières minutes.
const LIVE_WINDOW_MINUTES: i64 = 5;

/// Nombre maximum de sessions renvoyées au tableau de bord (carte).
/// La liste latérale n'en affiche qu'une partie pour rester fluide.
const SESSION_LIMIT: i64 = 20000;

#[derive(Template, WebTemplate)]
#[template(path = "usage.html", escape = "none")]
pub struct UsageTemplate;

#[derive(Template, WebTemplate)]
#[template(path = "usage_login.html", escape = "none")]
pub struct UsageLoginTemplate {
    pub admin_acct: String,
}

/// Compte Fédivers autorisé à consulter les statistiques.
fn usage_admin_acct() -> String {
    env::var("USAGE_ADMIN_ACCT")
        .unwrap_or_else(|_| "MartinNHamel@mastodon.social".to_string())
}

/// Normalise `@utilisateur@instance`, `utilisateur@instance` ou une URL de
/// profil en `utilisateur@instance` minuscule.
fn normalize_acct(value: &str) -> String {
    let value = value.trim();
    if let Ok(url) = reqwest::Url::parse(value) {
        if let Some(host) = url.host_str() {
            if let Some(segment) = url
                .path_segments()
                .and_then(|segments| segments.filter(|part| !part.is_empty()).last())
            {
                return format!("{}@{}", segment.trim_start_matches('@'), host)
                    .to_lowercase();
            }
        }
    }
    value.trim_start_matches('@').to_lowercase()
}

fn usage_admin_label() -> String {
    format!("@{}", normalize_acct(&usage_admin_acct()))
}

/// Vérifie que la session navigateur est reliée au compte administrateur.
async fn is_usage_admin(state: &VeloinfoState, jar: &CookieJar) -> bool {
    let expected = normalize_acct(&usage_admin_acct());
    let mut candidates = Vec::new();

    if let Some(id) = auth::get_user_id_from_jar(jar) {
        candidates.push(id);
    }
    if let Some(cookie) = jar.get("uuid") {
        if let Ok(id) = Uuid::parse_str(cookie.value()) {
            if !candidates.contains(&id) {
                candidates.push(id);
            }
        }
    }

    for user_id in candidates {
        if let Ok(Some(account)) = FediverseAccount::get_by_user_id(user_id, &state.conn).await {
            if normalize_acct(&account.acct) == expected {
                return true;
            }
        }
    }
    false
}

/// Tableau de bord, réservé à l'administrateur connecté au Fédivers.
#[axum::debug_handler]
pub async fn usage_page(
    State(state): State<VeloinfoState>,
    jar: CookieJar,
) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0, must-revalidate"),
    );

    if !is_usage_admin(&state, &jar).await {
        return (
            headers,
            UsageLoginTemplate {
                admin_acct: usage_admin_label(),
            },
        )
            .into_response();
    }

    (headers, UsageTemplate).into_response()
}

#[derive(Debug, Deserialize, Default)]
pub struct UsageQuery {
    /// Fenêtre affichée : `live` (5 min), `24h`, `7d` ou `all`.
    window: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UsageTrafficQuery {
    /// Granularité : `day`, `week` ou `month`.
    period: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UsageRoutesQuery {
    /// Fenêtre : `24h`, `7d` ou `all`.
    window: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UsageContributionsQuery {
    /// Fenêtre : `24h`, `7d` ou `all`.
    window: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UsageRouteTrack {
    session_id: String,
    event: String,
    route_id: Option<String>,
    start_lat: Option<f64>,
    start_lng: Option<f64>,
    end_lat: Option<f64>,
    end_lng: Option<f64>,
    allow_ferry: Option<bool>,
    safe_distance_km: Option<f64>,
    fast_distance_km: Option<f64>,
    chosen_route: Option<String>,
    progress: Option<f64>,
    remaining_km: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct UsageTrack {
    session_id: String,
    event: String,
    path: Option<String>,
    referrer: Option<String>,
    language: Option<String>,
    screen_width: Option<i32>,
    screen_height: Option<i32>,
    lat: Option<f64>,
    lng: Option<f64>,
    zoom: Option<f64>,
    action: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct SessionRow {
    id: Uuid,
    started_at: DateTime<Utc>,
    last_seen: DateTime<Utc>,
    duration_seconds: f64,
    last_lat: Option<f64>,
    last_lng: Option<f64>,
    last_zoom: Option<f64>,
    path: Option<String>,
    page_views: i32,
    events: i32,
    device: Option<String>,
    browser: Option<String>,
    os: Option<String>,
    language: Option<String>,
    referrer: Option<String>,
    source: String,
    live: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct TrafficRow {
    /// Date locale de Montréal, sans fuseau.
    bucket: NaiveDateTime,
    sessions: i64,
    page_views: i64,
}

#[derive(Debug, Serialize)]
struct TrafficBucket {
    bucket: String,
    sessions: i64,
    page_views: i64,
}

#[derive(Debug, Serialize)]
struct TrafficTotals {
    sessions: i64,
    page_views: i64,
}

#[derive(Debug, Serialize)]
struct UsageTrafficResponse {
    generated_at: String,
    period: String,
    buckets: Vec<TrafficBucket>,
    totals: TrafficTotals,
}

#[derive(Debug, sqlx::FromRow)]
struct RouteRow {
    id: Uuid,
    created_at: DateTime<Utc>,
    start_lat: f64,
    start_lng: f64,
    end_lat: f64,
    end_lng: f64,
    safe_distance_km: Option<f64>,
    fast_distance_km: Option<f64>,
    chosen_route: Option<String>,
    status: String,
    progress: f64,
    remaining_km: Option<f64>,
    device: Option<String>,
    browser: Option<String>,
    os: Option<String>,
}

#[derive(Debug, Serialize)]
struct RouteJson {
    id: String,
    created_at: String,
    start_lat: f64,
    start_lng: f64,
    end_lat: f64,
    end_lng: f64,
    safe_distance_km: Option<f64>,
    fast_distance_km: Option<f64>,
    chosen_route: Option<String>,
    status: String,
    progress: f64,
    remaining_km: Option<f64>,
    device: Option<String>,
    browser: Option<String>,
    os: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct RouteStatsRow {
    requests: i64,
    followed: i64,
    completed: i64,
    abandoned: i64,
}

#[derive(Debug, Serialize)]
struct RouteStats {
    requests: i64,
    followed: i64,
    completed: i64,
    abandoned: i64,
    completion_rate: f64,
}

#[derive(Debug, Serialize)]
struct UsageRoutesResponse {
    generated_at: String,
    window: String,
    stats: RouteStats,
    routes: Vec<RouteJson>,
}

#[derive(Debug, sqlx::FromRow)]
struct ContributionRow {
    kind: String,
    id: i32,
    created_at: DateTime<Utc>,
    user_name: String,
    score: f64,
    lng: f64,
    lat: f64,
    comment: Option<String>,
    photo_path_thumbnail: Option<String>,
    report_id: i32,
    label: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ContributionStatsRow {
    reports: i64,
    comments: i64,
    photos: i64,
    contributors: i64,
}

#[derive(Debug, Serialize)]
struct ContributionJson {
    kind: String,
    id: i32,
    created_at: String,
    user_name: String,
    score: f64,
    lat: f64,
    lng: f64,
    comment: Option<String>,
    photo_path_thumbnail: Option<String>,
    report_id: i32,
    label: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ContributorRow {
    name: String,
    contributions: i64,
}

#[derive(Debug, Serialize)]
struct ContributionStats {
    reports: i64,
    comments: i64,
    photos: i64,
    contributors: i64,
}

#[derive(Debug, Serialize)]
struct UsageContributionsResponse {
    generated_at: String,
    window: String,
    stats: ContributionStats,
    top_contributors: Vec<ContributorRow>,
    contributions: Vec<ContributionJson>,
}

#[derive(Debug, sqlx::FromRow)]
struct SessionCityRow {
    id: Uuid,
    name: Option<String>,
    distance: Option<f64>,
}

#[derive(Debug, sqlx::FromRow)]
struct StatsRow {
    live: i64,
    sessions_24h: i64,
    sessions_total: i64,
    avg_duration_seconds: f64,
}

#[derive(Debug, Serialize)]
struct UsageStats {
    live: i64,
    sessions_24h: i64,
    sessions_total: i64,
    page_views_24h: i64,
    actions_24h: i64,
    avg_duration_seconds: f64,
}

#[derive(Debug, Serialize)]
struct SessionJson {
    id: String,
    started_at: String,
    last_seen: String,
    duration_seconds: f64,
    lat: Option<f64>,
    lng: Option<f64>,
    zoom: Option<f64>,
    path: Option<String>,
    page_views: i32,
    events: i32,
    device: Option<String>,
    browser: Option<String>,
    os: Option<String>,
    language: Option<String>,
    referrer: Option<String>,
    source: String,
    city: Option<String>,
    city_distance_km: Option<f64>,
    live: bool,
}

#[derive(Debug, Serialize)]
struct UsageSessionsResponse {
    generated_at: String,
    window: String,
    stats: UsageStats,
    sessions: Vec<SessionJson>,
}

fn iso(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Fenêtre glissante pour les itinéraires et les contributions.
fn range_start(window: Option<&str>, default: &str) -> (String, Option<DateTime<Utc>>) {
    match window.unwrap_or(default) {
        "24h" => ("24h".to_string(), Some(Utc::now() - Duration::hours(24))),
        "7d" => ("7d".to_string(), Some(Utc::now() - Duration::days(7))),
        "all" => ("all".to_string(), None),
        _ => (
            default.to_string(),
            Some(Utc::now() - Duration::hours(24)),
        ),
    }
}

/// Arrondit une coordonnée à ~100 m pour l'affichage public.
fn round_3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// Tronque et nettoie une chaîne fournie par le navigateur.
fn clean(value: Option<String>, max_chars: usize) -> Option<String> {
    value
        .map(|value| value.trim().chars().take(max_chars).collect::<String>())
        .filter(|value| !value.is_empty())
}

fn valid_lat(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && (-90.0..=90.0).contains(value))
}

fn valid_lng(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && (-180.0..=180.0).contains(value))
}

fn valid_zoom(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && (0.0..=24.0).contains(value))
}

/// Déduit un appareil, un navigateur et un système d'exploitation à partir de
/// l'en-tête `User-Agent`. Le User-Agent brut n'est jamais conservé.
fn detect_client(user_agent: &str) -> (String, String, String) {
    let ua = user_agent.to_lowercase();

    let device = if ua.contains("ipad")
        || ua.contains("tablet")
        || (ua.contains("android") && !ua.contains("mobile"))
    {
        "tablette"
    } else if ua.contains("mobile")
        || ua.contains("iphone")
        || ua.contains("ipod")
        || ua.contains("android")
    {
        "mobile"
    } else {
        "ordinateur"
    };

    let browser = if ua.contains("edg/") {
        "Edge"
    } else if ua.contains("opr/") || ua.contains("opera") {
        "Opera"
    } else if ua.contains("firefox") || ua.contains("fxios") {
        "Firefox"
    } else if ua.contains("chrome") || ua.contains("crios") {
        "Chrome"
    } else if ua.contains("safari") {
        "Safari"
    } else {
        "Autre"
    };

    let os = if ua.contains("android") {
        "Android"
    } else if ua.contains("iphone") || ua.contains("ipad") || ua.contains("ipod") || ua.contains("ios") {
        "iOS"
    } else if ua.contains("windows") {
        "Windows"
    } else if ua.contains("mac os") || ua.contains("macintosh") {
        "macOS"
    } else if ua.contains("linux") {
        "Linux"
    } else {
        "Autre"
    };

    (device.to_string(), browser.to_string(), os.to_string())
}

/// Enregistre une activité de session.
#[axum::debug_handler]
pub async fn usage_track(
    State(state): State<VeloinfoState>,
    headers: HeaderMap,
    Json(payload): Json<UsageTrack>,
) -> StatusCode {
    let Ok(session_id) = Uuid::parse_str(payload.session_id.trim()) else {
        return StatusCode::BAD_REQUEST;
    };

    let event = match payload.event.as_str() {
        "page_view" | "heartbeat" | "map_move" | "action" => payload.event.as_str(),
        _ => return StatusCode::BAD_REQUEST,
    };

    let path = clean(payload.path, 300);
    let referrer = clean(payload.referrer, 300);
    let language = clean(payload.language, 40);
    let action = clean(payload.action, 60);

    let screen_width = payload.screen_width.filter(|value| (1..=20000).contains(value));
    let screen_height = payload.screen_height.filter(|value| (1..=20000).contains(value));
    let lat = valid_lat(payload.lat);
    let lng = valid_lng(payload.lng);
    let zoom = valid_zoom(payload.zoom);

    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let (device, browser, os) = detect_client(user_agent);

    let page_view_increment: i32 = if event == "page_view" { 1 } else { 0 };

    let result = sqlx::query(
        r#"
        insert into usage_session (
            id, started_at, last_seen, last_lat, last_lng, last_zoom,
            path, referrer, language, device, browser, os,
            screen_width, screen_height, page_views, events
        )
        values (
            $1, now(), now(), $2, $3, $4,
            $5, $6, $7, $8, $9, $10,
            $11, $12, $13, 1
        )
        on conflict (id) do update set
            last_seen = now(),
            last_lat = coalesce(excluded.last_lat, usage_session.last_lat),
            last_lng = coalesce(excluded.last_lng, usage_session.last_lng),
            last_zoom = coalesce(excluded.last_zoom, usage_session.last_zoom),
            path = coalesce(excluded.path, usage_session.path),
            referrer = coalesce(usage_session.referrer, excluded.referrer),
            language = coalesce(usage_session.language, excluded.language),
            device = coalesce(usage_session.device, excluded.device),
            browser = coalesce(usage_session.browser, excluded.browser),
            os = coalesce(usage_session.os, excluded.os),
            screen_width = coalesce(usage_session.screen_width, excluded.screen_width),
            screen_height = coalesce(usage_session.screen_height, excluded.screen_height),
            page_views = usage_session.page_views + excluded.page_views,
            events = usage_session.events + 1
        "#,
    )
    .bind(session_id)
    .bind(lat)
    .bind(lng)
    .bind(zoom)
    .bind(&path)
    .bind(&referrer)
    .bind(&language)
    .bind(&device)
    .bind(&browser)
    .bind(&os)
    .bind(screen_width)
    .bind(screen_height)
    .bind(page_view_increment)
    .execute(&state.conn)
    .await;

    if let Err(error) = result {
        eprintln!("Usage: enregistrement de la session: {:#}", error);
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    if event == "page_view" || event == "action" {
        if let Err(error) = sqlx::query(
            r#"
            insert into usage_event (
                session_id, kind, action, path, lat, lng, zoom
            )
            values ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(session_id)
        .bind(event)
        .bind(&action)
        .bind(&path)
        .bind(lat)
        .bind(lng)
        .bind(zoom)
        .execute(&state.conn)
        .await
        {
            eprintln!("Usage: enregistrement de l'événement: {:#}", error);
        }
    }

    StatusCode::NO_CONTENT
}

/// Renvoie les sessions et les compteurs affichés sur `/usage`.
#[axum::debug_handler]
pub async fn usage_sessions(
    State(state): State<VeloinfoState>,
    jar: CookieJar,
    Query(params): Query<UsageQuery>,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    if !is_usage_admin(&state, &jar).await {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let (window, start) = match params.window.as_deref().unwrap_or("live") {
        "24h" => ("24h", Some(Utc::now() - Duration::hours(24))),
        "7d" => ("7d", Some(Utc::now() - Duration::days(7))),
        "all" => ("all", None),
        _ => (
            "live",
            Some(Utc::now() - Duration::minutes(LIVE_WINDOW_MINUTES)),
        ),
    };

    let sessions = sessions_since(&state.conn, start)
        .await
        .map_err(|error| {
            eprintln!("Usage: lecture des sessions: {:#}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // La ville n'est utile que pour les sessions listées (les 500 plus
    // récentes), pas pour tous les points de la carte.
    let cities = match session_cities(&state.conn, &sessions).await {
        Ok(cities) => cities,
        Err(error) => {
            eprintln!("Usage: recherche des villes: {:#}", error);
            HashMap::new()
        }
    };

    let stats = load_stats(&state.conn).await.map_err(|error| {
        eprintln!("Usage: lecture des statistiques: {:#}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let response = UsageSessionsResponse {
        generated_at: iso(Utc::now()),
        window: window.to_string(),
        stats,
        sessions: sessions
            .into_iter()
            .map(|session| {
                let city = cities.get(&session.id);
                SessionJson {
                    id: session.id.to_string(),
                    started_at: iso(session.started_at),
                    last_seen: iso(session.last_seen),
                    duration_seconds: session.duration_seconds.max(0.0),
                    lat: session.last_lat,
                    lng: session.last_lng,
                    zoom: session.last_zoom,
                    path: session.path,
                    page_views: session.page_views,
                    events: session.events,
                    device: session.device,
                    browser: session.browser,
                    os: session.os,
                    language: session.language,
                    referrer: session.referrer,
                    source: session.source,
                    city: city.map(|(name, _)| name.clone()),
                    city_distance_km: city.map(|(_, distance)| *distance),
                    live: session.live,
                }
            })
            .collect(),
    };

    Ok((
        [(header::CACHE_CONTROL, "no-store, max-age=0, must-revalidate")],
        Json(response),
    ))
}

/// Granularité du graphique : nombre d'intervalles affichés, bornes comprises.
fn traffic_period(period: Option<&str>) -> (&'static str, i32) {
    match period.unwrap_or("day") {
        "week" => ("week", 25),
        "month" => ("month", 35),
        _ => ("day", 29),
    }
}

/// Renvoie le trafic agrégé par jour, semaine ou mois.
#[axum::debug_handler]
pub async fn usage_traffic(
    State(state): State<VeloinfoState>,
    jar: CookieJar,
    Query(params): Query<UsageTrafficQuery>,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    if !is_usage_admin(&state, &jar).await {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let (period, intervals) = traffic_period(params.period.as_deref());

    let rows = sqlx::query_as::<_, TrafficRow>(
        r#"
        with bounds as (
            select
                case
                    -- Vue mensuelle : commencer au premier mois avec des
                    -- données, sans dépasser 36 mois d'historique.
                    when $1 = 'month' then greatest(
                        date_trunc('month', coalesce(
                            (select min(started_at at time zone 'America/Montreal')
                             from usage_session),
                            now() at time zone 'America/Montreal'
                        )),
                        date_trunc('month', now() at time zone 'America/Montreal')
                            - interval '35 months'
                    )
                    else date_trunc($1, now() at time zone 'America/Montreal')
                         - (($2::int || ' ' || $1)::interval)
                end as start,
                date_trunc($1, now() at time zone 'America/Montreal') as finish
        ),
        periods as (
            select generate_series(
                start,
                finish,
                ('1 ' || $1)::interval
            ) as bucket
            from bounds
        ),
        session_stats as (
            select date_trunc(
                       $1,
                       started_at at time zone 'America/Montreal'
                   ) as bucket,
                   count(*)::bigint as sessions,
                   coalesce(sum(page_views), 0)::bigint as page_views
            from usage_session
            where started_at >= (
                (select min(bucket) from periods) at time zone 'America/Montreal'
            )
            group by 1
        )
        select p.bucket,
               coalesce(s.sessions, 0)::bigint as sessions,
               coalesce(s.page_views, 0)::bigint as page_views
        from periods p
        left join session_stats s on s.bucket = p.bucket
        order by p.bucket
        "#,
    )
    .bind(period)
    .bind(intervals)
    .fetch_all(&state.conn)
    .await
    .map_err(|error| {
        eprintln!("Usage: lecture du trafic: {:#}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut total_sessions = 0i64;
    let mut total_page_views = 0i64;
    let buckets: Vec<TrafficBucket> = rows
        .into_iter()
        .map(|row| {
            total_sessions += row.sessions;
            total_page_views += row.page_views;
            TrafficBucket {
                bucket: row.bucket.format("%Y-%m-%d").to_string(),
                sessions: row.sessions,
                page_views: row.page_views,
            }
        })
        .collect();

    let response = UsageTrafficResponse {
        generated_at: iso(Utc::now()),
        period: period.to_string(),
        buckets,
        totals: TrafficTotals {
            sessions: total_sessions,
            page_views: total_page_views,
        },
    };

    Ok((
        [(header::CACHE_CONTROL, "no-store, max-age=0, must-revalidate")],
        Json(response),
    ))
}

/// Enregistre une demande d'itinéraire depuis le calcul WebSocket.
///
/// Le navigateur envoie normalement `/usage/route` avant d'ouvrir le
/// WebSocket. Ce filet de sécurité couvre les clients qui exécutent encore une
/// version en cache du JavaScript (le service worker peut conserver d'anciens
/// custom elements) ou qui ouvrent directement un lien d'itinéraire : le
/// calcul est quand même compté dans `/usage`.
///
/// `route_id` et `session_id` proviennent du suivi navigateur lorsqu'il est
/// disponible. Sans session, l'itinéraire est tout de même conservé. La
/// fonction est idempotente : si la demande a déjà été enregistrée, l'insertion
/// est ignorée.
pub async fn record_route_request(
    conn: &PgPool,
    route_id: Option<Uuid>,
    session_id: Option<Uuid>,
    start_lat: f64,
    start_lng: f64,
    end_lat: f64,
    end_lng: f64,
    allow_ferry: Option<bool>,
    user_agent: &str,
) -> Result<(), sqlx::Error> {
    let (Some(start_lat), Some(start_lng), Some(end_lat), Some(end_lng)) = (
        valid_lat(Some(start_lat)),
        valid_lng(Some(start_lng)),
        valid_lat(Some(end_lat)),
        valid_lng(Some(end_lng)),
    ) else {
        return Ok(());
    };

    // Une version déjà déployée du JavaScript envoie `/usage/route` mais
    // n'ajoute pas encore l'identifiant au WebSocket. On reconnaît cette
    // demande grâce aux coordonnées arrondies et à l'heure récente pour ne pas
    // la compter deux fois. Le filtre de session protège les clients récents.
    let existing = sqlx::query_scalar::<_, Uuid>(
        r#"
        select id
        from usage_route
        where created_at > now() - interval '30 seconds'
          and round(start_lat::numeric, 3) = round($1::numeric, 3)
          and round(start_lng::numeric, 3) = round($2::numeric, 3)
          and round(end_lat::numeric, 3) = round($3::numeric, 3)
          and round(end_lng::numeric, 3) = round($4::numeric, 3)
          and allow_ferry is not distinct from $5
          and ($6::uuid is null or session_id = $6)
        order by created_at desc
        limit 1
        "#,
    )
    .bind(start_lat)
    .bind(start_lng)
    .bind(end_lat)
    .bind(end_lng)
    .bind(allow_ferry)
    .bind(session_id)
    .fetch_optional(conn)
    .await?;

    if existing.is_some() {
        return Ok(());
    }

    // Un client avec une version en cache du tracker n'envoie pas de
    // session_id. On enregistre quand même l'itinéraire, sans créer une
    // session artificielle qui gonflerait les statistiques de visites.
    if let Some(session_id) = session_id {
        let (device, browser, os) = detect_client(user_agent);
        sqlx::query(
            r#"
            insert into usage_session (id, device, browser, os)
            values ($1, $2, $3, $4)
            on conflict (id) do nothing
            "#,
        )
        .bind(session_id)
        .bind(&device)
        .bind(&browser)
        .bind(&os)
        .execute(conn)
        .await?;
    }

    let route_id = route_id.unwrap_or_else(Uuid::new_v4);
    sqlx::query(
        r#"
        insert into usage_route (
            id, session_id, start_lat, start_lng, end_lat, end_lng,
            allow_ferry, status
        )
        values ($1, $2, $3, $4, $5, $6, $7, 'requested')
        on conflict (id) do nothing
        "#,
    )
    .bind(route_id)
    .bind(session_id)
    .bind(start_lat)
    .bind(start_lng)
    .bind(end_lat)
    .bind(end_lng)
    .bind(allow_ferry)
    .execute(conn)
    .await?;

    Ok(())
}

/// Enregistre une demande ou un suivi d'itinéraire.
#[axum::debug_handler]
pub async fn usage_route_track(
    State(state): State<VeloinfoState>,
    Json(payload): Json<UsageRouteTrack>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let session_id = Uuid::parse_str(payload.session_id.trim())
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // La session peut arriver avant son page_view : on la crée au besoin.
    sqlx::query(
        "insert into usage_session (id) values ($1) on conflict (id) do nothing",
    )
    .bind(session_id)
    .execute(&state.conn)
    .await
    .map_err(|error| {
        eprintln!("Usage: création de session pour itinéraire: {:#}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let event = payload.event.as_str();
    if event == "request" {
        let start_lat = valid_lat(payload.start_lat).ok_or(StatusCode::BAD_REQUEST)?;
        let start_lng = valid_lng(payload.start_lng).ok_or(StatusCode::BAD_REQUEST)?;
        let end_lat = valid_lat(payload.end_lat).ok_or(StatusCode::BAD_REQUEST)?;
        let end_lng = valid_lng(payload.end_lng).ok_or(StatusCode::BAD_REQUEST)?;
        let route_id = Uuid::new_v4();

        sqlx::query(
            r#"
            insert into usage_route (
                id, session_id, start_lat, start_lng, end_lat, end_lng,
                allow_ferry, status
            )
            values ($1, $2, $3, $4, $5, $6, $7, 'requested')
            "#,
        )
        .bind(route_id)
        .bind(session_id)
        .bind(start_lat)
        .bind(start_lng)
        .bind(end_lat)
        .bind(end_lng)
        .bind(payload.allow_ferry)
        .execute(&state.conn)
        .await
        .map_err(|error| {
            eprintln!("Usage: création d'itinéraire: {:#}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        return Ok(Json(serde_json::json!({
            "route_id": route_id.to_string()
        })));
    }

    let route_id = payload
        .route_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value.trim()).ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let safe_distance = payload
        .safe_distance_km
        .filter(|value| value.is_finite() && *value >= 0.0);
    let fast_distance = payload
        .fast_distance_km
        .filter(|value| value.is_finite() && *value >= 0.0);
    let remaining = payload
        .remaining_km
        .filter(|value| value.is_finite() && *value >= 0.0);
    let progress = payload
        .progress
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 1.0));
    let chosen_route = match payload.chosen_route.as_deref() {
        None => None,
        Some("safe") => Some("safe"),
        Some("fast") => Some("fast"),
        Some(_) => return Err(StatusCode::BAD_REQUEST),
    };

    let result = match event {
        "result" => {
            sqlx::query(
                r#"
                update usage_route
                set updated_at = now(),
                    safe_distance_km = coalesce($2, safe_distance_km),
                    fast_distance_km = coalesce($3, fast_distance_km)
                where id = $1 and session_id = $4
                "#,
            )
            .bind(route_id)
            .bind(safe_distance)
            .bind(fast_distance)
            .bind(session_id)
            .execute(&state.conn)
            .await
        }
        "follow" => {
            sqlx::query(
                r#"
                update usage_route
                set updated_at = now(),
                    status = 'following',
                    chosen_route = coalesce($2, chosen_route),
                    followed_at = coalesce(followed_at, now()),
                    progress = 0,
                    remaining_km = coalesce($3, remaining_km),
                    last_progress_at = now()
                where id = $1 and session_id = $4
                "#,
            )
            .bind(route_id)
            .bind(chosen_route)
            .bind(remaining)
            .bind(session_id)
            .execute(&state.conn)
            .await
        }
        "progress" => {
            sqlx::query(
                r#"
                update usage_route
                set updated_at = now(),
                    status = case
                        when status = 'requested' then 'following'
                        else status
                    end,
                    progress = coalesce($2, progress),
                    remaining_km = coalesce($3, remaining_km),
                    last_progress_at = now()
                where id = $1 and session_id = $4
                "#,
            )
            .bind(route_id)
            .bind(progress)
            .bind(remaining)
            .bind(session_id)
            .execute(&state.conn)
            .await
        }
        "complete" => {
            sqlx::query(
                r#"
                update usage_route
                set updated_at = now(),
                    status = 'completed',
                    progress = 1,
                    remaining_km = 0,
                    completed_at = now(),
                    last_progress_at = now()
                where id = $1 and session_id = $2
                "#,
            )
            .bind(route_id)
            .bind(session_id)
            .execute(&state.conn)
            .await
        }
        "abandon" => {
            sqlx::query(
                r#"
                update usage_route
                set updated_at = now(),
                    status = case
                        when status = 'completed' then status
                        else 'abandoned'
                    end,
                    progress = coalesce($2, progress),
                    remaining_km = coalesce($3, remaining_km)
                where id = $1 and session_id = $4
                "#,
            )
            .bind(route_id)
            .bind(progress)
            .bind(remaining)
            .bind(session_id)
            .execute(&state.conn)
            .await
        }
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    result.map_err(|error| {
        eprintln!("Usage: mise à jour d'itinéraire: {:#}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Une action d'itinéraire est une activité de session à part entière.
    let _ = sqlx::query("update usage_session set last_seen = now() where id = $1")
        .bind(session_id)
        .execute(&state.conn)
        .await;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Liste les itinéraires récents et leurs statistiques de suivi.
#[axum::debug_handler]
pub async fn usage_routes(
    State(state): State<VeloinfoState>,
    jar: CookieJar,
    Query(params): Query<UsageRoutesQuery>,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    if !is_usage_admin(&state, &jar).await {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let (window, start) = range_start(params.window.as_deref(), "24h");

    let rows = sqlx::query_as::<_, RouteRow>(
        r#"
        select r.id,
               r.created_at,
               r.start_lat,
               r.start_lng,
               r.end_lat,
               r.end_lng,
               r.safe_distance_km,
               r.fast_distance_km,
               r.chosen_route,
               r.status,
               r.progress,
               r.remaining_km,
               s.device,
               s.browser,
               s.os
        from usage_route r
        left join usage_session s on s.id = r.session_id
        where r.created_at > coalesce($1::timestamptz, 'epoch'::timestamptz)
        order by r.created_at desc
        limit 200
        "#,
    )
    .bind(start)
    .fetch_all(&state.conn)
    .await
    .map_err(|error| {
        eprintln!("Usage: lecture des itinéraires: {:#}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let stats = sqlx::query_as::<_, RouteStatsRow>(
        r#"
        select (count(*))::bigint as requests,
               (count(*) filter (where status <> 'requested'))::bigint as followed,
               (count(*) filter (where status = 'completed'))::bigint as completed,
               (count(*) filter (where status = 'abandoned'))::bigint as abandoned
        from usage_route
        where created_at > coalesce($1::timestamptz, 'epoch'::timestamptz)
        "#,
    )
    .bind(start)
    .fetch_one(&state.conn)
    .await
    .map_err(|error| {
        eprintln!("Usage: statistiques des itinéraires: {:#}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let routes = rows
        .into_iter()
        .map(|row| RouteJson {
            id: row.id.to_string(),
            created_at: iso(row.created_at),
            start_lat: round_3(row.start_lat),
            start_lng: round_3(row.start_lng),
            end_lat: round_3(row.end_lat),
            end_lng: round_3(row.end_lng),
            safe_distance_km: row.safe_distance_km,
            fast_distance_km: row.fast_distance_km,
            chosen_route: row.chosen_route,
            status: row.status,
            progress: row.progress,
            remaining_km: row.remaining_km,
            device: row.device,
            browser: row.browser,
            os: row.os,
        })
        .collect();

    let completion_rate = if stats.followed > 0 {
        stats.completed as f64 / stats.followed as f64
    } else {
        0.0
    };

    Ok((
        [(header::CACHE_CONTROL, "no-store, max-age=0, must-revalidate")],
        Json(UsageRoutesResponse {
            generated_at: iso(Utc::now()),
            window,
            stats: RouteStats {
                requests: stats.requests,
                followed: stats.followed,
                completed: stats.completed,
                abandoned: stats.abandoned,
                completion_rate,
            },
            routes,
        }),
    ))
}

#[derive(Debug, Serialize)]
struct RouteGeometryJson {
    route_type: String,
    /// `false` quand aucun chemin n'a été trouvé entre les deux extrémités.
    found: bool,
    coordinates: Vec<[f64; 2]>,
}

/// Calcule la géométrie d'un itinéraire enregistré pour l'aperçu au survol.
///
/// La géométrie n'est pas conservée en base : on la recalcule à la demande à
/// partir des coordonnées arrondies de la demande. Le calcul reste réservé à
/// l'administrateur et n'enregistre aucune nouvelle demande d'itinéraire.
#[axum::debug_handler]
pub async fn usage_route_geometry(
    State(state): State<VeloinfoState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    if !is_usage_admin(&state, &jar).await {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let id = Uuid::parse_str(id.trim()).map_err(|_| StatusCode::BAD_REQUEST)?;

    let (start_lat, start_lng, end_lat, end_lng, chosen_route, allow_ferry) =
        sqlx::query_as::<_, (f64, f64, f64, f64, Option<String>, Option<bool>)>(
            r#"
            select start_lat, start_lng, end_lat, end_lng, chosen_route, allow_ferry
            from usage_route
            where id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&state.conn)
        .await
        .map_err(|error| {
            eprintln!("Usage: lecture de la géométrie d'itinéraire: {:#}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let fast = chosen_route.as_deref() == Some("fast");
    let route_type = if fast { "fast" } else { "safe" }.to_string();
    let cache_key = (id, fast);
    if let Some((found, coordinates)) = ROUTE_GEOMETRY_CACHE
        .lock()
        .ok()
        .and_then(|cache| cache.get(&cache_key).cloned())
    {
        return Ok((
            [(header::CACHE_CONTROL, "private, max-age=600")],
            Json(RouteGeometryJson {
                route_type,
                found,
                coordinates,
            }),
        ));
    }

    let start = Edge::find_closest_node(&start_lng, &start_lat, &state.conn)
        .await
        .map_err(|error| {
            eprintln!("Usage: nœud de départ introuvable: {:#}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let end = Edge::find_closest_node(&end_lng, &end_lat, &state.conn)
        .await
        .map_err(|error| {
            eprintln!("Usage: nœud d'arrivée introuvable: {:#}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let heuristic = if fast {
        get_h_rapid()
    } else {
        get_h_moyen()
    };
    let points = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        Edge::a_star_bidirectional(
            start.node_id,
            end.node_id,
            heuristic,
            &state.conn,
            None,
            allow_ferry.unwrap_or(true),
        ),
    )
    .await
    .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?;

    let found = !points.is_empty();
    let mut coordinates = Vec::with_capacity(points.len() + 2);
    coordinates.push([start_lng, start_lat]);
    coordinates.extend(points.iter().map(|point| [point.lng, point.lat]));
    coordinates.push([end_lng, end_lat]);

    if let Ok(mut cache) = ROUTE_GEOMETRY_CACHE.lock() {
        if cache.len() >= ROUTE_GEOMETRY_CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(cache_key, (found, coordinates.clone()));
    }

    Ok((
        [(header::CACHE_CONTROL, "private, max-age=600")],
        Json(RouteGeometryJson {
            route_type,
            found,
            coordinates,
        }),
    ))
}

/// Liste les contributions et commentaires récents.
#[axum::debug_handler]
pub async fn usage_contributions(
    State(state): State<VeloinfoState>,
    jar: CookieJar,
    Query(params): Query<UsageContributionsQuery>,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    if !is_usage_admin(&state, &jar).await {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let (window, start) = range_start(params.window.as_deref(), "24h");

    let stats = sqlx::query_as::<_, ContributionStatsRow>(
        r#"
        with report_stats as (
            select (count(*))::bigint as reports,
                   (count(*) filter (where photo_path_thumbnail is not null))::bigint
                       as photos
            from report
            where created_at > coalesce($1::timestamptz, 'epoch'::timestamptz)
        ),
        comment_stats as (
            select (count(*))::bigint as comments,
                   (count(*) filter (where photo_path_thumbnail is not null))::bigint
                       as photos
            from report_comment
            where created_at > coalesce($1::timestamptz, 'epoch'::timestamptz)
        ),
        contributors as (
            select (count(distinct user_key))::bigint as contributors
            from (
                select user_id::text as user_key
                from report
                where created_at > coalesce($1::timestamptz, 'epoch'::timestamptz)
                  and user_id is not null
                union
                select user_id::text
                from report_comment
                where created_at > coalesce($1::timestamptz, 'epoch'::timestamptz)
                  and user_id is not null
                union
                select ap_remote_actor_id
                from report_comment
                where created_at > coalesce($1::timestamptz, 'epoch'::timestamptz)
                  and ap_remote_actor_id is not null
            ) all_contributors
        )
        select report_stats.reports,
               comment_stats.comments,
               (report_stats.photos + comment_stats.photos)::bigint as photos,
               contributors.contributors
        from report_stats, comment_stats, contributors
        "#,
    )
    .bind(start)
    .fetch_one(&state.conn)
    .await
    .map_err(|error| {
        eprintln!("Usage: statistiques des contributions: {:#}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let top_contributors = sqlx::query_as::<_, ContributorRow>(
        r#"
        with names as (
            select coalesce(nullif(u.name, ''), 'Anonyme') as name
            from report r
            left join users u on u.id = r.user_id
            where r.created_at > coalesce($1::timestamptz, 'epoch'::timestamptz)
            union all
            select coalesce(
                nullif(u.name, ''),
                nullif(rc.ap_remote_actor_name, ''),
                'Anonyme'
            )
            from report_comment rc
            left join users u on u.id = rc.user_id
            where rc.created_at > coalesce($1::timestamptz, 'epoch'::timestamptz)
        )
        select name, (count(*))::bigint as contributions
        from names
        group by name
        order by contributions desc, name
        limit 5
        "#,
    )
    .bind(start)
    .fetch_all(&state.conn)
    .await
    .map_err(|error| {
        eprintln!("Usage: principaux contributeurs: {:#}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let rows = sqlx::query_as::<_, ContributionRow>(
        r#"
        with items as (
            select 'report' as kind,
                   r.id,
                   r.created_at,
                   coalesce(nullif(u.name, ''), 'Anonyme') as user_name,
                   r.score,
                   ST_X(ST_Centroid(ST_Transform(r.geom, 4326))) as lng,
                   ST_Y(ST_Centroid(ST_Transform(r.geom, 4326))) as lat,
                   null::text as comment,
                   r.photo_path_thumbnail,
                   r.id as report_id,
                   array_to_string(r.name, ' · ') as label
            from report r
            left join users u on u.id = r.user_id
            where r.created_at > coalesce($1::timestamptz, 'epoch'::timestamptz)
            union all
            select 'comment' as kind,
                   rc.id,
                   rc.created_at,
                   coalesce(
                       nullif(u.name, ''),
                       nullif(rc.ap_remote_actor_name, ''),
                       'Anonyme'
                   ) as user_name,
                   r.score,
                   ST_X(ST_Centroid(ST_Transform(r.geom, 4326))) as lng,
                   ST_Y(ST_Centroid(ST_Transform(r.geom, 4326))) as lat,
                   left(rc.comment, 300) as comment,
                   rc.photo_path_thumbnail,
                   rc.report_id,
                   array_to_string(r.name, ' · ') as label
            from report_comment rc
            join report r on r.id = rc.report_id
            left join users u on u.id = rc.user_id
            where rc.created_at > coalesce($1::timestamptz, 'epoch'::timestamptz)
        )
        select kind, id, created_at, user_name, score, lng, lat,
               comment, photo_path_thumbnail, report_id, label
        from items
        order by created_at desc
        limit 100
        "#,
    )
    .bind(start)
    .fetch_all(&state.conn)
    .await
    .map_err(|error| {
        eprintln!("Usage: lecture des contributions: {:#}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let contributions = rows
        .into_iter()
        .map(|row| ContributionJson {
            kind: row.kind,
            id: row.id,
            created_at: iso(row.created_at),
            user_name: row.user_name,
            score: row.score,
            lat: round_3(row.lat),
            lng: round_3(row.lng),
            comment: row.comment,
            photo_path_thumbnail: row.photo_path_thumbnail,
            report_id: row.report_id,
            label: row.label,
        })
        .collect();

    Ok((
        [(header::CACHE_CONTROL, "no-store, max-age=0, must-revalidate")],
        Json(UsageContributionsResponse {
            generated_at: iso(Utc::now()),
            window,
            stats: ContributionStats {
                reports: stats.reports,
                comments: stats.comments,
                photos: stats.photos,
                contributors: stats.contributors,
            },
            top_contributors,
            contributions,
        }),
    ))
}

/// Trouve la ville la plus proche pour les sessions listées (500 max).
async fn session_cities(
    conn: &PgPool,
    sessions: &[SessionRow],
) -> Result<HashMap<Uuid, (String, f64)>, sqlx::Error> {
    let ids: Vec<Uuid> = sessions
        .iter()
        .filter(|session| session.last_lat.is_some() && session.last_lng.is_some())
        .take(500)
        .map(|session| session.id)
        .collect();

    if ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, SessionCityRow>(
        r#"
        with ids as (
            select unnest($1::uuid[]) as id
        )
        select i.id,
               c.name,
               c.distance
        from ids i
        join usage_session s on s.id = i.id
        left join lateral (
            select name,
                   ST_Distance(
                       ST_Transform(geom, 4326)::geography,
                       ST_SetSRID(ST_MakePoint(s.last_lng, s.last_lat), 4326)::geography
                   ) as distance
            from city
            where name is not null and name <> ''
            order by geom <-> ST_Transform(
                ST_SetSRID(ST_MakePoint(s.last_lng, s.last_lat), 4326),
                3857
            )
            limit 1
        ) c on s.last_lat is not null and s.last_lng is not null
        "#,
    )
    .bind(&ids)
    .fetch_all(conn)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| {
            row.name
                .map(|name| (row.id, (name, row.distance.unwrap_or(0.0) / 1000.0)))
        })
        .collect())
}

async fn sessions_since(
    conn: &PgPool,
    start: Option<DateTime<Utc>>,
) -> Result<Vec<SessionRow>, sqlx::Error> {
    sqlx::query_as::<_, SessionRow>(
        r#"
        select id,
               started_at,
               last_seen,
               extract(epoch from (last_seen - started_at))::double precision
                   as duration_seconds,
               last_lat,
               last_lng,
               last_zoom,
               path,
               page_views,
               events,
               device,
               browser,
               os,
               language,
               referrer,
               source,
               (last_seen > now() - interval '5 minutes') as live
        from usage_session
        where last_seen > coalesce($1::timestamptz, 'epoch'::timestamptz)
        order by last_seen desc
        limit $2
        "#,
    )
    .bind(start)
    .bind(SESSION_LIMIT)
    .fetch_all(conn)
    .await
}

async fn load_stats(conn: &PgPool) -> Result<UsageStats, sqlx::Error> {
    let stats = sqlx::query_as::<_, StatsRow>(
        r#"
        select
            (count(*) filter (
                where last_seen > now() - interval '5 minutes'
            ))::bigint as live,
            (count(*) filter (
                where last_seen > now() - interval '24 hours'
            ))::bigint as sessions_24h,
            count(*)::bigint as sessions_total,
            coalesce(
                (avg(extract(epoch from (last_seen - started_at))) filter (
                    where last_seen > now() - interval '24 hours'
                )),
                0
            )::double precision as avg_duration_seconds
        from usage_session
        "#,
    )
    .fetch_one(conn)
    .await?;

    // Les sessions importées de Matomo n'ont pas d'événement individuel par
    // page : on ajoute leurs compteurs agrégés pour ne pas les perdre des
    // statistiques récentes.
    let page_views_24h = sqlx::query_scalar::<_, i64>(
        r#"
        select
            (
                select count(*)::bigint
                from usage_event
                where kind = 'page_view'
                  and created_at > now() - interval '24 hours'
            )
            + (
                select coalesce(sum(page_views), 0)::bigint
                from usage_session
                where source = 'matomo'
                  and last_seen > now() - interval '24 hours'
            )
        "#,
    )
    .fetch_one(conn)
    .await?;

    let actions_24h = sqlx::query_scalar::<_, i64>(
        r#"
        select
            (
                select count(*)::bigint
                from usage_event
                where kind = 'action'
                  and created_at > now() - interval '24 hours'
            )
            + (
                select coalesce(sum(events), 0)::bigint
                from usage_session
                where source = 'matomo'
                  and last_seen > now() - interval '24 hours'
            )
        "#,
    )
    .fetch_one(conn)
    .await?;

    Ok(UsageStats {
        live: stats.live,
        sessions_24h: stats.sessions_24h,
        sessions_total: stats.sessions_total,
        page_views_24h,
        actions_24h,
        avg_duration_seconds: stats.avg_duration_seconds.max(0.0),
    })
}

struct MatomoSession {
    id: Uuid,
    legacy_ref: String,
    started_at: DateTime<Utc>,
    last_seen: DateTime<Utc>,
    lat: Option<f64>,
    lng: Option<f64>,
    zoom: Option<f64>,
    path: Option<String>,
    referrer: Option<String>,
    language: Option<String>,
    device: Option<String>,
    browser: Option<String>,
    os: Option<String>,
    screen_width: Option<i32>,
    screen_height: Option<i32>,
    page_views: i32,
    events: i32,
}

impl MatomoSession {
    #[cfg(test)]
    fn duration_seconds(&self) -> i64 {
        (self.last_seen - self.started_at).num_seconds()
    }
}

/// Importe un export TSV de sessions Matomo.
///
/// Format attendu, sans en-tête et séparé par des tabulations :
/// `idvisit`, `visit_first_action_time`, `visit_last_action_time`,
/// `latitude`, `longitude`, `visit_total_actions`, `visit_total_time`,
/// `config_os`, `config_browser_name`, `config_device_type`,
/// `config_browser_lang`, `config_screen_width`, `config_screen_height`,
/// `referer_name`, `exit_url`, `page_views`.
///
/// Le script `import_matomo.sh` produit ce fichier. L'import est idempotent :
/// l'identifiant de session est dérivé de `idvisit` et les doublons sont
/// ignorés. Ni l'adresse IP ni l'identifiant visiteur de Matomo ne sont lus.
pub async fn import_matomo(conn: &PgPool, path: &str) -> anyhow::Result<()> {
    let reader: Box<dyn AsyncBufRead + Unpin> = if path == "-" {
        Box::new(BufReader::new(tokio::io::stdin()))
    } else {
        Box::new(BufReader::new(tokio::fs::File::open(path).await?))
    };

    let mut lines = reader.lines();
    let mut transaction = conn.begin().await?;
    let mut line_number = 0usize;
    let mut inserted = 0usize;
    let mut ignored = 0usize;

    while let Some(line) = lines.next_line().await? {
        line_number += 1;
        if line.trim().is_empty() {
            continue;
        }

        let session = match parse_matomo_line(&line) {
            Ok(session) => session,
            Err(error) => {
                eprintln!("Matomo ligne {} ignorée: {:#}", line_number, error);
                ignored += 1;
                continue;
            }
        };

        // Un savepoint par ligne : une ligne invalide n'annule pas tout
        // l'import, mais la transaction reste utilisable.
        sqlx::query("savepoint matomo_row")
            .execute(&mut *transaction)
            .await?;

        let result = sqlx::query(
            r#"
            insert into usage_session (
                id, started_at, last_seen, last_lat, last_lng, last_zoom,
                path, referrer, language, device, browser, os,
                screen_width, screen_height, page_views, events,
                source, legacy_ref
            )
            values (
                $1, $2, $3, $4, $5, $6,
                $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16,
                'matomo', $17
            )
            on conflict (id) do nothing
            "#,
        )
        .bind(session.id)
        .bind(session.started_at)
        .bind(session.last_seen)
        .bind(session.lat)
        .bind(session.lng)
        .bind(session.zoom)
        .bind(&session.path)
        .bind(&session.referrer)
        .bind(&session.language)
        .bind(&session.device)
        .bind(&session.browser)
        .bind(&session.os)
        .bind(session.screen_width)
        .bind(session.screen_height)
        .bind(session.page_views)
        .bind(session.events)
        .bind(&session.legacy_ref)
        .execute(&mut *transaction)
        .await;

        match result {
            Ok(outcome) => {
                if outcome.rows_affected() == 1 {
                    inserted += 1;
                } else {
                    ignored += 1;
                }
                sqlx::query("release savepoint matomo_row")
                    .execute(&mut *transaction)
                    .await?;
            }
            Err(error) => {
                eprintln!("Matomo ligne {} non importée: {:#}", line_number, error);
                ignored += 1;
                sqlx::query("rollback to savepoint matomo_row")
                    .execute(&mut *transaction)
                    .await?;
            }
        }

        if line_number % 1000 == 0 {
            println!("Matomo: {} lignes lues, {} sessions importées", line_number, inserted);
        }
    }

    transaction.commit().await?;
    println!(
        "Matomo: import terminé, {} sessions importées, {} lignes ignorées.",
        inserted, ignored
    );
    Ok(())
}

fn parse_matomo_line(line: &str) -> anyhow::Result<MatomoSession> {
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() < 16 {
        anyhow::bail!(
            "{} colonnes reçues, 16 attendues",
            fields.len()
        );
    }

    let legacy_ref = fields[0].trim().to_string();
    if legacy_ref.is_empty() {
        anyhow::bail!("idvisit vide");
    }

    let started_at = parse_matomo_time(fields[1])?;
    let last_seen = parse_matomo_time(fields[2])?;
    if last_seen < started_at {
        anyhow::bail!("date de fin antérieure au début");
    }

    let total_actions: i32 = fields[5].trim().parse().unwrap_or(0);
    let page_views: i32 = fields[15]
        .trim()
        .parse()
        .unwrap_or(total_actions)
        .max(0);

    // À partir de 2024, l'application met à jour l'URL avec le centre et le
    // zoom de la carte. C'est une bien meilleure information que la position
    // IP de Matomo, on la préfère quand elle est disponible.
    let exit_url = fields[14].trim();
    let (lat, lng, zoom) = match map_position_from_url(exit_url) {
        Some((lat, lng, zoom)) => (Some(lat), Some(lng), zoom),
        None => (
            valid_lat(parse_optional_f64(fields[3])),
            valid_lng(parse_optional_f64(fields[4])),
            None,
        ),
    };

    Ok(MatomoSession {
        id: matomo_uuid(&legacy_ref),
        legacy_ref,
        started_at,
        last_seen,
        lat,
        lng,
        zoom,
        path: path_from_url(exit_url),
        referrer: clean(Some(fields[13].to_string()), 300),
        language: clean(Some(fields[10].to_string()), 40),
        device: matomo_device(fields[9]),
        browser: normalize_browser(fields[8]),
        os: normalize_os(fields[7]),
        screen_width: parse_positive_i32(fields[11]),
        screen_height: parse_positive_i32(fields[12]),
        page_views,
        events: total_actions.max(page_views),
    })
}

fn matomo_uuid(idvisit: &str) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(b"matomo:");
    hasher.update(idvisit.as_bytes());
    let digest = hasher.finalize();

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    // Marque l'UUID comme version 4 et variant RFC 4122.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn parse_matomo_time(value: &str) -> anyhow::Result<DateTime<Utc>> {
    let value = value.trim();
    for format in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(value) = NaiveDateTime::parse_from_str(value, format) {
            return Ok(value.and_utc());
        }
    }
    anyhow::bail!("date Matomo invalide: {:?}", value)
}

fn parse_optional_f64(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn parse_positive_i32(value: &str) -> Option<i32> {
    value
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|value| (1..=20000).contains(value))
}

fn matomo_device(value: &str) -> Option<String> {
    match value.trim() {
        "0" => Some("ordinateur".to_string()),
        "1" => Some("mobile".to_string()),
        "2" => Some("tablette".to_string()),
        "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10" => {
            Some("autre".to_string())
        }
        _ => None,
    }
}

fn normalize_os(value: &str) -> Option<String> {
    let value = clean(Some(value.to_string()), 40)?;
    let normalized = match value.to_uppercase().as_str() {
        // Codes courts de DeviceDetector, utilisés par Matomo.
        "IOS" => "iOS".to_string(),
        "IPA" => "iPadOS".to_string(),
        "AND" => "Android".to_string(),
        "LIN" => "Linux".to_string(),
        "UBT" => "Linux (Ubuntu)".to_string(),
        "WIN" => "Windows".to_string(),
        "MAC" => "macOS".to_string(),
        "COS" => "ChromeOS".to_string(),
        "UNK" => "Inconnu".to_string(),
        _ => match value.to_lowercase().as_str() {
            "mac" | "macos" | "mac os" | "mac os x" => "macOS".to_string(),
            "gnu/linux" | "linux" => "Linux".to_string(),
            "windows" | "win32" | "windows 10" | "windows 11" => "Windows".to_string(),
            "ios" | "iphone" | "ipad" => "iOS".to_string(),
            "android" => "Android".to_string(),
            _ => value,
        },
    };
    Some(normalized)
}

fn normalize_browser(value: &str) -> Option<String> {
    let value = clean(Some(value.to_string()), 40)?;
    let normalized = match value.to_uppercase().as_str() {
        // Codes courts de DeviceDetector, utilisés par Matomo.
        "CH" => "Chrome".to_string(),
        "CM" => "Chrome Mobile".to_string(),
        "CI" => "Chrome Mobile iOS".to_string(),
        "CR" => "Chromium".to_string(),
        "CV" => "Chrome WebView".to_string(),
        "FF" => "Firefox".to_string(),
        "FM" => "Firefox Mobile".to_string(),
        "F1" => "Firefox Focus".to_string(),
        "MF" => "Safari Mobile".to_string(),
        "SB" => "Samsung Internet".to_string(),
        "SF" => "Safari".to_string(),
        "BR" => "Brave".to_string(),
        "EC" => "Edge".to_string(),
        "ED" => "Edge".to_string(),
        "OP" => "Opera".to_string(),
        "OM" => "Opera Mobile".to_string(),
        "AN" => "Android Browser".to_string(),
        "DD" => "DuckDuckGo".to_string(),
        "HC" => "Headless Chrome".to_string(),
        "PS" => "Pale Moon".to_string(),
        "YA" => "Yandex".to_string(),
        "QM" => "Qwant Mobile".to_string(),
        "UNK" => "Inconnu".to_string(),
        _ => value,
    };
    Some(normalized)
}

fn map_position_from_url(value: &str) -> Option<(f64, f64, Option<f64>)> {
    let query = value.split_once('?')?.1;
    let mut lat = None;
    let mut lng = None;
    let mut zoom = None;

    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        match key {
            "lat" => lat = parse_optional_f64(value),
            "lng" => lng = parse_optional_f64(value),
            "zoom" => zoom = parse_optional_f64(value),
            _ => {}
        }
    }

    Some((valid_lat(lat)?, valid_lng(lng)?, valid_zoom(zoom)))
}

fn path_from_url(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    let without_query = value.split(['?', '#']).next().unwrap_or(value);
    let path = if let Some(scheme_end) = without_query.find("://") {
        let after_scheme = &without_query[scheme_end + 3..];
        match after_scheme.find('/') {
            Some(index) => &after_scheme[index..],
            None => "/",
        }
    } else if let Some(slash) = without_query.find('/') {
        let host = &without_query[..slash];
        if host.contains('.') {
            &without_query[slash..]
        } else {
            without_query
        }
    } else if without_query.contains('.') && !without_query.starts_with('/') {
        "/"
    } else {
        without_query
    };

    clean(Some(path.to_string()), 300)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_mobile_ios_safari() {
        let ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) \
                  AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 \
                  Mobile/15E148 Safari/604.1";
        let (device, browser, os) = detect_client(ua);
        assert_eq!(device, "mobile");
        assert_eq!(browser, "Safari");
        assert_eq!(os, "iOS");
    }

    #[test]
    fn detects_desktop_firefox_linux() {
        let ua = "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0";
        let (device, browser, os) = detect_client(ua);
        assert_eq!(device, "ordinateur");
        assert_eq!(browser, "Firefox");
        assert_eq!(os, "Linux");
    }

    #[test]
    fn rejects_invalid_coordinates() {
        assert_eq!(valid_lat(Some(120.0)), None);
        assert_eq!(valid_lng(Some(-200.0)), None);
        assert_eq!(valid_zoom(Some(30.0)), None);
        assert_eq!(valid_lat(Some(45.5)), Some(45.5));
    }

    #[test]
    fn parses_a_matomo_session_line() {
        let line = "42\t2025-06-01 12:00:00\t2025-06-01 12:07:30\t\
                    45.501900\t-73.567300\t8\t450\tMac\tSafari\t0\tfr-ca\t\
                    390\t844\texample.com\thttps://veloinfo.quebec/?lat=45.5\t5";
        let session = parse_matomo_line(line).unwrap();

        assert_eq!(session.legacy_ref, "42");
        assert_eq!(session.lat, Some(45.5019));
        assert_eq!(session.lng, Some(-73.5673));
        assert_eq!(session.page_views, 5);
        assert_eq!(session.events, 8);
        assert_eq!(session.device.as_deref(), Some("ordinateur"));
        assert_eq!(session.os.as_deref(), Some("macOS"));
        assert_eq!(session.path.as_deref(), Some("/"));
        assert_eq!(session.duration_seconds(), 450);
    }

    #[test]
    fn matomo_uuid_is_stable() {
        assert_eq!(matomo_uuid("42"), matomo_uuid("42"));
        assert_ne!(matomo_uuid("42"), matomo_uuid("43"));
    }

    #[test]
    fn extracts_path_from_url() {
        assert_eq!(
            path_from_url("https://veloinfo.quebec/carte?lat=1#x").as_deref(),
            Some("/carte")
        );
        assert_eq!(path_from_url("https://veloinfo.quebec").as_deref(), Some("/"));
        assert_eq!(
            path_from_url("veloinfo.quebec/?lat=45.5&lng=-73.5").as_deref(),
            Some("/")
        );
        assert_eq!(path_from_url("").as_deref(), None);
    }

    #[test]
    fn prefers_map_position_from_matomo_url() {
        let line = "7\t2026-01-01 10:00:00\t2026-01-01 10:05:00\t\
                    45.400000\t-73.600000\t3\t300\tAND\tCM\t1\tfr-ca\t\
                    390\t844\t\tveloinfo.quebec/?lat=46.8139&lng=-71.208&zoom=13.5\t3";
        let session = parse_matomo_line(line).unwrap();

        assert_eq!(session.lat, Some(46.8139));
        assert_eq!(session.lng, Some(-71.208));
        assert_eq!(session.zoom, Some(13.5));
        assert_eq!(session.os.as_deref(), Some("Android"));
        assert_eq!(session.browser.as_deref(), Some("Chrome Mobile"));
        assert_eq!(session.path.as_deref(), Some("/"));
    }

    #[test]
    fn normalizes_matomo_os_names() {
        assert_eq!(normalize_os("MAC").as_deref(), Some("macOS"));
        assert_eq!(normalize_os("AND").as_deref(), Some("Android"));
        assert_eq!(normalize_os("LIN").as_deref(), Some("Linux"));
        assert_eq!(normalize_os("IPA").as_deref(), Some("iPadOS"));
    }

    #[test]
    fn normalizes_matomo_browser_codes() {
        assert_eq!(normalize_browser("MF").as_deref(), Some("Safari Mobile"));
        assert_eq!(normalize_browser("CM").as_deref(), Some("Chrome Mobile"));
        assert_eq!(normalize_browser("FF").as_deref(), Some("Firefox"));
        assert_eq!(normalize_browser("Facebook").as_deref(), Some("Facebook"));
    }

    #[test]
    fn normalizes_admin_account_identifiers() {
        assert_eq!(
            normalize_acct("@MartinNHamel@mastodon.social"),
            "martinnhamel@mastodon.social"
        );
        assert_eq!(
            normalize_acct("https://mastodon.social/@MartinNHamel"),
            "martinnhamel@mastodon.social"
        );
        assert_eq!(
            normalize_acct("MartinNHamel@mastodon.social"),
            "martinnhamel@mastodon.social"
        );
    }

    #[test]
    fn maps_traffic_periods() {
        assert_eq!(traffic_period(Some("day")), ("day", 29));
        assert_eq!(traffic_period(Some("week")), ("week", 25));
        assert_eq!(traffic_period(Some("month")), ("month", 35));
        assert_eq!(traffic_period(None), ("day", 29));
    }

    #[test]
    fn renders_usage_page() {
        let html = UsageTemplate.render().unwrap();
        assert!(html.contains("custom-elements/vi-usage.js"));
        assert!(html.contains("<vi-usage>"));
    }
}
