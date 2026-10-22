//! Modèle de domaine de DevCardBattle, partagé entre le serveur et les futurs clients
//! (moteur de duel, client Bevy/WASM).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Limites de saisie, calées sur la place disponible dans le gabarit Figma (374×574).
pub mod limits {
    pub const NAME: usize = 24;
    /// Nom de fichier du bloc Effect façon code.
    pub const EFFECT_FILE: usize = 32;
    pub const DESCRIPTION: usize = 120;
    pub const EFFECT: usize = 220;
    pub const FLAVOR: usize = 60;
    pub const COST_MAX: i64 = 99;
}

/// Déclare un enum stocké en texte (base, formulaires, JSON) avec la liste de ses variantes.
macro_rules! text_enum {
    (
        $(#[$meta:meta])*
        $name:ident { $( $variant:ident => $slug:literal, $label:literal ),+ $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum $name { $( $variant ),+ }

        impl $name {
            pub const ALL: &'static [$name] = &[ $( $name::$variant ),+ ];

            /// Identifiant stable, utilisé en base, dans les URLs et comme classe CSS.
            pub fn slug(self) -> &'static str {
                match self { $( $name::$variant => $slug ),+ }
            }

            /// Libellé affiché.
            pub fn label(self) -> &'static str {
                match self { $( $name::$variant => $label ),+ }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.slug())
            }
        }

        impl FromStr for $name {
            type Err = UnknownVariant;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s.trim().to_ascii_lowercase().as_str() {
                    $( $slug => Ok($name::$variant), )+
                    _ => Err(UnknownVariant { kind: stringify!($name), value: s.to_owned() }),
                }
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("valeur inconnue pour {kind}: {value:?}")]
pub struct UnknownVariant {
    pub kind: &'static str,
    pub value: String,
}

text_enum! {
    /// Type de carte. Détermine la couleur de la bordure et du badge.
    /// Pour ajouter un type : une variante ici + une couleur dans `card.css`.
    CardType {
        Resource => "resource", "RESOURCE",
        Incident => "incident", "INCIDENT",
        Hr       => "hr",       "RESSOURCE HUMAINE",
        App      => "app",      "APP",
        Event    => "event",    "EVENT",
    }
}

text_enum! {
    /// Habillage de la carte (calques `skin/*` du Figma).
    Skin {
        Light => "light", "Light",
        Dark  => "dark",  "Dark",
    }
}

text_enum! {
    /// Finition de la carte, qui fait aussi office de rareté (composants `foilLayer/*` du
    /// Figma). `Basic` n'ajoute rien, les autres posent un overlay animé sur toute la carte.
    Foil {
        Basic   => "basic",   "Basic",
        Gold    => "gold",    "Gold",
        Stars   => "stars",   "Étoiles",
        Glitter => "glitter", "Paillettes",
        Shards  => "shards",  "Éclats",
        Rainbow => "rainbow", "Irisé",
    }
}

text_enum! {
    /// Style du contour aux couleurs du type (calque `cardTypeStroke/*` du Figma).
    /// `Solid` est le trait plein d'origine ; les autres sont des variantes plus modernes.
    Shape {
        Solid    => "solid",    "Plein",
        Neon     => "neon",     "Néon",
        Gradient => "gradient", "Dégradé",
        Sweep    => "sweep",    "Reflet qui tourne",
        Double   => "double",   "Double filet",
        Corners  => "corners",  "Coins de viseur",
        Chamfer  => "chamfer",  "Coins coupés",
        Topbar   => "topbar",   "Barre d'accent",
        Thin     => "thin",     "Filet",
    }
}

text_enum! {
    /// Taille de la carte. `Normal` garde les deux grandes cases de coût ; `Compact` les
    /// ramène à des chips avec icône et donne la place gagnée au bloc Effect.
    Size {
        Normal  => "normal",  "Normale",
        Compact => "compact", "Compacte",
    }
}

// Les enums sont générés par `text_enum!`, qui ne permet pas de marquer une variante `#[default]`.
#[allow(clippy::derivable_impls)]
impl Default for Shape {
    fn default() -> Self {
        Shape::Solid
    }
}

text_enum! {
    /// Emplacement du type sur la carte. `Text` : le nom du type à droite du bloc du nom (Figma) ;
    /// `Image` : une pastille de la couleur du type, avec son icône, posée sur l'illustration, et
    /// le nom prend toute la largeur de son bloc.
    Badge {
        Text  => "text",  "Dans le nom",
        Image => "image", "Sur l'illustration",
    }
}

text_enum! {
    /// Emplacement du nom. `Block` : dans son bloc au-dessus de l'illustration (Figma) ; `Image` :
    /// plus de bloc, le nom est écrit en bas de l'illustration, sur un dégradé sombre, et
    /// l'illustration grandit de 70 px. Le type passe alors en badge.
    Title {
        Block => "block", "Dans son bloc",
        Image => "image", "Sur l'illustration",
    }
}

#[allow(clippy::derivable_impls)]
impl Default for Title {
    fn default() -> Self {
        Title::Block
    }
}

text_enum! {
    /// Présentation de la description. `Classic` : bloc avec son titre (Figma) ; `Untitled` : le
    /// titre disparaît et le texte gagne une ligne ; `Comment` : plus de bloc, la description
    /// devient un commentaire en tête du bloc Effect, qui gagne 70 px de haut.
    DescStyle {
        Classic  => "classic",  "Classique",
        Untitled => "untitled", "Sans titre",
        Comment  => "comment",  "En commentaire",
    }
}

text_enum! {
    /// Cartes citées dans l'effet. `Plain` : texte simple ; `Linked` : le nom d'une autre carte
    /// du jeu prend la couleur de son type et se souligne, et mène à cette carte sur la fiche.
    Refs {
        Plain => "plain", "Texte simple",
        Linked => "linked", "Cartes citées",
    }
}

#[allow(clippy::derivable_impls)]
impl Default for Refs {
    fn default() -> Self {
        Refs::Plain
    }
}

text_enum! {
    /// Relief des blocs de la skin claire. `Flat` : blocs à plat (Figma) ; `Soft` : blocs avec une
    /// ombre douce et une bordure plus légère, sur un fond un peu plus soutenu.
    Relief {
        Flat => "flat", "Plat",
        Soft => "soft", "En relief",
    }
}

text_enum! {
    /// Présentation du bloc Effect. `Text` : titre « Effect » et texte (Figma) ; `Code` : bloc
    /// façon éditeur, avec un nom de fichier (`effect_file`, `effect.yaml` par défaut) et une
    /// ligne YAML par effet.
    EffectStyle {
        Text => "text", "Texte",
        Code => "code", "Code",
    }
}

#[allow(clippy::derivable_impls)]
impl Default for EffectStyle {
    fn default() -> Self {
        EffectStyle::Text
    }
}

text_enum! {
    /// Fond de la carte. `Plain` : la skin seule ; `Tinted` : le fond prend la couleur du type,
    /// sans toucher au contour (qui se choisit à part).
    Background {
        Plain  => "plain",  "Uni",
        Tinted => "tinted", "Teinté",
    }
}

#[allow(clippy::derivable_impls)]
impl Default for Background {
    fn default() -> Self {
        Background::Plain
    }
}

#[allow(clippy::derivable_impls)]
impl Default for Relief {
    fn default() -> Self {
        Relief::Flat
    }
}

#[allow(clippy::derivable_impls)]
impl Default for DescStyle {
    fn default() -> Self {
        DescStyle::Classic
    }
}

#[allow(clippy::derivable_impls)]
impl Default for Badge {
    fn default() -> Self {
        Badge::Text
    }
}

#[allow(clippy::derivable_impls)]
impl Default for Size {
    fn default() -> Self {
        Size::Normal
    }
}

impl Foil {
    /// Lit une valeur de finition en acceptant les anciens formats : `none` (avant que Basic
    /// soit une finition) et l'ancien champ `rarity` (`gold` donnait l'effet doré).
    pub fn from_legacy(foil: Option<&str>, rarity: Option<&str>) -> Result<Self, UnknownVariant> {
        match (foil.map(str::trim), rarity.map(str::trim)) {
            (Some(f), _) if f.eq_ignore_ascii_case("none") => Ok(Foil::Basic),
            (Some(f), _) => f.parse(),
            (None, Some(r)) if r.eq_ignore_ascii_case("gold") => Ok(Foil::Gold),
            (None, _) => Ok(Foil::Basic),
        }
    }
}

impl CardType {
    /// Couleur du type, reprise du Figma (`cardTypeStroke/*`).
    pub fn color(self) -> &'static str {
        match self {
            CardType::Resource => "#38bdf8",
            CardType::Incident => "#f83838",
            CardType::Hr => "#0800f6",
            CardType::App => "#f88838",
            CardType::Event => "#a538f8",
        }
    }
}

