//! Contenu de la page /regles, lu depuis `rules/regles.toml` (versionné, comme les cartes).
//!
//! Mise en forme dans les textes :
//! - `***terme***` affiche le terme en gras italique (termes de statut : to-do, pending…) ;
//! - les montants en dollars (`$25.000,00`) sont mis en évidence automatiquement.

use std::path::Path;

use dcb_core::CardType;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    #[serde(default)]
    pub draft_notice: String,
    pub title: String,
    pub context: Vec<String>,
    #[serde(default)]
    pub facts: Vec<Fact>,
    pub start: Start,
    pub victory: Victory,
    pub types: Types,
    pub day: Day,
    pub lifecycle: Lifecycle,
    pub rarity: Rarity,
    pub anatomy: Anatomy,
    pub concepts: Concepts,
    /// Understand the Basics : glossaire.
    #[serde(default)]
    pub glossary: Option<Terms>,
    /// Core Concepts : économie, incidents, interactions, plateau.
    #[serde(default)]
    pub economy: Option<TextPart>,
    #[serde(default)]
    pub incidents: Option<TextPart>,
    #[serde(default)]
    pub interactions: Option<TextPart>,
    #[serde(default)]
    pub board: Option<Board>,
    /// Getting Started : première partie, deck, création de carte, autres réglages.
    #[serde(default)]
    pub first_game: Option<Steps>,
    #[serde(default)]
    pub deck: Option<TextPart>,
    #[serde(default)]
    pub create_card: Option<Steps>,
    #[serde(default)]
    pub settings: Option<Terms>,
    /// Patch notes (`rules/releases.toml`), de la plus récente à la plus ancienne.
    #[serde(skip)]
    pub releases: Vec<Release>,
    /// Ce qui est prévu (`rules/roadmap.toml`).
    #[serde(skip)]
    pub roadmap: Vec<RoadItem>,
}

/// Section de texte simple ; `draft` affiche la pastille « Brouillon ».
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextPart {
    pub eyebrow: String,
    pub title: String,
    #[serde(default)]
    pub draft: bool,
    pub text: Vec<String>,
    /// Étapes d'un flux, de gauche à droite (entrée, milieu, sortie).
    #[serde(default)]
    pub flow: Vec<String>,
}

/// Liste de termes et de définitions (glossaire, réglages).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terms {
    pub eyebrow: String,
    pub title: String,
    #[serde(default)]
    pub draft: bool,
    pub items: Vec<Term>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Term {
    pub term: String,
    pub text: String,
}

/// Étapes numérotées (première partie, créer une carte).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Steps {
    pub eyebrow: String,
    pub title: String,
    #[serde(default)]
    pub draft: bool,
    pub items: Vec<Term>,
}

/// Schéma du plateau : `key` place la zone (voir `BOARD_ZONES`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Board {
    pub eyebrow: String,
    pub title: String,
    #[serde(default)]
    pub draft: bool,
    pub text: String,
    pub zones: Vec<BoardZone>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardZone {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub note: String,
}

/// Zones du schéma du plateau, placées par `app.css` (`.z-<clé>`).
pub const BOARD_ZONES: &[&str] = &[
    "opp", "mid", "cluster", "apps", "kanban", "team", "hand", "deck", "discard", "cash",
];

/// Une version du site : ce qui est nouveau, ce qui a changé, ce qui est corrigé.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub version: String,
    /// AAAA-MM-JJ
    pub date: String,
    pub title: String,
    #[serde(default)]
    pub new: Vec<String>,
    #[serde(default)]
    pub changed: Vec<String>,
    #[serde(default)]
    pub fixed: Vec<String>,
}

/// Une entrée de la roadmap ; `release` relie une entrée faite à sa version.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoadItem {
    pub status: String,
    pub title: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub release: String,
}

