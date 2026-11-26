//! Manifeste des assets : `manifeste.toml`, compilé dans le binaire.
//!
//! Comme un lockfile : chaque fichier a une taille et un SHA-256 figés, relus
//! en PR. La validation refuse ce qui sortirait des règles du projet (licence
//! hors liste, hôte inconnu, HTTP en clair) et tout ce qui pourrait servir à
//! écrire hors de la racine (noms limités à `[A-Za-z0-9._-]`, entrées d'archive
//! sans `..`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Le manifeste du dépôt.
pub const MANIFESTE: &str = include_str!("../manifeste.toml");

/// Licences acceptées : le jeu déployé sert les assets aux joueurs.
pub const LICENCES: [&str; 3] = ["CC0-1.0", "CC-BY-4.0", "OFL-1.1"];

/// Hôtes de téléchargement autorisés (HTTPS seul).
pub const HOTES: [&str; 4] = ["static.poly.pizza", "dl.polyhaven.org", "opengameart.org", "raw.githubusercontent.com"];

/// Hôtes des pages de présentation (citées dans les crédits).
pub const HOTES_PAGES: [&str; 5] = ["sketchfab.com", "poly.pizza", "polyhaven.com", "opengameart.org", "github.com"];

/// Taille maximale d'un fichier ou d'une entrée extraite : un serveur ou une
/// archive ne remplit pas le disque.
pub const OCTETS_MAX: u64 = 256 << 20;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Brut {
    asset: Vec<Asset>,
}

/// Une œuvre : l'unité d'attribution.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    /// Dossier local : `<racine>/<id>/`.
    pub id: String,
    pub titre: String,
    pub auteur: String,
    /// Identifiant SPDX, dans [`LICENCES`].
    pub licence: String,
    /// Page de présentation de l'œuvre.
    pub page: String,
    /// Ce que le projet en fait (CC-BY demande de l'indiquer).
    pub modifications: String,
    #[serde(default)]
    pub remarque: Option<String>,
    pub fichier: Vec<Fichier>,
}

/// Un fichier à télécharger.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fichier {
    #[serde(default)]
    pub url: Option<String>,
    /// Uid d'un modèle Sketchfab (lien de téléchargement donné par l'API).
    #[serde(default)]
    pub sketchfab: Option<String>,
    /// Nom local.
    pub nom: String,
    pub octets: u64,
    pub sha256: String,
    /// Entrées d'une archive zip à extraire à côté d'elle.
    #[serde(default)]
    pub extraire: Vec<String>,
}

/// D'où vient un fichier (validé).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origine<'a> {
    Url(&'a str),
    Sketchfab(&'a str),
}

impl Fichier {
    pub fn origine(&self) -> Origine<'_> {
        match (&self.url, &self.sketchfab) {
            (_, Some(uid)) => Origine::Sketchfab(uid),
            (Some(url), None) => Origine::Url(url),
            // Impossible après validation ; une URL vide échouera au téléchargement.
            (None, None) => Origine::Url(""),
        }
    }

    /// `<racine>/<id>/<nom>`.
    pub fn chemin(&self, racine: &Path, asset: &Asset) -> PathBuf {
        racine.join(&asset.id).join(&self.nom)
    }

    /// (entrée de l'archive, nom local) des fichiers à extraire.
    pub fn extraits(&self) -> impl Iterator<Item = (&str, &str)> {
        self.extraire.iter().map(|e| (e.as_str(), e.rsplit('/').next().unwrap_or_default()))
    }
}

/// Lit et valide un manifeste ; rend toutes les erreurs, pas seulement la première.
pub fn lire(texte: &str) -> Result<Vec<Asset>, Vec<String>> {
    let brut: Brut = toml::from_str(texte).map_err(|e| vec![format!("manifeste illisible : {e}")])?;
    let mut erreurs = Vec::new();
    let mut ids = HashSet::new();
    for a in &brut.asset {
        if !ids.insert(a.id.as_str()) {
            erreurs.push(format!("{} : identifiant en double", a.id));
        }
        for e in valider(a) {
            erreurs.push(format!("{} : {e}", a.id));
        }
    }
    if erreurs.is_empty() { Ok(brut.asset) } else { Err(erreurs) }
}

/// Le manifeste du dépôt, validé.
pub fn charger() -> Result<Vec<Asset>, Vec<String>> {
    lire(MANIFESTE)
}

