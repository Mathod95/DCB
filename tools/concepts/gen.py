"""Génère le document « Carte DCB v2 » (rules/concepts.html), affiché sur /regles/concepts.

Pistes de design pour la carte : contours, idées de modernisation, skins et skins animées,
dessinées avec les vraies cartes de cards/ et le card.css de l'app.

    python3 tools/concepts/gen.py            # écrit rules/concepts.html
    python3 tools/concepts/gen.py autre.html

Dépendance : Pillow (pip install pillow).
"""
import base64, html, io, re, sys, tomllib
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]  # racine du repo
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "rules/concepts.html"
CARD_CSS = (ROOT / "crates/dcb-server/static/card.css").read_text()

TYPES = {"resource": ["Resource"], "incident": ["Incident"], "hr": ["Ressource", "humaine"],
         "app": ["App"], "event": ["Event"]}
FOILS = {"basic": "Basic", "gold": "Gold"}


def art_uri(name):
    im = Image.open(ROOT / "cards/art" / name).convert("RGB")
    im.thumbnail((680, 680))
    buf = io.BytesIO()
    im.save(buf, "JPEG", quality=82)
    return "data:image/jpeg;base64," + base64.b64encode(buf.getvalue()).decode()


def load(slug, **over):
    c = tomllib.loads((ROOT / "cards" / f"{slug}.toml").read_text())
    c.update(over)
    c.setdefault("skin", "light")
    c.setdefault("foil", "basic")
    for k in ("description", "effect", "flavor_text"):
        c.setdefault(k, "")
    c["art_uri"] = art_uri(c["art"]) if c.get("art") else None
    return c


def name_size(n):
    return "l" if len(n) <= 18 else "m" if len(n) <= 24 else "s"


# ---------------------------------------------------------------------------------- YAML


NUM = re.compile(r"(\$[\d.,]+|[+x×]?\d+(?:[.,]\d+)?%?)")


def scalar(text):
    """Valeur YAML : texte simple, nombres et montants mis en avant."""
    out, last = [], 0
    for m in NUM.finditer(text):
        # pas de surlignage au milieu d'un mot (ex. « k8s »)
        if m.start() > 0 and text[m.start() - 1].isalpha():
            continue
        out.append(html.escape(text[last:m.start()]))
        out.append(f'<span class="y-num">{html.escape(m.group())}</span>')
        last = m.end()
    out.append(html.escape(text[last:]))
    return "".join(out)


KEY = re.compile(r"^([A-Za-zÀ-ÿ][\wÀ-ÿ' .-]{0,28}?):\s+(.*)$")


def yaml_lines(effect):
    """Chaque ligne du champ effect devient un élément de liste YAML."""
    items = [l.strip() for l in effect.strip().splitlines() if l.strip()]
    if not items:
        return ['<span class="y-dash">[] <span class="y-comment"># aucun effet</span></span>']
    rows = []
    for it in items:
        it = it[2:] if it.startswith("- ") else it.lstrip("-").strip()
        m = KEY.match(it)
        if m and m.group(2):
            body = f'<span class="y-key">{html.escape(m.group(1))}</span><span class="y-dash">:</span> {scalar(m.group(2))}'
        elif it.endswith(":"):
            body = f'<span class="y-key">{html.escape(it[:-1])}</span><span class="y-dash">:</span>'
        else:
            body = scalar(it)
        rows.append(f'<span class="y-dash">-</span><span class="y-val">{body}</span>')
    return rows


def effect_block_old(c):
    return f'''<div class="card-block card-effect">
    <p class="card-block-title">Effect</p>
    <p class="card-block-text">{html.escape(c["effect"].strip())}</p>
  </div>'''


def effect_block_new(c):
    lines = "".join(f'<li>{row}</li>' for row in yaml_lines(c["effect"]))
    return f'''<div class="card-block card-effect card-code">
    <div class="code-head"><span class="code-file">effect.yaml</span></div>
    <ol class="code-body">{lines}</ol>
  </div>'''


