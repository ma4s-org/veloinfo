//! Connexion des utilisateurs avec leur propre compte Fédivers.
//!
//! Vélo info utilise l'API Mastodon-compatible (OAuth 2.0) de l'instance de
//! l'utilisateur. Une fois le compte relié, chaque nouveau signalement est
//! publié sur ce compte avec le texte, la vignette cartographique et la photo
//! éventuelle. Les commentaires et réponses continuent d'être fédérés par le
//! compte principal `@veloinfo@<domaine>`.

use crate::activity_pub::map_image;
use crate::activity_pub::signature;
use crate::auth;
use crate::db::fediverse::{FediverseAccount, FediverseApp, FediversePublication};
use crate::db::report::Report;
use crate::db::user::User;
use crate::VeloinfoState;
use anyhow::{anyhow, bail, Context, Result};
use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::{Form, Json};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use lazy_static::lazy_static;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashMap;
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

lazy_static! {
    static ref BASE_URL: String = env::var("VELOINFO_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string())
        .trim_end_matches('/')
        .to_string();
    static ref VISIBILITY: String =
        env::var("FEDIVERSE_VISIBILITY").unwrap_or_else(|_| "public".to_string());
}

// Portée volontairement large (`read write`) pour rester compatible avec
// Mastodon, GoToSocial, Pleroma et Akkoma. L'application ne lit que le profil
// du compte et n'écrit que des statuts et des médias.
const OAUTH_SCOPE: &str = "read write";
const STATE_COOKIE: &str = "fediverse_state";
const INSTANCE_COOKIE: &str = "fediverse_instance";
const RETURN_COOKIE: &str = "fediverse_return_to";

/// N'accepte que les retours vers le tableau de bord d'usage, pour éviter
/// toute redirection ouverte après l'OAuth.
fn safe_return_path(value: &str) -> bool {
    (value == "/usage" || value.starts_with("/usage?"))
        && !value.starts_with("//")
        && !value.contains('\\')
}

fn redirect_uri() -> String {
    format!("{}/fediverse/callback", *BASE_URL)
}

fn api_url(instance: &str, path: &str) -> String {
    format!("{}{}", instance.trim_end_matches('/'), path)
}

fn host_of(instance: &str) -> String {
    reqwest::Url::parse(instance)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_else(|| instance.to_string())
}

/// Client HTTP pour l'envoi des médias et des statuts. Le délai est plus long
/// que pour les activités ActivityPub, car une photo peut être volumineuse.
fn media_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .user_agent(format!(
            "VeloInfo/{} (+{})",
            env!("CARGO_PKG_VERSION"),
            *BASE_URL
        ))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 3
                || signature::validate_remote_url(attempt.url().as_str()).is_err()
            {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()?)
}

/// Normalise le domaine saisi par l'utilisateur.
///
/// Accepte `mastodon.social`, `https://mastodon.social`, `@moi@mastodon.social`
/// ou `moi@mastodon.social`. Le schéma `http://` n'est conservé qu'en
/// développement local (comme pour les autres appels ActivityPub).
fn normalize_instance(input: &str) -> Result<String> {
    let raw = input.trim();
    if raw.is_empty() {
        bail!("indiquez le domaine de votre instance");
    }

    let candidate = if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.to_string()
    } else {
        // On accepte aussi la forme @utilisateur@instance ou utilisateur@instance.
        let without_at = raw.trim_start_matches('@');
        let host = without_at
            .rsplit('@')
            .next()
            .unwrap_or(without_at)
            .trim_end_matches('/');
        format!("https://{}", host)
    };

    let url = signature::validate_remote_url(&candidate)?;
    if !url.path().is_empty() && url.path() != "/" {
        bail!("indiquez seulement le domaine, sans chemin");
    }
    if url.query().is_some() || url.fragment().is_some() {
        bail!("le domaine ne doit pas contenir de paramètres");
    }

    let host = url.host_str().ok_or_else(|| anyhow!("domaine invalide"))?;
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{}]", host)
    } else {
        host.to_string()
    };
    let port = url
        .port()
        .map(|port| format!(":{}", port))
        .unwrap_or_default();

    Ok(format!("{}://{}{}", url.scheme(), host, port))
}

