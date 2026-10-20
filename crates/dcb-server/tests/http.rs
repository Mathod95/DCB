//! Tests de bout en bout sur le routeur, avec un dossier `cards/` et une base temporaires.

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{HeaderValue, Request, StatusCode, header};
use dcb_server::auth::{self, Role};
use dcb_server::{AppState, CardStore, db, router};
use http_body_util::BodyExt;
use tower::ServiceExt;

struct TestApp {
    /// Routeur où chaque requête porte le cookie de session d'un admin.
    app: axum::Router,
    /// Même app, connecté en utilisateur simple.
    user: axum::Router,
    /// Même app, sans session.
    anon: axum::Router,
    state: AppState,
    dir: PathBuf,
    admin_token: String,
    user_token: String,
}

impl TestApp {
    fn cards(&self) -> PathBuf {
        self.dir.join("cards")
    }

    /// Reconstruit les routeurs après un changement d'état (règles, éditeur…).
    fn rebuild(&mut self) {
        self.app = with_session(router(self.state.clone()), &self.admin_token);
        self.user = with_session(router(self.state.clone()), &self.user_token);
        self.anon = router(self.state.clone());
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

const ADMIN_PASSWORD: &str = "admin-password-123";
const USER_PASSWORD: &str = "joueur-password-123";

fn with_session(r: axum::Router, token: &str) -> axum::Router {
    let cookie = HeaderValue::from_str(&format!("{}={token}", auth::COOKIE_NAME)).unwrap();
    r.layer(axum::middleware::map_request(
        move |mut req: axum::extract::Request| {
            let cookie = cookie.clone();
            async move {
                if !req.headers().contains_key(header::COOKIE) {
                    req.headers_mut().insert(header::COOKIE, cookie);
                }
                req
            }
        },
    ))
}

/// Démarre l'app sur un dossier `cards/` contenant les fichiers donnés, avec un compte admin
/// (« admin ») et un compte utilisateur (« joueur ») déjà connectés.
async fn app(files: &[(&str, &str)]) -> TestApp {
    let dir = std::env::temp_dir().join(format!("dcb-test-{}", uuid::Uuid::new_v4()));
    let cards = dir.join("cards");
    std::fs::create_dir_all(&cards).unwrap();
    for (name, body) in files {
        std::fs::write(cards.join(name), body).unwrap();
    }
    let pool = db::connect(&format!("sqlite://{}", dir.join("cache.db").display()))
        .await
        .unwrap();
    let app_db = auth::connect(&format!("sqlite://{}", dir.join("app.db").display()))
        .await
        .unwrap();
    let admin = auth::create_user(&app_db, "admin", ADMIN_PASSWORD, Role::Admin)
        .await
        .unwrap()
        .unwrap();
    let user = auth::create_user(&app_db, "joueur", USER_PASSWORD, Role::User)
        .await
        .unwrap()
        .unwrap();
    let admin_token = auth::create_session(&app_db, admin).await.unwrap();
    let user_token = auth::create_session(&app_db, user).await.unwrap();
    let state = AppState::new(pool, app_db, CardStore::new(&cards));
    state.reload().await.unwrap();
    let mut t = TestApp {
        app: router(state.clone()),
        user: router(state.clone()),
        anon: router(state.clone()),
        state,
        dir,
        admin_token,
        user_token,
    };
    t.rebuild();
    t
}

fn form(uri: &str, body: &str) -> Request<Body> {
    Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, Option<String>, String) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let location = res
        .headers()
        .get(header::LOCATION)
        .map(|v| v.to_str().unwrap().to_owned());
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        location,
        String::from_utf8_lossy(&body).into_owned(),
    )
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

fn post(uri: &str) -> Request<Body> {
    Request::post(uri).body(Body::empty()).unwrap()
}

const BOUNDARY: &str = "dcbboundary";

/// Construit un corps multipart. Un champ `art` est envoyé comme fichier.
fn multipart(uri: &str, fields: &[(&str, &[u8])]) -> Request<Body> {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        if *name == "art" {
            body.extend_from_slice(
                b"Content-Disposition: form-data; name=\"art\"; filename=\"a.png\"\r\nContent-Type: image/png\r\n\r\n",
            );
        } else {
            body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
            );
        }
        body.extend_from_slice(value);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    Request::post(uri)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(body))
        .unwrap()
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const JPG: &[u8] = b"\xFF\xD8\xFF\xE0\0\x10JFIF";

fn card_fields<'a>(name: &'a str, extra: &[(&'a str, &'a [u8])]) -> Vec<(&'a str, &'a [u8])> {
    let mut f: Vec<(&str, &[u8])> = vec![
        ("name", name.as_bytes()),
        ("card_type", b"incident"),
        ("skin", b"dark"),
        ("foil", b"shards"),
        ("description", "Le cluster ne répond plus".as_bytes()),
        ("effect", b"- Inflige 3 degats\r\n- Duree 2 tours"),
        ("deploy_cost", b"4"),
        ("daily_cost", b"1"),
        ("flavor_text", b"Ca marchait sur ma machine."),
    ];
    f.extend_from_slice(extra);
    f
}

const DDOS: &str = "name = \"DDOS\"\ntype = \"incident\"\ndeploy_cost = 4\n";
const ARGO: &str = "name = \"ARGO CD\"\ntype = \"app\"\nfoil = \"gold\"\n";

#[tokio::test]
async fn starts_empty_without_demo_cards() {
    let t = app(&[]).await;
    let (status, _, json) = send(&t.app, get("/api/cards")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, "[]");
    let (_, _, body) = send(&t.app, get("/gallery")).await;
    assert!(body.contains("Aucune carte pour l'instant"));
    let (_, _, body) = send(&t.app, get("/")).await;
    assert!(body.contains("Aucune carte pour l'instant"));
}

