use anyhow::{anyhow, bail, Context, Result};
use axum::http::HeaderMap;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::pkcs1v15::{Signature, SigningKey, VerifyingKey};
use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::rand_core::OsRng;
use rsa::signature::{SignatureEncoding, Signer, Verifier};
use rsa::{RsaPrivateKey, RsaPublicKey};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::net::IpAddr;
use std::time::{Duration, SystemTime};
use tokio::sync::OnceCell;

use crate::db::activity_pub::{self, RemoteActor};

pub struct KeyPair {
    pub private_key: RsaPrivateKey,
    pub public_key_pem: String,
}

static KEYPAIR: OnceCell<KeyPair> = OnceCell::const_new();

/// Charge la clé RSA du compte ActivityPub, ou en génère une au premier
/// démarrage. La clé est conservée en base de données pour rester stable.
pub async fn keypair(conn: &PgPool) -> Result<&'static KeyPair> {
    KEYPAIR
        .get_or_try_init(|| async { load_or_create_keypair(conn).await })
        .await
}

async fn load_or_create_keypair(conn: &PgPool) -> Result<KeyPair> {
    if let Some((private_pem, public_pem)) = activity_pub::get_keypair(conn).await? {
        let private_key = RsaPrivateKey::from_pkcs8_pem(&private_pem)
            .context("clé privée ActivityPub invalide")?;
        return Ok(KeyPair {
            private_key,
            public_key_pem: public_pem,
        });
    }

    let private_key = RsaPrivateKey::new(&mut OsRng, 2048)
        .context("génération de la clé RSA ActivityPub")?;
    let private_pem = private_key
        .to_pkcs8_pem(LineEnding::LF)
        .context("encodage de la clé privée ActivityPub")?
        .to_string();
    let public_pem = private_key
        .to_public_key()
        .to_public_key_pem(LineEnding::LF)
        .context("encodage de la clé publique ActivityPub")?;

    activity_pub::insert_keypair(&private_pem, &public_pem, conn).await?;

    // Un autre processus a pu insérer sa clé entre-temps : on relit toujours
    // la valeur effectivement stockée.
    let (private_pem, public_pem) = activity_pub::get_keypair(conn)
        .await?
        .ok_or_else(|| anyhow!("impossible d'enregistrer la clé ActivityPub"))?;
    let private_key = RsaPrivateKey::from_pkcs8_pem(&private_pem)
        .context("clé privée ActivityPub invalide")?;
    Ok(KeyPair {
        private_key,
        public_key_pem: public_pem,
    })
}

pub struct SignedHeaders {
    pub date: String,
    pub digest: String,
    pub signature: String,
}

pub struct SignedGetHeaders {
    pub date: String,
    pub signature: String,
}

/// Signe une requête GET sortante. Nécessaire pour récupérer les acteurs et
/// leurs clés sur les instances en mode « authorized fetch » (Mastodon).
pub fn sign_get_request(
    private_key: &RsaPrivateKey,
    key_id: &str,
    url: &reqwest::Url,
) -> Result<SignedGetHeaders> {
    let date = httpdate::fmt_http_date(SystemTime::now());
    let host = match url.port() {
        Some(port) => format!(
            "{}:{}",
            url.host_str().ok_or_else(|| anyhow!("URL sans hôte"))?,
            port
        ),
        None => url
            .host_str()
            .ok_or_else(|| anyhow!("URL sans hôte"))?
            .to_string(),
    };
    let target = match url.query() {
        Some(query) => format!("{}?{}", url.path(), query),
        None => url.path().to_string(),
    };

    let signing_string = format!(
        "(request-target): get {}\nhost: {}\ndate: {}",
        target, host, date
    );

    let signing_key = SigningKey::<Sha256>::new(private_key.clone());
    let signature = BASE64.encode(signing_key.sign(signing_string.as_bytes()).to_bytes());

    Ok(SignedGetHeaders {
        date,
        signature: format!(
            "keyId=\"{}\",algorithm=\"rsa-sha256\",headers=\"(request-target) host date\",signature=\"{}\"",
            key_id, signature
        ),
    })
}