/// Colonnes de la roadmap : clé de `status` et titre.
pub const ROAD_STATUSES: &[(&str, &str)] = &[
    ("study", "À l'étude"),
    ("planned", "Prévu"),
    ("doing", "En cours"),
    ("done", "Fait"),
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleasesFile {
    #[serde(default)]
    releases: Vec<Release>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RoadmapFile {
    #[serde(default)]
    items: Vec<RoadItem>,
}

/// `2026-10-21` → `21 octobre 2026`.
pub fn french_date(iso: &str) -> Option<String> {
    const MONTHS: [&str; 12] = [
        "janvier",
        "février",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "août",
        "septembre",
        "octobre",
        "novembre",
        "décembre",
    ];
    let mut it = iso.split('-');
    let (y, m, d) = (it.next()?, it.next()?, it.next()?);
    if it.next().is_some() || y.len() != 4 {
        return None;
    }
    let (y, m, d): (u32, usize, u32) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(format!("{d} {} {y}", MONTHS[m - 1]))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fact {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    pub title: String,
    pub text: String,
    /// Annotation, affichée en commentaire sous le texte.
    #[serde(default)]
    pub note: String,
    pub groups: Vec<TileGroup>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TileGroup {
    pub title: String,
    pub items: Vec<Tile>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tile {
    pub label: String,
    /// Type de carte dont la tuile prend la couleur.
    #[serde(rename = "type")]
    pub card_type: CardType,
    /// Noms tapés puis effacés l'un après l'autre à la place du libellé.
    #[serde(default)]
    pub typing: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Victory {
    pub title: String,
    pub text: String,
    /// Annotation, affichée en commentaire sous le texte.
    #[serde(default)]
    pub note: String,
    /// Budget de départ et objectif, en dollars : servent à dessiner la piste à l'échelle.
    pub start_amount: u64,
    pub goal_amount: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Types {
    pub title: String,
    pub items: Vec<TypeEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeEntry {
    #[serde(rename = "type")]
    pub card_type: CardType,
    /// Titre affiché (ex. « RESSOURCES HUMAINES »).
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Day {
    pub title: String,
    pub intro: String,
    pub phases: Vec<Phase>,
    pub points: Points,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Phase {
    pub name: String,
    #[serde(default)]
    pub when: String,
    pub text: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Points {
    pub title: String,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lifecycle {
    pub title: String,
    pub text: String,
    pub features: Vec<String>,
    pub kpis: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rarity {
    pub title: String,
    pub text: String,
}

/// Partie « Anatomie d'une carte » : schéma annoté, fiche de chaque zone, fichier de la carte.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anatomy {
    pub title: String,
    pub text: String,
    pub zones_title: String,
    pub zones_text: String,
    pub file_title: String,
    pub file_text: String,
    pub file_notes: Vec<String>,
    pub looks_title: String,
    pub looks_text: String,
    pub zones: Vec<AnatomyZone>,
}

/// Une zone de la carte : `key` choisit son emplacement sur le schéma (voir `anatomy::ZONES`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnatomyZone {
    pub key: String,
    pub title: String,
    /// Champ du fichier de la carte qui remplit la zone.
    pub field: String,
    pub text: String,
    /// Règle d'affichage, précédée de son intitulé en gras.
    pub rule_label: String,
    pub rule: String,
}

/// Partie « Concepts » : skin, finition (texte de `rarity`) et contour.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Concepts {
    pub title: String,
    pub text: String,
    pub skin: SkinPart,
    pub shape: ShapePart,
}

/// Contours de la partie « Concepts », avec la description de chacun pour la vue zoomée.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShapePart {
    pub title: String,
    pub text: String,
    #[serde(default)]
    pub items: Vec<ShapeItem>,
}

/// Description d'un contour : `key` est la valeur de `shape` (solid, neon…).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShapeItem {
    pub key: String,
    pub text: String,
}

/// Skins de la partie « Concepts » : deux bandes, classiques et animées.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkinPart {
    pub title: String,
    pub text: String,
    pub classic: Vec<SkinItem>,
    pub animated: Vec<SkinItem>,
}

/// Une skin : sa clé (voir `concepts::skin_style`), son nom et sa description, affichée dans la
/// vue zoomée.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkinItem {
    pub key: String,
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConceptPart {
    pub title: String,
    pub text: String,
}

impl Rules {
    pub fn parse(text: &str) -> Result<Self, String> {
        let rules: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        let skin = &rules.concepts.skin;
        for item in skin.classic.iter().chain(&skin.animated) {
            if crate::concepts::skin_style(&item.key).is_none() {
                return Err(format!(
                    "skin « {} » : clé {:?} inconnue (valeurs : {})",
                    item.title,
                    item.key,
                    crate::concepts::SKIN_KEYS.join(", ")
                ));
            }
        }
        for z in &rules.anatomy.zones {
            if crate::anatomy::zone(&z.key).is_none() {
                let valid: Vec<_> = crate::anatomy::ZONES.iter().map(|z| z.key).collect();
                return Err(format!(
                    "anatomie : zone {:?} inconnue (valeurs : {})",
                    z.key,
                    valid.join(", ")
                ));
            }
        }
        for item in &rules.concepts.shape.items {
            if item.key.parse::<dcb_core::Shape>().is_err() {
                let valid: Vec<_> = dcb_core::Shape::ALL.iter().map(|s| s.slug()).collect();
                return Err(format!(
                    "contour : clé {:?} inconnue (valeurs : {})",
                    item.key,
                    valid.join(", ")
                ));
            }
        }
        if let Some(b) = &rules.board {
            for z in &b.zones {
                if !BOARD_ZONES.contains(&z.key.as_str()) {
                    return Err(format!(
                        "plateau : zone {:?} inconnue (valeurs : {})",
                        z.key,
                        BOARD_ZONES.join(", ")
                    ));
                }
            }
        }
        Ok(rules)
    }

    /// Patch notes et roadmap, lus à part : chaque version vérifiée (date), chaque entrée de
    /// roadmap reliée à un statut connu et, si elle en cite une, à une version existante.
    pub fn with_notes(mut self, releases: &str, roadmap: &str) -> Result<Self, String> {
        let rel: ReleasesFile =
            toml::from_str(releases).map_err(|e| format!("releases.toml: {e}"))?;
        for r in &rel.releases {
            if french_date(&r.date).is_none() {
                return Err(format!(
                    "releases.toml : date {:?} de {} invalide (AAAA-MM-JJ)",
                    r.date, r.version
                ));
            }
        }
        let road: RoadmapFile =
            toml::from_str(roadmap).map_err(|e| format!("roadmap.toml: {e}"))?;
        for i in &road.items {
            if !ROAD_STATUSES.iter().any(|(k, _)| *k == i.status) {
                let valid: Vec<_> = ROAD_STATUSES.iter().map(|(k, _)| *k).collect();
                return Err(format!(
                    "roadmap.toml : statut {:?} de « {} » inconnu (valeurs : {})",
                    i.status,
                    i.title,
                    valid.join(", ")
                ));
            }
            if !i.release.is_empty() && !rel.releases.iter().any(|r| r.version == i.release) {
                return Err(format!(
                    "roadmap.toml : « {} » cite la version {:?}, absente de releases.toml",
                    i.title, i.release
                ));
            }
        }
        self.releases = rel.releases;
        self.roadmap = road.items;
        Ok(self)
    }

    /// Lit `regles.toml`, et `releases.toml` et `roadmap.toml` à côté s'ils existent.
    pub async fn load(path: &Path) -> Result<Self, String> {
        let text = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let rules = Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let side = |name: &str| {
            let p = path.with_file_name(name);
            async move { tokio::fs::read_to_string(&p).await.unwrap_or_default() }
        };
        let (releases, roadmap) = (side("releases.toml").await, side("roadmap.toml").await);
        rules.with_notes(&releases, &roadmap)
    }

    /// Part du budget de départ dans l'objectif, en pourcentage (pour la piste).
    pub fn start_percent(&self) -> f64 {
        if self.victory.goal_amount == 0 {
            return 0.0;
        }
        (self.victory.start_amount as f64 / self.victory.goal_amount as f64 * 100.0).min(100.0)
    }
}

/// Formate un montant comme dans le jeu : `$25.000,00`.
pub fn money(amount: u64) -> String {
    let digits = amount.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(c);
    }
    format!("${grouped},00")
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Entoure les montants (`$` suivi de chiffres, points et virgules) d'un `<span class="money">`.
/// Le texte reçu est déjà échappé.
fn highlight_money(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let starts_amount = c == '$' && s[i + 1..].starts_with(|n: char| n.is_ascii_digit());
        if !starts_amount {
            out.push(c);
            continue;
        }
        let mut end = i + 1;
        while let Some(&(j, n)) = chars.peek() {
            if n.is_ascii_digit()
                || ((n == '.' || n == ',') && s[j + 1..].starts_with(|d: char| d.is_ascii_digit()))
            {
                end = j + n.len_utf8();
                chars.next();
            } else {
                break;
            }
        }
        out.push_str("<span class=\"money\">");
        out.push_str(&s[i..end]);
        out.push_str("</span>");
    }
    out
}

/// Convertit un texte des règles en HTML sûr : échappement, `***terme***`, montants.
pub fn inline(text: &str) -> String {
    let escaped = escape(text);
    let mut out = String::with_capacity(escaped.len());
    let mut rest = escaped.as_str();
    while let Some(start) = rest.find("***") {
        let after = &rest[start + 3..];
        match after.find("***") {
            Some(end) if end > 0 => {
                out.push_str(&rest[..start]);
                out.push_str("<strong><em>");
                out.push_str(&after[..end]);
                out.push_str("</em></strong>");
                rest = &after[end + 3..];
            }
            _ => break,
        }
    }
    out.push_str(rest);
    highlight_money(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_money_like_the_game() {
        assert_eq!(money(25_000), "$25.000,00");
        assert_eq!(money(1_000_000), "$1.000.000,00");
        assert_eq!(money(5), "$5,00");
    }

    #[test]
    fn inline_formatting_is_safe() {
        assert_eq!(
            inline("tâches en ***to-do*** et ***done***"),
            "tâches en <strong><em>to-do</em></strong> et <strong><em>done</em></strong>"
        );
        assert_eq!(
            inline("investit $25.000,00. Bravo"),
            "investit <span class=\"money\">$25.000,00</span>. Bravo"
        );
        assert_eq!(
            inline("<script> & \"x\""),
            "&lt;script&gt; &amp; &quot;x&quot;"
        );
        assert_eq!(inline("prix: $ et ***"), "prix: $ et ***");
    }

    #[test]
    fn shipped_rules_file_is_valid() {
        let text = include_str!("../../../rules/regles.toml");
        let rules = Rules::parse(text).expect("rules/regles.toml invalide");
        assert_eq!(rules.types.items.len(), CardType::ALL.len());
        for t in CardType::ALL {
            assert!(
                rules.types.items.iter().any(|e| e.card_type == *t),
                "type {t} absent des règles"
            );
        }
        assert!((rules.start_percent() - 2.5).abs() < 1e-9);
        assert!(rules.glossary.is_some() && rules.board.is_some());
        let rules = rules
            .with_notes(
                include_str!("../../../rules/releases.toml"),
                include_str!("../../../rules/roadmap.toml"),
            )
            .expect("releases.toml ou roadmap.toml invalide");
        assert!(!rules.releases.is_empty() && !rules.roadmap.is_empty());
    }

    #[test]
    fn notes_are_checked() {
        let r = Rules::parse(include_str!("../../../rules/regles.toml")).unwrap();
        let rel = "[[releases]]\nversion = \"v1\"\ndate = \"2026-10-22\"\ntitle = \"T\"\n";
        assert!(
            r.clone()
                .with_notes(
                    rel,
                    "[[items]]\nstatus = \"done\"\ntitle = \"x\"\nrelease = \"v1\"\n"
                )
                .is_ok()
        );
        assert!(
            r.clone()
                .with_notes(rel, "[[items]]\nstatus = \"later\"\ntitle = \"x\"\n")
                .is_err()
        );
        assert!(
            r.clone()
                .with_notes(
                    rel,
                    "[[items]]\nstatus = \"done\"\ntitle = \"x\"\nrelease = \"v9\"\n"
                )
                .is_err()
        );
        assert!(
            r.with_notes(&rel.replace("2026-10-22", "22/10"), "")
                .is_err()
        );
        assert_eq!(
            french_date("2026-10-21").as_deref(),
            Some("21 octobre 2026")
        );
    }
}
