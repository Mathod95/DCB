//! Cache SQLite des cartes, reconstruit depuis `cards/*.toml` (voir `store`). Il sert aux
//! lectures (galerie, filtres, API) ; chaque écriture passe d'abord par le fichier TOML.
//!
//! Les requêtes sont vérifiées à l'exécution (pas de `query!`) pour ne pas exiger de base à la
//! compilation, et écrites en `&'static str` car sqlx refuse le SQL construit à l'exécution.

use std::str::FromStr;

use dcb_core::{Card, CardDraft, CardType, Foil, Shape, Size};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{FromRow, Sqlite, SqlitePool};

pub async fn connect(url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(url)?.create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    sqlx::migrate!("../../migrations").run(&pool).await?;
    Ok(pool)
}

#[derive(FromRow)]
struct CardRow {
    slug: String,
    name: String,
    card_type: String,
    skin: String,
    foil: String,
    shape: String,
    size: String,
    description: String,
    effect: String,
    deploy_cost: i64,
    daily_cost: i64,
    flavor_text: String,
    art: Option<String>,
}

impl TryFrom<CardRow> for Card {
    type Error = sqlx::Error;

    fn try_from(r: CardRow) -> Result<Self, Self::Error> {
        let decode = |e: dcb_core::UnknownVariant| sqlx::Error::Decode(Box::new(e));
        Ok(Card {
            slug: r.slug,
            draft: CardDraft {
                name: r.name,
                card_type: r.card_type.parse().map_err(decode)?,
                skin: r.skin.parse().map_err(decode)?,
                foil: r.foil.parse().map_err(decode)?,
                shape: r.shape.parse().map_err(decode)?,
                size: r.size.parse().map_err(decode)?,
                description: r.description,
                effect: r.effect,
                deploy_cost: r.deploy_cost,
                daily_cost: r.daily_cost,
                flavor_text: r.flavor_text,
                art: r.art,
            },
        })
    }
}

/// Colonnes lues, en macro pour composer des requêtes `&'static str` avec `concat!`.
macro_rules! columns {
    () => {
        "slug, name, card_type, skin, foil, shape, size, description, effect, \
         deploy_cost, daily_cost, flavor_text, art"
    };
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Filter {
    pub card_type: Option<CardType>,
    pub foil: Option<Foil>,
    pub shape: Option<Shape>,
    pub size: Option<Size>,
}

pub async fn list(pool: &SqlitePool, filter: Filter) -> sqlx::Result<Vec<Card>> {
    const SQL: &str = concat!(
        "SELECT ",
        columns!(),
        " FROM cards ",
        "WHERE (?1 IS NULL OR card_type = ?1) AND (?2 IS NULL OR foil = ?2) ",
        "AND (?3 IS NULL OR shape = ?3) AND (?4 IS NULL OR size = ?4) ",
        "ORDER BY card_type, name COLLATE NOCASE"
    );
    sqlx::query_as::<_, CardRow>(SQL)
        .bind(filter.card_type.map(|t| t.slug()))
        .bind(filter.foil.map(|f| f.slug()))
        .bind(filter.shape.map(|s| s.slug()))
        .bind(filter.size.map(|s| s.slug()))
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(Card::try_from)
        .collect()
}

pub async fn get(pool: &SqlitePool, slug: &str) -> sqlx::Result<Option<Card>> {
    const SQL: &str = concat!("SELECT ", columns!(), " FROM cards WHERE slug = ?");
    sqlx::query_as::<_, CardRow>(SQL)
        .bind(slug)
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

async fn upsert_with<'e, E>(executor: E, c: &Card) -> sqlx::Result<()>
where
    E: sqlx::Executor<'e, Database = Sqlite>,
{
    let d = &c.draft;
    sqlx::query(
        "INSERT INTO cards (slug, name, card_type, skin, foil, shape, size, description, \
                            effect, deploy_cost, daily_cost, flavor_text, art) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT (slug) DO UPDATE SET \
            name = excluded.name, card_type = excluded.card_type, skin = excluded.skin, \
            foil = excluded.foil, shape = excluded.shape, size = excluded.size, \
            description = excluded.description, \
            effect = excluded.effect, deploy_cost = excluded.deploy_cost, \
            daily_cost = excluded.daily_cost, flavor_text = excluded.flavor_text, \
            art = excluded.art",
    )
    .bind(&c.slug)
    .bind(&d.name)
    .bind(d.card_type.slug())
    .bind(d.skin.slug())
    .bind(d.foil.slug())
    .bind(d.shape.slug())
    .bind(d.size.slug())
    .bind(&d.description)
    .bind(&d.effect)
    .bind(d.deploy_cost)
    .bind(d.daily_cost)
    .bind(&d.flavor_text)
    .bind(&d.art)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn upsert(pool: &SqlitePool, card: &Card) -> sqlx::Result<()> {
    upsert_with(pool, card).await
}

pub async fn delete(pool: &SqlitePool, slug: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM cards WHERE slug = ?")
        .bind(slug)
        .execute(pool)
        .await?;
    Ok(())
}

/// Remplace tout le cache par les cartes données, en une transaction : en cas d'erreur, le
/// cache précédent reste intact.
pub async fn replace_all(pool: &SqlitePool, cards: &[Card]) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM cards").execute(&mut *tx).await?;
    for card in cards {
        upsert_with(&mut *tx, card).await?;
    }
    tx.commit().await
}

/// Nombre de cartes par type (les types sans carte sont absents).
pub async fn count_by_type(pool: &SqlitePool) -> sqlx::Result<Vec<(CardType, i64)>> {
    let rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT card_type, COUNT(*) FROM cards GROUP BY card_type")
            .fetch_all(pool)
            .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(t, n)| t.parse().ok().map(|t| (t, n)))
        .collect())
}

/// Une carte tirée au hasard, d'un type donné ou de toute la collection. Les cartes qui ont
/// une description ou un effet passent en priorité : une carte encore vide n'est tirée que
/// s'il n'y en a pas d'autre.
pub async fn random_card(
    pool: &SqlitePool,
    card_type: Option<CardType>,
) -> sqlx::Result<Option<Card>> {
    const SQL: &str = concat!(
        "SELECT ",
        columns!(),
        " FROM cards WHERE (?1 IS NULL OR card_type = ?1) ",
        "ORDER BY (description = '' AND effect = ''), RANDOM() LIMIT 1"
    );
    sqlx::query_as::<_, CardRow>(SQL)
        .bind(card_type.map(|t| t.slug()))
        .fetch_optional(pool)
        .await?
        .map(Card::try_from)
        .transpose()
}
