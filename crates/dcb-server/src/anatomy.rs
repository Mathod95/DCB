//! Partie « Anatomie d'une carte » de la page des règles : schéma annoté d'une carte de la
//! collection (étiquettes reliées à chaque zone), vue squelette, fiche de chaque zone et
//! fichier TOML de la carte, ligne par ligne.
//!
//! Le texte vient de `[anatomy]` dans `rules/regles.toml` ; les cotes des zones viennent de
//! `card.css` (carte de 374 × 574, posée dans une scène de 920 × 640).

use dcb_core::CardDraft;

/// Position de la carte dans la scène du schéma.
pub const CARD_X: i32 = 273;
pub const CARD_Y: i32 = 28;

/// Géométrie d'une zone, en cotes de carte (sauf `label_y`, en cotes de scène).
pub struct Zone {
    pub key: &'static str,
    /// Côté de l'étiquette.
    pub left: bool,
    /// Hauteur de l'étiquette dans la scène.
    pub label_y: i32,
    /// Point visé sur la carte.
    pub target: (i32, i32),
    /// Rectangle surligné : x, y, largeur, hauteur.
    pub rect: (i32, i32, i32, i32),
}

/// Les zones de la carte, dans l'ordre de numérotation.
pub const ZONES: &[Zone] = &[
    Zone {
        key: "name",
        left: true,
        label_y: 77,
        target: (60, 49),
        rect: (19, 19, 244, 60),
    },
    Zone {
        key: "type",
        left: false,
        label_y: 77,
        target: (312, 49),
        rect: (270, 19, 85, 60),
    },
    Zone {
        key: "stroke",
        left: false,
        label_y: 198,
        target: (365, 170),
        rect: (7, 7, 360, 560),
    },
    Zone {
        key: "art",
        left: true,
        label_y: 207,
        target: (60, 179),
        rect: (19, 89, 336, 180),
    },
    Zone {
        key: "description",
        left: true,
        label_y: 337,
        target: (60, 309),
        rect: (19, 279, 336, 60),
    },
    Zone {
        key: "effect",
        left: false,
        label_y: 432,
        target: (320, 404),
        rect: (19, 349, 336, 110),
    },
    Zone {
        key: "deploy",
        left: true,
        label_y: 529,
        target: (40, 501),
        rect: (19, 469, 163, 65),
    },
    Zone {
        key: "daily",
        left: false,
        label_y: 529,
        target: (330, 501),
        rect: (192, 469, 163, 65),
    },
    Zone {
        key: "flavor",
        left: true,
        label_y: 583,
        target: (60, 555),
        rect: (40, 544, 294, 22),
    },
];

pub fn zone(key: &str) -> Option<&'static Zone> {
    ZONES.iter().find(|z| z.key == key)
}

/// Une ligne du fichier TOML de la carte, colorée, avec la zone qu'elle remplit.
pub struct TomlLine {
    pub zone: Option<&'static str>,
    pub html: String,
}

/// Zone remplie par un champ du fichier (`None` : réglage d'allure, voir la partie Concepts).
fn zone_of_field(field: &str) -> Option<&'static str> {
    Some(match field {
        "name" => "name",
        "type" => "type",
        "shape" => "stroke",
        "art" => "art",
        "description" => "description",
        "effect" => "effect",
        "deploy_cost" => "deploy",
        "daily_cost" => "daily",
        "flavor_text" => "flavor",
        _ => return None,
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Valeur TOML colorée : nombre ou chaîne.
fn value(v: &str) -> String {
    let t = v.trim();
    if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || c == '-') {
        format!(r#"<span class="t-num">{}</span>"#, esc(v))
    } else {
        format!(r#"<span class="t-str">{}</span>"#, esc(v))
    }
}

/// Le fichier `cards/<slug>.toml` de la carte, tel que l'éditeur l'écrit, découpé en lignes.
pub fn toml_lines(d: &CardDraft) -> Vec<TomlLine> {
    let text = crate::store::card_toml(d);
    let mut out = Vec::new();
    let mut current: Option<&'static str> = None;
    let mut in_multiline = false;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if !in_multiline && let Some((k, v)) = line.split_once(" = ") {
            current = zone_of_field(k.trim());
            in_multiline = v.trim_start().starts_with("\"\"\"")
                && !v.trim_end().ends_with("\"\"\"")
                || v.trim() == "\"\"\"";
            let comment = if current.is_none() {
                r#"  <span class="t-com"># voir Concepts</span>"#
            } else {
                ""
            };
            out.push(TomlLine {
                zone: current,
                html: format!(
                    r#"<span class="t-key">{}</span> = {}{comment}"#,
                    esc(k),
                    value(v)
                ),
            });
            continue;
        }
        if line.trim_end().ends_with("\"\"\"") {
            in_multiline = false;
        }
        out.push(TomlLine {
            zone: current,
            html: value(line),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcb_core::{Badge, CardType, Foil, Shape, Size, Skin};

    #[test]
    fn toml_lines_follow_the_file_and_map_zones() {
        let d = CardDraft {
            name: "ARGO CD".into(),
            card_type: CardType::App,
            skin: Skin::Light,
            foil: Foil::Gold,
            shape: Shape::Solid,
            size: Size::Normal,
            badge: Badge::Text,
            description: "GitOps".into(),
            effect: "- Ajoute +1 deployment MAX\n- Multi <env>".into(),
            deploy_cost: 2,
            daily_cost: 1,
            flavor_text: "Pas de Flux".into(),
            art: Some("argo-cd.png".into()),
        };
        let lines = toml_lines(&d);
        let zones: Vec<_> = lines.iter().map(|l| l.zone).collect();
        assert!(zones.contains(&Some("effect")) && zones.contains(&Some("deploy")));
        // la suite de l'effet sur plusieurs lignes reste dans la zone Effect
        let effect = lines.iter().filter(|l| l.zone == Some("effect")).count();
        assert!(effect >= 3, "{effect}");
        // skin et finition : réglages d'allure, sans zone
        assert!(
            lines
                .iter()
                .any(|l| l.zone.is_none() && l.html.contains("foil"))
        );
        assert!(lines.iter().all(|l| !l.html.contains("<env>")));
        assert!(
            lines
                .iter()
                .any(|l| l.html.contains(r#"<span class="t-num">2</span>"#))
        );
    }

    #[test]
    fn zones_are_unique() {
        for (i, z) in ZONES.iter().enumerate() {
            assert!(ZONES[i + 1..].iter().all(|o| o.key != z.key), "{}", z.key);
        }
    }
}
