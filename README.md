# DevCardBattle

Jeu de cartes stratégique sur le thème DevOps, inspiré de AWS Card Clash. À terme : des duels multijoueurs tour par tour, jouables depuis un navigateur.

Ce premier jalon pose la **collection de cartes** : une galerie web et un éditeur avec aperçu en direct. Chaque carte est un fichier TOML versionné dans `cards/`, rendu en reproduisant fidèlement le gabarit Figma.

## Démarrer

```bash
nix develop        # optionnel, fournit la toolchain Rust (voir flake.nix)
cargo run -p dcb-server
```

Puis ouvrir <http://127.0.0.1:3000>. Lance le serveur depuis la racine du repo : les cartes sont lues dans `./cards`.

Accueil, galerie, fiches et documentation sont publics ; on se connecte depuis le bouton en haut à droite. Crée d'abord ton compte admin :

```bash
cargo run -p dcb-server -- user add mathod --admin   # mot de passe demandé au clavier
```

| Variable | Défaut | Rôle |
|---|---|---|
| `DCB_CARDS_DIR` | `cards` | Dossier des cartes (source de vérité) |
| `DCB_RULES_FILE` | `rules/regles.toml` | Contenu de la page des règles |
| `DATABASE_URL` | `sqlite://data/cache.db` | Cache SQLite, reconstruit à chaque démarrage |
| `DCB_APP_DATABASE_URL` | `sqlite://data/app.db` | Comptes et sessions (**à sauvegarder**, jamais reconstruite) |
| `DCB_BIND` | `127.0.0.1:3000` | Adresse d'écoute |
| `DCB_EDITOR` | `1` | `0` coupe l'éditeur : les cartes sont en lecture seule, même pour l'admin |
| `DCB_SANDBOX` | `0` | `1` active le mode démo : les créateurs créent des cartes en ligne et ne modifient ou ne suppriment que les leurs, l'admin modifie tout. Les cartes créées en ligne ne sont pas conservées au déploiement suivant |
| `DCB_SECURE_COOKIES` | `0` | `1` derrière HTTPS (cookie de session `Secure`) |
| `DCB_ADMIN_USERNAME`, `DCB_ADMIN_PASSWORD` | | Créent le premier admin au démarrage si aucun compte n'existe |
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

## Comptes

Le jeu se consulte sans compte. Pas d'inscription libre : l'admin crée les comptes, avec le rôle qui donne leurs droits. Une page qui demande un compte renvoie vers le panneau de connexion de la barre du haut, puis ramène à la page demandée.

| Rôle | Droits |
|---|---|
| Visiteur (sans compte) | Accueil, galerie, fiches cartes, documentation, API en lecture |
| Joueur | La même chose, avec un compte (son mot de passe ; decks et duels plus tard) |
| Créateur | Crée des cartes et modifie ou supprime les siennes (« Mes cartes »), quand l'éditeur est ouvert (`DCB_EDITOR=1` ou `DCB_SANDBOX=1`) |
| Admin | Tout : toutes les cartes, rechargement des fichiers, page **Comptes** (`/admin/users`) |

Depuis la page Comptes : créer un compte (laisser le mot de passe vide pour en générer un, affiché une seule fois), changer son rôle (Joueur, Créateur, Admin), générer un nouveau mot de passe, désactiver ou supprimer. Le dernier admin actif ne peut pas être retiré.

En ligne de commande (mot de passe demandé au clavier, ou lu dans `DCB_PASSWORD`) :

```bash
dcb-server user add <nom> [--admin]
dcb-server user passwd <nom>     # ferme aussi ses sessions
dcb-server user list
```

Sécurité : mots de passe hachés en Argon2id, sessions de 30 jours dont seul le SHA-256 du jeton est stocké, cookie `HttpOnly` et `SameSite=Lax`, et refus des requêtes POST venant d'un autre site (en-tête `Origin`).

## Déploiement

