//! Cache SQLite des cartes, reconstruit depuis `cards/*.toml` (voir `store`). Il sert aux
//! lectures (galerie, filtres, API) ; chaque écriture passe d'abord par le fichier TOML.
//!
//! Les requêtes sont vérifiées à l'exécution (pas de `query!`) pour ne pas exiger de base à la
//! compilation, et écrites en `&'static str` car sqlx refuse le SQL construit à l'exécution.

use std::str::FromStr;

use dcb_core::{Badge, Card, CardDraft, CardType, Foil, Shape, Size, Title};
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
    badge: String,
    title: String,
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
                badge: r.badge.parse().map_err(decode)?,
                title: r.title.parse().map_err(decode)?,
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
        "slug, name, card_type, skin, foil, shape, size, badge, title, description, effect, \
         deploy_cost, daily_cost, flavor_text, art"
    };
}

/// Réglages filtrables, dans l'ordre de la galerie : clé d'URL et valeur de la carte.
pub const FILTER_KEYS: &[&str] = &["type", "foil", "shape", "size", "badge", "title"];

/// Filtre de la galerie et de l'API : dans un même réglage, les valeurs choisies s'additionnent
/// (RESOURCE ou INCIDENT) ; d'un réglage à l'autre, elles se cumulent. Une liste vide ne filtre
/// pas.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Filter {
    pub card_types: Vec<CardType>,
    pub foils: Vec<Foil>,
    pub shapes: Vec<Shape>,
    pub sizes: Vec<Size>,
    pub badges: Vec<Badge>,
    pub titles: Vec<Title>,
}

/// Valeur d'un réglage sur une carte, sous la forme de son slug.
pub fn field(d: &CardDraft, key: &str) -> &'static str {
    match key {
        "type" => d.card_type.slug(),
        "foil" => d.foil.slug(),
        "shape" => d.shape.slug(),
        "size" => d.size.slug(),
        "badge" => d.badge.slug(),
        "title" => d.title.slug(),
        _ => "",
    }
}

impl Filter {
    /// Lit les paires `clé=valeur` d'une URL (clés répétées). `strict` : une valeur inconnue est
    /// une erreur (API) ; sinon elle est ignorée (galerie).
    pub fn from_pairs(
        pairs: &[(String, String)],
        strict: bool,
    ) -> Result<Self, dcb_core::UnknownVariant> {
        let mut f = Self::default();
        fn push<T: std::str::FromStr<Err = dcb_core::UnknownVariant> + PartialEq>(
            list: &mut Vec<T>,
            v: &str,
            strict: bool,
        ) -> Result<(), dcb_core::UnknownVariant> {
            match v.parse() {
                Ok(x) => {
                    if !list.contains(&x) {
                        list.push(x);
                    }
                    Ok(())
                }
                Err(e) if strict => Err(e),
                Err(_) => Ok(()),
            }
        }
        for (k, v) in pairs {
            let v = v.trim();
            if v.is_empty() {
                continue;
            }
            match k.as_str() {
                "type" => push(&mut f.card_types, v, strict)?,
                "foil" => push(&mut f.foils, v, strict)?,
                "shape" => push(&mut f.shapes, v, strict)?,
                "size" => push(&mut f.sizes, v, strict)?,
                "badge" => push(&mut f.badges, v, strict)?,
                "title" => push(&mut f.titles, v, strict)?,
                _ => {}
            }
        }
        Ok(f)
    }

    /// Valeurs choisies pour un réglage, en slugs.
    pub fn values(&self, key: &str) -> Vec<&'static str> {
        match key {
            "type" => self.card_types.iter().map(|v| v.slug()).collect(),
            "foil" => self.foils.iter().map(|v| v.slug()).collect(),
            "shape" => self.shapes.iter().map(|v| v.slug()).collect(),
            "size" => self.sizes.iter().map(|v| v.slug()).collect(),
            "badge" => self.badges.iter().map(|v| v.slug()).collect(),
            "title" => self.titles.iter().map(|v| v.slug()).collect(),
            _ => Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        FILTER_KEYS.iter().all(|k| self.values(k).is_empty())
    }

    /// La carte passe le filtre, en ignorant éventuellement un réglage (compteurs de la galerie).
    pub fn matches_except(&self, d: &CardDraft, skip: Option<&str>) -> bool {
        FILTER_KEYS.iter().all(|&k| {
            if skip == Some(k) {
                return true;
            }
            let vals = self.values(k);
            vals.is_empty() || vals.contains(&field(d, k))
        })
    }

    pub fn matches(&self, d: &CardDraft) -> bool {
        self.matches_except(d, None)
    }

    /// Paires `clé=valeur` du filtre, dans l'ordre des réglages.
    pub fn pairs(&self) -> Vec<(&'static str, &'static str)> {
        FILTER_KEYS
            .iter()
            .flat_map(|&k| self.values(k).into_iter().map(move |v| (k, v)))
            .collect()
    }
}

pub async fn list(pool: &SqlitePool, filter: Filter) -> sqlx::Result<Vec<Card>> {
    // La collection est petite : on lit tout et on filtre ici (sqlx 0.9 n'accepte que des
    // requêtes `&'static str`, impossible d'y composer un IN de longueur variable).
    const SQL: &str = concat!(
        "SELECT ",
        columns!(),
        " FROM cards ORDER BY card_type, name COLLATE NOCASE"
    );
    let cards = sqlx::query_as::<_, CardRow>(SQL)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(Card::try_from)
        .collect::<sqlx::Result<Vec<Card>>>()?;
    Ok(cards
        .into_iter()
        .filter(|c| filter.matches(&c.draft))
        .collect())
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
        "INSERT INTO cards (slug, name, card_type, skin, foil, shape, size, badge, title, \
                            description, effect, deploy_cost, daily_cost, flavor_text, art) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT (slug) DO UPDATE SET \
            name = excluded.name, card_type = excluded.card_type, skin = excluded.skin, \
            foil = excluded.foil, shape = excluded.shape, size = excluded.size, \
            badge = excluded.badge, \
            title = excluded.title, \
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
    .bind(d.badge.slug())
    .bind(d.title.slug())
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