def effect_block_a_small(c):
    return effect_block_new(c).replace("card-code", "card-code r-small", 1)


def effect_block_a_corner(c):
    return effect_block_new(c).replace("card-code", "card-code r-corner", 1)


def effect_block_c(c):
    return effect_block_b(c).replace("card-yaml", "card-yaml is-tight", 1)


def effect_block_b(c):
    """Version B : pas de barre ni de numéros, une clé effect: puis la liste."""
    items = yaml_lines(c["effect"])
    if items[0].startswith('<span class="y-dash">[]'):
        head = '<span class="y-key">effect</span><span class="y-dash">:</span> <span class="y-dash">[]</span> <span class="y-comment"># aucun effet</span>'
        rows = ""
    else:
        head = '<span class="y-key">effect</span><span class="y-dash">:</span>'
        rows = "".join(f"<li>{r}</li>" for r in items)
    return f'''<div class="card-block card-effect card-yaml">
    <p class="yaml-root">{head}</p>
    <ul class="yaml-list">{rows}</ul>
  </div>'''


# ---------------------------------------------------------------------------------- carte


def card(c, variant):
    new = variant != "old"
    art = (f'<img src="{c["art_uri"]}" alt="" draggable="false">' if c["art_uri"] else
           '<svg class="card-art-empty" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="m21 17-5-5-9 8"/></svg>')
    flavor = html.escape(c["flavor_text"]) or "Texte d'ambiance"
    placeholder = "" if c["flavor_text"] else " is-placeholder"
    rarity = "" if new else f'''<div class="card-stat card-stat-rarity">
    <div class="card-stat-value">{FOILS[c["foil"]]}</div>
    <div class="card-stat-label">Rarity</div>
  </div>'''
    foil = '<div class="card-foil" aria-hidden="true"><i class="card-foil-glare"></i></div>' if c["foil"] != "basic" else ""
    return f'''<div class="card-frame"><div class="card{' v2' if new else ''}" data-type="{c["type"]}" data-skin="{c["skin"]}" data-foil="{c["foil"]}">
  <div class="card-skin"></div>
  <div class="card-block card-name" data-size="{name_size(c["name"])}"><span>{html.escape(c["name"])}</span></div>
  <div class="card-block card-art">{art}</div>
  <div class="card-block card-description">
    <p class="card-block-title">Description</p>
    <p class="card-block-text">{html.escape(c["description"])}</p>
  </div>
  { {'old': effect_block_old, 'a': effect_block_new, 'b': effect_block_b, 'c': effect_block_c, 'a-small': effect_block_a_small, 'a-corner': effect_block_a_corner}[variant](c) }
  <div class="card-stat card-stat-deploy">
    <div class="card-stat-value">{c["deploy_cost"]}</div>
    <div class="card-stat-label">Deploy Cost</div>
  </div>
  <div class="card-stat card-stat-daily">
    <div class="card-stat-value">{c["daily_cost"]}<small>$</small></div>
    <div class="card-stat-label">Daily Cost</div>
  </div>
  {rarity}
  <p class="card-flavor{placeholder}">{flavor}</p>
  <div class="card-stroke"></div>
  <div class="card-type">{"".join(f"<span>{t}</span>" for t in TYPES[c["type"]])}</div>
  {foil}
</div></div>'''


argo = load("argo-cd")
others = [
    ("Light, plusieurs effets", load("argo-cd")),
    ("Dark, même carte", load("argo-cd", skin="dark")),
    ("Effet avec une compétence", load("arnaud-h")),
    ("Effet sans tiret", load("certificate-expired")),
    ("Finition Gold", load("leopold-p")),
    ("Sans effet", load("alert-storm")),
]

def gallery(v):
    return "\n".join(
        f'<figure class="pick">{card(c, v)}<figcaption>{label}</figcaption></figure>' for label, c in others)

