//! Téléchargement des fichiers du manifeste.
//!
//! Le fichier `x` est reçu dans `x.partiel`, vérifié (taille, SHA-256,
//! licence d'un GLB), puis renommé : un fichier présent sous son nom définitif
//! a été vérifié. Comme un `docker pull` : couche reçue, digest contrôlé, puis
//! seulement rendue visible. Les fichiers sont petits (8 Mo au plus) : pas de
//! reprise par plage, on recommence le fichier.
//!
//! Sketchfab : la licence est relue sur l'API publique avant chaque
//! téléchargement (un auteur peut la changer), puis l'API, avec le jeton,
//! donne un lien temporaire. Ce lien pointe vers le stockage de Sketchfab,
//! dont l'hôte n'est pas fixé : il n'est pas filtré, mais le fichier reçu doit
//! avoir l'empreinte du manifeste.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use ureq::Agent;

use crate::archive;
use crate::manifeste::{Asset, Fichier, Origine};
use crate::verification;

pub const AGENT_HTTP: &str =
    concat!("beta1-assets/", env!("CARGO_PKG_VERSION"), " (+https://github.com/labatailledefrance/beta1-assets)");

pub const API_SKETCHFAB: &str = "https://api.sketchfab.com";

#[derive(Debug, Clone)]
pub struct Options {
    pub racine: PathBuf,
    pub essais: u32,
    /// Attente après le premier échec, doublée à chaque essai.
    pub attente: Duration,
    /// Jeton personnel Sketchfab (`SKETCHFAB_TOKEN`).
    pub jeton_sketchfab: Option<String>,
    pub api_sketchfab: String,
    /// HTTPS seul, redirections comprises (faux seulement pour les tests).
    pub https_seul: bool,
}

