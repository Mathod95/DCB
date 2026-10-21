//! Comptes et sessions.
//!
//! - Les comptes sont créés par un admin (pas d'inscription libre), mots de passe hachés en
//!   Argon2id.
//! - La session est un jeton aléatoire de 256 bits dans un cookie `HttpOnly; SameSite=Lax` ;
//!   seul son hash SHA-256 est stocké en base.
//! - Les handlers demandent un niveau d'accès avec les extracteurs [`CurrentUser`],
//!   [`AdminUser`] et [`EditorUser`].

use std::str::FromStr;

use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{FromRow, SqlitePool};

use crate::web::AppState;

pub const COOKIE_NAME: &str = "dcb_session";
/// Durée de vie d'une session : 30 jours.
pub const SESSION_SECONDS: i64 = 30 * 24 * 3600;
pub const USERNAME_MIN: usize = 3;
pub const USERNAME_MAX: usize = 32;
pub const PASSWORD_MIN: usize = 10;

pub async fn connect(url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    sqlx::migrate!("../../migrations-app").run(&pool).await?;
    Ok(pool)
}

// ---------------------------------------------------------------------------------------------
// Comptes

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Role {
    /// Tous les droits, y compris les comptes.
    Admin,
    /// Crée des cartes et modifie les siennes.
    Creator,
    /// Joueur : galerie et documentation, comme un visiteur, avec un compte pour la suite
    /// (decks, duels). Slug `user`, celui des comptes existants.
    User,
}

impl Role {
    /// Du moins au plus de droits, dans l'ordre des menus.
    pub const ALL: [Role; 3] = [Role::User, Role::Creator, Role::Admin];

    /// Peut créer des cartes (quand l'éditeur est ouvert).
    pub fn can_create(self) -> bool {
        matches!(self, Role::Admin | Role::Creator)
    }

    pub fn slug(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Creator => "creator",
            Role::User => "user",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Role::Admin => "Admin",
            Role::Creator => "Créateur",
            Role::User => "Joueur",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "admin" => Some(Role::Admin),
            "creator" => Some(Role::Creator),
            "user" => Some(Role::User),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub role: Role,
    pub active: bool,
    pub created_at: i64,
    pub last_login_at: Option<i64>,
}

impl User {
    pub fn is_admin(&self) -> bool {
        self.role == Role::Admin
    }
}

#[derive(FromRow)]
struct UserRow {
    id: i64,
    username: String,
    role: String,
    active: bool,
    created_at: i64,
    last_login_at: Option<i64>,
}

impl From<UserRow> for User {
    fn from(r: UserRow) -> Self {
        Self {
            id: r.id,
            username: r.username,
            // la contrainte CHECK garantit l'une des trois valeurs
            role: Role::parse(&r.role).unwrap_or(Role::User),
            active: r.active,
            created_at: r.created_at,
            last_login_at: r.last_login_at,
        }
    }
}

const USER_COLUMNS: &str = "id, username, role, active, created_at, last_login_at";

/// Identifiant : 3 à 32 caractères parmi `a-z 0-9 . _ -`, sans distinction de casse.
pub fn validate_username(name: &str) -> Result<String, String> {
    let name = name.trim().to_ascii_lowercase();
    let len = name.chars().count();
    if !(USERNAME_MIN..=USERNAME_MAX).contains(&len) {
        return Err(format!("entre {USERNAME_MIN} et {USERNAME_MAX} caractères"));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
    {
        return Err("lettres, chiffres, point, tiret et tiret bas uniquement".into());
    }
    Ok(name)
}

pub fn validate_password(password: &str) -> Result<(), String> {
    if password.chars().count() < PASSWORD_MIN {
        return Err(format!("{PASSWORD_MIN} caractères minimum"));
    }
    Ok(())
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("hachage du mot de passe impossible: {e}"))
}

fn verify_password(password: &str, hash: &str) -> bool {
    Argon2::default()
        .verify_password(password.as_bytes(), hash)
        .is_ok()
}

/// Mot de passe temporaire lisible, pour une invitation (16 caractères, sans caractères ambigus).
pub fn generate_password() -> String {
    const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("générateur aléatoire indisponible");
    bytes
        .iter()
        .map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char)
        .collect()
}

pub async fn count_users(pool: &SqlitePool) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await
}

pub async fn count_active_admins(pool: &SqlitePool) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role = 'admin' AND active = 1")
        .fetch_one(pool)
        .await
}

pub async fn list_users(pool: &SqlitePool) -> sqlx::Result<Vec<User>> {
    let sql = format!("SELECT {USER_COLUMNS} FROM users ORDER BY username");
    Ok(sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(sql))
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(User::from)
        .collect())
}

pub async fn find_user(pool: &SqlitePool, id: i64) -> sqlx::Result<Option<User>> {
    let sql = format!("SELECT {USER_COLUMNS} FROM users WHERE id = ?");
    Ok(sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .map(User::from))
}

