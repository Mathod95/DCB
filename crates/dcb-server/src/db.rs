//! Accès SQLite. Les requêtes sont vérifiées à l'exécution (pas de `query!`) pour ne pas
//! exiger une base au moment de la compilation.

use std::str::FromStr;

use dcb_core::{Card, CardDraft, CardType, Rarity};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{FromRow, SqlitePool};

pub async fn connect(url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    sqlx::migrate!("../../migrations").run(&pool).await?;
    Ok(pool)
}

#[derive(FromRow)]
struct CardRow {
    id: i64,
    name: String,
    card_type: String,
    skin: String,
    rarity: String,
    description: String,
    effect: String,
    deploy_cost: i64,
    daily_cost: i64,
    flavor_text: String,
    art_path: Option<String>,
}

impl TryFrom<CardRow> for Card {
    type Error = sqlx::Error;

    fn try_from(r: CardRow) -> Result<Self, Self::Error> {
        let decode = |e: dcb_core::UnknownVariant| sqlx::Error::Decode(Box::new(e));
        Ok(Card {
            id: r.id,
            draft: CardDraft {
                name: r.name,
                card_type: r.card_type.parse().map_err(decode)?,
                skin: r.skin.parse().map_err(decode)?,
                rarity: r.rarity.parse().map_err(decode)?,
                description: r.description,
                effect: r.effect,
                deploy_cost: r.deploy_cost,
                daily_cost: r.daily_cost,
                flavor_text: r.flavor_text,
                art_path: r.art_path,
            },
        })
    }
}

/// Colonnes lues, sous forme de macro pour composer des requêtes `&'static str` avec `concat!`
/// (sqlx refuse les chaînes SQL construites à l'exécution).
macro_rules! columns {
    () => {
        "id, name, card_type, skin, rarity, description, effect, \
         deploy_cost, daily_cost, flavor_text, art_path"
    };
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Filter {
    pub card_type: Option<CardType>,
    pub rarity: Option<Rarity>,
}

pub async fn list(pool: &SqlitePool, filter: Filter) -> sqlx::Result<Vec<Card>> {
    const SQL: &str = concat!(
        "SELECT ",
        columns!(),
        " FROM cards ",
        "WHERE (?1 IS NULL OR card_type = ?1) AND (?2 IS NULL OR rarity = ?2) ",
        "ORDER BY card_type, name COLLATE NOCASE"
    );
    sqlx::query_as::<_, CardRow>(SQL)
        .bind(filter.card_type.map(|t| t.slug()))
        .bind(filter.rarity.map(|r| r.slug()))
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(Card::try_from)
        .collect()
}

pub async fn get(pool: &SqlitePool, id: i64) -> sqlx::Result<Option<Card>> {
    const SQL: &str = concat!("SELECT ", columns!(), " FROM cards WHERE id = ?");
    sqlx::query_as::<_, CardRow>(SQL)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .map(Card::try_from)
        .transpose()
}

pub async fn count(pool: &SqlitePool) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM cards")
        .fetch_one(pool)
        .await
}

pub async fn insert(pool: &SqlitePool, d: &CardDraft) -> sqlx::Result<i64> {
    let res = sqlx::query(
        "INSERT INTO cards (name, card_type, skin, rarity, description, effect, \
                            deploy_cost, daily_cost, flavor_text, art_path) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&d.name)
    .bind(d.card_type.slug())
    .bind(d.skin.slug())
    .bind(d.rarity.slug())
    .bind(&d.description)
    .bind(&d.effect)
    .bind(d.deploy_cost)
    .bind(d.daily_cost)
    .bind(&d.flavor_text)
    .bind(&d.art_path)
    .execute(pool)
    .await?;
    Ok(res.last_insert_rowid())
}

/// Renvoie `false` si la carte n'existe pas.
pub async fn update(pool: &SqlitePool, id: i64, d: &CardDraft) -> sqlx::Result<bool> {
    let res = sqlx::query(
        "UPDATE cards SET name = ?, card_type = ?, skin = ?, rarity = ?, description = ?, \
                          effect = ?, deploy_cost = ?, daily_cost = ?, flavor_text = ?, \
                          art_path = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
         WHERE id = ?",
    )
    .bind(&d.name)
    .bind(d.card_type.slug())
    .bind(d.skin.slug())
    .bind(d.rarity.slug())
    .bind(&d.description)
    .bind(&d.effect)
    .bind(d.deploy_cost)
    .bind(d.daily_cost)
    .bind(&d.flavor_text)
    .bind(&d.art_path)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// Supprime la carte et renvoie son illustration, pour que l'appelant nettoie le fichier.
pub async fn delete(pool: &SqlitePool, id: i64) -> sqlx::Result<Option<Option<String>>> {
    sqlx::query_scalar::<_, Option<String>>("DELETE FROM cards WHERE id = ? RETURNING art_path")
        .bind(id)
        .fetch_optional(pool)
        .await
}
