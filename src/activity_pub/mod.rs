//! Implémentation minimale d'ActivityPub pour Vélo info.
//!
//! Le compte `@veloinfo@<domaine>` publie chaque commentaire local sous forme
//! de Note ActivityPub. Les abonnés du Fédivers reçoivent les nouveaux
//! commentaires, et les réponses envoyées depuis Mastodon (ou un autre
//! logiciel ActivityPub) sont enregistrées comme commentaires imbriqués.

pub mod map_image;
pub mod signature;

use crate::db::activity_pub::{self, CommentFederation, ImportErrorNote, RemoteActor};
use crate::VeloinfoState;
use axum::body::{Body, Bytes};
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use chrono::{SecondsFormat, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashMap;
use std::env;
use std::time::Duration;
use uuid::Uuid;

lazy_static! {
    static ref BASE_URL: String = env::var("VELOINFO_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string())
        .trim_end_matches('/')
        .to_string();
    static ref ACTOR_USERNAME: String =
        env::var("FEDIVERSE_USERNAME").unwrap_or_else(|_| "veloinfo".to_string());
    static ref ACTOR_NAME: String =
        env::var("FEDIVERSE_NAME").unwrap_or_else(|_| "Vélo info".to_string());
    static ref ACTOR_SUMMARY: String = env::var("FEDIVERSE_SUMMARY").unwrap_or_else(|_| {
        "Les commentaires et contributions de Vélo info, directement sur le Fédivers. \
         Répondez ici pour participer à la carte collaborative."
            .to_string()
    });
    static ref ALERT_USERNAME: String =
        env::var("FEDIVERSE_ALERT_USERNAME").unwrap_or_else(|_| "veloinfo-alertes".to_string());
    static ref ALERT_NAME: String =
        env::var("FEDIVERSE_ALERT_NAME").unwrap_or_else(|_| "Alertes Vélo info".to_string());
    static ref ALERT_SUMMARY: String = env::var("FEDIVERSE_ALERT_SUMMARY")
        .unwrap_or_else(|_| {
            "Rapports d'import des travaux routiers et alertes techniques de Vélo info."
                .to_string()
        });
    static ref VERIFY_SIGNATURES: bool = env::var("FEDIVERSE_VERIFY_SIGNATURES")
        .map(|value| value != "false" && value != "0")
        .unwrap_or(true);
}

const ACTIVITY_JSON: &str = "application/activity+json; charset=utf-8";
const PUBLIC: &str = "https://www.w3.org/ns/activitystreams#Public";
const PAGE_SIZE: i64 = 20;

pub fn actor_id() -> String {
    format!("{}/ap/actor", *BASE_URL)
}

pub fn key_id() -> String {
    format!("{}#main-key", actor_id())
}

pub fn alert_actor_id() -> String {
    format!("{}/ap/alert-actor", *BASE_URL)
}

pub fn alert_key_id() -> String {
    format!("{}#main-key", alert_actor_id())
}

fn alert_followers_url() -> String {
    format!("{}/followers", alert_actor_id())
}

fn alert_outbox_url() -> String {
    format!("{}/outbox", alert_actor_id())
}

fn alert_inbox_url() -> String {
    format!("{}/inbox", alert_actor_id())
}

fn note_id(comment_id: i32) -> String {
    format!("{}/ap/comment/{}", *BASE_URL, comment_id)
}

fn activity_id(kind: &str, id: &str) -> String {
    format!("{}/ap/activities/{}/{}", *BASE_URL, kind, id)
}

fn report_context_id(report_id: i32) -> String {
    format!("{}/ap/report/{}", *BASE_URL, report_id)
}

fn report_url(report_id: i32) -> String {
    format!("{}/?report={}", *BASE_URL, report_id)
}

fn followers_url() -> String {
    format!("{}/followers", actor_id())
}

fn outbox_url() -> String {
    format!("{}/outbox", actor_id())
}

fn import_error_url(id: i32) -> String {
    format!("{}/ap/import-error/{}", *BASE_URL, id)
}

fn inbox_url() -> String {
    format!("{}/inbox", actor_id())
}

fn shared_inbox_url() -> String {
    format!("{}/ap/inbox", *BASE_URL)
}

fn host() -> String {
    base_host(&BASE_URL)
}

fn base_host(base: &str) -> String {
    base.split("://")
        .nth(1)
        .unwrap_or(base)
        .split('/')
        .next()
        .unwrap_or(base)
        .to_string()
}

fn activity_response(value: Value) -> Response {
    let body = serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec());
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, ACTIVITY_JSON)
        .body(Body::from(body))
        .unwrap()
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, message.to_string()).into_response()
}

// ---------------------------------------------------------------------------
// WebFinger et profil de l'acteur
// ---------------------------------------------------------------------------

pub async fn webfinger(Query(query): Query<HashMap<String, String>>) -> Response {
    let resource = match query.get("resource") {
        Some(resource) => resource,
        None => return error_response(StatusCode::BAD_REQUEST, "resource manquant"),
    };

    let main_acct = format!("acct:{}@{}", *ACTOR_USERNAME, host());
    let alert_acct = format!("acct:{}@{}", *ALERT_USERNAME, host());

    let (subject, profile_actor, profile_page) = if resource.eq_ignore_ascii_case(&main_acct)
        || resource.eq_ignore_ascii_case(&actor_id())
        || resource.eq_ignore_ascii_case(&format!(
            "acct:{}@{}",
            *ACTOR_USERNAME,
            host().to_lowercase()
        ))
    {
        (main_acct, actor_id(), BASE_URL.to_string())
    } else if resource.eq_ignore_ascii_case(&alert_acct)
        || resource.eq_ignore_ascii_case(&alert_actor_id())
        || resource.eq_ignore_ascii_case(&format!(
            "acct:{}@{}",
            *ALERT_USERNAME,
            host().to_lowercase()
        ))
    {
        (
            alert_acct,
            alert_actor_id(),
            format!("{}/ap/alert-actor", *BASE_URL),
        )
    } else {
        return error_response(StatusCode::NOT_FOUND, "compte inconnu");
    };

    let jrd = json!({
        "subject": subject,
        "aliases": [profile_actor.clone(), profile_page.clone()],
        "links": [
            {
                "rel": "self",
                "type": "application/activity+json",
                "href": profile_actor
            },
            {
                "rel": "http://webfinger.net/rel/profile-page",
                "type": "text/html",
                "href": profile_page
            }
        ]
    });

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/jrd+json; charset=utf-8")
        .body(Body::from(serde_json::to_vec(&jrd).unwrap_or_default()))
        .unwrap()
}

