//! Sons du jeu, portés de `p25-manche/outils/sons.py` : coups isolés dans les
//! enregistrements CC0, mono 16 bits 44,1 kHz ; réacteur synthétisé.
//!
//! Mêmes calculs, dans le même ordre, que le script Python (flottants 64 bits,
//! troncatures de `int()`) : les fichiers produits sont identiques octet pour
//! octet à ceux de P-25 et P-26.

use std::fs;
use std::path::Path;

use super::{mt::Mt, wav};

pub const R: u32 = 44_100;

/// Fichiers lus (id du manifeste, nom).
pub const ENTREES: [(&str, &str); 17] = [
    ("oga-gunshot-sounds", "sks.wav"),
    ("oga-gunshot-sounds", "mosin.wav"),
    ("oga-explosion", "explosion.wav"),
    ("oga-muffled-distant-explosion", "lointaine.wav"),
    ("oga-racing-car-engine", "loop_0.wav"),
    ("oga-wind", "Wind.ogg"),
    ("oga-beach-ocean-waves", "jasinski-wave-prev.ogg"),
    ("oga-zombies-sound-pack", "zombie-1.wav"),
    ("oga-zombies-sound-pack", "zombie-5.wav"),
    ("oga-zombies-sound-pack", "zombie-8.wav"),
    ("oga-zombies-sound-pack", "zombie-10.wav"),
    ("oga-zombies-sound-pack", "zombie-12.wav"),
    ("oga-zombies-sound-pack", "zombie-15.wav"),
    ("oga-zombies-sound-pack", "zombie-16.wav"),
    ("oga-zombies-sound-pack", "zombie-17.wav"),
    ("oga-zombies-sound-pack", "zombie-18.wav"),
    ("oga-zombies-sound-pack", "zombie-21.wav"),
];

/// Mono, en flottants dans [-1, 1], rééchantillonné à 44,1 kHz (plus proche voisin).
fn lire(chemin: &Path) -> Result<Vec<f64>, String> {
    let w = wav::lire(chemin)?;
    let (v, e): (Vec<i64>, f64) = if w.largeur == 2 {
        (w.donnees.as_chunks::<2>().0.iter().map(|o| i64::from(i16::from_le_bytes(*o))).collect(), 32768.0)
    } else {
        let v = w
            .donnees
            .as_chunks::<3>()
            .0
            .iter()
            .map(|o| i64::from(i32::from_le_bytes([0, o[0], o[1], o[2]]) >> 8))
            .collect();
        (v, f64::from(1u32 << 23))
    };
    let c = w.canaux;
    let mut m = Vec::with_capacity(v.len() / c);
    let mut i = 0;
    while i + c <= v.len() {
        let s: i64 = v[i..i + c].iter().sum();
        m.push(s as f64 / c as f64 / e);
        i += c;
    }
    if m.is_empty() {
        return Err(format!("{} : aucun échantillon", chemin.display()));
    }
    let r = f64::from(w.frequence);
    let n2 = (m.len() as f64 * f64::from(R) / r) as usize;
    Ok((0..n2).map(|i| m[(m.len() - 1).min((i as f64 * r / f64::from(R)) as usize)]).collect())
}

fn crete(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |p, x| if x.abs() > p { x.abs() } else { p })
}

fn encoder(v: &[f64], niveau: f64) -> Vec<u8> {
    let p = crete(v);
    let p = if p == 0.0 { 1.0 } else { p };
    let e: Vec<i16> = v.iter().map(|x| (x / p * niveau * 32767.0) as i16).collect();
    wav::encoder(&e, R)
}

fn vitesse(v: &[f64], k: f64) -> Vec<f64> {
    (0..(v.len() as f64 / k) as usize).map(|i| v[(i as f64 * k) as usize]).collect()
}

/// Garde `t` secondes, avec un fondu quadratique sur les derniers 40 %.
fn coupe(v: &[f64], t: f64) -> Vec<f64> {
    let n = (t * f64::from(R)) as usize;
    let v = &v[..n.min(v.len())];
    let f = v.len().min((0.4 * n as f64) as usize);
    v.iter()
        .enumerate()
        .map(|(i, x)| if i < v.len() - f { *x } else { x * ((v.len() - i) as f64 / f as f64).powf(2.0) })
        .collect()
}

/// Débuts des coups de feu : dépassement de la moitié de la crête, espacés d'au moins `mini` s.
fn coups(v: &[f64], mini: f64) -> Vec<usize> {
    let p = crete(v);
    let mut out = Vec::new();
    let mut der = -(i64::from(R) * 9);
    for (i, x) in v.iter().enumerate() {
        let i = i as i64;
        if x.abs() > 0.5 * p && (i - der) as f64 > mini * f64::from(R) {
            out.push((i - (0.004 * f64::from(R)) as i64).max(0) as usize);
            der = i;
        }
    }
    out
}

fn depuis(v: &[f64], i: usize) -> &[f64] {
    &v[i.min(v.len())..]
}

