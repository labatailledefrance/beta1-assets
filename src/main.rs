//! `assets` : vérifie le manifeste, télécharge et contrôle les assets tiers,
//! génère ATTRIBUTIONS.md.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use beta1_assets::{attributions, conversion, manifeste, publication, telechargement, verification};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "Assets tiers de La Bataille de France (beta 1)")]
struct Cli {
    #[command(subcommand)]
    commande: Commande,
}

#[derive(Subcommand)]
enum Commande {
    /// Valide le manifeste ; avec --racine, vérifie aussi les fichiers présents.
    Verifier {
        #[arg(long)]
        racine: Option<PathBuf>,
    },
    /// Télécharge les fichiers absents sous --racine et les vérifie.
    /// Modèles Sketchfab : jeton personnel dans SKETCHFAB_TOKEN.
    Telecharger {
        #[arg(long)]
        racine: PathBuf,
        /// Seulement ces assets (identifiants du manifeste).
        #[arg(long = "seulement")]
        seulement: Vec<String>,
        /// Sans les modèles Sketchfab (pas de jeton, comme en CI).
        #[arg(long)]
        sans_sketchfab: bool,
        /// Seulement ce qu'il faut à `convertir --sans-blender` (pas de
        /// modèles : Poly Pizza refuse les IP de datacenter, dont la CI).
        #[arg(long)]
        sans_modeles: bool,
    },
    /// Convertit les fichiers téléchargés en assets du jeu (modèles, textures, sons, police).
    Convertir {
        #[arg(long)]
        racine: PathBuf,
        #[arg(long)]
        sortie: PathBuf,
        /// Exécutable de Blender (5.3.0 : sorties identiques à P-26).
        #[arg(long, default_value = "blender")]
        blender: PathBuf,
        /// Dossier des scripts Blender.
        #[arg(long, default_value = "blender")]
        scripts: PathBuf,
        /// Sans les modèles (pas de Blender).
        #[arg(long)]
        sans_blender: bool,
    },
    /// Job du chart `game` : publication complète, adressée par ses entrées.
    /// Sort aussitôt si `<sortie>/<publication>/COMPLET` existe ; sinon
    /// télécharge ce qui manque sous --sources, convertit dans
    /// `<publication>.partiel`, écrit MANIFESTE et COMPLET, puis renomme.
    Publier {
        #[arg(long)]
        sources: PathBuf,
        #[arg(long)]
        sortie: PathBuf,
        /// Identifiant (`[a-z0-9-]`), donné par le chart.
        #[arg(long)]
        publication: String,
        #[arg(long, default_value = "/opt/blender/blender")]
        blender: PathBuf,
        #[arg(long, default_value = "/opt/assets/blender")]
        scripts: PathBuf,
    },
    /// Attend qu'un fichier existe (initContainer : marqueur COMPLET d'une publication).
    Attendre {
        #[arg(long)]
        fichier: PathBuf,
        /// Secondes entre deux essais.
        #[arg(long, default_value_t = 10)]
        intervalle: u64,
    },
    /// Écrit ATTRIBUTIONS.md (ou l'affiche).
    Attributions {
        /// Écrit le fichier au lieu de l'afficher.
        #[arg(long, num_args = 0..=1, default_missing_value = "ATTRIBUTIONS.md")]
        ecrire: Option<PathBuf>,
    },
    /// Taille et SHA-256 d'un fichier, pour l'inscrire au manifeste.
    Empreinte { fichier: PathBuf },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Commande::Attendre { fichier, intervalle } = &cli.commande {
        publication::attendre(fichier, Duration::from_secs((*intervalle).max(1)));
        return ExitCode::SUCCESS;
    }
    if let Commande::Empreinte { fichier } = &cli.commande {
        return match verification::empreinte(fichier) {
            Ok((sha256, octets)) => {
                println!("octets = {octets}\nsha256 = \"{sha256}\"");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{}: {e}", fichier.display());
                ExitCode::FAILURE
            }
        };
    }
    let assets = match manifeste::charger() {
        Ok(a) => a,
        Err(erreurs) => {
            for e in erreurs {
                eprintln!("manifeste : {e}");
            }
            return ExitCode::FAILURE;
        }
    };
    match cli.commande {
        Commande::Verifier { racine } => {
            let n: usize = assets.iter().map(|a| a.fichier.len()).sum();
            println!("manifeste valide : {} assets, {n} fichiers", assets.len());
            let Some(racine) = racine else { return ExitCode::SUCCESS };
            let mut echecs = 0;
            for a in &assets {
                for f in &a.fichier {
                    let c = f.chemin(&racine, a);
                    let r =
                        if c.exists() { verification::verifier(f, &a.licence, &c) } else { Err("absent".to_owned()) };
                    let r = r.and_then(|()| {
                        let manquants: Vec<&str> =
                            f.extraits().map(|(_, nom)| nom).filter(|nom| !c.with_file_name(nom).exists()).collect();
                        if manquants.is_empty() { Ok(()) } else { Err(format!("non extraits : {manquants:?}")) }
                    });
                    if let Err(e) = r {
                        eprintln!("{} : {e}", c.display());
                        echecs += 1;
                    }
                }
            }
            println!("{} fichiers vérifiés, {echecs} en échec", n - echecs);
            if echecs == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Commande::Telecharger { racine, seulement, sans_sketchfab, sans_modeles } => {
            let inconnus: Vec<&String> = seulement.iter().filter(|s| !assets.iter().any(|a| &a.id == *s)).collect();
            if !inconnus.is_empty() {
                eprintln!("assets inconnus : {inconnus:?}");
                return ExitCode::FAILURE;
            }
            let sketchfab = |a: &manifeste::Asset| a.fichier.iter().any(|f| f.sketchfab.is_some());
            let choisis: Vec<manifeste::Asset> = assets
                .into_iter()
                .filter(|a| seulement.is_empty() || seulement.contains(&a.id))
                .filter(|a| !(sans_sketchfab && sketchfab(a)))
                .filter(|a| !sans_modeles || conversion::entrees_sans_blender().iter().any(|(id, _)| *id == a.id))
                .collect();
            let jeton = std::env::var("SKETCHFAB_TOKEN").ok();
            let b = telechargement::telecharger(&choisis, &telechargement::Options::nouvelles(racine, jeton));
            println!(
                "{} reçus, {} déjà présents, {} extraits, {} en échec",
                b.recus,
                b.presents,
                b.extraits,
                b.echecs.len()
            );
            if b.echecs.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Commande::Convertir { racine, sortie, blender, scripts, sans_blender } => {
            let b = conversion::Blender { executable: blender, scripts };
            match conversion::convertir(&assets, &racine, &sortie, (!sans_blender).then_some(&b)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("conversion : {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Commande::Publier { sources, sortie, publication: id, blender, scripts } => {
            let b = conversion::Blender { executable: blender, scripts };
            let jeton = std::env::var("SKETCHFAB_TOKEN").ok();
            let r = publication::publier(&sortie, &id, |travail| {
                let bilan =
                    telechargement::telecharger(&assets, &telechargement::Options::nouvelles(sources.clone(), jeton));
                if !bilan.echecs.is_empty() {
                    let liste: Vec<String> =
                        bilan.echecs.iter().map(|(c, e)| format!("{} ({e})", c.display())).collect();
                    return Err(format!(
                        "{} fichiers manquants : {} ; Poly Pizza refuse les IP de datacenter : copier ces fichiers \
                         sous {} depuis une machine personnelle",
                        liste.len(),
                        liste.join(", "),
                        sources.display()
                    ));
                }
                conversion::convertir(&assets, &sources, travail, Some(&b))
            });
            match r {
                Ok(publication::Etat::DejaComplete) => {
                    println!("publication {id} déjà complète");
                    ExitCode::SUCCESS
                }
                Ok(publication::Etat::Produite) => {
                    println!("publication {id} complète");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("publication {id} : {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Commande::Attributions { ecrire } => {
            let texte = attributions::generer(&assets);
            match ecrire {
                None => {
                    print!("{texte}");
                    ExitCode::SUCCESS
                }
                Some(chemin) => match std::fs::write(&chemin, texte) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(e) => {
                        eprintln!("{}: {e}", chemin.display());
                        ExitCode::FAILURE
                    }
                },
            }
        }
        Commande::Empreinte { .. } | Commande::Attendre { .. } => ExitCode::SUCCESS,
    }
}
