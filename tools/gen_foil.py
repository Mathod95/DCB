#!/usr/bin/env python3
"""Génère les textures holographiques (tuiles SVG répétables) de crates/dcb-server/static/foil/.

Motifs blancs sur fond transparent : ils servent de masque, la couleur irisée vient du CSS.
Déterministe (graine fixe) : relancer le script redonne exactement les mêmes fichiers.
Usage : python3 tools/gen_foil.py
"""
import math
import random
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "crates/dcb-server/static/foil"


def svg(size, body):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" '
            f'viewBox="0 0 {size} {size}">{body}</svg>\n')


def wrap(size, x, y, draw):
    """Dessine aussi les copies décalées pour que la tuile se raccorde sans couture."""
    return "".join(draw(x + dx, y + dy) for dx in (-size, 0, size) for dy in (-size, 0, size)
                   if -12 < x + dx < size + 12 and -12 < y + dy < size + 12)


def sparkle(cx, cy, r, o):
    w = r * 0.22  # finesse des branches
    d = (f"M{cx:.1f} {cy - r:.1f}Q{cx + w:.1f} {cy - w:.1f} {cx + r:.1f} {cy:.1f}"
         f"Q{cx + w:.1f} {cy + w:.1f} {cx:.1f} {cy + r:.1f}"
         f"Q{cx - w:.1f} {cy + w:.1f} {cx - r:.1f} {cy:.1f}"
         f"Q{cx - w:.1f} {cy - w:.1f} {cx:.1f} {cy - r:.1f}Z")
    return f'<path d="{d}" fill="#fff" fill-opacity="{o:.2f}"/>'


def stars():
    rnd, size, parts = random.Random(7), 150, []
    for _ in range(16):
        x, y, r = rnd.uniform(0, size), rnd.uniform(0, size), rnd.choice([3, 4, 5, 6, 8, 9])
        o = rnd.uniform(0.75, 1)
        parts.append(wrap(size, x, y, lambda a, b: sparkle(a, b, r, o)))
    for _ in range(34):
        x, y, r = rnd.uniform(0, size), rnd.uniform(0, size), rnd.uniform(0.6, 1.7)
        o = rnd.uniform(0.5, 1)
        parts.append(wrap(size, x, y, lambda a, b: f'<circle cx="{a:.1f}" cy="{b:.1f}" r="{r:.2f}" fill="#fff" fill-opacity="{o:.2f}"/>'))
    return svg(size, "".join(parts))


def glitter():
    rnd, size, parts = random.Random(11), 120, []
    for _ in range(380):
        x, y = rnd.uniform(0, size), rnd.uniform(0, size)
        s, o = rnd.choice([1.0, 1.2, 1.5, 1.8, 2.2]), rnd.uniform(0.5, 1)
        parts.append(wrap(size, x, y, lambda a, b: f'<rect x="{a:.1f}" y="{b:.1f}" width="{s}" height="{s}" fill="#fff" fill-opacity="{o:.2f}"/>'))
    return svg(size, "".join(parts))


def shards():
    rnd, size, parts = random.Random(23), 220, []
    for _ in range(48):
        x, y = rnd.uniform(0, size), rnd.uniform(0, size)
        n, r = rnd.choice([3, 3, 3, 4]), rnd.uniform(3, 9)
        a0, o = rnd.uniform(0, math.tau), rnd.uniform(0.3, 1)
        pts = [(math.cos(a0 + i * math.tau / n + rnd.uniform(-0.6, 0.6)) * r * rnd.uniform(0.5, 1.3),
                math.sin(a0 + i * math.tau / n + rnd.uniform(-0.6, 0.6)) * r * rnd.uniform(0.5, 1.3))
               for i in range(n)]
        def draw(a, b, pts=pts, o=o):
            p = " ".join(f"{a + px:.1f},{b + py:.1f}" for px, py in pts)
            return f'<polygon points="{p}" fill="#fff" fill-opacity="{o:.2f}"/>'
        parts.append(wrap(size, x, y, draw))
    return svg(size, "".join(parts))


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    for name, fn in [("stars", stars), ("glitter", glitter), ("shards", shards)]:
        path = OUT / f"{name}.svg"
        path.write_text(fn())
        print(f"{path.relative_to(OUT.parents[2])}: {path.stat().st_size} octets")
