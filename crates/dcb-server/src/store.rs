//! Source de vérité des cartes : un fichier TOML par carte dans `cards/`, les illustrations
//! dans `cards/art/`. Ces fichiers sont faits pour être commités. SQLite n'est qu'un cache
//! reconstruit à partir d'eux (voir `db::replace_all`).

use std::path::{Path, PathBuf};

use dcb_core::Background;
use dcb_core::CardVersion;
use dcb_core::DescStyle;
use dcb_core::EffectStyle;
use dcb_core::Refs;
use dcb_core::Relief;
use dcb_core::{
    Badge, Card, CardDraft, CardType, Foil, Shape, Size, Skin, Title, is_valid_slug, slugify,
};
use serde::{Deserialize, Serialize};

/// Contenu d'un fichier `cards/<slug>.toml`. Le slug n'y figure pas : c'est le nom du fichier.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CardFile {
    name: String,
    #[serde(rename = "type")]
    card_type: CardType,
    #[serde(default = "default_skin")]
    skin: Skin,
    /// Finition (et rareté), absente du fichier quand c'est `basic`. Lue en texte pour
    /// accepter l'ancienne valeur `none`, voir `Foil::from_legacy`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    foil: Option<String>,
    /// Ancien champ, remplacé par `foil` : `rarity = "gold"` donne `foil = "gold"`.
    /// Lu pour la compatibilité, jamais réécrit.
    #[serde(default, skip_serializing)]
    rarity: Option<String>,
    /// Style du contour, absent du fichier quand c'est `solid`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shape: Option<String>,
    /// Taille de la carte, absente du fichier quand c'est `normal`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    size: Option<String>,
    /// Emplacement du type, absent du fichier quand c'est `text`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    badge: Option<String>,
    /// Présentation de la description, absente du fichier quand c'est `classic`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    desc_style: Option<String>,
    /// Relief des blocs, absent du fichier quand c'est `flat`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    relief: Option<String>,
    /// Fond de la carte, absent du fichier quand c'est `plain`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    background: Option<String>,
    /// Version de la carte, absente du fichier quand c'est `v1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    /// Bloc Effect façon code, absent du fichier quand c'est `text`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    effect_style: Option<String>,
    /// Nom de fichier du bloc Effect façon code, absent quand il est vide.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    effect_file: Option<String>,
    /// Cartes citées dans l'effet, absent du fichier quand c'est `plain`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refs: Option<String>,
    /// Emplacement du nom, absent du fichier quand c'est `block`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(default)]
    deploy_cost: i64,
    #[serde(default)]
    daily_cost: i64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    effect: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    flavor_text: String,
    /// Nom du fichier dans `cards/art/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    art: Option<String>,
}

fn default_skin() -> Skin {
    Skin::Light
}

impl From<&CardDraft> for CardFile {
    fn from(d: &CardDraft) -> Self {
        Self {
            name: d.name.clone(),
            card_type: d.card_type,
            skin: d.skin,
            foil: (d.foil != Foil::Basic).then(|| d.foil.slug().to_owned()),
            rarity: None,
            shape: (d.shape != Shape::Solid).then(|| d.shape.slug().to_owned()),
            size: (d.size != Size::Normal).then(|| d.size.slug().to_owned()),
            badge: (d.badge != Badge::Text).then(|| d.badge.slug().to_owned()),
            desc_style: (d.desc_style != DescStyle::Classic)
                .then(|| d.desc_style.slug().to_owned()),
            relief: (d.relief != Relief::Flat).then(|| d.relief.slug().to_owned()),
            background: (d.background != Background::Plain).then(|| d.background.slug().to_owned()),
            version: (d.version != CardVersion::V1).then(|| d.version.slug().to_owned()),
            effect_style: (d.effect_style != EffectStyle::Text)
                .then(|| d.effect_style.slug().to_owned()),
            effect_file: (!d.effect_file.is_empty()).then(|| d.effect_file.clone()),
            refs: (d.refs != Refs::Plain).then(|| d.refs.slug().to_owned()),
            title: (d.title != Title::Block).then(|| d.title.slug().to_owned()),
            deploy_cost: d.deploy_cost,
            daily_cost: d.daily_cost,
            description: d.description.clone(),
            effect: d.effect.clone(),
            flavor_text: d.flavor_text.clone(),
            art: d.art.clone(),
        }
    }
}

impl TryFrom<CardFile> for CardDraft {
    type Error = String;

