use serde_yaml::Value;

/// Erreurs de parsing du frontmatter.
#[derive(Debug, thiserror::Error)]
pub enum FrontmatterError {
    /// Le YAML du bloc est invalide.
    #[error("frontmatter invalide (YAML) : {0}")]
    InvalidYaml(#[from] serde_yaml::Error),
    /// Un bloc `---` ouvrant ne trouve jamais sa fermeture.
    #[error("frontmatter non terminé (bloc `---` jamais fermé)")]
    Unterminated,
}

/// Parse le bloc frontmatter YAML `--- ... ---` en tête de document.
///
/// - `Ok(None)` : pas de bloc frontmatter (le document ne commence pas par `---`).
/// - `Ok(Some(value))` : bloc valide, YAML parsé.
/// - `Err(...)` : bloc malformé ou non terminé.
pub fn parse_frontmatter(source: &str) -> Result<Option<Value>, FrontmatterError> {
    // Tolère un BOM UTF-8 en tête de document.
    let source = source.strip_prefix('\u{FEFF}').unwrap_or(source);

    // Découpe : première ligne → délimiteur ouvrant. Elle doit être exactement
    // `---` (après trim), sinon pas de bloc frontmatter.
    let mut lines = source.lines();
    let Some(opening) = lines.next() else {
        return Ok(None);
    };
    if opening.trim() != "---" {
        return Ok(None);
    }

    // Cherche la ligne de fermeture suivante : seule une ligne dont le trim est
    // exactement `---` ferme le bloc.
    let mut content: Vec<&str> = Vec::new();
    for line in lines {
        if line.trim() == "---" {
            let content = content.join("\n");
            return Ok(Some(serde_yaml::from_str(&content)?));
        }
        content.push(line);
    }

    // Aucune ligne de fermeture trouvée.
    Err(FrontmatterError::Unterminated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pas_de_bloc_si_premiere_ligne_pas_ouvrante() {
        assert_eq!(parse_frontmatter("title: x\ncorps\n").unwrap(), None);
    }

    #[test]
    fn bom_utf8_tolere_en_tete() {
        // BOM suivi d'un bloc valide : reconnu.
        let value = parse_frontmatter("\u{FEFF}---\nk: v\n---\ncorps\n")
            .unwrap()
            .unwrap();
        assert_eq!(value["k"], "v");
        // BOM sans bloc : toujours Ok(None).
        assert_eq!(parse_frontmatter("\u{FEFF}bonjour\n").unwrap(), None);
    }

    #[test]
    fn frontmatter_valide_retourne_mapping() {
        let value = parse_frontmatter("---\nclé: valeur\n---\ncorps")
            .unwrap()
            .unwrap();
        assert_eq!(value["clé"], "valeur");
    }

    #[test]
    fn yaml_invalide_dans_bloc() {
        assert!(matches!(
            parse_frontmatter("---\na: [b\n---"),
            Err(FrontmatterError::InvalidYaml(_))
        ));
    }

    #[test]
    fn bloc_non_termine() {
        assert!(matches!(
            parse_frontmatter("---\na: b"),
            Err(FrontmatterError::Unterminated)
        ));
    }

    #[test]
    fn ligne_dash_au_milieu_ne_ferme_pas() {
        let value = parse_frontmatter("---\na: ---\nb: c\n---\ncorps\n")
            .unwrap()
            .unwrap();
        assert_eq!(value["a"], "---");
        assert_eq!(value["b"], "c");
    }

    #[test]
    fn frontmatter_vide_retourne_null() {
        assert_eq!(parse_frontmatter("---\n---").unwrap(), Some(Value::Null));
    }
}
