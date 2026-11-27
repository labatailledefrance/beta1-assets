# beta1-assets

Assets tiers de la beta 1 de **La Bataille de France** : modèles 3D, textures, sons, polices.

**Ce dépôt ne contient aucun asset.** Il contient leur **manifeste** ([manifeste.toml](manifeste.toml) : URL, taille, SHA-256, licence SPDX, auteur, modifications) et l'outil `assets`, qui les télécharge et les vérifie. Le projet ne redistribue donc rien dans git ; le jeu déployé, lui, sert les assets aux joueurs, d'où la liste fermée de licences. Les crédits sont dans [ATTRIBUTIONS.md](ATTRIBUTIONS.md), généré depuis le manifeste.

**En construction** : Job de téléchargement et de conversion dans le chart `game` ([beta1-helm](https://github.com/labatailledefrance/beta1-helm)).

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

Sans `SKETCHFAB_TOKEN`, les quatre modèles Sketchfab sont signalés en échec et le reste est téléchargé (`--sans-sketchfab` les ignore). Le jeton se crée dans les paramètres du compte Sketchfab (« API token »).

## Conversion

```sh
cargo run --release -- convertir --racine assets --sortie jeu   # Blender 5.3.0 dans le PATH
cargo run --release -- convertir --racine assets --sortie jeu --sans-blender
```

Sortie, servie aux clients :

| Dossier | Contenu | Produit par |
|---|---|---|
| `modeles/` | véhicules et arbre (`.glb`, +X avant, +Z haut, échelle réelle), personnages animés (`.vat`) | [blender/vehicules.py](blender/vehicules.py), [blender/personnages.py](blender/personnages.py), Blender sans fenêtre |
| `modeles/textures.p24t` | 12 couches de 512 px avec mipmaps, ramenées à un détail autour de la moyenne | Rust (`src/conversion/textures.rs`) |
| `sons/` | coups de feu, explosions, zombies, moteur, réacteur synthétisé (WAV mono 16 bits), vent et vagues (OGG) | Rust (`src/conversion/sons.rs`) |
| `polices/` | DotGothic16 et sa licence OFL | copie |
| `ATTRIBUTIONS.md` | crédits | généré depuis le manifeste |

Blender ne sert qu'aux maillages et aux animations ; tout le reste est en Rust, sans Python ni PIL. Les sorties ont été comparées à celles de P-26 : modèles (Blender 5.3.0), sons et police identiques octet pour octet ; textures à un ou deux niveaux près, le décodeur JPEG n'étant pas celui de PIL. La CI (`essai de conversion`) télécharge les entrées des textures, sons et police (`--sans-modeles` : Poly Pizza refuse les IP de datacenter), convertit sans Blender et compare les empreintes à [essais/sorties.sha256](essais/sorties.sha256).

## Ajouter un asset

1. Vérifier la licence sur la page de l'œuvre (CC0-1.0, CC-BY-4.0 ou OFL-1.1) et noter l'auteur.
2. Ajouter un `[[asset]]` au manifeste (voir l'en-tête du fichier), avec `cargo run -- empreinte` pour la taille et le SHA-256.
3. `cargo run -- attributions --ecrire`, `cargo test`, puis PR.

## Image

`ghcr.io/labatailledefrance/beta1-assets`, publiée par la CI après fusion, signée (cosign) et attestée (SLSA), comme les autres images du projet :

- `assets` (le manifeste est compilé dedans : le digest de l'image fixe les assets) et les scripts de `blender/` ;
- Blender 5.3.0 officiel, vérifié par le SHA-256 publié sur download.blender.org, et les bibliothèques système qu'il charge même sans fenêtre (X11, xkbcommon, GL), prises dans le dépôt apt de DHI aux versions exactes ;
- base `dhi.io/debian-base:trixie` épinglée par digest, sans gestionnaire de paquets, utilisateur 65532.

Essai local dans les conditions d'un Job (racine en lecture seule, sans capacités, sans réseau pour la conversion) :

```sh
docker build -t beta1-assets .
docker run --rm --read-only --tmpfs /tmp --cap-drop ALL --security-opt no-new-privileges \
  -v "$PWD/sources:/sources" beta1-assets telecharger --racine /sources
docker run --rm --read-only --tmpfs /tmp --cap-drop ALL --security-opt no-new-privileges --network none \
  -v "$PWD/sources:/sources:ro" -v "$PWD/sortie:/sortie" \
  beta1-assets convertir --racine /sources --sortie /sortie --blender /opt/blender/blender --scripts /opt/assets/blender
```

Poly Pizza (Cloudflare) refuse les IP de datacenter, srv compris : les 8 modèles Quaternius se téléchargent depuis une machine personnelle, puis se copient dans le volume des sources. `telecharger` vérifie les fichiers présents et ne télécharge que les autres.

Contribution : [CONTRIBUTING.md](https://github.com/labatailledefrance/.github/blob/main/CONTRIBUTING.md). Licence de l'outil : AGPL-3.0-or-later ; chaque asset garde la sienne.