    fn try_from(f: CardFile) -> Result<Self, Self::Error> {
        let foil = Foil::from_legacy(f.foil.as_deref(), f.rarity.as_deref()).map_err(|e| {
            let valid: Vec<_> = Foil::ALL.iter().map(|f| f.slug()).collect();
            format!(
                "foil {:?} inconnu (valeurs : {})",
                e.value,
                valid.join(", ")
            )
        })?;
        let shape = match f.shape.as_deref().map(str::trim) {
            None | Some("") => Shape::Solid,
            Some(v) => v.parse().map_err(|e: dcb_core::UnknownVariant| {
                let valid: Vec<_> = Shape::ALL.iter().map(|s| s.slug()).collect();
                format!(
                    "shape {:?} inconnu (valeurs : {})",
                    e.value,
                    valid.join(", ")
                )
            })?,
        };
        let size = match f.size.as_deref().map(str::trim) {
            None | Some("") => Size::Normal,
            Some(v) => v.parse().map_err(|e: dcb_core::UnknownVariant| {
                let valid: Vec<_> = Size::ALL.iter().map(|s| s.slug()).collect();
                format!(
                    "size {:?} inconnu (valeurs : {})",
                    e.value,
                    valid.join(", ")
                )
            })?,
        };
        let badge = match f.badge.as_deref().map(str::trim) {
            None | Some("") => Badge::Text,
            Some(v) => v.parse().map_err(|e: dcb_core::UnknownVariant| {
                let valid: Vec<_> = Badge::ALL.iter().map(|b| b.slug()).collect();
                format!(
                    "badge {:?} inconnu (valeurs : {})",
                    e.value,
                    valid.join(", ")
                )
            })?,
        };
        let title = match f.title.as_deref().map(str::trim) {
            None | Some("") => Title::Block,
            Some(v) => v.parse().map_err(|e: dcb_core::UnknownVariant| {
                let valid: Vec<_> = Title::ALL.iter().map(|t| t.slug()).collect();
                format!(
                    "title {:?} inconnu (valeurs : {})",
                    e.value,
                    valid.join(", ")
                )
            })?,
        };
        let refs = match f.refs.as_deref().map(str::trim) {
            None | Some("") => Refs::Plain,
            Some(v) => v.parse().map_err(|e: dcb_core::UnknownVariant| {
                let valid: Vec<_> = Refs::ALL.iter().map(|t| t.slug()).collect();
                format!(
                    "refs {:?} inconnu (valeurs : {})",
                    e.value,
                    valid.join(", ")
                )
            })?,
        };
        let effect_style = match f.effect_style.as_deref().map(str::trim) {
            None | Some("") => EffectStyle::Text,
            Some(v) => v.parse().map_err(|e: dcb_core::UnknownVariant| {
                let valid: Vec<_> = EffectStyle::ALL.iter().map(|t| t.slug()).collect();
                format!(
                    "effect_style {:?} inconnu (valeurs : {})",
                    e.value,
                    valid.join(", ")
                )
            })?,
        };
        let version = match f.version.as_deref().map(str::trim) {
            None | Some("") => CardVersion::V1,
            Some(v) => v.parse().map_err(|e: dcb_core::UnknownVariant| {
                let valid: Vec<_> = CardVersion::ALL.iter().map(|t| t.slug()).collect();
                format!(
                    "version {:?} inconnue (valeurs : {})",
                    e.value,
                    valid.join(", ")
                )
            })?,
        };
        let background = match f.background.as_deref().map(str::trim) {
            None | Some("") => Background::Plain,
            Some(v) => v.parse().map_err(|e: dcb_core::UnknownVariant| {
                let valid: Vec<_> = Background::ALL.iter().map(|t| t.slug()).collect();
                format!(
                    "background {:?} inconnu (valeurs : {})",
                    e.value,
                    valid.join(", ")
                )
            })?,
        };
        let relief = match f.relief.as_deref().map(str::trim) {
            None | Some("") => Relief::Flat,
            Some(v) => v.parse().map_err(|e: dcb_core::UnknownVariant| {
                let valid: Vec<_> = Relief::ALL.iter().map(|t| t.slug()).collect();
                format!(
                    "relief {:?} inconnu (valeurs : {})",
                    e.value,
                    valid.join(", ")
                )
            })?,
        };
        let desc_style = match f.desc_style.as_deref().map(str::trim) {
            None | Some("") => DescStyle::Classic,
            Some(v) => v.parse().map_err(|e: dcb_core::UnknownVariant| {
                let valid: Vec<_> = DescStyle::ALL.iter().map(|t| t.slug()).collect();
                format!(
                    "desc_style {:?} inconnu (valeurs : {})",
                    e.value,
                    valid.join(", ")
                )
            })?,
        };
        Ok(Self {
            name: f.name,
            card_type: f.card_type,
            skin: f.skin,
            foil,
            shape,
            size,
            badge,
            desc_style,
            relief,
            background,
            version,
            effect_style,
            effect_file: f.effect_file.unwrap_or_default(),
            refs,
            title,
            description: f.description,
            effect: f.effect,
            deploy_cost: f.deploy_cost,
            daily_cost: f.daily_cost,
            flavor_text: f.flavor_text,
            art: f.art,
        })
    }
}