pub async fn find_user_by_name(pool: &SqlitePool, username: &str) -> sqlx::Result<Option<User>> {
    let sql = format!("SELECT {USER_COLUMNS} FROM users WHERE username = ?");
    Ok(sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(sql))
        .bind(username.trim())
        .fetch_optional(pool)
        .await?
        .map(User::from))
}

/// Crée un compte. Renvoie une erreur lisible si l'identifiant est déjà pris.
pub async fn create_user(
    pool: &SqlitePool,
    username: &str,
    password: &str,
    role: Role,
) -> anyhow::Result<Result<i64, String>> {
    let username = match validate_username(username) {
        Ok(u) => u,
        Err(e) => return Ok(Err(format!("identifiant : {e}"))),
    };
    if let Err(e) = validate_password(password) {
        return Ok(Err(format!("mot de passe : {e}")));
    }
    if find_user_by_name(pool, &username).await?.is_some() {
        return Ok(Err(format!("l'identifiant « {username} » est déjà pris")));
    }
    let hash = hash_password(password)?;
    let id = sqlx::query("INSERT INTO users (username, password_hash, role) VALUES (?, ?, ?)")
        .bind(&username)
        .bind(hash)
        .bind(role.slug())
        .execute(pool)
        .await?
        .last_insert_rowid();
    Ok(Ok(id))
}

pub async fn set_password(pool: &SqlitePool, id: i64, password: &str) -> anyhow::Result<()> {
    let hash = hash_password(password)?;
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(hash)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_role(pool: &SqlitePool, id: i64, role: Role) -> sqlx::Result<()> {
    sqlx::query("UPDATE users SET role = ? WHERE id = ?")
        .bind(role.slug())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_active(pool: &SqlitePool, id: i64, active: bool) -> sqlx::Result<()> {
    sqlx::query("UPDATE users SET active = ? WHERE id = ?")
        .bind(active)
        .bind(id)
        .execute(pool)
        .await?;
    if !active {
        delete_sessions_of(pool, id).await?;
    }
    Ok(())
}

pub async fn delete_user(pool: &SqlitePool, id: i64) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Vérifie identifiant et mot de passe. Un compte inexistant coûte le même calcul qu'un compte
/// existant, pour ne pas révéler par le temps de réponse quels identifiants existent.
pub async fn check_login(
    pool: &SqlitePool,
    username: &str,
    password: &str,
) -> anyhow::Result<Option<User>> {
    let row: Option<(i64, String)> =
        sqlx::query_as("SELECT id, password_hash FROM users WHERE username = ? AND active = 1")
            .bind(username.trim())
            .fetch_optional(pool)
            .await?;
    let Some((id, hash)) = row else {
        let _ = verify_password(password, dummy_hash());
        return Ok(None);
    };
    if !verify_password(password, &hash) {
        return Ok(None);
    }
    sqlx::query("UPDATE users SET last_login_at = unixepoch() WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(find_user(pool, id).await?)
}

/// Hash Argon2id d'un mot de passe quelconque, vérifié quand l'identifiant n'existe pas.
fn dummy_hash() -> &'static str {
    static HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HASH.get_or_init(|| hash_password("compte-inexistant").expect("hachage"))
}

// ---------------------------------------------------------------------------------------------
// Sessions

fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Ouvre une session et renvoie le jeton à mettre dans le cookie.
pub async fn create_session(pool: &SqlitePool, user_id: i64) -> anyhow::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("aléa indisponible: {e}"))?;
    let token = hex::encode(bytes);
    sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, expires_at) VALUES (?, ?, unixepoch() + ?)",
    )
    .bind(token_hash(&token))
    .bind(user_id)
    .bind(SESSION_SECONDS)
    .execute(pool)
    .await?;
    // ménage des sessions expirées
    sqlx::query("DELETE FROM sessions WHERE expires_at < unixepoch()")
        .execute(pool)
        .await?;
    Ok(token)
}

pub async fn user_for_session(pool: &SqlitePool, token: &str) -> sqlx::Result<Option<User>> {
    let sql = format!(
        "SELECT {} FROM users u JOIN sessions s ON s.user_id = u.id \
         WHERE s.token_hash = ? AND s.expires_at > unixepoch() AND u.active = 1",
        USER_COLUMNS
            .split(", ")
            .map(|c| format!("u.{c}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(sqlx::query_as::<_, UserRow>(sqlx::AssertSqlSafe(sql))
        .bind(token_hash(token))
        .fetch_optional(pool)
        .await?
        .map(User::from))
}

pub async fn delete_session(pool: &SqlitePool, token: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(token_hash(token))
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_sessions_of(pool: &SqlitePool, user_id: i64) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// En-tête `Set-Cookie` qui ouvre la session.
pub fn session_cookie(token: &str, secure: bool) -> HeaderValue {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={SESSION_SECONDS}{secure}"
    ))
    .expect("cookie valide")
}

/// En-tête `Set-Cookie` qui efface la session.
pub fn clear_cookie(secure: bool) -> HeaderValue {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure}"
    ))
    .expect("cookie valide")
}

