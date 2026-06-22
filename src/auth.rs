use axum::{extract::Query, response::Redirect};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

lazy_static! {
    static ref KEYCLOAK_SERVER_URL: String =
        std::env::var("KEYCLOAK_SERVER_URL").expect("KEYCLOAK_SERVER_URL must be set");
    static ref VELOINFO_URL: String =
        std::env::var("VELOINFO_URL").expect("VELOINFO_URL must be set");
}

#[derive(Debug, Deserialize)]
pub struct Auth {
    code: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Token {
    access_token: String,
    expires_in: i32,
    refresh_expires_in: i32,
    refresh_token: String,
    token_type: String,
}

pub async fn auth(auth: Query<Auth>, jar: CookieJar) -> (CookieJar, Redirect) {
    let code = auth.code.clone();

    let client = reqwest::Client::new();
    let redirect_uri = format!("{}{}", VELOINFO_URL.to_string(), "/auth");
    println!("redirect_uri: {:?}", redirect_uri);
    let params = [
        ("code", code.as_str()),
        ("grant_type", "authorization_code"),
        ("client_id", "veloinfo"),
        ("scope", "openid"),
        ("redirect_uri", redirect_uri.as_str()),
    ];
    let token_url = format!(
        "{}{}",
        KEYCLOAK_SERVER_URL.to_string(),
        "/protocol/openid-connect/token"
    );
    println!("POST url: {:?}", token_url);
    let token = match client.post(token_url.as_str()).form(&params).send().await {
        Ok(token) => match token.text().await {
            Ok(token) => match serde_json::from_str::<Token>(&token) {
                Ok(token) => token,
                Err(e) => {
                    eprintln!("Token: {:?}", token);
                    eprintln!("Error deserialize Token: {:?}", e);
                    return (jar.clone(), Redirect::to("/"));
                }
            },
            Err(e) => {
                eprintln!("Error get text from the response: {:?}", e);
                return (jar.clone(), Redirect::to("/"));
            }
        },
        Err(e) => {
            eprintln!("Error making the token request: {:?}", e);
            return (jar.clone(), Redirect::to("/"));
        }
    };

    let userinfo = match client
        .get(KEYCLOAK_SERVER_URL.to_string() + "/protocol/openid-connect/userinfo")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Authorization", format!("Bearer {}", token.access_token))
        .send()
        .await
    {
        Ok(userinfo) => match userinfo.json::<Userinfo>().await {
            Ok(userinfo) => userinfo,
            Err(e) => {
                eprintln!("Error json: {:?}", e);
                return (jar.clone(), Redirect::to("/"));
            }
        },
        Err(e) => {
            eprintln!("Error calling keycloak: {:?}", e);
            return (jar.clone(), Redirect::to("/"));
        }
    };

    (
        jar.clone().add(Cookie::new(
            "userinfo",
            match serde_json::to_string(&userinfo) {
                Ok(userinfo) => userinfo,
                Err(e) => {
                    eprintln!("Error json: {:?}", e);
                    return (jar, Redirect::to("/"));
                }
            },
        )),
        Redirect::to("/"),
    )
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Userinfo {
    pub sub: String,
    pub email: String,
    pub email_verified: bool,
    pub name: String,
    pub preferred_username: String,
    pub given_name: String,
    pub family_name: String,
}

/// Extrait l'UUID utilisateur depuis le cookie `userinfo` (Keycloak).
/// Retourne None si l'utilisateur n'est pas connecté ou si le cookie est invalide.
pub fn get_user_id_from_jar(jar: &CookieJar) -> Option<Uuid> {
    jar.get("userinfo").and_then(|cookie| {
        serde_json::from_str::<Userinfo>(cookie.value())
            .ok()
            .and_then(|info| Uuid::parse_str(&info.sub).ok())
    })
}

pub async fn logout(jar: CookieJar) -> (CookieJar, Redirect) {
    println!("Logout");

    (jar.remove(Cookie::build("userinfo")), Redirect::to("/"))
}