impl Options {
    pub fn nouvelles(racine: PathBuf, jeton_sketchfab: Option<String>) -> Self {
        Options {
            racine,
            essais: 5,
            attente: Duration::from_secs(5),
            jeton_sketchfab,
            api_sketchfab: API_SKETCHFAB.to_owned(),
            https_seul: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct Bilan {
    pub recus: usize,
    pub presents: usize,
    pub extraits: usize,
    /// (fichier, dernière erreur).
    pub echecs: Vec<(PathBuf, String)>,
}

/// Télécharge les fichiers absents (ou invalides) des `assets`, puis extrait
/// les entrées d'archive manquantes.
pub fn telecharger(assets: &[Asset], o: &Options) -> Bilan {
    let agent: Agent = Agent::config_builder()
        .user_agent(AGENT_HTTP)
        .https_only(o.https_seul)
        .max_redirects(5)
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .timeout_per_read(Some(Duration::from_secs(60)))
        .build()
        .into();
    let mut b = Bilan::default();
    for a in assets {
        for f in &a.fichier {
            let chemin = f.chemin(&o.racine, a);
            match un_fichier(&agent, a, f, &chemin, o) {
                Ok(Etat::Present) => b.presents += 1,
                Ok(Etat::Recu) => {
                    b.recus += 1;
                    println!("ok {}", chemin.display());
                }
                Err(e) => {
                    eprintln!("échec {} : {e}", chemin.display());
                    b.echecs.push((chemin, e));
                    continue;
                }
            }
            match chemin.parent().map(|d| archive::extraire(f, &chemin, d)) {
                Some(Ok(n)) => b.extraits += n,
                Some(Err(e)) => {
                    eprintln!("échec {} : {e}", chemin.display());
                    b.echecs.push((chemin, e));
                }
                None => {}
            }
        }
    }
    b
}

enum Etat {
    Present,
    Recu,
}

enum Echec {
    /// Inutile de réessayer (licence changée, jeton absent, 404…).
    Definitif(String),
    Passager(String),
}

fn un_fichier(agent: &Agent, a: &Asset, f: &Fichier, but: &Path, o: &Options) -> Result<Etat, String> {
    if but.exists() {
        match verification::verifier(f, &a.licence, but) {
            Ok(()) => return Ok(Etat::Present),
            Err(e) => {
                eprintln!("{} invalide ({e}) : téléchargé à nouveau", but.display());
                fs::remove_file(but).map_err(|e| format!("{}: {e}", but.display()))?;
            }
        }
    }
    let dossier = but.parent().ok_or("chemin sans dossier")?;
    fs::create_dir_all(dossier).map_err(|e| format!("{}: {e}", dossier.display()))?;
    let tmp = partiel(but);
    let mut derniere = String::new();
    for essai in 0..o.essais.max(1) {
        if essai > 0 {
            let attente = o.attente.saturating_mul(1 << (essai - 1).min(10));
            eprintln!("essai {essai} {} dans {} s : {derniere}", but.display(), attente.as_secs());
            std::thread::sleep(attente);
        }
        let r = lien(agent, a, f, o).and_then(|url| recevoir(agent, &url, f.octets, &tmp));
        match r {
            Ok(()) => {}
            Err(Echec::Definitif(e)) => {
                let _ = fs::remove_file(&tmp);
                return Err(e);
            }
            Err(Echec::Passager(e)) => {
                let _ = fs::remove_file(&tmp);
                derniere = e;
                continue;
            }
        }
        match verification::verifier(f, &a.licence, &tmp) {
            Ok(()) => {
                fs::rename(&tmp, but).map_err(|e| format!("renommage : {e}"))?;
                return Ok(Etat::Recu);
            }
            Err(e) => {
                let _ = fs::remove_file(&tmp);
                // Le contenu publié n'est plus celui du manifeste : réessayer
                // ne changera rien.
                return Err(e);
            }
        }
    }
    Err(derniere)
}

/// `x.glb` → `x.glb.partiel`.
pub fn partiel(but: &Path) -> PathBuf {
    let mut p = but.as_os_str().to_owned();
    p.push(".partiel");
    PathBuf::from(p)
}

fn echec_http(e: ureq::Error) -> Echec {
    match e {
        ureq::Error::StatusCode(c) if (400..500).contains(&c) && c != 408 && c != 429 => {
            Echec::Definitif(format!("statut HTTP {c}"))
        }
        e => Echec::Passager(e.to_string()),
    }
}

/// URL à télécharger : celle du manifeste, ou le lien temporaire de Sketchfab.
fn lien(agent: &Agent, a: &Asset, f: &Fichier, o: &Options) -> Result<String, Echec> {
    let uid = match f.origine() {
        Origine::Url(u) => return Ok(u.to_owned()),
        Origine::Sketchfab(uid) => uid,
    };
    let jeton = o
        .jeton_sketchfab
        .as_deref()
        .filter(|j| !j.is_empty())
        .ok_or_else(|| Echec::Definitif("SKETCHFAB_TOKEN absent : modèle Sketchfab non téléchargé".to_owned()))?;
    let licence = licence_sketchfab(agent, &o.api_sketchfab, uid)?;
    if licence != a.licence {
        return Err(Echec::Definitif(format!(
            "licence actuelle sur Sketchfab : {licence}, manifeste : {} ; à revoir en PR",
            a.licence
        )));
    }
    #[derive(Deserialize)]
    struct Lien {
        url: String,
        size: Option<u64>,
    }
    #[derive(Deserialize)]
    struct Liens {
        glb: Option<Lien>,
    }
    let liens: Liens = json(
        agent
            .get(format!("{}/v3/models/{uid}/download", o.api_sketchfab))
            .header("Authorization", format!("Token {jeton}")),
    )?;
    let glb = liens.glb.ok_or_else(|| Echec::Definitif("Sketchfab ne propose pas de GLB".to_owned()))?;
    if let Some(n) = glb.size
        && n != f.octets
    {
        return Err(Echec::Definitif(format!(
            "Sketchfab propose un GLB de {n} octets, manifeste : {} ; le modèle a changé",
            f.octets
        )));
    }
    Ok(glb.url)
}

/// Licence SPDX d'un modèle, lue sur l'API publique.
fn licence_sketchfab(agent: &Agent, api: &str, uid: &str) -> Result<String, Echec> {
    #[derive(Deserialize)]
    struct Licence {
        slug: String,
    }
    #[derive(Deserialize)]
    struct Modele {
        license: Option<Licence>,
    }
    let m: Modele = json(agent.get(format!("{api}/v3/models/{uid}")))?;
    let slug = m.license.map(|l| l.slug).unwrap_or_default();
    match slug.as_str() {
        "by" => Ok("CC-BY-4.0".to_owned()),
        "cc0" => Ok("CC0-1.0".to_owned()),
        s => Ok(format!("licence Sketchfab « {s} »")),
    }
}

fn json<T: for<'de> Deserialize<'de>>(requete: ureq::RequestBuilder<ureq::typestate::WithoutBody>) -> Result<T, Echec> {
    let mut r = requete.call().map_err(echec_http)?;
    let texte =
        r.body_mut().with_config().limit(1 << 20).read_to_string().map_err(|e| Echec::Passager(e.to_string()))?;
    serde_json::from_str(&texte).map_err(|e| Echec::Definitif(format!("réponse de Sketchfab inattendue : {e}")))
}

fn recevoir(agent: &Agent, url: &str, octets: u64, tmp: &Path) -> Result<(), Echec> {
    let mut r = agent.get(url).call().map_err(echec_http)?;
    let mut sortie = File::create(tmp).map_err(|e| Echec::Definitif(format!("{}: {e}", tmp.display())))?;
    // Jamais plus que la taille du manifeste (plus un octet, pour détecter un
    // fichier trop long) : un serveur ne remplit pas le disque.
    let mut corps = r.body_mut().as_reader().take(octets + 1);
    let mut tampon = vec![0u8; 1 << 16];
    loop {
        let n = match corps.read(&mut tampon) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(Echec::Passager(format!("réception : {e}"))),
        };
        sortie.write_all(&tampon[..n]).map_err(|e| Echec::Passager(format!("écriture : {e}")))?;
    }
    sortie.sync_all().map_err(|e| Echec::Passager(format!("écriture : {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verification::tests::{dossier, glb};
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// Serveur HTTP minimal : chemin → (statut, corps) ; garde les en-têtes
    /// `Authorization` reçus.
    fn serveur(routes: HashMap<String, (u16, Vec<u8>)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let jetons = Arc::new(Mutex::new(Vec::new()));
        let j = jetons.clone();
        std::thread::spawn(move || {
            for c in l.incoming() {
                let Ok(mut c) = c else { return };
                let mut lecteur = BufReader::new(c.try_clone().unwrap());
                let mut ligne = String::new();
                lecteur.read_line(&mut ligne).unwrap();
                let chemin = ligne.split_whitespace().nth(1).unwrap_or_default().to_owned();
                loop {
                    let mut h = String::new();
                    lecteur.read_line(&mut h).unwrap();
                    if h.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = h.strip_prefix("authorization: ").or_else(|| h.strip_prefix("Authorization: ")) {
                        j.lock().unwrap().push(v.trim().to_owned());
                    }
                }
                let (statut, corps) = routes.get(&chemin).cloned().unwrap_or((404, b"absent".to_vec()));
                let _ =
                    write!(c, "HTTP/1.1 {statut} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", corps.len());
                let _ = c.write_all(&corps);
            }
        });
        (base, jetons)
    }

    fn asset(licence: &str, fichier: Fichier) -> Asset {
        Asset {
            id: "essai".into(),
            titre: "Essai".into(),
            auteur: "x".into(),
            licence: licence.into(),
            page: "https://opengameart.org/x".into(),
            modifications: "aucune".into(),
            remarque: None,
            fichier: vec![fichier],
        }
    }

    fn options(racine: PathBuf, base: &str, jeton: Option<&str>) -> Options {
        Options {
            essais: 2,
            attente: Duration::from_millis(10),
            api_sketchfab: base.to_owned(),
            https_seul: false,
            ..Options::nouvelles(racine, jeton.map(str::to_owned))
        }
    }

    fn fichier(contenu: &[u8], url: Option<String>, sketchfab: Option<&str>, nom: &str) -> Fichier {
        let d = dossier(&format!("source-{nom}"));
        let (mut f, _) = crate::verification::tests::fichier_pour(nom, contenu, &d);
        f.url = url;
        f.sketchfab = sketchfab.map(str::to_owned);
        f
    }

    #[test]
    fn telechargement_direct() {
        let (base, _) = serveur(HashMap::from([("/a.wav".to_owned(), (200, b"contenu".to_vec()))]));
        let racine = dossier("direct");
        let mut a = asset("CC0-1.0", fichier(b"contenu", Some(format!("{base}/a.wav")), None, "a.wav"));
        let o = options(racine.clone(), &base, None);
        let b = telecharger(std::slice::from_ref(&a), &o);
        assert_eq!((b.recus, b.presents, b.echecs.len()), (1, 0, 0), "{:?}", b.echecs);
        assert_eq!(fs::read(racine.join("essai/a.wav")).unwrap(), b"contenu");
        // Déjà là et valide.
        let b = telecharger(std::slice::from_ref(&a), &o);
        assert_eq!((b.recus, b.presents), (0, 1));
        // Contenu publié différent du manifeste : refusé, rien de visible.
        a.fichier[0].sha256 = "0".repeat(64);
        let b = telecharger(&[a], &o);
        assert_eq!(b.echecs.len(), 1);
        assert!(b.echecs[0].1.contains("SHA-256"), "{:?}", b.echecs);
        assert!(!racine.join("essai/a.wav").exists());
        assert!(!racine.join("essai/a.wav.partiel").exists());
    }

    #[test]
    fn sketchfab() {
        let uid = "84a0918d2f534c2eb003ab3cb3029c03";
        let contenu = glb(r#"{"license":"CC-BY-4.0 (http://creativecommons.org/licenses/by/4.0/)"}"#);
        // Stockage (lien temporaire), puis API qui donne ce lien.
        let (stockage, _) = serveur(HashMap::from([("/s3/m.glb".to_owned(), (200, contenu.clone()))]));
        let lien = format!(r#"{{"glb":{{"url":"{stockage}/s3/m.glb","size":{}}}}}"#, contenu.len());
        let (base, jetons) = serveur(HashMap::from([
            (format!("/v3/models/{uid}"), (200, br#"{"license":{"slug":"by"}}"#.to_vec())),
            (format!("/v3/models/{uid}/download"), (200, lien.into_bytes())),
        ]));
        let racine = dossier("sketchfab");
        let a = asset("CC-BY-4.0", fichier(&contenu, None, Some(uid), "m.glb"));
        // Sans jeton : échec immédiat, sans requête.
        let b = telecharger(std::slice::from_ref(&a), &options(racine.clone(), &base, None));
        assert!(b.echecs[0].1.contains("SKETCHFAB_TOKEN"), "{:?}", b.echecs);
        let b = telecharger(std::slice::from_ref(&a), &options(racine.clone(), &base, Some("secret")));
        assert_eq!((b.recus, b.echecs.len()), (1, 0), "{:?}", b.echecs);
        assert_eq!(jetons.lock().unwrap().as_slice(), ["Token secret"]);
        // Licence changée sur Sketchfab : refus, même avec la bonne empreinte.
        let b = telecharger(&[asset("CC0-1.0", a.fichier[0].clone())], &options(dossier("sf2"), &base, Some("s")));
        assert!(b.echecs[0].1.contains("licence actuelle"), "{:?}", b.echecs);
    }
}
