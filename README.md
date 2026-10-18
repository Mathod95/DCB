# DevCardBattle

Jeu de cartes stratégique sur le thème DevOps, inspiré de AWS Card Clash. À terme : des duels multijoueurs tour par tour, jouables depuis un navigateur.

Ce premier jalon pose la **collection de cartes** : une galerie web et un éditeur avec aperçu en direct. Chaque carte est un fichier TOML versionné dans `cards/`, rendu en reproduisant fidèlement le gabarit Figma.

## Démarrer

```bash
nix develop        # optionnel, fournit la toolchain Rust (voir flake.nix)
cargo run -p dcb-server
```

Puis ouvrir <http://127.0.0.1:3000>. Lance le serveur depuis la racine du repo : les cartes sont lues dans `./cards`.

| Variable | Défaut | Rôle |
|---|---|---|
| `DCB_CARDS_DIR` | `cards` | Dossier des cartes (source de vérité) |
| `DCB_RULES_FILE` | `rules/regles.toml` | Contenu de la page des règles |
| `DATABASE_URL` | `sqlite://data/cache.db` | Cache SQLite, reconstruit à chaque démarrage |
| `DCB_BIND` | `127.0.0.1:3000` | Adresse d'écoute |
| `RUST_LOG` | `dcb_server=info,tower_http=info` | Niveau de logs |

## Ajouter des cartes

1. Crée la carte dans l'éditeur (`/cards/new`). Il écrit `cards/<slug>.toml`, et `cards/art/<slug>.png` si tu ajoutes une illustration.
2. Commite ces fichiers :

   ```bash
   git add cards/
   git commit -m "feat(cards): ajouter Argo CD"
   git push
   ```

Les fichiers peuvent aussi être écrits ou modifiés à la main. Le format est décrit dans [`cards/README.md`](cards/README.md). Après une modification à la main ou un `git pull`, clique sur **Recharger les fichiers** dans la galerie.

La base SQLite n'est qu'un cache : elle est vidée et reconstruite depuis `cards/` au démarrage et à chaque rechargement. Tu peux supprimer `data/` à tout moment.

## Fonctionnalités

- **Galerie** : grille de toutes les cartes, filtres par type et par finition.
- **Éditeur** : création et modification avec aperçu de la carte mis à jour pendant la saisie, compteurs de caractères calés sur la place disponible dans la carte, upload d'illustration (PNG, JPEG, WebP, GIF, 5 Mo max).
- **Règles** (`/regles`) : contexte, début de partie, condition de victoire, types de cartes, phases d'une journée, cycle de vie des apps et finitions. Chaque type montre une carte de la collection tirée au hasard à chaque chargement (les cartes qui ont un texte d'abord), avec son nombre de cartes et un lien vers la galerie filtrée. Le texte vient de `rules/regles.toml`.
- **Fiche carte** : affichage en grand, modification, suppression (le fichier TOML et l'illustration sont supprimés avec la carte), et rappel des fichiers à commiter.
- **API JSON** pour les futurs clients du jeu :
  - `GET /api/cards` (filtres `?type=incident&foil=gold`)
  - `GET /api/cards/{slug}`
  - `GET /api/meta` : types, couleurs, skins, finitions et limites de saisie

## La carte

Reproduite depuis le composant `card` du fichier Figma DevCardBattle (374×574), en trois couches :

| Couche | Valeurs |
|---|---|
| Type (bordure + badge) | Resource `#38bdf8`, Incident `#f83838`, Ressource humaine `#0800f6`, App `#f88838`, Event `#a538f8` |
| Skin | Light, Dark |
| Finition (`foil`, fait office de rareté) | Basic, Gold (overlay doré du Figma), Étoiles, Paillettes, Éclats, Irisé, avec un reflet qui suit la souris |

Le rendu est en HTML/CSS (`crates/dcb-server/static/card.css`) : la carte est dessinée à ses cotes Figma puis mise à l'échelle, donc le même code sert à la galerie, à l'aperçu et à la fiche.

**Ajouter un type de carte** : une variante dans `CardType` (`crates/dcb-core/src/lib.rs`, avec sa couleur) et une ligne `[data-type]` dans `card.css`. Aucune migration : les types sont stockés en texte.

## Architecture

```
crates/
  dcb-core/     Modèle de domaine partagé (Card, CardType, Skin, Rarity, validation)
  dcb-server/   Serveur axum : pages HTML (askama + htmx), API JSON
    src/store.rs  Lecture et écriture des fichiers cards/*.toml
    src/db.rs     Cache SQLite (sqlx)
    templates/    Pages et partial de carte
    static/       CSS de l'app et de la carte, htmx embarqué
cards/          Les cartes : un .toml par carte + art/ (à commiter)
rules/          Texte de la page des règles (regles.toml, à commiter)
migrations/     Schéma du cache SQLite
```

Choix techniques :

- **Rendu serveur + htmx** plutôt qu'une SPA : pas de toolchain JavaScript ni de WASM à maintenir pour la partie collection. Les assets sont embarqués dans le binaire, un seul exécutable à déployer.
- **`dcb-core` séparé** pour que le futur moteur de duel (et un éventuel client Bevy compilé en WASM) réutilise exactement les mêmes types que la base et l'API.
- **Cartes en fichiers TOML** : elles se versionnent, se relisent en diff et se modifient à la main. L'éditeur écrit d'abord le fichier (de façon atomique), puis met à jour le cache.
- **SQLite comme cache** pour les lectures et les filtres. Les données des joueurs (comptes, decks, parties) iront plus tard dans une vraie base.

## Développement

```bash
cargo test --all                       # unitaires + tests HTTP de bout en bout
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```

La CI GitHub Actions lance ces trois commandes à chaque push et pull request.

## Prochaines étapes

- Effets structurés (type, valeur, durée) en plus du texte libre, pour que le moteur puisse les appliquer
- Deck builder
- Moteur de duel et parties en ligne via WebSocket
- Comptes joueurs
