//! Extraction des entrées listées d'une archive zip vérifiée.
//!
//! L'archive a déjà passé le contrôle du SHA-256 : son contenu est figé. Le
//! zip contrôle en plus le CRC de chaque entrée. On n'extrait que les entrées
//! du manifeste, sous leur seul nom de fichier, avec une taille plafonnée.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

use crate::manifeste::{Fichier, OCTETS_MAX};
use crate::telechargement::partiel;

/// Extrait dans `dossier` les entrées absentes ; rend le nombre d'entrées écrites.
pub fn extraire(f: &Fichier, archive: &Path, dossier: &Path) -> Result<usize, String> {
    let a_faire: Vec<(&str, &str)> = f.extraits().filter(|(_, nom)| !dossier.join(nom).exists()).collect();
    if a_faire.is_empty() {
        return Ok(0);
    }
    let lecteur = File::open(archive).map_err(|e| format!("{}: {e}", archive.display()))?;
    let mut zip = zip::ZipArchive::new(lecteur).map_err(|e| format!("{}: {e}", archive.display()))?;
    for (entree, nom) in &a_faire {
        let mut e = zip.by_name(entree).map_err(|e| format!("entrée {entree} : {e}"))?;
        if e.size() > OCTETS_MAX {
            return Err(format!("entrée {entree} trop grande : {} octets", e.size()));
        }
        let but = dossier.join(nom);
        let tmp = partiel(&but);
        if let Err(err) = copier(&mut e, &tmp) {
            let _ = fs::remove_file(&tmp);
            return Err(format!("entrée {entree} : {err}"));
        }
        fs::rename(&tmp, &but).map_err(|e| format!("renommage : {e}"))?;
    }
    Ok(a_faire.len())
}

/// Copie plafonnée ; le CRC de l'entrée est contrôlé en fin de lecture.
fn copier(entree: &mut impl Read, but: &Path) -> io::Result<()> {
    let mut sortie = File::create(but)?;
    let n = io::copy(&mut entree.take(OCTETS_MAX + 1), &mut sortie)?;
    if n > OCTETS_MAX {
        return Err(io::Error::other("entrée plus grande qu'annoncé"));
    }
    sortie.flush()?;
    sortie.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verification::tests::{dossier, fichier_pour};
    use zip::write::SimpleFileOptions;

    fn archive(entrees: &[(&str, &[u8])]) -> Vec<u8> {
        let mut z = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        for (n, c) in entrees {
            z.start_file(*n, SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated))
                .unwrap();
            z.write_all(c).unwrap();
        }
        z.finish().unwrap().into_inner()
    }

    #[test]
    fn extraction_des_seules_entrees_listees() {
        let d = dossier("archive");
        let zip = archive(&[("sons/a.wav", b"aaaa"), ("sons/b.wav", b"bbbb"), ("__MACOSX/._a.wav", b"x")]);
        let (mut f, chemin) = fichier_pour("x.zip", &zip, &d);
        f.extraire = vec!["sons/a.wav".into()];
        assert_eq!(extraire(&f, &chemin, &d).unwrap(), 1);
        assert_eq!(fs::read(d.join("a.wav")).unwrap(), b"aaaa");
        assert!(!d.join("b.wav").exists());
        // Déjà extrait : rien à refaire.
        assert_eq!(extraire(&f, &chemin, &d).unwrap(), 0);
        f.extraire = vec!["sons/c.wav".into()];
        assert!(extraire(&f, &chemin, &d).is_err());
    }
}
