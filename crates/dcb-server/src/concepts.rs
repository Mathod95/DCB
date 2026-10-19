//! Page /regles/concepts : le document « Carte DCB v2 » (pistes de design pour la carte).
//!
//! Comme la page des règles, le texte vient d'un fichier versionné (`rules/concepts.toml`) et
//! les exemples sont de vraies cartes de la collection tirées au hasard à chaque affichage.
//! Chaque section est reconnue par sa `key`, qui choisit le rendu de ses cartes.

use std::fmt::Write as _;
use std::path::Path;

use dcb_core::{Card, CardType, Shape};
use serde::Deserialize;

use crate::views::CardView;

// ---------------------------------------------------------------------------------------------
// Fichier de texte

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConceptsDoc {
    pub title: String,
    pub intro: String,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PartKind {
    Shapes,
    Ideas,
    Skins,
    Dynamic,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub kind: PartKind,
    pub eyebrow: String,
    pub title: String,
    pub lead: String,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub key: String,
    pub title: String,
    pub text: String,
}

const IDEAS: &[&str] = &[
    "artfill", "badge", "chips", "overlay", "notitle", "comment", "serial", "term", "refs",
    "relief", "all",
];
const SKINS: &[&str] = &[
    "current",
    "terminal",
    "dracula",
    "solarized",
    "nord",
    "blueprint",
    "synthwave",
    "glass",
    "kraft",
    "crt",
];
const DYNAMIC: &[&str] = &[
    "ascii",
    "matrix",
    "glitch",
    "circuit",
    "monitoring",
    "drive",
    "aurora",
];

impl ConceptsDoc {
    pub fn parse(text: &str) -> Result<Self, String> {
        let doc: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        for part in &doc.parts {
            for s in &part.sections {
                let known = match part.kind {
                    PartKind::Shapes => s.key.parse::<Shape>().is_ok(),
                    PartKind::Ideas => IDEAS.contains(&s.key.as_str()),
                    PartKind::Skins => SKINS.contains(&s.key.as_str()),
                    PartKind::Dynamic => DYNAMIC.contains(&s.key.as_str()),
                };
                if !known {
                    return Err(format!(
                        "section « {} » : clé {:?} inconnue pour une partie {:?}",
                        s.title, s.key, part.kind
                    ));
                }
            }
        }
        Ok(doc)
    }

    pub async fn load(path: &Path) -> Result<Self, String> {
        let text = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

// ---------------------------------------------------------------------------------------------
// Rendu des cartes d'exemple

/// Variantes de la carte v2 envisagées dans la partie « idées ».
#[derive(Debug, Default, Clone, Copy)]
pub struct Opts {
    pub badge: bool,
    pub chips: bool,
    pub overlay: bool,
    pub notitle: bool,
    pub comment: bool,
    pub serial: bool,
    pub term: bool,
    pub refs: bool,
    pub relief: bool,
}

impl Opts {
    fn for_idea(key: &str) -> Self {
        let mut o = Self::default();
        match key {
            "badge" => o.badge = true,
            "chips" => o.chips = true,
            "overlay" => {
                o.overlay = true;
                o.badge = true;
            }
            "notitle" => o.notitle = true,
            "comment" => o.comment = true,
            "serial" => o.serial = true,
            "term" => o.term = true,
            "refs" => o.refs = true,
            "relief" => o.relief = true,
            "all" => {
                o = Self {
                    badge: true,
                    chips: true,
                    overlay: true,
                    notitle: false,
                    comment: true,
                    serial: true,
                    term: true,
                    refs: true,
                    relief: true,
                }
            }
            _ => {}
        }
        o
    }

    /// Classes CSS `o-…` posées sur la carte.
    fn classes(&self) -> String {
        let mut c = String::new();
        for (on, name) in [
            (self.badge, "badge"),
            (self.chips, "chips"),
            (self.overlay, "overlay"),
            (self.notitle, "notitle"),
            (self.comment, "comment"),
            (self.serial, "serial"),
            (self.term, "term"),
            (self.relief, "relief"),
        ] {
            if on {
                c.push_str(" o-");
                c.push_str(name);
            }
        }
        c
    }
}

/// Une carte de la page, prête pour `concept_card.html`.
pub struct ConceptCard {
    pub card: CardView,
    /// Classes du conteneur (skin, police…), dont dépend le CSS de la carte.
    pub wrap: String,
    /// Classes `o-…` posées sur la carte.
    pub opts_class: String,
    pub opts: Opts,
    /// Lignes de `effect.yaml`, déjà en HTML échappé.
    pub effect_rows: Vec<String>,
    /// Calque animé du fond (skins dynamiques), HTML.
    pub fx: String,
    /// Skin ASCII : l'illustration est calculée par le navigateur (`concepts.js`).
    pub ascii: bool,
    /// Sigle dessiné en ASCII quand la carte n'a pas d'illustration.
    pub ascii_word: &'static str,
    /// Nom en `data-text` (effet de brouillage et de glitch).
    pub data_text: bool,
    pub serial: String,
    pub type_label: String,
}

pub struct SectionView {
    pub title: String,
    pub text: String,
    pub cards: Vec<ConceptCard>,
}

pub struct PartView {
    pub eyebrow: String,
    pub title: String,
    pub lead: String,
    pub sections: Vec<SectionView>,
}

/// Cartes tirées au hasard pour une page.
pub struct Picks {
    /// Une carte App, une Incident, une Ressource humaine (celles qui existent).
    pub trio: Vec<Card>,
    /// Une carte par type.
    pub types: Vec<Card>,
    /// Cartes dont un effet cite une autre carte (repli : le trio).
    pub refs: Vec<Card>,
    /// Toute la collection : numéros de série et cartes citées.
    pub all: Vec<Card>,
}

pub fn build(doc: &ConceptsDoc, picks: &Picks) -> Vec<PartView> {
    let names = ref_names(&picks.all);
    let mut slugs: Vec<&str> = picks.all.iter().map(|c| c.slug.as_str()).collect();
    slugs.sort_unstable();
    let total = slugs.len();
    let serial = |slug: &str| {
        let n = slugs.binary_search(&slug).map_or(0, |i| i + 1);
        format!("DCB-{n:03}<span>/{total:03}</span>")
    };

    let make = |c: &Card, wrap: &str, opts: Opts, shape: Option<Shape>, fx_key: Option<&str>| {
        let mut draft = c.draft.clone();
        if let Some(s) = shape {
            draft.shape = s;
        }
        let refs = opts.refs.then(|| {
            names
                .iter()
                .filter(|(n, _)| *n != c.draft.name.to_lowercase())
        });
        let refs: Vec<(String, CardType)> = refs.map(|r| r.cloned().collect()).unwrap_or_default();
        let mut effect_rows = effect_rows(&draft.effect, &refs);
        if opts.comment && !draft.description.trim().is_empty() {
            effect_rows.insert(
                0,
                format!(
                    r#"<li class="y-c"><span class="y-comment"># {}</span></li>"#,
                    esc(draft.description.trim())
                ),
            );
        }
        let ascii = fx_key == Some("ascii");
        ConceptCard {
            type_label: draft.card_type.label().to_owned(),
            card: CardView::new(Some(&c.slug), &draft),
            wrap: wrap.to_owned(),
            opts_class: opts.classes(),
            opts,
            effect_rows,
            fx: fx_key.map(|k| fx(k, &c.slug)).unwrap_or_default(),
            ascii,
            ascii_word: ascii_word(draft.card_type),
            data_text: matches!(fx_key, Some("ascii" | "glitch")),
            serial: serial(&c.slug),
        }
    };

    doc.parts
        .iter()
        .map(|part| PartView {
            eyebrow: part.eyebrow.clone(),
            title: part.title.clone(),
            lead: part.lead.clone(),
            sections: part
                .sections
                .iter()
                .map(|s| {
                    let key = s.key.as_str();
                    let cards = match part.kind {
                        PartKind::Shapes => picks
                            .trio
                            .iter()
                            .map(|c| make(c, "", Opts::default(), key.parse().ok(), None))
                            .collect(),
                        PartKind::Ideas => {
                            let set = match key {
                                "badge" | "all" => &picks.types,
                                "refs" => &picks.refs,
                                _ => &picks.trio,
                            };
                            set.iter()
                                .map(|c| make(c, "", Opts::for_idea(key), None, None))
                                .collect()
                        }
                        PartKind::Skins if key == "current" => {
                            let mut v: Vec<ConceptCard> = Vec::new();
                            if let Some(c) = picks.trio.first() {
                                for skin in [dcb_core::Skin::Light, dcb_core::Skin::Dark] {
                                    let mut c = c.clone();
                                    c.draft.skin = skin;
                                    v.push(make(&c, "", Opts::default(), None, None));
                                }
                            }
                            if let Some(c) = picks.trio.get(1) {
                                let mut c = c.clone();
                                c.draft.skin = dcb_core::Skin::Dark;
                                v.push(make(&c, "", Opts::default(), None, None));
                            }
                            v
                        }
                        PartKind::Skins => picks
                            .trio
                            .iter()
                            .map(|c| {
                                let mut light = c.clone();
                                light.draft.skin = dcb_core::Skin::Light;
                                make(
                                    &light,
                                    &format!(" sk sk-{key}"),
                                    Opts::default(),
                                    None,
                                    None,
                                )
                            })
                            .collect(),
                        PartKind::Dynamic => picks
                            .trio
                            .iter()
                            .map(|c| {
                                let mut light = c.clone();
                                light.draft.skin = dcb_core::Skin::Light;
                                make(
                                    &light,
                                    &format!(" sk dyn dyn-{key}"),
                                    Opts::default(),
                                    None,
                                    Some(key),
                                )
                            })
                            .collect(),
                    };
                    SectionView {
                        title: s.title.clone(),
                        text: s.text.clone(),
                        cards,
                    }
                })
                .collect(),
        })
        .collect()
}

/// Noms de cartes repérables dans un effet (5 caractères au moins), avec leur type.
fn ref_names(all: &[Card]) -> Vec<(String, CardType)> {
    let mut v: Vec<(String, CardType)> = all
        .iter()
        .filter(|c| c.draft.name.chars().count() >= 5)
        .map(|c| (c.draft.name.to_lowercase(), c.draft.card_type))
        .collect();
    // les plus longs d'abord : « DROP DATABASE; » avant « DROP »
    v.sort_by_key(|n| std::cmp::Reverse(n.0.len()));
    v.dedup_by(|a, b| a.0 == b.0);
    v
}

/// Cartes de la collection qui citent une autre carte dans leur effet.
pub fn citing_cards(all: &[Card]) -> Vec<&Card> {
    let names = ref_names(all);
    all.iter()
        .filter(|c| {
            let effect = c.draft.effect.to_lowercase();
            let own = c.draft.name.to_lowercase();
            names
                .iter()
                .any(|(n, _)| *n != own && effect.contains(n.as_str()))
        })
        .collect()
}

fn ascii_word(t: CardType) -> &'static str {
    match t {
        CardType::Resource => "RES",
        CardType::Incident => "INC",
        CardType::Hr => "RH",
        CardType::App => "APP",
        CardType::Event => "EVT",
    }
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
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

// ---------------------------------------------------------------------------------------------
// effect.yaml : chaque ligne du champ devient un élément de liste YAML

/// Lignes HTML (`<li>…</li>`) du bloc `effect.yaml`.
pub fn effect_rows(effect: &str, refs: &[(String, CardType)]) -> Vec<String> {
    let items: Vec<&str> = effect
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if items.is_empty() {
        return vec![
            r#"<li><span class="y-dash">[] <span class="y-comment"># aucun effet</span></span></li>"#
                .to_owned(),
        ];
    }
    items
        .into_iter()
        .map(|item| {
            let item = item
                .strip_prefix("- ")
                .unwrap_or_else(|| item.trim_start_matches('-').trim_start());
            let body = match split_key(item) {
                Some((key, rest)) if !rest.is_empty() => format!(
                    r#"<span class="y-key">{}</span><span class="y-dash">:</span> {}"#,
                    esc(key),
                    scalar(rest, refs)
                ),
                _ if item.ends_with(':') => format!(
                    r#"<span class="y-key">{}</span><span class="y-dash">:</span>"#,
                    esc(&item[..item.len() - 1])
                ),
                _ => scalar(item, refs),
            };
            format!(r#"<li><span class="y-dash">-</span><span class="y-val">{body}</span></li>"#)
        })
        .collect()
}

/// `Compétence AppOfApps: texte` → (`Compétence AppOfApps`, `texte`). La clé commence par une
/// lettre, fait 30 caractères au plus et ne contient que lettres, chiffres, espaces et `'._-`.
fn split_key(item: &str) -> Option<(&str, &str)> {
    let (idx, _) = item
        .char_indices()
        .take(31)
        .find(|&(i, c)| c == ':' && item[i + 1..].starts_with(char::is_whitespace))?;
    let key = &item[..idx];
    let mut chars = key.chars();
    let first_ok = chars.next().is_some_and(char::is_alphabetic);
    let rest_ok = key
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '\'' | '.' | '_' | '-'));
    (first_ok && rest_ok).then(|| (key, item[idx + 1..].trim_start()))
}

/// Valeur YAML : texte simple, avec les cartes citées et les nombres mis en avant.
fn scalar(text: &str, refs: &[(String, CardType)]) -> String {
    let mut out = String::new();
    let mut plain = String::new();
    let lower = text.to_lowercase();
    // `to_lowercase` peut changer les longueurs hors ASCII : on ne cherche les noms que si
    // les deux chaînes restent alignées.
    let aligned = lower.len() == text.len();
    let mut i = 0;
    while i < text.len() {
        if aligned
            && let Some((name, t)) = refs.iter().find(|(n, _)| {
                lower[i..].starts_with(n.as_str()) && text.is_char_boundary(i + n.len())
            })
        {
            out.push_str(&numbers(&plain));
            plain.clear();
            let _ = write!(
                out,
                r#"<span class="y-ref" data-type="{}">{}</span>"#,
                t.slug(),
                esc(&text[i..i + name.len()])
            );
            i += name.len();
            continue;
        }
        let ch = text[i..].chars().next().expect("indice sur une frontière");
        plain.push(ch);
        i += ch.len_utf8();
    }
    out.push_str(&numbers(&plain));
    out
}

/// Met en avant les nombres, pourcentages, multiplicateurs et montants (`+1`, `50%`, `x2`,
/// `$2.000,00`), sauf collés à un mot (`k8s`).
fn numbers(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let prev_alpha = i > 0 && chars[i - 1].is_alphabetic();
        let start = i;
        let mut j = i;
        let is_money = chars[j] == '$' && chars.get(j + 1).is_some_and(char::is_ascii_digit);
        if !prev_alpha && is_money {
            j += 1;
            while j < chars.len() && (chars[j].is_ascii_digit() || matches!(chars[j], '.' | ',')) {
                j += 1;
            }
        } else if !prev_alpha
            && (chars[j].is_ascii_digit()
                || (matches!(chars[j], '+' | 'x' | '×')
                    && chars.get(j + 1).is_some_and(char::is_ascii_digit)))
        {
            if !chars[j].is_ascii_digit() {
                j += 1;
            }
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j + 1 < chars.len() && matches!(chars[j], '.' | ',') && chars[j + 1].is_ascii_digit()
            {
                j += 1;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
            }
            if j < chars.len() && chars[j] == '%' {
                j += 1;
            }
        }
        if j > start {
            let s: String = chars[start..j].iter().collect();
            let _ = write!(out, r#"<span class="y-num">{}</span>"#, esc(&s));
            i = j;
        } else {
            out.push_str(&esc(&chars[i].to_string()));
            i += 1;
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Calques animés des skins dynamiques

/// Générateur pseudo-aléatoire déterministe (xorshift), graine tirée du slug : une carte
/// garde le même dessin d'un affichage à l'autre.
struct Rng(u64);

impl Rng {
    fn new(seed: &str) -> Self {
        // FNV-1a
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in seed.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        Self(h | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Entier dans [lo, hi].
    fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % ((hi - lo + 1) as u64)) as i64
    }
    fn float(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (self.next() % 1_000_000) as f64 / 1_000_000.0 * (hi - lo)
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[(self.next() % v.len() as u64) as usize]
    }
}

fn fx(key: &str, slug: &str) -> String {
    let mut rng = Rng::new(&format!("{slug}{key}"));
    match key {
        "matrix" => matrix(&mut rng),
        "circuit" => circuit(&mut rng),
        "monitoring" => monitoring(&mut rng),
        "drive" => {
            r#"<div class="fx-sun"></div><div class="fx-floor"><div class="fx-grid"></div></div>"#
                .to_owned()
        }
        "aurora" => {
            r#"<i class="fx-blob b1"></i><i class="fx-blob b2"></i><i class="fx-blob b3"></i>"#
                .to_owned()
        }
        "glitch" => r#"<div class="fx-noise"></div>"#.to_owned(),
        _ => String::new(),
    }
}

fn matrix(rng: &mut Rng) -> String {
    const KATA: &[char] = &[
        'ｱ', 'ｲ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ', 'ｸ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ', 'ﾀ', 'ﾁ', 'ﾂ',
        'ﾃ', 'ﾄ', 'ﾅ', 'ﾆ', 'ﾇ', 'ﾈ', 'ﾉ', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '$',
        '<', '>', '{', '}', '#',
    ];
    let mut out = String::from(r#"<div class="fx-matrix">"#);
    for i in 0..18 {
        let text: String = (0..30).map(|_| *rng.pick(KATA)).collect();
        let _ = write!(
            out,
            r#"<span style="left:{}px;animation-duration:{:.1}s;animation-delay:-{:.1}s">{}</span>"#,
            i * 21 + rng.int(0, 6),
            rng.float(4.0, 9.0),
            rng.float(0.0, 9.0),
            esc(&text)
        );
    }
    out.push_str("</div>");
    out
}

fn circuit(rng: &mut Rng) -> String {
    let mut out = String::from(
        r#"<svg class="fx-circuit" viewBox="0 0 374 574" preserveAspectRatio="none">"#,
    );
    for _ in 0..14 {
        let (mut x, mut y) = (*rng.pick(&[0, 374]), rng.int(20, 554));
        let mut pts = vec![(x, y)];
        for _ in 0..rng.int(3, 5) {
            if pts.len() % 2 == 1 {
                x = (x + *rng.pick(&[-1, 1]) * rng.int(40, 140)).clamp(8, 366);
            } else {
                y = (y + *rng.pick(&[-1, 1]) * rng.int(30, 120)).clamp(8, 566);
            }
            pts.push((x, y));
        }
        let d = pts
            .iter()
            .enumerate()
            .map(|(i, (a, b))| format!("{}{a} {b}", if i == 0 { "M" } else { " L" }))
            .collect::<String>();
        let dur = rng.float(2.5, 5.0);
        let (lx, ly) = pts[pts.len() - 1];
        let _ = write!(
            out,
            r#"<path class="trace" d="{d}"/><path class="pulse" d="{d}" style="animation-duration:{dur:.1}s;animation-delay:-{:.1}s"/><circle class="node" cx="{lx}" cy="{ly}" r="3"/>"#,
            rng.float(0.0, dur)
        );
    }
    out.push_str("</svg>");
    out
}

fn monitoring(rng: &mut Rng) -> String {
    let mut line = |base: f64, amp: f64| {
        let mut ys = Vec::with_capacity(26);
        let mut y = base;
        for _ in 0..25 {
            y = (y + rng.float(-amp / 2.0, amp / 2.0)).clamp(base - amp, base + amp);
            ys.push(y);
        }
        ys.push(ys[0]);
        // deux périodes à la suite, pour un défilement sans couture
        let mut pts = String::new();
        for offset in [0.0, 374.0] {
            for (i, v) in ys.iter().enumerate() {
                let _ = write!(pts, "{:.0},{v:.0} ", offset + i as f64 * 374.0 / 24.0);
            }
        }
        pts
    };
    let (l1, l2, l3) = (line(150.0, 60.0), line(330.0, 50.0), line(500.0, 40.0));
    format!(
        r#"<div class="fx-mon"><svg viewBox="0 0 748 574" preserveAspectRatio="none"><polyline class="l1" points="{l1}"/><polyline class="l2" points="{l2}"/><polyline class="l3" points="{l3}"/></svg></div>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_lines_become_a_yaml_list() {
        let rows = effect_rows(
            "- Ajoute +1 deployment MAX\nCompétence Honnête: x2 de revenus",
            &[],
        );
        assert_eq!(rows.len(), 2);
        assert!(
            rows[0].contains(r#"<span class="y-num">+1</span>"#),
            "{}",
            rows[0]
        );
        assert!(rows[1].contains(r#"<span class="y-key">Compétence Honnête</span>"#));
        assert!(rows[1].contains(r#"<span class="y-num">x2</span>"#));
        assert!(effect_rows("", &[])[0].contains("aucun effet"));
        // pas de nombre au milieu d'un mot, montants entiers
        assert!(!effect_rows("- k8s", &[])[0].contains("y-num"));
        assert!(effect_rows("- Payez $2.000,00", &[])[0].contains(">$2.000,00<"));
        // HTML échappé
        assert!(effect_rows("- <b>", &[])[0].contains("&lt;b&gt;"));
    }

    #[test]
    fn cited_cards_are_highlighted() {
        let refs = vec![("application set".to_owned(), CardType::App)];
        let rows = effect_rows("- Débloque la carte Application Set", &refs);
        assert!(
            rows[0].contains(r#"<span class="y-ref" data-type="app">Application Set</span>"#),
            "{}",
            rows[0]
        );
    }

    #[test]
    fn real_file_parses_and_unknown_keys_fail() {
        let doc = ConceptsDoc::parse(include_str!("../../../rules/concepts.toml")).unwrap();
        assert_eq!(doc.parts.len(), 4);
        let bad = "title = \"t\"\nintro = \"i\"\n[[parts]]\nkind = \"ideas\"\neyebrow = \"e\"\ntitle = \"t\"\nlead = \"l\"\n[[parts.sections]]\nkey = \"oups\"\ntitle = \"T\"\ntext = \"x\"\n";
        assert!(ConceptsDoc::parse(bad).unwrap_err().contains("oups"));
    }

    #[test]
    fn fx_is_stable_per_card() {
        assert_eq!(fx("matrix", "argo-cd"), fx("matrix", "argo-cd"));
        assert_ne!(fx("circuit", "argo-cd"), fx("circuit", "backup"));
        assert!(fx("monitoring", "a").contains("<polyline"));
    }
}
