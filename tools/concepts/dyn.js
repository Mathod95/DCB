// Skin ASCII : quelques caractères de l'illustration changent en continu, et le nom se brouille au survol.
// Les éléments sont relus à chaque tick, pour que les cartes clonées dans la fenêtre agrandie s'animent aussi.
(() => {
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  const RAMP = " .:-=+*#%@";
  setInterval(() => {
    document.querySelectorAll(".dyn-ascii .ascii-art").forEach((el) => {
      if (!el.dataset.base) el.dataset.base = el.textContent;
      const chars = el.dataset.base.split("");
      for (let i = 0; i < 40; i++) {
        const k = Math.floor(Math.random() * chars.length);
        if (chars[k] !== "\n" && chars[k] !== " ") chars[k] = RAMP[1 + Math.floor(Math.random() * (RAMP.length - 1))];
      }
      el.textContent = chars.join("");
    });
  }, 140);

  const GLYPHS = "!<>-_\\/[]{}=+*^?#01";
  const scramble = (span) => {
    const target = span.dataset.text;
    if (!target || span.dataset.busy) return;
    span.dataset.busy = "1";
    let frame = 0;
    const timer = setInterval(() => {
      frame++;
      span.textContent = [...target]
        .map((ch, i) => (ch === " " || i < frame / 2 ? ch : GLYPHS[Math.floor(Math.random() * GLYPHS.length)]))
        .join("");
      if (frame / 2 >= target.length) {
        clearInterval(timer);
        span.textContent = target;
        delete span.dataset.busy;
      }
    }, 35);
  };
  document.addEventListener("pointerover", (e) => {
    const card = e.target.closest?.(".dyn-ascii .card");
    if (!card || card.contains(e.relatedTarget)) return;
    const span = card.querySelector(".card-name span");
    if (span) scramble(span);
  });
  window.dcbScramble = scramble;
})();

// Fenêtre agrandie : clic sur une carte pour la voir en grand, inclinée et éclairée comme dans l'app.
(() => {
  const dialog = document.getElementById("card-modal");
  const stage = dialog.querySelector(".modal-stage");
  const caption = dialog.querySelector(".modal-caption");
  const counter = dialog.querySelector(".modal-count");
  const reduce = window.matchMedia("(prefers-reduced-motion: reduce)");
  const MAX_TILT = 12;
  let frames = [];
  let index = 0;

  const sourceFrames = () => [...document.querySelectorAll(".wrap .card-frame")];

  sourceFrames().forEach((f) => {
    f.tabIndex = 0;
    f.setAttribute("role", "button");
    f.setAttribute("aria-label", "Agrandir la carte " + (f.querySelector(".card-name")?.textContent.trim() || ""));
  });

  function scaleFor() {
    const h = (window.innerHeight - 150) / 574;
    const w = (window.innerWidth - 140) / 374;
    return Math.max(0.5, Math.min(h, w, 1.7));
  }

  function show(i) {
    index = (i + frames.length) % frames.length;
    const src = frames[index];
    const wrapper = src.parentElement;
    const holder = document.createElement("div");
    // reprend les classes du conteneur d'origine : skin, contour, police dépendent de lui
    holder.className = wrapper.className + " modal-holder";
    const clone = src.cloneNode(true);
    clone.removeAttribute("tabindex");
    clone.removeAttribute("role");
    clone.style.setProperty("--s", scaleFor().toFixed(3));
    const card = clone.querySelector(".card");
    const glare = document.createElement("i");
    glare.className = "modal-glare";
    card.append(glare);
    holder.append(clone);
    stage.replaceChildren(holder);

    const section = src.closest("section");
    const title = section?.querySelector("h2")?.textContent || "";
    const name = card.querySelector(".card-name")?.textContent.trim() || "";
    caption.textContent = title ? `${title} · ${name}` : name;
    counter.textContent = `${index + 1} / ${frames.length}`;
    track(clone, card);
    card.querySelectorAll(".card-name span[data-text]").forEach((s) => window.dcbScramble?.(s));
  }

  function track(frame, card) {
    const move = (e) => {
      if (reduce.matches) return;
      const r = frame.getBoundingClientRect();
      const x = Math.min(Math.max((e.clientX - r.left) / r.width, 0), 1);
      const y = Math.min(Math.max((e.clientY - r.top) / r.height, 0), 1);
      frame.classList.add("is-tracking");
      card.classList.add("is-lit");
      frame.style.setProperty("--rx", `${((0.5 - y) * 2 * MAX_TILT).toFixed(2)}deg`);
      frame.style.setProperty("--ry", `${((x - 0.5) * 2 * MAX_TILT).toFixed(2)}deg`);
      card.style.setProperty("--mx", `${(x * 100).toFixed(1)}%`);
      card.style.setProperty("--my", `${(y * 100).toFixed(1)}%`);
    };
    const leave = () => {
      frame.classList.remove("is-tracking");
      card.classList.remove("is-lit");
      frame.style.removeProperty("--rx");
      frame.style.removeProperty("--ry");
    };
    stage.onpointermove = move;
    stage.onpointerleave = leave;
  }

  function open(frame) {
    // toutes les cartes de la page, pour pouvoir passer d'une section à l'autre avec les flèches
    frames = sourceFrames();
    show(frames.indexOf(frame));
    dialog.showModal();
  }

  document.addEventListener("click", (e) => {
    const frame = e.target.closest?.(".wrap .card-frame");
    if (frame) open(frame);
  });
  document.addEventListener("keydown", (e) => {
    if (dialog.open) {
      if (e.key === "ArrowRight") show(index + 1);
      if (e.key === "ArrowLeft") show(index - 1);
      return;
    }
    const frame = e.target.closest?.(".wrap .card-frame");
    if (frame && (e.key === "Enter" || e.key === " ")) {
      e.preventDefault();
      open(frame);
    }
  });
  dialog.querySelector(".modal-prev").addEventListener("click", () => show(index - 1));
  dialog.querySelector(".modal-next").addEventListener("click", () => show(index + 1));
  dialog.querySelector(".modal-close").addEventListener("click", () => dialog.close());
  // clic sur le fond (hors carte et hors boutons) : fermeture
  dialog.addEventListener("click", (e) => {
    if (e.target === dialog || e.target === stage) dialog.close();
  });
  window.addEventListener("resize", () => {
    const clone = stage.querySelector(".card-frame");
    if (dialog.open && clone) clone.style.setProperty("--s", scaleFor().toFixed(3));
  });
})();
