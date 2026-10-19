// Skin ASCII : l'illustration (ou, sans image, le sigle du type) est convertie en caractères
// dans le navigateur, sur un canvas. Remplit les <pre class="ascii-art" data-src data-word>.
(() => {
  const RAMP = " .:-=+*#%@";
  const COLS = 56, ROWS = 20;

  function toAscii(ctx) {
    const { data } = ctx.getImageData(0, 0, COLS, ROWS);
    const lum = [];
    for (let i = 0; i < data.length; i += 4) lum.push(0.299 * data[i] + 0.587 * data[i + 1] + 0.114 * data[i + 2]);
    // égalisation d'histogramme : les images sombres gardent du relief
    const sorted = [...lum].sort((a, b) => a - b);
    const rank = (v) => {
      let lo = 0, hi = sorted.length;
      while (lo < hi) { const m = (lo + hi) >> 1; if (sorted[m] < v) lo = m + 1; else hi = m; }
      return lo / sorted.length;
    };
    let out = "";
    lum.forEach((v, i) => {
      out += RAMP[Math.min(RAMP.length - 1, Math.floor(rank(v) * RAMP.length))];
      if ((i + 1) % COLS === 0 && i + 1 < lum.length) out += "\n";
    });
    return out;
  }

  function fromWord(word) {
    const big = document.createElement("canvas");
    big.width = COLS * 10; big.height = ROWS * 18;
    const b = big.getContext("2d");
    b.fillStyle = "#000"; b.fillRect(0, 0, big.width, big.height);
    b.fillStyle = "#fff"; b.font = "bold 250px Inter, system-ui, sans-serif";
    b.textAlign = "center"; b.textBaseline = "middle";
    b.fillText(word, big.width / 2, big.height / 2 + 10);
    const small = document.createElement("canvas");
    small.width = COLS; small.height = ROWS;
    const ctx = small.getContext("2d", { willReadFrequently: true });
    ctx.drawImage(big, 0, 0, COLS, ROWS);
    // le fond noir doit rester vide : pas d'égalisation pour le sigle
    const { data } = ctx.getImageData(0, 0, COLS, ROWS);
    let out = "";
    for (let i = 0; i < data.length; i += 4) {
      out += RAMP[Math.min(RAMP.length - 1, Math.floor(data[i] / 256 * RAMP.length))];
      if ((i / 4 + 1) % COLS === 0 && i / 4 + 1 < COLS * ROWS) out += "\n";
    }
    return out;
  }

  document.querySelectorAll(".ascii-art").forEach((pre) => {
    const fill = (text) => { pre.textContent = text; pre.dataset.base = text; };
    const src = pre.dataset.src;
    if (!src) { fill(fromWord(pre.dataset.word || "?")); return; }
    const img = new Image();
    img.onload = () => {
      const c = document.createElement("canvas");
      c.width = COLS; c.height = ROWS;
      const ctx = c.getContext("2d", { willReadFrequently: true });
      ctx.drawImage(img, 0, 0, COLS, ROWS);
      fill(toAscii(ctx));
    };
    img.onerror = () => fill(fromWord(pre.dataset.word || "?"));
    img.src = src;
  });
})();

// Sommaire : met en évidence la section affichée.
(() => {
  const links = new Map([...document.querySelectorAll(".cdoc-toc a[href^='#']")].map((a) => [a.getAttribute("href").slice(1), a]));
  if (!links.size || !("IntersectionObserver" in window)) return;
  let current = null;
  const mark = (id) => {
    if (id === current) return;
    current = id;
    links.forEach((a) => a.classList.remove("is-current"));
    const a = links.get(id);
    if (!a) return;
    a.classList.add("is-current");
    links.get(id.split("-")[0])?.classList.add("is-current");
  };
  const visible = new Set();
  const io = new IntersectionObserver((entries) => {
    entries.forEach((e) => (e.isIntersecting ? visible.add(e.target) : visible.delete(e.target)));
    const top = [...visible].sort((a, b) => a.getBoundingClientRect().top - b.getBoundingClientRect().top)[0];
    if (top) mark(top.id);
  }, { rootMargin: "-80px 0px -55% 0px" });
  links.forEach((_, id) => { const el = document.getElementById(id); if (el) io.observe(el); });
})();

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

  const sourceFrames = () => [...document.querySelectorAll(".cdoc-body .card-frame")];

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
    const frame = e.target.closest?.(".cdoc-body .card-frame");
    if (frame) open(frame);
  });
  document.addEventListener("keydown", (e) => {
    if (dialog.open) {
      if (e.key === "ArrowRight") show(index + 1);
      if (e.key === "ArrowLeft") show(index - 1);
      return;
    }
    const frame = e.target.closest?.(".cdoc-body .card-frame");
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
