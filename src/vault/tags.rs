/// Extrait les tags `#...` d'un document, dans l'ordre d'apparition.
///
/// Règles :
/// - Un tag débute par `#` précédé du début de ligne, d'un whitespace ou
///   d'une ponctuation (jamais d'un alphanumérique ou d'un `_`).
/// - Le caractère qui suit le `#` doit être un caractère de tag :
///   alphanumérique Unicode, `_`, `-` ou `/`. Sinon le `#` est ignoré.
/// - Le tag s'arrête au premier caractère qui n'est pas un caractère de tag.
/// - L'imbrication `#a/b` produit un seul tag `a/b`.
/// - Les doublons sont conservés (le dédoublonnage se fait dans scan.rs).
pub fn parse_tags(source: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut chars = source.chars().peekable();
    let mut prev: Option<char> = None;

    while let Some(c) = chars.next() {
        if c == '#' {
            // Un `#` précédé d'un alphanumérique ou d'un `_` est un simple caractère.
            let peut_ouvrir = match prev {
                Some(p) => !(p.is_alphanumeric() || p == '_'),
                None => true,
            };
            let suivante_autorisee = match chars.peek() {
                Some(&n) => est_caractere_de_tag(n),
                None => false,
            };

            if peut_ouvrir && suivante_autorisee {
                let mut tag = String::new();
                let mut dernier = '\0';
                while let Some(&n) = chars.peek() {
                    if est_caractere_de_tag(n) {
                        dernier = n;
                        tag.push(n);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tags.push(tag);
                prev = Some(dernier);
                continue;
            }
        }
        prev = Some(c);
    }

    tags
}

/// Retourne `true` si `c` peut faire partie d'un tag.
fn est_caractere_de_tag(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || c == '/'
}

#[cfg(test)]
mod tests {
    use super::parse_tags;

    /// Construit le vecteur de tags attendu depuis des `&str`.
    fn tags(attendus: &[&str]) -> Vec<String> {
        attendus.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_tag_unique_et_ordre_d_apparition() {
        assert_eq!(parse_tags("#tag"), tags(&["tag"]));
        assert_eq!(
            parse_tags("#alpha #beta #gamma"),
            tags(&["alpha", "beta", "gamma"])
        );
        assert_eq!(
            parse_tags("#premier\n#second"),
            tags(&["premier", "second"])
        );
    }

    #[test]
    fn test_imbrication_avec_slash() {
        assert_eq!(parse_tags("#a/b"), tags(&["a/b"]));
        assert_eq!(
            parse_tags("#dossier/sous-dossier/tag"),
            tags(&["dossier/sous-dossier/tag"])
        );
    }

    #[test]
    fn test_hash_precede_alphanumerique_ou_underscore() {
        assert!(parse_tags("C#").is_empty());
        assert!(parse_tags("foo#bar").is_empty());
        assert!(parse_tags("identifiant_#tag").is_empty());
        assert!(parse_tags("x#").is_empty());
    }

    #[test]
    fn test_hash_precede_debut_ligne_whitespace_ou_ponctuation() {
        assert_eq!(parse_tags("#tag"), tags(&["tag"]));
        assert_eq!(parse_tags("(#tag"), tags(&["tag"]));
        assert_eq!(parse_tags(". #tag"), tags(&["tag"]));
        assert_eq!(parse_tags("\n#tag"), tags(&["tag"]));
        assert_eq!(parse_tags("- #tag"), tags(&["tag"]));
    }

    #[test]
    fn test_caracteres_autorises_du_tag() {
        assert_eq!(parse_tags("#foo_bar-baz/qux"), tags(&["foo_bar-baz/qux"]));
        assert_eq!(parse_tags("#café"), tags(&["café"]));
        assert_eq!(parse_tags("#tag,"), tags(&["tag"]));
        assert_eq!(parse_tags("#tag. suite"), tags(&["tag"]));
    }

    #[test]
    fn test_hash_suivi_d_un_caractere_non_autorise() {
        assert!(parse_tags("#").is_empty());
        assert!(parse_tags("# ").is_empty());
        assert!(parse_tags("#,").is_empty());
        assert!(parse_tags("#.").is_empty());
        assert!(parse_tags("#\n").is_empty());
        assert!(parse_tags("#\t").is_empty());
    }

    #[test]
    fn test_doublons_conserves() {
        assert_eq!(parse_tags("#tag #tag"), tags(&["tag", "tag"]));
        assert_eq!(parse_tags("#a #b #a"), tags(&["a", "b", "a"]));
    }
}
