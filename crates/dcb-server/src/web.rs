//! Routes HTTP : pages HTML (rendu serveur + htmx) et API JSON pour les futurs clients.

use std::path::{Path as FsPath, PathBuf};

use askama::Template;
use axum::Json;
use axum::Router;
use axum::extract::{DefaultBodyLimit, Form, Multipart, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use dcb_core::{Card, CardDraft, CardType, FieldError, Rarity};
use serde::Deserialize;
use sqlx::SqlitePool;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::db::{self, Filter};
use crate::views::*;

/// Taille max d'une illustration envoyée.
pub const MAX_ART_BYTES: usize = 5 * 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub upload_dir: PathBuf,
}

pub fn router(state: AppState) -> Router {
    let uploads = ServeDir::new(&state.upload_dir);
    Router::new()
        .route("/", get(menu))
        .route("/gallery", get(gallery))
        .route("/cards/new", get(new_form))
        .route("/cards", post(create))
        .route("/cards/{id}", get(detail).post(update))
        .route("/cards/{id}/edit", get(edit_form))
        .route("/cards/{id}/delete", post(delete))
        .route("/preview", post(preview))
        .route("/api/cards", get(api_list))
        .route("/api/cards/{id}", get(api_get))
        .route("/api/meta", get(api_meta))
        .route(
            "/static/app.css",
            get(|| async { asset("text/css", include_str!("../static/app.css")) }),
        )
        .route(
            "/static/card.css",
            get(|| async { asset("text/css", include_str!("../static/card.css")) }),
        )
        .route(
            "/static/htmx.min.js",
            get(|| async {
                // htmx est servi par l'app (pas de dépendance à un CDN au runtime).
                asset(
                    "text/javascript",
                    include_str!("../static/htmx-2.0.11.min.js"),
                )
            }),
        )
        .route(
            "/static/app.js",
            get(|| async { asset("text/javascript", include_str!("../static/app.js")) }),
        )
        .nest_service("/uploads", uploads)
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(MAX_ART_BYTES + 64 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// ---------------------------------------------------------------------------------------------
// Erreurs

pub enum AppError {
    NotFound,
    BadRequest(String),
    Internal(anyhow::Error),
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(e: E) -> Self {
        AppError::Internal(e.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::NotFound => not_found_response(),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
            AppError::Internal(e) => {
                tracing::error!("{e:#}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Erreur interne").into_response()
            }
        }
    }
}

type AppResult<T> = Result<T, AppError>;

fn render(t: impl Template) -> AppResult<Html<String>> {
    Ok(Html(t.render()?))
}

fn not_found_response() -> Response {
    let body = NotFoundPage
        .render()
        .unwrap_or_else(|_| "Introuvable".into());
    (StatusCode::NOT_FOUND, Html(body)).into_response()
}

async fn not_found() -> Response {
    not_found_response()
}

fn asset(content_type: &'static str, body: &'static str) -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
}

// ---------------------------------------------------------------------------------------------
// Pages

async fn menu(State(s): State<AppState>) -> AppResult<Html<String>> {
    render(MenuPage {
        card_count: db::count(&s.pool).await?,
    })
}

#[derive(Deserialize, Default)]
struct GalleryQuery {
    #[serde(rename = "type")]
    card_type: Option<String>,
    rarity: Option<String>,
}

async fn gallery(
    State(s): State<AppState>,
    Query(q): Query<GalleryQuery>,
) -> AppResult<Html<String>> {
    // Une valeur inconnue dans l'URL revient à « pas de filtre » plutôt qu'à une erreur.
    let filter = Filter {
        card_type: q.card_type.as_deref().and_then(|v| v.parse().ok()),
        rarity: q.rarity.as_deref().and_then(|v| v.parse().ok()),
    };
    let cards = db::list(&s.pool, filter).await?;

    let href = |t: Option<CardType>, r: Option<Rarity>| {
        let mut parts = Vec::new();
        if let Some(t) = t {
            parts.push(format!("type={t}"));
        }
        if let Some(r) = r {
            parts.push(format!("rarity={r}"));
        }
        if parts.is_empty() {
            "/gallery".to_owned()
        } else {
            format!("/gallery?{}", parts.join("&"))
        }
    };

    let mut type_links = vec![FilterLink {
        label: "Tous",
        href: href(None, filter.rarity),
        active: filter.card_type.is_none(),
        color: None,
    }];
    type_links.extend(CardType::ALL.iter().map(|&t| FilterLink {
        label: t.label(),
        href: href(Some(t), filter.rarity),
        active: filter.card_type == Some(t),
        color: Some(t.color()),
    }));

    let mut rarity_links = vec![FilterLink {
        label: "Toutes",
        href: href(filter.card_type, None),
        active: filter.rarity.is_none(),
        color: None,
    }];
    rarity_links.extend(Rarity::ALL.iter().map(|&r| FilterLink {
        label: r.label(),
        href: href(filter.card_type, Some(r)),
        active: filter.rarity == Some(r),
        color: None,
    }));

    render(GalleryPage {
        cards: cards
            .iter()
            .map(|c| CardView::new(Some(c.id), &c.draft))
            .collect(),
        type_links,
        rarity_links,
        filtered: filter.card_type.is_some() || filter.rarity.is_some(),
    })
}

async fn load(pool: &SqlitePool, id: i64) -> AppResult<Card> {
    db::get(pool, id).await?.ok_or(AppError::NotFound)
}

async fn detail(State(s): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let card = load(&s.pool, id).await?;
    render(DetailPage {
        card: CardView::new(Some(card.id), &card.draft),
    })
}

async fn new_form() -> AppResult<Html<String>> {
    let values = FormValues::blank();
    let preview = parse_values(&values).unwrap_or_else(|(d, _)| d);
    render(FormPage::new(
        "Nouvelle carte",
        "/cards".into(),
        "/gallery".into(),
        values,
        Vec::new(),
        CardView::new(None, &preview),
    ))
}

async fn edit_form(State(s): State<AppState>, Path(id): Path<i64>) -> AppResult<Html<String>> {
    let card = load(&s.pool, id).await?;
    render(FormPage::new(
        "Modifier la carte",
        format!("/cards/{id}"),
        format!("/cards/{id}"),
        FormValues::from_draft(&card.draft),
        Vec::new(),
        CardView::new(Some(id), &card.draft),
    ))
}

// ---------------------------------------------------------------------------------------------
// Formulaire

/// Convertit les valeurs brutes en brouillon. En cas d'erreur, renvoie quand même un brouillon
/// « au mieux » pour l'aperçu, avec la liste des erreurs.
#[allow(clippy::result_large_err)] // appelé une fois par requête, la taille est sans effet
fn parse_values(v: &FormValues) -> Result<CardDraft, (CardDraft, Vec<FieldError>)> {
    let mut errors = Vec::new();

    fn pick<T: std::str::FromStr + Copy>(
        raw: &str,
        default: T,
        field: &'static str,
        errors: &mut Vec<FieldError>,
    ) -> T {
        raw.parse().unwrap_or_else(|_| {
            errors.push(FieldError {
                field,
                message: "valeur inconnue".into(),
            });
            default
        })
    }

    fn number(raw: &str, field: &'static str, errors: &mut Vec<FieldError>) -> i64 {
        let raw = raw.trim();
        if raw.is_empty() {
            return 0;
        }
        raw.parse().unwrap_or_else(|_| {
            errors.push(FieldError {
                field,
                message: "nombre attendu".into(),
            });
            0
        })
    }

    let draft = CardDraft {
        name: v.name.clone(),
        card_type: pick(&v.card_type, CardType::App, "card_type", &mut errors),
        skin: pick(&v.skin, dcb_core::Skin::Light, "skin", &mut errors),
        rarity: pick(&v.rarity, Rarity::Basic, "rarity", &mut errors),
        description: v.description.clone(),
        effect: v.effect.clone(),
        deploy_cost: number(&v.deploy_cost, "deploy_cost", &mut errors),
        daily_cost: number(&v.daily_cost, "daily_cost", &mut errors),
        flavor_text: v.flavor_text.clone(),
        art_path: v.current_art.clone(),
    };

    match draft.clone().validated() {
        Ok(d) if errors.is_empty() => Ok(d),
        Ok(d) => Err((d, errors)),
        Err(more) => {
            errors.extend(more);
            Err((draft, errors))
        }
    }
}

struct Upload {
    bytes: Vec<u8>,
    ext: &'static str,
}

struct Submission {
    values: FormValues,
    art: Option<Upload>,
    /// Fichier refusé : affiché comme erreur de formulaire plutôt qu'en 400 brut.
    art_error: Option<FieldError>,
    remove_art: bool,
}

impl Submission {
    /// Valide les champs et ajoute l'éventuelle erreur d'illustration.
    #[allow(clippy::result_large_err)]
    fn parse(&self) -> Result<CardDraft, (CardDraft, Vec<FieldError>)> {
        match (parse_values(&self.values), self.art_error.clone()) {
            (Ok(d), None) => Ok(d),
            (Ok(d), Some(e)) => Err((d, vec![e])),
            (Err((d, mut errs)), e) => {
                errs.extend(e);
                Err((d, errs))
            }
        }
    }
}

/// Identifie le format par les premiers octets plutôt que par le nom de fichier.
fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => Some("png"),
        [0xFF, 0xD8, 0xFF, ..] => Some("jpg"),
        [b'G', b'I', b'F', b'8', ..] => Some("gif"),
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => Some("webp"),
        _ => None,
    }
}