FONTS = [
    ("mixed", "Titre effect.yaml en gras 700", "Description et effect.yaml en Inter gras 14 px, seules les lignes de code restent en mono."),
    ("mixed-light", "Titre effect.yaml allégé, 600", "Même chose, mais effect.yaml passe en demi-gras (600) et en blanc adouci, pour compenser l'effet du texte clair sur fond sombre."),
]


STROKES = [
    ("solid", "Actuel", "Trait plein de 4 px, comme le calque cardTypeStroke du Figma. Pour comparer."),
    ("glow", "Néon", "Trait fin de 2 px avec un halo lumineux de la couleur du type, à l'intérieur et à l'extérieur."),
    ("gradient", "Dégradé", "Trait de 3 px qui passe de la couleur du type à une version éclaircie, en diagonale."),
    ("sweep", "Reflet qui tourne", "Un trait fin, avec une lumière de la couleur du type qui fait le tour de la carte en continu (6 s par tour)."),
    ("double", "Double filet", "Deux traits fins, un plein, un plus transparent juste à l'intérieur. Style carte premium."),
    ("corners", "Coins de viseur", "Seuls les quatre coins sont marqués, reliés par un filet très fin. Style interface, HUD."),
    ("chamfer", "Coins coupés", "Carte et trait aux coins biseautés au lieu d'arrondis. Style tech, terminal."),
    ("topbar", "Barre d'accent", "Filet fin tout autour, et une barre épaisse de la couleur du type en haut de la carte."),
    ("tinted", "Fond teinté", "Plus de trait épais, le fond de la carte prend une teinte du type, avec un filet fin."),
]


def stroke_sections():
    cards = [load("argo-cd"), load("certificate-expired"), load("arnaud-h")]
    out = []
    for key, title, text in STROKES:
        row = "".join(f'<div class="st st-{key} font-mixed-light">{card(c, "a-small")}</div>' for c in cards)
        out.append(f'''  <section class="stroke">
    <h2>{title}</h2>
    <p class="lead">{html.escape(text)}</p>
    <div class="gallery">{row}</div>
  </section>''')
    return "\n\n".join(out)


IDEAS = [
    ("artfill", "1. Cartes sans illustration", "Fond dans la couleur du type avec un motif de points et une grande icône du type, au lieu du cadre blanc et de l'icône grise. Argo CD garde son image.", ["artfill"], "base"),
    ("badge", "2. Badge de type", "Pastille de la couleur du type avec une icône, posée sur l'illustration. Le nom prend toute la largeur de son bloc. Les cinq types.", ["badge"], "types"),
    ("chips", "3. Coûts compacts", "Deux chips avec une icône (fusée, pièce) au lieu des grandes cases pastel. Les 25 px gagnés vont au bloc effect.yaml.", ["chips"], "base"),
    ("overlay", "4. Nom sur l'illustration", "Plus de bloc nom, le nom est écrit sur le bas de l'image, sur un dégradé sombre, et l'illustration grandit de 70 px. Le type passe en badge.", ["overlay", "badge"], "base"),
    ("notitle", "5a. Description sans titre", "Le titre Description disparaît, le texte commence en haut du bloc, ce qui donne une ligne de plus.", ["notitle"], "base"),
    ("comment", "5b. Description en commentaire YAML", "Le bloc Description disparaît, la description devient un commentaire en tête de effect.yaml, et le bloc gagne 70 px de haut.", ["comment"], "base"),
    ("serial", "6. Numéro de collection", "Un numéro en bas à droite, comme sur un jeu de cartes à collectionner (ici l'ordre alphabétique des fichiers, sur 86).", ["serial"], "base"),
    ("term", "7. Texte d'ambiance façon terminal", "Police mono, un $ de la couleur du type devant et un curseur qui clignote.", ["term"], "base"),
    ("refs", "8. Cartes citées dans les effets", "Le nom d'une autre carte du jeu cité dans un effet prend la couleur de son type et se souligne. Sur la fiche, il pourrait mener à cette carte.", ["refs"], "refs"),
    ("relief", "9. Relief de la peau claire", "Blocs blancs avec une ombre douce et une bordure plus légère, pour mieux les détacher du fond.", ["relief"], "base"),
    ("all", "Tout ensemble", "Les idées 1, 2, 3, 4, 5b, 6, 7, 8 et 9 combinées.", ["artfill", "badge", "chips", "overlay", "comment", "serial", "term", "refs", "relief"], "types"),
]
SETS = {
    "base": ["argo-cd", "certificate-expired", "arnaud-h"],
    "types": ["argo-cd", "certificate-expired", "arnaud-h", "bug-bounty", "waf"],
    "refs": ["argo-cd", "bug-bounty", "waf"],
}


