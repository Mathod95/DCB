//! Templates askama et modèles de vue.

use askama::Template;
use dcb_core::Background;
use dcb_core::DescStyle;
use dcb_core::EffectStyle;
use dcb_core::Refs;
use dcb_core::Relief;
use dcb_core::{
    Badge, Card, CardDraft, CardType, FieldError, Foil, Shape, Size, Skin, Title, limits,
};

use crate::auth::User;
use crate::rules::{self, Rules, TypeEntry, inline};

/// Ce que la barre du haut et les pages doivent savoir du visiteur.
#[derive(Clone, Default)]
pub struct Ctx {
    pub username: Option<String>,
    pub is_admin: bool,
    /// Libellé du rôle, affiché dans le menu du compte (vide pour un visiteur).
    pub role_label: &'static str,
    pub role: &'static str,
    /// Admin avec l'éditeur activé : rechargement, fichiers à commiter, erreurs de chargement.
    pub can_edit: bool,
    /// Peut créer des cartes : admin ou créateur, éditeur ouvert.
    pub can_create: bool,
    /// Mode démo (`DCB_SANDBOX`) : les cartes créées en ligne disparaissent au prochain
    /// déploiement.
    pub sandbox: bool,
    /// Panneau de connexion de la barre du haut (visiteur).
    pub login: LoginBox,
}

