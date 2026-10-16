//! Tests de bout en bout sur le routeur, avec un dossier `cards/` et une base temporaires.

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use dcb_server::{AppState, CardStore, db, router};
use http_body_util::BodyExt;
use tower::ServiceExt;

struct TestApp {
    app: axum::Router,
    state: AppState,
    dir: PathBuf,
}

impl TestApp {
    fn cards(&self) -> PathBuf {
        self.dir.join("cards")
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Démarre l'app sur un dossier `cards/` contenant les fichiers donnés.
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
    let state = AppState::new(pool, CardStore::new(&cards));
    state.reload().await.unwrap();
    TestApp {
        app: router(state.clone()),
        state,
        dir,
    }
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
        ("rarity", b"gold"),
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
const ARGO: &str = "name = \"ARGO CD\"\ntype = \"app\"\nrarity = \"gold\"\n";

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
        ("rarity", b"basic"),
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
            "name=%3Cscript%3E&card_type=hr&skin=dark&rarity=gold&deploy_cost=7&current_art=..%2F..%2Fetc%2Fpasswd",
        ))
        .unwrap();
    let (status, _, body) = send(&t.app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(r#"data-type="hr""#));
    assert!(body.contains(r#"data-rarity="gold""#));
    assert!(!body.contains("<script>"), "le nom doit être échappé");
    assert!(!body.contains("passwd"));
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
}
