//! Export statique du vault en site HTML (équivalent Publish).
//!
//! Génère un site statique depuis la racine du vault : une page HTML par
//! note (`<nom>.html`), un `index.html` (sommaire trié par titre) et une
//! feuille de style `style.css` unique. Les pièces jointes référencées par
//! `![[…]]` sont copiées dans `assets/` et référencées en relatif.
//!
//! Sécurité :
//! - le HTML est produit à partir des segments de [`super::live_preview`]
//!   (aucun nouveau moteur Markdown), et tout texte utilisateur est échappé
//!   (`&`, `<`, `>`, `"`) — une note contenant `<script>` ne produit jamais
//!   de HTML exécutable ;
//! - chaque écriture est atomique (fichier temporaire + `rename`) ;
//! - les noms de notes et les chemins de pièces jointes sont confinés :
//!   un nom ou un chemin contenant `..`, un composant absolu ou un préfixe
//!   est refusé, afin qu'aucune écriture ne sorte du répertoire de
//!   destination.

use std::collections::HashSet;
use std::fmt::Write as FmtWrite;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde_yaml::Value;

use super::editor::frontmatter_region;
use super::live_preview::{render_markdown, Segment, SegmentStyle};
use super::scan::scan_vault;

/// Erreurs de l'export statique.
#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    /// La racine du vault n'existe pas ou n'est pas un dossier.
    #[error("la racine du vault n'existe pas ou n'est pas un dossier : {0}")]
    VaultRootMissing(String),
    /// Nom de note refusé par le confinement (sortirait de la destination).
    #[error("nom de note refusé (hors du répertoire de destination) : {0}")]
    InvalidNoteName(String),
    /// Chemin de pièce jointe refusé par le confinement.
    #[error("chemin de pièce jointe refusé : {0}")]
    InvalidAttachmentPath(String),
    /// Pièce jointe référencée mais introuvable.
    #[error("pièce jointe introuvable : {0}")]
    MissingAttachment(String),
    /// Erreur lors du scan du vault.
    #[error("scan du vault : {0}")]
    Scan(String),
    /// Lecture impossible du contenu d'une note.
    #[error("lecture de la note « {0} » : {1}")]
    ReadNote(String, String),
    /// Erreur d'entrée-sortie.
    #[error("erreur d'entrée-sortie : {0}")]
    Io(#[from] std::io::Error),
}

/// Bilan d'un export statique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublishReport {
    /// Nombre de pages HTML produites (une par note).
    pub pages: usize,
    /// Nombre de pièces jointes copiées dans `assets/`.
    pub assets: usize,
}

/// Exporte le vault situé à `vault_root` en site statique dans `dest`.
///
/// Écrit une page HTML par note (`<nom>.html`), un `index.html` (sommaire
/// des notes triées par titre) et une feuille de style `style.css`. Les
/// pièces jointes (images, PDF) référencées par `![[…]]` sont copiées dans
/// `dest/assets/` et référencées en relatif.
///
/// # Sécurité
///
/// - Tous les noms de notes sont validés **avant** la moindre écriture :
///   un nom qui permettrait d'écrire hors de `dest` (`..`, composant absolu,
///   préfixe) fait échouer l'export sans rien écrire.
/// - Chaque fichier est écrit de façon atomique (temporaire + `rename`).
///
/// # Erreurs
///
/// Retourne [`PublishError::VaultRootMissing`] si `vault_root` n'existe pas,
/// [`PublishError::InvalidNoteName`] si un nom de note n'est pas confiné, et
/// une erreur d'entrée-sortie si une écriture échoue.
pub fn publish_site(vault_root: &Path, dest: &Path) -> Result<PublishReport, PublishError> {
    if !vault_root.is_dir() {
        return Err(PublishError::VaultRootMissing(
            vault_root.display().to_string(),
        ));
    }

    let notes = scan_vault(vault_root).map_err(|e| PublishError::Scan(e.to_string()))?;

    // Pré-validation : on refuse tout nom qui sortirait du répertoire de
    // destination, avant d'écrire quoi que ce soit.
    let mut plan: Vec<(String, PathBuf)> = Vec::with_capacity(notes.len());
    for note in &notes {
        let filename = html_filename(&note.name)?;
        plan.push((note.name.clone(), dest.join(filename)));
    }

    fs::create_dir_all(dest)?;
    let assets_dir = dest.join("assets");
    fs::create_dir_all(&assets_dir)?;

    // Cibles existantes (minuscules) pour résoudre les wikilinks.
    let targets: HashSet<String> = notes.iter().map(|n| n.name.to_lowercase()).collect();

    let mut pages: Vec<(String, String, PathBuf)> = Vec::with_capacity(notes.len());
    let mut assets_copied = 0usize;

    for (index, note) in notes.iter().enumerate() {
        let source_path = vault_root.join(&note.path);
        let content = fs::read_to_string(&source_path)
            .map_err(|e| PublishError::ReadNote(note.name.clone(), e.to_string()))?;

        let (frontmatter_title, tags) = frontmatter_title_tags(&note.metadata.frontmatter);
        let title = frontmatter_title.unwrap_or_else(|| note.name.clone());

        let body = render_body(
            &content,
            &targets,
            vault_root,
            &assets_dir,
            &mut assets_copied,
        )?;
        let html = page_html(&title, &tags, &body);
        write_atomic_str(&plan[index].1, &html)?;

        pages.push((title.to_lowercase(), title, plan[index].1.clone()));
    }

    pages.sort_by(|a, b| a.0.cmp(&b.0));
    write_atomic_str(&dest.join("index.html"), &index_html(&pages))?;
    write_atomic_str(&dest.join("style.css"), STYLE_CSS)?;

    Ok(PublishReport {
        pages: notes.len(),
        assets: assets_copied,
    })
}

