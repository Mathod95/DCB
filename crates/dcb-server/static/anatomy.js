// Anatomie d'une carte (page des règles) : traits reliant les étiquettes à leur zone, zone
// surlignée au survol d'une étiquette, d'une fiche ou d'une ligne du fichier, bascule Carte /
// Squelette.
(() => {
  const root = document.querySelector(".anat");
  if (!root) return;
  const stage = root.querySelector(".anat-stage");
  const svg = root.querySelector(".anat-leaders");
  const NS = "http://www.w3.org/2000/svg";
  const CARD_X = 273, CARD_W = 374;

  root.querySelectorAll(".anat-pin").forEach((pin) => {
    const left = pin.classList.contains("is-left");
    const ly = +pin.dataset.ly, tx = +pin.dataset.tx, ty = +pin.dataset.ty;
    const lx = left ? 246 : 674;
    const ex = left ? CARD_X - 14 : CARD_X + CARD_W + 14;
    const g = document.createElementNS(NS, "g");
    g.dataset.zone = pin.dataset.zone;
    const path = document.createElementNS(NS, "path");
    path.setAttribute("d", ly === ty ? `M${lx} ${ly} H${tx}` : `M${lx} ${ly} H${ex} L${tx} ${ty}`);
    const dot = document.createElementNS(NS, "circle");
    dot.setAttribute("cx", tx);
    dot.setAttribute("cy", ty);
    dot.setAttribute("r", 4.5);
    g.append(path, dot);
    svg?.append(g);
  });

  const all = () => root.querySelectorAll("[data-zone]");
  const activate = (z) => all().forEach((el) => el.classList.toggle("is-active", !!z && el.dataset.zone === z));
  root.querySelectorAll(".anat-pin, .anat-item, .anat-code li[data-zone]").forEach((el) => {
    el.addEventListener("mouseenter", () => activate(el.dataset.zone));
    el.addEventListener("mouseleave", () => activate(null));
    el.addEventListener("focus", () => activate(el.dataset.zone));
    el.addEventListener("blur", () => activate(null));
  });

  const real = root.querySelector(".anat-real");
  const skel = root.querySelector(".anat-skeleton");
  const btns = root.querySelectorAll(".anat-switch button");
  btns.forEach((b) => b.addEventListener("click", () => {
    const sk = b.dataset.view === "skeleton";
    if (real) real.hidden = sk;
    if (skel) skel.hidden = !sk;
    btns.forEach((o) => o.setAttribute("aria-pressed", String(o === b)));
  }));
  if (!stage) return;
})();
