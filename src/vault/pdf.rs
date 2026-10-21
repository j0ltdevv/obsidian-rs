//! Générateur de PDF minimal, sans dépendance externe.
//!
//! Produit un fichier PDF 1.4 lisible par n'importe quel lecteur à partir du
//! rendu Markdown déjà disponible (`live_preview::render_markdown`).
//!
//! # Format produit
//!
//! - En-tête `%PDF-1.4`, table xref, `startxref` et `%%EOF` calculés en
//!   octets exacts.
//! - Polices de base 14 (Helvetica, Helvetica-Bold, Courier) : aucune
//!   incorporation de fichier de police.
//! - Texte encodé Windows-1252 (WinAnsi), échappé pour les caractères PDF
//!   `(`, `)`, `\`. Les caractères non représentables sont remplacés par `?`
//!   sans jamais provoquer de panic.
//! - Mise en page A4 (595 × 842 pt) avec marges, interligne et pagination
//!   automatique (numéro de page en pied de page).
//!
//! # Exclusion du frontmatter
//!
//! Le bloc frontmatter YAML `--- ... ---` en tête de document est retiré du
//! corps avant rendu (même règle que `frontmatter::parse_frontmatter`).

use super::live_preview::{render_markdown, Segment, SegmentStyle};
use std::collections::HashMap;
use std::io::Write;

// ─────────────────────────────── Constantes de mise en page ───────────────────────────────

/// Largeur d'une page A4 en points.
const PAGE_LARGEUR: f32 = 595.0;
/// Hauteur d'une page A4 en points.
const PAGE_HAUTEUR: f32 = 842.0;
/// Marge gauche/droite en points.
const MARGE_X: f32 = 56.0;
/// Position Y de la première ligne (baseline) en points.
const CONTENU_HAUT: f32 = 770.0;
/// En dessous de cette position Y, on passe à la page suivante.
const CONTENU_BAS: f32 = 50.0;
/// Position Y du numéro de page en pied de page.
const PIED_Y: f32 = 30.0;

/// Largeur utile du corps de texte (A4 moins les deux marges).
const LARGEUR_TEXTE: f32 = PAGE_LARGEUR - 2.0 * MARGE_X;

// ─────────────────────────────── Erreurs ───────────────────────────────

/// Erreurs du générateur de PDF.
#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    /// Échec d'écriture du fichier sur disque.
    #[error("échec d'écriture du PDF : {0}")]
    Ecriture(#[from] std::io::Error),
    /// Le chemin fourni est inutilisable (pas de parent ni de nom de fichier).
    #[error("chemin invalide pour le PDF")]
    CheminInvalide,
}

// ─────────────────────────────── Polices et métriques ───────────────────────────────

/// Police utilisée pour un morceau de texte.
#[derive(Clone, Copy, PartialEq)]
enum Police {
    /// Helvetica (corps).
    Romaine,
    /// Helvetica-Bold (titres, gras).
    Grasse,
    /// Courier (code).
    ChasseFixe,
}

/// Largeurs Helvetica (AFM, en 1/1000 d'em) pour les caractères ASCII 32..=126.
const LARGEURS_HELVETICA: &[u16] = &[
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, // 32-47
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, // 48-63
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, // 64-79
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, // 80-95
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, // 96-111
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584, // 112-126
];

/// Largeur d'un caractère donné (en points) pour une police et une taille.
fn largeur_caractere(police: Police, taille: f32, c: char) -> f32 {
    match police {
        Police::ChasseFixe => 0.6 * taille,
        Police::Romaine | Police::Grasse => {
            let b = c as u32;
            if (32..=126).contains(&b) {
                LARGEURS_HELVETICA[(b - 32) as usize] as f32 * taille / 1000.0
            } else {
                // Les caractères hors plage sont remplacés par `?` à l'encodage.
                0.5 * taille
            }
        }
    }
}

