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
})();
