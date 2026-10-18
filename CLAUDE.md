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
- Ordre d'écriture : fichier TOML (atomique : tmp + rename) puis cache, sous le verrou de `CardStore`
- Requêtes SQL en `&'static str` (sqlx 0.9 refuse les chaînes construites à l'exécution) : composer avec `concat!` et la macro `columns!` dans `db.rs`

## Commandes

```bash
cargo run -p dcb-server                 # http://127.0.0.1:3000
cargo test --all
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```

## Conventions

- Textes de l'interface en français ; termes techniques en anglais (deploy, cluster, pod, scaling...)
- Pas de tiret cadratin dans les textes
- La rareté n'existe plus en tant que champ : c'est la finition `foil` (basic, gold, stars, glitter, shards, rainbow) qui en tient lieu. L'ancien champ TOML `rarity` est lu pour compatibilité (`Foil::from_legacy`), jamais écrit
- Les enums (type, skin, foil) sont stockés en texte via leur `slug()` ; ajouter une valeur = variante dans `dcb-core` + style CSS, sans migration
- Les cotes de `card.css` viennent du Figma : ne pas les changer sans vérifier la maquette
- Pas de cartes de démo ni de seed : le dépôt ne contient que les cartes créées par l'utilisateur
- Le slug (nom du fichier) ne change pas quand une carte est renommée
- L'illustration n'est jamais prise depuis le client lors d'une mise à jour (`current_art` sert uniquement à l'aperçu) ; les uploads sont identifiés par leurs octets, pas par leur extension

## État

Fait : finitions (`foil`, qui remplace la rareté, reflet suivant la souris), 61 cartes de l'utilisateur dans `cards/`, galerie filtrable, CRUD complet avec upload, cartes en TOML versionnées, rechargement des fichiers, aperçu htmx en direct, API JSON, tests HTTP de bout en bout, CI.

À faire : effets structurés pour le moteur, deck builder, moteur de duel + WebSocket, comptes joueurs, déploiement.