async fn read_submission(mut mp: Multipart) -> AppResult<Submission> {
    let mut v = FormValues::default();
    let mut art = None;
    let mut art_error = None;
    let mut remove_art = false;

    while let Some(field) = mp
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
    {
        let name = field.name().unwrap_or_default().to_owned();
        if name == "art" {
            let has_file = field.file_name().is_some_and(|f| !f.is_empty());
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(e.to_string()))?;
            if has_file && !bytes.is_empty() {
                match sniff_image(&bytes) {
                    Some(ext) => {
                        art = Some(Upload {
                            bytes: bytes.to_vec(),
                            ext,
                        })
                    }
                    None => {
                        art_error = Some(FieldError {
                            field: "art",
                            message: "PNG, JPEG, GIF ou WebP attendu".into(),
                        })
                    }
                }
            }
            continue;
        }
        let text = field
            .text()
            .await
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        match name.as_str() {
            "name" => v.name = text,
            "card_type" => v.card_type = text,
            "skin" => v.skin = text,
            "rarity" => v.rarity = text,
            "description" => v.description = text,
            "effect" => v.effect = text,
            "deploy_cost" => v.deploy_cost = text,
            "daily_cost" => v.daily_cost = text,
            "flavor_text" => v.flavor_text = text,
            "remove_art" => remove_art = !text.is_empty(),
            // `current_art` est ignoré ici : la valeur de référence est celle en base.
            _ => {}
        }
    }
    Ok(Submission {
        values: v,
        art,
        art_error,
        remove_art,
    })
}

