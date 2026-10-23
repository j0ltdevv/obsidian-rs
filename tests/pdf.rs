//! Tests d'intégration du générateur de PDF (`vault::pdf`).
//!
//! Tous les tests écrivent uniquement dans `std::env::temp_dir()`.

use obsy::vault::pdf::{ecrire_pdf_atomique, note_to_pdf};

/// Lit la table xref et retourne le tableau des décalages d'objets.
///
/// Les décalages du PDF sont des **positions en octets** : on travaille donc
/// sur le tableau d'octets, et non sur une chaîne UTF-8 (l'en-tête binaire du
/// PDF décalerait tous les index).
fn table_xref(doc: &[u8]) -> Vec<usize> {
    let pos = doc
        .windows(9)
        .rposition(|fenetre| fenetre == b"startxref")
        .expect("startxref présent");
    let texte = String::from_utf8_lossy(&doc[pos..]);
    let decalage: usize = texte
        .split_whitespace()
        .nth(1)
        .expect("décalage startxref")
        .parse()
        .expect("décalage numérique");
    let section = &doc[decalage..];
    assert!(section.starts_with(b"xref\n"), "table xref attendue");
    let apres = String::from_utf8_lossy(section.strip_prefix(b"xref\n").unwrap());
    let mut lignes = apres.lines();
    let entete = lignes.next().expect("en-tête xref");
    let nombre: usize = entete
        .split_whitespace()
        .nth(1)
        .expect("nombre d'objets")
        .parse()
        .expect("nombre numérique");
    (0..nombre)
        .map(|_| {
            let ligne = lignes.next().expect("ligne xref");
            ligne
                .split_whitespace()
                .next()
                .expect("décalage")
                .parse()
                .expect("décalage numérique")
        })
        .collect()
}

/// Vérifie que chaque objet en service pointe bien sur son en-tête.
fn verifier_consistance_xref(doc: &[u8]) {
    let offsets = table_xref(doc);
    assert_eq!(offsets[0], 0, "l'objet 0 est libre");
    for i in 1..offsets.len() {
        let attendu = format!("{i} 0 obj");
        assert!(
            doc[offsets[i]..].starts_with(attendu.as_bytes()),
            "l'objet {i} ne pointe pas sur son en-tête"
        );
    }
}

/// Nombre de pages (objets `/Type /Page`).
fn nombre_de_pages(doc: &[u8]) -> usize {
    doc.windows(12).filter(|w| w == b"/Type /Page ").count()
}

/// Vérifie les invariants structurels d'un PDF (en-tête, fin, xref).
fn verifier_structure(doc: &[u8]) {
    assert!(doc.starts_with(b"%PDF-1.4"), "en-tête %PDF-1.4");
    // La spécification PDF autorise un retour à la ligne après `%%EOF`.
    assert!(doc.trim_ascii_end().ends_with(b"%%EOF"), "fin %%EOF");
    verifier_consistance_xref(doc);
}

#[test]
fn entete_et_fin_de_fichier() {
    let pdf = note_to_pdf("Test", "# Titre\nBonjour").unwrap();
    verifier_structure(&pdf);
}

#[test]
fn phrase_ascii_present_dans_le_flux() {
    let md = "un mot\n";
    let pdf = note_to_pdf("T", md).unwrap();
    verifier_structure(&pdf);
    // Le texte est écrit dans le flux de contenu : on vérifie la présence des
    // mots, pas la mise en forme exacte du flux.
    assert!(pdf.windows(2).any(|w| w == b"un"), "texte « un » présent");
    assert!(pdf.windows(3).any(|w| w == b"mot"), "texte « mot » présent");
}

#[test]
fn parentheses_echappees() {
    let md = "voir (exemple) ci\n";
    let pdf = note_to_pdf("T", md).unwrap();
    verifier_structure(&pdf);
    // Dans un littéral PDF, `(` et `)` doivent être échappés : le texte
    // `(exemple)` s'écrit `\(exemple\)` dans le flux.
    assert!(
        pdf.windows(11).any(|w| w == br"\(exemple\)"),
        "parenthèses échappées"
    );
}

#[test]
fn backslash_echappe() {
    let md = "\\fin\n";
    let pdf = note_to_pdf("T", md).unwrap();
    verifier_structure(&pdf);
    // Le littéral PDF du mot `\fin` devient `(\\fin )`.
    assert!(
        pdf.windows(8).any(|w| w == b"(\\\\fin )"),
        "backslash échappé"
    );
}

#[test]
fn frontmatter_exclu_du_corps() {
    let md = "---\ntitle: Test\n---\n# Vrai titre\n";
    let pdf = note_to_pdf("X", md).unwrap();
    verifier_structure(&pdf);
    assert!(
        !pdf.windows(7).any(|w| w == b"(title )"),
        "le frontmatter ne doit pas apparaître"
    );
    assert!(
        !pdf.windows(6).any(|w| w == b"(Test )"),
        "le frontmatter ne doit pas apparaître"
    );
    assert!(
        pdf.windows(7).any(|w| w == b"(Vrai )"),
        "le corps doit apparaître"
    );
    assert!(
        pdf.windows(8).any(|w| w == b"(titre )"),
        "le corps doit apparaître"
    );
}

#[test]
fn non_ascii_ne_panique_pas() {
    let md = "français écrè être à 中文 テスト test\n";
    let pdf = note_to_pdf("T", md).unwrap();
    verifier_structure(&pdf);
    // Les accents sont encodés en Windows-1252 (octet ≥ 0x80).
    assert!(pdf.contains(&0xE9), "l'accent aigu doit être encodé");
    // Les idéogrammes non représentables sont remplacés par `?`.
    assert!(pdf.contains(&b'?'), "les idéogrammes sont remplacés");
}

#[test]
fn document_vide_produit_un_pdf_valide() {
    let pdf = note_to_pdf("Vide", "").unwrap();
    verifier_structure(&pdf);
    assert!(nombre_de_pages(&pdf) >= 1, "au moins une page");
}

#[test]
fn note_longue_produit_plusieurs_pages() {
    let mut md = String::new();
    for i in 0..200 {
        md.push_str(&format!(
            "Paragraphe {i} avec assez de texte pour remplir des lignes et provoquer plusieurs pages.\n\n"
        ));
    }
    let pdf = note_to_pdf("Longue", &md).unwrap();
    verifier_structure(&pdf);
    assert!(
        nombre_de_pages(&pdf) > 1,
        "une note longue doit produire plusieurs pages"
    );
}

#[test]
fn listes_numerotees_conservent_leur_numero() {
    let md = "1. premier\n2. second\n";
    let pdf = note_to_pdf("T", md).unwrap();
    verifier_structure(&pdf);
    assert!(
        pdf.windows(5).any(|w| w == b"(1. )"),
        "le numéro 1 est conservé"
    );
    assert!(
        pdf.windows(5).any(|w| w == b"(2. )"),
        "le numéro 2 est conservé"
    );
}

#[test]
fn ecriture_atomique_dans_repertoire_temporaire() {
    let dossier = std::env::temp_dir();
    let chemin = dossier.join(format!("test_pdf_{}.pdf", std::process::id()));
    let pdf = note_to_pdf("T", "# Titre\nBonjour").unwrap();

    ecrire_pdf_atomique(&chemin, &pdf).expect("écriture atomique");
    let relu = std::fs::read(&chemin).expect("lecture du fichier écrit");
    assert_eq!(relu, pdf, "le fichier relu correspond aux octets");
    std::fs::remove_file(&chemin).ok();
}
