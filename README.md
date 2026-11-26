# beta1-assets

Assets tiers de la beta 1 de **La Bataille de France** : modèles 3D, textures, sons, polices.

**Ce dépôt ne contient aucun asset.** Il contient leur **manifeste** ([manifeste.toml](manifeste.toml) : URL, taille, SHA-256, licence SPDX, auteur, modifications) et l'outil `assets`, qui les télécharge et les vérifie. Le projet ne redistribue donc rien dans git ; le jeu déployé, lui, sert les assets aux joueurs, d'où la liste fermée de licences. Les crédits sont dans [ATTRIBUTIONS.md](ATTRIBUTIONS.md), généré depuis le manifeste.

**En construction** : conversions (scripts Blender de P-26, textures et sons), puis image et Job de téléchargement dans le chart `game`.

## Règles

- Licences acceptées : **CC0-1.0, CC-BY-4.0, OFL-1.1**. Les auteurs sont cités dans l'écran de crédits, généré depuis le manifeste.
- **Pas de modèles 3D générés par IA.**
- Direction artistique low-poly, textures peu détaillées : la passe pixel art du client unifie le reste.
- Téléchargement en HTTPS seul, depuis une liste fermée d'hôtes (Poly Pizza, Poly Haven, OpenGameArt, GitHub par SHA de commit) ; Sketchfab par son API, avec un jeton personnel (`SKETCHFAB_TOKEN`).

Le manifeste joue le rôle d'un lockfile : un fichier n'est accepté que s'il a la taille et le SHA-256 inscrits. Ajouter un asset ou en changer la version passe par une PR relue, comme une dépendance. `cargo test` (donc la CI) refuse une licence hors liste, un hôte inconnu, un nom de fichier qui sortirait de la racine, et un `ATTRIBUTIONS.md` qui n'est pas à jour.

Deux contrôles de licence s'ajoutent au téléchargement :

- **Sketchfab** : la licence actuelle du modèle est relue sur l'API publique avant chaque téléchargement. Si l'auteur l'a changée, le modèle est refusé.
- **GLB** : si l'en-tête du fichier indique une licence (`asset.extras.license`, écrit par Sketchfab), elle doit être celle du manifeste.

## Utilisation

```sh
cargo run --release -- verifier                    # valide le manifeste
cargo run --release -- telecharger --racine assets # télécharge et vérifie ce qui manque
cargo run --release -- verifier --racine assets    # revérifie tous les fichiers présents
cargo run --release -- empreinte fichier.glb       # taille et SHA-256 à inscrire au manifeste
cargo run --release -- attributions --ecrire       # régénère ATTRIBUTIONS.md
```

Les fichiers sont rangés sous `<racine>/<id>/<nom>`. Chaque fichier est reçu sous `<nom>.partiel`, vérifié, puis renommé : un fichier visible sous son nom a été vérifié. Les archives zip (sons d'OpenGameArt) sont extraites à côté d'elles, pour les seules entrées listées.

Sans `SKETCHFAB_TOKEN`, les quatre modèles Sketchfab sont signalés en échec et le reste est téléchargé. Le jeton se crée dans les paramètres du compte Sketchfab (« API token »).

## Ajouter un asset

1. Vérifier la licence sur la page de l'œuvre (CC0-1.0, CC-BY-4.0 ou OFL-1.1) et noter l'auteur.
2. Ajouter un `[[asset]]` au manifeste (voir l'en-tête du fichier), avec `cargo run -- empreinte` pour la taille et le SHA-256.
3. `cargo run -- attributions --ecrire`, `cargo test`, puis PR.

Contribution : [CONTRIBUTING.md](https://github.com/labatailledefrance/.github/blob/main/CONTRIBUTING.md). Licence de l'outil : AGPL-3.0-or-later ; chaque asset garde la sienne.