/// Enregistre (ou récupère) l'application OAuth auprès de l'instance.
async fn register_app(conn: &PgPool, instance: &str) -> Result<FediverseApp> {
    if let Some(app) = FediverseApp::get(instance, conn).await? {
        return Ok(app);
    }

    let client = signature::remote_client()?;
    let redirect = redirect_uri();
    let response = client
        .post(api_url(instance, "/api/v1/apps"))
        .form(&[
            ("client_name", "Vélo info"),
            ("redirect_uris", redirect.as_str()),
            ("scopes", OAUTH_SCOPE),
            ("website", BASE_URL.as_str()),
        ])
        .send()
        .await
        .with_context(|| format!("enregistrement de l'application sur {}", instance))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!(
            "l'instance {} a refusé l'enregistrement (HTTP {}) : {}",
            instance,
            status,
            body.chars().take(200).collect::<String>()
        );
    }

    let value: Value =
        serde_json::from_str(&body).context("réponse d'enregistrement OAuth invalide")?;
    let client_id = value
        .get("client_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("client_id manquant dans la réponse de l'instance"))?
        .to_string();
    let client_secret = value
        .get("client_secret")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("client_secret manquant dans la réponse de l'instance"))?
        .to_string();

    FediverseApp::upsert(instance, &client_id, &client_secret, conn).await?;

    Ok(FediverseApp {
        client_id,
        client_secret,
    })
}

fn authorize_url(instance: &str, client_id: &str, state: &str) -> Result<String> {
    let mut url = reqwest::Url::parse(&api_url(instance, "/oauth/authorize"))?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", &redirect_uri())
        .append_pair("scope", OAUTH_SCOPE)
        .append_pair("state", state);
    Ok(url.to_string())
}

async fn exchange_code(
    instance: &str,
    app: &FediverseApp,
    code: &str,
) -> Result<(String, Option<String>)> {
    let client = signature::remote_client()?;
    let redirect = redirect_uri();
    let response = client
        .post(api_url(instance, "/oauth/token"))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", app.client_id.as_str()),
            ("client_secret", app.client_secret.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("scope", OAUTH_SCOPE),
        ])
        .send()
        .await
        .with_context(|| format!("échange du code OAuth auprès de {}", instance))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!(
            "l'instance {} a refusé le jeton (HTTP {}) : {}",
            instance,
            status,
            body.chars().take(200).collect::<String>()
        );
    }

    let value: Value =
        serde_json::from_str(&body).context("réponse de jeton OAuth invalide")?;
    let access_token = value
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("access_token manquant dans la réponse de l'instance"))?
        .to_string();
    let scope = value
        .get("scope")
        .and_then(Value::as_str)
        .map(str::to_string);

    Ok((access_token, scope))
}

#[derive(Debug)]
struct RemoteAccount {
    id: String,
    acct: String,
    username: Option<String>,
    display_name: Option<String>,
    avatar_url: Option<String>,
    profile_url: Option<String>,
}

fn json_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|field| {
        field
            .as_str()
            .map(str::to_string)
            .or_else(|| field.as_i64().map(|number| number.to_string()))
    })
}

async fn verify_credentials(instance: &str, access_token: &str) -> Result<RemoteAccount> {
    let client = signature::remote_client()?;
    let response = client
        .get(api_url(instance, "/api/v1/accounts/verify_credentials"))
        .bearer_auth(access_token)
        .send()
        .await
        .with_context(|| format!("vérification du compte auprès de {}", instance))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!(
            "l'instance {} a refusé le compte (HTTP {}) : {}",
            instance,
            status,
            body.chars().take(200).collect::<String>()
        );
    }

    let value: Value = serde_json::from_str(&body).context("profil de compte invalide")?;
    let id = json_string(&value, "id").ok_or_else(|| anyhow!("identifiant de compte manquant"))?;
    let raw_acct = json_string(&value, "acct").unwrap_or_default();
    let acct = if raw_acct.contains('@') {
        raw_acct
    } else if raw_acct.is_empty() {
        host_of(instance)
    } else {
        format!("{}@{}", raw_acct, host_of(instance))
    };

    Ok(RemoteAccount {
        id,
        acct,
        username: json_string(&value, "username"),
        display_name: json_string(&value, "display_name"),
        avatar_url: json_string(&value, "avatar"),
        profile_url: json_string(&value, "url"),
    })
}

