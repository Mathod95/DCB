"""Sections « idées » de la maquette : une carte reconstruite avec des options."""
import glob, html, re, tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]  # racine du repo

ICONS = {
    # serveur
    "resource": '<rect x="3" y="4" width="18" height="7" rx="2"/><rect x="3" y="13" width="18" height="7" rx="2"/><path d="M7 7.5h.01M7 16.5h.01M11 7.5h6M11 16.5h6"/>',
    # alerte
    "incident": '<path d="M12 3 2 20h20L12 3z"/><path d="M12 10v4M12 17h.01"/>',
    # personne
    "hr": '<circle cx="12" cy="8" r="4"/><path d="M4 21c0-4.4 3.6-7 8-7s8 2.6 8 7"/>',
    # cube
    "app": '<path d="M12 2 3 7v10l9 5 9-5V7l-9-5z"/><path d="M3 7l9 5 9-5M12 12v10"/>',
    # calendrier
    "event": '<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M3 10h18M8 3v4M16 3v4"/>',
    "rocket": '<path d="M5 15c-1.5 1.5-2 5-2 5s3.5-.5 5-2"/><path d="M9 15l-3-3c1-3 3-6 6.5-8.5C15 2 19 2 21 3c1 2 1 6-.5 8.5C18 15 15 17 12 18l-3-3z"/><circle cx="15" cy="9" r="1.5"/>',
    "coin": '<circle cx="12" cy="12" r="9"/><path d="M14.5 9.5c-.5-1-1.5-1.5-2.5-1.5-1.5 0-2.5.8-2.5 2s1 1.6 2.5 2 2.5.8 2.5 2-1 2-2.5 2c-1 0-2-.5-2.5-1.5M12 6.5v1.5M12 16v1.5"/>',
}


def icon(name, cls=""):
    return (f'<svg class="{cls}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" '
            f'stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{ICONS[name]}</svg>')


ALL = {}
for f in sorted(glob.glob(str(ROOT / "cards/*.toml"))):
    d = tomllib.load(open(f, "rb"))
    ALL[Path(f).stem] = d
SERIAL = {slug: i + 1 for i, slug in enumerate(sorted(ALL))}
REF_TYPES = {d["name"].lower(): d["type"] for d in ALL.values() if len(d["name"]) >= 5}


def build(g):
    """g : le module gen (scalar, yaml_lines, TYPES, name_size, html)."""

    def effect(c, o):
        rows = g.yaml_lines(c["effect"])
        if "refs" in o:
            own = c["name"].lower()
            # on refait les lignes avec la détection des cartes citées
            original = g.scalar
            def patched(t, _o=original):
                return scalar_refs_plain(t, own, _o)
            g.scalar = patched
            rows = g.yaml_lines(c["effect"])
            g.scalar = original
        comment = ""
        if "comment" in o and c["description"].strip():
            comment = f'<li class="y-c"><span class="y-comment"># {html.escape(c["description"].strip())}</span></li>'
        lines = comment + "".join(f"<li>{r}</li>" for r in rows)
        return f'''<div class="card-block card-effect card-code r-small">
    <div class="code-head"><span class="code-file">effect.yaml</span></div>
    <ol class="code-body">{lines}</ol>
  </div>'''

    def scalar_refs_plain(text, own, base):
        names = sorted((n for n in REF_TYPES if n != own), key=len, reverse=True)
        pat = re.compile("(" + "|".join(re.escape(n) for n in names) + ")", re.I)
        out, last = [], 0
        for m in pat.finditer(text):
            out.append(base(text[last:m.start()]))
            t = REF_TYPES[m.group().lower()]
            out.append(f'<span class="y-ref" data-type="{t}">{html.escape(m.group())}</span>')
            last = m.end()
        out.append(base(text[last:]))
        return "".join(out)

    def card(c, o, slug):
        o = set(o)
        cls = " ".join("o-" + x for x in sorted(o))
        if c["art_uri"]:
            art = f'<img src="{c["art_uri"]}" alt="" draggable="false">'
        elif "artfill" in o:
            art = f'<div class="art-fill">{icon(c["type"], "art-fill-icon")}</div>'
        else:
            art = ('<svg class="card-art-empty" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true">'
                   '<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="m21 17-5-5-9 8"/></svg>')
        shade = '<div class="art-shade"></div>' if "overlay" in o else ""
        label = " ".join(g.TYPES[c["type"]])
        if "badge" in o:
            type_html = f'<div class="type-pill">{icon(c["type"])}<span>{label}</span></div>'
        else:
            type_html = f'<div class="card-type">{"".join(f"<span>{t}</span>" for t in g.TYPES[c["type"]])}</div>'
        if "chips" in o:
            stats = f'''<div class="card-stat card-stat-deploy"><i class="stat-ico">{icon("rocket")}</i><div class="card-stat-value">{c["deploy_cost"]}</div><div class="card-stat-label">Deploy Cost</div></div>
  <div class="card-stat card-stat-daily"><i class="stat-ico">{icon("coin")}</i><div class="card-stat-value">{c["daily_cost"]}<small>$</small></div><div class="card-stat-label">Daily Cost</div></div>'''
        else:
            stats = f'''<div class="card-stat card-stat-deploy"><div class="card-stat-value">{c["deploy_cost"]}</div><div class="card-stat-label">Deploy Cost</div></div>
  <div class="card-stat card-stat-daily"><div class="card-stat-value">{c["daily_cost"]}<small>$</small></div><div class="card-stat-label">Daily Cost</div></div>'''
        desc = ""
        if "comment" not in o:
            title = "" if "notitle" in o else '<p class="card-block-title">Description</p>'
            desc = f'''<div class="card-block card-description">{title}<p class="card-block-text">{html.escape(c["description"])}</p></div>'''
        flavor_text = c["flavor_text"] or "Texte d'ambiance"
        ph = "" if c["flavor_text"] else " is-placeholder"
        if "term" in o:
            flavor = f'<p class="card-flavor{ph}"><b>$</b> {html.escape(flavor_text)}<i class="cursor"></i></p>'
        else:
            flavor = f'<p class="card-flavor{ph}">{html.escape(flavor_text)}</p>'
        serial = (f'<p class="card-serial">DCB-{SERIAL[slug]:03d}<span>/{len(SERIAL):03d}</span></p>'
                  if "serial" in o else "")
        return f'''<div class="card-frame"><div class="card v2 {cls}" data-type="{c["type"]}" data-skin="{c["skin"]}" data-foil="{c["foil"]}">
  <div class="card-skin"></div>
  <div class="card-block card-name" data-size="{g.name_size(c["name"])}"><span>{html.escape(c["name"])}</span></div>
  <div class="card-block card-art">{art}{shade}</div>
  {desc}
  {effect(c, o)}
  {stats}
  {flavor}{serial}
  <div class="card-stroke"></div>
  {type_html}
</div></div>'''

    return card
