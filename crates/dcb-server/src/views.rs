//! Templates askama et modèles de vue.

use askama::Template;
use dcb_core::{Card, CardDraft, CardType, FieldError, Foil, Shape, Size, Skin, limits};

use crate::auth::{Role, User};
use crate::rules::{self, Rules, TypeEntry, inline};

/// Ce que la barre du haut et les pages doivent savoir du visiteur.
#[derive(Clone, Default)]
pub struct Ctx {
    pub username: Option<String>,
    pub is_admin: bool,
    /// Admin avec l'éditeur activé : rechargement, fichiers à commiter, erreurs de chargement.
    pub can_edit: bool,
    /// Peut créer des cartes : l'admin quand l'éditeur est activé, tout joueur en mode démo.
    pub can_create: bool,
    /// Mode démo (`DCB_SANDBOX`) : les cartes créées en ligne disparaissent au prochain
    /// déploiement.
    pub sandbox: bool,
}

impl Ctx {
    pub fn anonymous() -> Self {
        Self::default()
    }

    /// Initiale affichée dans l'avatar de la barre du haut.
    pub fn initial(&self) -> String {
        self.username
            .as_deref()
            .and_then(|n| n.chars().next())
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default()
    }

    pub fn for_user(user: Option<&User>, editor_enabled: bool, sandbox: bool) -> Self {
        match user {
            None => Self::default(),
            Some(u) => {
                let editor = editor_enabled || sandbox;
                Self {
                    username: Some(u.username.clone()),
                    is_admin: u.is_admin(),
                    can_edit: u.is_admin() && editor,
                    can_create: editor && (u.is_admin() || sandbox),
                    sandbox,
                }
            }
        }
    }
}

/// Ce dont le partial `card.html` a besoin pour dessiner une carte.
pub struct CardView {
    pub slug: Option<String>,
    pub name: String,
    pub name_size: &'static str,
    pub type_slug: &'static str,
    pub type_label: &'static str,
    pub skin: &'static str,
    pub foil: &'static str,
    pub foil_label: &'static str,
    pub shape: &'static str,
    /// Taille de la carte (`normal`, `compact`), posée en `data-size` sur la carte.
    pub size: &'static str,
    pub size_label: &'static str,
    pub description: String,
    pub effect: String,
    pub deploy_cost: i64,
    pub daily_cost: i64,
    pub flavor_text: String,
    /// Aperçu de l'éditeur avec texte d'ambiance vide : on montre l'emplacement en grisé.
    pub flavor_placeholder: bool,
    /// URL publique de l'illustration (`/art/<fichier>`).
    pub art_url: Option<String>,
}

