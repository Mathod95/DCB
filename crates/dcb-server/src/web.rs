//! Routes HTTP : pages HTML (rendu serveur + htmx) et API JSON pour les futurs clients.
//!
//! Chaque modification écrit d'abord le fichier `cards/<slug>.toml` (source de vérité, à
//! commiter), puis met à jour le cache SQLite.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use askama::Template;
use axum::Json;
use axum::Router;
use axum::extract::Request;
use axum::extract::{DefaultBodyLimit, Form, Multipart, Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use dcb_core::ArtPresence;
use dcb_core::Background;
use dcb_core::CardVersion;
use dcb_core::DescStyle;
use dcb_core::EffectStyle;
use dcb_core::Refs;
use dcb_core::Relief;
use dcb_core::{
    Badge, Card, CardDraft, CardType, FieldError, Foil, Shape, Size, Title, is_valid_slug,
};
use serde::Deserialize;
use sqlx::SqlitePool;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::auth::{EditorUser, MaybeUser, User};
use crate::concepts::{self, ConceptsDoc};
use crate::db::{self, Filter};
use crate::rules::Rules;
use crate::store::{CardStore, LoadError, is_safe_file_name};
use crate::views::*;

/// Taille max d'une illustration envoyée.
pub const MAX_ART_BYTES: usize = 5 * 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    /// Cache des cartes, reconstruit depuis `cards/` à chaque démarrage.
    pub pool: SqlitePool,
    /// Base persistante : comptes et sessions (`data/app.db`).
    pub app_db: SqlitePool,
    /// Éditeur de cartes (création, modification, rechargement), réservé à l'admin. Coupé en
    /// production (`DCB_EDITOR=0`) : les cartes arrivent alors avec l'image.
    pub editor_enabled: bool,
    /// Mode démo (`DCB_SANDBOX=1`) : tout joueur connecté peut créer des cartes et ne modifie
    /// que les siennes ; l'admin modifie tout. Les cartes vivent dans le conteneur et
    /// disparaissent au déploiement suivant.
    pub sandbox: bool,
    /// Auteur de chaque carte créée en ligne (slug → id du compte). Gardé en mémoire : il
    /// disparaît avec le conteneur, comme les cartes de démo.
    pub owners: Arc<RwLock<HashMap<String, i64>>>,
    /// Cookie de session marqué `Secure` (servi en HTTPS).
    pub secure_cookies: bool,
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
    /// Texte de la page Concepts (`rules/concepts.toml`, à côté des règles).
    pub concepts: Option<ConceptsDoc>,
    pub concepts_error: Option<String>,
}