/// Largeur totale d'une chaîne (en points).
fn largeur_texte(police: Police, taille: f32, texte: &str) -> f32 {
    texte
        .chars()
        .map(|c| largeur_caractere(police, taille, c))
        .sum()
}

// ─────────────────────────────── Modèle de mise en page ───────────────────────────────

/// Un morceau de texte stylé sur une ligne physique.
struct Morceau {
    texte: String,
    police: Police,
    taille: f32,
}

/// Une ligne physique de texte (suite de morceaux).
struct Ligne {
    morceaux: Vec<Morceau>,
    /// Hauteur occupée par cette ligne (interligne).
    hauteur: f32,
}

/// Un bloc de contenu (paragraphe, titre, code, règle…).
struct Bloc {
    lignes: Vec<Ligne>,
    /// Espace vertical avant le bloc.
    espace_avant: f32,
    /// Indentation gauche supplémentaire (en points).
    indentation: f32,
    /// `true` si le bloc est une règle horizontale (pas de texte).
    regle: bool,
}

// ─────────────────────────────── API publique ───────────────────────────────

/// Produit un PDF 1.4 d'une note Markdown.
///
/// Le frontmatter YAML en tête de document est exclu du corps. La mise en
/// page est A4, avec pagination automatique et numéro de page en pied de
/// page. Les caractères non représentables en Windows-1252 sont remplacés par
/// `?` (jamais de panic).
///
/// # Arguments
///
/// - `titre` : titre de la note, inscrit dans les métadonnées du PDF.
/// - `markdown` : contenu Markdown brut de la note.
///
/// # Retour
///
/// Les octets du PDF, prêts à être écrits sur disque.
pub fn note_to_pdf(titre: &str, markdown: &str) -> Result<Vec<u8>, PdfError> {
    let corps = corps_sans_frontmatter(markdown);
    let segments = render_markdown(corps);
    let numeros = numeros_listes(corps);
    let blocs = construire_blocs(&segments, &numeros);
    let pages = mettre_en_page(&blocs);
    Ok(assembler(pages, titre))
}

/// Écrit un PDF de manière atomique (fichier temporaire puis `rename`).
///
/// Le fichier temporaire est créé dans le même dossier que `chemin`, puis
/// renommé par-dessus après écriture et synchronisation. Le chemin est confiné
/// à celui demandé.
pub fn ecrire_pdf_atomique(chemin: &std::path::Path, octets: &[u8]) -> Result<(), PdfError> {
    let dossier = chemin.parent().ok_or(PdfError::CheminInvalide)?;
    let nom = chemin.file_name().ok_or(PdfError::CheminInvalide)?;
    let temporaire = dossier.join(format!(
        ".{}.tmp{}",
        nom.to_string_lossy(),
        std::process::id()
    ));
    let mut fichier = std::fs::File::create(&temporaire)?;
    fichier.write_all(octets)?;
    fichier.sync_all()?;
    std::fs::rename(&temporaire, chemin)?;
    Ok(())
}

// ─────────────────────────────── Frontmatter ───────────────────────────────

/// Retire le bloc frontmatter `--- ... ---` en tête de document.
///
/// Retourne le corps (substring). Si le bloc n'est pas fermé, retourne une
/// chaîne vide. Si la première ligne n'est pas `---`, retourne `source` tel
/// quel.
fn corps_sans_frontmatter(source: &str) -> &str {
    let mut lignes = source.lines();
    let Some(ouvrant) = lignes.next() else {
        return source;
    };
    if ouvrant.trim() != "---" {
        return source;
    }

    let mut curseur = source.find('\n').map_or(source.len(), |i| i + 1);
    loop {
        match source[curseur..].find('\n') {
            Some(nl) => {
                let fin = curseur + nl;
                if source[curseur..fin].trim() == "---" {
                    return &source[fin + 1..];
                }
                curseur = fin + 1;
            }
            None => {
                if source[curseur..].trim() == "---" {
                    return "";
                }
                return "";
            }
        }
    }
}