fn valider(a: &Asset) -> Vec<String> {
    let mut e = Vec::new();
    let id_ok = !a.id.is_empty()
        && a.id.len() <= 64
        && !a.id.starts_with('-')
        && a.id.bytes().all(|o| o.is_ascii_lowercase() || o.is_ascii_digit() || o == b'-');
    if !id_ok {
        e.push("identifiant invalide (minuscules, chiffres, tirets)".to_owned());
    }
    for (champ, v) in [("titre", &a.titre), ("auteur", &a.auteur), ("modifications", &a.modifications)] {
        if let Err(m) = texte(v) {
            e.push(format!("{champ} {m}"));
        }
    }
    if let Some(r) = &a.remarque
        && let Err(m) = texte(r)
    {
        e.push(format!("remarque {m}"));
    }
    if !LICENCES.contains(&a.licence.as_str()) {
        e.push(format!("licence refusée : {} (acceptées : {})", a.licence, LICENCES.join(", ")));
    }
    if let Err(m) = url(&a.page, &HOTES_PAGES) {
        e.push(format!("page {m}"));
    }
    if a.fichier.is_empty() {
        e.push("aucun fichier".to_owned());
    }
    let mut noms = HashSet::new();
    for f in &a.fichier {
        e.extend(fichier(f, &a.licence).into_iter().map(|m| format!("{} : {m}", f.nom)));
        if !noms.insert(f.nom.as_str()) {
            e.push(format!("{} : nom en double", f.nom));
        }
        for (_, nom) in f.extraits() {
            if !noms.insert(nom) {
                e.push(format!("{} : nom extrait en double : {nom}", f.nom));
            }
        }
    }
    e
}

fn fichier(f: &Fichier, licence: &str) -> Vec<String> {
    let mut e = Vec::new();
    if let Err(m) = segment(&f.nom) {
        e.push(format!("nom {m}"));
    }
    if f.nom.ends_with(".partiel") {
        e.push("nom réservé (.partiel)".to_owned());
    }
    match (&f.url, &f.sketchfab) {
        (Some(_), Some(_)) | (None, None) => e.push("il faut soit `url`, soit `sketchfab`".to_owned()),
        (Some(u), None) => {
            if let Err(m) = url(u, &HOTES) {
                e.push(format!("url {m}"));
            } else if let Some(chemin) = u.strip_prefix("https://raw.githubusercontent.com/") {
                // Un commit, jamais une branche : le contenu ne bouge plus.
                let reference = chemin.split('/').nth(2).unwrap_or_default();
                if !hexa(reference, 40) {
                    e.push("url GitHub sans SHA de commit".to_owned());
                }
            }
        }
        (None, Some(uid)) => {
            if !hexa(uid, 32) {
                e.push(format!("uid Sketchfab invalide : {uid}"));
            }
            if licence != "CC-BY-4.0" && licence != "CC0-1.0" {
                e.push(format!("licence {licence} impossible sur Sketchfab"));
            }
        }
    }
    if f.octets == 0 || f.octets > OCTETS_MAX {
        e.push(format!("taille hors limites : {}", f.octets));
    }
    if !hexa(&f.sha256, 64) {
        e.push(format!("sha256 invalide : {}", f.sha256));
    }
    if !f.extraire.is_empty() && !f.nom.ends_with(".zip") {
        e.push("`extraire` sur un fichier qui n'est pas un .zip".to_owned());
    }
    for x in &f.extraire {
        let ok = !x.starts_with('/')
            && !x.contains('\\')
            && x.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
            && segment(x.rsplit('/').next().unwrap_or_default()).is_ok();
        if !ok {
            e.push(format!("entrée d'archive refusée : {x:?}"));
        }
    }
    e
}

/// Texte d'une ligne, sans caractère de contrôle.
fn texte(s: &str) -> Result<(), String> {
    if s.trim().is_empty() {
        Err("vide".to_owned())
    } else if s.chars().any(char::is_control) || s.len() > 300 {
        Err("invalide (une ligne, 300 octets au plus)".to_owned())
    } else {
        Ok(())
    }
}

/// URL HTTPS vers un hôte de la liste, sans requête, fragment ni chemin relatif.
fn url(u: &str, hotes: &[&str]) -> Result<(), String> {
    let reste = u.strip_prefix("https://").ok_or_else(|| format!("sans HTTPS : {u}"))?;
    let hote = reste.split('/').next().unwrap_or_default();
    if !hotes.contains(&hote) {
        return Err(format!("hôte non autorisé : {hote}"));
    }
    let refus = u.contains(['?', '#', '\\', ' ', '"', '<', '>'])
        || u.chars().any(char::is_control)
        || reste.split('/').skip(1).any(|s| s == "." || s == "..");
    if refus { Err(format!("refusée : {u}")) } else { Ok(()) }
}

/// Un segment de chemin sûr : `[A-Za-z0-9._-]`, ne commence pas par un point.
fn segment(s: &str) -> Result<(), String> {
    let ok = !s.is_empty()
        && s.len() <= 200
        && !s.starts_with('.')
        && s.bytes().all(|o| o.is_ascii_alphanumeric() || matches!(o, b'.' | b'_' | b'-'));
    if ok { Ok(()) } else { Err(format!("invalide : {s:?}")) }
}