impl AppState {
    pub fn new(pool: SqlitePool, app_db: SqlitePool, store: CardStore) -> Self {
        Self {
            pool,
            app_db,
            editor_enabled: true,
            sandbox: false,
            owners: Arc::default(),
            secure_cookies: false,
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

    pub fn with_editor(mut self, enabled: bool) -> Self {
        self.editor_enabled = enabled;
        self
    }

    pub fn with_sandbox(mut self, enabled: bool) -> Self {
        self.sandbox = enabled;
        self
    }

    /// L'éditeur est ouvert (admin en local, ou tout le monde en mode démo).
    pub fn editor_open(&self) -> bool {
        self.editor_enabled || self.sandbox
    }

    /// Le compte peut modifier ou supprimer cette carte : l'admin toujours, un créateur
    /// seulement les cartes qu'il a créées en ligne.
    pub fn may_modify(&self, user: &User, slug: &str) -> bool {
        if !self.editor_open() {
            return false;
        }
        user.is_admin()
            || (user.role.can_create()
                && self.owners.read().expect("verrou empoisonné").get(slug) == Some(&user.id))
    }

    /// Cartes créées en ligne par ce compte (« Mes cartes »).
    pub fn owned_by(&self, user_id: i64) -> Vec<String> {
        self.owners
            .read()
            .expect("verrou empoisonné")
            .iter()
            .filter(|(_, id)| **id == user_id)
            .map(|(slug, _)| slug.clone())
            .collect()
    }

    pub fn with_secure_cookies(mut self, secure: bool) -> Self {
        self.secure_cookies = secure;
        self
    }

    pub fn ctx(&self, user: &User) -> Ctx {
        Ctx::for_user(Some(user), self.editor_enabled, self.sandbox)
    }

    /// Contexte d'une page publique : visiteur ou compte connecté.
    pub fn ctx_for(&self, user: Option<&User>) -> Ctx {
        Ctx::for_user(user, self.editor_enabled, self.sandbox)
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
        let concepts = ConceptsDoc::load(&path.with_file_name("concepts.toml")).await;
        let mut state = self.rules.write().expect("verrou empoisonné");
        match loaded {
            Ok(rules) => {
                state.rules = Some(rules);
                state.error = None;
            }
            Err(e) => {
                tracing::warn!("règles: {e}");
                state.error = Some(e);
            }
        }
        match concepts {
            Ok(doc) => {
                state.concepts = Some(doc);
                state.concepts_error = None;
            }
            Err(e) => {
                tracing::warn!("concepts: {e}");
                state.concepts_error = Some(e);
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
        .route("/plateau", get(board_page))
        .route("/plateau/complet", get(board_full_page))
        .route("/regles", get(rules_page))
        .route("/regles/concepts", get(concepts_page))
        .route("/regles/{page}", get(doc_page))
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
            "/static/concepts.css",
            get(|| async { asset("text/css", include_str!("../static/concepts.css")) }),
        )
        .route(
            "/static/concepts.js",
            get(|| async { asset("text/javascript", include_str!("../static/concepts.js")) }),
        )
        .route(
            "/static/rules.js",
            get(|| async { asset("text/javascript", include_str!("../static/rules.js")) }),
        )
        .route(
            "/static/anatomy.js",
            get(|| async { asset("text/javascript", include_str!("../static/anatomy.js")) }),
        )
        .route(
            "/static/app.js",
            get(|| async { asset("text/javascript", include_str!("../static/app.js")) }),
        )
        .nest("/art", art)
        .merge(crate::accounts::routes())
        .fallback(not_found)
        .layer(middleware::from_fn(same_origin_only))
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

pub(crate) type AppResult<T> = Result<T, AppError>;

pub(crate) fn render(t: impl Template) -> AppResult<Html<String>> {
    Ok(Html(t.render()?))
}

fn not_found_with(ctx: Ctx) -> Response {
    let body = NotFoundPage { ctx }
        .render()
        .unwrap_or_else(|_| "Introuvable".into());
    (StatusCode::NOT_FOUND, Html(body)).into_response()
}

fn not_found_response() -> Response {
    not_found_with(Ctx::anonymous())
}

pub(crate) fn not_found_for(user: Option<&User>, state: &AppState) -> Response {
    not_found_with(Ctx::for_user(user, state.editor_enabled, state.sandbox))
}

pub(crate) fn forbidden_response(user: Option<&User>, state: &AppState) -> Response {
    let body = ForbiddenPage {
        ctx: Ctx::for_user(user, state.editor_enabled, state.sandbox),
    }
    .render()
    .unwrap_or_else(|_| "Accès refusé".into());
    (StatusCode::FORBIDDEN, Html(body)).into_response()
}

async fn not_found(State(s): State<AppState>, MaybeUser(u): MaybeUser) -> Response {
    not_found_for(u.as_ref(), &s)
}

/// Middleware : refuse les envois de formulaire venant d'un autre site (protection CSRF, en
/// plus du cookie `SameSite=Lax`). Les navigateurs envoient `Origin` sur les POST ; il doit
/// correspondre à l'hôte de la requête.
async fn same_origin_only(req: Request, next: Next) -> Response {
    let safe_method = matches!(
        *req.method(),
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    );
    if !safe_method && let Some(origin) = req.headers().get(header::ORIGIN) {
        let host = req
            .headers()
            .get(header::HOST)
            .and_then(|h| h.to_str().ok());
        let origin_host = origin
            .to_str()
            .ok()
            .and_then(|o| o.split_once("://"))
            .map(|(_, rest)| rest.trim_end_matches('/'));
        if host.is_none() || origin_host != host {
            return (
                StatusCode::FORBIDDEN,
                "Requête refusée : origine différente",
            )
                .into_response();
        }
    }
    next.run(req).await
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

async fn menu(State(s): State<AppState>, MaybeUser(u): MaybeUser) -> AppResult<Html<String>> {
    menu_page(&s, s.ctx_for(u.as_ref())).await
}

/// Maquette du plateau de jeu, publique : la moitié du joueur, à la taille de la page.
async fn board_page(State(s): State<AppState>, MaybeUser(u): MaybeUser) -> AppResult<Html<String>> {
    render(BoardPage {
        ctx: s.ctx_for(u.as_ref()),
        full: false,
    })
}

/// Le plateau entier avec l'adversaire, pour exemple.
async fn board_full_page(
    State(s): State<AppState>,
    MaybeUser(u): MaybeUser,
) -> AppResult<Html<String>> {
    render(BoardPage {
        ctx: s.ctx_for(u.as_ref()),
        full: true,
    })
}

/// Accueil, aussi rendu par `/login` avec le panneau de connexion ouvert.
pub(crate) async fn menu_page(s: &AppState, ctx: Ctx) -> AppResult<Html<String>> {
    let build = env_nonempty("DCB_CHANNEL").map(|channel| {
        let version = s
            .rules
            .read()
            .expect("verrou empoisonné")
            .rules
            .as_ref()
            .and_then(|r| r.releases.first().map(|v| v.version.clone()));
        BuildBadge {
            channel,
            version,
            commit: env_nonempty("DCB_COMMIT").map(|c| c.chars().take(7).collect()),
        }
    });
    render(MenuPage {
        ctx,
        card_count: db::count(&s.pool).await?,
        build,
    })
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

/// URL de la galerie pour un filtre.
fn gallery_href(f: &Filter, mine: bool, view: &[(String, String)]) -> String {
    let mut q: Vec<String> = f.pairs().iter().map(|(k, v)| format!("{k}={v}")).collect();
    if mine {
        q.push("mine=1".to_owned());
    }
    q.extend(view.iter().map(|(k, v)| format!("{k}={v}")));
    if q.is_empty() {
        return "/gallery".to_owned();
    }
    format!("/gallery?{}", q.join("&"))
}

/// Réglages d'affichage de la galerie (`v_skin=dark&v_size=compact`…). Une valeur inconnue est
/// ignorée ; vide : la carte garde son réglage.
struct GalleryView {
    /// Paires reconnues, dans l'ordre de l'URL (pour garder l'affichage dans les liens).
    pairs: Vec<(String, String)>,
    draft: Vec<(String, String)>,
    /// Skin de la documentation : classes du conteneur et calque animé.
    wrap: Option<(dcb_core::Skin, String, Option<&'static str>)>,
}

/// Réglages d'affichage, dans l'ordre du panneau : clé d'URL et titre.
const VIEW_KEYS: &[(&str, &str)] = &[
    ("v_skin", "Skin"),
    ("v_foil", "Finition"),
    ("v_shape", "Contour"),
    ("v_size", "Taille"),
    ("v_badge", "Badge de type"),
    ("v_title", "Nom"),
    ("v_desc", "Description"),
    ("v_effect", "Bloc Effect"),
    ("v_refs", "Cartes citées"),
    ("v_relief", "Relief"),
    ("v_bg", "Background"),
    ("v_version", "Version"),
];

impl GalleryView {
    fn from_pairs(q: &[(String, String)]) -> Self {
        let mut v = Self {
            pairs: Vec::new(),
            draft: Vec::new(),
            wrap: None,
        };
        for (k, val) in q {
            let val = val.trim();
            if val.is_empty() || !VIEW_KEYS.iter().any(|(key, _)| key == k) {
                continue;
            }
            let ok = match (k.as_str(), concepts::skin_style(val)) {
                ("v_skin", Some((skin, wrap, fx))) if !wrap.is_empty() => {
                    v.wrap = Some((skin, wrap, fx));
                    true
                }
                _ => {
                    let ok = view_value_ok(k, val);
                    if ok {
                        v.draft.push((k.clone(), val.to_owned()));
                    }
                    ok
                }
            };
            if ok {
                v.pairs.retain(|(pk, _)| pk != k);
                v.pairs.push((k.clone(), val.to_owned()));
            }
        }
        v
    }

    fn value(&self, key: &str) -> &str {
        self.pairs
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map_or("", |(_, v)| v.as_str())
    }

    /// La carte telle que la galerie l'affiche.
    fn card(&self, c: &Card, idx: &RefIndex) -> GalleryCard {
        let mut d = c.draft.clone();
        if let Some((_, v)) = self.draft.iter().find(|(k, _)| k == "v_version") {
            set_view(&mut d, "v_version", v);
        }
        // la v2 fixe ses réglages ; ceux de l'affichage passent ensuite par-dessus
        let v2 = d.version == CardVersion::V2;
        let mut d = d.effective();
        d.version = CardVersion::V1;
        for (k, v) in &self.draft {
            if k != "v_version" {
                set_view(&mut d, k, v);
            }
        }
        let mut wrap = String::new();
        if let Some((skin, w, _)) = &self.wrap {
            d.skin = *skin;
            wrap = w.clone();
        }
        let mut view = CardView::new(Some(&c.slug), &d).cite(idx, false);
        if v2 {
            view.version = CardVersion::V2.slug();
            view.version_label = CardVersion::V2.label();
        }
        if let Some((_, _, Some(k))) = &self.wrap {
            view.fx = concepts::fx(k, &c.slug);
            view.data_text = matches!(*k, "ascii" | "glitch");
            if *k == "ascii" {
                view.ascii_word = Some(concepts::ascii_word(d.card_type));
            }
        }
        GalleryCard { card: view, wrap }
    }
}

/// La valeur est connue pour ce réglage d'affichage.
fn view_value_ok(key: &str, val: &str) -> bool {
    match key {
        "v_skin" => val.parse::<dcb_core::Skin>().is_ok(),
        "v_foil" => val.parse::<Foil>().is_ok(),
        "v_shape" => val.parse::<Shape>().is_ok(),
        "v_size" => val.parse::<Size>().is_ok(),
        "v_badge" => val.parse::<Badge>().is_ok(),
        "v_title" => val.parse::<Title>().is_ok(),
        "v_desc" => val.parse::<DescStyle>().is_ok(),
        "v_effect" => val.parse::<EffectStyle>().is_ok(),
        "v_refs" => val.parse::<Refs>().is_ok(),
        "v_relief" => val.parse::<Relief>().is_ok(),
        "v_bg" => val.parse::<Background>().is_ok(),
        "v_version" => val.parse::<CardVersion>().is_ok(),
        _ => false,
    }
}

/// Applique un réglage d'affichage à une carte ; `false` si la valeur est inconnue.
fn set_view(d: &mut CardDraft, key: &str, val: &str) -> bool {
    fn set<T: std::str::FromStr>(field: &mut T, val: &str) -> bool {
        match val.parse() {
            Ok(v) => {
                *field = v;
                true
            }
            Err(_) => false,
        }
    }
    match key {
        "v_skin" => set(&mut d.skin, val),
        "v_foil" => set(&mut d.foil, val),
        "v_shape" => set(&mut d.shape, val),
        "v_size" => set(&mut d.size, val),
        "v_badge" => set(&mut d.badge, val),
        "v_title" => set(&mut d.title, val),
        "v_desc" => set(&mut d.desc_style, val),
        "v_effect" => set(&mut d.effect_style, val),
        "v_refs" => set(&mut d.refs, val),
        "v_relief" => set(&mut d.relief, val),
        "v_bg" => set(&mut d.background, val),
        "v_version" => set(&mut d.version, val),
        _ => false,
    }
}

/// Menus du panneau Affichage : « Comme enregistré » puis les valeurs de chaque réglage. Les
/// skins de la documentation (classiques et animées) suivent Light et Dark.
fn view_groups(s: &AppState, view: &GalleryView) -> Vec<ViewGroup> {
    fn values<T: Copy>(
        all: &[T],
        slug: fn(T) -> &'static str,
        label: fn(T) -> &'static str,
    ) -> Vec<(String, String, &'static str)> {
        all.iter()
            .map(|&v| (slug(v).to_owned(), label(v).to_owned(), ""))
            .collect()
    }
    let doc_skins: Vec<(String, String, &'static str)> = {
        let rules = s.rules.read().expect("verrou empoisonné");
        rules.rules.as_ref().map_or_else(Vec::new, |r| {
            let sk = &r.concepts.skin;
            sk.classic
                .iter()
                .map(|i| (i.key.clone(), i.title.clone(), "Skins classiques"))
                .chain(
                    sk.animated
                        .iter()
                        .map(|i| (i.key.clone(), i.title.clone(), "Skins animées")),
                )
                .filter(|(k, _, _)| !matches!(k.as_str(), "light" | "dark" | "current"))
                .collect()
        })
    };
    VIEW_KEYS
        .iter()
        .map(|&(key, title)| {
            let mut vals = match key {
                "v_skin" => values(
                    dcb_core::Skin::ALL,
                    dcb_core::Skin::slug,
                    dcb_core::Skin::label,
                ),
                "v_foil" => values(Foil::ALL, Foil::slug, Foil::label),
                "v_shape" => values(Shape::ALL, Shape::slug, Shape::label),
                "v_size" => values(Size::ALL, Size::slug, Size::label),
                "v_badge" => values(Badge::ALL, Badge::slug, Badge::label),
                "v_title" => values(Title::ALL, Title::slug, Title::label),
                "v_desc" => values(DescStyle::ALL, DescStyle::slug, DescStyle::label),
                "v_effect" => values(EffectStyle::ALL, EffectStyle::slug, EffectStyle::label),
                "v_refs" => values(Refs::ALL, Refs::slug, Refs::label),
                "v_bg" => values(Background::ALL, Background::slug, Background::label),
                "v_version" => values(CardVersion::ALL, CardVersion::slug, CardVersion::label),
                _ => values(Relief::ALL, Relief::slug, Relief::label),
            };
            if key == "v_skin" {
                vals.extend(doc_skins.iter().cloned());
            }
            let current = view.value(key);
            let summary = vals
                .iter()
                .find(|(v, _, _)| v == current)
                .map_or("Inchangé".to_owned(), |(_, l, _)| l.clone());
            let mut options = vec![ViewOption {
                value: String::new(),
                label: "Comme enregistré".to_owned(),
                checked: current.is_empty(),
                section: "",
            }];
            let mut prev = "";
            options.extend(vals.into_iter().map(|(value, label, section)| {
                // sous-titre au début de chaque groupe de skins de la documentation
                let head = if section != prev { section } else { "" };
                prev = section;
                ViewOption {
                    checked: value == current,
                    value,
                    label,
                    section: head,
                }
            }));
            ViewGroup {
                key,
                title,
                is_set: !current.is_empty(),
                open: !current.is_empty(),
                summary,
                options,
            }
        })
        .collect()
}

async fn gallery(
    State(s): State<AppState>,
    MaybeUser(u): MaybeUser,
    Query(q): Query<Vec<(String, String)>>,
) -> AppResult<Html<String>> {
    // Une valeur inconnue dans l'URL est ignorée plutôt que de renvoyer une erreur.
    let filter = Filter::from_pairs(&q, false).unwrap_or_default();
    // « Mes cartes » (menu du compte) : seulement les cartes créées en ligne par ce compte.
    let mine = u.is_some() && q.iter().any(|(k, v)| k == "mine" && v == "1");
    let mut all = db::list(&s.pool, Filter::default()).await?;
    if let (true, Some(user)) = (mine, &u) {
        let owned = s.owned_by(user.id);
        all.retain(|c| owned.contains(&c.slug));
    }
    let cards: Vec<&Card> = all.iter().filter(|c| filter.matches(c)).collect();
    // Affichage (panneau de droite) : réglages appliqués à toutes les cartes de la galerie, sans
    // toucher aux fichiers.
    let view = GalleryView::from_pairs(&q);

    // Un menu par réglage : valeurs cochables et nombre de cartes qu'on obtiendrait en cochant
    // chacune, avec les autres réglages déjà choisis.
    fn options<T: Copy>(
        values: &[T],
        slug: fn(T) -> &'static str,
        label: fn(T) -> &'static str,
        color: fn(T) -> Option<&'static str>,
    ) -> Vec<(&'static str, &'static str, Option<&'static str>)> {
        values
            .iter()
            .map(|&v| (slug(v), label(v), color(v)))
            .collect()
    }
    let menus: [(&'static str, &'static str, &'static str, Vec<_>); 13] = [
        (
            "type",
            "Type",
            "Tous les types",
            options(CardType::ALL, CardType::slug, CardType::label, |t| {
                Some(t.color())
            }),
        ),
        (
            "art",
            "Illustration",
            "Toutes",
            options(
                ArtPresence::ALL,
                ArtPresence::slug,
                ArtPresence::label,
                |_| None,
            ),
        ),
        (
            "foil",
            "Finition",
            "Toutes",
            options(Foil::ALL, Foil::slug, Foil::label, |_| None),
        ),
        (
            "shape",
            "Contour",
            "Tous",
            options(Shape::ALL, Shape::slug, Shape::label, |_| None),
        ),
        (
            "size",
            "Taille",
            "Toutes",
            options(Size::ALL, Size::slug, Size::label, |_| None),
        ),
        (
            "badge",
            "Badge de type",
            "Tous",
            options(Badge::ALL, Badge::slug, Badge::label, |_| None),
        ),
        (
            "title",
            "Nom",
            "Tous",
            options(Title::ALL, Title::slug, Title::label, |_| None),
        ),
        (
            "desc",
            "Description",
            "Toutes",
            options(DescStyle::ALL, DescStyle::slug, DescStyle::label, |_| None),
        ),
        (
            "relief",
            "Relief",
            "Tous",
            options(Relief::ALL, Relief::slug, Relief::label, |_| None),
        ),
        (
            "bg",
            "Background",
            "Tous",
            options(Background::ALL, Background::slug, Background::label, |_| {
                None
            }),
        ),
        (
            "version",
            "Version",
            "Toutes",
            options(
                CardVersion::ALL,
                CardVersion::slug,
                CardVersion::label,
                |_| None,
            ),
        ),
        (
            "effect",
            "Bloc Effect",
            "Tous",
            options(
                EffectStyle::ALL,
                EffectStyle::slug,
                EffectStyle::label,
                |_| None,
            ),
        ),
        (
            "refs",
            "Cartes citées",
            "Toutes",
            options(Refs::ALL, Refs::slug, Refs::label, |_| None),
        ),
    ];
    let mut active = Vec::new();
    let groups = menus
        .into_iter()
        .map(|(key, title, all_label, values)| {
            let chosen = filter.values(key);
            let options: Vec<FilterOption> = values
                .into_iter()
                .map(|(value, label, color)| {
                    let checked = chosen.contains(&value);
                    if checked {
                        let pairs: Vec<(String, String)> = filter
                            .pairs()
                            .into_iter()
                            .filter(|p| *p != (key, value))
                            .map(|(k, v)| (k.to_owned(), v.to_owned()))
                            .collect();
                        active.push(ActiveFilter {
                            title,
                            label,
                            href: gallery_href(
                                &Filter::from_pairs(&pairs, false).unwrap_or_default(),
                                mine,
                                &view.pairs,
                            ),
                        });
                    }
                    FilterOption {
                        value,
                        label,
                        color,
                        checked,
                        count: all
                            .iter()
                            .filter(|c| {
                                db::field(c, key) == value && filter.matches_except(c, Some(key))
                            })
                            .count(),
                    }
                })
                .collect();
            let picked: Vec<&str> = options
                .iter()
                .filter(|o| o.checked)
                .map(|o| o.label)
                .collect();
            FilterGroup {
                key,
                title,
                summary: match picked.len() {
                    0 => all_label.to_owned(),
                    1 => picked[0].to_owned(),
                    n => format!("{n} choisis"),
                },
                is_set: !picked.is_empty(),
                open: key == "type" || !picked.is_empty(),
                options,
            }
        })
        .collect();

    let load_errors = s
        .load_errors
        .read()
        .expect("verrou empoisonné")
        .iter()
        .map(ToString::to_string)
        .collect();

    render(GalleryPage {
        ctx: s.ctx_for(u.as_ref()),
        cards: {
            let idx = RefIndex::new(&all);
            cards.iter().map(|c| view.card(c, &idx)).collect()
        },
        groups,
        active,
        filtered: !filter.is_empty() || mine,
        mine,
        mine_off_href: gallery_href(&filter, false, &view.pairs),
        view_reset_href: gallery_href(&filter, mine, &[]),
        view_set: !view.pairs.is_empty(),
        uses_concepts: view.wrap.is_some(),
        view_groups: view_groups(&s, &view),
        load_errors,
    })
}

/// Autres pages de la documentation : Basics, Core Concepts, Getting Started, Releases, Roadmap.
async fn doc_page(
    State(s): State<AppState>,
    user: MaybeUser,
    Path(seg): Path<String>,
) -> AppResult<Response> {
    let Some(page) = DocPage::from_segment(&seg) else {
        return Ok(not_found_for(user.0.as_ref(), &s));
    };
    rules_page_for(s, user, page).await
}

async fn rules_page(State(s): State<AppState>, user: MaybeUser) -> AppResult<Response> {
    rules_page_for(s, user, DocPage::Overview).await
}

async fn rules_page_for(
    s: AppState,
    MaybeUser(u): MaybeUser,
    page: DocPage,
) -> AppResult<Response> {
    let (rules, error) = {
        let state = s.rules.read().expect("verrou empoisonné");
        (state.rules.clone(), state.error.clone())
    };
    let Some(rules) = rules else {
        let page = RulesPage::missing(s.ctx_for(u.as_ref()), error).on(page);
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

    // Partie « Concepts » : chaque exemple des bandes est une carte différente de la collection,
    // tirée au hasard (cartes remplies d'abord, puis les autres ; on recommence la pioche quand
    // la collection est plus petite que le nombre d'exemples).
    let all = db::list(&s.pool, Filter::default()).await?;
    let filled =
        |c: &Card| !c.draft.description.trim().is_empty() && !c.draft.effect.trim().is_empty();
    let deck: Vec<&Card> = {
        fn shuffle<'a>(mut v: Vec<&'a Card>, n: &mut u64) -> Vec<&'a Card> {
            for k in (1..v.len()).rev() {
                *n ^= *n << 13;
                *n ^= *n >> 7;
                *n ^= *n << 17;
                v.swap(k, (*n % (k as u64 + 1)) as usize);
            }
            v
        }
        let mut seed = [0u8; 8];
        let _ = getrandom::fill(&mut seed);
        let mut n = u64::from_le_bytes(seed) | 1;
        let mut deck = shuffle(all.iter().filter(|c| filled(c)).collect(), &mut n);
        deck.extend(shuffle(all.iter().filter(|c| !filled(c)).collect(), &mut n));
        deck
    };
    let next = std::cell::Cell::new(0usize);
    let draw = || {
        (!deck.is_empty()).then(|| {
            let i = next.get();
            next.set(i + 1);
            deck[i % deck.len()]
        })
    };
    let plain = |label: &str, card: &Card, draft: &CardDraft| FoilSample {
        label: label.to_owned(),
        card: CardView::new(Some(&card.slug), draft),
        wrap: String::new(),
        desc: String::new(),
    };
    let skins = |items: &[crate::rules::SkinItem]| -> Vec<FoilSample> {
        items
            .iter()
            .filter_map(|item| {
                let (skin, wrap, fx) = concepts::skin_style(&item.key)?;
                let card = draw()?;
                let mut draft = card.draft.clone();
                draft.skin = skin;
                let mut view = CardView::new(Some(&card.slug), &draft);
                if let Some(k) = fx {
                    view.fx = concepts::fx(k, &card.slug);
                    view.data_text = matches!(k, "ascii" | "glitch");
                    if k == "ascii" {
                        view.ascii_word = Some(concepts::ascii_word(draft.card_type));
                    }
                }
                Some(FoilSample {
                    label: item.title.clone(),
                    card: view,
                    wrap,
                    desc: item.text.clone(),
                })
            })
            .collect()
    };
    let skins_classic = skins(&rules.concepts.skin.classic);
    let skins_animated = skins(&rules.concepts.skin.animated);
    let foils: Vec<FoilSample> = Foil::ALL
        .iter()
        .filter_map(|&f| {
            let card = draw()?;
            let mut draft = card.draft.clone();
            draft.foil = f;
            Some(plain(f.label(), card, &draft))
        })
        .collect();
    let shapes: Vec<FoilSample> = Shape::ALL
        .iter()
        .filter_map(|&sh| {
            let card = draw()?;
            let mut draft = card.draft.clone();
            draft.shape = sh;
            let mut sample = plain(sh.label(), card, &draft);
            sample.desc = rules
                .concepts
                .shape
                .items
                .iter()
                .find(|i| i.key == sh.slug())
                .map(|i| i.text.clone())
                .unwrap_or_default();
            Some(sample)
        })
        .collect();

    // Anatomie : une carte remplie et illustrée de préférence, tirée au hasard.
    let anatomy = {
        let mut pool: Vec<&Card> = all
            .iter()
            .filter(|c| c.draft.art.is_some() && filled(c))
            .collect();
        if pool.is_empty() {
            pool = deck.clone();
        }
        let mut seed = [0u8; 8];
        let _ = getrandom::fill(&mut seed);
        (!pool.is_empty())
            .then(|| pool[(u64::from_le_bytes(seed) % pool.len() as u64) as usize].clone())
    };
    let samples = ConceptSamples {
        anatomy,
        skins_classic,
        skins_animated,
        foils,
        shapes,
        templates: all
            .iter()
            .map(|c| TemplateCard {
                card: CardView::new(Some(&c.slug), &c.draft),
                filled: filled(c),
            })
            .collect(),
    };

    Ok(
        render(RulesPage::new(s.ctx_for(u.as_ref()), &rules, error, types, samples).on(page))?
            .into_response(),
    )
}

/// Document « Carte DCB v2 » (pistes de design) : texte de `rules/concepts.toml`, cartes de
/// la collection tirées au hasard à chaque affichage, comme la page des règles.
async fn concepts_page(State(s): State<AppState>, MaybeUser(u): MaybeUser) -> AppResult<Response> {
    let (doc, error) = {
        let state = s.rules.read().expect("verrou empoisonné");
        (state.concepts.clone(), state.concepts_error.clone())
    };
    let Some(doc) = doc else {
        let page = ConceptsPage {
            ctx: s.ctx_for(u.as_ref()),
            doc: None,
            parts: Vec::new(),
            error,
        };
        return Ok((StatusCode::SERVICE_UNAVAILABLE, render(page)?).into_response());
    };

    let mut trio = Vec::new();
    for t in [CardType::App, CardType::Incident, CardType::Hr] {
        trio.extend(db::random_card(&s.pool, Some(t)).await?);
    }
    let mut types = Vec::new();
    for &t in CardType::ALL {
        types.extend(db::random_card(&s.pool, Some(t)).await?);
    }
    let all = db::list(&s.pool, Filter::default()).await?;
    let refs = {
        let mut citing: Vec<Card> = concepts::citing_cards(&all).into_iter().cloned().collect();
        // trois cartes au hasard parmi celles qui en citent une autre
        let mut seed = [0u8; 8];
        let _ = getrandom::fill(&mut seed);
        let mut n = u64::from_le_bytes(seed) | 1;
        let mut picked = Vec::new();
        while !citing.is_empty() && picked.len() < 3 {
            n ^= n << 13;
            n ^= n >> 7;
            n ^= n << 17;
            picked.push(citing.swap_remove((n % citing.len() as u64) as usize));
        }
        if picked.is_empty() {
            trio.clone()
        } else {
            picked
        }
    };
    let picks = concepts::Picks {
        trio,
        types,
        refs,
        all,
    };
    let parts = concepts::build(&doc, &picks);
    let page = ConceptsPage {
        ctx: s.ctx_for(u.as_ref()),
        doc: Some(doc),
        parts,
        error,
    };
    Ok(render(page)?.into_response())
}

async fn reload(State(s): State<AppState>, EditorUser(u): EditorUser) -> AppResult<Response> {
    if !u.is_admin() {
        return Ok(forbidden_response(Some(&u), &s));
    }
    s.reload().await?;
    Ok(Redirect::to("/gallery").into_response())
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

async fn detail(
    State(s): State<AppState>,
    MaybeUser(u): MaybeUser,
    Path(slug): Path<String>,
) -> AppResult<Html<String>> {
    let card = load(&s.pool, &slug).await?;
    let idx = RefIndex::new(&db::list(&s.pool, Filter::default()).await?);
    let demo_card = s.sandbox
        && s.owners
            .read()
            .expect("verrou empoisonné")
            .contains_key(&card.slug);
    render(DetailPage {
        ctx: s.ctx_for(u.as_ref()),
        can_modify: u.as_ref().is_some_and(|u| s.may_modify(u, &card.slug)),
        demo_card,
        file: card_file(&s, &card.slug),
        art_file: card
            .draft
            .art
            .as_ref()
            .map(|a| s.store.art_dir().join(a).display().to_string()),
        card: CardView::new(Some(&card.slug), &card.draft).cite(&idx, true),
    })
}

async fn new_form(State(s): State<AppState>, EditorUser(u): EditorUser) -> AppResult<Html<String>> {
    let values = FormValues::blank();
    let preview = parse_values(&values).unwrap_or_else(|(d, _)| d);
    render(FormPage::new(
        s.ctx(&u),
        "Nouvelle carte",
        "/cards".into(),
        "/gallery".into(),
        values,
        Vec::new(),
        CardView::new(None, &preview),
    ))
}

async fn edit_form(
    State(s): State<AppState>,
    EditorUser(u): EditorUser,
    Path(slug): Path<String>,
) -> AppResult<Response> {
    let card = load(&s.pool, &slug).await?;
    if !s.may_modify(&u, &slug) {
        return Ok(forbidden_response(Some(&u), &s));
    }
    Ok(render(FormPage::new(
        s.ctx(&u),
        "Modifier la carte",
        format!("/cards/{slug}"),
        format!("/cards/{slug}"),
        FormValues::from_draft(&card.draft),
        Vec::new(),
        CardView::new(Some(&slug), &card.draft).cite(
            &RefIndex::new(&db::list(&s.pool, Filter::default()).await?),
            false,
        ),
    ))?
    .into_response())
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
        shape: if v.shape.is_empty() {
            Shape::Solid
        } else {
            pick(&v.shape, Shape::Solid, "shape", &mut errors)
        },
        size: if v.size.is_empty() {
            Size::Normal
        } else {
            pick(&v.size, Size::Normal, "size", &mut errors)
        },
        badge: if v.badge.is_empty() {
            Badge::Text
        } else {
            pick(&v.badge, Badge::Text, "badge", &mut errors)
        },
        title: if v.title.is_empty() {
            Title::Block
        } else {
            pick(&v.title, Title::Block, "title", &mut errors)
        },
        desc_style: if v.desc_style.is_empty() {
            DescStyle::Classic
        } else {
            pick(&v.desc_style, DescStyle::Classic, "desc_style", &mut errors)
        },
        relief: if v.relief.is_empty() {
            Relief::Flat
        } else {
            pick(&v.relief, Relief::Flat, "relief", &mut errors)
        },
        background: if v.background.is_empty() {
            Background::Plain
        } else {
            pick(&v.background, Background::Plain, "background", &mut errors)
        },
        version: if v.version.is_empty() {
            CardVersion::V1
        } else {
            pick(&v.version, CardVersion::V1, "version", &mut errors)
        },
        effect_style: if v.effect_style.is_empty() {
            EffectStyle::Text
        } else {
            pick(
                &v.effect_style,
                EffectStyle::Text,
                "effect_style",
                &mut errors,
            )
        },
        effect_file: v.effect_file.clone(),
        refs: if v.refs.is_empty() {
            Refs::Plain
        } else {
            pick(&v.refs, Refs::Plain, "refs", &mut errors)
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
            "shape" => v.shape = text,
            "size" => v.size = text,
            "badge" => v.badge = text,
            "title" => v.title = text,
            "desc_style" => v.desc_style = text,
            "relief" => v.relief = text,
            "background" => v.background = text,
            "version" => v.version = text,
            "effect_style" => v.effect_style = text,
            "effect_file" => v.effect_file = text,
            "refs" => v.refs = text,
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
    ctx: Ctx,
    title: &'static str,
    action: String,
    cancel_href: String,
    values: FormValues,
    draft: &CardDraft,
    errors: Vec<FieldError>,
) -> AppResult<Response> {
    let page = FormPage::new(
        ctx,
        title,
        action,
        cancel_href,
        values,
        errors,
        CardView::new(None, draft),
    );
    Ok((StatusCode::UNPROCESSABLE_ENTITY, render(page)?).into_response())
}

async fn create(
    State(s): State<AppState>,
    EditorUser(u): EditorUser,
    mp: Multipart,
) -> AppResult<Response> {
    let sub = read_submission(mp).await?;
    let mut draft = match sub.parse() {
        Ok(d) => d,
        Err((draft, errors)) => {
            return form_with_errors(
                s.ctx(&u),
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
    if s.sandbox {
        s.owners
            .write()
            .expect("verrou empoisonné")
            .insert(card.slug.clone(), u.id);
    }
    Ok(Redirect::to(&format!("/cards/{}", card.slug)).into_response())
}

async fn update(
    State(s): State<AppState>,
    EditorUser(u): EditorUser,
    Path(slug): Path<String>,
    mp: Multipart,
) -> AppResult<Response> {
    let existing = load(&s.pool, &slug).await?;
    if !s.may_modify(&u, &slug) {
        return Ok(forbidden_response(Some(&u), &s));
    }
    let mut sub = read_submission(mp).await?;
    sub.values.current_art = existing.draft.art.clone();

    let mut draft = match sub.parse() {
        Ok(d) => d,
        Err((draft, errors)) => {
            return form_with_errors(
                s.ctx(&u),
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

async fn delete(
    State(s): State<AppState>,
    EditorUser(u): EditorUser,
    Path(slug): Path<String>,
) -> AppResult<Response> {
    let card = load(&s.pool, &slug).await?;
    if !s.may_modify(&u, &slug) {
        return Ok(forbidden_response(Some(&u), &s));
    }
    let _guard = s.store.lock().await;
    s.store.remove(&slug).await?;
    db::delete(&s.pool, &slug).await?;
    if let Some(art) = &card.draft.art {
        s.store.remove_art(art).await;
    }
    s.owners.write().expect("verrou empoisonné").remove(&slug);
    Ok(Redirect::to("/gallery").into_response())
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
    shape: String,
    #[serde(default)]
    size: String,
    #[serde(default)]
    badge: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    desc_style: String,
    #[serde(default)]
    relief: String,
    #[serde(default)]
    background: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    effect_style: String,
    #[serde(default)]
    effect_file: String,
    #[serde(default)]
    refs: String,
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
async fn preview(
    State(s): State<AppState>,
    _editor: EditorUser,
    Form(f): Form<PreviewForm>,
) -> AppResult<Html<String>> {
    let current_art =
        Some(f.current_art).filter(|a| f.remove_art.is_empty() && is_safe_file_name(a));
    let values = FormValues {
        name: f.name,
        card_type: f.card_type,
        skin: f.skin,
        foil: f.foil,
        shape: f.shape,
        size: f.size,
        badge: f.badge,
        title: f.title,
        desc_style: f.desc_style,
        relief: f.relief,
        background: f.background,
        version: f.version,
        effect_style: f.effect_style,
        effect_file: f.effect_file,
        refs: f.refs,
        description: f.description,
        effect: f.effect,
        deploy_cost: f.deploy_cost,
        daily_cost: f.daily_cost,
        flavor_text: f.flavor_text,
        current_art,
    };
    let draft = parse_values(&values).unwrap_or_else(|(d, _)| d);
    let idx = RefIndex::new(&db::list(&s.pool, Filter::default()).await?);
    render(CardPreview {
        card: CardView::new(None, &draft).cite(&idx, false),
    })
}

// ---------------------------------------------------------------------------------------------
// API JSON (pour le futur moteur de duel et les clients)

async fn api_list(
    State(s): State<AppState>,
    Query(q): Query<Vec<(String, String)>>,
) -> AppResult<Json<Vec<Card>>> {
    // Clés répétables : ?type=resource&type=incident&foil=gold. Valeur inconnue : 400.
    let filter = Filter::from_pairs(&q, true).map_err(|e| AppError::BadRequest(e.to_string()))?;
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
        "shapes": Shape::ALL.iter().map(|s| serde_json::json!({
            "slug": s.slug(), "label": s.label()
        })).collect::<Vec<_>>(),
        "sizes": Size::ALL.iter().map(|s| serde_json::json!({
            "slug": s.slug(), "label": s.label()
        })).collect::<Vec<_>>(),
        "badges": Badge::ALL.iter().map(|b| serde_json::json!({
            "slug": b.slug(), "label": b.label()
        })).collect::<Vec<_>>(),
        "titles": Title::ALL.iter().map(|t| serde_json::json!({
            "slug": t.slug(), "label": t.label()
        })).collect::<Vec<_>>(),
        "desc_styles": DescStyle::ALL.iter().map(|t| serde_json::json!({
            "slug": t.slug(), "label": t.label()
        })).collect::<Vec<_>>(),
        "effect_styles": EffectStyle::ALL.iter().map(|t| serde_json::json!({
            "slug": t.slug(), "label": t.label()
        })).collect::<Vec<_>>(),
        "versions": CardVersion::ALL.iter().map(|t| serde_json::json!({
            "slug": t.slug(), "label": t.label()
        })).collect::<Vec<_>>(),
        "backgrounds": Background::ALL.iter().map(|t| serde_json::json!({
            "slug": t.slug(), "label": t.label()
        })).collect::<Vec<_>>(),
        "reliefs": Relief::ALL.iter().map(|t| serde_json::json!({
            "slug": t.slug(), "label": t.label()
        })).collect::<Vec<_>>(),
        "refs": Refs::ALL.iter().map(|t| serde_json::json!({
            "slug": t.slug(), "label": t.label()
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