/// Indexe les préfixes de listes numérotées (`1.`, `2.`…) par numéro de ligne.
///
/// La ligne de chaque segment (`Segment::line`, 0-based) sert de clé pour
/// retrouver le numéro d'origine du marqueur, perdu par le rendu.
fn numeros_listes(source: &str) -> HashMap<usize, String> {
    let mut map = HashMap::new();
    for (i, ligne) in source.lines().enumerate() {
        let t = ligne.trim_start();
        if let Some(pos) = t.find('.') {
            if pos > 0 && t[..pos].chars().all(|c| c.is_ascii_digit()) {
                let apres = &t[pos + 1..];
                if apres.is_empty() || apres.starts_with(char::is_whitespace) {
                    map.insert(i, t[..=pos].to_string());
                }
            }
        }
    }
    map
}

// ─────────────────────────────── Construction des blocs ───────────────────────────────

/// `true` si le style démarre un nouveau bloc (vs. continuation inline).
fn est_bloc(style: SegmentStyle) -> bool {
    matches!(
        style,
        SegmentStyle::H1
            | SegmentStyle::H2
            | SegmentStyle::H3
            | SegmentStyle::H4
            | SegmentStyle::H5
            | SegmentStyle::H6
            | SegmentStyle::CodeBlock
            | SegmentStyle::Blockquote
            | SegmentStyle::ListBullet
            | SegmentStyle::ListNumbered
            | SegmentStyle::TaskCheckboxTodo
            | SegmentStyle::TaskCheckboxDone
            | SegmentStyle::HorizontalRule
            | SegmentStyle::Callout
            | SegmentStyle::CalloutTitle
            | SegmentStyle::Table
            | SegmentStyle::MathDisplay
            | SegmentStyle::Footnote
            | SegmentStyle::Embed
            | SegmentStyle::EmbedImage
            | SegmentStyle::EmbedPdf
    )
}

/// Taille de police et interligne pour un style de bloc.
fn metriques_bloc(style: SegmentStyle) -> (f32, f32, Police) {
    match style {
        SegmentStyle::H1 => (24.0, 30.0, Police::Grasse),
        SegmentStyle::H2 => (20.0, 26.0, Police::Grasse),
        SegmentStyle::H3 => (16.0, 22.0, Police::Grasse),
        SegmentStyle::H4 => (14.0, 19.0, Police::Grasse),
        SegmentStyle::H5 => (13.0, 18.0, Police::Grasse),
        SegmentStyle::H6 => (12.0, 16.0, Police::Grasse),
        SegmentStyle::CodeBlock => (10.0, 13.0, Police::ChasseFixe),
        SegmentStyle::Table => (11.0, 14.0, Police::Romaine),
        SegmentStyle::Footnote => (10.0, 13.0, Police::Romaine),
        _ => (12.0, 16.0, Police::Romaine),
    }
}

/// Construit les morceaux inline d'un segment de continuation.
fn morceaux_inline(seg: &Segment) -> Vec<Morceau> {
    let (police, taille) = match seg.style {
        SegmentStyle::Bold | SegmentStyle::BoldItalic => (Police::Grasse, 12.0),
        SegmentStyle::InlineCode => (Police::ChasseFixe, 11.0),
        _ => (Police::Romaine, 12.0),
    };
    vec![Morceau {
        texte: seg.text.clone(),
        police,
        taille,
    }]
}

/// Construit un bloc de texte à partir de morceaux, en coupant aux lignes.
fn bloc_texte(morceaux: Vec<Morceau>, hauteur: f32, indentation: f32) -> Bloc {
    let mut lignes = decouper_lignes(&morceaux, LARGEUR_TEXTE - indentation);
    for ligne in &mut lignes {
        ligne.hauteur = hauteur;
    }
    Bloc {
        lignes,
        espace_avant: 6.0,
        indentation,
        regle: false,
    }
}