pub async fn actor(State(state): State<VeloinfoState>) -> Response {
    let keypair = match signature::keypair(&state.conn).await {
        Ok(keypair) => keypair,
        Err(error) => {
            eprintln!("ActivityPub: chargement de la clé impossible: {:#}", error);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "clé indisponible");
        }
    };

    let actor = json!({
        "@context": [
            "https://www.w3.org/ns/activitystreams",
            "https://w3id.org/security/v1"
        ],
        "id": actor_id(),
        "type": "Service",
        "preferredUsername": *ACTOR_USERNAME,
        "name": *ACTOR_NAME,
        "summary": *ACTOR_SUMMARY,
        "url": BASE_URL.to_string(),
        "inbox": inbox_url(),
        "outbox": outbox_url(),
        "followers": followers_url(),
        "endpoints": {
            "sharedInbox": shared_inbox_url()
        },
        "publicKey": {
            "id": key_id(),
            "owner": actor_id(),
            "publicKeyPem": keypair.public_key_pem
        },
        "icon": {
            "type": "Image",
            "mediaType": "image/png",
            "url": format!("{}/pub/logo-big.png", *BASE_URL)
        },
        "manuallyApprovesFollowers": false,
        "discoverable": true,
        "indexable": true
    });

    activity_response(actor)
}

pub async fn alert_actor(State(state): State<VeloinfoState>) -> Response {
    let keypair = match signature::alert_keypair(&state.conn).await {
        Ok(keypair) => keypair,
        Err(error) => {
            eprintln!(
                "ActivityPub: chargement de la clé d'alertes impossible: {:#}",
                error
            );
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "clé indisponible");
        }
    };

    let actor = json!({
        "@context": [
            "https://www.w3.org/ns/activitystreams",
            "https://w3id.org/security/v1"
        ],
        "id": alert_actor_id(),
        "type": "Service",
        "preferredUsername": *ALERT_USERNAME,
        "name": *ALERT_NAME,
        "summary": *ALERT_SUMMARY,
        "url": format!("{}/ap/alert-actor", *BASE_URL),
        "inbox": alert_inbox_url(),
        "outbox": alert_outbox_url(),
        "followers": alert_followers_url(),
        "publicKey": {
            "id": alert_key_id(),
            "owner": alert_actor_id(),
            "publicKeyPem": keypair.public_key_pem
        },
        "icon": {
            "type": "Image",
            "mediaType": "image/png",
            "url": format!("{}/pub/logo-big.png", *BASE_URL)
        },
        "manuallyApprovesFollowers": false,
        "discoverable": true,
        "indexable": true
    });

    activity_response(actor)
}

pub async fn alert_followers(State(state): State<VeloinfoState>) -> Response {
    let total = activity_pub::count_alert_followers(&state.conn)
        .await
        .unwrap_or(0);
    activity_response(json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": alert_followers_url(),
        "type": "OrderedCollection",
        "totalItems": total,
        "orderedItems": []
    }))
}

pub async fn alert_outbox(State(state): State<VeloinfoState>) -> Response {
    // Les notes d'erreur sont peu nombreuses : on les renvoie directement.
    let notes: Vec<Value> = sqlx::query_as::<_, ImportErrorNote>(
        "SELECT id, failures, content, created_at
         FROM ap_import_error
         ORDER BY created_at DESC, id DESC
         LIMIT 20",
    )
    .fetch_all(&state.conn)
    .await
    .unwrap_or_default()
    .iter()
    .map(import_error_note_json)
    .collect();

    activity_response(json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": alert_outbox_url(),
        "type": "OrderedCollection",
        "totalItems": notes.len(),
        "orderedItems": notes
    }))
}

pub async fn followers(State(state): State<VeloinfoState>) -> Response {
    let total = activity_pub::count_followers(&state.conn).await.unwrap_or(0);
    activity_response(json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": followers_url(),
        "type": "OrderedCollection",
        "totalItems": total,
        "orderedItems": []
    }))
}

pub async fn outbox(
    State(state): State<VeloinfoState>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let total = activity_pub::CommentFederation::count_local(&state.conn)
        .await
        .unwrap_or(0);

    if query.get("page").map(|value| value == "true").unwrap_or(false) {
        let offset = query
            .get("offset")
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0)
            .max(0);
        let comments = activity_pub::CommentFederation::list_local(
            PAGE_SIZE,
            offset,
            &state.conn,
        )
        .await
        .unwrap_or_default();

        let items: Vec<Value> = comments
            .iter()
            .filter(|comment| !comment.comment.trim().is_empty())
            .map(|comment| create_activity_for_comment(comment))
            .collect();

        let mut page = json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "id": format!("{}?page=true&offset={}", outbox_url(), offset),
            "type": "OrderedCollectionPage",
            "partOf": outbox_url(),
            "orderedItems": items
        });
        if offset + PAGE_SIZE < total {
            page["next"] = json!(format!(
                "{}?page=true&offset={}",
                outbox_url(),
                offset + PAGE_SIZE
            ));
        }
        return activity_response(page);
    }

    activity_response(json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": outbox_url(),
        "type": "OrderedCollection",
        "totalItems": total,
        "first": format!("{}?page=true", outbox_url())
    }))
}