impl CardView {
    pub fn new(slug: Option<&str>, d: &CardDraft) -> Self {
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
            slug: slug.map(str::to_owned),
            name,
            name_size,
            type_slug: d.card_type.slug(),
            type_label: d.card_type.label(),
            skin: d.skin.slug(),
            foil: d.foil.slug(),
            foil_label: d.foil.label(),
            shape: d.shape.slug(),
            size: d.size.slug(),
            size_label: d.size.label(),
            description: d.description.clone(),
            effect: d.effect.clone(),
            deploy_cost: d.deploy_cost,
            daily_cost: d.daily_cost,
            flavor_text: d.flavor_text.clone(),
            flavor_placeholder: slug.is_none() && d.flavor_text.trim().is_empty(),
            art_url: d.art.as_ref().map(|f| format!("/art/{f}")),
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

pub fn foil_choices() -> Vec<Choice> {
    Foil::ALL
        .iter()
        .map(|f| Choice {
            value: f.slug(),
            label: f.label(),
            color: None,
        })
        .collect()
}

pub fn shape_choices() -> Vec<Choice> {
    Shape::ALL
        .iter()
        .map(|s| Choice {
            value: s.slug(),
            label: s.label(),
            color: None,
        })
        .collect()
}

pub fn size_choices() -> Vec<Choice> {
    Size::ALL
        .iter()
        .map(|s| Choice {
            value: s.slug(),
            label: s.label(),
            color: None,
        })
        .collect()
}

#[derive(Template)]
#[template(path = "menu.html")]
pub struct MenuPage {
    pub ctx: Ctx,
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
    pub ctx: Ctx,
    pub cards: Vec<CardView>,
    pub type_links: Vec<FilterLink>,
    pub foil_links: Vec<FilterLink>,
    pub shape_links: Vec<FilterLink>,
    pub size_links: Vec<FilterLink>,
    pub filtered: bool,
    /// Fichiers `cards/*.toml` ignorés au dernier chargement.
    pub load_errors: Vec<String>,
}

#[derive(Template)]
#[template(path = "detail.html")]
pub struct DetailPage {
    pub ctx: Ctx,
    /// Le visiteur peut modifier et supprimer cette carte.
    pub can_modify: bool,
    /// Carte créée en ligne en mode démo (elle disparaîtra au prochain déploiement).
    pub demo_card: bool,
    pub card: CardView,
    /// Chemin du fichier source, affiché pour savoir quoi commiter.
    pub file: String,
    pub art_file: Option<String>,
}

/// Valeurs brutes du formulaire, renvoyées telles quelles en cas d'erreur.
#[derive(Debug, Clone, Default)]
pub struct FormValues {
    pub name: String,
    pub card_type: String,
    pub skin: String,
    pub foil: String,
    pub shape: String,
    pub size: String,
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
            foil: d.foil.slug().into(),
            shape: d.shape.slug().into(),
            size: d.size.slug().into(),
            description: d.description.clone(),
            effect: d.effect.clone(),
            deploy_cost: d.deploy_cost.to_string(),
            daily_cost: d.daily_cost.to_string(),
            flavor_text: d.flavor_text.clone(),
            current_art: d.art.clone(),
        }
    }

    pub fn blank() -> Self {
        Self {
            card_type: CardType::App.slug().into(),
            skin: Skin::Light.slug().into(),
            foil: Foil::Basic.slug().into(),
            shape: Shape::Solid.slug().into(),
            size: Size::Normal.slug().into(),
            deploy_cost: "0".into(),
            daily_cost: "0".into(),
            ..Default::default()
        }
    }
}

#[derive(Template)]
#[template(path = "form.html")]
pub struct FormPage {
    pub ctx: Ctx,
    pub title: &'static str,
    pub action: String,
    pub cancel_href: String,
    pub values: FormValues,
    pub errors: Vec<FieldError>,
    pub card: CardView,
    pub types: Vec<Choice>,
    pub skins: Vec<Choice>,
    pub foils: Vec<Choice>,
    pub shapes: Vec<Choice>,
    pub sizes: Vec<Choice>,
    pub lim_name: usize,
    pub lim_description: usize,
    pub lim_effect: usize,
    pub lim_flavor: usize,
}