def idea_sections():
    import gen_ideas
    build = gen_ideas.build(sys.modules[__name__])
    out = []
    for key, title, text, opts, set_name in IDEAS:
        row = "".join(f'<div class="st font-mixed-light">{build(load(slug), opts, slug)}</div>' for slug in SETS[set_name])
        out.append(f'''  <section class="stroke idea">
    <h2>{title}</h2>
    <p class="lead">{html.escape(text)}</p>
    <div class="gallery">{row}</div>
  </section>''')
    return "\n\n".join(out)


SKINS = [
    ("terminal", "Terminal", "Noir profond et vert phosphore, avec de fines lignes de balayage comme un vieil écran."),
    ("dracula", "Dracula", "Le thème d'éditeur bien connu : gris violacé, rose, violet et cyan."),
    ("solarized", "Solarized Light", "Fond crème chaud et texte ardoise, avec un bloc de code en Solarized Dark."),
    ("nord", "Nord", "Gris bleutés froids et accents givrés, calme et lisible."),
    ("blueprint", "Blueprint", "Plan d'architecte : fond bleu quadrillé, blocs transparents à bords pointillés, texte blanc."),
    ("synthwave", "Synthwave", "Dégradé violet nuit, rose et cyan néon, ambiance années 80."),
    ("glass", "Verre", "Fond sombre teinté par la couleur du type, blocs en verre dépoli translucides."),
    ("kraft", "Kraft", "Carton kraft et papier, avec un bloc de code sombre. Côté jeu de plateau."),
]


def skin_sections():
    cards = ["argo-cd", "certificate-expired", "arnaud-h"]
    out = [f'''  <section class="stroke skin">
    <h2>Actuelles, Light et Dark</h2>
    <p class="lead">Les deux skins qui existent déjà, pour comparer.</p>
    <div class="gallery">{"".join(f'<div class="st font-mixed-light">{card(load(sl, skin=sk), "a-small")}</div>' for sl, sk in [("argo-cd", "light"), ("argo-cd", "dark"), ("certificate-expired", "dark")])}</div>
  </section>''']
    for key, title, text in SKINS:
        row = "".join(f'<div class="st sk sk-{key} font-mixed-light">{card(load(sl), "a-small")}</div>' for sl in cards)
        out.append(f'''  <section class="stroke skin">
    <h2>{title}</h2>
    <p class="lead">{html.escape(text)}</p>
    <div class="gallery">{row}</div>
  </section>''')
    return "\n\n".join(out)


def crt_section():
    row = "".join(f'<div class="st sk sk-crt font-mixed-light">{card(load(sl), "a-small")}</div>'
                  for sl in ["argo-cd", "certificate-expired", "arnaud-h"])
    text = ("Écran cathodique inspiré des terminaux de Fallout. Tout passe au vert phosphore, illustration comprise. "
            "Police pixel pour les titres, texte qui luit, lignes de balayage qui défilent, bande lumineuse qui descend, "
            "léger scintillement, coins assombris comme un écran bombé, et un curseur qui clignote après le texte d'ambiance. "
            "Seul le contour garde la couleur du type.")
    return f'''  <section class="stroke skin">
    <h2>Terminal CRT, animé</h2>
    <p class="lead">{html.escape(text)}</p>
    <div class="gallery">{row}</div>
  </section>'''


