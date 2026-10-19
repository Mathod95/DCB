# CLAUDE.md

Contexte pour reprendre le projet dans Claude Code.

## Projet

DevCardBattle (DCB) : jeu de cartes stratégique DevOps, inspiré de AWS Card Clash. Objectif final : duels multijoueurs tour par tour sur un site web. Jalon actuel : collection de cartes (galerie, éditeur). Pour l'instant c'est un projet Rust local, sans déploiement ni prod.

Design source : fichier Figma `DevCardBattle` (fileKey `pG6o4Zvee9YjNf3AzGKV81`), composant `card` (node `30:1386`), calques `cardTypeStroke/*` (`30:1488`), `skin/*` (`30:1652`). Finitions : section `foilLayer` (`107:1435`, composants `foilLayer/Basic|Gold|Stars|Glitter|Shards|Rainbow`, statiques ; l'ancien `rarityLayer` y a été fusionné). La section `Collection` (`36:1521`) contient les cartes maquettées ; le frame `Board` (`36:2621`) esquisse le plateau de jeu.

## Stack

- Rust workspace, edition 2024
- `dcb-core` : types de domaine + validation, sans dépendance serveur
- `dcb-server` : axum 0.8, askama 0.16, sqlx 0.9 (SQLite), toml, htmx 2 embarqué
- Source de vérité : `cards/<slug>.toml` + `cards/art/` (commités par l'utilisateur). SQLite (`data/cache.db`) n'est qu'un cache reconstruit au démarrage et via `POST /reload`
- Page `/regles` : texte dans `rules/regles.toml` (lu au démarrage et au rechargement, en cas d'erreur la dernière version valide reste affichée), mise en forme `***terme***` et montants `$…` via `rules::inline`. Exemples de cartes tirés au hasard par `db::random_card` (cartes remplies d'abord)
- Ordre d'écriture : fichier TOML (atomique : tmp + rename) puis cache, sous le verrou de `CardStore`
- Comptes : seconde base SQLite persistante `data/app.db` (`migrations-app/`, module `auth`), jamais reconstruite. Argon2id, sessions en cookie `dcb_session` (SHA-256 du jeton en base). Accès par extracteurs : `MaybeUser`, `CurrentUser` (sinon redirection `/login?next=`), `AdminUser` (403), `EditorUser` (admin + `DCB_EDITOR`, sinon 404 si éditeur coupé). Middleware `same_origin_only` contre le CSRF. Pas d'inscription : comptes créés par l'admin (`/admin/users` ou `dcb-server user add`)
- Prod : image `ghcr.io/mathod95/dcb` (Dockerfile, workflow `image.yml`), `DCB_EDITOR=0`, volume `/data`. Cartes en lecture seule en prod : création en local, push, nouvelle image
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
- La rareté n'existe plus en tant que champ : c'est la finition `foil` (basic, gold, stars, glitter, shards, rainbow) qui en tient lieu. L'ancien champ TOML `rarity` est lu pour compatibilité (`Foil::from_legacy`), jamais écrit
- Les enums (type, skin, foil) sont stockés en texte via leur `slug()` ; ajouter une valeur = variante dans `dcb-core` + style CSS, sans migration
- Carte sans illustration : bloc `.card-art.is-empty` teinté par la couleur du type, motif de points et icône du type (dans `card.html`, une icône par type)
- Les cotes de `card.css` viennent du Figma : ne pas les changer sans vérifier la maquette
- Pas de cartes de démo ni de seed : le dépôt ne contient que les cartes créées par l'utilisateur
- Le slug (nom du fichier) ne change pas quand une carte est renommée
- L'illustration n'est jamais prise depuis le client lors d'une mise à jour (`current_art` sert uniquement à l'aperçu) ; les uploads sont identifiés par leurs octets, pas par leur extension

## État

Fait : connexion et rôles (admin, utilisateur), administration des comptes, image Docker sur GHCR, page des règles (/regles), finitions (`foil`, qui remplace la rareté, reflet suivant la souris), 86 cartes de l'utilisateur dans `cards/`, galerie filtrable, CRUD complet avec upload, cartes en TOML versionnées, rechargement des fichiers, aperçu htmx en direct, API JSON, tests HTTP de bout en bout, CI.

À faire : effets structurés pour le moteur, deck builder, moteur de duel + WebSocket, hébergement (cible à choisir : VPS + Caddy, Fly.io ou Kubernetes/ArgoCD).
