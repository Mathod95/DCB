//! Templates askama et modèles de vue.

use askama::Template;
use dcb_core::{CardDraft, CardType, FieldError, Rarity, Skin, limits};

/// Ce dont le partial `card.html` a besoin pour dessiner une carte.
pub struct CardView {
    pub id: Option<i64>,
    pub name: String,
    pub name_size: &'static str,
    pub type_slug: &'static str,
    pub type_label: &'static str,
    pub skin: &'static str,
    pub rarity_slug: &'static str,
    pub rarity_label: &'static str,
    pub description: String,
    pub effect: String,
    pub deploy_cost: i64,
    pub daily_cost: i64,
    pub flavor_text: String,
    pub art_path: Option<String>,
}

impl CardView {
    pub fn new(id: Option<i64>, d: &CardDraft) -> Self {
        let name = if d.name.trim().is_empty() {
            "CARD NAME".to_owned()
        } else {
            d.name.clone()
        };
        // Le bloc nom fait 336px de large, le badge de type en occupe ~80 : on réduit la police
        // au lieu de couper le nom.
        let name_size = match name.chars().count() {
            0..=14 => "l",
            15..=18 => "m",
            _ => "s",
        };
        Self {
            id,
            name,
            name_size,
            type_slug: d.card_type.slug(),
            type_label: d.card_type.label(),
            skin: d.skin.slug(),
            rarity_slug: d.rarity.slug(),
            rarity_label: d.rarity.label(),
            description: d.description.clone(),
            effect: d.effect.clone(),
            deploy_cost: d.deploy_cost,
            daily_cost: d.daily_cost,
            flavor_text: d.flavor_text.clone(),
            art_path: d.art_path.clone(),
        }
    }

    /// Le libellé RESSOURCE HUMAINE passe sur deux lignes, comme dans le Figma.
    pub fn type_label_lines(&self) -> Vec<&str> {
        self.type_label.split(' ').collect()
    }
}

pub struct Choice {
    pub value: &'static str,
    pub label: &'static str,
    pub color: Option<&'static str>,
}

pub fn type_choices() -> Vec<Choice> {
    CardType::ALL
        .iter()
        .map(|t| Choice {
            value: t.slug(),
            label: t.label(),
            color: Some(t.color()),
        })
        .collect()
}

pub fn skin_choices() -> Vec<Choice> {
    Skin::ALL
        .iter()
        .map(|s| Choice {
            value: s.slug(),
            label: s.label(),
            color: None,
        })
        .collect()
}

pub fn rarity_choices() -> Vec<Choice> {
    Rarity::ALL
        .iter()
        .map(|r| Choice {
            value: r.slug(),
            label: r.label(),
            color: None,
        })
        .collect()
}

#[derive(Template)]
#[template(path = "menu.html")]
pub struct MenuPage {
    pub card_count: i64,
}

pub struct FilterLink {
    pub label: &'static str,
    pub href: String,
    pub active: bool,
    pub color: Option<&'static str>,
}

#[derive(Template)]
#[template(path = "gallery.html")]
pub struct GalleryPage {
    pub cards: Vec<CardView>,
    pub type_links: Vec<FilterLink>,
    pub rarity_links: Vec<FilterLink>,
    pub filtered: bool,
}

#[derive(Template)]
#[template(path = "detail.html")]
pub struct DetailPage {
    pub card: CardView,
}

/// Valeurs brutes du formulaire, renvoyées telles quelles en cas d'erreur.
#[derive(Debug, Clone, Default)]
pub struct FormValues {
    pub name: String,
    pub card_type: String,
    pub skin: String,
    pub rarity: String,
    pub description: String,
    pub effect: String,
    pub deploy_cost: String,
    pub daily_cost: String,
    pub flavor_text: String,
    pub current_art: Option<String>,
}

impl FormValues {
    pub fn from_draft(d: &CardDraft) -> Self {
        Self {
            name: d.name.clone(),
            card_type: d.card_type.slug().into(),
            skin: d.skin.slug().into(),
            rarity: d.rarity.slug().into(),
            description: d.description.clone(),
            effect: d.effect.clone(),
            deploy_cost: d.deploy_cost.to_string(),
            daily_cost: d.daily_cost.to_string(),
            flavor_text: d.flavor_text.clone(),
            current_art: d.art_path.clone(),
        }
    }

    pub fn blank() -> Self {
        Self {
            card_type: CardType::App.slug().into(),
            skin: Skin::Light.slug().into(),
            rarity: Rarity::Basic.slug().into(),
            deploy_cost: "0".into(),
            daily_cost: "0".into(),
            ..Default::default()
        }
    }
}

#[derive(Template)]
#[template(path = "form.html")]
pub struct FormPage {
    pub title: &'static str,
    pub action: String,
    pub cancel_href: String,
    pub values: FormValues,
    pub errors: Vec<FieldError>,
    pub card: CardView,
    pub types: Vec<Choice>,
    pub skins: Vec<Choice>,
    pub rarities: Vec<Choice>,
    pub lim_name: usize,
    pub lim_description: usize,
    pub lim_effect: usize,
    pub lim_flavor: usize,
}

impl FormPage {
    pub fn new(
        title: &'static str,
        action: String,
        cancel_href: String,
        values: FormValues,
        errors: Vec<FieldError>,
        card: CardView,
    ) -> Self {
        Self {
            title,
            action,
            cancel_href,
            values,
            errors,
            card,
            types: type_choices(),
            skins: skin_choices(),
            rarities: rarity_choices(),
            lim_name: limits::NAME,
            lim_description: limits::DESCRIPTION,
            lim_effect: limits::EFFECT,
            lim_flavor: limits::FLAVOR,
        }
    }

    /// Message d'erreur d'un champ, vide s'il n'y en a pas.
    pub fn error(&self, field: &str) -> String {
        self.errors
            .iter()
            .filter(|e| e.field == field)
            .map(|e| e.message.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Partial renvoyé par `/preview`.
#[derive(Template)]
#[template(path = "card_preview.html")]
pub struct CardPreview {
    pub card: CardView,
}

#[derive(Template)]
#[template(path = "not_found.html")]
pub struct NotFoundPage;
