//! Benchmarks obsy (ignorés par défaut, exécution manuelle).
//!
//! Génère des vaults synthétiques (frontmatter, tags, wikilinks croisés,
//! corps ~1,5 Ko par note) et mesure : scan, index NoteIndex, index
//! full-text, recherche, requête Dataview, construction du graphe, export
//! PDF et export ZIP.
//!
//! Exécution :
//!   cargo test --test benchmarks --release -- --ignored --nocapture

use obsy::vault::dataview::query_notes;
use obsy::vault::graph::build_graph;
use obsy::vault::note::Note;
use obsy::vault::pdf::note_to_pdf;
use obsy::vault::scan::scan_vault;
use obsy::vault::search::build_text_index;
use obsy::vault::vault_archive::export_vault_zip;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Génère un vault de `n` notes dans 40 dossiers.
///
/// Chaque note : frontmatter (titre, priorité, tags), ~40 lignes de corps
/// (~1,5 Ko), 6 wikilinks croisés vers ses voisines.
fn générer_vault(racine: &Path, n: usize) {
    fs::create_dir_all(racine).expect("racine");
    for dossier in 0..40 {
        fs::create_dir_all(racine.join(format!("dossier-{dossier:02}"))).ok();
    }
    for i in 0..n {
        let dossier = format!("dossier-{:02}", i % 40);
        let mut corps = format!(
            "---\ntitle: Note {i}\npriority: {}\ntags: [bench, equipe{}]\n---\n\n# Note {i}\n\n",
            i % 5,
            i % 7
        );
        for ligne in 0..30 {
            corps.push_str(&format!(
                "Paragraphe de remplissage {ligne} pour la note {i} : la mise en forme et les liens font le poids du coffre.\n"
            ));
        }
        for k in 1..=6usize {
            let voisin = (i + k * 7) % n;
            corps.push_str(&format!("- Voir [[note_{voisin}]]\n"));
        }
        fs::write(racine.join(dossier).join(format!("note_{i}.md")), corps)
            .expect("écriture note");
    }
}

fn vault_temporaire(nom: &str, n: usize) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "obsy-bench-{nom}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    générer_vault(&dir, n);
    dir
}

/// Mesure et affiche un résultat.
fn mesurer(label: &str, f: impl FnOnce()) {
    let t0 = Instant::now();
    f();
    println!("{label} : {:?}", t0.elapsed());
}

#[test]
#[ignore = "benchmark manuel : cargo test --test benchmarks --release -- --ignored --nocapture"]
fn benchmarks_vault_5000() {
    let racine = vault_temporaire("5k", 5_000);
    println!("=== Vault 5 000 notes (40 dossiers, ~7,5 Mo) ===");

    let notes: Vec<Note> = {
        let t0 = Instant::now();
        let notes = scan_vault(&racine).expect("scan");
        println!("scan + frontmatter : {:?}", t0.elapsed());
        notes
    };

    {
        let t0 = Instant::now();
        let index = obsy::vault::index::NoteIndex::build_index(&notes);
        println!("index NoteIndex (liens, tags) : {:?}", t0.elapsed());
        let t0 = Instant::now();
        let graphe = build_graph(&notes, &index);
        println!(
            "graphe ({} nœuds, {} composantes) : {:?}",
            notes.len(),
            graphe.components().0,
            t0.elapsed()
        );
    }

    {
        let items: Vec<(String, String)> = notes
            .iter()
            .map(|note| {
                (
                    note.name.clone(),
                    fs::read_to_string(racine.join(&note.path)).unwrap_or_default(),
                )
            })
            .collect();
        let t0 = Instant::now();
        let index = build_text_index(&items);
        println!("index full-text : {:?}", t0.elapsed());

        // 200 requêtes représentatives : latence moyenne et maximale.
        let mut total = std::time::Duration::ZERO;
        let mut max = std::time::Duration::ZERO;
        for r in 0..200 {
            let requête = format!("remplissage {}", r % 30);
            let t0 = Instant::now();
            let _ = index.search_text(&requête, 50);
            let d = t0.elapsed();
            total += d;
            max = max.max(d);
        }
        println!(
            "recherche full-text : moyenne {:?} · max {:?} (200 requêtes)",
            total / 200,
            max
        );
    }

    {
        let t0 = Instant::now();
        let résultats = query_notes(&notes, "SELECT file.name, priority FROM #bench SORT priority DESC LIMIT 50")
            .expect("dql");
        println!(
            "Dataview SELECT ({} résultats) : {:?}",
            résultats.len(),
            t0.elapsed()
        );
    }

    {
        let source = fs::read_to_string(racine.join("dossier-00/note_0.md")).expect("note");
        let t0 = Instant::now();
        let pdf = note_to_pdf("Note 0", &source).expect("pdf");
        println!("export PDF ({} octets) : {:?}", pdf.len(), t0.elapsed());
    }

    {
        let dest = racine.parent().unwrap().join("obsy-bench-export.zip");
        let t0 = Instant::now();
        export_vault_zip(&racine, &dest).expect("zip");
        println!("export ZIP : {:?}", t0.elapsed());
        fs::remove_file(&dest).ok();
    }

    let _ = fs::remove_dir_all(&racine);
}

#[test]
#[ignore = "benchmark manuel : montée en charge du scan"]
fn benchmarks_scan_10000() {
    let racine = vault_temporaire("10k", 10_000);
    println!("=== Vault 10 000 notes (~15 Mo) ===");
    mesurer("scan + frontmatter", || {
        let notes = scan_vault(&racine).expect("scan");
        assert_eq!(notes.len(), 10_000);
    });
    let _ = fs::remove_dir_all(&racine);
}