pub fn session_token(parts: &Parts) -> Option<String> {
    axum_extra::extract::CookieJar::from_headers(&parts.headers)
        .get(COOKIE_NAME)
        .map(|c| c.value().to_owned())
        .filter(|v| v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Seules les redirections internes sont suivies après connexion (pas de `//site.com`).
pub fn safe_next(next: Option<&str>) -> String {
    match next {
        Some(n) if n.starts_with('/') && !n.starts_with("//") && !n.starts_with("/\\") => {
            n.to_owned()
        }
        _ => "/".to_owned(),
    }
}

// ---------------------------------------------------------------------------------------------
// Extracteurs

/// Utilisateur connecté (ou `None`), résolu une fois par requête.
#[derive(Clone)]
struct Resolved(Option<User>);

async fn resolve(
    parts: &mut Parts,
    state: &AppState,
) -> Result<Option<User>, crate::web::AppError> {
    if let Some(Resolved(u)) = parts.extensions.get::<Resolved>() {
        return Ok(u.clone());
    }
    let user = match session_token(parts) {
        Some(token) => user_for_session(&state.app_db, &token).await?,
        None => None,
    };
    parts.extensions.insert(Resolved(user.clone()));
    Ok(user)
}

fn login_redirect(parts: &Parts) -> Response {
    let target = parts
        .uri
        .path_and_query()
        .map(|p| p.as_str())
        .unwrap_or("/");
    let next: String = target
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (b as char).to_string()
            }
            b => format!("%{b:02X}"),
        })
        .collect();
    let location = format!("/login?next={next}");
    // htmx suit les redirections en arrière-plan : on lui demande une vraie navigation.
    let mut res = Redirect::to(&location).into_response();
    if let Ok(v) = HeaderValue::from_str(&location) {
        res.headers_mut().insert("HX-Redirect", v);
    }
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

/// Visiteur, connecté ou non : accueil, galerie, fiches, documentation sont publics.
pub struct MaybeUser(pub Option<User>);

impl FromRequestParts<AppState> for MaybeUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(MaybeUser(
            resolve(parts, state)
                .await
                .map_err(IntoResponse::into_response)?,
        ))
    }
}

/// Utilisateur connecté ; sinon redirection vers /login (l'accueil avec le panneau de connexion
/// ouvert), qui ramène ensuite à la page demandée.
pub struct CurrentUser(pub User);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match resolve(parts, state)
            .await
            .map_err(IntoResponse::into_response)?
        {
            Some(u) => Ok(CurrentUser(u)),
            None => Err(login_redirect(parts)),
        }
    }
}

/// Admin connecté ; un utilisateur simple reçoit une page 403.
pub struct AdminUser(pub User);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let CurrentUser(u) = CurrentUser::from_request_parts(parts, state).await?;
        if u.is_admin() {
            Ok(AdminUser(u))
        } else {
            Err(crate::web::forbidden_response(Some(&u), state))
        }
    }
}

/// Compte qui peut créer des cartes (admin ou créateur), éditeur ouvert. L'éditeur est ouvert en
/// local (`DCB_EDITOR`) et en mode démo (`DCB_SANDBOX`) ; sinon ces pages répondent 404. Un
/// joueur reçoit une page 403 ; les droits sur chaque carte sont vérifiés dans les routes.
pub struct EditorUser(pub User);

impl FromRequestParts<AppState> for EditorUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let CurrentUser(u) = CurrentUser::from_request_parts(parts, state).await?;
        if !state.editor_open() {
            return Err(crate::web::not_found_for(Some(&u), state));
        }
        if u.role.can_create() {
            Ok(EditorUser(u))
        } else {
            Err(crate::web::forbidden_response(Some(&u), state))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames_are_normalized_and_checked() {
        assert_eq!(validate_username("  Mathod ").unwrap(), "mathod");
        assert_eq!(validate_username("jean.dupont_2").unwrap(), "jean.dupont_2");
        assert!(validate_username("ab").is_err());
        assert!(validate_username("avec espace").is_err());
        assert!(validate_username("é").is_err());
    }

    #[test]
    fn passwords_hash_and_verify() {
        let h = hash_password("un-mot-de-passe").unwrap();
        assert!(h.starts_with("$argon2id$"));
        assert!(verify_password("un-mot-de-passe", &h));
        assert!(!verify_password("autre-chose", &h));
        assert!(!verify_password("x", dummy_hash()));
        assert!(validate_password("court").is_err());
    }

    #[test]
    fn generated_passwords_are_valid() {
        let p = generate_password();
        assert_eq!(p.len(), 16);
        assert!(validate_password(&p).is_ok());
        assert_ne!(p, generate_password());
    }

    #[test]
    fn next_is_restricted_to_local_paths() {
        assert_eq!(safe_next(Some("/gallery?type=app")), "/gallery?type=app");
        assert_eq!(safe_next(Some("//evil.example")), "/");
        assert_eq!(safe_next(Some("https://evil.example")), "/");
        assert_eq!(safe_next(None), "/");
    }
}
