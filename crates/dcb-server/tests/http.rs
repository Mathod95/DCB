//! Tests de bout en bout sur le routeur, avec une base SQLite temporaire.

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use dcb_server::{AppState, db, router, seed};
use http_body_util::BodyExt;
use tower::ServiceExt;

struct TestApp {
    app: axum::Router,
    dir: PathBuf,
}

impl Drop for TestApp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn app(with_seed: bool) -> TestApp {
    let dir = std::env::temp_dir().join(format!("dcb-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let pool = db::connect(&format!("sqlite://{}", dir.join("t.db").display()))
        .await
        .unwrap();
    if with_seed {
        seed::run_if_empty(&pool).await.unwrap();
    }
    let upload_dir = dir.join("uploads");
    TestApp {
        app: router(AppState { pool, upload_dir }),
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

fn card_fields<'a>(name: &'a str, extra: &[(&'a str, &'a [u8])]) -> Vec<(&'a str, &'a [u8])> {
    let mut f: Vec<(&str, &[u8])> = vec![
        ("name", name.as_bytes()),
        ("card_type", b"incident"),
        ("skin", b"dark"),
        ("rarity", b"gold"),
        ("description", b"Le cluster ne repond plus"),
        ("effect", b"- Inflige 3 degats\r\n- Duree 2 tours"),
        ("deploy_cost", b"4"),
        ("daily_cost", b"1"),
        ("flavor_text", b"Ca marchait sur ma machine."),
    ];
    f.extend_from_slice(extra);
    f
}

#[tokio::test]
async fn pages_render() {
    let t = app(true).await;
    for uri in [
        "/",
        "/gallery",
        "/gallery?type=app",
        "/cards/new",
        "/cards/1",
        "/cards/1/edit",
    ] {
        let (status, _, body) = send(&t.app, get(uri)).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert!(body.contains("DevCard"), "{uri}");
    }
    let (status, _, _) = send(&t.app, get("/cards/9999")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn gallery_filters_by_type() {
    let t = app(true).await;
    let (_, _, body) = send(&t.app, get("/gallery?type=incident")).await;
    assert!(body.contains("DDOS"));
    assert!(!body.contains("ARGO CD"));
}

#[tokio::test]
async fn create_edit_delete_lifecycle() {
    let t = app(false).await;

    // Création avec illustration
    let (status, location, _) = send(
        &t.app,
        multipart("/cards", &card_fields("CRASHLOOP", &[("art", PNG)])),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let location = location.unwrap();
    let id: i64 = location.trim_start_matches("/cards/").parse().unwrap();

    let (_, _, json) = send(&t.app, get(&format!("/api/cards/{id}"))).await;
    let card: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(card["name"], "CRASHLOOP");
    assert_eq!(card["card_type"], "incident");
    assert_eq!(card["effect"], "- Inflige 3 degats\n- Duree 2 tours");
    let art = card["art_path"].as_str().unwrap().to_owned();
    assert!(art.starts_with("/uploads/") && art.ends_with(".png"));
    let (status, _, _) = send(&t.app, get(&art)).await;
    assert_eq!(status, StatusCode::OK);

    // Mise à jour sans nouveau fichier : l'illustration est conservée, current_art est ignoré
    let (status, _, _) = send(
        &t.app,
        multipart(
            &format!("/cards/{id}"),
            &card_fields("CRASHLOOP v2", &[("current_art", b"/uploads/x.png")]),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let (_, _, json) = send(&t.app, get(&format!("/api/cards/{id}"))).await;
    let card: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(card["name"], "CRASHLOOP v2");
    assert_eq!(card["art_path"].as_str(), Some(art.as_str()));

    // Retrait de l'illustration : le fichier est supprimé
    send(
        &t.app,
        multipart(
            &format!("/cards/{id}"),
            &card_fields("CRASHLOOP v2", &[("remove_art", b"1")]),
        ),
    )
    .await;
    let (status, _, _) = send(&t.app, get(&art)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Suppression
    let delete = Request::post(format!("/cards/{id}/delete"))
        .body(Body::empty())
        .unwrap();
    let (status, location, _) = send(&t.app, delete).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location.as_deref(), Some("/gallery"));
    let (status, _, _) = send(&t.app, get(&format!("/cards/{id}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn invalid_submission_rerenders_form_with_errors() {
    let t = app(false).await;
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

    let (_, _, json) = send(&t.app, get("/api/cards")).await;
    assert_eq!(json, "[]");
}

#[tokio::test]
async fn preview_renders_card_and_drops_unsafe_art() {
    let t = app(false).await;
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
    assert!(
        body.contains("&#60;script&#62;") || body.contains("&lt;script&gt;"),
        "le nom doit être échappé"
    );
    assert!(!body.contains("<script>"));
    assert!(!body.contains("passwd"));
}

#[tokio::test]
async fn api_rejects_unknown_filter() {
    let t = app(true).await;
    let (status, _, _) = send(&t.app, get("/api/cards?type=package")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, body) = send(&t.app, get("/api/meta")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("#a538f8"));
}