/// Découpe une suite de morceaux en lignes physiques (retour à la ligne).
///
/// Chaque mot est conservé avec un espace final, pour garder des espaces
/// littéraux dans le contenu du PDF (plus lisible et vérifiable en test).
fn decouper_lignes(morceaux: &[Morceau], largeur_max: f32) -> Vec<Ligne> {
    let mut atomes: Vec<Morceau> = Vec::new();
    for m in morceaux {
        for mot in m.texte.split_whitespace() {
            atomes.push(Morceau {
                texte: format!("{mot} "),
                police: m.police,
                taille: m.taille,
            });
        }
    }

    let mut lignes: Vec<Ligne> = Vec::new();
    let mut courante: Vec<Morceau> = Vec::new();
    let mut largeur_courante = 0.0_f32;

    for atome in atomes {
        let largeur_atome = largeur_texte(atome.police, atome.taille, &atome.texte);

        if largeur_courante + largeur_atome > largeur_max && !courante.is_empty() {
            lignes.push(Ligne {
                morceaux: courante,
                hauteur: 0.0, // rempli plus tard
            });
            courante = Vec::new();
            largeur_courante = 0.0;
        }

        if largeur_atome > largeur_max {
            // Un seul mot trop long : découpe caractère par caractère.
            let mut tampon = String::new();
            let mut largeur_tampon = 0.0_f32;
            for c in atome.texte.chars() {
                let lc = largeur_caractere(atome.police, atome.taille, c);
                if largeur_tampon + lc > largeur_max && !tampon.is_empty() {
                    lignes.push(Ligne {
                        morceaux: vec![Morceau {
                            texte: std::mem::take(&mut tampon),
                            police: atome.police,
                            taille: atome.taille,
                        }],
                        hauteur: 0.0,
                    });
                    largeur_tampon = 0.0;
                }
                tampon.push(c);
                largeur_tampon += lc;
            }
            if !tampon.is_empty() {
                courante.push(Morceau {
                    texte: tampon,
                    police: atome.police,
                    taille: atome.taille,
                });
                largeur_courante = largeur_tampon;
            }
        } else {
            largeur_courante += largeur_atome;
            courante.push(atome);
        }
    }

    if !courante.is_empty() {
        lignes.push(Ligne {
            morceaux: courante,
            hauteur: 0.0,
        });
    }

    lignes
}

/// Vide le paragraphe courant dans la liste de blocs, s'il est non vide.
fn vider_paragraphe(blocs: &mut Vec<Bloc>, paragraphe: &mut Vec<Morceau>) {
    if !paragraphe.is_empty() {
        blocs.push(bloc_texte(std::mem::take(paragraphe), 16.0, 0.0));
    }
}

