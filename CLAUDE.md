# CLAUDE.md

Contexte pour reprendre le projet dans Claude Code.

## Projet

DevCardBattle (DCB) : jeu de cartes stratégique DevOps, inspiré de AWS Card Clash. Objectif final : duels multijoueurs tour par tour sur un site web. Jalon actuel : collection de cartes (galerie, éditeur). Développé en local, publié en démo sur dcb.mathod.fr.

Design source : fichier Figma `DevCardBattle` (fileKey `pG6o4Zvee9YjNf3AzGKV81`), composant `card` (node `30:1386`), calques `cardTypeStroke/*` (`30:1488`), `skin/*` (`30:1652`). Finitions : section `foilLayer` (`107:1435`, composants `foilLayer/Basic|Gold|Stars|Glitter|Shards|Rainbow`, statiques ; l'ancien `rarityLayer` y a été fusionné). La section `Collection` (`36:1521`) contient les cartes maquettées ; le frame `Board` (`36:2621`) esquisse le plateau de jeu.

## Stack

- Rust workspace, edition 2024
- `dcb-core` : types de domaine + validation, sans dépendance serveur
- `dcb-server` : axum 0.8, askama 0.16, sqlx 0.9 (SQLite), toml, htmx 2 embarqué
- Source de vérité : `cards/<slug>.toml` + `cards/art/` (commités par l'utilisateur). SQLite (`data/cache.db`) n'est qu'un cache reconstruit au démarrage et via `POST /reload`
- Page `/regles` : texte dans `rules/regles.toml` (lu au démarrage et au rechargement, en cas d'erreur la dernière version valide reste affichée), mise en forme `***terme***` et montants `$…` via `rules::inline`. Exemples de cartes tirés au hasard par `db::random_card` (cartes remplies d'abord)
- Partie Concepts de `/regles` : une bande qui défile en boucle par réglage (skins classiques, skins animées, finitions, contours ; contenu doublé, copie `aria-hidden`), avec compteur ; chaque exemple est une carte différente tirée au hasard (cartes remplies d'abord, pioche relancée si la collection est plus petite). Skins décrites dans `[[concepts.skin.classic|animated]]` de `regles.toml` (clé → `concepts::skin_style`, CSS `sk-*`/`dyn-*` de `concepts.css`). Vue zoomée au clic (`concepts.js`, `#card-modal` avec `data-frames`/`data-click`) : description de la skin, et choix de la catégorie qui tire une carte du type parmi les `<template class="tpl-card">` de toute la collection (cartes remplies d'abord)
- `/regles/concepts` : document « Carte DCB v2 » (pistes de design restantes : contours, idées ; les skins sont passées sur `/regles`, on retire au fur et à mesure ce qui est repris), rendu par l'app comme `/regles`. Texte dans `rules/concepts.toml` (rechargé avec les règles ; chaque section a une `key` qui choisit le rendu, clé inconnue signalée), cartes tirées au hasard à chaque affichage, module `concepts.rs`, gabarit `concept_card.html`, `static/concepts.css` et `concepts.js` (sommaire, fenêtre agrandie, ASCII calculé dans le navigateur sur un canvas)
- Ordre d'écriture : fichier TOML (atomique : tmp + rename) puis cache, sous le verrou de `CardStore`
- Comptes : seconde base SQLite persistante `data/app.db` (`migrations-app/`, module `auth`), jamais reconstruite. Argon2id, sessions en cookie `dcb_session` (SHA-256 du jeton en base). Accès par extracteurs : `MaybeUser`, `CurrentUser` (sinon redirection `/login?next=`), `AdminUser` (403), `EditorUser` (admin + `DCB_EDITOR`, sinon 404 si éditeur coupé ; tout compte connecté en mode démo). Middleware `same_origin_only` contre le CSRF. Pas d'inscription : comptes créés par l'admin (`/admin/users` ou `dcb-server user add`)
- Prod : image `ghcr.io/mathod95/dcb` (Dockerfile, workflow `image.yml`), `DCB_EDITOR=0`, `DCB_SANDBOX=1`, volume `/data`. Hébergée sur le serveur Saltbox de l'utilisateur (Traefik), à dcb.mathod.fr. Les vraies cartes se créent en local, puis push et nouvelle image
- Mode démo (`DCB_SANDBOX=1`) : tout joueur connecté crée des cartes (`Ctx.can_create`), mais ne modifie ou ne supprime que les siennes (`AppState::may_modify`, 403 sinon) ; l'admin modifie tout ; `/reload` reste admin. L'auteur de chaque carte est gardé en mémoire (`AppState.owners`) et les cartes sont écrites dans `cards/` du conteneur : tout disparaît au redéploiement. Bandeau `demo_banner.html`, pas de « Fichiers à commiter »
- Requêtes SQL en `&'static str` (sqlx 0.9 refuse les chaînes construites à l'exécution) : composer avec `concat!` et la macro `columns!` dans `db.rs`

## Commandes

```bash
cargo run -p dcb-server                 # http://127.0.0.1:3000
cargo run -p dcb-server -- user add <nom> --admin
cargo test --all
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```

## Conventions

- Textes de l'interface en français ; termes techniques en anglais (deploy, cluster, pod, scaling...)
- Pas de tiret cadratin dans les textes
- Cible : navigateur sur ordinateur uniquement. Pas de vue mobile à concevoir ni à tester
- Navigation : la barre du haut ne contient que le logo (retour au menu) et le menu du compte. Le menu d'accueil propose Galerie, Documentation (/regles) et Comptes (admin seulement). La création de carte se fait depuis le bouton « + Nouvelle carte » de la galerie
- La rareté n'existe plus en tant que champ : c'est la finition `foil` (basic, gold, stars, glitter, shards, rainbow) qui en tient lieu. L'ancien champ TOML `rarity` est lu pour compatibilité (`Foil::from_legacy`), jamais écrit
- Contour : champ `shape` (`Shape` dans dcb-core), `solid` par défaut et non écrit dans le TOML. CSS via `.card[data-shape=…]` dans `card.css`. `sweep` ne tourne qu'au survol dans la galerie, en continu sur la fiche et dans l'aperçu
- Taille : champ `size` (`Size` dans dcb-core), `normal` par défaut et non écrit dans le TOML. `normal` : deux cases de coût de 163 px ; `compact` : chips de 40 px avec icône et bloc Effect de 135 px au lieu de 110. CSS via `.card[data-size=…]` (le `data-size` de `.card-name` est la taille de police du nom, sans rapport). Plus de case Rarity sur les cartes : la finition se voit à son reflet et sur la fiche
- Les enums (type, skin, foil, shape, size) sont stockés en texte via leur `slug()` ; ajouter une valeur = variante dans `dcb-core` + style CSS, sans migration
- Bloc illustration : contour teinté par la couleur du type (25 %). Carte sans illustration : bloc `.card-art.is-empty` teinté par la couleur du type, motif de points et icône du type (dans `card.html`, une icône par type)
- Les cotes de `card.css` viennent du Figma : ne pas les changer sans vérifier la maquette
- Pas de cartes de démo ni de seed : le dépôt ne contient que les cartes créées par l'utilisateur
- Le slug (nom du fichier) ne change pas quand une carte est renommée
- L'illustration n'est jamais prise depuis le client lors d'une mise à jour (`current_art` sert uniquement à l'aperçu) ; les uploads sont identifiés par leurs octets, pas par leur extension

## État

Fait : skins classiques et animées sur la page des règles (bandes défilantes, vue zoomée par catégorie), taille normale ou compacte (`size`) et retrait de la case Rarity, mode démo de l'éditeur en prod (`DCB_SANDBOX`), hébergement sur Saltbox, contours sélectionnables (`shape`), connexion et rôles (admin, utilisateur), administration des comptes, image Docker sur GHCR, page des règles (/regles), finitions (`foil`, qui remplace la rareté, reflet suivant la souris), 86 cartes de l'utilisateur dans `cards/`, galerie filtrable, CRUD complet avec upload, cartes en TOML versionnées, rechargement des fichiers, aperçu htmx en direct, API JSON, tests HTTP de bout en bout, CI.

À faire : effets structurés pour le moteur, deck builder, moteur de duel + WebSocket.
