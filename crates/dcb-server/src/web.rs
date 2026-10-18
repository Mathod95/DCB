//! Routes HTTP : pages HTML (rendu serveur + htmx) et API JSON pour les futurs clients.
//!
//! Chaque modification écrit d'abord le fichier `cards/<slug>.toml` (source de vérité, à
//! commiter), puis met à jour le cache SQLite.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use askama::Template;
use axum::Json;
use axum::Router;
use axum::extract::{DefaultBodyLimit, Form, Multipart, Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use dcb_core::{Card, CardDraft, CardType, FieldError, Foil, is_valid_slug};
use serde::Deserialize;
use sqlx::SqlitePool;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::db::{self, Filter};
use crate::rules::Rules;
use crate::store::{CardStore, LoadError, is_safe_file_name};
use crate::views::*;

/// Taille max d'une illustration envoyée.
pub const MAX_ART_BYTES: usize = 5 * 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub store: Arc<CardStore>,
    /// Fichiers ignorés au dernier chargement, affichés dans la galerie.
    pub load_errors: Arc<RwLock<Vec<LoadError>>>,
    /// Fichier des règles (`rules/regles.toml`), absent dans certains tests.
    pub rules_path: Option<PathBuf>,
    pub rules: Arc<RwLock<RulesState>>,
}

/// Dernières règles valides, et l'erreur du dernier chargement s'il a échoué : une faute de
/// frappe dans le fichier n'efface pas la page, elle est signalée au-dessus.
#[derive(Default)]
pub struct RulesState {
    pub rules: Option<Rules>,
    pub error: Option<String>,
}

impl AppState {
    pub fn new(pool: SqlitePool, store: CardStore) -> Self {
        Self {
            pool,
            store: Arc::new(store),
            load_errors: Arc::default(),
            rules_path: None,
            rules: Arc::default(),
        }
    }

    pub fn with_rules_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.rules_path = Some(path.into());
        self
    }

    /// Relit `cards/*.toml` et reconstruit le cache. Renvoie le nombre de cartes chargées.
    pub async fn reload(&self) -> anyhow::Result<usize> {
        let _guard = self.store.lock().await;
        let (cards, errors) = self.store.load_all().await?;
        db::replace_all(&self.pool, &cards).await?;
        for e in &errors {
            tracing::warn!("cards/{e}");
        }
        *self.load_errors.write().expect("verrou empoisonné") = errors;
        self.reload_rules().await;
        Ok(cards.len())
    }

    async fn reload_rules(&self) {
        let Some(path) = &self.rules_path else { return };
        let loaded = Rules::load(path).await;
        let mut state = self.rules.write().expect("verrou empoisonné");
        match loaded {
            Ok(rules) => {
                *state = RulesState {
                    rules: Some(rules),
                    error: None,
                }
            }
            Err(e) => {
                tracing::warn!("règles: {e}");
                state.error = Some(e);
            }
        }
    }
}

pub fn router(state: AppState) -> Router {
    // Les illustrations gardent le même nom quand on les remplace : `no-cache` force le
    // navigateur à revalider (Last-Modified) au lieu d'afficher l'ancienne image.
    let art = Router::new()
        .fallback_service(ServeDir::new(state.store.art_dir()))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ));

    Router::new()
        .route("/", get(menu))
        .route("/gallery", get(gallery))
        .route("/regles", get(rules_page))
        .route("/reload", post(reload))
        .route("/cards/new", get(new_form))
        .route("/cards", post(create))
        .route("/cards/{slug}", get(detail).post(update))
        .route("/cards/{slug}/edit", get(edit_form))
        .route("/cards/{slug}/delete", post(delete))
        .route("/preview", post(preview))
        .route("/api/cards", get(api_list))
        .route("/api/cards/{slug}", get(api_get))
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
        .route("/static/foil/{name}", get(foil_texture))
        .route(
            "/static/app.js",
            get(|| async { asset("text/javascript", include_str!("../static/app.js")) }),
        )
        .nest("/art", art)
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

/// Textures des finitions holographiques, embarquées dans le binaire.
async fn foil_texture(Path(name): Path<String>) -> Response {
    let body = match name.as_str() {
        "stars.svg" => include_str!("../static/foil/stars.svg"),
        "glitter.svg" => include_str!("../static/foil/glitter.svg"),
        "shards.svg" => include_str!("../static/foil/shards.svg"),
        _ => return not_found_response(),
    };
    (
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        body,
    )
        .into_response()
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
    foil: Option<String>,
}

