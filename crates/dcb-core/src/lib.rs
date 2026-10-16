//! Modèle de domaine de DevCardBattle, partagé entre le serveur et les futurs clients
//! (moteur de duel, client Bevy/WASM).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Limites de saisie, calées sur la place disponible dans le gabarit Figma (374×574).
pub mod limits {
    pub const NAME: usize = 24;
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
    /// Rareté (calques `rarityLayer/*` du Figma). `Gold` ajoute l'overlay doré.
    Rarity {
        Basic => "basic", "Basic",
        Gold  => "gold",  "Gold",
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
    pub rarity: Rarity,
    pub description: String,
    pub effect: String,
    pub deploy_cost: i64,
    pub daily_cost: i64,
    pub flavor_text: String,
    /// Chemin public de l'illustration (`/uploads/...`), si présente.
    pub art_path: Option<String>,
}

/// Carte enregistrée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub id: i64,
    #[serde(flatten)]
    pub draft: CardDraft,
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

    fn draft() -> CardDraft {
        CardDraft {
            name: "  Argo CD ".into(),
            card_type: CardType::App,
            skin: Skin::Light,
            rarity: Rarity::Basic,
            description: "GitOps".into(),
            effect: "\r\n- Ajoute +1 deployment MAX  \r\n- Multi environment\r\n\r\n".into(),
            deploy_cost: 2,
            daily_cost: 1,
            flavor_text: String::new(),
            art_path: None,
        }
    }

    #[test]
    fn enums_roundtrip_through_slug() {
        for t in CardType::ALL {
            assert_eq!(t.slug().parse::<CardType>().unwrap(), *t);
        }
        assert_eq!("GOLD".parse::<Rarity>().unwrap(), Rarity::Gold);
        assert!("package".parse::<CardType>().is_err());
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
            id: 7,
            draft: draft(),
        };
        let json = serde_json::to_string(&card).unwrap();
        assert!(json.contains("\"id\":7"));
        assert!(json.contains("\"card_type\":\"app\""));
        assert!(!json.contains("draft"));
    }
}