/// Construit tous les blocs à partir des segments de rendu.
fn construire_blocs(segments: &[Segment], numeros: &HashMap<usize, String>) -> Vec<Bloc> {
    let mut blocs: Vec<Bloc> = Vec::new();
    let mut paragraphe: Vec<Morceau> = Vec::new();

    for seg in segments {
        if est_bloc(seg.style) {
            vider_paragraphe(&mut blocs, &mut paragraphe);
            let (taille, hauteur, police) = metriques_bloc(seg.style);

            match seg.style {
                SegmentStyle::HorizontalRule => {
                    blocs.push(Bloc {
                        lignes: Vec::new(),
                        espace_avant: 6.0,
                        indentation: 0.0,
                        regle: true,
                    });
                }
                SegmentStyle::CodeBlock => {
                    let lignes = seg
                        .text
                        .split('\n')
                        .map(|ligne| Ligne {
                            morceaux: vec![Morceau {
                                texte: ligne.to_string(),
                                police,
                                taille,
                            }],
                            hauteur,
                        })
                        .collect();
                    blocs.push(Bloc {
                        lignes,
                        espace_avant: 6.0,
                        indentation: 12.0,
                        regle: false,
                    });
                }
                SegmentStyle::ListBullet => {
                    let morceaux = vec![Morceau {
                        texte: format!("- {}", seg.text),
                        police,
                        taille,
                    }];
                    blocs.push(bloc_texte(morceaux, hauteur, 14.0));
                }
                SegmentStyle::ListNumbered => {
                    let prefixe = numeros
                        .get(&seg.line)
                        .cloned()
                        .filter(|p| p.is_ascii())
                        .unwrap_or_else(|| "- ".to_string());
                    let morceaux = vec![Morceau {
                        texte: format!("{prefixe} {}", seg.text),
                        police,
                        taille,
                    }];
                    blocs.push(bloc_texte(morceaux, hauteur, 14.0));
                }
                SegmentStyle::TaskCheckboxTodo => {
                    let morceaux = vec![Morceau {
                        texte: format!("[ ] {}", seg.text),
                        police,
                        taille,
                    }];
                    blocs.push(bloc_texte(morceaux, hauteur, 16.0));
                }
                SegmentStyle::TaskCheckboxDone => {
                    let morceaux = vec![Morceau {
                        texte: format!("[x] {}", seg.text),
                        police,
                        taille,
                    }];
                    blocs.push(bloc_texte(morceaux, hauteur, 16.0));
                }
                SegmentStyle::Blockquote => {
                    let morceaux = vec![Morceau {
                        texte: seg.text.clone(),
                        police,
                        taille,
                    }];
                    blocs.push(bloc_texte(morceaux, hauteur, 20.0));
                }
                SegmentStyle::Callout | SegmentStyle::CalloutTitle => {
                    let morceaux = vec![Morceau {
                        texte: seg.text.clone(),
                        police,
                        taille,
                    }];
                    blocs.push(bloc_texte(morceaux, hauteur, 12.0));
                }
                SegmentStyle::MathDisplay => {
                    let morceaux = vec![Morceau {
                        texte: seg.text.clone(),
                        police,
                        taille,
                    }];
                    blocs.push(bloc_texte(morceaux, hauteur, 8.0));
                }
                SegmentStyle::Embed | SegmentStyle::EmbedImage | SegmentStyle::EmbedPdf => {
                    let morceaux = vec![Morceau {
                        texte: format!("[embed: {}]", seg.text),
                        police: Police::Romaine,
                        taille: 11.0,
                    }];
                    blocs.push(bloc_texte(morceaux, 14.0, 0.0));
                }
                _ => {
                    // Titres, tables, notes de bas de page : un texte simple.
                    let morceaux = vec![Morceau {
                        texte: seg.text.clone(),
                        police,
                        taille,
                    }];
                    let indentation = if seg.style == SegmentStyle::Footnote {
                        12.0
                    } else {
                        0.0
                    };
                    blocs.push(bloc_texte(morceaux, hauteur, indentation));
                }
            }
        } else {
            paragraphe.extend(morceaux_inline(seg));
        }
    }

    vider_paragraphe(&mut blocs, &mut paragraphe);
    blocs
}

// ─────────────────────────────── Mise en page ───────────────────────────────

/// Émet la ligne de texte `ligne` dans le flux `sortie` à la position Y.
fn emettre_ligne(sortie: &mut Vec<u8>, ligne: &Ligne, indentation: f32, y: f32) {
    let mut x = MARGE_X + indentation;
    for morceau in &ligne.morceaux {
        let numero_police = match morceau.police {
            Police::Romaine => 1,
            Police::Grasse => 2,
            Police::ChasseFixe => 3,
        };
        sortie.extend(
            format!("BT /F{numero_police} {} {} {} Td ", morceau.taille, x, y).into_bytes(),
        );
        pousser_chaine_echappee(sortie, &morceau.texte);
        sortie.extend(b" Tj ET\n");
        x += largeur_texte(morceau.police, morceau.taille, &morceau.texte);
    }
}