#[tokio::test]
async fn pages_render_from_toml_files() {
    let t = app(&[("ddos.toml", DDOS), ("argo-cd.toml", ARGO)]).await;
    for uri in [
        "/",
        "/gallery",
        "/gallery?type=app",
        "/cards/new",
        "/cards/ddos",
        "/cards/argo-cd/edit",
    ] {
        let (status, _, body) = send(&t.app, get(uri)).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert!(body.contains("DevCard"), "{uri}");
    }
    let (_, _, body) = send(&t.app, get("/gallery?type=incident")).await;
    assert!(body.contains("DDOS") && !body.contains("ARGO CD"));
    let (_, _, body) = send(&t.app, get("/cards/ddos")).await;
    assert!(
        body.contains("ddos.toml"),
        "le fichier à commiter est affiché"
    );

    for uri in ["/cards/nope", "/cards/..%2Fetc", "/cards/DDOS"] {
        let (status, _, _) = send(&t.app, get(uri)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
}

#[tokio::test]
async fn invalid_files_are_listed_in_gallery() {
    let t = app(&[
        ("ddos.toml", DDOS),
        ("broken.toml", "name = \"X\"\ntype = \"package\"\n"),
    ])
    .await;
    let (_, _, body) = send(&t.app, get("/gallery")).await;
    assert!(body.contains("DDOS"));
    assert!(body.contains("broken.toml"), "{body}");
}

#[tokio::test]
async fn create_writes_toml_and_art() {
    let t = app(&[]).await;
    let (status, location, _) = send(
        &t.app,
        multipart("/cards", &card_fields("Crash Loop", &[("art", PNG)])),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location.as_deref(), Some("/cards/crash-loop"));

    let toml = std::fs::read_to_string(t.cards().join("crash-loop.toml")).unwrap();
    assert!(toml.contains("name = \"Crash Loop\""), "{toml}");
    assert!(toml.contains("type = \"incident\""), "{toml}");
    assert!(toml.contains("art = \"crash-loop.png\""), "{toml}");
    assert!(toml.contains("foil = \"shards\""), "{toml}");
    assert!(!toml.contains('\r'), "retours chariot normalisés : {toml}");
    assert_eq!(
        std::fs::read(t.cards().join("art/crash-loop.png")).unwrap(),
        PNG
    );

    let (status, _, _) = send(&t.app, get("/art/crash-loop.png")).await;
    assert_eq!(status, StatusCode::OK);

    // Même nom : un deuxième fichier, le premier n'est pas écrasé
    let (_, location, _) = send(&t.app, multipart("/cards", &card_fields("CRASH LOOP", &[]))).await;
    assert_eq!(location.as_deref(), Some("/cards/crash-loop-2"));
    assert!(t.cards().join("crash-loop.toml").exists());

    // Le cache reconstruit depuis les fichiers donne le même résultat
    let (_, _, before) = send(&t.app, get("/api/cards")).await;
    t.state.reload().await.unwrap();
    let (_, _, after) = send(&t.app, get("/api/cards")).await;
    assert_eq!(before, after);
    assert!(after.contains("\"effect\":\"- Inflige 3 degats\\n- Duree 2 tours\""));
}

#[tokio::test]
async fn update_keeps_slug_and_manages_art() {
    let t = app(&[]).await;
    send(
        &t.app,
        multipart("/cards", &card_fields("Crash Loop", &[("art", PNG)])),
    )
    .await;

    // Renommer ne change pas le fichier ; current_art envoyé par le client est ignoré
    let (status, location, _) = send(
        &t.app,
        multipart(
            "/cards/crash-loop",
            &card_fields("CrashLoopBackOff", &[("current_art", b"../../x.png")]),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location.as_deref(), Some("/cards/crash-loop"));
    let toml = std::fs::read_to_string(t.cards().join("crash-loop.toml")).unwrap();
    assert!(toml.contains("name = \"CrashLoopBackOff\""));
    assert!(toml.contains("art = \"crash-loop.png\""));

    // Nouvelle illustration dans un autre format : l'ancienne disparaît
    send(
        &t.app,
        multipart(
            "/cards/crash-loop",
            &card_fields("CrashLoopBackOff", &[("art", JPG)]),
        ),
    )
    .await;
    assert!(t.cards().join("art/crash-loop.jpg").exists());
    assert!(!t.cards().join("art/crash-loop.png").exists());

    // Retrait de l'illustration
    send(
        &t.app,
        multipart(
            "/cards/crash-loop",
            &card_fields("CrashLoopBackOff", &[("remove_art", b"1")]),
        ),
    )
    .await;
    assert!(!t.cards().join("art/crash-loop.jpg").exists());
    let toml = std::fs::read_to_string(t.cards().join("crash-loop.toml")).unwrap();
    assert!(!toml.contains("art"), "{toml}");
}

#[tokio::test]
async fn delete_removes_files() {
    let t = app(&[]).await;
    send(
        &t.app,
        multipart("/cards", &card_fields("Crash Loop", &[("art", PNG)])),
    )
    .await;
    let (status, location, _) = send(&t.app, post("/cards/crash-loop/delete")).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location.as_deref(), Some("/gallery"));
    assert!(!t.cards().join("crash-loop.toml").exists());
    assert!(!t.cards().join("art/crash-loop.png").exists());
    let (status, _, _) = send(&t.app, get("/cards/crash-loop")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = send(&t.app, post("/cards/crash-loop/delete")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn reload_picks_up_external_changes() {
    let t = app(&[("ddos.toml", DDOS)]).await;
    // Comme après un `git pull` : un fichier ajouté, un supprimé
    std::fs::write(t.cards().join("argo-cd.toml"), ARGO).unwrap();
    std::fs::remove_file(t.cards().join("ddos.toml")).unwrap();
    let (status, _, _) = send(&t.app, post("/reload")).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let (_, _, json) = send(&t.app, get("/api/cards")).await;
    assert!(json.contains("argo-cd") && !json.contains("ddos"), "{json}");
}

#[tokio::test]
async fn invalid_submission_writes_nothing() {
    let t = app(&[]).await;
    let fields: Vec<(&str, &[u8])> = vec![
        ("name", b""),
        ("card_type", b"incident"),
        ("skin", b"light"),
        ("deploy_cost", b"150"),
        ("daily_cost", b"abc"),
        ("description", b"garde ma saisie"),
        ("art", b"<svg></svg>"),
    ];
    let (status, _, body) = send(&t.app, multipart("/cards", &fields)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body.contains("le nom est obligatoire"));
    assert!(body.contains("entre 0 et 99"));
    assert!(body.contains("nombre attendu"));
    assert!(body.contains("PNG, JPEG, GIF ou WebP attendu"));
    assert!(
        body.contains("garde ma saisie"),
        "la saisie doit être conservée"
    );
    assert_eq!(std::fs::read_dir(t.cards()).unwrap().count(), 0);
}

#[tokio::test]
async fn preview_renders_card_and_drops_unsafe_art() {
    let t = app(&[]).await;
    let req = Request::post("/preview")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(
            "name=%3Cscript%3E&card_type=hr&skin=dark&foil=gold&deploy_cost=7&current_art=..%2F..%2Fetc%2Fpasswd",
        ))
        .unwrap();
    let (status, _, body) = send(&t.app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(r#"data-type="hr""#));
    assert!(body.contains(r#"data-foil="gold""#));
    assert!(!body.contains("<script>"), "le nom doit être échappé");
    assert!(!body.contains("passwd"));
    assert!(
        body.contains("is-placeholder"),
        "emplacement du texte d'ambiance visible"
    );
}

#[tokio::test]
async fn api_rejects_unknown_filter() {
    let t = app(&[("ddos.toml", DDOS)]).await;
    let (status, _, _) = send(&t.app, get("/api/cards?type=package")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, body) = send(&t.app, get("/api/cards/ddos")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"slug\":\"ddos\""));
    let (_, _, body) = send(&t.app, get("/api/meta")).await;
    assert!(body.contains("#a538f8"));
    assert!(body.contains("\"glitter\""));
    for tex in ["stars", "glitter", "shards"] {
        let (status, _, _) = send(&t.app, get(&format!("/static/foil/{tex}.svg"))).await;
        assert_eq!(status, StatusCode::OK, "{tex}");
    }
    let (status, _, _) = send(&t.app, get("/static/foil/..%2Fapp.css")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

const RULES: &str = include_str!("../../../rules/regles.toml");

async fn app_with_rules(files: &[(&str, &str)], rules: Option<&str>) -> TestApp {
    let mut t = app(files).await;
    let path = t.dir.join("regles.toml");
    if let Some(r) = rules {
        std::fs::write(&path, r).unwrap();
    }
    t.state = t.state.clone().with_rules_file(&path);
    t.state.reload().await.unwrap();
    t.rebuild();
    t
}

#[tokio::test]
async fn rules_page_shows_rules_counts_and_random_examples() {
    let t = app_with_rules(
        &[
            (
                "ddos.toml",
                "name = \"DDOS\"\ntype = \"incident\"\ndescription = \"Trafic massif\"\n",
            ),
            (
                "empty-incident.toml",
                "name = \"VIDE\"\ntype = \"incident\"\n",
            ),
            ("argo-cd.toml", ARGO),
        ],
        Some(RULES),
    )
    .await;
    let (status, _, body) = send(&t.app, get("/regles")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Atteignez le million"));
    assert!(
        body.contains("2 cartes dans la collection"),
        "compteur des incidents"
    );
    assert!(
        body.contains("1 carte dans la collection"),
        "compteur des apps"
    );
    assert!(
        body.contains("Aucune carte pour l&#39;instant")
            || body.contains("Aucune carte pour l'instant")
    );
    assert!(body.contains(r#"href="/gallery?type=incident""#));
    // la carte remplie passe avant la carte vide (la partie Concepts garde toute la collection
    // en modèles pour la vue zoomée)
    let types_part = body.split(r#"id="t-concepts""#).next().unwrap();
    assert!(
        types_part.contains("DDOS") && !types_part.contains(">VIDE<"),
        "exemple d'incident"
    );
    // mise en forme : montants et termes de statut
    assert!(body.contains(r#"<span class="money">$25.000,00</span>"#));
    assert!(body.contains("<strong><em>to-do</em></strong>"));
    // partie Concepts : 10 skins classiques, 8 animées, 6 finitions et 9 contours, chacune en
    // double dans sa bande qui défile
    assert!(body.contains("Anatomie d") && body.contains(r#"id="t-concepts""#));
    assert_eq!(
        body.matches(r#"<div class="foil-item""#).count(),
        (10 + 8 + 6 + 9) * 2
    );
    assert_eq!(
        body.matches(r#"<div class="foil-item" aria-hidden="true""#)
            .count(),
        10 + 8 + 6 + 9
    );
    assert_eq!(body.matches(r#"<div class="skin-rail""#).count(), 4);
    for count in [10, 8, 6, 9] {
        assert!(
            body.contains(&format!(r#"<span class="rail-count">{count}</span>"#)),
            "{count}"
        );
    }
    assert!(
        body.contains("Les skins")
            && body.contains("Les finitions")
            && body.contains("Les contours")
    );
    // chaque exemple des bandes est une carte différente de la collection (ici 3 cartes pour
    // 33 exemples : les trois apparaissent)
    let rails = body
        .split(r#"id="t-concepts""#)
        .nth(1)
        .unwrap()
        .split("<dialog")
        .next()
        .unwrap();
    for name in ["DDOS", "VIDE", "ARGO CD"] {
        assert!(rails.contains(&format!("<span>{name}</span>")), "{name}");
    }
    assert!(!body.contains("Deux skins"));
    // skins de concepts.css, avec calque animé, ASCII et description pour la vue zoomée
    assert!(
        body.contains(r#"class="st sk sk-dracula""#) && body.contains(r#"class="st sk sk-crt""#)
    );
    assert!(body.contains(r#"class="st sk dyn dyn-matrix""#) && body.contains("fx-matrix"));
    assert!(body.contains(r#"class="ascii-art""#) && body.contains(r#" data-text=""#));
    assert!(body.contains("Plan d&#39;architecte") || body.contains("Plan d'architecte"));
    // description des contours dans la vue zoomée
    assert!(body.contains("Trait fin de 2 px avec un halo lumineux"));
    // vue zoomée : choix de la catégorie et une carte modèle par carte de la collection
    assert!(body.contains(r#"<dialog id="card-modal""#));
    assert_eq!(body.matches(r#"class="modal-type""#).count(), 5);
    assert_eq!(body.matches(r#"<template class="tpl-card""#).count(), 3);
    assert!(body.contains(r#"data-type="incident" data-filled="0""#));
    assert!(body.contains(r#"src="/static/concepts.js""#));
    for foil in ["basic", "gold", "stars", "glitter", "shards", "rainbow"] {
        assert!(body.contains(&format!(r#"data-foil="{foil}""#)), "{foil}");
    }
    for shape in [
        "solid", "neon", "gradient", "sweep", "double", "corners", "chamfer", "topbar", "tinted",
    ] {
        assert!(
            body.contains(&format!(r#"data-shape="{shape}""#)),
            "{shape}"
        );
    }
    assert!(body.contains(r#"data-skin="light""#) && body.contains(r#"data-skin="dark""#));
}

#[tokio::test]
async fn rules_page_handles_empty_collection() {
    let t = app_with_rules(&[], Some(RULES)).await;
    let (status, _, body) = send(&t.app, get("/regles")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.matches("Aucune carte de ce type pour l").count(), 5);
    assert!(!body.contains("foil-item"));
    assert!(body.contains("Les exemples appara"));
}

#[tokio::test]
async fn broken_rules_file_keeps_last_valid_version() {
    let t = app_with_rules(&[], Some(RULES)).await;
    std::fs::write(t.dir.join("regles.toml"), "title = \"oups\"\n").unwrap();
    t.state.reload().await.unwrap();
    let (status, _, body) = send(&t.app, get("/regles")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("Atteignez le million"),
        "ancienne version conservée"
    );
    assert!(body.contains("n&#39;a pas pu être lu") || body.contains("n'a pas pu être lu"));

    let missing = app_with_rules(&[], None).await;
    let (status, _, body) = send(&missing.app, get("/regles")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("pas encore disponibles"));
}

// ---------------------------------------------------------------------------------------------
// Authentification et rôles

fn set_cookie(res_headers: &axum::http::HeaderMap) -> Option<String> {
    res_headers
        .get(header::SET_COOKIE)
        .map(|v| v.to_str().unwrap().to_owned())
}

async fn send_full(
    app: &axum::Router,
    req: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn anonymous_visitors_are_sent_to_login() {
    let t = app(&[("ddos.toml", DDOS)]).await;
    for uri in [
        "/",
        "/gallery",
        "/regles",
        "/cards/ddos",
        "/api/cards",
        "/admin/users",
        "/art/x.png",
    ] {
        let (status, location, _) = send(&t.anon, get(uri)).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "{uri}");
        assert!(
            location.as_deref().unwrap().starts_with("/login?next="),
            "{uri}"
        );
    }
    let (_, location, _) = send(&t.anon, get("/gallery?type=app")).await;
    assert_eq!(
        location.as_deref(),
        Some("/login?next=/gallery%3Ftype%3Dapp")
    );
    // la page de connexion et ses styles restent accessibles
    let (status, _, body) = send(&t.anon, get("/login")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Connexion") && !body.contains("topnav"));
    let (status, _, _) = send(&t.anon, get("/static/app.css")).await;
    assert_eq!(status, StatusCode::OK);
    // un cookie invalide ne passe pas
    let req = Request::get("/")
        .header(header::COOKIE, "dcb_session=abc")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&t.anon, req).await.0, StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn login_sets_a_safe_session_cookie() {
    let t = app(&[]).await;
    let (status, _, body) = send(
        &t.anon,
        form(
            "/login",
            "username=joueur&password=mauvais-mdp&next=/regles",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.contains("Identifiant ou mot de passe incorrect"));
    assert!(body.contains(r#"value="joueur""#), "identifiant conservé");

    let (status, headers, _) = send_full(
        &t.anon,
        form(
            "/login",
            &format!("username=Joueur&password={USER_PASSWORD}&next=/regles"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers.get(header::LOCATION).unwrap(), "/regles");
    let cookie = set_cookie(&headers).unwrap();
    assert!(
        cookie.contains("HttpOnly") && cookie.contains("SameSite=Lax"),
        "{cookie}"
    );
    assert!(!cookie.contains("Secure"), "pas de Secure en HTTP local");
    let token = cookie.split(';').next().unwrap().to_owned();

    let req = Request::get("/")
        .header(header::COOKIE, &token)
        .body(Body::empty())
        .unwrap();
    let (status, _, body) = send(&t.anon, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("joueur"));

    // pas de redirection vers un autre site après connexion
    let (_, location, _) = send(
        &t.anon,
        form(
            "/login",
            &format!("username=joueur&password={USER_PASSWORD}&next=//evil.example"),
        ),
    )
    .await;
    assert_eq!(location.as_deref(), Some("/"));

    // déconnexion : la session est supprimée côté serveur
    let req = Request::post("/logout")
        .header(header::COOKIE, &token)
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = send_full(&t.anon, req).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert!(set_cookie(&headers).unwrap().contains("Max-Age=0"));
    let req = Request::get("/")
        .header(header::COOKIE, &token)
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&t.anon, req).await.0, StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn users_can_browse_but_not_edit() {
    let t = app(&[("ddos.toml", DDOS)]).await;
    for uri in [
        "/",
        "/gallery",
        "/regles",
        "/cards/ddos",
        "/api/cards",
        "/account",
    ] {
        let (status, _, _) = send(&t.user, get(uri)).await;
        assert!(
            status == StatusCode::OK || status == StatusCode::SERVICE_UNAVAILABLE,
            "{uri}: {status}"
        );
    }
    let (_, _, gallery) = send(&t.user, get("/gallery")).await;
    assert!(!gallery.contains("Nouvelle carte") && !gallery.contains("Recharger"));
    let (_, _, detail) = send(&t.user, get("/cards/ddos")).await;
    assert!(!detail.contains("/cards/ddos/edit") && !detail.contains("Supprimer"));
    let (_, _, menu) = send(&t.user, get("/")).await;
    assert!(!menu.contains("Créer une carte") && !menu.contains("/admin/users"));

    for req in [
        get("/cards/new"),
        get("/cards/ddos/edit"),
        post("/reload"),
        post("/cards/ddos/delete"),
        multipart("/cards", &card_fields("Pirate", &[])),
        get("/admin/users"),
        form("/admin/users", "username=pirate&password=&role=admin"),
    ] {
        let uri = req.uri().to_string();
        let (status, _, _) = send(&t.user, req).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
    }
    assert!(t.cards().join("ddos.toml").exists());
    assert!(!t.cards().join("pirate.toml").exists());
}

#[tokio::test]
async fn read_only_mode_hides_the_editor_even_for_admins() {
    let mut t = app(&[("ddos.toml", DDOS)]).await;
    t.state = t.state.clone().with_editor(false);
    t.rebuild();
    for req in [
        get("/cards/new"),
        post("/reload"),
        multipart("/cards", &card_fields("X", &[])),
    ] {
        let uri = req.uri().to_string();
        assert_eq!(send(&t.app, req).await.0, StatusCode::NOT_FOUND, "{uri}");
    }
    let (_, _, menu) = send(&t.app, get("/")).await;
    assert!(!menu.contains("Créer une carte") && menu.contains("/admin/users"));
    let (status, _, body) = send(&t.app, get("/admin/users")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Éditeur de cartes désactivé"));
}

#[tokio::test]
async fn cross_site_posts_are_rejected() {
    let t = app(&[]).await;
    let req = Request::post("/reload")
        .header(header::HOST, "dcb.example")
        .header(header::ORIGIN, "https://evil.example")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&t.app, req).await.0, StatusCode::FORBIDDEN);
    let req = Request::post("/reload")
        .header(header::HOST, "dcb.example")
        .header(header::ORIGIN, "https://dcb.example")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&t.app, req).await.0, StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn admin_invites_and_manages_accounts() {
    let t = app(&[]).await;
    // mot de passe généré, affiché une fois
    let (status, _, body) = send(
        &t.app,
        form("/admin/users", "username=Jean.Dupont&password=&role=user"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Compte « jean.dupont » créé"));
    let generated = body
        .split(r#"<code class="secret-value">"#)
        .nth(1)
        .and_then(|s| s.split("</code>").next())
        .unwrap()
        .to_owned();
    assert_eq!(generated.len(), 16);
    let (status, _, _) = send(
        &t.anon,
        form(
            "/login",
            &format!("username=jean.dupont&password={generated}"),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "le joueur invité peut se connecter"
    );

    // erreurs de saisie
    let (status, _, body) = send(
        &t.app,
        form("/admin/users", "username=jean.dupont&password=&role=user"),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body.contains("déjà pris"));
    let (status, _, body) = send(
        &t.app,
        form("/admin/users", "username=x&password=court&role=user"),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body.contains("Identifiant"));

    let joueur = auth::find_user_by_name(&t.state.app_db, "joueur")
        .await
        .unwrap()
        .unwrap();
    // promotion
    let (status, _, body) = send(
        &t.app,
        form(&format!("/admin/users/{}/role", joueur.id), "role=admin"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("est maintenant Admin"));
    let (status, _, _) = send(&t.user, get("/admin/users")).await;
    assert_eq!(status, StatusCode::OK, "le rôle s'applique tout de suite");
    send(
        &t.app,
        form(&format!("/admin/users/{}/role", joueur.id), "role=user"),
    )
    .await;

    // désactivation : ses sessions sont fermées
    let (_, _, body) = send(&t.app, post(&format!("/admin/users/{}/active", joueur.id))).await;
    assert!(body.contains("désactivé"));
    assert_eq!(send(&t.user, get("/")).await.0, StatusCode::SEE_OTHER);
    let (status, _, _) = send(
        &t.anon,
        form(
            "/login",
            &format!("username=joueur&password={USER_PASSWORD}"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "compte désactivé");
    send(&t.app, post(&format!("/admin/users/{}/active", joueur.id))).await;

    // nouveau mot de passe généré
    let (_, _, body) = send(
        &t.app,
        post(&format!("/admin/users/{}/password", joueur.id)),
    )
    .await;
    assert!(body.contains("secret-value"));
    let (status, _, _) = send(
        &t.anon,
        form(
            "/login",
            &format!("username=joueur&password={USER_PASSWORD}"),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "l'ancien mot de passe ne marche plus"
    );

    // un admin ne se modifie pas lui-même ici
    let admin = auth::find_user_by_name(&t.state.app_db, "admin")
        .await
        .unwrap()
        .unwrap();
    for uri in [
        format!("/admin/users/{}/active", admin.id),
        format!("/admin/users/{}/delete", admin.id),
    ] {
        let (status, _, body) = send(&t.app, post(&uri)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{uri}");
        assert!(body.contains("ton propre compte"));
    }

    // suppression
    let (_, _, body) = send(&t.app, post(&format!("/admin/users/{}/delete", joueur.id))).await;
    assert!(body.contains("supprimé"));
    assert!(
        auth::find_user_by_name(&t.state.app_db, "joueur")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn users_change_their_own_password() {
    let t = app(&[]).await;
    let (status, _, body) = send(
        &t.user,
        form(
            "/account",
            "current=faux-mot-de-passe&new=nouveau-mdp-123&confirm=nouveau-mdp-123",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body.contains("actuel est incorrect"));
    let (status, _, body) = send(
        &t.user,
        form(
            "/account",
            &format!("current={USER_PASSWORD}&new=nouveau-mdp-123&confirm=autre-chose-123"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body.contains("ne correspondent pas"));

    let (status, headers, body) = send_full(
        &t.user,
        form(
            "/account",
            &format!("current={USER_PASSWORD}&new=nouveau-mdp-123&confirm=nouveau-mdp-123"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Mot de passe changé"));
    assert!(
        set_cookie(&headers).is_some(),
        "nouvelle session pour cet appareil"
    );
    // l'ancienne session est fermée, le nouveau mot de passe fonctionne
    assert_eq!(send(&t.user, get("/")).await.0, StatusCode::SEE_OTHER);
    let (status, _, _) = send(
        &t.anon,
        form("/login", "username=joueur&password=nouveau-mdp-123"),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn cards_without_art_get_a_type_placeholder() {
    let t = app(&[("ddos.toml", DDOS)]).await;
    let (_, _, body) = send(&t.app, get("/cards/ddos")).await;
    assert!(
        body.contains(r#"class="card-block card-art is-empty""#),
        "{body}"
    );
    assert!(body.contains("card-art-icon"));
    // l'icône Incident (triangle) et pas l'ancienne icône d'image
    assert!(body.contains(r#"<path d="M12 3 2 20h20L12 3z"/>"#));

    let (_, _, _) = send(
        &t.app,
        multipart("/cards", &card_fields("Crash Loop", &[("art", PNG)])),
    )
    .await;
    let (_, _, body) = send(&t.app, get("/cards/crash-loop")).await;
    assert!(body.contains(r#"<img src="/art/crash-loop.png""#));
    assert!(!body.contains("is-empty"));
}

#[tokio::test]
async fn shape_is_saved_rendered_and_filterable() {
    let t = app(&[("ddos.toml", DDOS)]).await;
    // une carte existante sans `shape` garde le contour plein, et le fichier n'est pas réécrit
    let (_, _, body) = send(&t.app, get("/cards/ddos")).await;
    assert!(body.contains(r#"data-shape="solid""#));

    let (status, _, _) = send(
        &t.app,
        multipart("/cards", &card_fields("Neon Pod", &[("shape", b"neon")])),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let toml = std::fs::read_to_string(t.cards().join("neon-pod.toml")).unwrap();
    assert!(toml.contains("shape = \"neon\""), "{toml}");
    let (_, _, body) = send(&t.app, get("/cards/neon-pod")).await;
    assert!(body.contains(r#"data-shape="neon""#));

    // `solid` n'est pas écrit dans le fichier
    send(
        &t.app,
        multipart("/cards", &card_fields("Plain Pod", &[("shape", b"solid")])),
    )
    .await;
    let toml = std::fs::read_to_string(t.cards().join("plain-pod.toml")).unwrap();
    assert!(!toml.contains("shape"), "{toml}");

    // filtre de la galerie et de l'API
    let (_, _, body) = send(&t.app, get("/gallery?shape=neon")).await;
    assert!(body.contains("NEON POD") || body.contains("Neon Pod"));
    assert!(!body.contains("DDOS"));
    assert!(body.contains("Tous les contours"));
    let (_, _, json) = send(&t.app, get("/api/cards?shape=neon")).await;
    assert!(
        json.contains("\"shape\":\"neon\"") && !json.contains("DDOS"),
        "{json}"
    );
    let (status, _, _) = send(&t.app, get("/api/cards?shape=round")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, _, meta) = send(&t.app, get("/api/meta")).await;
    assert!(meta.contains("\"shapes\"") && meta.contains("chamfer"));

    // valeur inconnue dans le formulaire
    let (status, _, body) = send(
        &t.app,
        multipart("/cards", &card_fields("Bad Pod", &[("shape", b"round")])),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body.contains("valeur inconnue"));

    // valeur inconnue dans un fichier : la carte est ignorée avec un message clair
    std::fs::write(
        t.cards().join("broken.toml"),
        "name = \"BROKEN\"\ntype = \"app\"\nshape = \"round\"\n",
    )
    .unwrap();
    send(&t.app, post("/reload")).await;
    let (_, _, body) = send(&t.app, get("/gallery")).await;
    // le message liste les valeurs valides (guillemets échappés en HTML)
    assert!(
        body.contains("broken.toml") && body.contains("inconnu") && body.contains("chamfer"),
        "{body}"
    );
}

#[tokio::test]
async fn concepts_page_renders_live_cards_with_a_toc() {
    const CONCEPTS: &str = include_str!("../../../rules/concepts.toml");
    let argo = "name = \"ARGO CD\"\ntype = \"app\"\neffect = \"- Débloque la carte Application Set\\n- Ajoute +1 deployment MAX\"\n";
    let set = "name = \"APPLICATION SET\"\ntype = \"app\"\n";
    let t = app_with_rules(
        &[
            ("argo-cd.toml", argo),
            ("application-set.toml", set),
            ("ddos.toml", DDOS),
        ],
        Some(RULES),
    )
    .await;
    // sans concepts.toml : page explicative
    let (status, _, body) = send(&t.app, get("/regles/concepts")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("rules/concepts.toml"));

    std::fs::write(t.dir.join("concepts.toml"), CONCEPTS).unwrap();
    t.state.reload().await.unwrap();
    let (status, _, body) = send(&t.user, get("/regles/concepts")).await;
    assert_eq!(status, StatusCode::OK);
    // sommaire avec la partie restante (skins et contours sont passés sur la page des règles)
    assert!(body.contains(r#"<nav class="cdoc-toc""#));
    assert!(body.contains(r##"href="#p1""##) && body.contains(r##"href="#p1-s11""##));
    assert!(!body.contains(r##"href="#p2""##) && !body.contains("sk-dracula"));
    assert!(!body.contains("Moderniser le contour"));
    // vraies cartes de la collection, au gabarit v2
    assert!(body.contains("ARGO CD") && body.contains("DDOS"));
    assert!(body.contains(r#"<span class="code-file">effect.yaml</span>"#));
    assert!(body.contains(r#"<span class="y-num">+1</span>"#));
    // carte citée dans un effet
    assert!(body.contains(r#"<span class="y-ref" data-type="app">Application Set</span>"#));
    assert!(body.contains("DCB-"));
    let (status, _, _) = send(&t.app, get("/static/concepts.css")).await;
    assert_eq!(status, StatusCode::OK);
    // réservé aux joueurs connectés, lien depuis les règles
    assert_eq!(
        send(&t.anon, get("/regles/concepts")).await.0,
        StatusCode::SEE_OTHER
    );
    let (_, _, rules) = send(&t.app, get("/regles")).await;
    assert!(rules.contains(r#"href="/regles/concepts""#));

    // une clé inconnue est signalée, la dernière version valide reste affichée
    std::fs::write(
        t.dir.join("concepts.toml"),
        CONCEPTS.replacen("key = \"badge\"", "key = \"badgee\"", 1),
    )
    .unwrap();
    t.state.reload().await.unwrap();
    let (status, _, body) = send(&t.app, get("/regles/concepts")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("badgee") && body.contains("dernière version valide"));
}

#[tokio::test]
async fn simplified_menu_and_top_bar() {
    let t = app(&[("ddos.toml", DDOS)]).await;
    let (_, _, menu) = send(&t.app, get("/")).await;
    assert!(menu.contains("Documentation") && menu.contains("and more"));
    assert!(menu.contains(r#"<span class="pill-admin">Admin</span>"#));
    for gone in [
        "Créer une carte",
        "Deck builder",
        "Duel en ligne",
        "Règles du jeu",
    ] {
        assert!(!menu.contains(gone), "{gone}");
    }
    // barre du haut : logo et menu du compte seulement
    let topnav = menu.split(r#"<nav class="topnav">"#).nth(1).unwrap();
    let topnav = topnav.split("</nav>").next().unwrap();
    assert!(!topnav.contains(r#"href="/gallery""#) && !topnav.contains("Nouvelle carte"));
    assert!(topnav.contains("Mon compte"));
    // la création passe par la galerie
    let (_, _, gallery) = send(&t.app, get("/gallery")).await;
    assert!(
        gallery.contains(
            r#"<a class="btn btn-primary btn-sm" href="/cards/new">+ Nouvelle carte</a>"#
        )
    );
    // un joueur ne voit pas Comptes
    let (_, _, menu) = send(&t.user, get("/")).await;
    assert!(menu.contains("Documentation") && !menu.contains("/admin/users"));
}

#[tokio::test]
async fn sandbox_lets_players_create_and_edit_only_their_cards() {
    let mut t = app(&[("ddos.toml", DDOS)]).await;
    t.state = t.state.clone().with_editor(false).with_sandbox(true);
    t.rebuild();
    // un second joueur
    let other = auth::create_user(&t.state.app_db, "voisin", USER_PASSWORD, Role::User)
        .await
        .unwrap()
        .unwrap();
    let other_token = auth::create_session(&t.state.app_db, other).await.unwrap();
    let voisin = with_session(router(t.state.clone()), &other_token);

    // bandeau et bouton de création pour un joueur, sans rechargement ni fichiers à commiter
    let (_, _, gallery) = send(&t.user, get("/gallery")).await;
    assert!(gallery.contains("Mode démo") && gallery.contains("Nouvelle carte"));
    assert!(!gallery.contains("Recharger"));
    let (status, _, form_page) = send(&t.user, get("/cards/new")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(form_page.contains("Mode démo"));

    // le joueur crée sa carte et peut la modifier
    let (status, location, _) =
        send(&t.user, multipart("/cards", &card_fields("Ma Carte", &[]))).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location.as_deref(), Some("/cards/ma-carte"));
    let (_, _, detail) = send(&t.user, get("/cards/ma-carte")).await;
    assert!(detail.contains("/cards/ma-carte/edit") && detail.contains("Carte de démo"));
    assert!(!detail.contains("Fichiers à commiter"));
    assert_eq!(
        send(&t.user, get("/cards/ma-carte/edit")).await.0,
        StatusCode::OK
    );
    let (status, _, _) = send(
        &t.user,
        multipart("/cards/ma-carte", &card_fields("Ma Carte v2", &[])),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    // cartes du dépôt et cartes d'un autre joueur : lecture seule
    let (_, _, detail) = send(&t.user, get("/cards/ddos")).await;
    assert!(!detail.contains("/cards/ddos/edit") && !detail.contains("Carte de démo"));
    let (_, _, detail) = send(&voisin, get("/cards/ma-carte")).await;
    assert!(!detail.contains("/cards/ma-carte/edit"));
    for (who, req) in [
        (&t.user, get("/cards/ddos/edit")),
        (
            &t.user,
            multipart("/cards/ddos", &card_fields("Pirate", &[])),
        ),
        (&t.user, post("/cards/ddos/delete")),
        (&t.user, post("/reload")),
        (&voisin, get("/cards/ma-carte/edit")),
        (&voisin, post("/cards/ma-carte/delete")),
    ] {
        let uri = req.uri().to_string();
        assert_eq!(send(who, req).await.0, StatusCode::FORBIDDEN, "{uri}");
    }
    assert!(t.cards().join("ddos.toml").exists());

    // les visiteurs anonymes sont toujours renvoyés vers la connexion
    assert_eq!(
        send(&t.anon, get("/cards/new")).await.0,
        StatusCode::SEE_OTHER
    );

    // l'admin peut tout faire, y compris supprimer la carte d'un joueur
    assert_eq!(
        send(&t.app, get("/cards/ddos/edit")).await.0,
        StatusCode::OK
    );
    assert_eq!(
        send(&t.app, post("/cards/ma-carte/delete")).await.0,
        StatusCode::SEE_OTHER
    );
    assert!(!t.cards().join("ma-carte.toml").exists());

    // le joueur supprime sa propre carte
    send(&t.user, multipart("/cards", &card_fields("Autre", &[]))).await;
    assert_eq!(
        send(&t.user, post("/cards/autre/delete")).await.0,
        StatusCode::SEE_OTHER
    );
    assert!(!t.cards().join("autre.toml").exists());
}

#[tokio::test]
async fn size_is_saved_rendered_and_filterable_without_rarity_box() {
    let t = app(&[("ddos.toml", DDOS)]).await;
    // une carte existante sans `size` reste en taille normale, sans case Rarity
    let (_, _, body) = send(&t.app, get("/cards/ddos")).await;
    assert!(body.contains(r#"data-size="normal""#));
    assert!(!body.contains("card-stat-rarity") && !body.contains(">Rarity<"));
    assert!(!body.contains("card-stat-icon"));

    let (status, _, _) = send(
        &t.app,
        multipart("/cards", &card_fields("Small Pod", &[("size", b"compact")])),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let toml = std::fs::read_to_string(t.cards().join("small-pod.toml")).unwrap();
    assert!(toml.contains("size = \"compact\""), "{toml}");
    let (_, _, body) = send(&t.app, get("/cards/small-pod")).await;
    assert!(body.contains(r#"data-size="compact""#) && body.contains("card-stat-icon"));
    assert!(body.contains("Compacte"));

    // `normal` n'est pas écrit dans le fichier
    send(
        &t.app,
        multipart("/cards", &card_fields("Big Pod", &[("size", b"normal")])),
    )
    .await;
    let toml = std::fs::read_to_string(t.cards().join("big-pod.toml")).unwrap();
    assert!(!toml.contains("size"), "{toml}");

    // éditeur, galerie et API
    let (_, _, form_page) = send(&t.app, get("/cards/new")).await;
    assert!(form_page.contains(r#"name="size" value="compact""#));
    let (_, _, body) = send(&t.app, get("/gallery?size=compact")).await;
    assert!(body.contains("Small Pod") && !body.contains("DDOS"));
    assert!(body.contains("Toutes les tailles"));
    let (_, _, json) = send(&t.app, get("/api/cards?size=compact")).await;
    assert!(
        json.contains("\"size\":\"compact\"") && !json.contains("DDOS"),
        "{json}"
    );
    let (status, _, _) = send(&t.app, get("/api/cards?size=tiny")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, _, meta) = send(&t.app, get("/api/meta")).await;
    assert!(meta.contains("\"sizes\"") && meta.contains("compact"));

    // valeur inconnue dans le formulaire puis dans un fichier
    let (status, _, _) = send(
        &t.app,
        multipart("/cards", &card_fields("Bad Pod", &[("size", b"tiny")])),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    std::fs::write(
        t.cards().join("broken.toml"),
        "name = \"BROKEN\"\ntype = \"app\"\nsize = \"tiny\"\n",
    )
    .unwrap();
    send(&t.app, post("/reload")).await;
    let (_, _, body) = send(&t.app, get("/gallery")).await;
    assert!(
        body.contains("broken.toml") && body.contains("compact"),
        "{body}"
    );
}