DYNAMIC = [
    ("ascii", "ASCII", "Toute la carte en caractères. L'illustration est convertie en art ASCII (les cartes sans image affichent le sigle du type), avec des cadres en tirets et des crochets autour des valeurs. Des caractères changent au hasard en continu, et le nom se brouille puis se recompose au survol."),
    ("matrix", "Matrix", "Pluie de caractères verts qui tombe derrière des blocs sombres translucides, à une vitesse différente par colonne."),
    ("glitch", "Glitch", "Fond noir, magenta et cyan. Le nom et l'illustration se décalent en couleurs séparées par à-coups, avec une bande de parasites qui traverse la carte."),
    ("circuit", "Circuit", "Pistes de circuit imprimé dans la couleur du type, parcourues par des impulsions lumineuses."),
    ("monitoring", "Monitoring", "Fond de dashboard façon Grafana, avec une grille, trois courbes qui défilent, et une LED d'état qui pulse dans le bloc du nom (rouge pour un Incident)."),
    ("drive", "Synthwave Drive", "Soleil rayé et sol quadrillé en perspective qui défile vers l'avant, derrière des blocs translucides."),
    ("aurora", "Aurore", "Trois nappes de couleur floues, dont une de la couleur du type, qui dérivent lentement derrière des blocs en verre."),
]


def dynamic_sections():
    import gen_fx
    out = []
    for key, title, text in DYNAMIC:
        row = "".join(
            f'<div class="st sk dyn dyn-{key} font-mixed-light">{gen_fx.apply(key, card(load(sl), "a-small"), load(sl), sl)}</div>'
            for sl in ["argo-cd", "certificate-expired", "arnaud-h"])
        out.append(f'''  <section class="stroke skin dynamic">
    <h2>{title}</h2>
    <p class="lead">{html.escape(text)}</p>
    <div class="gallery">{row}</div>
  </section>''')
    return "\n\n".join(out)


def font_sections():
    sides = "".join(
        f'''<div class="side font-{key}"><span class="tag">{title}</span>{card(argo, "a-small")}<p class="note">{html.escape(text)}</p></div>'''
        for key, title, text in FONTS)
    return f'''  <section><div class="compare radius">{sides}</div></section>'''


page = (Path(__file__).parent / "template.html").read_text()
page = page.replace("/*CARD_CSS*/", CARD_CSS).replace("<!--BEFORE-->", card(argo, "old")) \
           .replace("<!--AFTER-A-->", card(argo, "a")).replace("<!--AFTER-B-->", card(argo, "b")).replace("<!--AFTER-C-->", card(argo, "c")) \
           .replace("<!--RADIUS-A-->", card(argo, "a")).replace("<!--RADIUS-SMALL-->", card(argo, "a-small")) \
           .replace("<!--RADIUS-CORNER-->", card(argo, "a-corner")) \
           .replace("<!--FONT-SECTIONS-->", font_sections()).replace("<!--STROKE-SECTIONS-->", stroke_sections()).replace("<!--IDEA-SECTIONS-->", idea_sections()).replace("<!--SKIN-SECTIONS-->", skin_sections() + "\n\n" + crt_section()).replace("<!--DYNAMIC-SECTIONS-->", dynamic_sections()) \
           .replace("/*DYN_CSS*/", (Path(__file__).parent / "dyn.css").read_text()) \
           .replace("/*DYN_JS*/", (Path(__file__).parent / "dyn.js").read_text()) \
           .replace("<!--RADIUS-CORNER-DARK-->", card(load("argo-cd", skin="dark"), "a-corner")) \
           .replace("<!--RADIUS-SMALL-DARK-->", card(load("argo-cd", skin="dark"), "a-small")) \
           .replace("<!--GALLERY-A-->", gallery("a")).replace("<!--GALLERY-B-->", gallery("b")).replace("<!--GALLERY-C-->", gallery("c"))
OUT.write_text(page)
print(OUT, len(page) // 1024, "Ko")