async fn save_upload(dir: &FsPath, up: Upload) -> AppResult<String> {
    tokio::fs::create_dir_all(dir).await?;
    let file = format!("{}.{}", uuid::Uuid::new_v4(), up.ext);
    tokio::fs::write(dir.join(&file), up.bytes).await?;
    Ok(format!("/uploads/{file}"))
}

/// Supprime un fichier d'illustration à partir de son chemin public. Ignore les chemins
/// qui ne pointent pas dans le dossier d'upload.
async fn remove_upload(dir: &FsPath, public: &str) {
    if let Some(file) = public
        .strip_prefix("/uploads/")
        .filter(|f| is_safe_file_name(f))
        && let Err(e) = tokio::fs::remove_file(dir.join(file)).await
    {
        tracing::warn!("suppression de {public} impossible: {e}");
    }
}

fn is_safe_file_name(f: &str) -> bool {
    !f.is_empty()
        && f.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        && !f.contains("..")
}

/// Rend le formulaire avec ses erreurs (statut 422).
fn form_with_errors(
    title: &'static str,
    action: String,
    cancel_href: String,
    values: FormValues,
    draft: &CardDraft,
    errors: Vec<FieldError>,
) -> AppResult<Response> {
    let page = FormPage::new(
        title,
        action,
        cancel_href,
        values,
        errors,
        CardView::new(None, draft),
    );
    Ok((StatusCode::UNPROCESSABLE_ENTITY, render(page)?).into_response())
}

async fn create(State(s): State<AppState>, mp: Multipart) -> AppResult<Response> {
    let sub = read_submission(mp).await?;
    match sub.parse() {
        Ok(mut draft) => {
            draft.art_path = match sub.art {
                Some(up) => Some(save_upload(&s.upload_dir, up).await?),
                None => None,
            };
            let id = db::insert(&s.pool, &draft).await?;
            Ok(Redirect::to(&format!("/cards/{id}")).into_response())
        }
        Err((draft, errors)) => form_with_errors(
            "Nouvelle carte",
            "/cards".into(),
            "/gallery".into(),
            sub.values,
            &draft,
            errors,
        ),
    }
}

