//! `ATTRIBUTIONS.md`, généré depuis le manifeste.
//!
//! Le test `attributions_a_jour` échoue si le fichier du dépôt n'est pas celui
//! qu'on génère : impossible d'ajouter un asset sans son attribution.

use crate::manifeste::Asset;

pub const FICHIER: &str = include_str!("../ATTRIBUTIONS.md");

fn lien_licence(spdx: &str) -> &'static str {
    match spdx {
        "CC0-1.0" => "https://creativecommons.org/publicdomain/zero/1.0/",
        "CC-BY-4.0" => "https://creativecommons.org/licenses/by/4.0/",
        "OFL-1.1" => "https://openfontlicense.org/",
        _ => "",
    }
}

/// Texte d'une cellule de tableau Markdown.
fn cellule(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '|' | '[' | ']' | '\\' | '*' | '_' | '`' | '<' | '>') {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

pub fn generer(assets: &[Asset]) -> String {
    let mut o = String::from(
        "# Attributions\n\n\
         Œuvres tierces utilisées par La Bataille de France. Aucune n'est dans ce dépôt : \
         `manifeste.toml` dit où les télécharger, et chacune garde sa licence. Les auteurs \
         sont aussi cités dans l'écran de crédits du jeu.\n\n\
         Fichier généré depuis `manifeste.toml` (`cargo run -- attributions --ecrire`) : \
         ne pas le modifier à la main.\n\n\
         | Œuvre | Auteur | Licence | Modifications |\n|---|---|---|---|\n",
    );
    for a in assets {
        let mut modifs = cellule(&a.modifications);
        if let Some(r) = &a.remarque {
            modifs.push_str(&format!(" (remarque : {})", cellule(r)));
        }
        o.push_str(&format!(
            "| [{}]({}) | {} | [{}]({}) | {} |\n",
            cellule(&a.titre),
            a.page,
            cellule(&a.auteur),
            a.licence,
            lien_licence(&a.licence),
            modifs
        ));
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifeste;

    #[test]
    fn attributions_a_jour() {
        let assets = manifeste::charger().unwrap_or_else(|e| panic!("{}", e.join("\n")));
        assert!(generer(&assets) == FICHIER, "ATTRIBUTIONS.md n'est pas à jour : cargo run -- attributions --ecrire");
    }

    #[test]
    fn echappement() {
        assert_eq!(cellule("a | b [c]"), "a \\| b \\[c\\]");
    }

    #[test]
    fn toutes_les_licences_ont_un_lien() {
        for l in manifeste::LICENCES {
            assert!(!lien_licence(l).is_empty(), "{l}");
        }
    }
}
