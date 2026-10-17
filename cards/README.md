# Cartes

Chaque carte est un fichier `<slug>.toml` dans ce dossier, et son illustration éventuelle est dans `art/`. Ces fichiers sont la source de vérité : ils sont faits pour être commités.

L'éditeur web (`/cards/new`) crée et modifie ces fichiers pour toi. Tu peux aussi les écrire ou les modifier à la main, puis cliquer sur **Recharger les fichiers** dans la galerie (ou relancer le serveur).

## Format

```toml
name = "ARGO CD"
type = "app"            # resource | incident | hr | app | event
skin = "light"          # light | dark (défaut : light)
rarity = "gold"         # basic | gold (défaut : basic)
foil = "stars"          # none | stars | glitter | shards | rainbow (défaut : none)
deploy_cost = 3         # 0 à 99 (défaut : 0)
daily_cost = 1          # 0 à 99 (défaut : 0)
description = "Permet de structurer vos repository Git et de déployer des resources Argo CD"
effect = """
- Débloque la carte applicationSet
- Ajoute +1 deployment MAX"""
flavor_text = "Pas de Flux dans ce jeu !"
art = "argo-cd.png"     # fichier dans art/ (optionnel)
```

Seuls `name` et `type` sont obligatoires.

Longueurs max, calées sur la place disponible dans la carte : nom 24, description 120, effect 220, texte d'ambiance 60 caractères.

## Règles

- **Nom du fichier :** le slug, en minuscules, chiffres et tirets (`argo-cd.toml`). L'éditeur le dérive du nom de la carte à la création.
- **Renommage :** renommer une carte dans l'éditeur ne renomme pas son fichier. Le slug reste stable, ce qui garde un historique Git propre.
- **Fichier invalide :** il est ignoré, sans bloquer les autres cartes, et signalé en haut de la galerie avec la raison.
- **Finition holographique (`foil`) :** indépendante de la rareté. Étoiles, paillettes, éclats ou irisé, avec un reflet qui suit la souris. Les textures sont dans `crates/dcb-server/static/foil/`, générées par `tools/gen_foil.py`.
- **Illustrations :** PNG, JPEG, WebP ou GIF, 5 Mo max, format idéal 336×180.