/// Envoie une image à l'instance et retourne son identifiant de média.
async fn upload_media(
    client: &reqwest::Client,
    instance: &str,
    access_token: &str,
    bytes: Vec<u8>,
    filename: &str,
    mime: &str,
    description: &str,
) -> Result<String> {
    let build_form = || -> Result<reqwest::multipart::Form> {
        let part = reqwest::multipart::Part::bytes(bytes.clone())
            .file_name(filename.to_string())
            .mime_str(mime)?;
        Ok(reqwest::multipart::Form::new()
            .text("description", description.to_string())
            .part("file", part))
    };

    let mut response = client
        .post(api_url(instance, "/api/v2/media"))
        .bearer_auth(access_token)
        .multipart(build_form()?)
        .send()
        .await
        .with_context(|| format!("envoi du média {} à {}", filename, instance))?;

    if matches!(
        response.status(),
        StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED | StatusCode::NOT_IMPLEMENTED
    ) {
        // Instances plus anciennes : l'API v2 n'existe pas encore.
        response = client
            .post(api_url(instance, "/api/v1/media"))
            .bearer_auth(access_token)
            .multipart(build_form()?)
            .send()
            .await
            .with_context(|| format!("envoi du média {} à {}", filename, instance))?;
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!(
            "l'instance {} a refusé le média (HTTP {}) : {}",
            instance,
            status,
            body.chars().take(200).collect::<String>()
        );
    }

    let value: Value = serde_json::from_str(&body).context("réponse de média invalide")?;
    let id = json_string(&value, "id").ok_or_else(|| anyhow!("identifiant de média manquant"))?;

    // En v2 la réponse peut être 202 tant que le média est en cours de
    // traitement. On attend qu'il soit prêt avant de publier le statut.
    if status == StatusCode::ACCEPTED {
        for _ in 0..30 {
            sleep(Duration::from_millis(500)).await;
            let poll = client
                .get(api_url(instance, &format!("/api/v1/media/{}", id)))
                .bearer_auth(access_token)
                .send()
                .await;
            match poll {
                Ok(response) if response.status().is_success() => break,
                Ok(response) if response.status() == StatusCode::NOT_FOUND => break,
                Ok(_) => {}
                Err(error) => {
                    eprintln!("Fediverse: attente du média {}: {:#}", id, error);
                    break;
                }
            }
        }
    }

    Ok(id)
}

/// Publie un statut et retourne `(id, url)`.
async fn post_status(
    client: &reqwest::Client,
    instance: &str,
    access_token: &str,
    text: &str,
    media_ids: &[String],
    idempotency_key: &str,
) -> Result<(String, String)> {
    let visibility = match VISIBILITY.as_str() {
        "unlisted" => "unlisted",
        "private" => "private",
        "direct" => "direct",
        _ => "public",
    };

    let response = client
        .post(api_url(instance, "/api/v1/statuses"))
        .bearer_auth(access_token)
        .header("Idempotency-Key", idempotency_key)
        .json(&json!({
            "status": text,
            "visibility": visibility,
            "language": "fr",
            "media_ids": media_ids,
            "sensitive": false
        }))
        .send()
        .await
        .with_context(|| format!("publication du statut sur {}", instance))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!(
            "l'instance {} a refusé le statut (HTTP {}) : {}",
            instance,
            status,
            body.chars().take(300).collect::<String>()
        );
    }

    let value: Value = serde_json::from_str(&body).context("réponse de statut invalide")?;
    let id = json_string(&value, "id").ok_or_else(|| anyhow!("identifiant de statut manquant"))?;
    let url = json_string(&value, "url")
        .or_else(|| json_string(&value, "uri"))
        .unwrap_or_default();

    Ok((id, url))
}