async fn update(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    mp: Multipart,
) -> AppResult<Response> {
    let existing = load(&s.pool, id).await?;
    let mut sub = read_submission(mp).await?;
    sub.values.current_art = existing.draft.art_path.clone();

    match sub.parse() {
        Ok(mut draft) => {
            let old_art = existing.draft.art_path;
            draft.art_path = match (sub.art, sub.remove_art) {
                (Some(up), _) => Some(save_upload(&s.upload_dir, up).await?),
                (None, true) => None,
                (None, false) => old_art.clone(),
            };
            if !db::update(&s.pool, id, &draft).await? {
                return Err(AppError::NotFound);
            }
            if let Some(old) = old_art.filter(|old| Some(old) != draft.art_path.as_ref()) {
                remove_upload(&s.upload_dir, &old).await;
            }
            Ok(Redirect::to(&format!("/cards/{id}")).into_response())
        }
        Err((draft, errors)) => form_with_errors(
            "Modifier la carte",
            format!("/cards/{id}"),
            format!("/cards/{id}"),
            sub.values,
            &draft,
            errors,
        ),
    }
}

async fn delete(State(s): State<AppState>, Path(id): Path<i64>) -> AppResult<Redirect> {
    match db::delete(&s.pool, id).await? {
        None => Err(AppError::NotFound),
        Some(art) => {
            if let Some(art) = art {
                remove_upload(&s.upload_dir, &art).await;
            }
            Ok(Redirect::to("/gallery"))
        }
    }
}

#[derive(Deserialize)]
struct PreviewForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    card_type: String,
    #[serde(default)]
    skin: String,
    #[serde(default)]
    rarity: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    effect: String,
    #[serde(default)]
    deploy_cost: String,
    #[serde(default)]
    daily_cost: String,
    #[serde(default)]
    flavor_text: String,
    #[serde(default)]
    current_art: String,
    #[serde(default)]
    remove_art: String,
}

/// Aperçu en direct : rend la carte telle qu'elle serait enregistrée, même si le formulaire
/// contient encore des erreurs.
async fn preview(Form(f): Form<PreviewForm>) -> AppResult<Html<String>> {
    let current_art = Some(f.current_art).filter(|p| {
        f.remove_art.is_empty() && p.strip_prefix("/uploads/").is_some_and(is_safe_file_name)
    });
    let values = FormValues {
        name: f.name,
        card_type: f.card_type,
        skin: f.skin,
        rarity: f.rarity,
        description: f.description,
        effect: f.effect,
        deploy_cost: f.deploy_cost,
        daily_cost: f.daily_cost,
        flavor_text: f.flavor_text,
        current_art,
    };
    let draft = parse_values(&values).unwrap_or_else(|(d, _)| d);
    render(CardPreview {
        card: CardView::new(None, &draft),
    })
}

// ---------------------------------------------------------------------------------------------
// API JSON (pour le futur moteur de duel et les clients)

async fn api_list(
    State(s): State<AppState>,
    Query(q): Query<GalleryQuery>,
) -> AppResult<Json<Vec<Card>>> {
    let parse_err = |e: dcb_core::UnknownVariant| AppError::BadRequest(e.to_string());
    let filter = Filter {
        card_type: q
            .card_type
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(parse_err)?,
        rarity: q
            .rarity
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(parse_err)?,
    };
    Ok(Json(db::list(&s.pool, filter).await?))
}

async fn api_get(State(s): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<Card>> {
    Ok(Json(load(&s.pool, id).await?))
}

async fn api_meta() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "card_types": CardType::ALL.iter().map(|t| serde_json::json!({
            "slug": t.slug(), "label": t.label(), "color": t.color()
        })).collect::<Vec<_>>(),
        "skins": dcb_core::Skin::ALL.iter().map(|s| s.slug()).collect::<Vec<_>>(),
        "rarities": Rarity::ALL.iter().map(|r| r.slug()).collect::<Vec<_>>(),
        "limits": {
            "name": dcb_core::limits::NAME,
            "description": dcb_core::limits::DESCRIPTION,
            "effect": dcb_core::limits::EFFECT,
            "flavor_text": dcb_core::limits::FLAVOR,
            "cost_max": dcb_core::limits::COST_MAX,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_common_formats() {
        assert_eq!(sniff_image(b"\x89PNG\r\n\x1a\n...."), Some("png"));
        assert_eq!(sniff_image(b"\xFF\xD8\xFF\xE0...."), Some("jpg"));
        assert_eq!(sniff_image(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(sniff_image(b"<svg xmlns="), None);
    }

    #[test]
    fn rejects_unsafe_upload_names() {
        assert!(is_safe_file_name("3f2a-11.png"));
        assert!(!is_safe_file_name("../dcb.db"));
        assert!(!is_safe_file_name("a/b.png"));
        assert!(!is_safe_file_name(""));
    }
}