/// Fil de discussion d'une contribution (objet `context` des Notes).
pub async fn report_context(
    State(state): State<VeloinfoState>,
    Path(report_id): Path<i32>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let ids: Vec<i32> = sqlx::query_scalar(
        "SELECT id FROM report_comment WHERE report_id = $1 ORDER BY created_at ASC, id ASC",
    )
    .bind(report_id)
    .fetch_all(&state.conn)
    .await
    .unwrap_or_default();

    let context_id = report_context_id(report_id);
    if query.get("page").map(|value| value == "true").unwrap_or(false) {
        return activity_response(json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "id": format!("{}?page=true", context_id),
            "type": "OrderedCollectionPage",
            "partOf": context_id,
            "orderedItems": ids.iter().map(|id| note_id(*id)).collect::<Vec<_>>()
        }));
    }

    activity_response(json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": context_id,
        "type": "OrderedCollection",
        "totalItems": ids.len(),
        "first": format!("{}?page=true", context_id)
    }))
}

pub async fn note(State(state): State<VeloinfoState>, Path(comment_id): Path<i32>) -> Response {
    let comment = match CommentFederation::get(comment_id, &state.conn).await {
        Ok(Some(comment)) => comment,
        Ok(None) => return error_response(StatusCode::NOT_FOUND, "commentaire inconnu"),
        Err(error) => {
            eprintln!("ActivityPub: lecture du commentaire {}: {}", comment_id, error);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "erreur de base de données");
        }
    };

    // Les notes d'origine distante ne nous appartiennent pas : leur URL
    // canonique est celle de l'instance qui les a publiées.
    if comment.ap_remote_actor_id.is_some() {
        return error_response(StatusCode::NOT_FOUND, "commentaire distant");
    }

    activity_response(note_for_comment(&comment))
}

// ---------------------------------------------------------------------------
// Publication des commentaires locaux
// ---------------------------------------------------------------------------

fn note_for_comment(comment: &CommentFederation) -> Value {
    let content_text = escape_html(comment.comment.trim());
    let content = format!(
        "<p>{}</p><p><a href=\"{}\">Voir sur Vélo info</a> · <a href=\"https://www.openstreetmap.org/copyright\">© OpenStreetMap</a></p>",
        content_text.replace('\n', "<br>"),
        escape_html(&report_url(comment.report_id))
    );

    let id = comment
        .ap_id
        .clone()
        .unwrap_or_else(|| note_id(comment.id));

    // Une réponse à un message distant est adressée à son auteur pour que
    // l'instance d'origine l'affiche bien dans le fil de discussion.
    let mut cc = vec![followers_url()];
    if let Some(parent_actor_id) = &comment.parent_remote_actor_id {
        cc.push(parent_actor_id.clone());
    }

    let mut note = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": id,
        "type": "Note",
        "attributedTo": actor_id(),
        "content": content,
        "published": comment.created_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        "to": [PUBLIC],
        "cc": cc,
        "context": report_context_id(comment.report_id),
        "conversation": report_context_id(comment.report_id),
        "url": report_url(comment.report_id),
        "sensitive": false
    });

    if let Some(parent_id) = comment.parent_comment_id {
        let in_reply_to = match (
            &comment.parent_remote_actor_id,
            &comment.parent_ap_id,
        ) {
            (Some(_), Some(ap_id)) => ap_id.clone(),
            _ => note_id(parent_id),
        };
        note["inReplyTo"] = json!(in_reply_to);
    }

    // La vignette de la carte est toujours jointe au message. La photo
    // éventuelle du commentaire est ajoutée ensuite.
    let mut attachments = vec![json!({
        "type": "Image",
        "mediaType": "image/png",
        "url": format!("{}/report/{}/map.png?v=400m", *BASE_URL, comment.report_id),
        "name": "Carte de la contribution Vélo info"
    })];
    if let Some(photo) = &comment.photo_path_thumbnail {
        if let Some(url) = public_photo_url(photo) {
            attachments.push(json!({
                "type": "Image",
                "mediaType": "image/jpeg",
                "url": url,
                "name": "Photo jointe au commentaire"
            }));
        }
    }
    note["attachment"] = json!(attachments);

    note
}

fn public_photo_url(path: &str) -> Option<String> {
    let path = path.trim();
    if path.is_empty() || path == "null" {
        return None;
    }
    if path.starts_with("http://") || path.starts_with("https://") {
        return Some(path.to_string());
    }
    if path.starts_with("/images/") {
        return Some(format!("{}{}", *BASE_URL, path));
    }
    if path.starts_with('/') {
        // Chemin interne au conteneur : on ne l'expose pas.
        return None;
    }
    Some(format!("{}/images/{}", *BASE_URL, path))
}

fn create_activity_for_comment(comment: &CommentFederation) -> Value {
    let note = note_for_comment(comment);
    json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": activity_id("create", &comment.id.to_string()),
        "type": "Create",
        "actor": actor_id(),
        "published": note["published"].clone(),
        "to": note["to"].clone(),
        "cc": note["cc"].clone(),
        "object": note
    })
}

