# CLAUDE.md

Contexte pour reprendre le projet dans Claude Code.

## Projet

DevCardBattle (DCB) : jeu de cartes stratégique DevOps, inspiré de AWS Card Clash. Objectif final : duels multijoueurs tour par tour sur un site web. Jalon actuel : collection de cartes (galerie, éditeur, base de données).

Design source : fichier Figma `DevCardBattle` (fileKey `pG6o4Zvee9YjNf3AzGKV81`), composant `card` (node `30:1386`), calques `cardTypeStroke/*` (`30:1488`), `skin/*` (`30:1652`), `rarityLayer/*` (`30:1653`). La section `Collection` (`36:1521`) contient les cartes maquettées ; le frame `Board` (`36:2621`) esquisse le plateau de jeu.

## Stack

- Rust workspace, edition 2024
- `dcb-core` : types de domaine + validation, sans dépendance serveur
- `dcb-server` : axum 0.8, askama 0.16, sqlx 0.9 (SQLite), htmx 2 embarqué
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
- Les enums (type, skin, rareté) sont stockés en texte via leur `slug()` ; ajouter une valeur = variante dans `dcb-core` + style CSS, sans migration
- Les cotes de `card.css` viennent du Figma : ne pas les changer sans vérifier la maquette
- L'illustration n'est jamais prise depuis le client lors d'une mise à jour (`current_art` sert uniquement à l'aperçu) ; les uploads sont identifiés par leurs octets, pas par leur extension

## État

Fait : galerie filtrable, CRUD complet avec upload, aperçu htmx en direct, API JSON, 12 cartes de démo (coûts provisoires), tests HTTP de bout en bout, CI.

À faire : effets structurés pour le moteur, deck builder, moteur de duel + WebSocket, comptes joueurs, déploiement.