fn report_status_text(comment: &str, report_id: i32) -> String {
    let link = format!("{}/report/{}", *BASE_URL, report_id);
    let mut body = comment.trim().replace("\r\n", "\n");
    if body.is_empty() {
        body = "Nouveau signalement sur Vélo info.".to_string();
    }

    // Mastodon compte les URL comme 23 caractères et limite par défaut le
    // statut à 500. On garde une marge confortable pour le lien et les
    // hashtags, tout en laissant les longues instances accepter le texte.
    const MAX_BODY_CHARS: usize = 380;
    if body.chars().count() > MAX_BODY_CHARS {
        body = body.chars().take(MAX_BODY_CHARS).collect::<String>();
        body.push('…');
    }

    format!("🚲 {}\n\n📍 {}\n#VeloInfo · © OpenStreetMap", body, link)
}

/// Chemin local de la photo d'un signalement, si elle est disponible.
fn local_photo_path(path: &str) -> Option<String> {
    let path = path.trim();
    if path.is_empty() || path == "null" {
        return None;
    }

    let image_dir = env::var("IMAGE_DIR").ok()?;
    let image_dir = image_dir.trim_end_matches('/');

    // La base peut contenir un chemin déjà préfixé par IMAGE_DIR.
    if path.starts_with(image_dir) {
        return Some(path.to_string());
    }
    if let Some(rest) = path.strip_prefix("/images/") {
        return Some(format!("{}/{}", image_dir, rest));
    }
    if let Some(rest) = path.strip_prefix("images/") {
        return Some(format!("{}/{}", image_dir, rest));
    }
    if path.starts_with('/') {
        return Some(path.to_string());
    }
    Some(format!("{}/{}", image_dir, path))
}

/// Lit la photo du signalement. La base ne conserve que le chemin de la
/// vignette ; on préfère l'image originale lorsque le fichier existe.
async fn read_report_photo(path: &str) -> Option<Vec<u8>> {
    let thumbnail = local_photo_path(path)?;
    let full = thumbnail.replace("_thumbnail.jpeg", ".jpeg");
    if full != thumbnail {
        if let Ok(bytes) = tokio::fs::read(&full).await {
            return Some(bytes);
        }
    }
    tokio::fs::read(&thumbnail).await.ok()
}

async fn publish_report_inner(
    account: &FediverseAccount,
    report: &Report,
    comment: &str,
    conn: &PgPool,
) -> Result<(String, String)> {
    let client = media_client()?;
    let mut media_ids = Vec::new();

    match map_image::render_report_map_cached(conn, report.id).await {
        Ok(png) => {
            match upload_media(
                &client,
                &account.instance,
                &account.access_token,
                png,
                "carte-veloinfo.png",
                "image/png",
                "Carte de la contribution Vélo info — © OpenStreetMap",
            )
            .await
            {
                Ok(id) => media_ids.push(id),
                Err(error) => eprintln!(
                    "Fediverse: vignette cartographique du signalement {} non jointe: {:#}",
                    report.id, error
                ),
            }
        }
        Err(error) => eprintln!(
            "Fediverse: génération de la carte du signalement {} impossible: {:#}",
            report.id, error
        ),
    }

    if let Some(path) = &report.photo_path_thumbnail {
        if let Some(bytes) = read_report_photo(path).await {
            match upload_media(
                &client,
                &account.instance,
                &account.access_token,
                bytes,
                "photo-signalement.jpg",
                "image/jpeg",
                "Photo jointe au signalement Vélo info",
            )
            .await
            {
                Ok(id) => media_ids.push(id),
                Err(error) => eprintln!(
                    "Fediverse: photo du signalement {} non jointe: {:#}",
                    report.id, error
                ),
            }
        } else {
            eprintln!(
                "Fediverse: photo du signalement {} introuvable",
                report.id
            );
        }
    }

    let text = report_status_text(comment, report.id);
    let idempotency_key = format!(
        "veloinfo-report-{}-{}",
        report.id, account.user_id
    );
    post_status(
        &client,
        &account.instance,
        &account.access_token,
        &text,
        &media_ids,
        &idempotency_key,
    )
    .await
}