/// Met en file la livraison d'un commentaire local à tous les abonnés.
/// Un worker s'occupe ensuite des envois réseau.
pub async fn publish_comment(conn: &PgPool, comment_id: i32) -> anyhow::Result<()> {
    let comment = match CommentFederation::get(comment_id, conn).await? {
        Some(comment) => comment,
        None => return Ok(()),
    };
    if comment.ap_remote_actor_id.is_some() || comment.comment.trim().is_empty() {
        return Ok(());
    }

    let activity = create_activity_for_comment(&comment);
    let activity_id = activity
        .get("id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let payload = serde_json::to_string(&activity)?;

    let mut inboxes: std::collections::HashSet<String> =
        activity_pub::follower_inboxes(conn).await?.into_iter().collect();

    // En plus des abonnés, la réponse est livrée à l'auteur du message
    // distant auquel elle répond.
    if let Some(parent_actor_id) = &comment.parent_remote_actor_id {
        if let Ok(Some(actor)) = RemoteActor::get(parent_actor_id, conn).await {
            inboxes.insert(actor.inbox);
        } else if let Ok(actor) =
            signature::fetch_remote_actor(conn, parent_actor_id).await
        {
            inboxes.insert(actor.inbox);
        }
    }

    for inbox in inboxes {
        if let Err(error) =
            activity_pub::enqueue_delivery(&activity_id, &inbox, &payload, conn).await
        {
            eprintln!(
                "ActivityPub: mise en file pour {} impossible: {}",
                inbox, error
            );
        }
    }
    Ok(())
}

fn import_error_note_json(note: &ImportErrorNote) -> Value {
    json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": import_error_url(note.id),
        "type": "Note",
        "attributedTo": alert_actor_id(),
        "content": note.content,
        "published": note.created_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        "to": [PUBLIC],
        "cc": [alert_followers_url()],
        "url": import_error_url(note.id),
        "sensitive": false
    })
}

/// Publie sur le Fédivers une note listant les sources dont l'import a échoué.
///
/// Une même liste de sources n'est pas republiée avant six heures, pour éviter
/// de spammer les abonnés lorsqu'un service reste indisponible.
pub async fn publish_import_failures(
    conn: &PgPool,
    failures: &[&str],
) -> anyhow::Result<()> {
    if failures.is_empty() {
        return Ok(());
    }

    let failures_text = failures.join(", ");

    if let Some(last) = ImportErrorNote::last(conn).await? {
        let age = Utc::now().signed_duration_since(last.created_at);
        if last.failures == failures_text && age < chrono::Duration::hours(6) {
            return Ok(());
        }
    }

    let sources = failures
        .iter()
        .map(|source| format!("<li>{}</li>", escape_html(source)))
        .collect::<Vec<_>>()
        .join("");
    let content = format!(
        "<p>⚠️ Import des travaux routiers en échec pour :</p><ul>{}</ul><p>Les données précédentes sont conservées. Les détails sont dans les journaux du serveur.</p><p><a href=\"{}/\">Voir la carte</a></p>",
        sources, *BASE_URL
    );

    let note = ImportErrorNote::insert(&failures_text, &content, conn).await?;
    let note_json = import_error_note_json(&note);
    let activity = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": format!(
            "{}/ap/activities/create-alert/import-error-{}",
            *BASE_URL, note.id
        ),
        "type": "Create",
        "actor": alert_actor_id(),
        "published": note_json["published"].clone(),
        "to": note_json["to"].clone(),
        "cc": note_json["cc"].clone(),
        "object": note_json
    });
    let activity_id = activity
        .get("id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let payload = serde_json::to_string(&activity)?;

    for inbox in activity_pub::alert_follower_inboxes(conn).await? {
        if let Err(error) = activity_pub::enqueue_delivery_for(
            "alert",
            &activity_id,
            &inbox,
            &payload,
            conn,
        )
        .await
        {
            eprintln!(
                "ActivityPub: mise en file de l'erreur d'import pour {} impossible: {}",
                inbox, error
            );
        }
    }

    Ok(())
}

/// Note publique décrivant un échec d'import, consultable depuis le Fédivers.
pub async fn import_error_note(
    State(state): State<VeloinfoState>,
    Path(id): Path<i32>,
) -> Response {
    match ImportErrorNote::get(id, &state.conn).await {
        Ok(Some(note)) => activity_response(import_error_note_json(&note)),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Note inconnue"),
        Err(error) => {
            eprintln!("ActivityPub: lecture de la note d'import: {:#}", error);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "Erreur interne")
        }
    }
}