async fn gallery(
    State(s): State<AppState>,
    Query(q): Query<GalleryQuery>,
) -> AppResult<Html<String>> {
    // Une valeur inconnue dans l'URL revient à « pas de filtre » plutôt qu'à une erreur.
    let filter = Filter {
        card_type: q.card_type.as_deref().and_then(|v| v.parse().ok()),
        foil: q.foil.as_deref().and_then(|v| v.parse().ok()),
    };
    let cards = db::list(&s.pool, filter).await?;

    let href = |t: Option<CardType>, f: Option<Foil>| {
        let mut parts = Vec::new();
        if let Some(t) = t {
            parts.push(format!("type={t}"));
        }
        if let Some(f) = f {
            parts.push(format!("foil={f}"));
        }
        if parts.is_empty() {
            "/gallery".to_owned()
        } else {
            format!("/gallery?{}", parts.join("&"))
        }
    };

    let mut type_links = vec![FilterLink {
        label: "Tous",
        href: href(None, filter.foil),
        active: filter.card_type.is_none(),
        color: None,
    }];
    type_links.extend(CardType::ALL.iter().map(|&t| FilterLink {
        label: t.label(),
        href: href(Some(t), filter.foil),
        active: filter.card_type == Some(t),
        color: Some(t.color()),
    }));

    let mut foil_links = vec![FilterLink {
        label: "Toutes",
        href: href(filter.card_type, None),
        active: filter.foil.is_none(),
        color: None,
    }];
    foil_links.extend(Foil::ALL.iter().map(|&f| FilterLink {
        label: f.label(),
        href: href(filter.card_type, Some(f)),
        active: filter.foil == Some(f),
        color: None,
    }));

    let load_errors = s
        .load_errors
        .read()
        .expect("verrou empoisonné")
        .iter()
        .map(ToString::to_string)
        .collect();

    render(GalleryPage {
        cards: cards
            .iter()
            .map(|c| CardView::new(Some(&c.slug), &c.draft))
            .collect(),
        type_links,
        foil_links,
        filtered: filter.card_type.is_some() || filter.foil.is_some(),
        load_errors,
    })
}

async fn rules_page(State(s): State<AppState>) -> AppResult<Response> {
    let (rules, error) = {
        let state = s.rules.read().expect("verrou empoisonné");
        (state.rules.clone(), state.error.clone())
    };
    let Some(rules) = rules else {
        let page = RulesPage::missing(error);
        return Ok((StatusCode::SERVICE_UNAVAILABLE, render(page)?).into_response());
    };

    let counts = db::count_by_type(&s.pool).await?;
    let mut types = Vec::with_capacity(rules.types.items.len());
    for entry in &rules.types.items {
        let card = db::random_card(&s.pool, Some(entry.card_type)).await?;
        let count = counts
            .iter()
            .find(|(t, _)| *t == entry.card_type)
            .map_or(0, |(_, n)| *n);
        types.push(RulesType::new(entry, count, card.as_ref()));
    }

    // Une même carte pour les six finitions, pour que seule la finition change.
    let foils = match db::random_card(&s.pool, None).await? {
        Some(card) => Foil::ALL
            .iter()
            .map(|&foil| {
                let mut draft = card.draft.clone();
                draft.foil = foil;
                FoilSample {
                    label: foil.label(),
                    card: CardView::new(Some(&card.slug), &draft),
                }
            })
            .collect(),
        None => Vec::new(),
    };

    Ok(render(RulesPage::new(&rules, error, types, foils))?.into_response())
}

async fn reload(State(s): State<AppState>) -> AppResult<Redirect> {
    s.reload().await?;
    Ok(Redirect::to("/gallery"))
}

async fn load(pool: &SqlitePool, slug: &str) -> AppResult<Card> {
    if !is_valid_slug(slug) {
        return Err(AppError::NotFound);
    }
    db::get(pool, slug).await?.ok_or(AppError::NotFound)
}

fn card_file(s: &AppState, slug: &str) -> String {
    s.store
        .dir()
        .join(format!("{slug}.toml"))
        .display()
        .to_string()
}