fn reacteur() -> Vec<f64> {
    let mut rng = Mt::python(1);
    let (mut a, mut b) = (0.0f64, 0.0f64);
    let mut v = Vec::with_capacity(4 * R as usize);
    for i in 0..4 * R as usize {
        let x = rng.uniform(-1.0, 1.0);
        a += 0.09 * (x - a);
        b += 0.02 * (a - b);
        v.push(a * 0.6 + b * 2.0 + 0.03 * (i as f64 * 0.31).sin());
    }
    // Bouclé par fondu.
    let f = R as usize / 2;
    let n = v.len();
    let mut o: Vec<f64> =
        (0..f).map(|i| v[i] * i as f64 / f as f64 + v[n - f + i] * (1.0 - i as f64 / f as f64)).collect();
    o.extend_from_slice(&v[f..n - f]);
    o
}

/// Produit `sortie/*.wav` et `sortie/*.ogg` depuis la racine des téléchargements.
pub fn convertir(racine: &Path, sortie: &Path) -> Result<Vec<String>, String> {
    fs::create_dir_all(sortie).map_err(|e| format!("{}: {e}", sortie.display()))?;
    let src = |id: &str, nom: &str| racine.join(id).join(nom);
    let mut produits = Vec::new();
    let mut ecrire = |nom: &str, v: &[f64], niveau: f64| -> Result<(), String> {
        let c = sortie.join(format!("{nom}.wav"));
        fs::write(&c, encoder(v, niveau)).map_err(|e| format!("{}: {e}", c.display()))?;
        produits.push(format!("{nom}.wav"));
        Ok(())
    };
    let sks = lire(&src("oga-gunshot-sounds", "sks.wav"))?;
    let mos = lire(&src("oga-gunshot-sounds", "mosin.wav"))?;
    let (cs, cm) = (coups(&sks, 0.8), coups(&mos, 1.2));
    if cs.is_empty() || cm.is_empty() {
        return Err("aucun coup de feu détecté".to_owned());
    }
    for k in 0..3 {
        ecrire(&format!("fusil_{k}"), &coupe(depuis(&sks, cs[k % cs.len()]), 0.55), 0.9)?;
    }
    for k in 0..2 {
        ecrire(&format!("mitrailleuse_{k}"), &coupe(&vitesse(depuis(&sks, cs[(k + 1) % cs.len()]), 1.12), 0.28), 0.9)?;
    }
    ecrire("canon25", &coupe(&vitesse(depuis(&mos, cm[0]), 0.7), 0.6), 0.9)?;
    let ex = lire(&src("oga-explosion", "explosion.wav"))?;
    let lo = lire(&src("oga-muffled-distant-explosion", "lointaine.wav"))?;
    let grave = vitesse(&ex, 0.6);
    let mix: Vec<f64> =
        (0..lo.len().min(3 * R as usize)).map(|i| if i < grave.len() { grave[i] } else { 0.0 } + 0.9 * lo[i]).collect();
    ecrire("canon120", &coupe(&mix, 2.2), 0.9)?;
    ecrire("explosion_0", &coupe(&vitesse(&ex, 0.75), 1.3), 0.9)?;
    let lent = vitesse(&ex, 0.5);
    let n = (lent.len() + 9 * R as usize).min(lo.len());
    let ex1: Vec<f64> = (0..n).map(|i| 0.5 * lent.get(i).copied().unwrap_or(0.0) + lo[i]).collect();
    ecrire("explosion_1", &coupe(&ex1, 3.0), 0.9)?;
    for (i, k) in [1, 5, 8, 10, 12, 15, 16, 17, 18, 21].iter().enumerate() {
        ecrire(&format!("zombie_{i}"), &lire(&src("oga-zombies-sound-pack", &format!("zombie-{k}.wav")))?, 0.8)?;
    }
    let m = lire(&src("oga-racing-car-engine", "loop_0.wav"))?;
    ecrire("moteur", &[m.as_slice(), m.as_slice()].concat(), 0.7)?;
    ecrire("reacteur", &reacteur(), 0.7)?;
    for (id, nom, but) in
        [("oga-wind", "Wind.ogg", "vent.ogg"), ("oga-beach-ocean-waves", "jasinski-wave-prev.ogg", "vagues.ogg")]
    {
        fs::copy(src(id, nom), sortie.join(but)).map_err(|e| format!("{id}/{nom}: {e}"))?;
        produits.push(but.to_owned());
    }
    Ok(produits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coupe_et_fondu() {
        let v = vec![1.0; 2 * R as usize];
        let c = coupe(&v, 1.0);
        assert_eq!(c.len(), R as usize);
        assert_eq!(c[0], 1.0);
        assert!(c[c.len() - 1] < 1e-6);
    }

    #[test]
    fn detection_des_coups() {
        let mut v = vec![0.0; 3 * R as usize];
        v[1000] = 1.0;
        v[1100] = 1.0; // trop proche : ignoré
        v[2 * R as usize] = -1.0;
        assert_eq!(coups(&v, 0.8), [1000 - 176, 2 * R as usize - 176]);
    }

    #[test]
    fn reacteur_boucle() {
        let v = reacteur();
        assert_eq!(v.len(), 4 * R as usize - R as usize / 2);
    }
}
