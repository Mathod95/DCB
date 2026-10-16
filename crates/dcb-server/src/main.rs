use std::path::PathBuf;

use dcb_server::{AppState, db, router, seed};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dcb_server=info,tower_http=info".into()),
        )
        .init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data/dcb.db".into());
    let upload_dir =
        PathBuf::from(std::env::var("DCB_UPLOAD_DIR").unwrap_or_else(|_| "data/uploads".into()));
    let bind = std::env::var("DCB_BIND").unwrap_or_else(|_| "127.0.0.1:3000".into());

    if let Some(path) = database_url
        .strip_prefix("sqlite://")
        .and_then(|p| PathBuf::from(p).parent().map(PathBuf::from))
        && !path.as_os_str().is_empty()
    {
        std::fs::create_dir_all(&path)?;
    }
    std::fs::create_dir_all(&upload_dir)?;

    let pool = db::connect(&database_url).await?;
    if std::env::var("DCB_SEED").as_deref() != Ok("0") {
        let n = seed::run_if_empty(&pool).await?;
        if n > 0 {
            tracing::info!("{n} cartes de démo ajoutées");
        }
    }

    let app = router(AppState { pool, upload_dir });
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("DevCardBattle sur http://{bind}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