/// Contenu d'une carte, sans identifiant. Sert à la création, à l'édition et à l'aperçu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardDraft {
    pub name: String,
    pub card_type: CardType,
    pub skin: Skin,
    pub foil: Foil,
    /// Style du contour ; absent des anciens JSON, d'où la valeur par défaut.
    #[serde(default)]
    pub shape: Shape,
    /// Taille ; absente des anciens JSON, d'où la valeur par défaut.
    #[serde(default)]
    pub size: Size,
    /// Emplacement du type ; absent des anciens JSON, d'où la valeur par défaut.
    #[serde(default)]
    pub badge: Badge,
    /// Emplacement du nom ; absent des anciens JSON, d'où la valeur par défaut.
    #[serde(default)]
    pub title: Title,
    /// Présentation de la description ; absente des anciens JSON, d'où la valeur par défaut.
    #[serde(default)]
    pub desc_style: DescStyle,
    /// Relief des blocs ; absent des anciens JSON, d'où la valeur par défaut.
    #[serde(default)]
    pub relief: Relief,
    /// Fond de la carte ; absent des anciens JSON, d'où la valeur par défaut.
    #[serde(default)]
    pub background: Background,
    /// Présentation du bloc Effect ; absente des anciens JSON, d'où la valeur par défaut.
    #[serde(default)]
    pub effect_style: EffectStyle,
    /// Nom de fichier affiché en tête du bloc Effect façon code (vide : `effect.yaml`).
    #[serde(default)]
    pub effect_file: String,
    /// Cartes citées dans l'effet ; absent des anciens JSON, d'où la valeur par défaut.
    #[serde(default)]
    pub refs: Refs,
    pub description: String,
    pub effect: String,
    pub deploy_cost: i64,
    pub daily_cost: i64,
    pub flavor_text: String,
    /// Nom du fichier d'illustration dans `cards/art/` (ex. `argo-cd.png`), si présente.
    pub art: Option<String>,
}

