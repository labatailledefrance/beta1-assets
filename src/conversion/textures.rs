//! Tableau de textures du sol et des bâtiments (`textures.p24t`), porté de
//! `p26-france/outils/textures.py`.
//!
//! Chaque texture Poly Haven est réduite à 512 px (Lanczos, comme PIL), puis
//! ramenée à un détail autour de sa moyenne (× 0,4) : la teinte vient du jeu.
//! Format (petit-boutien) : `"P24T"`, u32 côté, u32 nombre de couches, puis
//! pour chaque couche ses mipmaps RGBA 8 bits (sRGB), de 512 à 1 px.
//!
//! Le décodeur JPEG (zune-jpeg) n'est pas celui de PIL (libjpeg-turbo) : les
//! octets diffèrent d'un niveau ou deux de ceux de P-26, sans effet visible.

use std::fs;
use std::path::Path;

use zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions};

pub const COTE: usize = 512;

/// (id du manifeste, fichier, saturation gardée), dans l'ordre des couches
/// attendu par le client.
pub const COUCHES: [(&str, &str, f64); 12] = [
    ("polyhaven-aerial-grass-rock", "aerial_grass_rock.jpg", 0.35),
    ("polyhaven-aerial-sand", "aerial_sand.jpg", 0.6),
    ("polyhaven-brown-mud", "brown_mud.jpg", 0.8),
    ("polyhaven-cliff-side", "cliff_side.jpg", 0.45),
    ("polyhaven-forest-ground-04", "forest_ground_04.jpg", 0.7),
    ("polyhaven-asphalt-02", "asphalt_02.jpg", 0.5),
    ("polyhaven-castle-wall-slates", "castle_wall_slates.jpg", 0.55),
    ("polyhaven-roof-slates-02", "roof_slates_02.jpg", 0.4),
    ("polyhaven-roof-tiles-14", "roof_tiles_14.jpg", 0.8),
    ("polyhaven-cobblestone-floor-04", "cobblestone_floor_04.jpg", 0.5),
    ("polyhaven-old-stone-wall-02", "old_stone_wall_02.jpg", 0.5),
    ("polyhaven-leafy-grass", "leafy_grass.jpg", 0.5),
];

/// RGB 8 bits, (largeur, hauteur, pixels).
fn decoder(chemin: &Path) -> Result<(usize, usize, Vec<u8>), String> {
    let d = fs::read(chemin).map_err(|e| format!("{}: {e}", chemin.display()))?;
    let o = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut dec = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(&d), o);
    let px = dec.decode().map_err(|e| format!("{}: {e:?}", chemin.display()))?;
    let (l, h) = dec.dimensions().ok_or_else(|| format!("{}: dimensions inconnues", chemin.display()))?;
    if px.len() != l * h * 3 {
        return Err(format!("{}: {} octets pour {l} × {h}", chemin.display(), px.len()));
    }
    Ok((l, h, px))
}

fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        1.0
    } else {
        let p = x * std::f64::consts::PI;
        p.sin() / p
    }
}

fn lanczos(x: f64) -> f64 {
    if -3.0 < x && x < 3.0 { sinc(x) * sinc(x / 3.0) } else { 0.0 }
}

/// Coefficients de PIL (`precompute_coeffs`) : (premier indice, poids normalisés).
fn coefficients(entree: usize, sortie: usize) -> Vec<(usize, Vec<f64>)> {
    let echelle = entree as f64 / sortie as f64;
    let filtre = echelle.max(1.0);
    let support = 3.0 * filtre;
    (0..sortie)
        .map(|x| {
            let centre = (x as f64 + 0.5) * echelle;
            let min = ((centre - support + 0.5) as i64).max(0) as usize;
            let max = ((centre + support + 0.5) as usize).min(entree);
            let mut k: Vec<f64> = (min..max).map(|xx| lanczos((xx as f64 - centre + 0.5) / filtre)).collect();
            let s: f64 = k.iter().sum();
            if s != 0.0 {
                k.iter_mut().for_each(|w| *w /= s);
            }
            (min, k)
        })
        .collect()
}