/// Émet une règle horizontale dans le flux `sortie` à la position Y.
fn emettre_regle(sortie: &mut Vec<u8>, indentation: f32, y: f32) {
    let x1 = MARGE_X + indentation;
    let x2 = PAGE_LARGEUR - MARGE_X;
    sortie.extend(format!("{x1} {y} m {x2} {y} l S\n").into_bytes());
}

/// Émet le numéro de page centré en pied de page.
fn emettre_pied(sortie: &mut Vec<u8>, numero_page: usize) {
    let texte = numero_page.to_string();
    let largeur = largeur_texte(Police::Romaine, 10.0, &texte);
    let cx = MARGE_X + (LARGEUR_TEXTE - largeur) / 2.0;
    sortie.extend(format!("BT /F1 10 {cx} {PIED_Y} Td ").into_bytes());
    pousser_chaine_echappee(sortie, &texte);
    sortie.extend(b" Tj ET\n");
}

/// Termine une page (ajoute le pied de page) et la retourne.
fn finaliser_page(flux: &mut Vec<u8>, numero_page: usize) -> Vec<u8> {
    emettre_pied(flux, numero_page);
    std::mem::take(flux)
}

/// Dispose les blocs sur des pages A4 avec pagination.
fn mettre_en_page(blocs: &[Bloc]) -> Vec<Vec<u8>> {
    let mut pages: Vec<Vec<u8>> = Vec::new();
    let mut flux: Vec<u8> = Vec::new();
    let mut y = CONTENU_HAUT;

    for bloc in blocs {
        let hauteur_bloc = if bloc.regle {
            bloc.espace_avant
        } else {
            bloc.espace_avant + bloc.lignes.iter().map(|l| l.hauteur).sum::<f32>()
        };

        // La page courante contient-elle du contenu ? (le tampon de flux est
        // vidé à chaque saut de page).
        if !flux.is_empty() && y - hauteur_bloc < CONTENU_BAS {
            let page = finaliser_page(&mut flux, pages.len() + 1);
            pages.push(page);
            flux = Vec::new();
            y = CONTENU_HAUT;
        }

        if bloc.regle {
            y -= bloc.espace_avant;
            emettre_regle(&mut flux, bloc.indentation, y);
        } else {
            y -= bloc.espace_avant;
            for ligne in &bloc.lignes {
                if y - ligne.hauteur < CONTENU_BAS {
                    let page = finaliser_page(&mut flux, pages.len() + 1);
                    pages.push(page);
                    flux = Vec::new();
                    y = CONTENU_HAUT;
                }
                emettre_ligne(&mut flux, ligne, bloc.indentation, y);
                y -= ligne.hauteur;
            }
        }
    }

    if !flux.is_empty() || pages.is_empty() {
        let page = finaliser_page(&mut flux, pages.len() + 1);
        pages.push(page);
    }

    pages
}

// ─────────────────────────────── Encodage et échappement ───────────────────────────────

/// Mappe un caractère vers un octet Windows-1252 (WinAnsi).
///
/// Retourne `None` pour les caractères non représentables (remplacés par `?`).
fn octet_winansi(c: char) -> Option<u8> {
    match c as u32 {
        0x20..=0x7E => Some(c as u8),
        0x80..=0x9F => Some(c as u8), // octets Win-1252 (€, œ…)
        0xA0..=0xFF => Some(c as u8), // Latin-1 direct
        _ => None,
    }
}

/// Pousse une chaîne en littéral PDF `(…)` échappé et encodé Windows-1252.
fn pousser_chaine_echappee(sortie: &mut Vec<u8>, texte: &str) {
    sortie.push(b'(');
    for c in texte.chars() {
        match c {
            '(' => sortie.extend(b"\\("),
            ')' => sortie.extend(b"\\)"),
            '\\' => sortie.extend(b"\\\\"),
            '\n' => sortie.extend(b"\\n"),
            '\r' => sortie.extend(b"\\r"),
            '\t' => sortie.push(b' '),
            _ => match octet_winansi(c) {
                Some(o) => sortie.push(o),
                None => sortie.push(b'?'),
            },
        }
    }
    sortie.push(b')');
}