/// Retourne le nom de fichier HTML sûr pour une note `name`.
///
/// Refuse tout nom vide, absolu ou contenant un composant `..` / un préfixe
/// (qui permettrait d'écrire hors du répertoire de destination). Le nom
/// retourné est toujours un simple nom de fichier, dépourvu de séparateur.
pub fn html_filename(name: &str) -> Result<String, PublishError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(PublishError::InvalidNoteName(name.to_string()));
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err(PublishError::InvalidNoteName(name.to_string()));
    }
    let mut normal_parts = 0usize;
    for component in path.components() {
        match component {
            Component::Normal(_) => normal_parts += 1,
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(PublishError::InvalidNoteName(name.to_string()));
            }
        }
    }
    if normal_parts == 0 {
        return Err(PublishError::InvalidNoteName(name.to_string()));
    }
    Ok(format!("{trimmed}.html"))
}

/// Rend le corps d'une note en HTML, à partir des segments de
/// [`super::live_preview::render_markdown`].
fn render_body(
    content: &str,
    targets: &HashSet<String>,
    vault_root: &Path,
    assets_dir: &Path,
    assets_copied: &mut usize,
) -> Result<String, PublishError> {
    let segments = render_markdown(content);
    let region = frontmatter_region(content);

    // Supprime le bloc frontmatter (rendu en `<meta>`, jamais dans le corps).
    let filtered: Vec<&Segment> = segments
        .iter()
        .filter(|s| match region {
            Some((start, end)) => s.line < start || s.line > end,
            None => true,
        })
        .collect();

    let mut out = String::new();
    let mut i = 0usize;
    while i < filtered.len() {
        let line = filtered[i].line;
        let mut j = i;
        while j < filtered.len() && filtered[j].line == line {
            j += 1;
        }
        render_line(
            &filtered[i..j],
            targets,
            vault_root,
            assets_dir,
            assets_copied,
            &mut out,
        )?;
        i = j;
    }
    Ok(out)
}

