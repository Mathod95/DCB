use std::path::PathBuf;

use dcb_server::auth::{self, Role};
use dcb_server::{AppState, CardStore, db, router};
use tracing_subscriber::EnvFilter;

const USAGE: &str = "\
Usage :
  dcb-server                         démarre le serveur
  dcb-server user add <nom> [--admin] crée un compte (mot de passe demandé, ou DCB_PASSWORD)
  dcb-server user passwd <nom>       change le mot de passe d'un compte
  dcb-server user list               liste les comptes";

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn env_flag(key: &str, default: bool) -> bool {
    match std::env::var(key).as_deref() {
        Ok("1" | "true" | "yes" | "on") => true,
        Ok("0" | "false" | "no" | "off") => false,
        _ => default,
    }
}

/// Crée le dossier parent d'une base SQLite (`sqlite://data/x.db` → `data/`).
fn ensure_parent(url: &str) -> anyhow::Result<()> {
    if let Some(path) = url
        .strip_prefix("sqlite://")
        .and_then(|p| PathBuf::from(p).parent().map(PathBuf::from))
        && !path.as_os_str().is_empty()
    {
        std::fs::create_dir_all(&path)?;
    }
    Ok(())
}

async fn app_db() -> anyhow::Result<sqlx::SqlitePool> {
    // Base persistante (comptes) : à sauvegarder, jamais reconstruite.
    let url = env_or("DCB_APP_DATABASE_URL", "sqlite://data/app.db");
    ensure_parent(&url)?;
    auth::connect(&url).await
}

fn read_password(prompt_for: &str) -> anyhow::Result<String> {
    if let Ok(p) = std::env::var("DCB_PASSWORD") {
        return Ok(p);
    }
    let first = rpassword::prompt_password(format!("Mot de passe pour {prompt_for} : "))?;
    let second = rpassword::prompt_password("Confirmer : ")?;
    anyhow::ensure!(
        first == second,
        "les deux mots de passe ne correspondent pas"
    );
    Ok(first)
}

async fn user_command(args: &[String]) -> anyhow::Result<()> {
    let pool = app_db().await?;
    match args {
        [cmd, name, rest @ ..] if cmd == "add" => {
            let role = if rest.iter().any(|a| a == "--admin") {
                Role::Admin
            } else {
                Role::User
            };
            let password = read_password(name)?;
            match auth::create_user(&pool, name, &password, role).await? {
                Ok(_) => println!(
                    "Compte {} créé ({}).",
                    name.trim().to_lowercase(),
                    role.label()
                ),
                Err(e) => anyhow::bail!(e),
            }
        }
        [cmd, name] if cmd == "passwd" => {
            let user = auth::find_user_by_name(&pool, name)
                .await?
                .ok_or_else(|| anyhow::anyhow!("aucun compte « {name} »"))?;
            let password = read_password(&user.username)?;
            auth::validate_password(&password)
                .map_err(|e| anyhow::anyhow!("mot de passe : {e}"))?;
            auth::set_password(&pool, user.id, &password).await?;
            auth::delete_sessions_of(&pool, user.id).await?;
            println!(
                "Mot de passe de {} changé, ses sessions ont été fermées.",
                user.username
            );
        }
        [cmd] if cmd == "list" => {
            for u in auth::list_users(&pool).await? {
                println!(
                    "{:<24} {:<12} {}",
                    u.username,
                    u.role.label(),
                    if u.active { "actif" } else { "désactivé" }
                );
            }
        }
        _ => anyhow::bail!("{USAGE}"),
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dcb_server=info,tower_http=info".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => {}
        Some("user") => return user_command(&args[1..]).await,
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            return Ok(());
        }
        Some(other) => anyhow::bail!("commande inconnue « {other} »\n\n{USAGE}"),
    }

    // Source de vérité, à commiter.
    let cards_dir = PathBuf::from(env_or("DCB_CARDS_DIR", "cards"));
    let rules_file = PathBuf::from(env_or("DCB_RULES_FILE", "rules/regles.toml"));
    // Cache reconstruit à chaque démarrage, jamais commité.
    let database_url = env_or("DATABASE_URL", "sqlite://data/cache.db");
    let bind = env_or("DCB_BIND", "127.0.0.1:3000");
    // En production : éditeur coupé (cartes en lecture seule) et cookies Secure (HTTPS).
    let editor = env_flag("DCB_EDITOR", true);
    let secure_cookies = env_flag("DCB_SECURE_COOKIES", false);
    // Mode démo : éditeur ouvert à tous, cartes créées en ligne perdues au déploiement suivant.
    let sandbox = env_flag("DCB_SANDBOX", false);

    ensure_parent(&database_url)?;
    let pool = db::connect(&database_url).await?;
    let accounts = app_db().await?;

    // Premier démarrage d'un conteneur : création de l'admin depuis l'environnement.
    if auth::count_users(&accounts).await? == 0 {
        match (
            std::env::var("DCB_ADMIN_USERNAME"),
            std::env::var("DCB_ADMIN_PASSWORD"),
        ) {
            (Ok(name), Ok(password)) => {
                match auth::create_user(&accounts, &name, &password, Role::Admin).await? {
                    Ok(_) => tracing::info!("compte admin {name} créé depuis DCB_ADMIN_USERNAME"),
                    Err(e) => anyhow::bail!("DCB_ADMIN_USERNAME/DCB_ADMIN_PASSWORD : {e}"),
                }
            }
            _ => tracing::warn!(
                "aucun compte : crée le premier admin avec `dcb-server user add <nom> --admin`"
            ),
        }
    }

    let state = AppState::new(pool, accounts, CardStore::new(&cards_dir))
        .with_rules_file(&rules_file)
        .with_editor(editor)
        .with_secure_cookies(secure_cookies)
        .with_sandbox(sandbox);
    let n = state.reload().await?;
    tracing::info!(
        "{n} carte{} chargée{} depuis {}{}",
        if n > 1 { "s" } else { "" },
        if n > 1 { "s" } else { "" },
        cards_dir.display(),
        if sandbox {
            " (mode démo: éditeur ouvert à tous, cartes non conservées)"
        } else if editor {
            ""
        } else {
            " (éditeur désactivé, lecture seule)"
        }
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
