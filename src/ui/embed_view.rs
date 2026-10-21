//! Affichage des embeds d'image `![[fichier.png]]` dans l'aperçu.
//!
//! La logique de décision et de dimensionnement est **pure** et testée :
//! `decide_display`, `within_size_limit` et `display_size` ne font aucune
//! entrée/sortie. La lecture des octets et le confinement au vault sont
//! regroupés dans `resolve_display`.
//!
//! # Fluidité
//!
//! La vue est reconstruite à chaque frame ; lire le fichier image à chaque
//! rendu bloquerait l'UI. On lit donc chaque fichier **une seule fois** puis
//! on le met en cache (clef = chemin canonisé, mémoire partagée entre les
//! frames). Un fichier illisible ou hors vault est mémorisé comme
//! indisponible pour éviter de le relire en boucle.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use iced::widget::{image, row, text};
use iced::{Color, ContentFit, Element, Length};

use crate::app::Message;
use crate::vault::embeds::{
    image_dimensions, resolve_embed_target, EmbedType, ImageDimensions, ResolveOutcome,
};

/// Largeur maximale d'affichage d'une image d'embed (pt).
pub const MAX_IMAGE_WIDTH: f32 = 400.0;

/// Plafond de taille pour lire un fichier image (octets) : 32 Mio.
pub const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Logique pure (testable sans I/O)
// ---------------------------------------------------------------------------

/// `true` si le type d'embed est une image.
pub fn is_image(kind: EmbedType) -> bool {
    matches!(kind, EmbedType::Image)
}

/// `true` si une taille de fichier respecte le plafond donné.
pub fn within_size_limit(file_len: u64, limit: u64) -> bool {
    file_len <= limit
}

/// Taille d'affichage conservant le rapport d'aspect, bornée à `max_width` :
/// l'image est réduite si elle est plus large que `max_width` (jamais
/// agrandie au-delà de sa taille naturelle), et jamais déformée.
pub fn display_size(dimensions: ImageDimensions, max_width: f32) -> (f32, f32) {
    let width = dimensions.width as f32;
    let height = dimensions.height as f32;
    if width <= 0.0 || height <= 0.0 {
        return (0.0, 0.0);
    }
    if width <= max_width {
        (width, height)
    } else {
        (max_width, max_width * height / width)
    }
}

/// Décision d'affichage d'un embed à partir de données déjà obtenues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayKind {
    /// Afficher l'image réelle (dimensions pixels connues).
    Image(ImageDimensions),
    /// Image trop volumineuse : message discret.
    TooLarge,
    /// Retomber sur le placeholder sobre.
    Placeholder,
}

/// Décide si un embed est affichable comme image réelle. Fonction pure :
/// `kind` (type de l'embed), `file_len` (taille déjà lue) et `dimensions`
/// (résultat de `image_dimensions`, `Some` si lisible).
pub fn decide_display(
    kind: EmbedType,
    file_len: u64,
    dimensions: Option<ImageDimensions>,
) -> DisplayKind {
    if !is_image(kind) {
        return DisplayKind::Placeholder;
    }
    if !within_size_limit(file_len, MAX_IMAGE_BYTES) {
        return DisplayKind::TooLarge;
    }
    match dimensions {
        Some(dimensions) => DisplayKind::Image(dimensions),
        None => DisplayKind::Placeholder,
    }
}

// ---------------------------------------------------------------------------
// Lecture confinée au vault, avec cache
// ---------------------------------------------------------------------------

/// Image chargée : octets bruts + dimensions pixels.
#[derive(Clone)]
struct ImageAsset {
    bytes: Vec<u8>,
    dimensions: ImageDimensions,
}

/// Résultat d'une lecture mise en cache.
#[derive(Clone)]
enum CachedImage {
    /// Image chargée avec succès.
    Loaded(ImageAsset),
    /// Fichier au-delà du plafond de taille.
    TooLarge,
    /// Fichier absent, illisible, hors vault ou format non pris en charge.
    Unavailable,
}

/// Cache global des images lues (clef = chemin canonisé).
static IMAGE_CACHE: LazyLock<Mutex<HashMap<PathBuf, CachedImage>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Chemin canonisé du fichier si et seulement si il est **dans** le vault.
/// Rejette les liens symboliques qui sortent du vault et les chemins absents.
/// `candidate` est le chemin complet déjà résolu par `resolve_embed_target`.
fn canonical_inside_vault(root: &Path, candidate: &Path) -> Option<PathBuf> {
    let root_canon = fs::canonicalize(root).ok()?;
    let full_canon = fs::canonicalize(candidate).ok()?;
    if full_canon.starts_with(&root_canon) {
        Some(full_canon)
    } else {
        None
    }
}

