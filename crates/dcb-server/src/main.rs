use std::path::PathBuf;

use dcb_server::{AppState, CardStore, db, router};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dcb_server=info,tower_http=info".into()),
        )
        .init();

    // Source de vérité, à commiter.
    let cards_dir =
        PathBuf::from(std::env::var("DCB_CARDS_DIR").unwrap_or_else(|_| "cards".into()));
    // Cache reconstruit à chaque démarrage, jamais commité.
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data/cache.db".into());
    let bind = std::env::var("DCB_BIND").unwrap_or_else(|_| "127.0.0.1:3000".into());

    if let Some(path) = database_url
        .strip_prefix("sqlite://")
        .and_then(|p| PathBuf::from(p).parent().map(PathBuf::from))
        && !path.as_os_str().is_empty()
    {
        std::fs::create_dir_all(&path)?;
    }

    let pool = db::connect(&database_url).await?;
    let rules_file = PathBuf::from(
        std::env::var("DCB_RULES_FILE").unwrap_or_else(|_| "rules/regles.toml".into()),
    );
    let state = AppState::new(pool, CardStore::new(&cards_dir)).with_rules_file(&rules_file);
    let n = state.reload().await?;
    tracing::info!(
        "{n} carte{} chargée{} depuis {}",
        if n > 1 { "s" } else { "" },
        if n > 1 { "s" } else { "" },
        cards_dir.display()
    );

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("DevCardBattle sur http://{bind}");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