Chaque push sur `main` publie l'image `ghcr.io/mathod95/dcb:latest` (et `sha-<commit>`). Elle contient le binaire, `cards/` et `rules/`, avec l'éditeur de fichiers coupé (`DCB_EDITOR=0`), le mode démo actif (`DCB_SANDBOX=1`) et les cookies `Secure`. En prod, les créateurs peuvent essayer l'éditeur : leurs cartes sont écrites dans le conteneur, hors du volume `/data`, et disparaissent à la mise à jour suivante. Les cartes du dépôt n'y sont modifiables que par l'admin, et ces modifications sont perdues aussi. Pour ajouter ou modifier une carte pour de bon, on passe par l'éditeur en local, on commite, on push, et on redéploie la nouvelle image.

```bash
docker run -d -p 3000:3000 -v dcb-data:/data \
  -e DCB_ADMIN_USERNAME=mathod -e DCB_ADMIN_PASSWORD='un-mot-de-passe-long' \
  ghcr.io/mathod95/dcb:latest
```

Le volume `/data` contient `app.db` (les comptes, à sauvegarder) et `cache.db` (jetable). Les variables `DCB_ADMIN_*` ne servent qu'au premier démarrage. L'app doit être servie en HTTPS (reverse proxy type Caddy ou un Ingress) ; sinon, mettre `DCB_SECURE_COOKIES=0`. Pour gérer les comptes depuis le serveur : `docker exec -it <conteneur> dcb-server user list`.

## Fonctionnalités

- **Galerie** : grille de toutes les cartes, avec un panneau Affichage à droite (changer la skin, la taille, le bloc Effect… de toutes les cartes affichées, sans les modifier) et un panneau de filtres à gauche (type, finition, contour, taille, badge de type, nom, description, cartes citées, relief, background, version). Plusieurs valeurs cochées dans un même réglage s'additionnent, les réglages se cumulent, et chaque valeur affiche le nombre de cartes qu'on obtiendrait en la cochant.
- **Éditeur** : création et modification avec aperçu de la carte mis à jour pendant la saisie, compteurs de caractères calés sur la place disponible dans la carte, upload d'illustration (PNG, JPEG, WebP, GIF, 5 Mo max).
- **Documentation** (`/regles`) : six pages, Overview, Understand the Basics, Core Concepts, Getting Started, Releases (patch notes, `rules/releases.toml`) et Roadmap (`rules/roadmap.toml`). Ancienne description de la page des règles :
  **Règles** (`/regles`) : contexte, début de partie, condition de victoire, types de cartes, phases d'une journée, cycle de vie des apps et finitions. Chaque type montre une carte de la collection tirée au hasard à chaque chargement (les cartes qui ont un texte d'abord), avec son nombre de cartes et un lien vers la galerie filtrée. Le texte vient de `rules/regles.toml`.
- **Pistes de design** (`/regles/concepts`, lien depuis la partie Concepts des règles) : le document Carte DCB v2, avec les styles de contour, les idées de modernisation, des skins et des skins animées, dessinés avec des cartes de la collection tirées au hasard à chaque affichage. Sommaire sur le côté, clic sur une carte pour la voir en grand. Le texte vient de `rules/concepts.toml`.
- **Fiche carte** : affichage en grand, modification, suppression (le fichier TOML et l'illustration sont supprimés avec la carte), et rappel des fichiers à commiter.
- **API JSON** pour les futurs clients du jeu :
  - `GET /api/cards` (filtres `?type=resource&type=incident&foil=gold&shape=neon&size=compact&badge=image&title=image&desc=comment&refs=linked&relief=soft&bg=tinted&version=v2`, clés répétables)
  - `GET /api/cards/{slug}`
  - `GET /api/meta` : types, couleurs, skins, finitions et limites de saisie

## La carte

Reproduite depuis le composant `card` du fichier Figma DevCardBattle (374×574), en trois couches :

| Couche | Valeurs |
|---|---|
| Type (bordure + badge) | Resource `#38bdf8`, Incident `#f83838`, Ressource humaine `#0800f6`, App `#f88838`, Event `#a538f8` |
| Skin | Light, Dark |
| Contour (`shape`) | Plein, Néon, Dégradé, Reflet qui tourne, Double filet, Coins de viseur, Coins coupés, Barre d'accent, Filet |
| Background (`background`) | Uni (par défaut), Teinté (le fond prend la couleur du type, avec n'importe quel contour) |
| Taille (`size`) | Normale (deux cases de coût, par défaut), Compacte (coûts en chips, bloc Effect agrandi) |
| Badge de type (`badge`) | Dans le nom (par défaut), Sur l'illustration (pastille avec l'icône du type) |
| Description (`desc_style`) | Classique (par défaut), Sans titre (le texte gagne une ligne), En commentaire (en tête du bloc Effect, qui gagne 70 px) |
| Cartes citées (`refs`) | Texte simple (par défaut), Cartes citées (le nom d'une autre carte dans l'effet prend la couleur de son type et mène à elle depuis la fiche) |
| Bloc Effect (`effect_style`, `effect_file`) | Texte (par défaut), Code (bloc façon éditeur, une ligne YAML par effet, nom de fichier libre, `effect.yaml` par défaut) |
| Relief (`relief`) | Plat (par défaut), En relief (ombre douce des blocs, toutes les skins) |
| Version (`version`) | Classique v1 (par défaut, chaque réglage comme choisi), Carte v2 (la carte finale : barre d'accent, compacte, badge et nom sur l'illustration, description en commentaire, relief, fond teinté, bloc Code, cartes citées ; skin et finition libres) |
| Nom (`title`) | Dans son bloc (par défaut), Sur l'illustration (en bas de l'image agrandie, le type passe en badge) |
| Finition (`foil`, fait office de rareté) | Basic, Gold (overlay doré du Figma), Étoiles, Paillettes, Éclats, Irisé, avec un reflet qui suit la souris |