/// Fichier ignoré au chargement, avec la raison. Affiché dans la galerie.
#[derive(Debug, Clone)]
pub struct LoadError {
    pub file: String,
    pub message: String,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.file, self.message)
    }
}

/// Nom de fichier d'illustration acceptable : pas de dossier, pas de `..`.
/// Contenu du fichier `cards/<slug>.toml` d'une carte, tel que l'éditeur l'écrit.
pub fn card_toml(d: &CardDraft) -> String {
    toml::to_string_pretty(&CardFile::from(d)).unwrap_or_default()
}

pub fn is_safe_file_name(f: &str) -> bool {
    !f.is_empty()
        && f.len() <= 80
        && !f.starts_with('.')
        && !f.contains("..")
        && f.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

pub struct CardStore {
    dir: PathBuf,
    /// Sérialise les écritures (choix du slug, fichiers) entre requêtes concurrentes.
    write_lock: tokio::sync::Mutex<()>,
}

impl CardStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            write_lock: tokio::sync::Mutex::new(()),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn art_dir(&self) -> PathBuf {
        self.dir.join("art")
    }

    fn card_path(&self, slug: &str) -> PathBuf {
        self.dir.join(format!("{slug}.toml"))
    }

    /// Garde à prendre pendant une modification (création, mise à jour, suppression).
    pub async fn lock(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.write_lock.lock().await
    }

    /// Lit toutes les cartes. Un fichier invalide est ignoré et signalé, il ne bloque pas les autres.
    pub async fn load_all(&self) -> std::io::Result<(Vec<Card>, Vec<LoadError>)> {
        tokio::fs::create_dir_all(&self.dir).await?;
        let mut entries = tokio::fs::read_dir(&self.dir).await?;
        let mut paths = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "toml") && path.is_file() {
                paths.push(path);
            }
        }
        paths.sort();

        let mut cards = Vec::new();
        let mut errors = Vec::new();
        for path in paths {
            let file = path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            match self.read_card(&path).await {
                Ok((card, warning)) => {
                    if let Some(message) = warning {
                        errors.push(LoadError {
                            file: file.clone(),
                            message,
                        });
                    }
                    cards.push(card);
                }
                Err(message) => errors.push(LoadError { file, message }),
            }
        }
        Ok((cards, errors))
    }

    /// Lit une carte. Renvoie aussi un avertissement non bloquant (illustration introuvable).
    async fn read_card(&self, path: &Path) -> Result<(Card, Option<String>), String> {
        let slug = path
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| is_valid_slug(s))
            .ok_or("nom de fichier invalide (attendu : minuscules, chiffres et tirets)")?
            .to_owned();
        let text = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| e.to_string())?;
        let file: CardFile = toml::from_str(&text).map_err(|e| e.message().to_owned())?;
        let mut draft = CardDraft::try_from(file)?.validated().map_err(|errs| {
            errs.iter()
                .map(|e| format!("{} {}", e.field, e.message))
                .collect::<Vec<_>>()
                .join(", ")
        })?;

        let mut warning = None;
        if let Some(art) = &draft.art {
            if !is_safe_file_name(art) {
                warning = Some(format!("illustration {art:?} ignorée (nom invalide)"));
                draft.art = None;
            } else if !self.art_dir().join(art).is_file() {
                warning = Some(format!("illustration art/{art} introuvable"));
                draft.art = None;
            }
        }
        Ok((Card { slug, draft }, warning))
    }

    /// Slug libre dérivé du nom : `argo-cd`, puis `argo-cd-2`, `argo-cd-3`...
    /// À appeler en tenant le verrou d'écriture.
    pub fn unique_slug(&self, name: &str) -> String {
        let base = slugify(name);
        if !self.card_path(&base).exists() {
            return base;
        }
        (2..)
            .map(|n| {
                let suffix = format!("-{n}");
                let mut b = base.clone();
                b.truncate(dcb_core::SLUG_MAX - suffix.len());
                format!("{}{suffix}", b.trim_end_matches('-'))
            })
            .find(|s| !self.card_path(s).exists())
            .expect("suite infinie")
    }

    /// Écrit `cards/<slug>.toml` de façon atomique (fichier temporaire puis renommage), pour ne
    /// jamais laisser un fichier à moitié écrit.
    pub async fn write(&self, card: &Card) -> anyhow::Result<()> {
        anyhow::ensure!(is_valid_slug(&card.slug), "slug invalide: {}", card.slug);
        tokio::fs::create_dir_all(&self.dir).await?;
        let body = toml::to_string_pretty(&CardFile::from(&card.draft))?;
        let tmp = self.dir.join(format!(".{}.toml.tmp", card.slug));
        tokio::fs::write(&tmp, body).await?;
        tokio::fs::rename(&tmp, self.card_path(&card.slug)).await?;
        Ok(())
    }

    /// Supprime le fichier de la carte. Renvoie `false` s'il n'existait pas.
    pub async fn remove(&self, slug: &str) -> anyhow::Result<bool> {
        anyhow::ensure!(is_valid_slug(slug), "slug invalide: {slug}");
        match tokio::fs::remove_file(self.card_path(slug)).await {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// Enregistre l'illustration sous `art/<slug>.<ext>` et renvoie le nom du fichier.
    pub async fn save_art(&self, slug: &str, ext: &str, bytes: &[u8]) -> anyhow::Result<String> {
        anyhow::ensure!(is_valid_slug(slug), "slug invalide: {slug}");
        let dir = self.art_dir();
        tokio::fs::create_dir_all(&dir).await?;
        let file = format!("{slug}.{ext}");
        let tmp = dir.join(format!(".{file}.tmp"));
        tokio::fs::write(&tmp, bytes).await?;
        tokio::fs::rename(&tmp, dir.join(&file)).await?;
        Ok(file)
    }

    /// Supprime une illustration. Une erreur est seulement journalisée : ce n'est qu'un fichier
    /// orphelin, la carte reste cohérente.
    pub async fn remove_art(&self, file: &str) {
        if !is_safe_file_name(file) {
            return;
        }
        if let Err(e) = tokio::fs::remove_file(self.art_dir().join(file)).await
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!("suppression de art/{file} impossible: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> CardDraft {
        CardDraft {
            name: "ARGO CD".into(),
            card_type: CardType::App,
            skin: Skin::Light,
            foil: Foil::Shards,
            shape: Shape::Neon,
            size: Size::Compact,
            badge: Badge::Image,
            desc_style: DescStyle::Comment,
            relief: Relief::Soft,
            background: Background::Tinted,
            version: CardVersion::V2,
            effect_style: EffectStyle::Code,
            effect_file: "deploy.sh".into(),
            refs: Refs::Linked,
            title: Title::Image,
            description: "GitOps".into(),
            effect: "- Ajoute +1 deployment MAX\n- Multi environment".into(),
            deploy_cost: 3,
            daily_cost: 1,
            flavor_text: "Pas de Flux dans ce jeu !".into(),
            art: None,
        }
    }

    fn temp_store() -> (CardStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("dcb-store-{}", uuid::Uuid::new_v4()));
        (CardStore::new(&dir), dir)
    }

    #[tokio::test]
    async fn roundtrip_and_readable_toml() {
        let (store, dir) = temp_store();
        let card = Card {
            slug: "argo-cd".into(),
            draft: draft(),
        };
        store.write(&card).await.unwrap();

        let text = std::fs::read_to_string(dir.join("argo-cd.toml")).unwrap();
        assert!(text.contains("name = \"ARGO CD\""), "{text}");
        assert!(text.contains("type = \"app\""), "{text}");
        assert!(!text.contains("art"), "{text}");
        assert!(text.contains("foil = \"shards\""), "{text}");
        assert!(text.contains("shape = \"neon\""), "{text}");
        assert!(text.contains("size = \"compact\""), "{text}");
        assert!(text.contains("badge = \"image\""), "{text}");
        assert!(text.contains("desc_style = \"comment\""), "{text}");
        assert!(text.contains("relief = \"soft\""), "{text}");
        assert!(text.contains("background = \"tinted\""), "{text}");
        assert!(text.contains("version = \"v2\""), "{text}");
        assert!(text.contains("effect_style = \"code\""), "{text}");
        assert!(text.contains("effect_file = \"deploy.sh\""), "{text}");
        assert!(text.contains("refs = \"linked\""), "{text}");
        assert!(text.contains("title = \"image\""), "{text}");

        let (cards, errors) = store.load_all().await.unwrap();
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(cards, vec![card]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn hand_written_file_with_defaults() {
        let (store, dir) = temp_store();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("ddos.toml"),
            "name = \"DDOS\"\ntype = \"incident\"\neffect = \"\"\"\n- Inflige 5 dégâts\n- Ralentit de 70%\n\"\"\"\n",
        )
        .unwrap();
        let (cards, errors) = store.load_all().await.unwrap();
        assert!(errors.is_empty(), "{errors:?}");
        let d = &cards[0].draft;
        assert_eq!(cards[0].slug, "ddos");
        assert_eq!(d.skin, Skin::Light);
        assert_eq!(d.foil, Foil::Basic);
        assert_eq!(d.effect, "- Inflige 5 dégâts\n- Ralentit de 70%");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn legacy_rarity_files_still_load() {
        let (store, dir) = temp_store();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("a.toml"),
            "name = \"A\"\ntype = \"app\"\nrarity = \"gold\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("b.toml"),
            "name = \"B\"\ntype = \"app\"\nrarity = \"basic\"\nfoil = \"none\"\n",
        )
        .unwrap();
        let (cards, errors) = store.load_all().await.unwrap();
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(cards[0].draft.foil, Foil::Gold);
        assert_eq!(cards[1].draft.foil, Foil::Basic);

        // réécrit au nouveau format : plus de `rarity`, et rien pour `basic`
        store.write(&cards[0]).await.unwrap();
        store.write(&cards[1]).await.unwrap();
        let a = std::fs::read_to_string(dir.join("a.toml")).unwrap();
        let b = std::fs::read_to_string(dir.join("b.toml")).unwrap();
        assert!(
            a.contains("foil = \"gold\"") && !a.contains("rarity"),
            "{a}"
        );
        assert!(!b.contains("foil") && !b.contains("rarity"), "{b}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn invalid_files_are_reported_not_fatal() {
        let (store, dir) = temp_store();
        store
            .write(&Card {
                slug: "ok".into(),
                draft: draft(),
            })
            .await
            .unwrap();
        std::fs::write(
            dir.join("broken.toml"),
            "name = \"X\"\ntype = \"package\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("typo.toml"),
            "name = \"X\"\ntype = \"app\"\ncolour = 1\n",
        )
        .unwrap();
        std::fs::write(dir.join("Bad Name.toml"), "name = \"X\"\ntype = \"app\"\n").unwrap();
        std::fs::write(
            dir.join("noart.toml"),
            "name = \"X\"\ntype = \"app\"\nart = \"missing.png\"\n",
        )
        .unwrap();

        let (cards, errors) = store.load_all().await.unwrap();
        let slugs: Vec<_> = cards.iter().map(|c| c.slug.as_str()).collect();
        assert_eq!(slugs, ["noart", "ok"]);
        assert_eq!(cards[0].draft.art, None);
        let files: Vec<_> = errors.iter().map(|e| e.file.as_str()).collect();
        assert_eq!(
            files,
            ["Bad Name.toml", "broken.toml", "noart.toml", "typo.toml"]
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn unique_slug_appends_suffix() {
        let (store, dir) = temp_store();
        assert_eq!(store.unique_slug("Argo CD"), "argo-cd");
        for slug in ["argo-cd", "argo-cd-2"] {
            store
                .write(&Card {
                    slug: slug.into(),
                    draft: draft(),
                })
                .await
                .unwrap();
        }
        assert_eq!(store.unique_slug("ARGO  cd"), "argo-cd-3");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn art_names() {
        assert!(is_safe_file_name("argo-cd.png"));
        assert!(!is_safe_file_name("../x.png"));
        assert!(!is_safe_file_name("a/b.png"));
        assert!(!is_safe_file_name(".hidden"));
    }
}