// ─────────────────────────────── Assemblage du document ───────────────────────────────

/// Assemble le document PDF final à partir des flux de pages.
fn assembler(pages: Vec<Vec<u8>>, titre: &str) -> Vec<u8> {
    let p = pages.len();
    let f1 = 3 + 2 * p;
    let f2 = f1 + 1;
    let f3 = f2 + 1;
    let info = f3 + 1;
    let total = info + 1;

    let mut doc: Vec<u8> = Vec::new();
    doc.extend(b"%PDF-1.4\n%");
    doc.extend(&[0xe2, 0xe3, 0xcf, 0xd3]);
    doc.push(b'\n');

    let mut offsets = vec![0usize; total];

    // Pousse un objet simple (hors flux) et enregistre son décalage.
    let pousser = |doc: &mut Vec<u8>, offsets: &mut Vec<usize>, numero: usize, corps: &[u8]| {
        offsets[numero] = doc.len();
        doc.extend(format!("{numero} 0 obj\n").into_bytes());
        doc.extend(corps);
        doc.extend(b"\nendobj\n");
    };

    pousser(
        &mut doc,
        &mut offsets,
        1,
        b"<< /Type /Catalog /Pages 2 0 R >>",
    );

    let kids: Vec<String> = (3..3 + p).map(|o| format!("{o} 0 R")).collect();
    pousser(
        &mut doc,
        &mut offsets,
        2,
        format!("<< /Type /Pages /Kids [{}] /Count {p} >>", kids.join(" ")).as_bytes(),
    );

    for (i, flux) in pages.iter().enumerate() {
        let objet_page = 3 + i;
        let objet_flux = 3 + p + i;
        let corps_page = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_LARGEUR} {PAGE_HAUTEUR}] \
             /Resources << /Font << /F1 {f1} 0 R /F2 {f2} 0 R /F3 {f3} 0 R >> >> \
             /Contents {objet_flux} 0 R >>"
        );
        pousser(&mut doc, &mut offsets, objet_page, corps_page.as_bytes());

        offsets[objet_flux] = doc.len();
        doc.extend(format!("{objet_flux} 0 obj\n").into_bytes());
        doc.extend(format!("<< /Length {} >>\nstream\n", flux.len()).into_bytes());
        doc.extend(flux);
        doc.extend(b"endstream\nendobj\n");
    }

    pousser(
        &mut doc,
        &mut offsets,
        f1,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    );
    pousser(
        &mut doc,
        &mut offsets,
        f2,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>",
    );
    pousser(
        &mut doc,
        &mut offsets,
        f3,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>",
    );

    let mut titre_echappe: Vec<u8> = Vec::new();
    pousser_chaine_echappee(&mut titre_echappe, titre);
    let mut corps_info: Vec<u8> = Vec::new();
    corps_info.extend(b"<< /Producer (obsidian-rs) /Title ");
    corps_info.extend_from_slice(&titre_echappe);
    corps_info.extend(b" >>");
    pousser(&mut doc, &mut offsets, info, &corps_info);

    let decalage_xref = doc.len();
    doc.extend(b"xref\n");
    doc.extend(format!("0 {total}\n").into_bytes());
    doc.extend(b"0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        doc.extend(format!("{offset:010} 00000 n \n").into_bytes());
    }
    doc.extend(b"trailer\n");
    doc.extend(format!("<< /Size {total} /Root 1 0 R >>\n").into_bytes());
    doc.extend(b"startxref\n");
    doc.extend(format!("{decalage_xref}\n").into_bytes());
    doc.extend(b"%%EOF\n");

    doc
}
