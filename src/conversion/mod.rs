//! Conversion des fichiers téléchargés en assets du jeu.
//!
//! Sortie (celle de P-26, servie aux clients) :
//! - `modeles/` : véhicules en GLB, personnages en `.vat` (Blender, scripts de
//!   `blender/`), `textures.p24t` ;
//! - `sons/` : WAV mono 16 bits et deux OGG ;
//! - `polices/` : DotGothic16 et sa licence ;
//! - `ATTRIBUTIONS.md`, pour l'écran de crédits.
//!
//! Blender ne sert qu'aux maillages et aux animations ; le reste est en Rust.

pub mod mt;
pub mod sons;
pub mod textures;
pub mod wav;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::attributions;
use crate::manifeste::Asset;

/// Modèles lus par les scripts Blender (id du manifeste, fichier).
pub const ENTREES_BLENDER: [(&str, &str); 12] = [
    ("leclerc", "char_leclerc.glb"),
    ("vbci", "vbci_low_poly.glb"),
    ("rafale", "dassault_rafale.glb"),
    ("charles-de-gaulle", "charles_de_gaulle_aircraft_carrier.glb"),
    ("quaternius-truck", "truck2.glb"),
    ("quaternius-tree", "tree.glb"),
    ("quaternius-casual", "casual.glb"),
    ("quaternius-worker", "worker.glb"),
    ("quaternius-farmer", "farmer.glb"),
    ("quaternius-adventurer", "adv.glb"),
    ("quaternius-swat", "swat.glb"),
    ("quaternius-assault-rifle", "rifle.glb"),
];

/// Police : (fichier téléchargé, nom servi).
pub const POLICE: [(&str, &str); 2] =
    [("DotGothic16-Regular.ttf", "DotGothic16-Regular.ttf"), ("OFL.txt", "DotGothic16-OFL.txt")];

/// Tous les fichiers lus par la conversion, ceux de Blender en premier.
pub fn entrees() -> Vec<(&'static str, &'static str)> {
    let mut e: Vec<(&str, &str)> = ENTREES_BLENDER.to_vec();
    e.extend(textures::COUCHES.iter().map(|(id, nom, _)| (*id, *nom)));
    e.extend(sons::ENTREES);
    e.extend(POLICE.iter().map(|(nom, _)| ("dotgothic16", *nom)));
    e
}

/// Blender, sans fenêtre.
#[derive(Debug, Clone)]
pub struct Blender {
    pub executable: PathBuf,
    /// Dossier de `vehicules.py` et `personnages.py`.
    pub scripts: PathBuf,
}

fn ecrire(chemin: &Path, contenu: &[u8]) -> Result<(), String> {
    fs::write(chemin, contenu).map_err(|e| format!("{}: {e}", chemin.display()))
}

pub fn convertir(assets: &[Asset], racine: &Path, sortie: &Path, blender: Option<&Blender>) -> Result<(), String> {
    let manquants: Vec<String> = entrees()
        .into_iter()
        .skip(if blender.is_some() { 0 } else { ENTREES_BLENDER.len() })
        .filter(|(id, nom)| !racine.join(id).join(nom).exists())
        .map(|(id, nom)| format!("{id}/{nom}"))
        .collect();
    if !manquants.is_empty() {
        return Err(format!("fichiers absents (assets telecharger) : {}", manquants.join(", ")));
    }
    let modeles = sortie.join("modeles");
    let polices = sortie.join("polices");
    for d in [&modeles, &polices] {
        fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    ecrire(&modeles.join("textures.p24t"), &textures::convertir(racine)?)?;
    println!("textures : {} couches", textures::COUCHES.len());
    let s = sons::convertir(racine, &sortie.join("sons"))?;
    println!("sons : {} fichiers", s.len());
    for (nom, but) in POLICE {
        fs::copy(racine.join("dotgothic16").join(nom), polices.join(but)).map_err(|e| format!("{nom}: {e}"))?;
    }
    ecrire(&sortie.join("ATTRIBUTIONS.md"), attributions::generer(assets).as_bytes())?;
    if let Some(b) = blender {
        for script in ["vehicules.py", "personnages.py"] {
            let etat = Command::new(&b.executable)
                .args(["-b", "--factory-startup", "--python-exit-code", "1", "-P"])
                .arg(b.scripts.join(script))
                .arg("--")
                .arg(racine)
                .arg(&modeles)
                .status()
                .map_err(|e| format!("{} : {e}", b.executable.display()))?;
            if !etat.success() {
                return Err(format!("Blender, {script} : {etat}"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifeste;

    /// Chaque fichier lu par la conversion est au manifeste (téléchargé ou extrait).
    #[test]
    fn entrees_au_manifeste() {
        let assets = manifeste::charger().unwrap_or_else(|e| panic!("{}", e.join("\n")));
        for (id, nom) in entrees() {
            let a = assets.iter().find(|a| a.id == id).unwrap_or_else(|| panic!("asset absent : {id}"));
            let present = a.fichier.iter().any(|f| f.nom == nom || f.extraits().any(|(_, n)| n == nom));
            assert!(present, "{id}/{nom} absent du manifeste");
        }
    }

    /// Chaque asset du manifeste sert à la conversion.
    #[test]
    fn aucun_asset_inutile() {
        let assets = manifeste::charger().unwrap_or_else(|e| panic!("{}", e.join("\n")));
        let e = entrees();
        for a in &assets {
            assert!(e.iter().any(|(id, _)| *id == a.id), "{} n'est utilisé par aucune conversion", a.id);
        }
    }
}