pub async fn run_delivery_worker(conn: PgPool) {
    let client = match signature::remote_client() {
        Ok(client) => client,
        Err(error) => {
            eprintln!("ActivityPub: création du client HTTP impossible: {:#}", error);
            return;
        }
    };

    loop {
        if let Err(error) = deliver_pending(&conn, &client).await {
            eprintln!("ActivityPub: livraison: {:#}", error);
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

async fn deliver_pending(conn: &PgPool, client: &reqwest::Client) -> anyhow::Result<()> {
    let deliveries = activity_pub::due_deliveries(20, conn).await?;
    if deliveries.is_empty() {
        return Ok(());
    }

    for delivery in deliveries {
        let (keypair, key_id) = if delivery.account == "alert" {
            (signature::alert_keypair(conn).await?, alert_key_id())
        } else {
            (signature::keypair(conn).await?, key_id())
        };

        let result = deliver_one(conn, client, keypair, &key_id, &delivery).await;
        if let Err(error) = result {
            eprintln!(
                "ActivityPub: livraison vers {} échouée: {:#}",
                delivery.inbox, error
            );
            activity_pub::delivery_failed(
                delivery.id,
                delivery.attempts + 1,
                &format!("{:#}", error),
                conn,
            )
            .await?;
        }
    }
    Ok(())
}

async fn deliver_one(
    conn: &PgPool,
    client: &reqwest::Client,
    keypair: &signature::KeyPair,
    key_id: &str,
    delivery: &activity_pub::Delivery,
) -> anyhow::Result<()> {
    let url = signature::validate_remote_url(&delivery.inbox)?;
    let signed = signature::sign_request(
        &keypair.private_key,
        key_id,
        "post",
        &url,
        delivery.payload.as_bytes(),
    )?;

    let response = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/activity+json")
        .header(reqwest::header::DATE, signed.date)
        .header("Digest", signed.digest)
        .header("Signature", signed.signature)
        .body(delivery.payload.clone())
        .send()
        .await?;

    let status = response.status();
    if status.is_success() {
        activity_pub::delivery_succeeded(delivery.id, conn).await?;
        return Ok(());
    }

    let body = response.text().await.unwrap_or_default();
    anyhow::bail!(
        "HTTP {} {}",
        status,
        body.chars().take(300).collect::<String>()
    )
}

// ---------------------------------------------------------------------------
// Inbox : activités entrantes
// ---------------------------------------------------------------------------

pub async fn inbox(
    State(state): State<VeloinfoState>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    body: Bytes,
) -> Response {
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/ap/actor/inbox");

    let activity: Value = match serde_json::from_slice(&body) {
        Ok(activity) => activity,
        Err(error) => {
            eprintln!("ActivityPub: activité JSON invalide: {}", error);
            return error_response(StatusCode::BAD_REQUEST, "JSON invalide");
        }
    };

    let actor_id = match activity.get("actor").and_then(reference_id) {
        Some(actor_id) => actor_id,
        None => return error_response(StatusCode::BAD_REQUEST, "acteur manquant"),
    };

    let mut remote_actor: Option<RemoteActor> = None;
    if *VERIFY_SIGNATURES {
        let key_id = match signature::signature_key_id(&headers) {
            Ok(key_id) => key_id,
            Err(error) => {
                eprintln!("ActivityPub: {}", error);
                return error_response(StatusCode::UNAUTHORIZED, "signature absente");
            }
        };
        let (actor, public_key) =
            match signature::fetch_public_key(&state.conn, &actor_id, &key_id).await {
                Ok(result) => result,
                Err(error) => {
                    eprintln!(
                        "ActivityPub: clé de {} introuvable: {:#}",
                        actor_id, error
                    );
                    return error_response(StatusCode::UNAUTHORIZED, "clé introuvable");
                }
            };
        if let Err(error) = signature::verify_signature(
            &headers,
            "post",
            path_and_query,
            &body,
            &public_key,
            &key_id,
        ) {
            eprintln!("ActivityPub: signature invalide de {}: {:#}", actor_id, error);
            return error_response(StatusCode::UNAUTHORIZED, "signature invalide");
        }
        remote_actor = Some(actor);
    }

    if remote_actor.is_none() {
        remote_actor = signature::fetch_remote_actor(&state.conn, &actor_id)
            .await
            .ok();
    }

    if let Err(error) = process_activity(&state.conn, &activity, &actor_id, remote_actor).await {
        eprintln!("ActivityPub: traitement de l'activité: {:#}", error);
        return error_response(StatusCode::BAD_REQUEST, "activité refusée");
    }

    StatusCode::ACCEPTED.into_response()
}

/// Boîte de réception du compte d'alertes. Elle n'accepte que les Follow et
/// Undo ; les autres activités sont ignorées.
pub async fn alert_inbox(
    State(state): State<VeloinfoState>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    body: Bytes,
) -> Response {
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/ap/alert-actor/inbox");

    let activity: Value = match serde_json::from_slice(&body) {
        Ok(activity) => activity,
        Err(error) => {
            eprintln!("ActivityPub alertes: activité JSON invalide: {}", error);
            return error_response(StatusCode::BAD_REQUEST, "JSON invalide");
        }
    };

    let remote_actor_id = match activity.get("actor").and_then(reference_id) {
        Some(actor_id) => actor_id,
        None => return error_response(StatusCode::BAD_REQUEST, "acteur manquant"),
    };

    let mut remote_actor: Option<RemoteActor> = None;
    if *VERIFY_SIGNATURES {
        let key_id = match signature::signature_key_id(&headers) {
            Ok(key_id) => key_id,
            Err(error) => {
                eprintln!("ActivityPub alertes: {}", error);
                return error_response(StatusCode::UNAUTHORIZED, "signature absente");
            }
        };
        let (actor, public_key) =
            match signature::fetch_public_key(&state.conn, &remote_actor_id, &key_id).await {
                Ok(result) => result,
                Err(error) => {
                    eprintln!(
                        "ActivityPub alertes: clé de {} introuvable: {:#}",
                        remote_actor_id, error
                    );
                    return error_response(StatusCode::UNAUTHORIZED, "clé introuvable");
                }
            };
        if let Err(error) = signature::verify_signature(
            &headers,
            "post",
            path_and_query,
            &body,
            &public_key,
            &key_id,
        ) {
            eprintln!(
                "ActivityPub alertes: signature invalide de {}: {:#}",
                remote_actor_id, error
            );
            return error_response(StatusCode::UNAUTHORIZED, "signature invalide");
        }
        remote_actor = Some(actor);
    }

    if remote_actor.is_none() {
        remote_actor = signature::fetch_remote_actor(&state.conn, &remote_actor_id)
            .await
            .ok();
    }

    let result = match activity.get("type").and_then(Value::as_str) {
        Some("Follow") => handle_alert_follow(
            &state.conn,
            &activity,
            &remote_actor_id,
            remote_actor,
        )
        .await,
        Some("Undo") => handle_alert_undo(&state.conn, &activity, &remote_actor_id).await,
        _ => Ok(()),
    };

    if let Err(error) = result {
        eprintln!("ActivityPub alertes: traitement de l'activité: {:#}", error);
        return error_response(StatusCode::BAD_REQUEST, "activité refusée");
    }

    StatusCode::ACCEPTED.into_response()
}

async fn handle_alert_follow(
    conn: &PgPool,
    activity: &Value,
    remote_actor_id: &str,
    remote_actor: Option<RemoteActor>,
) -> anyhow::Result<()> {
    if activity.get("object").and_then(reference_id).as_deref()
        != Some(&alert_actor_id())
    {
        return Ok(());
    }

    let actor = remote_actor.ok_or_else(|| {
        anyhow::anyhow!(
            "impossible de récupérer l'acteur {} pour Follow (alertes)",
            remote_actor_id
        )
    })?;

    let follow_activity_id = activity.get("id").and_then(Value::as_str);
    activity_pub::upsert_alert_follower(
        remote_actor_id,
        &actor.inbox,
        actor.shared_inbox.as_deref(),
        actor.display_name.as_deref(),
        follow_activity_id,
        conn,
    )
    .await?;

    let accept = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": format!("{}/ap/activities/accept-alert/{}", *BASE_URL, Uuid::new_v4()),
        "type": "Accept",
        "actor": alert_actor_id(),
        "object": activity
    });
    let payload = serde_json::to_string(&accept)?;
    activity_pub::enqueue_delivery_for(
        "alert",
        accept
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        &actor.inbox,
        &payload,
        conn,
    )
    .await?;

    Ok(())
}

async fn handle_alert_undo(
    conn: &PgPool,
    activity: &Value,
    remote_actor_id: &str,
) -> anyhow::Result<()> {
    let Some(object) = activity.get("object") else {
        return Ok(());
    };

    if object.get("type").and_then(Value::as_str) != Some("Follow") {
        return Ok(());
    }

    if let Some(follow_id) = object.get("id").and_then(Value::as_str) {
        activity_pub::remove_alert_follower_by_activity(
            follow_id,
            remote_actor_id,
            conn,
        )
        .await?;
    } else {
        activity_pub::remove_alert_follower(remote_actor_id, conn).await?;
    }

    Ok(())
}

async fn process_activity(
    conn: &PgPool,
    activity: &Value,
    actor_id: &str,
    remote_actor: Option<RemoteActor>,
) -> anyhow::Result<()> {
    match activity.get("type").and_then(|value| value.as_str()) {
        Some("Follow") => handle_follow(conn, activity, actor_id, remote_actor).await,
        Some("Undo") => handle_undo(conn, activity, actor_id).await,
        Some("Create") => handle_create(conn, activity, actor_id, remote_actor).await,
        Some("Update") => handle_update(conn, activity, actor_id).await,
        Some("Delete") => handle_delete(conn, activity, actor_id).await,
        other => {
            println!("ActivityPub: activité ignorée: {:?}", other);
            Ok(())
        }
    }
}

async fn handle_follow(
    conn: &PgPool,
    activity: &Value,
    remote_actor_id: &str,
    remote_actor: Option<RemoteActor>,
) -> anyhow::Result<()> {
    if activity
        .get("object")
        .and_then(reference_id)
        .as_deref()
        != Some(&crate::activity_pub::actor_id())
    {
        return Ok(());
    }
    let actor = remote_actor.ok_or_else(|| {
        anyhow::anyhow!(
            "impossible de récupérer l'acteur {} pour Follow",
            remote_actor_id
        )
    })?;

    let follow_activity_id = activity.get("id").and_then(|value| value.as_str());
    activity_pub::upsert_follower(
        remote_actor_id,
        &actor.inbox,
        actor.shared_inbox.as_deref(),
        actor.display_name.as_deref(),
        follow_activity_id,
        conn,
    )
    .await?;

    let accept = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": activity_id("accept", &Uuid::new_v4().to_string()),
        "type": "Accept",
        "actor": crate::activity_pub::actor_id(),
        "object": activity
    });
    let payload = serde_json::to_string(&accept)?;
    activity_pub::enqueue_delivery(
        accept.get("id").and_then(|value| value.as_str()).unwrap_or_default(),
        &actor.inbox,
        &payload,
        conn,
    )
    .await?;

    // Rattrapage : on envoie les derniers commentaires au nouvel abonné pour
    // qu'il voie immédiatement le fil, même si son instance ne parcourt pas
    // l'outbox après l'acceptation du suivi.
    if let Ok(mut comments) = CommentFederation::list_local(PAGE_SIZE, 0, conn).await {
        // Du plus ancien au plus récent pour que le fil se construise dans
        // le bon ordre chez le nouvel abonné.
        comments.reverse();
        for comment in comments {
            if comment.comment.trim().is_empty() {
                continue;
            }
            let activity = create_activity_for_comment(&comment);
            let id = activity
                .get("id")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let payload = serde_json::to_string(&activity)?;
            activity_pub::enqueue_delivery(id, &actor.inbox, &payload, conn).await?;
        }
    }

    println!(
        "ActivityPub: nouvel abonné {} ({})",
        actor.id,
        actor.display_name.as_deref().unwrap_or("sans nom")
    );
    Ok(())
}

