//! `assets` : vérifie le manifeste, télécharge et contrôle les assets tiers,
//! génère ATTRIBUTIONS.md.

use std::path::PathBuf;
use std::process::ExitCode;

use beta1_assets::{attributions, manifeste, telechargement, verification};
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
        Commande::Telecharger { racine, seulement } => {
            let inconnus: Vec<&String> = seulement.iter().filter(|s| !assets.iter().any(|a| &a.id == *s)).collect();
            if !inconnus.is_empty() {
                eprintln!("assets inconnus : {inconnus:?}");
                return ExitCode::FAILURE;
            }
            let choisis: Vec<manifeste::Asset> =
                assets.into_iter().filter(|a| seulement.is_empty() || seulement.contains(&a.id)).collect();
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
        Commande::Empreinte { .. } => ExitCode::SUCCESS,
    }
}
