"""Skins dynamiques : calques animés injectés dans la carte (fond, illustration ASCII)."""
import html
import random
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont, ImageOps

ROOT = Path(__file__).resolve().parents[2]  # racine du repo
FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
RAMP = " .:-=+*#%@"
COLS, ROWS = 56, 20  # bloc illustration 336 × 180 px


def to_ascii(im):
    im = ImageOps.equalize(im.convert("L").resize((COLS, ROWS)))
    px = im.load()
    return "\n".join("".join(RAMP[px[x, y] * (len(RAMP) - 1) // 255] for x in range(COLS)) for y in range(ROWS))


def ascii_art(c):
    if c.get("art"):
        return to_ascii(Image.open(ROOT / "cards/art" / c["art"]))
    # pas d'illustration : le sigle du type en grosses lettres
    word = {"resource": "RES", "incident": "INC", "hr": "RH", "app": "APP", "event": "EVT"}[c["type"]]
    im = Image.new("L", (COLS * 10, ROWS * 18), 0)
    d = ImageDraw.Draw(im)
    f = ImageFont.truetype(FONT, 250)
    box = d.textbbox((0, 0), word, font=f)
    d.text(((im.width - box[2]) / 2 - box[0], (im.height - box[3]) / 2 - box[1]), word, font=f, fill=255)
    return to_ascii(im)


KATA = "ｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄﾅﾆﾇﾈﾉ0123456789$<>{}#"


def matrix(rng):
    cols = []
    for i in range(18):
        text = "".join(rng.choice(KATA) for _ in range(30))
        cols.append(
            f'<span style="left:{i * 21 + rng.randint(0, 6)}px;animation-duration:{rng.uniform(4, 9):.1f}s;'
            f'animation-delay:-{rng.uniform(0, 9):.1f}s">{text}</span>')
    return f'<div class="fx-matrix">{"".join(cols)}</div>'


def circuit(rng):
    paths = []
    for _ in range(14):
        x, y = rng.choice([0, 374]), rng.randint(20, 554)
        pts = [(x, y)]
        for _ in range(rng.randint(3, 5)):
            if len(pts) % 2:
                x = max(8, min(366, x + rng.choice([-1, 1]) * rng.randint(40, 140)))
            else:
                y = max(8, min(566, y + rng.choice([-1, 1]) * rng.randint(30, 120)))
            pts.append((x, y))
        d = "M" + " L".join(f"{a} {b}" for a, b in pts)
        dur = rng.uniform(2.5, 5)
        paths.append(f'<path class="trace" d="{d}"/><path class="pulse" d="{d}" style="animation-duration:{dur:.1f}s;'
                     f'animation-delay:-{rng.uniform(0, dur):.1f}s"/><circle class="node" cx="{pts[-1][0]}" cy="{pts[-1][1]}" r="3"/>')
    return f'<svg class="fx-circuit" viewBox="0 0 374 574" preserveAspectRatio="none">{"".join(paths)}</svg>'


def monitoring(rng):
    def line(base, amp):
        ys, y = [], base
        for _ in range(25):
            y = max(base - amp, min(base + amp, y + rng.uniform(-amp / 2, amp / 2)))
            ys.append(y)
        ys += ys[:1]
        pts = " ".join(f"{i * 374 / 24:.0f},{v:.0f}" for i, v in enumerate(ys))
        pts2 = " ".join(f"{374 + i * 374 / 24:.0f},{v:.0f}" for i, v in enumerate(ys))
        return pts + " " + pts2

    return (f'<div class="fx-mon"><svg viewBox="0 0 748 574" preserveAspectRatio="none">'
            f'<polyline class="l1" points="{line(150, 60)}"/><polyline class="l2" points="{line(330, 50)}"/>'
            f'<polyline class="l3" points="{line(500, 40)}"/></svg></div>')


def fx_markup(key, c, slug):
    rng = random.Random(slug + key)
    return {
        "matrix": matrix(rng),
        "circuit": circuit(rng),
        "monitoring": monitoring(rng),
        "drive": '<div class="fx-sun"></div><div class="fx-floor"><div class="fx-grid"></div></div>',
        "aurora": '<i class="fx-blob b1"></i><i class="fx-blob b2"></i><i class="fx-blob b3"></i>',
        "glitch": '<div class="fx-noise"></div>',
    }.get(key, "")


def apply(key, html_card, c, slug):
    html_card = html_card.replace('<div class="card-skin"></div>', f'<div class="card-skin">{fx_markup(key, c, slug)}</div>', 1)
    if key == "ascii":
        pre = f'<pre class="ascii-art">{html.escape(ascii_art(c))}</pre>'
        start = html_card.index('<div class="card-block card-art">') + len('<div class="card-block card-art">')
        end = html_card.index("</div>", start)
        html_card = html_card[:start] + pre + html_card[end:]
    if key in ("glitch", "ascii"):
        html_card = html_card.replace('<span>' + html.escape(c["name"]) + '</span>',
                                      f'<span data-text="{html.escape(c["name"])}">{html.escape(c["name"])}</span>', 1)
    return html_card