/// Publie un signalement sur le compte Fédivers de son auteur, s'il en a lié
/// un. La publication est idempotente : un même signalement n'est jamais
/// publié deux fois sur le même compte.
pub async fn publish_report(conn: &PgPool, report_id: i32, user_id: Uuid) -> Result<()> {
    if FediverseAccount::get_by_user_id(user_id, conn).await?.is_none() {
        return Ok(());
    }

    if FediversePublication::exists(report_id, user_id, conn).await? {
        return Ok(());
    }

    let publication_id = FediversePublication::insert_pending(report_id, user_id, conn).await?;

    publish_publication(publication_id, report_id, user_id, conn).await
}

/// Tente une publication déjà enregistrée dans `fediverse_publications`.
/// Utilisée à la création et par le worker de relance.
async fn publish_publication(
    publication_id: i64,
    report_id: i32,
    user_id: Uuid,
    conn: &PgPool,
) -> Result<()> {
    let Some(account) = FediverseAccount::get_by_user_id(user_id, conn).await? else {
        FediversePublication::mark_failed(
            publication_id,
            "compte Fédivers déconnecté",
            conn,
        )
        .await?;
        return Ok(());
    };

    let report = Report::get_by_id(report_id, conn).await?;
    if !report.enabled {
        FediversePublication::mark_failed(publication_id, "signalement désactivé", conn).await?;
        return Ok(());
    }

    let comment = Report::get_comment_by_report_id(report_id, conn)
        .await?
        .map(|(_, comment)| comment)
        .unwrap_or_default();

    match publish_report_inner(&account, &report, &comment, conn).await {
        Ok((status_id, status_url)) => {
            FediversePublication::mark_published(
                publication_id,
                &status_id,
                &status_url,
                conn,
            )
            .await?;
            println!(
                "Fediverse: signalement {} publié sur @{} ({})",
                report_id, account.acct, status_url
            );
            Ok(())
        }
        Err(error) => {
            let message = format!("{:#}", error);
            FediversePublication::mark_failed(publication_id, &message, conn).await?;
            Err(error)
        }
    }
}

/// Relance périodiquement les publications en échec ou interrompues.
///
/// On retente jusqu'à cinq fois, avec un délai d'au moins deux minutes entre
/// deux tentatives. Au-delà, la publication reste marquée `failed` et son
/// erreur est conservée dans la table.
pub async fn run_publication_worker(conn: PgPool) {
    loop {
        if let Err(error) = retry_publications(&conn).await {
            eprintln!("Fediverse: relance des publications: {:#}", error);
        }
        sleep(Duration::from_secs(60)).await;
    }
}

