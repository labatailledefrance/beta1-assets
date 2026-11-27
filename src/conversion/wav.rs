//! WAV PCM : lecture (16 ou 24 bits, mono ou stéréo) et écriture (mono 16 bits).
//!
//! Lecture comme le module `wave` de Python, utilisé par P-25 : on ne garde
//! que les blocs `fmt ` et `data`. Le reste du format (WAVE_FORMAT_EXTENSIBLE,
//! blocs LIST…) est ignoré.

use std::path::Path;

pub struct Wav {
    pub canaux: usize,
    /// Octets par échantillon (2 ou 3).
    pub largeur: usize,
    pub frequence: u32,
    pub donnees: Vec<u8>,
}

fn u16_le(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]))
}

fn u32_le(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*b.get(i)?, *b.get(i + 1)?, *b.get(i + 2)?, *b.get(i + 3)?]))
}

pub fn lire(chemin: &Path) -> Result<Wav, String> {
    let b = std::fs::read(chemin).map_err(|e| format!("{}: {e}", chemin.display()))?;
    decoder(&b).map_err(|e| format!("{}: {e}", chemin.display()))
}

pub fn decoder(b: &[u8]) -> Result<Wav, String> {
    if b.get(..4) != Some(b"RIFF") || b.get(8..12) != Some(b"WAVE") {
        return Err("pas un fichier WAV".to_owned());
    }
    let mut i = 12;
    let mut fmt = None;
    while i + 8 <= b.len() {
        let nom = &b[i..i + 4];
        let n = u32_le(b, i + 4).ok_or("bloc tronqué")? as usize;
        let debut = i + 8;
        let fin = debut.checked_add(n).ok_or("bloc trop grand")?;
        if nom == b"fmt " {
            let f = b.get(debut..fin).ok_or("bloc fmt tronqué")?;
            let canaux = u16_le(f, 2).ok_or("fmt court")? as usize;
            let frequence = u32_le(f, 4).ok_or("fmt court")?;
            let bits = u16_le(f, 14).ok_or("fmt court")? as usize;
            fmt = Some((canaux, bits.div_ceil(8), frequence));
        } else if nom == b"data" {
            let (canaux, largeur, frequence) = fmt.ok_or("bloc data avant fmt")?;
            if canaux == 0 || !(largeur == 2 || largeur == 3) {
                return Err(format!("format non pris en charge : {canaux} canaux, {largeur} octets"));
            }
            // Comme `wave` : des trames entières, dans la limite du fichier.
            let trame = canaux * largeur;
            let dispo = b.len().saturating_sub(debut).min(n);
            let donnees = b[debut..debut + dispo / trame * trame].to_vec();
            return Ok(Wav { canaux, largeur, frequence, donnees });
        }
        // Blocs alignés sur 2 octets.
        i = fin + (n & 1);
    }
    Err("pas de bloc data".to_owned())
}

/// WAV mono 16 bits, en-tête de 44 octets (celui du module `wave`).
pub fn encoder(echantillons: &[i16], frequence: u32) -> Vec<u8> {
    let n = (echantillons.len() * 2) as u32;
    let mut o = Vec::with_capacity(44 + n as usize);
    o.extend(b"RIFF");
    o.extend((36 + n).to_le_bytes());
    o.extend(b"WAVEfmt ");
    o.extend(16u32.to_le_bytes());
    o.extend(1u16.to_le_bytes());
    o.extend(1u16.to_le_bytes());
    o.extend(frequence.to_le_bytes());
    o.extend((frequence * 2).to_le_bytes());
    o.extend(2u16.to_le_bytes());
    o.extend(16u16.to_le_bytes());
    o.extend(b"data");
    o.extend(n.to_le_bytes());
    for e in echantillons {
        o.extend(e.to_le_bytes());
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour() {
        let w = decoder(&encoder(&[1, -2, 300], 44100)).unwrap();
        assert_eq!((w.canaux, w.largeur, w.frequence), (1, 2, 44100));
        assert_eq!(w.donnees, [1, 0, 0xfe, 0xff, 0x2c, 0x01]);
        assert!(decoder(b"RIFF\0\0\0\0WAVE").is_err());
        assert!(decoder(b"pas un wav").is_err());
    }
}