/// Carte enregistrée. Le slug est son identifiant stable : c'est le nom de son fichier
/// (`cards/<slug>.toml`) et la clé dans les URLs. Il ne change pas si la carte est renommée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub slug: String,
    #[serde(flatten)]
    pub draft: CardDraft,
}

/// Longueur max d'un slug.
pub const SLUG_MAX: usize = 48;

/// Dérive un slug d'un nom de carte : minuscules ASCII, accents retirés, tirets.
/// `"Ressource Humaine : SRE d'astreinte"` donne `"ressource-humaine-sre-d-astreinte"`.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for c in name.chars().flat_map(char::to_lowercase) {
        let folded: &str = match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' => "a",
            'é' | 'è' | 'ê' | 'ë' => "e",
            'î' | 'ï' | 'í' | 'ì' => "i",
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => "o",
            'ù' | 'û' | 'ü' | 'ú' => "u",
            'ç' => "c",
            'ÿ' | 'ý' => "y",
            'ñ' => "n",
            'œ' => "oe",
            'æ' => "ae",
            'ß' => "ss",
            c if c.is_ascii_alphanumeric() => {
                if pending_dash && !out.is_empty() {
                    out.push('-');
                }
                pending_dash = false;
                out.push(c);
                continue;
            }
            _ => {
                pending_dash = true;
                continue;
            }
        };
        if pending_dash && !out.is_empty() {
            out.push('-');
        }
        pending_dash = false;
        out.push_str(folded);
    }
    out.truncate(SLUG_MAX);
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "carte".to_owned()
    } else {
        out
    }
}

/// Un slug ne contient que `[a-z0-9-]`, sans tiret au début ni à la fin. Garantit qu'il est
/// utilisable tel quel comme nom de fichier et dans une URL.
pub fn is_valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= SLUG_MAX
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Erreur de validation rattachée à un champ du formulaire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldError {
    pub field: &'static str,
    pub message: String,
}