async fn retry_publications(conn: &PgPool) -> Result<()> {
    let pending: Vec<(i64, i32, Uuid)> = sqlx::query_as(
        r#"SELECT id, report_id, user_id
           FROM fediverse_publications
           WHERE state IN ('pending', 'failed')
             AND attempts < 5
             AND updated_at < NOW() - INTERVAL '2 minutes'
           ORDER BY id
           LIMIT 10"#,
    )
    .fetch_all(conn)
    .await?;

    for (publication_id, report_id, user_id) in pending {
        if let Err(error) = publish_publication(publication_id, report_id, user_id, conn).await {
            eprintln!(
                "Fediverse: nouvelle tentative pour le signalement {} échouée: {:#}",
                report_id, error
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Handlers HTTP
// ---------------------------------------------------------------------------

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Nettoie le nom repris du profil Fédivers avant de le stocker. Le nom est
/// réinjecté dans le formulaire de signalement (innerHTML côté navigateur),
/// donc on retire les caractères HTML sensibles.
fn safe_user_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            !character.is_control() && !matches!(character, '<' | '>' | '"' | '\'' | '&')
        })
        .take(60)
        .collect::<String>()
        .trim()
        .to_string()
}

fn html_error(status: StatusCode, message: &str) -> Response {
    let html = format!(
        r#"<!doctype html>
<html lang="fr">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Connexion Fédivers — Vélo info</title>
  <style>
    body {{ font-family: system-ui, sans-serif; max-width: 38rem; margin: 3rem auto; padding: 0 1rem; line-height: 1.55; }}
    a {{ color: #1d4ed8; }}
  </style>
</head>
<body>
  <h1>Connexion Fédivers impossible</h1>
  <p>{message}</p>
  <p><a href="/">Retour à la carte</a></p>
</body>
</html>"#,
        message = escape_html(message)
    );

    (status, Html(html)).into_response()
}

/// Démarre le parcours OAuth auprès de l'instance indiquée.
pub async fn connect(
    State(state): State<VeloinfoState>,
    jar: CookieJar,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let Some(input) = form.get("instance") else {
        return html_error(
            StatusCode::BAD_REQUEST,
            "Indiquez le domaine de votre instance Fédivers.",
        );
    };

    let instance = match normalize_instance(input) {
        Ok(instance) => instance,
        Err(error) => {
            return html_error(
                StatusCode::BAD_REQUEST,
                &format!("Instance invalide : {:#}", error),
            );
        }
    };

    let return_to = form
        .get("return_to")
        .map(|value| value.trim())
        .filter(|value| safe_return_path(value))
        .map(str::to_string);

    let app = match register_app(&state.conn, &instance).await {
        Ok(app) => app,
        Err(error) => {
            eprintln!("Fediverse: enregistrement sur {}: {:#}", instance, error);
            return html_error(
                StatusCode::BAD_GATEWAY,
                &format!(
                    "Impossible de joindre l'instance {} : {:#}",
                    instance, error
                ),
            );
        }
    };

    let state_token = Uuid::new_v4().to_string();
    let authorize = match authorize_url(&instance, &app.client_id, &state_token) {
        Ok(url) => url,
        Err(error) => {
            return html_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("URL d'autorisation invalide : {:#}", error),
            );
        }
    };

    let secure = BASE_URL.starts_with("https://");
    let mut jar = jar
        .add(
            Cookie::build((STATE_COOKIE, state_token))
                .path("/")
                .http_only(true)
                .same_site(SameSite::Lax)
                .secure(secure)
                .build(),
        )
        .add(
            Cookie::build((INSTANCE_COOKIE, instance))
                .path("/")
                .http_only(true)
                .same_site(SameSite::Lax)
                .secure(secure)
                .build(),
        );

    if let Some(return_to) = return_to {
        jar = jar.add(
            Cookie::build((RETURN_COOKIE, return_to))
                .path("/")
                .http_only(true)
                .same_site(SameSite::Lax)
                .secure(secure)
                .build(),
        );
    }

    (jar, Redirect::to(&authorize)).into_response()
}

/// Retour de l'instance après autorisation.
pub async fn callback(
    State(state): State<VeloinfoState>,
    jar: CookieJar,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if let Some(error) = query.get("error") {
        let detail = query
            .get("error_description")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(error);
        return html_error(
            StatusCode::BAD_REQUEST,
            &format!("L'instance a refusé l'autorisation : {}", detail),
        );
    }

    let expected_state = jar
        .get(STATE_COOKIE)
        .map(|cookie| cookie.value().to_string());
    let instance = jar
        .get(INSTANCE_COOKIE)
        .map(|cookie| cookie.value().to_string());
    let code = query.get("code").cloned();
    let return_to = jar
        .get(RETURN_COOKIE)
        .map(|cookie| cookie.value().to_string())
        .filter(|value| safe_return_path(value));

    let (Some(expected_state), Some(instance), Some(code)) = (expected_state, instance, code)
    else {
        return html_error(
            StatusCode::BAD_REQUEST,
            "La session d'autorisation a expiré. Recommencez la connexion.",
        );
    };

    let instance = match normalize_instance(&instance) {
        Ok(instance) => instance,
        Err(error) => {
            return html_error(
                StatusCode::BAD_REQUEST,
                &format!("Instance invalide : {:#}", error),
            );
        }
    };

    if query.get("state").map(String::as_str) != Some(expected_state.as_str()) {
        return html_error(
            StatusCode::BAD_REQUEST,
            "Le paramètre de sécurité (state) ne correspond pas. Recommencez la connexion.",
        );
    }

    let app = match FediverseApp::get(&instance, &state.conn).await {
        Ok(Some(app)) => app,
        Ok(None) => {
            return html_error(
                StatusCode::BAD_REQUEST,
                "L'application n'est plus enregistrée sur cette instance. Recommencez la connexion.",
            );
        }
        Err(error) => {
            eprintln!("Fediverse: lecture de l'application {}: {:#}", instance, error);
            return html_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Erreur de base de données.",
            );
        }
    };

    let (access_token, scope) = match exchange_code(&instance, &app, &code).await {
        Ok(result) => result,
        Err(error) => {
            eprintln!("Fediverse: échange du code pour {}: {:#}", instance, error);
            // Les identifiants de l'application peuvent avoir été révoqués ou
            // invalidés par une réinitialisation de l'instance : on les oublie
            // pour que la prochaine tentative réenregistre l'application.
            let _ = FediverseApp::delete(&instance, &state.conn).await;
            return html_error(
                StatusCode::BAD_GATEWAY,
                &format!("Impossible de récupérer le jeton d'accès : {:#}", error),
            );
        }
    };

    let account = match verify_credentials(&instance, &access_token).await {
        Ok(account) => account,
        Err(error) => {
            eprintln!("Fediverse: vérification du compte sur {}: {:#}", instance, error);
            return html_error(
                StatusCode::BAD_GATEWAY,
                &format!("Impossible de lire le compte : {:#}", error),
            );
        }
    };

    let (user_id, jar) = auth::current_or_new_user(jar.clone(), &state.conn).await;

    if let Err(error) = FediverseAccount::upsert(
        user_id,
        &instance,
        &account.id,
        &account.acct,
        account.username.as_deref(),
        account.display_name.as_deref(),
        account.avatar_url.as_deref(),
        account.profile_url.as_deref(),
        &access_token,
        scope.as_deref(),
        &state.conn,
    )
    .await
    {
        eprintln!("Fediverse: enregistrement du compte: {:#}", error);
        return html_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Impossible d'enregistrer le compte Fédivers.",
        );
    }

    println!("Fediverse: compte @{} relié à l'utilisateur {}", account.acct, user_id);

    // Préremplit le nom du formulaire de signalement avec le profil Fédivers,
    // sans écraser un nom déjà choisi par l'utilisateur.
    if let Some(user) = User::get(&user_id, &state.conn).await {
        if user.name.trim().is_empty() {
            let candidate = account
                .display_name
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(account.acct.as_str());
            let mut name = safe_user_name(candidate);
            if name.is_empty() {
                name = safe_user_name(account.acct.as_str());
            }
            if !name.is_empty() {
                User::update(&user_id, &name, &state.conn).await;
            }
        }
    }

    let jar = jar
        .remove(Cookie::build(STATE_COOKIE))
        .remove(Cookie::build(INSTANCE_COOKIE))
        .remove(Cookie::build(RETURN_COOKIE));

    let destination = return_to.unwrap_or_else(|| "/?fediverse=connected".to_string());
    (jar, Redirect::to(&destination)).into_response()
}