/// Signe une requête POST sortante avec HTTP Signatures (draft-cavage).
pub fn sign_request(
    private_key: &RsaPrivateKey,
    key_id: &str,
    method: &str,
    url: &reqwest::Url,
    body: &[u8],
) -> Result<SignedHeaders> {
    let date = httpdate::fmt_http_date(SystemTime::now());
    let digest = format!("SHA-256={}", BASE64.encode(Sha256::digest(body)));
    let host = match url.port() {
        Some(port) => format!(
            "{}:{}",
            url.host_str().ok_or_else(|| anyhow!("URL sans hôte"))?,
            port
        ),
        None => url
            .host_str()
            .ok_or_else(|| anyhow!("URL sans hôte"))?
            .to_string(),
    };
    let target = match url.query() {
        Some(query) => format!("{}?{}", url.path(), query),
        None => url.path().to_string(),
    };

    let signing_string = format!(
        "(request-target): {} {}\nhost: {}\ndate: {}\ndigest: {}\ncontent-type: application/activity+json",
        method.to_ascii_lowercase(),
        target,
        host,
        date,
        digest
    );

    let signing_key = SigningKey::<Sha256>::new(private_key.clone());
    let signature = BASE64.encode(signing_key.sign(signing_string.as_bytes()).to_bytes());

    Ok(SignedHeaders {
        date,
        digest,
        signature: format!(
            "keyId=\"{}\",algorithm=\"rsa-sha256\",headers=\"(request-target) host date digest content-type\",signature=\"{}\"",
            key_id, signature
        ),
    })
}

/// Vérifie la signature HTTP d'une requête entrante et retourne le `keyId`
/// utilisé par l'expéditeur.
pub fn signature_key_id(headers: &HeaderMap) -> Result<String> {
    let value = headers
        .get("signature")
        .or_else(|| headers.get("authorization"))
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| anyhow!("en-tête Signature absent"))?;

    let value = value.trim();
    let value = value.strip_prefix("Signature ").unwrap_or(value);
    let params = parse_signature_params(value);
    params
        .get("keyId")
        .cloned()
        .ok_or_else(|| anyhow!("keyId absent de la signature"))
}

pub fn verify_signature(
    headers: &HeaderMap,
    method: &str,
    path_and_query: &str,
    body: &[u8],
    public_key_pem: &str,
    expected_key_id: &str,
) -> Result<()> {
    let value = headers
        .get("signature")
        .or_else(|| headers.get("authorization"))
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| anyhow!("en-tête Signature absent"))?;
    let value = value.trim();
    let value = value.strip_prefix("Signature ").unwrap_or(value);
    let params = parse_signature_params(value);

    let key_id = params
        .get("keyId")
        .ok_or_else(|| anyhow!("keyId absent de la signature"))?;
    if key_id != expected_key_id {
        bail!("la signature ne correspond pas à la clé attendue");
    }

    // Vérifie le corps de la requête.
    let digest_header = headers
        .get("digest")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| anyhow!("en-tête Digest absent"))?;
    verify_digest(digest_header, body)?;

    // Empêche le rejeu de vieilles requêtes.
    let date_header = headers
        .get("date")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| anyhow!("en-tête Date absent"))?;
    let date = httpdate::parse_http_date(date_header)
        .context("en-tête Date invalide")?;
    let now = SystemTime::now();
    let age = now
        .duration_since(date)
        .or_else(|_| date.duration_since(now))
        .unwrap_or(Duration::ZERO);
    if age > Duration::from_secs(12 * 60 * 60) {
        bail!("signature trop ancienne");
    }

    let signed_headers = params
        .get("headers")
        .map(String::as_str)
        .unwrap_or("date");
    let mut signing_string = String::new();
    for (index, name) in signed_headers.split_whitespace().enumerate() {
        if index > 0 {
            signing_string.push('\n');
        }
        let value = match name {
            "(request-target)" => {
                format!("{} {}", method.to_ascii_lowercase(), path_and_query)
            }
            "(created)" => params
                .get("created")
                .cloned()
                .ok_or_else(|| anyhow!("paramètre created absent"))?,
            "(expires)" => params
                .get("expires")
                .cloned()
                .ok_or_else(|| anyhow!("paramètre expires absent"))?,
            other => headers
                .get(other)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| anyhow!("en-tête signé {} absent", other))?
                .to_string(),
        };
        signing_string.push_str(&format!("{}: {}", name, value));
    }

    let signature_b64 = params
        .get("signature")
        .ok_or_else(|| anyhow!("signature absente"))?;
    let signature_bytes = BASE64
        .decode(signature_b64)
        .context("signature base64 invalide")?;
    let signature = Signature::try_from(signature_bytes.as_slice())
        .map_err(|_| anyhow!("signature RSA invalide"))?;

    let public_key = parse_public_key(public_key_pem)?;
    let verifying_key = VerifyingKey::<Sha256>::new(public_key);
    verifying_key
        .verify(signing_string.as_bytes(), &signature)
        .map_err(|_| anyhow!("signature HTTP invalide"))
}