impl CardDraft {
    /// Normalise (trim) puis valide. Renvoie toutes les erreurs d'un coup pour le formulaire.
    pub fn validated(mut self) -> Result<Self, Vec<FieldError>> {
        self.name = self.name.trim().to_owned();
        self.description = self.description.trim().to_owned();
        self.effect_file = self.effect_file.trim().to_owned();
        self.effect = normalize_multiline(&self.effect);
        self.flavor_text = self.flavor_text.trim().to_owned();

        let mut errors = Vec::new();
        let mut check_len = |field: &'static str, value: &str, max: usize| {
            let len = value.chars().count();
            if len > max {
                errors.push(FieldError {
                    field,
                    message: format!("{len} caractères, {max} maximum"),
                });
            }
        };
        check_len("name", &self.name, limits::NAME);
        check_len("description", &self.description, limits::DESCRIPTION);
        check_len("effect", &self.effect, limits::EFFECT);
        check_len("effect_file", &self.effect_file, limits::EFFECT_FILE);
        check_len("flavor_text", &self.flavor_text, limits::FLAVOR);

        if self.name.is_empty() {
            errors.push(FieldError {
                field: "name",
                message: "le nom est obligatoire".into(),
            });
        }
        for (field, value) in [
            ("deploy_cost", self.deploy_cost),
            ("daily_cost", self.daily_cost),
        ] {
            if !(0..=limits::COST_MAX).contains(&value) {
                errors.push(FieldError {
                    field,
                    message: format!("doit être entre 0 et {}", limits::COST_MAX),
                });
            }
        }

        if errors.is_empty() {
            Ok(self)
        } else {
            Err(errors)
        }
    }
}