/// État de connexion Fédivers de l'utilisateur courant (sans le jeton).
pub async fn me(State(state): State<VeloinfoState>, jar: CookieJar) -> Response {
    let payload = match auth::get_any_user_id_from_jar(&jar) {
        None => json!({ "connected": false }),
        Some(user_id) => match FediverseAccount::get_by_user_id(user_id, &state.conn).await {
            Ok(Some(account)) => json!({
                "connected": true,
                "account": {
                    "acct": account.acct,
                    "username": account.username,
                    "display_name": account.display_name,
                    "instance": account.instance,
                    "avatar_url": account.avatar_url,
                    "profile_url": account.profile_url
                }
            }),
            Ok(None) => json!({ "connected": false }),
            Err(error) => {
                eprintln!("Fediverse: état de connexion: {:#}", error);
                json!({ "connected": false })
            }
        },
    };

    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(payload),
    )
        .into_response()
}

/// Supprime le lien avec le compte Fédivers de l'utilisateur courant.
pub async fn disconnect(State(state): State<VeloinfoState>, jar: CookieJar) -> Json<Value> {
    if let Some(user_id) = auth::get_any_user_id_from_jar(&jar) {
        if let Ok(Some(account)) = FediverseAccount::get_by_user_id(user_id, &state.conn).await {
            if let Ok(Some(app)) = FediverseApp::get(&account.instance, &state.conn).await {
                revoke_token(&account.instance, &app, &account.access_token).await;
            }
        }

        if let Err(error) = FediverseAccount::delete_for_user(user_id, &state.conn).await {
            eprintln!("Fediverse: déconnexion de {}: {:#}", user_id, error);
            return Json(json!({ "success": false }));
        }
    }
    Json(json!({ "success": true }))
}

