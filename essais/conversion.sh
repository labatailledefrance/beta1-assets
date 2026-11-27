#!/usr/bin/env bash
# Essai de conversion, sans Blender ni Sketchfab : télécharge les entrées
# (hôtes publics), convertit textures, sons et police, puis compare les
# empreintes à essais/sorties.sha256. Les sons et la police sont identiques
# octet pour octet à ceux de P-26 ; textures.p24t est figé ici.
#   essais/conversion.sh telecharger <dossier>
#   essais/conversion.sh verifier <dossier>
set -euo pipefail
cd "$(dirname "$0")/.."
etape=$1
dossier=$2
assets=target/release/assets
case "$etape" in
  telecharger)
    "$assets" telecharger --racine "$dossier/racine" --sans-sketchfab
    ;;
  verifier)
    rm -rf "$dossier/sortie"
    "$assets" convertir --racine "$dossier/racine" --sortie "$dossier/sortie" --sans-blender
    reference=$PWD/essais/sorties.sha256
    (cd "$dossier/sortie" && sha256sum --strict -c "$reference")
    # Aucune sortie en plus de celles attendues.
    diff <(cd "$dossier/sortie" && find . -type f -printf '%P\n' | sort) <(awk '{print $2}' "$reference" | sort)
    ;;
  *)
    echo "étape inconnue : $etape" >&2
    exit 2
    ;;
esac