async fn detail(State(s): State<AppState>, Path(slug): Path<String>) -> AppResult<Html<String>> {
    let card = load(&s.pool, &slug).await?;
    render(DetailPage {
        file: card_file(&s, &card.slug),
        art_file: card
            .draft
            .art
            .as_ref()
            .map(|a| s.store.art_dir().join(a).display().to_string()),
        card: CardView::new(Some(&card.slug), &card.draft),
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

async fn edit_form(State(s): State<AppState>, Path(slug): Path<String>) -> AppResult<Html<String>> {
    let card = load(&s.pool, &slug).await?;
    render(FormPage::new(
        "Modifier la carte",
        format!("/cards/{slug}"),
        format!("/cards/{slug}"),
        FormValues::from_draft(&card.draft),
        Vec::new(),
        CardView::new(Some(&slug), &card.draft),
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
        foil: if v.foil.is_empty() {
            Foil::Basic
        } else {
            pick(&v.foil, Foil::Basic, "foil", &mut errors)
        },
        description: v.description.clone(),
        effect: v.effect.clone(),
        deploy_cost: number(&v.deploy_cost, "deploy_cost", &mut errors),
        daily_cost: number(&v.daily_cost, "daily_cost", &mut errors),
        flavor_text: v.flavor_text.clone(),
        art: v.current_art.clone(),
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
            "foil" => v.foil = text,
            "description" => v.description = text,
            "effect" => v.effect = text,
            "deploy_cost" => v.deploy_cost = text,
            "daily_cost" => v.daily_cost = text,
            "flavor_text" => v.flavor_text = text,
            "remove_art" => remove_art = !text.is_empty(),
            // `current_art` est ignoré ici : la valeur de référence est celle déjà enregistrée.
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
    let mut draft = match sub.parse() {
        Ok(d) => d,
        Err((draft, errors)) => {
            return form_with_errors(
                "Nouvelle carte",
                "/cards".into(),
                "/gallery".into(),
                sub.values,
                &draft,
                errors,
            );
        }
    };

    let _guard = s.store.lock().await;
    let slug = s.store.unique_slug(&draft.name);
    draft.art = match &sub.art {
        Some(up) => Some(s.store.save_art(&slug, up.ext, &up.bytes).await?),
        None => None,
    };
    let card = Card { slug, draft };
    s.store.write(&card).await?;
    db::upsert(&s.pool, &card).await?;
    Ok(Redirect::to(&format!("/cards/{}", card.slug)).into_response())
}

async fn update(
    State(s): State<AppState>,
    Path(slug): Path<String>,
    mp: Multipart,
) -> AppResult<Response> {
    let existing = load(&s.pool, &slug).await?;
    let mut sub = read_submission(mp).await?;
    sub.values.current_art = existing.draft.art.clone();

    let mut draft = match sub.parse() {
        Ok(d) => d,
        Err((draft, errors)) => {
            return form_with_errors(
                "Modifier la carte",
                format!("/cards/{slug}"),
                format!("/cards/{slug}"),
                sub.values,
                &draft,
                errors,
            );
        }
    };

    let _guard = s.store.lock().await;
    let old_art = existing.draft.art;
    draft.art = match (&sub.art, sub.remove_art) {
        (Some(up), _) => Some(s.store.save_art(&slug, up.ext, &up.bytes).await?),
        (None, true) => None,
        (None, false) => old_art.clone(),
    };
    let card = Card { slug, draft };
    s.store.write(&card).await?;
    db::upsert(&s.pool, &card).await?;
    // L'ancienne illustration n'est supprimée qu'une fois la carte enregistrée. Même nom de
    // fichier (même extension) : elle vient d'être écrasée, rien à faire.
    if let Some(old) = old_art.filter(|old| Some(old) != card.draft.art.as_ref()) {
        s.store.remove_art(&old).await;
    }
    Ok(Redirect::to(&format!("/cards/{}", card.slug)).into_response())
}

async fn delete(State(s): State<AppState>, Path(slug): Path<String>) -> AppResult<Redirect> {
    let card = load(&s.pool, &slug).await?;
    let _guard = s.store.lock().await;
    s.store.remove(&slug).await?;
    db::delete(&s.pool, &slug).await?;
    if let Some(art) = &card.draft.art {
        s.store.remove_art(art).await;
    }
    Ok(Redirect::to("/gallery"))
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
    foil: String,
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
/// contient encore des erreurs. N'écrit rien.
async fn preview(Form(f): Form<PreviewForm>) -> AppResult<Html<String>> {
    let current_art =
        Some(f.current_art).filter(|a| f.remove_art.is_empty() && is_safe_file_name(a));
    let values = FormValues {
        name: f.name,
        card_type: f.card_type,
        skin: f.skin,
        foil: f.foil,
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
        foil: q
            .foil
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(parse_err)?,
    };
    Ok(Json(db::list(&s.pool, filter).await?))
}

async fn api_get(State(s): State<AppState>, Path(slug): Path<String>) -> AppResult<Json<Card>> {
    Ok(Json(load(&s.pool, &slug).await?))
}

async fn api_meta() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "card_types": CardType::ALL.iter().map(|t| serde_json::json!({
            "slug": t.slug(), "label": t.label(), "color": t.color()
        })).collect::<Vec<_>>(),
        "skins": dcb_core::Skin::ALL.iter().map(|s| s.slug()).collect::<Vec<_>>(),
        "foils": Foil::ALL.iter().map(|f| serde_json::json!({
            "slug": f.slug(), "label": f.label()
        })).collect::<Vec<_>>(),
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
}