async fn handle_undo(conn: &PgPool, activity: &Value, actor_id: &str) -> anyhow::Result<()> {
    let object = activity.get("object");
    match object {
        Some(Value::Object(follow))
            if follow.get("type").and_then(|value| value.as_str()) == Some("Follow") =>
        {
            activity_pub::remove_follower(actor_id, conn).await?;
            println!("ActivityPub: désabonnement de {}", actor_id);
        }
        Some(Value::String(follow_activity_id)) => {
            activity_pub::remove_follower_by_activity(
                follow_activity_id,
                actor_id,
                conn,
            )
            .await?;
            println!("ActivityPub: désabonnement de {}", actor_id);
        }
        _ => {}
    }
    Ok(())
}

async fn handle_create(
    conn: &PgPool,
    activity: &Value,
    actor_id: &str,
    remote_actor: Option<RemoteActor>,
) -> anyhow::Result<()> {
    let object = match activity.get("object") {
        Some(object) => object,
        None => return Ok(()),
    };

    if let Some(array) = object.as_array() {
        for item in array {
            handle_note(conn, item, actor_id, remote_actor.as_ref()).await?;
        }
    } else {
        handle_note(conn, object, actor_id, remote_actor.as_ref()).await?;
    }
    Ok(())
}

async fn handle_note(
    conn: &PgPool,
    note: &Value,
    actor_id: &str,
    remote_actor: Option<&RemoteActor>,
) -> anyhow::Result<()> {
    if note.get("type").and_then(|value| value.as_str()) != Some("Note") {
        return Ok(());
    }
    let remote_note_id = match note.get("id").and_then(|value| value.as_str()) {
        Some(id) if id.starts_with("http://") || id.starts_with("https://") => id,
        _ => return Ok(()),
    };
    if note.get("attributedTo").and_then(reference_id).as_deref() != Some(actor_id) {
        return Ok(());
    }
    if CommentFederation::find_by_ap_id(remote_note_id, conn)
        .await?
        .is_some()
    {
        return Ok(());
    }

    let in_reply_to = note
        .get("inReplyTo")
        .and_then(reference_id)
        .or_else(|| note.get("inReplyTo").and_then(|value| value.as_array()?.first().and_then(reference_id)));
    let in_reply_to = match in_reply_to {
        Some(value) => value,
        None => return Ok(()),
    };

    let parent = match resolve_parent(conn, &in_reply_to).await? {
        Some(parent) => parent,
        None => {
            println!(
                "ActivityPub: réponse ignorée, parent inconnu: {}",
                in_reply_to
            );
            return Ok(());
        }
    };

    let raw_content = note
        .get("content")
        .and_then(|value| value.as_str())
        .or_else(|| note.get("summary").and_then(|value| value.as_str()))
        .unwrap_or_default();
    let mut content = html_to_text(raw_content);
    if content.trim().is_empty() {
        return Ok(());
    }
    if content.chars().count() > 5000 {
        content = content.chars().take(5000).collect();
    }

    let actor_name = match remote_actor {
        Some(actor) => actor
            .display_name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| actor_handle(&actor.id, actor.username.as_deref())),
        None => actor_handle(actor_id, None),
    };

    activity_pub::CommentFederation::insert_remote(
        parent.report_id,
        parent.id,
        &content,
        remote_note_id,
        actor_id,
        &actor_name,
        conn,
    )
    .await?;
    Ok(())
}