fn octet(x: f64) -> u8 {
    x.round().clamp(0.0, 255.0) as u8
}

/// Redimensionnement Lanczos séparable, horizontal puis vertical, avec un
/// passage intermédiaire en 8 bits, comme PIL.
fn redimensionner(l: usize, h: usize, px: &[u8], cote: usize) -> Vec<u8> {
    let ch = coefficients(l, cote);
    let mut tmp = vec![0u8; cote * h * 3];
    for y in 0..h {
        for (x, (min, k)) in ch.iter().enumerate() {
            for c in 0..3 {
                let s: f64 = k.iter().enumerate().map(|(i, w)| w * f64::from(px[(y * l + min + i) * 3 + c])).sum();
                tmp[(y * cote + x) * 3 + c] = octet(s);
            }
        }
    }
    let cv = coefficients(h, cote);
    let mut o = vec![0u8; cote * cote * 3];
    for (y, (min, k)) in cv.iter().enumerate() {
        for x in 0..cote {
            for c in 0..3 {
                let s: f64 =
                    k.iter().enumerate().map(|(i, w)| w * f64::from(tmp[((min + i) * cote + x) * 3 + c])).sum();
                o[(y * cote + x) * 3 + c] = octet(s);
            }
        }
    }
    o
}

/// Une couche et ses mipmaps, RGBA.
fn couche(px: &[u8], sat: f64, o: &mut Vec<u8>) {
    let n = COTE * COTE;
    let lin: Vec<f64> = (0..256).map(|i| (f64::from(i) / 255.0).powf(2.2)).collect();
    let mut l: [Vec<f64>; 3] = std::array::from_fn(|k| (0..n).map(|i| lin[px[i * 3 + k] as usize]).collect());
    let m: [f64; 3] = std::array::from_fn(|k| l[k].iter().sum::<f64>() / n as f64);
    for i in 0..n {
        let (r, v, b) = (l[0][i] / m[0], l[1][i] / m[1], l[2][i] / m[2]);
        let g = (r + v + b) / 3.0;
        for (k, x) in [r, v, b].into_iter().enumerate() {
            l[k][i] = ((g + (x - g) * sat) * 0.4).clamp(0.0, 1.0);
        }
    }
    let mut w = COTE;
    loop {
        let debut = o.len();
        o.resize(debut + w * w * 4, 0xff);
        for (k, c) in l.iter().enumerate() {
            for (i, x) in c.iter().enumerate() {
                o[debut + i * 4 + k] = (x.powf(1.0 / 2.2) * 255.0 + 0.5) as u8;
            }
        }
        if w == 1 {
            break;
        }
        let h = w / 2;
        l = std::array::from_fn(|k| {
            let c = &l[k];
            (0..h * h)
                .map(|p| {
                    let (j, i) = (p / h, p % h);
                    (c[2 * j * w + 2 * i]
                        + c[2 * j * w + 2 * i + 1]
                        + c[(2 * j + 1) * w + 2 * i]
                        + c[(2 * j + 1) * w + 2 * i + 1])
                        / 4.0
                })
                .collect()
        });
        w = h;
    }
}

/// Produit `textures.p24t` (contenu du fichier).
pub fn convertir(racine: &Path) -> Result<Vec<u8>, String> {
    let mut o = b"P24T".to_vec();
    o.extend((COTE as u32).to_le_bytes());
    o.extend((COUCHES.len() as u32).to_le_bytes());
    for (id, nom, sat) in COUCHES {
        let (l, h, px) = decoder(&racine.join(id).join(nom))?;
        couche(&redimensionner(l, h, &px, COTE), sat, &mut o);
    }
    Ok(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coefficients_normalises() {
        for (_, k) in coefficients(1024, 512) {
            assert!((k.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        }
        // Agrandissement : support de 3 px.
        assert!(coefficients(4, 8).iter().all(|(_, k)| k.len() <= 6));
    }

    #[test]
    fn uniforme_reste_uniforme() {
        let px = vec![128u8; 8 * 8 * 3];
        assert!(redimensionner(8, 8, &px, 4).iter().all(|&v| v == 128));
    }
}