/// Rend un groupe de segments d'une même ligne en HTML.
fn render_line(
    group: &[&Segment],
    targets: &HashSet<String>,
    vault_root: &Path,
    assets_dir: &Path,
    assets_copied: &mut usize,
    out: &mut String,
) -> Result<(), PublishError> {
    // Bloc de code : un seul segment couvre toute la plage de lignes.
    if let Some(block) = group.iter().find(|s| s.style == SegmentStyle::CodeBlock) {
        out.push_str("<pre><code>");
        out.push_str(&html_escape(&block.text));
        out.push_str("</code></pre>\n");
        return Ok(());
    }

    // Titres.
    if let Some(tag) = group.first().and_then(|s| heading_tag(s.style)) {
        let _ = write!(out, "<{tag}>");
        push_inline(group, targets, vault_root, assets_dir, assets_copied, out)?;
        let _ = writeln!(out, "</{tag}>");
        return Ok(());
    }

    // Règle horizontale.
    if group
        .iter()
        .any(|s| s.style == SegmentStyle::HorizontalRule)
    {
        out.push_str("<hr>\n");
        return Ok(());
    }

    // Math display.
    if let Some(math) = group.iter().find(|s| s.style == SegmentStyle::MathDisplay) {
        out.push_str("<div class=\"math\">");
        out.push_str(&html_escape(&math.text));
        out.push_str("</div>\n");
        return Ok(());
    }

    // Citations et callouts.
    if group.iter().any(|s| {
        matches!(
            s.style,
            SegmentStyle::Blockquote | SegmentStyle::Callout | SegmentStyle::CalloutTitle
        )
    }) {
        out.push_str("<blockquote>");
        push_inline(group, targets, vault_root, assets_dir, assets_copied, out)?;
        out.push_str("</blockquote>\n");
        return Ok(());
    }

    // Tableau : une ligne de tableau devient une rangée.
    if group.iter().any(|s| s.style == SegmentStyle::Table) {
        let row: Vec<&str> = group.iter().map(|s| s.text.as_str()).collect();
        out.push_str("<table><tr><td>");
        out.push_str(&html_escape(&row.join(" ")));
        out.push_str("</td></tr></table>\n");
        return Ok(());
    }

    // Listes et tâches.
    if group.iter().any(|s| {
        matches!(
            s.style,
            SegmentStyle::ListBullet
                | SegmentStyle::ListNumbered
                | SegmentStyle::TaskCheckboxTodo
                | SegmentStyle::TaskCheckboxDone
        )
    }) {
        out.push_str("<li>");
        push_inline(group, targets, vault_root, assets_dir, assets_copied, out)?;
        out.push_str("</li>\n");
        return Ok(());
    }

    // Paragraphe par défaut.
    out.push_str("<p>");
    push_inline(group, targets, vault_root, assets_dir, assets_copied, out)?;
    out.push_str("</p>\n");
    Ok(())
}

/// Pousse le HTML inline d'un groupe de segments dans `out`.
fn push_inline(
    group: &[&Segment],
    targets: &HashSet<String>,
    vault_root: &Path,
    assets_dir: &Path,
    assets_copied: &mut usize,
    out: &mut String,
) -> Result<(), PublishError> {
    for seg in group {
        let esc = html_escape(&seg.text);
        match seg.style {
            SegmentStyle::Plain => out.push_str(&esc),
            SegmentStyle::Bold => {
                out.push_str("<strong>");
                out.push_str(&esc);
                out.push_str("</strong>");
            }
            SegmentStyle::Italic => {
                out.push_str("<em>");
                out.push_str(&esc);
                out.push_str("</em>");
            }
            SegmentStyle::BoldItalic => {
                out.push_str("<strong><em>");
                out.push_str(&esc);
                out.push_str("</em></strong>");
            }
            SegmentStyle::Strikethrough => {
                out.push_str("<del>");
                out.push_str(&esc);
                out.push_str("</del>");
            }
            SegmentStyle::InlineCode => {
                out.push_str("<code>");
                out.push_str(&esc);
                out.push_str("</code>");
            }
            SegmentStyle::Link => out.push_str(&esc),
            SegmentStyle::ExternalUrl => {
                out.push_str("<a href=\"");
                out.push_str(&esc);
                out.push_str("\">");
                out.push_str(&esc);
                out.push_str("</a>");
            }
            SegmentStyle::Wikilink | SegmentStyle::Embed => {
                let key = seg.text.to_lowercase();
                if targets.contains(&key) {
                    let href = html_escape(&format!("{}.html", seg.text));
                    out.push_str("<a href=\"");
                    out.push_str(&href);
                    out.push_str("\">");
                    out.push_str(&esc);
                    out.push_str("</a>");
                } else {
                    out.push_str(&esc);
                }
            }
            SegmentStyle::EmbedImage | SegmentStyle::EmbedPdf => {
                copy_attachment(seg, vault_root, assets_dir, assets_copied, out)?;
            }
            SegmentStyle::Tag => {
                out.push_str("<span class=\"tag\">");
                out.push_str(&esc);
                out.push_str("</span>");
            }
            SegmentStyle::TaskCheckboxTodo => out.push_str("<input type=\"checkbox\" disabled>"),
            SegmentStyle::TaskCheckboxDone => {
                out.push_str("<input type=\"checkbox\" disabled checked>");
            }
            SegmentStyle::ListBullet | SegmentStyle::ListNumbered => {}
            SegmentStyle::Footnote => {
                out.push_str("<sup>");
                out.push_str(&esc);
                out.push_str("</sup>");
            }
            SegmentStyle::MathInline => {
                out.push_str("<span class=\"math-inline\">");
                out.push_str(&esc);
                out.push_str("</span>");
            }
            SegmentStyle::H1
            | SegmentStyle::H2
            | SegmentStyle::H3
            | SegmentStyle::H4
            | SegmentStyle::H5
            | SegmentStyle::H6
            | SegmentStyle::CodeBlock
            | SegmentStyle::Table
            | SegmentStyle::HorizontalRule
            | SegmentStyle::Blockquote
            | SegmentStyle::Callout
            | SegmentStyle::CalloutTitle
            | SegmentStyle::MathDisplay => out.push_str(&esc),
        }
    }
    Ok(())
}

