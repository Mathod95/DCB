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
    pub concepts: Concepts,
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
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Victory {
    pub title: String,
    pub text: String,
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

/// Partie « Concepts » : skin, finition (texte de `rarity`) et contour.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Concepts {
    pub title: String,
    pub text: String,
    pub skin: ConceptPart,
    pub shape: ConceptPart,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConceptPart {
    pub title: String,
    pub text: String,
}

impl Rules {
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    pub async fn load(path: &Path) -> Result<Self, String> {
        let text = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
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
    }
}
