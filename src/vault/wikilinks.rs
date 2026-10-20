use super::note::Wikilink;

/// Extrait tous les wikilinks `[[...]]` d'un document, dans l'ordre.
///
/// Gère `[[cible]]` et `[[cible|alias]]` ; un lien vide `[[]]` est conservé
/// avec une cible vide.
pub fn parse_wikilinks(source: &str) -> Vec<Wikilink> {
    let bytes = source.as_bytes();
    let mut links = Vec::new();
    let mut pos = 0;

    while pos + 1 < bytes.len() {
        // Cherche l'ouverture suivante `[[`.
        if bytes[pos] != b'[' || bytes[pos + 1] != b'[' {
            pos += 1;
            continue;
        }

        // À partir du contenu, cherche la fermeture `]]` avant tout `\n`.
        let mut end = pos + 2;
        while end + 1 < bytes.len() && bytes[end] != b'\n' {
            if bytes[end] == b']' && bytes[end + 1] == b']' {
                break;
            }
            end += 1;
        }

        // Aucun `]]` trouvé avant la fin de la ligne (ou de la source) :
        // le fragment ouvert est ignoré.
        if end + 1 >= bytes.len() || bytes[end] != b']' || bytes[end + 1] != b']' {
            pos += 2;
            continue;
        }

        // Contenu entre `[[` et `]]`, découpé au premier `|`.
        let raw = &source[pos + 2..end];
        let (target, alias) = match raw.split_once('|') {
            Some((target, alias)) => (target.trim(), Some(alias.trim())),
            None => (raw.trim(), None),
        };

        let alias = match alias {
            Some(alias) if !alias.is_empty() => Some(alias.to_string()),
            _ => None,
        };

        links.push(Wikilink {
            target: target.to_string(),
            alias,
        });
        pos = end + 2;
    }

    links
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::note::Wikilink;

    fn link(target: &str, alias: Option<&str>) -> Wikilink {
        Wikilink {
            target: target.to_string(),
            alias: alias.map(str::to_string),
        }
    }

    #[test]
    fn lien_simple() {
        assert_eq!(parse_wikilinks("[[Cible]]"), vec![link("Cible", None)]);
    }

    #[test]
    fn lien_avec_alias() {
        assert_eq!(
            parse_wikilinks("[[Cible|Alias]]"),
            vec![link("Cible", Some("Alias"))]
        );
        // Le trim s'applique à la cible et à l'alias.
        assert_eq!(
            parse_wikilinks("[[ Cible | Alias ]]"),
            vec![link("Cible", Some("Alias"))]
        );
    }

    #[test]
    fn lien_vide() {
        assert_eq!(parse_wikilinks("[[]]"), vec![link("", None)]);
    }

    #[test]
    fn ancre_conservee() {
        assert_eq!(parse_wikilinks("[[#Titre]]"), vec![link("#Titre", None)]);
    }

    #[test]
    fn plusieurs_liens_ordre() {
        // Sur une même ligne.
        assert_eq!(
            parse_wikilinks("texte [[A]] puis [[B]] fin"),
            vec![link("A", None), link("B", None)]
        );
        // Et à travers plusieurs lignes.
        assert_eq!(
            parse_wikilinks("[[A]]\n[[B]]"),
            vec![link("A", None), link("B", None)]
        );
    }

    #[test]
    fn fragment_non_ferme_ignore() {
        // Le fragment `[[B` n'est pas fermé : seul `[[A]]` est conservé.
        assert_eq!(parse_wikilinks("[[A]] [[B"), vec![link("A", None)]);
        assert_eq!(parse_wikilinks("[[A"), Vec::new());
    }

    #[test]
    fn deux_liens_adjacents() {
        assert_eq!(
            parse_wikilinks("[[A]] [[B]]"),
            vec![link("A", None), link("B", None)]
        );
    }

    #[test]
    fn alias_vide_ignore() {
        assert_eq!(parse_wikilinks("[[X|]]"), vec![link("X", None)]);
        assert_eq!(parse_wikilinks("[[X|   ]]"), vec![link("X", None)]);
    }

    #[test]
    fn lien_ne_traverse_pas_les_lignes() {
        // `\n` entre `[[` et `]]` casse le lien.
        assert_eq!(parse_wikilinks("[[A\nB]]"), Vec::new());
        // Les liens bien formés suivants sont conservés.
        assert_eq!(parse_wikilinks("[[A\nB]] [[C]]"), vec![link("C", None)]);
    }
}