fn hexa(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|o| matches!(o, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BON: &str = r#"
[[asset]]
id = "exemple"
titre = "Exemple"
auteur = "Quelqu'un"
licence = "CC0-1.0"
page = "https://opengameart.org/content/exemple"
modifications = "aucune"
[[asset.fichier]]
url = "https://opengameart.org/sites/default/files/exemple.zip"
nom = "exemple.zip"
octets = 10
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
extraire = ["dossier/a.wav"]
"#;

    fn erreurs(remplacer: &str, par: &str) -> Vec<String> {
        assert!(BON.contains(remplacer), "{remplacer}");
        lire(&BON.replacen(remplacer, par, 1)).err().unwrap_or_default()
    }

    #[test]
    fn manifeste_du_depot_valide() {
        let assets = charger().unwrap_or_else(|e| panic!("{}", e.join("\n")));
        assert!(!assets.is_empty());
    }

    #[test]
    fn exemple_valide() {
        let a = lire(BON).unwrap();
        let f = &a[0].fichier[0];
        assert_eq!(f.origine(), Origine::Url("https://opengameart.org/sites/default/files/exemple.zip"));
        assert_eq!(f.chemin(Path::new("/r"), &a[0]), Path::new("/r/exemple/exemple.zip"));
        assert_eq!(f.extraits().collect::<Vec<_>>(), [("dossier/a.wav", "a.wav")]);
    }

    #[test]
    fn refus() {
        let cas = [
            ("CC0-1.0", "CC-BY-SA-4.0"),
            ("CC0-1.0", "CC-BY-3.0"),
            ("https://opengameart.org/sites", "http://opengameart.org/sites"),
            ("https://opengameart.org/sites", "https://opengameart.org.exemple.fr/sites"),
            ("files/exemple.zip", "files/exemple.zip?x=1"),
            ("files/exemple.zip", "files/../exemple.zip"),
            ("nom = \"exemple.zip\"", "nom = \"../exemple.zip\""),
            ("nom = \"exemple.zip\"", "nom = \".cache\""),
            ("nom = \"exemple.zip\"", "nom = \"exemple.wav\""),
            ("id = \"exemple\"", "id = \"../x\""),
            ("id = \"exemple\"", "id = \"Exemple\""),
            ("\"dossier/a.wav\"", "\"../a.wav\""),
            ("\"dossier/a.wav\"", "\"/etc/a.wav\""),
            ("\"dossier/a.wav\"", "\"dossier/\""),
            ("\"dossier/a.wav\"", "\"exemple.zip\""),
            ("octets = 10", "octets = 0"),
            ("0000000000000000000000000000000000000000000000000000000000000000", "abc"),
            ("auteur = \"Quelqu'un\"", "auteur = \"\""),
            ("auteur = \"Quelqu'un\"", "auteur = \"a\\nb\""),
            ("page = \"https://opengameart.org", "page = \"https://exemple.fr"),
            ("modifications = \"aucune\"", "modifications = \"aucune\"\ninconnu = 1"),
        ];
        for (a, b) in cas {
            assert!(!erreurs(a, b).is_empty(), "accepté : {a} → {b}");
        }
    }

    #[test]
    fn origines() {
        let sf = "sketchfab = \"84a0918d2f534c2eb003ab3cb3029c03\"";
        let url = "url = \"https://opengameart.org/sites/default/files/exemple.zip\"";
        // Sketchfab : uid de 32 caractères hexadécimaux, CC-BY-4.0 ou CC0-1.0.
        assert!(erreurs(url, sf).is_empty());
        assert!(!erreurs(url, "sketchfab = \"84a0918d\"").is_empty());
        assert!(!erreurs(url, &format!("{url}\n{sf}")).is_empty());
        assert!(!erreurs(url, "").is_empty());
        // GitHub : un commit, pas une branche.
        let gh = |r: &str| format!("url = \"https://raw.githubusercontent.com/google/fonts/{r}/ofl/x/exemple.zip\"");
        assert!(erreurs(url, &gh("9a6cc6b8ce992aff77b69c857c46af0b42cdff76")).is_empty());
        assert!(!erreurs(url, &gh("main")).is_empty());
    }

    #[test]
    fn doublons() {
        let deux = format!("{BON}\n{BON}");
        let e = lire(&deux).err().unwrap_or_default();
        assert!(e.iter().any(|m| m.contains("identifiant en double")), "{e:?}");
        let e = erreurs("\"dossier/a.wav\"", "\"a/exemple.zip\"");
        assert!(e.iter().any(|m| m.contains("en double")), "{e:?}");
    }
}