async fn resolve_parent(
    conn: &PgPool,
    in_reply_to: &str,
) -> anyhow::Result<Option<CommentFederation>> {
    if let Some(comment_id) = local_note_id(in_reply_to) {
        return Ok(CommentFederation::get(comment_id, conn).await?);
    }
    Ok(CommentFederation::find_by_ap_id(in_reply_to, conn).await?)
}

fn actor_handle(actor_id: &str, username: Option<&str>) -> String {
    let host = base_host(actor_id);
    match username {
        Some(username) if !username.is_empty() => format!("@{}@{}", username, host),
        _ => host,
    }
}

fn local_note_id(uri: &str) -> Option<i32> {
    let prefix = format!("{}/ap/comment/", *BASE_URL);
    let id = uri.strip_prefix(&prefix)?.trim_end_matches('/');
    if id.is_empty() || id.contains('/') {
        return None;
    }
    id.parse::<i32>().ok()
}

async fn handle_update(conn: &PgPool, activity: &Value, actor_id: &str) -> anyhow::Result<()> {
    let object = match activity.get("object") {
        Some(object) if object.get("type").and_then(|value| value.as_str()) == Some("Note") => {
            object
        }
        _ => return Ok(()),
    };
    let remote_note_id = match object.get("id").and_then(|value| value.as_str()) {
        Some(id) => id,
        None => return Ok(()),
    };
    let content = html_to_text(
        object
            .get("content")
            .and_then(|value| value.as_str())
            .unwrap_or_default(),
    );
    if content.trim().is_empty() {
        return Ok(());
    }
    activity_pub::CommentFederation::update_remote(
        remote_note_id,
        actor_id,
        &content,
        conn,
    )
    .await?;
    Ok(())
}

async fn handle_delete(conn: &PgPool, activity: &Value, actor_id: &str) -> anyhow::Result<()> {
    let object = activity.get("object");
    if let Some(remote_note_id) = object.and_then(reference_id) {
        if remote_note_id == actor_id {
            activity_pub::CommentFederation::delete_remote_actor_comments(actor_id, conn)
                .await?;
        } else {
            activity_pub::CommentFederation::delete_remote(
                &remote_note_id,
                actor_id,
                conn,
            )
            .await?;
        }
    }
    Ok(())
}

