//! Publication adressée par ses entrées, comme celle des tiles (H-06).
//!
//! Le chart donne l'identifiant (hash du digest de l'image, donc du manifeste
//! et des scripts compilés dedans) : il connaît le dossier à servir avant
//! qu'il existe. Une publication complète relancée sort aussitôt ; sinon elle
//! est produite dans `<id>.partiel`, avec `MANIFESTE` (SHA-256 de chaque
//! fichier) et `COMPLET`, puis renommée. Comme un store Nix : un dossier
//! visible sous son nom est complet et ne change plus.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::verification;

/// Marqueur d'une publication terminée (attendu par les `initContainer`).
pub const COMPLET: &str = "COMPLET";

/// `[a-z0-9-]`, 1 à 64 caractères.
pub fn identifiant_valide(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('-')
        && id.bytes().all(|o| o.is_ascii_lowercase() || o.is_ascii_digit() || o == b'-')
}

#[derive(Debug, PartialEq, Eq)]
pub enum Etat {
    DejaComplete,
    Produite,
}

/// Produit `sortie/<id>` avec `produire`, qui écrit dans le dossier donné.
pub fn publier(sortie: &Path, id: &str, produire: impl FnOnce(&Path) -> Result<(), String>) -> Result<Etat, String> {
    if !identifiant_valide(id) {
        return Err(format!("identifiant de publication invalide : {id:?} ([a-z0-9-], 1 à 64)"));
    }
    let fin = sortie.join(id);
    if fin.join(COMPLET).exists() {
        return Ok(Etat::DejaComplete);
    }
    let travail = sortie.join(format!("{id}.partiel"));
    for d in [&travail, &fin] {
        // Reste d'un pod tué (ou dossier final sans marqueur) : on repart de zéro.
        if d.exists() {
            fs::remove_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
    }
    fs::create_dir_all(&travail).map_err(|e| format!("{}: {e}", travail.display()))?;
    produire(&travail)?;
    let mut fichiers = Vec::new();
    lister(&travail, &travail, &mut fichiers)?;
    fichiers.sort();
    let mut m = format!("publication {id}\nversion {}\n", env!("CARGO_PKG_VERSION"));
    for f in &fichiers {
        let (sha256, _) = verification::empreinte(&travail.join(f)).map_err(|e| format!("{}: {e}", f.display()))?;
        m.push_str(&format!("{sha256}  {}\n", f.display()));
    }
    fs::write(travail.join("MANIFESTE"), m).map_err(|e| format!("MANIFESTE : {e}"))?;
    fs::write(travail.join(COMPLET), "").map_err(|e| format!("{COMPLET} : {e}"))?;
    fs::rename(&travail, &fin).map_err(|e| format!("renommage : {e}"))?;
    Ok(Etat::Produite)
}

fn lister(base: &Path, d: &Path, o: &mut Vec<PathBuf>) -> Result<(), String> {
    for e in fs::read_dir(d).map_err(|e| format!("{}: {e}", d.display()))? {
        let e = e.map_err(|e| format!("{}: {e}", d.display()))?;
        let p = e.path();
        if p.is_dir() {
            lister(base, &p, o)?;
        } else if let Ok(r) = p.strip_prefix(base) {
            o.push(r.to_owned());
        }
    }
    Ok(())
}

/// Attend qu'un fichier existe (marqueur `COMPLET` d'une publication).
pub fn attendre(fichier: &Path, intervalle: Duration) {
    let debut = Instant::now();
    let mut dernier = Instant::now();
    eprintln!("attente de {}", fichier.display());
    while !fichier.exists() {
        std::thread::sleep(intervalle);
        if dernier.elapsed() >= Duration::from_secs(300) {
            eprintln!("toujours en attente de {} ({} min)", fichier.display(), debut.elapsed().as_secs() / 60);
            dernier = Instant::now();
        }
    }
    eprintln!("{} présent", fichier.display());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verification::tests::dossier;

    #[test]
    fn identifiants() {
        assert!(identifiant_valide("3f2a9c0d1e4b5a67"));
        for id in ["", "../x", "A", "-x", "x.partiel", &"a".repeat(65)] {
            assert!(!identifiant_valide(id), "{id}");
        }
    }

    #[test]
    fn publication_puis_relance() {
        let d = dossier("publication");
        let e = publier(&d, "abc", |t| {
            fs::create_dir_all(t.join("sons")).unwrap();
            fs::write(t.join("sons/a.wav"), b"a").unwrap();
            Ok(())
        })
        .unwrap();
        assert_eq!(e, Etat::Produite);
        assert!(d.join("abc/COMPLET").exists());
        assert!(!d.join("abc.partiel").exists());
        let m = fs::read_to_string(d.join("abc/MANIFESTE")).unwrap();
        assert!(m.contains("ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb  sons/a.wav"), "{m}");
        // Complète : rien n'est refait.
        let e = publier(&d, "abc", |_| panic!("ne doit pas être rappelé")).unwrap();
        assert_eq!(e, Etat::DejaComplete);
    }

    #[test]
    fn echec_rien_de_visible() {
        let d = dossier("publication-echec");
        assert!(publier(&d, "abc", |_| Err("raté".into())).is_err());
        assert!(!d.join("abc").exists());
        // Le reste partiel est repris de zéro au lancement suivant.
        assert!(publier(&d, "abc", |t| fs::write(t.join("x"), b"x").map_err(|e| e.to_string())).is_ok());
        assert!(d.join("abc/x").exists());
    }

    #[test]
    fn attente() {
        let d = dossier("attente");
        let f = d.join("COMPLET");
        let g = f.clone();
        let t = std::thread::spawn(move || attendre(&g, Duration::from_millis(5)));
        std::thread::sleep(Duration::from_millis(30));
        fs::write(&f, "").unwrap();
        t.join().unwrap();
    }
}