Le rendu est en HTML/CSS (`crates/dcb-server/static/card.css`) : la carte est dessinée à ses cotes Figma puis mise à l'échelle, donc le même code sert à la galerie, à l'aperçu et à la fiche.

**Ajouter un type de carte** : une variante dans `CardType` (`crates/dcb-core/src/lib.rs`, avec sa couleur) et une ligne `[data-type]` dans `card.css`. Aucune migration : les types sont stockés en texte.

## Architecture

```
crates/
  dcb-core/     Modèle de domaine partagé (Card, CardType, Skin, Foil, validation)
  dcb-server/   Serveur axum : pages HTML (askama + htmx), API JSON
    src/store.rs  Lecture et écriture des fichiers cards/*.toml
    src/db.rs     Cache SQLite (sqlx)
    src/auth.rs   Comptes, mots de passe, sessions, extracteurs de rôle
    src/accounts.rs  Pages connexion, mon compte, administration des comptes
    templates/    Pages et partial de carte
    static/       CSS de l'app et de la carte, htmx embarqué
cards/          Les cartes : un .toml par carte + art/ (à commiter)
rules/          Texte de la page des règles (regles.toml, à commiter)
migrations/     Schéma du cache SQLite
migrations-app/ Schéma de la base persistante (comptes, sessions)
Dockerfile      Image de prod, publiée sur GHCR par .github/workflows/image.yml
```

Choix techniques :

- **Rendu serveur + htmx** plutôt qu'une SPA : pas de toolchain JavaScript ni de WASM à maintenir pour la partie collection. Les assets sont embarqués dans le binaire, un seul exécutable à déployer.
- **`dcb-core` séparé** pour que le futur moteur de duel (et un éventuel client Bevy compilé en WASM) réutilise exactement les mêmes types que la base et l'API.
- **Cartes en fichiers TOML** : elles se versionnent, se relisent en diff et se modifient à la main. L'éditeur écrit d'abord le fichier (de façon atomique), puis met à jour le cache.
- **SQLite comme cache** pour les lectures et les filtres. Les données des joueurs vivent dans une seconde base SQLite, persistante (`app.db`) : comptes aujourd'hui, decks et parties plus tard.

## Développement

```bash
cargo test --all                       # unitaires + tests HTTP de bout en bout
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```

La CI GitHub Actions lance ces trois commandes à chaque push et pull request, et publie l'image Docker à chaque push sur `main`.

## Prochaines étapes

- Effets structurés (type, valeur, durée) en plus du texte libre, pour que le moteur puisse les appliquer
- Deck builder
- Moteur de duel et parties en ligne via WebSocket
- Hébergement (VPS, Fly.io ou cluster Kubernetes)