/// Trim chaque ligne et retire les lignes vides en début et fin, en gardant les retours à la ligne
/// voulus (les effets s'écrivent souvent en liste à puces).
fn normalize_multiline(s: &str) -> String {
    s.replace("\r\n", "\n")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim_matches('\n')
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_handles_french_names() {
        assert_eq!(slugify("ARGO CD"), "argo-cd");
        assert_eq!(slugify("  Health Check!! "), "health-check");
        assert_eq!(slugify("SRE d'astreinte"), "sre-d-astreinte");
        assert_eq!(
            slugify("Équipe Sécurité & Conformité"),
            "equipe-securite-conformite"
        );
        assert_eq!(slugify("Cœur"), "coeur");
        assert_eq!(slugify("!!!"), "carte");
        assert!(slugify(&"x".repeat(100)).len() <= SLUG_MAX);
        for name in ["ARGO CD", "Équipe", "a - b", "-x-", "日本"] {
            assert!(is_valid_slug(&slugify(name)), "{name}");
        }
    }

    #[test]
    fn slug_validation_rejects_paths() {
        assert!(is_valid_slug("argo-cd-2"));
        assert!(!is_valid_slug("../etc"));
        assert!(!is_valid_slug("Argo"));
        assert!(!is_valid_slug("a/b"));
        assert!(!is_valid_slug("-a"));
        assert!(!is_valid_slug(""));
    }

    fn draft() -> CardDraft {
        CardDraft {
            name: "  Argo CD ".into(),
            card_type: CardType::App,
            skin: Skin::Light,
            foil: Foil::Basic,
            shape: Shape::Solid,
            size: Size::Normal,
            badge: Badge::Text,
            title: Title::Block,
            desc_style: DescStyle::Classic,
            relief: Relief::Flat,
            background: Background::Plain,
            effect_style: EffectStyle::Text,
            effect_file: String::new(),
            refs: Refs::Plain,
            description: "GitOps".into(),
            effect: "\r\n- Ajoute +1 deployment MAX  \r\n- Multi environment\r\n\r\n".into(),
            deploy_cost: 2,
            daily_cost: 1,
            flavor_text: String::new(),
            art: None,
        }
    }

    #[test]
    fn enums_roundtrip_through_slug() {
        for t in CardType::ALL {
            assert_eq!(t.slug().parse::<CardType>().unwrap(), *t);
        }
        assert_eq!("GOLD".parse::<Foil>().unwrap(), Foil::Gold);
        for f in Foil::ALL {
            assert_eq!(f.slug().parse::<Foil>().unwrap(), *f);
        }
        assert!("package".parse::<CardType>().is_err());
    }

    #[test]
    fn shapes_round_trip() {
        assert_eq!(Shape::default(), Shape::Solid);
        for s in Shape::ALL {
            assert_eq!(s.slug().parse::<Shape>().unwrap(), *s);
        }
        assert!("round".parse::<Shape>().is_err());
    }

    #[test]
    fn sizes_round_trip() {
        assert_eq!(Size::default(), Size::Normal);
        for s in Size::ALL {
            assert_eq!(s.slug().parse::<Size>().unwrap(), *s);
        }
        assert!("tiny".parse::<Size>().is_err());
    }

    #[test]
    fn titles_round_trip() {
        assert_eq!(Title::default(), Title::Block);
        for t in Title::ALL {
            assert_eq!(t.slug().parse::<Title>().unwrap(), *t);
        }
        assert!("top".parse::<Title>().is_err());
    }

    #[test]
    fn refs_round_trip() {
        assert_eq!(Refs::default(), Refs::Plain);
        for t in Refs::ALL {
            assert_eq!(t.slug().parse::<Refs>().unwrap(), *t);
        }
        assert!("nope".parse::<Refs>().is_err());
    }

    #[test]
    fn effect_style_round_trip() {
        assert_eq!(EffectStyle::default(), EffectStyle::Text);
        for t in EffectStyle::ALL {
            assert_eq!(t.slug().parse::<EffectStyle>().unwrap(), *t);
        }
        assert!("nope".parse::<EffectStyle>().is_err());
    }

    #[test]
    fn background_round_trip() {
        assert_eq!(Background::default(), Background::Plain);
        for t in Background::ALL {
            assert_eq!(t.slug().parse::<Background>().unwrap(), *t);
        }
        assert!("nope".parse::<Background>().is_err());
    }

    #[test]
    fn relief_round_trip() {
        assert_eq!(Relief::default(), Relief::Flat);
        for t in Relief::ALL {
            assert_eq!(t.slug().parse::<Relief>().unwrap(), *t);
        }
        assert!("nope".parse::<Relief>().is_err());
    }

    #[test]
    fn desc_styles_round_trip() {
        assert_eq!(DescStyle::default(), DescStyle::Classic);
        for t in DescStyle::ALL {
            assert_eq!(t.slug().parse::<DescStyle>().unwrap(), *t);
        }
        assert!("top".parse::<DescStyle>().is_err());
    }

    #[test]
    fn badges_round_trip() {
        assert_eq!(Badge::default(), Badge::Text);
        for b in Badge::ALL {
            assert_eq!(b.slug().parse::<Badge>().unwrap(), *b);
        }
        assert!("top".parse::<Badge>().is_err());
    }

    #[test]
    fn legacy_rarity_maps_to_foil() {
        assert_eq!(Foil::from_legacy(None, None).unwrap(), Foil::Basic);
        assert_eq!(Foil::from_legacy(Some("none"), None).unwrap(), Foil::Basic);
        assert_eq!(Foil::from_legacy(None, Some("gold")).unwrap(), Foil::Gold);
        assert_eq!(Foil::from_legacy(None, Some("basic")).unwrap(), Foil::Basic);
        // une finition explicite l'emporte sur l'ancienne rareté
        assert_eq!(
            Foil::from_legacy(Some("stars"), Some("gold")).unwrap(),
            Foil::Stars
        );
        assert!(Foil::from_legacy(Some("chrome"), None).is_err());
    }

    #[test]
    fn validation_normalizes_text() {
        let d = draft().validated().unwrap();
        assert_eq!(d.name, "Argo CD");
        assert_eq!(d.effect, "- Ajoute +1 deployment MAX\n- Multi environment");
    }

    #[test]
    fn validation_reports_every_error() {
        let mut d = draft();
        d.name = "   ".into();
        d.deploy_cost = 150;
        d.description = "x".repeat(limits::DESCRIPTION + 1);
        let errors = d.validated().unwrap_err();
        let fields: Vec<_> = errors.iter().map(|e| e.field).collect();
        assert!(fields.contains(&"name"));
        assert!(fields.contains(&"deploy_cost"));
        assert!(fields.contains(&"description"));
    }

    #[test]
    fn card_serializes_flat() {
        let card = Card {
            slug: "argo-cd".into(),
            draft: draft(),
        };
        let json = serde_json::to_string(&card).unwrap();
        assert!(json.contains("\"slug\":\"argo-cd\""));
        assert!(json.contains("\"card_type\":\"app\""));
        assert!(!json.contains("draft"));
    }
}
