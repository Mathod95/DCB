// Petites améliorations côté navigateur. Tout fonctionne sans ce fichier, il ne fait que du confort.
(() => {
  "use strict";

  // Compteurs de caractères sur les champs marqués data-count.
  function initCounters(root) {
    root.querySelectorAll("[data-count]").forEach((el) => {
      if (el.dataset.counted) return;
      el.dataset.counted = "1";
      const max = Number(el.getAttribute("maxlength"));
      const out = document.createElement("span");
      out.className = "counter";
      el.closest(".field")?.querySelector("label")?.append(out);
      const update = () => {
        const n = [...el.value].length;
        out.textContent = `${n}/${max}`;
        out.classList.toggle("is-near", n > max * 0.9);
      };
      el.addEventListener("input", update);
      update();
    });
  }

  // L'illustration choisie n'est envoyée qu'à l'enregistrement : on l'affiche dans l'aperçu
  // localement, et on la réapplique après chaque rafraîchissement htmx de l'aperçu.
  let localArt = null;

  function applyLocalArt() {
    // Après une navigation htmx, le champ fichier est neuf : l'ancienne image ne doit pas revenir.
    if (!localArt || !document.getElementById("f-art")?.files?.length) return;
    const box = document.querySelector("#preview .card-art");
    if (!box) return;
    let img = box.querySelector("img");
    if (!img) {
      box.replaceChildren();
      img = document.createElement("img");
      img.alt = "";
      box.append(img);
    }
    img.src = localArt;
  }

  document.addEventListener("change", (e) => {
    if (e.target.id !== "f-art") return;
    if (localArt) URL.revokeObjectURL(localArt);
    const file = e.target.files?.[0];
    localArt = file ? URL.createObjectURL(file) : null;
    applyLocalArt();
  });

  document.addEventListener("htmx:afterSwap", (e) => {
    if (e.detail.target.id === "preview") applyLocalArt();
  });

  document.addEventListener("htmx:load", (e) => initCounters(e.detail.elt));
  document.addEventListener("DOMContentLoaded", () => initCounters(document));

  // Reflet holographique : la carte survolée suit la souris (position du reflet + inclinaison).
  // Délégation sur document, donc valable aussi pour l'aperçu remplacé par htmx.
  const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
  const MAX_TILT = 9; // degrés

  function shiny(frame) {
    const card = frame?.querySelector(":scope > .card");
    if (!card) return null;
    const isShiny = card.dataset.foil && card.dataset.foil !== "basic";
    return isShiny ? card : null;
  }

  function release(frame) {
    frame.classList.remove("is-tracking");
    frame.style.removeProperty("--rx");
    frame.style.removeProperty("--ry");
    const card = frame.querySelector(":scope > .card");
    if (card) {
      card.classList.remove("is-tracking");
      card.style.removeProperty("--mx");
      card.style.removeProperty("--my");
    }
  }

  document.addEventListener("pointermove", (e) => {
    if (e.pointerType === "touch" || reduceMotion.matches) return;
    const frame = e.target.closest?.(".card-frame");
    const card = shiny(frame);
    if (!card) return;
    const r = frame.getBoundingClientRect();
    const x = Math.min(Math.max((e.clientX - r.left) / r.width, 0), 1);
    const y = Math.min(Math.max((e.clientY - r.top) / r.height, 0), 1);
    frame.classList.add("is-tracking");
    card.classList.add("is-tracking");
    frame.style.setProperty("--rx", `${((0.5 - y) * 2 * MAX_TILT).toFixed(2)}deg`);
    frame.style.setProperty("--ry", `${((x - 0.5) * 2 * MAX_TILT).toFixed(2)}deg`);
    card.style.setProperty("--mx", `${(x * 100).toFixed(1)}%`);
    card.style.setProperty("--my", `${(y * 100).toFixed(1)}%`);
  });

  document.addEventListener("pointerout", (e) => {
    const frame = e.target.closest?.(".card-frame");
    if (frame && !frame.contains(e.relatedTarget)) release(frame);
  });
})();