fn parse_signature_params(value: &str) -> std::collections::HashMap<String, String> {
    let mut params = std::collections::HashMap::new();
    for part in value.split(',') {
        if let Some((key, raw_value)) = part.trim().split_once('=') {
            let key = key.trim().to_string();
            let mut value = raw_value.trim();
            if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                value = &value[1..value.len() - 1];
            }
            params.insert(key, value.to_string());
        }
    }
    params
}

fn verify_digest(header: &str, body: &[u8]) -> Result<()> {
    let (algorithm, expected) = header
        .split_once('=')
        .ok_or_else(|| anyhow!("en-tête Digest invalide"))?;
    if !algorithm.trim().eq_ignore_ascii_case("sha-256") {
        bail!("algorithme de Digest non supporté : {}", algorithm);
    }
    let actual = BASE64.encode(Sha256::digest(body));
    if actual != expected.trim() {
        bail!("le Digest ne correspond pas au corps de la requête");
    }
    Ok(())
}

fn parse_public_key(pem: &str) -> Result<RsaPublicKey> {
    RsaPublicKey::from_public_key_pem(pem)
        .or_else(|_| RsaPublicKey::from_pkcs1_pem(pem))
        .context("clé publique RSA invalide")
}

/// Valide l'URL d'un acteur ou d'une inbox distante pour éviter de faire
/// pointer le serveur vers des adresses internes (SSRF).
pub fn validate_remote_url(value: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(value)?;
    let allow_http = std::env::var("VELOINFO_URL")
        .map(|base| base.starts_with("http://"))
        .unwrap_or(false);

    match url.scheme() {
        "https" => {}
        "http" if allow_http => {}
        scheme => bail!("schéma d'URL non supporté : {}", scheme),
    }

    if !allow_http {
        let host = url
            .host_str()
            .ok_or_else(|| anyhow!("URL sans hôte"))?;
        if host.eq_ignore_ascii_case("localhost") {
            bail!("adresse locale refusée");
        }
        if let Ok(ip) = host.parse::<IpAddr>() {
            let blocked = match ip {
                IpAddr::V4(ip) => {
                    ip.is_loopback()
                        || ip.is_private()
                        || ip.is_link_local()
                        || ip.is_unspecified()
                }
                IpAddr::V6(ip) => {
                    ip.is_loopback()
                        || ip.is_unspecified()
                        || ip.is_unique_local()
                        || ip.is_unicast_link_local()
                }
            };
            if blocked {
                bail!("adresse IP privée refusée");
            }
        }
    }

    Ok(url)
}

/// Client HTTP utilisé pour joindre d'autres instances du Fédivers.
/// Les redirections sont validées comme les URL initiales.
pub fn remote_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent(user_agent())
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5
                || validate_remote_url(attempt.url().as_str()).is_err()
            {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()?)
}

/// Récupère un acteur distant et met son profil en cache.
pub async fn fetch_remote_actor(conn: &PgPool, actor_id: &str) -> Result<RemoteActor> {
    let actor_url = validate_remote_url(actor_id)?;
    let client = remote_client()?;
    let keypair = keypair(conn).await?;
    let signed = sign_get_request(
        &keypair.private_key,
        &crate::activity_pub::key_id(),
        &actor_url,
    )?;

    let response = client
        .get(actor_url)
        .header(
            reqwest::header::ACCEPT,
            "application/activity+json, application/ld+json",
        )
        .header(reqwest::header::DATE, signed.date)
        .header("Signature", signed.signature)
        .send()
        .await
        .with_context(|| format!("récupération de l'acteur {}", actor_id))?;

    if !response.status().is_success() {
        // En cas d'échec réseau temporaire, on réutilise le cache.
        if let Some(cached) = RemoteActor::get(actor_id, conn).await? {
            return Ok(cached);
        }
        bail!("l'acteur {} a répondu {}", actor_id, response.status());
    }

    let json: serde_json::Value = response
        .json()
        .await
        .with_context(|| format!("réponse JSON invalide pour l'acteur {}", actor_id))?;

    let actor = parse_actor(actor_id, &json)?;
    RemoteActor::upsert(&actor, conn).await?;
    Ok(actor)
}

