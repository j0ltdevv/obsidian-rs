use std::path::PathBuf;

/// Métadonnées d'une note, déduites du contenu Markdown.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteMetadata {
    /// Frontmatter YAML parsé, si un bloc `--- ... ---` ouvre le document.
    pub frontmatter: Option<serde_yaml::Value>,
    /// Message d'erreur si le frontmatter est invalide ou non terminé.
    pub frontmatter_error: Option<String>,
    /// Wikilinks `[[...]]` dans l'ordre d'apparition.
    pub wikilinks: Vec<Wikilink>,
    /// Tags plats (imbrication `#a/b` conservée).
    pub tags: Vec<String>,
}

/// Un wikilink `[[cible]]` ou `[[cible|alias]]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wikilink {
    /// Cible du lien, avant tout `|` et tout préfixe `#`.
    pub target: String,
    /// Alias après `|`, si présent (trimé).
    pub alias: Option<String>,
}

/// Une note du vault.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    /// Chemin du fichier, relatif au root du vault.
    pub path: PathBuf,
    /// Nom de fichier sans extension.
    pub name: String,
    /// Métadonnées parsées.
    pub metadata: NoteMetadata,
}
