//! Contrôle d'un fichier reçu : taille, SHA-256, puis licence inscrite dans
//! l'en-tête d'un GLB.
//!
//! Sketchfab inscrit l'auteur et la licence dans `asset.extras` de chaque
//! GLB : si elle y est, elle doit être celle du manifeste. Un modèle dont
//! l'auteur aurait changé la licence est ainsi refusé même si quelqu'un a mis
//! à jour l'empreinte sans regarder.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::manifeste::Fichier;

/// SHA-256 (hexadécimal) et taille d'un fichier.
pub fn empreinte(chemin: &Path) -> io::Result<(String, u64)> {
    let mut f = File::open(chemin)?;
    let mut h = Sha256::new();
    let mut tampon = vec![0u8; 1 << 16];
    let mut n = 0u64;
    loop {
        let k = f.read(&mut tampon)?;
        if k == 0 {
            break;
        }
        h.update(&tampon[..k]);
        n += k as u64;
    }
    let hexa = h.finalize().iter().map(|o| format!("{o:02x}")).collect();
    Ok((hexa, n))
}

/// Taille, SHA-256 et, pour un GLB, licence de l'en-tête.
pub fn verifier(f: &Fichier, licence: &str, chemin: &Path) -> Result<(), String> {
    let (sha256, octets) = empreinte(chemin).map_err(|e| format!("{}: {e}", chemin.display()))?;
    if octets != f.octets {
        return Err(format!("taille {octets} au lieu de {}", f.octets));
    }
    if sha256 != f.sha256 {
        return Err(format!("SHA-256 {sha256} au lieu de {}", f.sha256));
    }
    if f.nom.ends_with(".glb") {
        let entete = entete_glb(chemin)?;
        if let Some(l) = licence_glb(&entete)? {
            // Sketchfab : « CC-BY-4.0 (http://creativecommons.org/licenses/by/4.0/) ».
            if l.split_whitespace().next() != Some(licence) {
                return Err(format!("licence du GLB « {l} », manifeste : {licence}"));
            }
        }
    }
    Ok(())
}

/// Bloc JSON d'un GLB (glTF binaire : en-tête de 12 octets, puis le bloc JSON).
fn entete_glb(chemin: &Path) -> Result<Vec<u8>, String> {
    let mut f = File::open(chemin).map_err(|e| format!("{}: {e}", chemin.display()))?;
    let mut t = [0u8; 20];
    f.read_exact(&mut t).map_err(|e| format!("GLB tronqué : {e}"))?;
    let mot = |i: usize| u32::from_le_bytes([t[i], t[i + 1], t[i + 2], t[i + 3]]);
    if &t[..4] != b"glTF" || mot(4) != 2 || &t[16..20] != b"JSON" {
        return Err("pas un GLB glTF 2.0".to_owned());
    }
    let n = u64::from(mot(12));
    if n > 64 << 20 {
        return Err(format!("bloc JSON du GLB trop grand : {n} octets"));
    }
    let mut json = Vec::new();
    f.take(n).read_to_end(&mut json).map_err(|e| format!("GLB : {e}"))?;
    if json.len() as u64 != n {
        return Err("GLB tronqué".to_owned());
    }
    Ok(json)
}

fn licence_glb(json: &[u8]) -> Result<Option<String>, String> {
    let v: serde_json::Value = serde_json::from_slice(json).map_err(|e| format!("JSON du GLB : {e}"))?;
    Ok(v.pointer("/asset/extras/license").and_then(|l| l.as_str()).map(str::to_owned))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// GLB minimal dont l'en-tête porte `extras`.
    pub fn glb(extras: &str) -> Vec<u8> {
        let mut json = format!(r#"{{"asset":{{"version":"2.0","extras":{extras}}}}}"#).into_bytes();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let mut o = b"glTF".to_vec();
        o.extend(2u32.to_le_bytes());
        o.extend((12 + 8 + json.len() as u32).to_le_bytes());
        o.extend((json.len() as u32).to_le_bytes());
        o.extend(b"JSON");
        o.extend(json);
        o
    }

    pub fn fichier_pour(nom: &str, contenu: &[u8], dossier: &Path) -> (Fichier, std::path::PathBuf) {
        let chemin = dossier.join(nom);
        std::fs::write(&chemin, contenu).unwrap();
        let (sha256, octets) = empreinte(&chemin).unwrap();
        let f = Fichier {
            url: Some("https://opengameart.org/x".into()),
            sketchfab: None,
            nom: nom.into(),
            octets,
            sha256,
            extraire: vec![],
        };
        (f, chemin)
    }

    pub fn dossier(nom: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("beta1-assets-{nom}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn empreinte_connue() {
        let d = dossier("empreinte");
        let (f, _) = fichier_pour("abc.txt", b"abc", &d);
        assert_eq!(f.sha256, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(f.octets, 3);
    }

    #[test]
    fn taille_et_empreinte() {
        let d = dossier("taille");
        let (mut f, chemin) = fichier_pour("a.wav", b"abc", &d);
        assert!(verifier(&f, "CC0-1.0", &chemin).is_ok());
        f.octets = 4;
        assert!(verifier(&f, "CC0-1.0", &chemin).unwrap_err().contains("taille"));
        f.octets = 3;
        f.sha256 = "0".repeat(64);
        assert!(verifier(&f, "CC0-1.0", &chemin).unwrap_err().contains("SHA-256"));
    }

    #[test]
    fn licence_des_glb() {
        let d = dossier("glb");
        let cc_by = r#"{"license":"CC-BY-4.0 (http://creativecommons.org/licenses/by/4.0/)","author":"x"}"#;
        let (f, chemin) = fichier_pour("a.glb", &glb(cc_by), &d);
        assert!(verifier(&f, "CC-BY-4.0", &chemin).is_ok());
        assert!(verifier(&f, "CC0-1.0", &chemin).unwrap_err().contains("licence"));
        // Pas de licence dans l'en-tête (Quaternius) : seul le manifeste fait foi.
        let (f, chemin) = fichier_pour("b.glb", &glb("{}"), &d);
        assert!(verifier(&f, "CC0-1.0", &chemin).is_ok());
        let (f, chemin) = fichier_pour("c.glb", b"pas un glb, assez long pour l'en-tete", &d);
        assert!(verifier(&f, "CC0-1.0", &chemin).is_err());
    }
}