/// Copie la pièce jointe référencée par un segment d'embed dans `assets/` et
/// émet la référence relative. Si la pièce jointe est introuvable, émet du
/// texte simple (pas de lien mort).
fn copy_attachment(
    seg: &Segment,
    vault_root: &Path,
    assets_dir: &Path,
    assets_copied: &mut usize,
    out: &mut String,
) -> Result<(), PublishError> {
    let rel = confine_rel_path(&seg.text)?;
    let source = vault_root.join(&rel);
    let Some(base) = rel
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
    else {
        return Err(PublishError::InvalidAttachmentPath(seg.text.clone()));
    };
    let destination = assets_dir.join(&base);

    if !source.is_file() {
        out.push_str(&html_escape(&seg.text));
        return Ok(());
    }

    copy_atomic(&source, &destination)?;
    *assets_copied += 1;

    let esc = html_escape(&seg.text);
    if seg.style == SegmentStyle::EmbedImage {
        out.push_str("<img src=\"assets/");
        out.push_str(&base);
        out.push_str("\" alt=\"");
        out.push_str(&esc);
        out.push_str("\">");
    } else {
        out.push_str("<a href=\"assets/");
        out.push_str(&base);
        out.push_str("\">");
        out.push_str(&esc);
        out.push_str("</a>");
    }
    Ok(())
}

/// Normalise et confine un chemin relatif de pièce jointe.
fn confine_rel_path(input: &str) -> Result<PathBuf, PublishError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(PublishError::InvalidAttachmentPath(trimmed.to_string()));
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err(PublishError::InvalidAttachmentPath(trimmed.to_string()));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(PublishError::InvalidAttachmentPath(trimmed.to_string()));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(PublishError::InvalidAttachmentPath(trimmed.to_string()));
    }
    Ok(normalized)
}

/// Échappe `&`, `<`, `>`, `"` pour du texte ou un attribut HTML.
fn html_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Balise de titre correspondant à un style, ou `None`.
fn heading_tag(style: SegmentStyle) -> Option<&'static str> {
    match style {
        SegmentStyle::H1 => Some("h1"),
        SegmentStyle::H2 => Some("h2"),
        SegmentStyle::H3 => Some("h3"),
        SegmentStyle::H4 => Some("h4"),
        SegmentStyle::H5 => Some("h5"),
        SegmentStyle::H6 => Some("h6"),
        _ => None,
    }
}

/// Édif la page HTML complète d'une note.
fn page_html(title: &str, tags: &[String], body: &str) -> String {
    let mut s = String::new();
    s.push_str("<!DOCTYPE html>\n<html lang=\"fr\">\n<head>\n");
    s.push_str("<meta charset=\"utf-8\">\n");
    let _ = writeln!(s, "<title>{}</title>", html_escape(title));
    let _ = writeln!(
        s,
        "<meta name=\"title\" content=\"{}\">",
        html_escape(title)
    );
    if !tags.is_empty() {
        let joined = tags.join(", ");
        let _ = writeln!(
            s,
            "<meta name=\"keywords\" content=\"{}\">",
            html_escape(&joined)
        );
    }
    s.push_str("<link rel=\"stylesheet\" href=\"style.css\">\n");
    s.push_str("</head>\n<body>\n");
    let _ = writeln!(s, "<h1>{}</h1>", html_escape(title));
    s.push_str(body);
    s.push_str("\n</body>\n</html>\n");
    s
}