/// Lit un fichier image (octets + dimensions) en respectant le plafond.
fn fetch_image(full: &Path) -> CachedImage {
    let meta = match fs::metadata(full) {
        Ok(meta) => meta,
        Err(_) => return CachedImage::Unavailable,
    };
    if !within_size_limit(meta.len(), MAX_IMAGE_BYTES) {
        return CachedImage::TooLarge;
    }
    let bytes = match fs::read(full) {
        Ok(bytes) => bytes,
        Err(_) => return CachedImage::Unavailable,
    };
    if !within_size_limit(bytes.len() as u64, MAX_IMAGE_BYTES) {
        return CachedImage::TooLarge;
    }
    match image_dimensions(full) {
        Ok(dimensions) => CachedImage::Loaded(ImageAsset { bytes, dimensions }),
        Err(_) => CachedImage::Unavailable,
    }
}

/// Retourne l'image mise en cache pour `full`, en la lisant au premier accès.
fn image_for(full: &Path) -> CachedImage {
    {
        let cache = IMAGE_CACHE
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(entry) = cache.get(full) {
            return entry.clone();
        }
    }
    let fetched = fetch_image(full);
    let mut cache = IMAGE_CACHE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    cache.insert(full.to_path_buf(), fetched.clone());
    fetched
}

/// Décision finale d'affichage d'un embed d'image, octets inclus.
pub enum EmbedDisplay {
    /// Afficher l'image réelle.
    Image {
        bytes: Vec<u8>,
        dimensions: ImageDimensions,
    },
    /// Image trop volumineuse : message discret.
    TooLarge,
    /// Retomber sur le placeholder sobre.
    Placeholder,
}

/// Résout une cible d'embed `![[...]]` vers une décision d'affichage.
///
/// Lecture confinée au vault (chemins canonisés), plafond de 32 Mio, et
/// repli systématique vers le placeholder en cas d'échec.
pub fn resolve_display(root: &Path, target: &str) -> EmbedDisplay {
    let resolved = match resolve_embed_target(root, target) {
        ResolveOutcome::Found(resolved) => resolved,
        ResolveOutcome::NotFound => return EmbedDisplay::Placeholder,
    };
    let Some(full) = canonical_inside_vault(root, &resolved.path) else {
        return EmbedDisplay::Placeholder;
    };
    match image_for(&full) {
        CachedImage::Loaded(asset) => EmbedDisplay::Image {
            bytes: asset.bytes,
            dimensions: asset.dimensions,
        },
        CachedImage::TooLarge => EmbedDisplay::TooLarge,
        CachedImage::Unavailable => EmbedDisplay::Placeholder,
    }
}

// ---------------------------------------------------------------------------
// Construction du widget iced
// ---------------------------------------------------------------------------

/// Placeholder sobre : icône + nom de la cible (même style que l'existant).
fn placeholder_row(icon: &str, target: &str) -> Element<'static, Message> {
    let color = Color::from_rgb(0.2, 0.4, 0.8);
    row![
        text(icon.to_string()).color(color),
        text(target.to_string()).color(color)
    ]
    .spacing(2)
    .into()
}

/// Message discret pour une image au-delà du plafond de taille.
fn too_large_row(target: &str) -> Element<'static, Message> {
    let accent = Color::from_rgb(0.2, 0.4, 0.8);
    let muted = Color::from_rgb(0.55, 0.55, 0.58);
    row![
        text("🖼 ").color(accent),
        text(target.to_string()).color(accent),
        text("(image trop volumineuse)").color(muted),
    ]
    .spacing(2)
    .into()
}

/// Widget d'un embed d'image : l'image réelle si lisible, sinon un placeholder
/// sobre (ou un message discret si trop volumineuse). Jamais d'erreur fatale.
pub fn embed_image_element(root: &Path, target: &str) -> Element<'static, Message> {
    match resolve_display(root, target) {
        EmbedDisplay::Image { bytes, dimensions } => {
            let (width, height) = display_size(dimensions, MAX_IMAGE_WIDTH);
            if width <= 0.0 || height <= 0.0 {
                return placeholder_row("🖼 ", target);
            }
            image(iced::widget::image::Handle::from_bytes(bytes))
                .width(Length::Fixed(width))
                .height(Length::Fixed(height))
                .content_fit(ContentFit::Contain)
                .into()
        }
        EmbedDisplay::TooLarge => too_large_row(target),
        EmbedDisplay::Placeholder => placeholder_row("🖼 ", target),
    }
}