impl FormPage {
    pub fn new(
        ctx: Ctx,
        title: &'static str,
        action: String,
        cancel_href: String,
        values: FormValues,
        errors: Vec<FieldError>,
        card: CardView,
    ) -> Self {
        Self {
            ctx,
            title,
            action,
            cancel_href,
            values,
            errors,
            card,
            types: type_choices(),
            skins: skin_choices(),
            foils: foil_choices(),
            shapes: shape_choices(),
            sizes: size_choices(),
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
pub struct NotFoundPage {
    pub ctx: Ctx,
}

#[derive(Template)]
#[template(path = "concepts.html")]
pub struct ConceptsPage {
    pub ctx: Ctx,
    pub doc: Option<crate::concepts::ConceptsDoc>,
    pub parts: Vec<crate::concepts::PartView>,
    /// Erreur du dernier chargement de `rules/concepts.toml`.
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "forbidden.html")]
pub struct ForbiddenPage {
    pub ctx: Ctx,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginPage {
    pub ctx: Ctx,
    pub next: String,
    pub username: String,
    pub error: Option<String>,
    /// Aucun compte n'existe encore : on explique comment créer le premier admin.
    pub no_users: bool,
}

#[derive(Template)]
#[template(path = "account.html")]
pub struct AccountPage {
    pub ctx: Ctx,
    pub role_label: &'static str,
    pub error: Option<String>,
    pub done: bool,
}

pub struct UserRowView {
    pub id: i64,
    pub username: String,
    pub role_label: &'static str,
    pub other_role: &'static str,
    pub other_role_label: &'static str,
    pub active: bool,
    pub is_self: bool,
    pub created: String,
    pub last_login: String,
}

/// Date UTC lisible (AAAA-MM-JJ) d'un horodatage Unix, sans dépendance de date.
pub fn date(ts: i64) -> String {
    let is_leap = |y: i64| (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let mut days = ts.div_euclid(86_400);
    let mut year = 1970;
    loop {
        let len = if is_leap(year) { 366 } else { 365 };
        if days < len {
            break;
        }
        days -= len;
        year += 1;
    }
    let months = [
        31,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0;
    while days >= months[month] {
        days -= months[month];
        month += 1;
    }
    format!("{year:04}-{:02}-{:02}", month + 1, days + 1)
}

impl UserRowView {
    pub fn new(u: &User, me: &User) -> Self {
        let other = if u.role == Role::Admin {
            Role::User
        } else {
            Role::Admin
        };
        Self {
            id: u.id,
            username: u.username.clone(),
            role_label: u.role.label(),
            other_role: other.slug(),
            other_role_label: other.label(),
            active: u.active,
            is_self: u.id == me.id,
            created: date(u.created_at),
            last_login: u.last_login_at.map(date).unwrap_or_else(|| "jamais".into()),
        }
    }
}

/// Message affiché en haut de la page d'administration après une action.
pub struct Notice {
    pub ok: bool,
    pub text: String,
    /// Mot de passe à transmettre à la personne, affiché une seule fois.
    pub password: Option<String>,
}

#[derive(Template)]
#[template(path = "admin_users.html")]
pub struct AdminUsersPage {
    pub ctx: Ctx,
    pub users: Vec<UserRowView>,
    pub notice: Option<Notice>,
    pub new_username: String,
    pub editor_enabled: bool,
    pub sandbox: bool,
}

#[cfg(test)]
mod date_tests {
    #[test]
    fn formats_unix_dates() {
        assert_eq!(super::date(0), "1970-01-01");
        assert_eq!(super::date(951_782_400), "2000-02-29");
        assert_eq!(super::date(1_792_000_000), "2026-10-14");
    }
}

// ---------------------------------------------------------------------------------------------
// Page des règles. Les textes sont convertis en HTML sûr par `rules::inline` (d'où `|safe`
// dans le template).

/// Couleur CSS d'un type de carte, définie dans app.css (`--t-<type>`).
fn type_color(t: CardType) -> String {
    format!("var(--t-{})", t.slug())
}

pub struct RulesType {
    pub name: String,
    pub color: String,
    pub text: String,
    pub count_label: String,
    pub gallery_href: String,
    pub card: Option<CardView>,
}

impl RulesType {
    pub fn new(entry: &TypeEntry, count: i64, card: Option<&Card>) -> Self {
        Self {
            name: entry.name.clone(),
            color: type_color(entry.card_type),
            text: inline(&entry.text),
            count_label: match count {
                0 => "Aucune carte pour l'instant".to_owned(),
                1 => "1 carte dans la collection".to_owned(),
                n => format!("{n} cartes dans la collection"),
            },
            gallery_href: format!("/gallery?type={}", entry.card_type.slug()),
            card: card.map(|c| CardView::new(Some(&c.slug), &c.draft)),
        }
    }
}

/// Une même carte déclinée dans chaque valeur d'un réglage (skin, finition, contour).
pub struct FoilSample {
    pub label: &'static str,
    pub card: CardView,
}

pub struct TileView {
    pub label: String,
    pub color: String,
}

pub struct TileGroupView {
    pub title: String,
    pub items: Vec<TileView>,
}

pub struct PhaseView {
    pub name: String,
    pub when: String,
    pub paragraphs: Vec<String>,
}

/// Contenu prêt à afficher, construit depuis `rules/regles.toml`.
pub struct RulesContent {
    pub draft_notice: String,
    pub title: String,
    pub context: Vec<String>,
    pub facts: Vec<(String, String)>,
    pub start_title: String,
    pub start_text: String,
    pub groups: Vec<TileGroupView>,
    pub victory_title: String,
    pub victory_text: String,
    pub start_label: String,
    pub goal_label: String,
    pub start_percent: String,
    pub types_title: String,
    pub types: Vec<RulesType>,
    pub day_title: String,
    pub day_intro: String,
    pub phases: Vec<PhaseView>,
    pub points_title: String,
    pub points: Vec<String>,
    pub life_title: String,
    pub life_text: String,
    pub features: Vec<String>,
    pub kpis: Vec<String>,
    pub rarity_title: String,
    pub rarity_text: String,
    pub foils: Vec<FoilSample>,
    pub concepts_title: String,
    pub concepts_text: String,
    pub skin_title: String,
    pub skin_text: String,
    pub skins: Vec<FoilSample>,
    pub shape_title: String,
    pub shape_text: String,
    pub shapes: Vec<FoilSample>,
}

/// Exemples de la partie « Concepts », une carte par réglage.
pub struct ConceptSamples {
    pub skins: Vec<FoilSample>,
    pub foils: Vec<FoilSample>,
    pub shapes: Vec<FoilSample>,
}

#[derive(Template)]
#[template(path = "rules.html")]
pub struct RulesPage {
    pub ctx: Ctx,
    pub content: Option<RulesContent>,
    /// Erreur du dernier chargement de `rules/regles.toml` (les règles affichées sont alors
    /// les dernières valides).
    pub error: Option<String>,
}

impl RulesPage {
    pub fn missing(ctx: Ctx, error: Option<String>) -> Self {
        Self {
            ctx,
            content: None,
            error,
        }
    }

    pub fn new(
        ctx: Ctx,
        r: &Rules,
        error: Option<String>,
        types: Vec<RulesType>,
        samples: ConceptSamples,
    ) -> Self {
        let percent = r.start_percent();
        let content = RulesContent {
            draft_notice: r.draft_notice.clone(),
            title: r.title.clone(),
            context: r.context.iter().map(|p| inline(p)).collect(),
            facts: r
                .facts
                .iter()
                .map(|f| (inline(&f.label), inline(&f.value)))
                .collect(),
            start_title: r.start.title.clone(),
            start_text: inline(&r.start.text),
            groups: r
                .start
                .groups
                .iter()
                .map(|g| TileGroupView {
                    title: g.title.clone(),
                    items: g
                        .items
                        .iter()
                        .map(|t| TileView {
                            label: t.label.clone(),
                            color: type_color(t.card_type),
                        })
                        .collect(),
                })
                .collect(),
            victory_title: r.victory.title.clone(),
            victory_text: inline(&r.victory.text),
            start_label: rules::money(r.victory.start_amount),
            goal_label: rules::money(r.victory.goal_amount),
            // Point décimal même en locale française : c'est une valeur CSS.
            start_percent: format!("{percent:.2}"),
            types_title: r.types.title.clone(),
            types,
            day_title: r.day.title.clone(),
            day_intro: inline(&r.day.intro),
            phases: r
                .day
                .phases
                .iter()
                .map(|p| PhaseView {
                    name: p.name.clone(),
                    when: p.when.clone(),
                    paragraphs: p.text.iter().map(|t| inline(t)).collect(),
                })
                .collect(),
            points_title: r.day.points.title.clone(),
            points: r.day.points.items.iter().map(|i| inline(i)).collect(),
            life_title: r.lifecycle.title.clone(),
            life_text: inline(&r.lifecycle.text),
            features: r.lifecycle.features.clone(),
            kpis: r.lifecycle.kpis.clone(),
            rarity_title: r.rarity.title.clone(),
            rarity_text: inline(&r.rarity.text),
            foils: samples.foils,
            concepts_title: r.concepts.title.clone(),
            concepts_text: inline(&r.concepts.text),
            skin_title: r.concepts.skin.title.clone(),
            skin_text: inline(&r.concepts.skin.text),
            skins: samples.skins,
            shape_title: r.concepts.shape.title.clone(),
            shape_text: inline(&r.concepts.shape.text),
            shapes: samples.shapes,
        };
        Self {
            ctx,
            content: Some(content),
            error,
        }
    }
}