/// Révoque le jeton auprès de l'instance. L'échec n'empêche pas la
/// déconnexion locale : le jeton est de toute façon supprimé de la base.
async fn revoke_token(instance: &str, app: &FediverseApp, access_token: &str) {
    let Ok(client) = signature::remote_client() else {
        return;
    };

    if let Err(error) = client
        .post(api_url(instance, "/oauth/revoke"))
        .form(&[
            ("client_id", app.client_id.as_str()),
            ("client_secret", app.client_secret.as_str()),
            ("token", access_token),
        ])
        .send()
        .await
    {
        eprintln!(
            "Fediverse: révocation du jeton sur {} impossible: {:#}",
            instance, error
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_les_domaines_saisis() {
        assert_eq!(
            normalize_instance("mastodon.social").unwrap(),
            "https://mastodon.social"
        );
        assert_eq!(
            normalize_instance("@moi@masto.bike").unwrap(),
            "https://masto.bike"
        );
        assert_eq!(
            normalize_instance("https://mastodon.social/").unwrap(),
            "https://mastodon.social"
        );
    }

    #[test]
    fn refuse_une_instance_avec_chemin() {
        assert!(normalize_instance("https://mastodon.social/about").is_err());
    }

    #[test]
    fn tronque_le_texte_du_statut() {
        let text = report_status_text(&"a".repeat(1000), 42);
        assert!(text.chars().count() < 460);
        assert!(text.contains("/report/42"));
        assert!(text.starts_with('🚲'));
    }

    #[test]
    fn nettoie_le_nom_du_profil() {
        assert_eq!(safe_user_name("Alice <script>"), "Alice script");
        assert_eq!(safe_user_name("\"Bob\" & Cie"), "Bob  Cie");
        assert_eq!(safe_user_name("   "), "");
    }

    #[test]
    fn construit_l_url_d_autorisation() {
        let url = authorize_url("https://mastodon.social", "client-123", "state-456").unwrap();
        assert!(url.starts_with("https://mastodon.social/oauth/authorize?"));
        assert!(url.contains("client_id=client-123"));
        assert!(url.contains("state=state-456"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("redirect_uri="));
    }

    #[test]
    fn accepte_uniquement_les_retours_vers_usage() {
        assert!(safe_return_path("/usage"));
        assert!(safe_return_path("/usage?window=all"));
        assert!(!safe_return_path("/"));
        assert!(!safe_return_path("/fediverse"));
        assert!(!safe_return_path("https://evil.example/usage"));
        assert!(!safe_return_path("//evil.example/usage"));
        assert!(!safe_return_path("/usage\\..\\evil"));
    }
}