fn parse_actor(actor_id: &str, json: &serde_json::Value) -> Result<RemoteActor> {
    let id = json
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or(actor_id)
        .to_string();
    let inbox = json
        .get("inbox")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("l'acteur {} n'a pas d'inbox", id))?
        .to_string();
    let shared_inbox = json
        .get("endpoints")
        .and_then(|e| e.get("sharedInbox"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let public_key = json
        .get("publicKey")
        .ok_or_else(|| anyhow!("l'acteur {} n'a pas de clé publique", id))?;
    let public_key = if let Some(array) = public_key.as_array() {
        array
            .iter()
            .find(|key| {
                key.get("id")
                    .and_then(|v| v.as_str())
                    .map(|key_id| key_id.starts_with(&id))
                    .unwrap_or(false)
            })
            .or_else(|| array.first())
            .ok_or_else(|| anyhow!("l'acteur {} n'a pas de clé publique", id))?
    } else {
        public_key
    };

    let public_key_pem = public_key
        .get("publicKeyPem")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("l'acteur {} n'a pas de publicKeyPem", id))?
        .to_string();
    let public_key_id = public_key
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let display_name = json
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string);
    let username = json
        .get("preferredUsername")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(RemoteActor {
        id,
        inbox,
        shared_inbox,
        public_key_id,
        public_key_pem,
        display_name,
        username,
    })
}

/// Récupère la clé publique associée à un `keyId`. La plupart des serveurs
/// exposent la clé directement dans le document de l'acteur.
pub async fn fetch_public_key(
    conn: &PgPool,
    actor_id: &str,
    key_id: &str,
) -> Result<(RemoteActor, String)> {
    let actor = fetch_remote_actor(conn, actor_id).await?;
    if actor.public_key_id.as_deref() == Some(key_id) || key_id.starts_with(&format!("{}#", actor.id)) {
        let public_key_pem = actor.public_key_pem.clone();
        return Ok((actor, public_key_pem));
    }

    // Cas plus rare : la clé est exposée à une URL distincte.
    let key_url = validate_remote_url(key_id)?;
    let client = remote_client()?;
    let keypair = keypair(conn).await?;
    let signed = sign_get_request(
        &keypair.private_key,
        &crate::activity_pub::key_id(),
        &key_url,
    )?;
    let response = client
        .get(key_url)
        .header(
            reqwest::header::ACCEPT,
            "application/activity+json, application/ld+json",
        )
        .header(reqwest::header::DATE, signed.date)
        .header("Signature", signed.signature)
        .send()
        .await
        .with_context(|| format!("récupération de la clé {}", key_id))?;
    let json: serde_json::Value = response.json().await?;
    let pem = json
        .get("publicKeyPem")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("clé publique introuvable à {}", key_id))?
        .to_string();
    let owner = json.get("owner").and_then(|v| v.as_str());
    if let Some(owner) = owner {
        if owner != actor.id {
            bail!("la clé {} n'appartient pas à {}", key_id, actor.id);
        }
    }
    Ok((actor, pem))
}

fn user_agent() -> String {
    let base = std::env::var("VELOINFO_URL").unwrap_or_default();
    if base.is_empty() {
        "VeloInfo/0.1".to_string()
    } else {
        format!("VeloInfo/0.1 (+{})", base)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_signature_roundtrip() {
        let private_key = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let public_key_pem = private_key
            .to_public_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap();
        let key_id = "https://example.com/ap/actor#main-key";
        let url = reqwest::Url::parse("https://example.com/ap/actor/inbox").unwrap();
        let body = br#"{"type":"Create"}"#;

        let signed =
            sign_request(&private_key, key_id, "post", &url, body).unwrap();

        let mut headers = HeaderMap::new();
        headers.insert("host", "example.com".parse().unwrap());
        headers.insert("date", signed.date.parse().unwrap());
        headers.insert("digest", signed.digest.parse().unwrap());
        headers.insert("content-type", "application/activity+json".parse().unwrap());
        headers.insert("signature", signed.signature.parse().unwrap());

        verify_signature(
            &headers,
            "post",
            "/ap/actor/inbox",
            body,
            &public_key_pem,
            key_id,
        )
        .unwrap();
    }

    #[test]
    fn http_get_signature_roundtrip() {
        let private_key = RsaPrivateKey::new(&mut OsRng, 2048).unwrap();
        let key_id = "https://veloinfo.quebec/ap/actor#main-key";
        let url = reqwest::Url::parse("https://mastodon.social/users/test").unwrap();

        let signed = sign_get_request(&private_key, key_id, &url).unwrap();
        let params = parse_signature_params(&signed.signature);
        assert_eq!(params.get("keyId").map(String::as_str), Some(key_id));
        assert_eq!(
            params.get("headers").map(String::as_str),
            Some("(request-target) host date")
        );

        let signing_string = format!(
            "(request-target): get /users/test\nhost: mastodon.social\ndate: {}",
            signed.date
        );
        let signature_bytes = BASE64
            .decode(params.get("signature").unwrap())
            .unwrap();
        let signature = Signature::try_from(signature_bytes.as_slice()).unwrap();
        VerifyingKey::<Sha256>::new(private_key.to_public_key())
            .verify(signing_string.as_bytes(), &signature)
            .unwrap();
    }
}