/// Édif la page `index.html` (sommaire trié par titre).
fn index_html(pages: &[(String, String, PathBuf)]) -> String {
    let mut s = String::new();
    s.push_str("<!DOCTYPE html>\n<html lang=\"fr\">\n<head>\n");
    s.push_str("<meta charset=\"utf-8\">\n");
    s.push_str("<title>Sommaire</title>\n");
    s.push_str("<link rel=\"stylesheet\" href=\"style.css\">\n");
    s.push_str("</head>\n<body>\n");
    s.push_str("<h1>Sommaire</h1>\n<ul>\n");
    for (_, title, path) in pages {
        let href = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let _ = writeln!(
            s,
            "<li><a href=\"{}\">{}</a></li>",
            html_escape(&href),
            html_escape(title)
        );
    }
    s.push_str("</ul>\n</body>\n</html>\n");
    s
}

/// Extrait le titre et les tags du frontmatter.
fn frontmatter_title_tags(frontmatter: &Option<Value>) -> (Option<String>, Vec<String>) {
    let Some(Value::Mapping(map)) = frontmatter else {
        return (None, Vec::new());
    };
    let title = map.get("title").and_then(value_to_string);
    let tags = match map.get("tags") {
        Some(Value::Sequence(seq)) => seq.iter().filter_map(value_to_string).collect(),
        Some(value) => value_to_string(value).into_iter().collect(),
        None => Vec::new(),
    };
    (title, tags)
}

/// Convertit une valeur YAML scalaire simple en chaîne.
fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Écrit `content` dans `target` de façon atomique : fichier temporaire dans
/// le même dossier, puis `rename`. En cas d'erreur, le temporaire est
/// nettoyé.
fn write_atomic_str(target: &Path, content: &str) -> Result<(), PublishError> {
    let dir = target.parent().ok_or_else(|| {
        PublishError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destination sans dossier parent",
        ))
    })?;
    let (temp, mut file) = open_unique_temp(dir, target)?;
    let result = (|| -> Result<(), PublishError> {
        file.write_all(content.as_bytes())?;
        file.flush()?;
        Ok(())
    })();
    if let Err(err) = result {
        let _ = fs::remove_file(&temp);
        return Err(err);
    }
    fs::rename(&temp, target)?;
    Ok(())
}

/// Copie `source` vers `destination` de façon atomique.
fn copy_atomic(source: &Path, destination: &Path) -> Result<(), PublishError> {
    let dir = destination.parent().ok_or_else(|| {
        PublishError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destination sans dossier parent",
        ))
    })?;
    let (temp, mut file) = open_unique_temp(dir, destination)?;
    let result = (|| -> Result<(), PublishError> {
        let mut reader = File::open(source)?;
        std::io::copy(&mut reader, &mut file)?;
        file.flush()?;
        Ok(())
    })();
    if let Err(err) = result {
        let _ = fs::remove_file(&temp);
        return Err(err);
    }
    fs::rename(&temp, destination)?;
    Ok(())
}

/// Ouvre un fichier temporaire unique dans `dir` (`.{nom}.tmp-{pid}[-n]`),
/// en `create_new` pour garantir l'unicité même en cas d'accès concurrent.
fn open_unique_temp(dir: &Path, target: &Path) -> Result<(PathBuf, File), PublishError> {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "sortie".to_string());
    let base = format!(".{name}.tmp-{}", std::process::id());
    for i in 0..100u32 {
        let temp = if i == 0 {
            dir.join(&base)
        } else {
            dir.join(format!("{base}-{i}"))
        };
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
        {
            Ok(file) => return Ok((temp, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(PublishError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "impossible de créer un fichier temporaire unique",
    )))
}

/// Feuille de style unique du site.
const STYLE_CSS: &str = r#"body {
  font-family: -apple-system, "Segoe UI", Roboto, sans-serif;
  line-height: 1.6;
  max-width: 720px;
  margin: 2rem auto;
  padding: 0 1rem;
  color: #222;
  background: #fff;
}
h1, h2, h3, h4, h5, h6 {
  line-height: 1.2;
}
pre {
  background: #f4f4f4;
  padding: 0.75rem;
  overflow-x: auto;
}
code {
  font-family: "SFMono-Regular", Consolas, monospace;
  font-size: 0.9em;
}
blockquote {
  margin: 0;
  padding: 0 1rem;
  border-left: 3px solid #ccc;
  color: #555;
}
table {
  border-collapse: collapse;
  margin: 0.75rem 0;
}
table td, table th {
  border: 1px solid #ccc;
  padding: 0.35rem 0.6rem;
}
.tag {
  background: #eef;
  border-radius: 3px;
  padding: 0 0.3em;
  color: #446;
}
"#;