fn reference_id(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Object(object) => object
            .get("id")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Conversion HTML -> texte
// ---------------------------------------------------------------------------

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Convertit le HTML des messages entrants en texte brut destiné à
/// l'interface de Vélo info (qui échappe déjà le texte affiché).
fn html_to_text(html: &str) -> String {
    lazy_static! {
        static ref BREAKS: Regex =
            Regex::new(r"(?i)<br\s*/?>|</p\s*>|</div\s*>|</li\s*>").unwrap();
        static ref TAGS: Regex = Regex::new(r"(?s)<[^>]*>").unwrap();
        static ref BLANK_LINES: Regex = Regex::new(r"\n{3,}").unwrap();
    }

    let with_breaks = BREAKS.replace_all(html, "\n");
    let without_tags = TAGS.replace_all(&with_breaks, "");
    let decoded = decode_entities(&without_tags);
    BLANK_LINES
        .replace_all(decoded.trim(), "\n\n")
        .trim()
        .to_string()
}

fn decode_entities(value: &str) -> String {
    lazy_static! {
        static ref ENTITY: Regex = Regex::new(r"&(#x?[0-9A-Fa-f]+|[A-Za-z]+);").unwrap();
    }

    ENTITY
        .replace_all(value, |captures: &regex::Captures| {
            let entity = &captures[1];
            match entity {
                "amp" => "&".to_string(),
                "lt" => "<".to_string(),
                "gt" => ">".to_string(),
                "quot" => "\"".to_string(),
                "apos" => "'".to_string(),
                "nbsp" => " ".to_string(),
                _ => {
                    let code = if let Some(hex) = entity.strip_prefix("#x").or_else(|| entity.strip_prefix("#X")) {
                        u32::from_str_radix(hex, 16).ok()
                    } else if let Some(decimal) = entity.strip_prefix('#') {
                        decimal.parse::<u32>().ok()
                    } else {
                        None
                    };
                    code.and_then(char::from_u32)
                        .map(|character| character.to_string())
                        .unwrap_or_else(|| captures[0].to_string())
                }
            }
        })
        .to_string()
}

// ---------------------------------------------------------------------------
// Redirection humaine
// ---------------------------------------------------------------------------

/// Les liens contenus dans les Notes pointent vers l'application. Cette route
/// sert de point d'entrée stable pour les navigateurs.
pub async fn report_redirect(Path(report_id): Path<i32>) -> Redirect {
    Redirect::temporary(&format!("/?report={}", report_id))
}

/// Vignette cartographique jointe aux Notes ActivityPub.
pub async fn report_map(
    State(state): State<VeloinfoState>,
    Path(report_id): Path<i32>,
) -> Response {
    match map_image::render_report_map_cached(&state.conn, report_id).await {
        Ok(png) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            .header(header::CACHE_CONTROL, "public, max-age=86400")
            .body(Body::from(png))
            .unwrap(),
        Err(error) => {
            eprintln!(
                "ActivityPub: génération de la carte {}: {:#}",
                report_id, error
            );
            error_response(StatusCode::NOT_FOUND, "carte introuvable")
        }
    }
}

/// Page intermédiaire chargée par Chromium headless pour capturer la carte.
pub async fn report_render_page(
    State(state): State<VeloinfoState>,
    Path(report_id): Path<i32>,
) -> Response {
    match map_image::report_map_html(&state.conn, report_id, &BASE_URL).await {
        Ok(html) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(html))
            .unwrap(),
        Err(error) => {
            eprintln!(
                "ActivityPub: page de rendu de la carte {}: {:#}",
                report_id, error
            );
            error_response(StatusCode::NOT_FOUND, "carte introuvable")
        }
    }
}

/// Petite page d'explication pour découvrir le compte et s'y abonner.
pub async fn fediverse_page() -> Response {
    let handle = format!("@{}@{}", *ACTOR_USERNAME, host());
    let html = format!(
        r#"<!doctype html>
<html lang="fr">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Vélo info sur le Fédivers</title>
  <style>
    body {{ font-family: system-ui, sans-serif; max-width: 42rem; margin: 2rem auto; padding: 0 1rem; line-height: 1.55; }}
    code {{ background: #f3f4f6; padding: 0.15rem 0.35rem; border-radius: 0.25rem; }}
    a {{ color: #1d4ed8; }}
  </style>
</head>
<body>
  <h1>Vélo info sur le Fédivers</h1>
  <p>
    Les commentaires publiés sur Vélo info sont aussi des messages
    ActivityPub. Vous pouvez suivre le compte
    <strong>{handle}</strong> depuis Mastodon, GoToSocial, Pleroma ou
    n'importe quel autre logiciel du Fédivers.
  </p>
  <h2>S'abonner</h2>
  <p>
    Dans la recherche de votre instance, entrez <code>{handle}</code> puis
    cliquez sur « Suivre ». Les nouveaux commentaires apparaîtront dans votre
    fil.
  </p>
  <h2>Répondre</h2>
  <p>
    Répondez directement à un message pour l'ajouter aux commentaires de la
    contribution correspondante sur <a href="{base}">la carte Vélo info</a>.
  </p>
  <p>
    <a href="{base}">Retour à la carte</a> ·
    <a href="{actor}">Profil ActivityPub</a>
  </p>
</body>
</html>"#,
        handle = handle,
        base = *BASE_URL,
        actor = actor_id()
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convertit_le_html_du_fedivers_en_texte() {
        let html = "<p>Bonjour &amp; bienvenue&nbsp;!</p><p>À bientôt<br>sur Vélo info</p>";
        assert_eq!(
            html_to_text(html),
            "Bonjour & bienvenue !\nÀ bientôt\nsur Vélo info"
        );
    }

    #[test]
    fn echappe_le_contenu_des_notes() {
        assert_eq!(
            escape_html("<script>alert('x')</script>"),
            "&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;"
        );
    }

    #[tokio::test]
    #[ignore = "nécessite une base PostgreSQL locale"]
    async fn publie_une_note_d_erreur_d_import() {
        let Ok(url) = std::env::var("DATABASE_URL") else {
            return;
        };
        let pool = PgPool::connect(&url).await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let keypair = signature::alert_keypair(&pool).await.unwrap();
        assert!(!keypair.public_key_pem.is_empty());

        publish_import_failures(&pool, &["ville-test"])
            .await
            .unwrap();

        let note = ImportErrorNote::last(&pool).await.unwrap().unwrap();
        assert!(note.failures.contains("ville-test"));
        let note_json = import_error_note_json(&note);
        assert_eq!(note_json["type"], "Note");
        assert_eq!(note_json["attributedTo"], alert_actor_id());

        sqlx::query("DELETE FROM ap_import_error WHERE failures = $1")
            .bind("ville-test")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM ap_alert_keys")
            .execute(&pool)
            .await
            .unwrap();
    }
}
