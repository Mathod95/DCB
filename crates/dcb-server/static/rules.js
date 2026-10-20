// Page des règles : sommaire de la page, avatars de l'équipe, nom d'application tapé, partie
// simulée de la condition de victoire.

// Sommaire : construit depuis les sections [data-toc], la section affichée est mise en évidence.
(() => {
  const list = document.querySelector(".doc-toc ol");
  if (!list) return;
  const sections = [...document.querySelectorAll("[data-toc]")];
  const links = new Map();
  sections.forEach((sec, i) => {
    if (!sec.id) sec.id = "s-" + (sec.getAttribute("aria-labelledby") || i);
    const a = document.createElement("a");
    a.href = "#" + sec.id;
    a.textContent = sec.dataset.toc;
    const li = document.createElement("li");
    li.append(a);
    list.append(li);
    links.set(sec, a);
  });
  const mark = () => {
    let current = sections[0];
    for (const sec of sections) if (sec.getBoundingClientRect().top < 140) current = sec;
    links.forEach((a, sec) => a.classList.toggle("is-current", sec === current));
  };
  addEventListener("scroll", mark, { passive: true });
  mark();
})();

// Équipe : une initiale au hasard dans chaque avatar, différente d'une personne à l'autre.
(() => {
  const letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ".split("");
  document.querySelectorAll(".avatar-dot").forEach((el) => {
    el.textContent = letters.splice(Math.floor(Math.random() * letters.length), 1)[0] || "?";
  });
})();

// Nom d'application : tapé lettre par lettre, effacé, puis un autre, comme si on hésitait encore.
(() => {
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  document.querySelectorAll(".typed[data-names]").forEach((el) => {
    const names = el.dataset.names.split("|").filter(Boolean);
    if (names.length < 2) return;
    let n = 0, i = names[0].length, deleting = true;
    const tick = () => {
      const word = names[n];
      if (!deleting) {
        i++;
        el.textContent = word.slice(0, i);
        if (i >= word.length) { deleting = true; return setTimeout(tick, 1800); }
        return setTimeout(tick, 70 + Math.random() * 90);
      }
      i--;
      el.textContent = word.slice(0, i);
      if (i <= 0) { deleting = false; n = (n + 1) % names.length; return setTimeout(tick, 400); }
      setTimeout(tick, 35);
    };
    setTimeout(tick, 2000);
  });
})();

// Condition de victoire : une partie simulée. Des gains et des pertes en $ font avancer (ou
// reculer) la barre jusqu'à l'objectif, puis la partie recommence.
(() => {
  const race = document.querySelector(".race");
  if (!race) return;
  const fill = race.querySelector(".race-fill");
  const amount = race.querySelector(".race-amount");
  const pops = race.querySelector(".race-pops");
  const START = +race.dataset.start || 25000;
  const GOAL = +race.dataset.goal || 1000000;
  const fmt = (n) => "$" + Math.round(n).toString().replace(/\B(?=(\d{3})+(?!\d))/g, ".") + ",00";
  const GAINS = ["Revenus MaSuperApp", "Nouveau client", "Levée de fonds", "Feature major", "Black Friday"];
  const LOSSES = ["Incident en prod", "Facture cloud", "DDOS", "Salaires", "CVE critique"];
  let cash = START;
  const show = () => {
    const pct = Math.max(1.2, Math.min(100, (cash / GOAL) * 100));
    fill.style.width = pct + "%";
    amount.textContent = fmt(cash);
    return pct;
  };
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  const step = () => {
    if (cash >= GOAL) {
      fill.classList.add("is-win");
      setTimeout(() => { cash = START; fill.classList.remove("is-win"); show(); setTimeout(step, 1200); }, 2600);
      return;
    }
    // la tendance monte, avec des coups durs de temps en temps (montants à l'échelle de l'objectif)
    const loss = Math.random() < 0.3;
    const unit = GOAL / 1000000;
    const size = (loss ? 15000 + Math.random() * 60000 : 30000 + Math.random() * 110000) * unit;
    const delta = Math.round((loss ? -size : size) / 500) * 500;
    cash = Math.min(GOAL, Math.max(START / 5, cash + delta));
    const pct = show();
    fill.classList.toggle("is-down", loss);
    const pop = document.createElement("span");
    pop.className = "race-pop " + (loss ? "is-loss" : "is-gain");
    pop.style.left = Math.min(92, Math.max(8, pct)) + "%";
    const why = (loss ? LOSSES : GAINS)[Math.floor(Math.random() * 5)];
    pop.textContent = (loss ? "−" : "+") + fmt(Math.abs(delta));
    const small = document.createElement("small");
    small.textContent = why;
    pop.append(small);
    pops.append(pop);
    setTimeout(() => pop.remove(), 1700);
    setTimeout(step, 1400 + Math.random() * 600);
  };
  setTimeout(step, 1200);
})();