/// État du panneau de connexion : ouvert par `/login` (page demandée sans être connecté, ou
/// mauvais identifiants), avec la page où revenir.
#[derive(Clone, Default)]
pub struct LoginBox {
    pub open: bool,
    /// Page où revenir après la connexion ; vide : la page courante (rempli par app.js).
    pub next: String,
    pub username: String,
    pub error: Option<String>,
    /// Aucun compte n'existe encore : comment créer le premier admin.
    pub no_users: bool,
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
            None => Self {
                sandbox,
                ..Self::default()
            },
            Some(u) => {
                let editor = editor_enabled || sandbox;
                Self {
                    username: Some(u.username.clone()),
                    is_admin: u.is_admin(),
                    role_label: u.role.label(),
                    role: u.role.slug(),
                    can_edit: u.is_admin() && editor,
                    can_create: editor && u.role.can_create(),
                    sandbox,
                    login: LoginBox::default(),
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
    /// Emplacement du type (`text`, `image`), posé en `data-badge` sur la carte.
    pub badge: &'static str,
    pub badge_label: &'static str,
    /// Emplacement du nom (`block`, `image`), posé en `data-title` sur la carte.
    pub title: &'static str,
    pub title_label: &'static str,
    /// Présentation de la description (`classic`, `untitled`, `comment`), en `data-desc`.
    pub desc_style: &'static str,
    pub desc_style_label: &'static str,
    /// Relief des blocs (`data-relief` sur la carte).
    pub relief: &'static str,
    /// Fond de la carte (`data-bg` sur la carte).
    pub background: &'static str,
    pub effect_style: &'static str,
    pub relief_label: &'static str,
    pub background_label: &'static str,
    pub effect_style_label: &'static str,
    /// Nom de fichier en tête du bloc Effect façon code.
    pub effect_file: String,
    /// Lignes du bloc Effect façon code (`<li>…</li>`), avec les cartes citées si demandé.
    pub effect_rows: Vec<String>,
    /// Cartes citées dans l'effet (`data-refs` sur la carte).
    pub refs: &'static str,
    pub refs_label: &'static str,
    pub description: String,
    pub effect: String,
    /// Effet en HTML, avec les cartes citées mises en avant (réglage `refs` = linked).
    pub effect_html: Option<String>,
    pub deploy_cost: i64,
    pub daily_cost: i64,
    pub flavor_text: String,
    /// Aperçu de l'éditeur avec texte d'ambiance vide : on montre l'emplacement en grisé.
    pub flavor_placeholder: bool,
    /// URL publique de l'illustration (`/art/<fichier>`).
    pub art_url: Option<String>,
    /// Calque animé posé dans le fond (skins animées), HTML.
    pub fx: String,
    /// Skin ASCII : sigle du type, l'illustration est convertie par le navigateur.
    pub ascii_word: Option<&'static str>,
    /// Nom en `data-text` (brouillage ASCII, décalage Glitch).
    pub data_text: bool,
}

/// Noms des cartes de la collection qu'un effet peut citer : en minuscules, avec le slug et le
/// type, les plus longs d'abord (« DROP DATABASE; » avant « DROP »). Les noms de moins de cinq
/// lettres sont ignorés, trop souvent des mots courants.
pub struct RefIndex(Vec<(String, String, CardType)>);

impl RefIndex {
    pub fn new(cards: &[Card]) -> Self {
        let mut v: Vec<(String, String, CardType)> = cards
            .iter()
            .filter(|c| c.draft.name.trim().chars().count() >= 5)
            .map(|c| {
                (
                    c.draft.name.trim().to_lowercase(),
                    c.slug.clone(),
                    c.draft.card_type,
                )
            })
            .collect();
        v.sort_by_key(|n| std::cmp::Reverse(n.0.len()));
        v.dedup_by(|a, b| a.0 == b.0);
        Self(v)
    }
}

fn esc_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Texte de l'effet en HTML, chaque carte citée (sauf la carte elle-même) entourée d'un lien ou
/// d'un `<span>` à la couleur de son type.
pub fn cite_html(text: &str, idx: &RefIndex, own: &str, link: bool) -> String {
    let own = own.trim().to_lowercase();
    let lower = text.to_lowercase();
    // `to_lowercase` peut changer les longueurs hors ASCII : on ne cherche les noms que si les
    // deux chaînes restent alignées.
    if lower.len() != text.len() {
        return esc_html(text);
    }
    let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
    let mut out = String::new();
    let mut i = 0;
    while i < text.len() {
        let hit = idx.0.iter().find(|(n, _, _)| {
            *n != own
                && lower[i..].starts_with(n.as_str())
                && text.is_char_boundary(i + n.len())
                && !word(text[..i].chars().next_back())
                && !word(text[i + n.len()..].chars().next())
        });
        if let Some((n, slug, t)) = hit {
            let shown = esc_html(&text[i..i + n.len()]);
            let t = t.slug();
            if link {
                out.push_str(&format!(
                    r#"<a class="card-ref" data-type="{t}" href="/cards/{slug}">{shown}</a>"#
                ));
            } else {
                out.push_str(&format!(
                    r#"<span class="card-ref" data-type="{t}">{shown}</span>"#
                ));
            }
            i += n.len();
            continue;
        }
        let ch = text[i..].chars().next().expect("indice sur une frontière");
        out.push_str(&esc_html(ch.encode_utf8(&mut [0; 4])));
        i += ch.len_utf8();
    }
    out
}

impl CardView {
    /// Met en avant les cartes citées dans l'effet si la carte le demande (`refs` = linked).
    /// `link` : les noms mènent aux cartes (fiche) ; sinon simple mise en couleur (la carte est
    /// déjà un lien dans la galerie).
    pub fn cite(mut self, idx: &RefIndex, link: bool) -> Self {
        if self.refs == "linked" {
            self.effect_html = Some(cite_html(&self.effect, idx, &self.name, link));
            if self.effect_style == "code" {
                // même rendu YAML, avec les noms des autres cartes à la couleur de leur type
                let own = self.name.trim().to_lowercase();
                let refs: Vec<(String, CardType)> = idx
                    .0
                    .iter()
                    .filter(|(n, _, _)| *n != own)
                    .map(|(n, _, t)| (n.clone(), *t))
                    .collect();
                let mut rows = crate::concepts::effect_rows(&self.effect, &refs);
                if let Some(first) = self.effect_rows.first()
                    && first.starts_with(r#"<li class="y-c">"#)
                {
                    rows.insert(0, first.clone());
                }
                self.effect_rows = rows;
            }
        }
        self
    }

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
            // nom sur l'illustration : le bloc du nom disparaît, le type passe en badge
            badge: if d.title == Title::Image {
                Badge::Image.slug()
            } else {
                d.badge.slug()
            },
            badge_label: d.badge.label(),
            title: d.title.slug(),
            title_label: d.title.label(),
            desc_style: d.desc_style.slug(),
            desc_style_label: d.desc_style.label(),
            relief: d.relief.slug(),
            background: d.background.slug(),
            effect_style: d.effect_style.slug(),
            relief_label: d.relief.label(),
            background_label: d.background.label(),
            effect_style_label: d.effect_style.label(),
            effect_file: if d.effect_file.trim().is_empty() {
                "effect.yaml".to_owned()
            } else {
                d.effect_file.trim().to_owned()
            },
            effect_rows: if d.effect_style == EffectStyle::Code {
                code_rows(d, &[])
            } else {
                Vec::new()
            },
            refs: d.refs.slug(),
            refs_label: d.refs.label(),
            description: d.description.clone(),
            effect: d.effect.clone(),
            effect_html: None,
            deploy_cost: d.deploy_cost,
            daily_cost: d.daily_cost,
            flavor_text: d.flavor_text.clone(),
            flavor_placeholder: slug.is_none() && d.flavor_text.trim().is_empty(),
            art_url: d.art.as_ref().map(|f| format!("/art/{f}")),
            fx: String::new(),
            ascii_word: None,
            data_text: false,
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

pub fn refs_choices() -> Vec<Choice> {
    Refs::ALL
        .iter()
        .map(|t| Choice {
            value: t.slug(),
            label: t.label(),
            color: None,
        })
        .collect()
}

pub fn effect_style_choices() -> Vec<Choice> {
    EffectStyle::ALL
        .iter()
        .map(|t| Choice {
            value: t.slug(),
            label: t.label(),
            color: None,
        })
        .collect()
}

/// Lignes YAML du bloc Effect façon code : la description en commentaire si elle y est placée
/// (`desc_style` = comment), puis une ligne par effet.
fn code_rows(d: &CardDraft, refs: &[(String, CardType)]) -> Vec<String> {
    let mut rows = crate::concepts::effect_rows(&d.effect, refs);
    if d.desc_style == DescStyle::Comment && !d.description.trim().is_empty() {
        rows.insert(
            0,
            format!(
                r#"<li class="y-c"><span class="y-comment"># {}</span></li>"#,
                esc_html(d.description.trim())
            ),
        );
    }
    rows
}

pub fn background_choices() -> Vec<Choice> {
    Background::ALL
        .iter()
        .map(|t| Choice {
            value: t.slug(),
            label: t.label(),
            color: None,
        })
        .collect()
}

pub fn relief_choices() -> Vec<Choice> {
    Relief::ALL
        .iter()
        .map(|t| Choice {
            value: t.slug(),
            label: t.label(),
            color: None,
        })
        .collect()
}

pub fn desc_style_choices() -> Vec<Choice> {
    DescStyle::ALL
        .iter()
        .map(|t| Choice {
            value: t.slug(),
            label: t.label(),
            color: None,
        })
        .collect()
}

pub fn title_choices() -> Vec<Choice> {
    Title::ALL
        .iter()
        .map(|t| Choice {
            value: t.slug(),
            label: t.label(),
            color: None,
        })
        .collect()
}

pub fn badge_choices() -> Vec<Choice> {
    Badge::ALL
        .iter()
        .map(|b| Choice {
            value: b.slug(),
            label: b.label(),
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

/// Une valeur cochable d'un menu de filtre.
pub struct FilterOption {
    pub value: &'static str,
    pub label: &'static str,
    /// Couleur du type (pastille), pour le menu Type.
    pub color: Option<&'static str>,
    pub checked: bool,
    /// Cartes obtenues en cochant cette valeur, avec les autres réglages déjà choisis.
    pub count: usize,
}

/// Un menu déroulant du panneau de filtres.
pub struct FilterGroup {
    pub key: &'static str,
    pub title: &'static str,
    /// Résumé dans le titre du menu : « Tous », la valeur choisie ou « 2 choisis ».
    pub summary: String,
    pub is_set: bool,
    pub open: bool,
    pub options: Vec<FilterOption>,
}

/// Carte de la galerie, avec les classes de son conteneur (skins de la documentation).
pub struct GalleryCard {
    pub card: CardView,
    pub wrap: String,
}

/// Un menu du panneau Affichage (un choix par réglage).
pub struct ViewGroup {
    pub key: &'static str,
    pub title: &'static str,
    pub summary: String,
    pub is_set: bool,
    pub open: bool,
    pub options: Vec<ViewOption>,
}

pub struct ViewOption {
    pub value: String,
    pub label: String,
    pub checked: bool,
    /// Sous-titre avant la première valeur d'un groupe (skins de la documentation).
    pub section: &'static str,
}

/// Filtre actif, affiché en pastille au-dessus des cartes ; le lien le retire.
pub struct ActiveFilter {
    pub title: &'static str,
    pub label: &'static str,
    pub href: String,
}

#[derive(Template)]
#[template(path = "gallery.html")]
pub struct GalleryPage {
    pub ctx: Ctx,
    pub cards: Vec<GalleryCard>,
    /// Panneau Affichage : réglages appliqués à toutes les cartes affichées.
    pub view_groups: Vec<ViewGroup>,
    pub view_set: bool,
    /// La même galerie, sans les réglages d'affichage.
    pub view_reset_href: String,
    /// Une skin de la documentation est appliquée : styles et scripts des skins à charger.
    pub uses_concepts: bool,
    pub groups: Vec<FilterGroup>,
    pub active: Vec<ActiveFilter>,
    pub filtered: bool,
    /// « Mes cartes » : la galerie ne montre que les cartes créées en ligne par ce compte.
    pub mine: bool,
    /// La même galerie sans « Mes cartes ».
    pub mine_off_href: String,
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
    pub badge: String,
    pub title: String,
    pub desc_style: String,
    pub relief: String,
    pub background: String,
    pub effect_style: String,
    pub effect_file: String,
    pub refs: String,
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
            badge: d.badge.slug().into(),
            title: d.title.slug().into(),
            desc_style: d.desc_style.slug().into(),
            relief: d.relief.slug().into(),
            background: d.background.slug().into(),
            effect_style: d.effect_style.slug().into(),
            effect_file: d.effect_file.clone(),
            refs: d.refs.slug().into(),
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
            badge: Badge::Text.slug().into(),
            title: Title::Block.slug().into(),
            desc_style: DescStyle::Classic.slug().into(),
            relief: Relief::Flat.slug().into(),
            background: Background::Plain.slug().into(),
            effect_style: EffectStyle::Text.slug().into(),
            effect_file: String::new(),
            refs: Refs::Plain.slug().into(),
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
    pub badges: Vec<Choice>,
    pub titles: Vec<Choice>,
    pub desc_styles: Vec<Choice>,
    pub reliefs: Vec<Choice>,
    pub backgrounds: Vec<Choice>,
    pub effect_styles: Vec<Choice>,
    pub refs_options: Vec<Choice>,
    pub lim_name: usize,
    pub lim_effect_file: usize,
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
            badges: badge_choices(),
            titles: title_choices(),
            desc_styles: desc_style_choices(),
            reliefs: relief_choices(),
            backgrounds: background_choices(),
            effect_styles: effect_style_choices(),
            refs_options: refs_choices(),
            lim_name: limits::NAME,
            lim_effect_file: limits::EFFECT_FILE,
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
    pub role: &'static str,
    pub role_label: &'static str,
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
        Self {
            id: u.id,
            username: u.username.clone(),
            role: u.role.slug(),
            role_label: u.role.label(),
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
    pub label: String,
    pub card: CardView,
    /// Classes du conteneur de la carte (skins de `concepts.css`).
    pub wrap: String,
    /// Description affichée dans la vue zoomée.
    pub desc: String,
}

/// Carte de la collection gardée en `<template>` : la vue zoomée en tire une au hasard quand on
/// change de catégorie.
pub struct TemplateCard {
    pub card: CardView,
    /// Description et effet remplis : tirée en priorité.
    pub filled: bool,
}

pub struct TileView {
    pub label: String,
    pub color: String,
    /// Membre de l'équipe : avatar rond avec une initiale tirée au hasard.
    pub avatar: bool,
    /// Noms tapés puis effacés, séparés par `|` (vide : libellé fixe).
    pub typing: String,
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

/// Une zone du schéma, avec son étiquette et son rectangle sur la carte.
pub struct AnatomyZoneView {
    pub n: usize,
    pub key: &'static str,
    pub title: String,
    pub field: String,
    pub text: String,
    pub rule_label: String,
    pub rule: String,
    pub left: bool,
    pub label_y: i32,
    pub target_x: i32,
    pub target_y: i32,
    pub rect: (i32, i32, i32, i32),
}

pub struct AnatomyLine {
    /// Numéro de la zone remplie par la ligne (0 : aucune).
    pub n: usize,
    pub zone: &'static str,
    pub html: String,
}

/// Partie « Anatomie d'une carte » : une carte de la collection, annotée.
pub struct AnatomyView {
    pub title: String,
    pub text: String,
    pub zones_title: String,
    pub zones_text: String,
    pub file_title: String,
    pub file_text: String,
    pub file_notes: Vec<String>,
    pub looks_title: String,
    pub looks_text: String,
    pub zones: Vec<AnatomyZoneView>,
    pub card: Option<CardView>,
    pub slug: String,
    pub lines: Vec<AnatomyLine>,
}

impl AnatomyView {
    pub fn new(a: &rules::Anatomy, card: Option<&Card>) -> Self {
        let zones: Vec<AnatomyZoneView> = a
            .zones
            .iter()
            .filter_map(|z| crate::anatomy::zone(&z.key).map(|g| (z, g)))
            .enumerate()
            .map(|(i, (z, g))| AnatomyZoneView {
                n: i + 1,
                key: g.key,
                title: z.title.clone(),
                field: z.field.clone(),
                text: inline(&z.text),
                rule_label: z.rule_label.clone(),
                rule: inline(&z.rule),
                left: g.left,
                label_y: g.label_y,
                target_x: g.target.0 + crate::anatomy::CARD_X,
                target_y: g.target.1 + crate::anatomy::CARD_Y,
                rect: g.rect,
            })
            .collect();
        let lines = card
            .map(|c| {
                crate::anatomy::toml_lines(&c.draft)
                    .into_iter()
                    .map(|l| AnatomyLine {
                        n: l.zone
                            .and_then(|k| zones.iter().find(|z| z.key == k))
                            .map_or(0, |z| z.n),
                        zone: l.zone.unwrap_or(""),
                        html: l.html,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            title: a.title.clone(),
            text: inline(&a.text),
            zones_title: a.zones_title.clone(),
            zones_text: inline(&a.zones_text),
            file_title: a.file_title.clone(),
            file_text: inline(&a.file_text),
            file_notes: a.file_notes.iter().map(|n| inline(n)).collect(),
            looks_title: a.looks_title.clone(),
            looks_text: inline(&a.looks_text),
            zones,
            // le schéma suit la disposition du Figma : nom dans son bloc avec le type, taille normale
            card: card.map(|c| {
                let mut d = c.draft.clone();
                d.badge = Badge::Text;
                d.title = Title::Block;
                d.desc_style = DescStyle::Classic;
                d.relief = Relief::Flat;
                d.background = Background::Plain;
                d.effect_style = EffectStyle::Text;
                d.refs = Refs::Plain;
                d.size = Size::Normal;
                CardView::new(Some(&c.slug), &d)
            }),
            slug: card.map(|c| c.slug.clone()).unwrap_or_default(),
            lines,
        }
    }
}

/// Contenu prêt à afficher, construit depuis `rules/regles.toml`.
pub struct RulesContent {
    pub draft_notice: String,
    pub title: String,
    pub context: Vec<String>,
    pub facts: Vec<(String, String)>,
    pub start_title: String,
    pub start_text: String,
    pub start_note: String,
    pub groups: Vec<TileGroupView>,
    pub victory_title: String,
    pub victory_text: String,
    pub victory_note: String,
    pub start_amount: u64,
    pub goal_amount: u64,
    pub start_label: String,
    pub goal_label: String,
    pub start_percent: String,
    pub types_title: String,
    pub types: Vec<RulesType>,
    pub anatomy: AnatomyView,
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
    pub skins_classic: Vec<FoilSample>,
    pub skins_animated: Vec<FoilSample>,
    pub templates: Vec<TemplateCard>,
    /// Boutons de catégorie de la vue zoomée.
    pub card_types: Vec<Choice>,
    pub shape_title: String,
    pub shape_text: String,
    pub shapes: Vec<FoilSample>,
    pub glossary: Option<TermsView>,
    pub economy: Option<TextView>,
    pub incidents: Option<TextView>,
    pub interactions: Option<TextView>,
    pub board: Option<BoardView>,
    pub first_game: Option<StepsView>,
    pub deck: Option<TextView>,
    pub create_card: Option<StepsView>,
    pub settings: Option<TermsView>,
    pub releases: Vec<ReleaseView>,
    pub road: Vec<RoadColumn>,
}

/// Exemples de la partie « Concepts », une carte par réglage.
pub struct ConceptSamples {
    /// Carte annotée de la partie « Anatomie d'une carte ».
    pub anatomy: Option<Card>,
    pub skins_classic: Vec<FoilSample>,
    pub skins_animated: Vec<FoilSample>,
    pub foils: Vec<FoilSample>,
    pub shapes: Vec<FoilSample>,
    pub templates: Vec<TemplateCard>,
}

#[derive(Template)]
#[template(path = "rules.html")]
pub struct RulesPage {
    pub ctx: Ctx,
    pub page: DocPage,
    pub nav: Vec<DocNavItem>,
    pub content: Option<RulesContent>,
    /// Erreur du dernier chargement de `rules/regles.toml` (les règles affichées sont alors
    /// les dernières valides).
    pub error: Option<String>,
}

impl RulesPage {
    pub fn missing(ctx: Ctx, error: Option<String>) -> Self {
        Self {
            ctx,
            page: DocPage::Overview,
            nav: Vec::new(),
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
            start_note: r.start.note.clone(),
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
                            avatar: t.card_type == CardType::Hr,
                            typing: t.typing.join("|"),
                        })
                        .collect(),
                })
                .collect(),
            victory_title: r.victory.title.clone(),
            victory_text: inline(&r.victory.text),
            victory_note: r.victory.note.clone(),
            start_amount: r.victory.start_amount,
            goal_amount: r.victory.goal_amount,
            start_label: rules::money(r.victory.start_amount),
            goal_label: rules::money(r.victory.goal_amount),
            // Point décimal même en locale française : c'est une valeur CSS.
            start_percent: format!("{percent:.2}"),
            types_title: r.types.title.clone(),
            types,
            anatomy: AnatomyView::new(&r.anatomy, samples.anatomy.as_ref()),
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
            skins_classic: samples.skins_classic,
            skins_animated: samples.skins_animated,
            templates: samples.templates,
            card_types: type_choices(),
            shape_title: r.concepts.shape.title.clone(),
            shape_text: inline(&r.concepts.shape.text),
            shapes: samples.shapes,
            glossary: r.glossary.as_ref().map(TermsView::new),
            economy: r.economy.as_ref().map(TextView::new),
            incidents: r.incidents.as_ref().map(TextView::new),
            interactions: r.interactions.as_ref().map(TextView::new),
            board: r.board.as_ref().map(|b| BoardView {
                eyebrow: b.eyebrow.clone(),
                title: b.title.clone(),
                draft: b.draft,
                text: inline(&b.text),
                zones: b
                    .zones
                    .iter()
                    .map(|z| (z.key.clone(), z.label.clone(), inline(&z.note)))
                    .collect(),
            }),
            first_game: r.first_game.as_ref().map(StepsView::new),
            deck: r.deck.as_ref().map(TextView::new),
            create_card: r.create_card.as_ref().map(StepsView::new),
            settings: r.settings.as_ref().map(TermsView::new),
            releases: r.releases.iter().map(ReleaseView::new).collect(),
            road: rules::ROAD_STATUSES
                .iter()
                .map(|&(key, title)| RoadColumn {
                    key,
                    title,
                    items: r
                        .roadmap
                        .iter()
                        .filter(|i| i.status == key)
                        .map(|i| RoadItemView {
                            title: i.title.clone(),
                            text: i.text.clone(),
                            release: i.release.clone(),
                        })
                        .collect(),
                })
                .collect(),
        };
        Self {
            ctx,
            page: DocPage::Overview,
            nav: Vec::new(),
            content: Some(content),
            error,
        }
    }

    /// Page de la documentation à afficher, et navigation de gauche.
    pub fn on(mut self, page: DocPage) -> Self {
        self.page = page;
        self.nav = DocPage::ALL
            .iter()
            .map(|&p| DocNavItem {
                label: p.label(),
                href: p.href(),
                current: p == page,
                sep_before: p == DocPage::Releases,
            })
            .collect();
        self
    }

    pub fn is(&self, key: &str) -> bool {
        self.page.key() == key
    }
}

/// Pages de la documentation, dans l'ordre de la navigation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DocPage {
    Overview,
    Basics,
    Core,
    Start,
    Releases,
    Roadmap,
}

impl DocPage {
    pub const ALL: [DocPage; 6] = [
        DocPage::Overview,
        DocPage::Basics,
        DocPage::Core,
        DocPage::Start,
        DocPage::Releases,
        DocPage::Roadmap,
    ];

    pub fn key(self) -> &'static str {
        match self {
            DocPage::Overview => "overview",
            DocPage::Basics => "basics",
            DocPage::Core => "core",
            DocPage::Start => "start",
            DocPage::Releases => "releases",
            DocPage::Roadmap => "roadmap",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DocPage::Overview => "Overview",
            DocPage::Basics => "Understand the Basics",
            DocPage::Core => "Core Concepts",
            DocPage::Start => "Getting Started",
            DocPage::Releases => "Releases",
            DocPage::Roadmap => "Roadmap",
        }
    }

    pub fn href(self) -> &'static str {
        match self {
            DocPage::Overview => "/regles",
            DocPage::Basics => "/regles/basics",
            DocPage::Core => "/regles/core-concepts",
            DocPage::Start => "/regles/getting-started",
            DocPage::Releases => "/regles/releases",
            DocPage::Roadmap => "/regles/roadmap",
        }
    }

    /// Page d'une adresse `/regles/<segment>`.
    pub fn from_segment(seg: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|p| p.href().rsplit('/').next() == Some(seg) && *p != DocPage::Overview)
    }
}

pub struct DocNavItem {
    pub label: &'static str,
    pub href: &'static str,
    pub current: bool,
    /// Trait de séparation avant l'entrée (Releases).
    pub sep_before: bool,
}

pub struct TextView {
    pub eyebrow: String,
    pub title: String,
    pub draft: bool,
    pub paragraphs: Vec<String>,
    pub flow: Vec<String>,
}

impl TextView {
    fn new(t: &rules::TextPart) -> Self {
        Self {
            eyebrow: t.eyebrow.clone(),
            title: t.title.clone(),
            draft: t.draft,
            paragraphs: t.text.iter().map(|p| inline(p)).collect(),
            flow: t.flow.clone(),
        }
    }
}

pub struct TermsView {
    pub eyebrow: String,
    pub title: String,
    pub draft: bool,
    /// Terme, puis définition en HTML.
    pub items: Vec<(String, String)>,
}

impl TermsView {
    fn new(t: &rules::Terms) -> Self {
        Self {
            eyebrow: t.eyebrow.clone(),
            title: t.title.clone(),
            draft: t.draft,
            items: t
                .items
                .iter()
                .map(|i| (i.term.clone(), inline(&i.text)))
                .collect(),
        }
    }
}

pub struct StepsView {
    pub eyebrow: String,
    pub title: String,
    pub draft: bool,
    pub items: Vec<(String, String)>,
}

impl StepsView {
    fn new(t: &rules::Steps) -> Self {
        Self {
            eyebrow: t.eyebrow.clone(),
            title: t.title.clone(),
            draft: t.draft,
            items: t
                .items
                .iter()
                .map(|i| (i.term.clone(), inline(&i.text)))
                .collect(),
        }
    }
}

pub struct BoardView {
    pub eyebrow: String,
    pub title: String,
    pub draft: bool,
    pub text: String,
    /// Clé de placement, libellé, précision en HTML.
    pub zones: Vec<(String, String, String)>,
}

pub struct ReleaseView {
    pub version: String,
    pub date: String,
    pub title: String,
    /// Rubrique (clé CSS, titre) et ses lignes, dans l'ordre Nouveau, Changé, Corrigé.
    pub groups: Vec<(&'static str, &'static str, Vec<String>)>,
}

impl ReleaseView {
    fn new(r: &rules::Release) -> Self {
        let groups = [
            ("new", "Nouveau", &r.new),
            ("changed", "Changé", &r.changed),
            ("fixed", "Corrigé", &r.fixed),
        ]
        .into_iter()
        .filter(|(_, _, items)| !items.is_empty())
        .map(|(k, t, items)| (k, t, items.iter().map(|i| inline(i)).collect()))
        .collect();
        Self {
            version: r.version.clone(),
            date: rules::french_date(&r.date).unwrap_or_else(|| r.date.clone()),
            title: r.title.clone(),
            groups,
        }
    }
}

pub struct RoadColumn {
    pub key: &'static str,
    pub title: &'static str,
    pub items: Vec<RoadItemView>,
}

pub struct RoadItemView {
    pub title: String,
    pub text: String,
    pub release: String,
}
