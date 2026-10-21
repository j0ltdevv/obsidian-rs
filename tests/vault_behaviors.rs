//! Tests des **comportements déduits** du moteur de vault d'Obsidian.
//!
//! Source unique : `demonstrations/obsidian-rea/ANALYSE.md`, section
//! « Comportements déduits » (analyse statique REA du bundle `app.js`
//! d'Obsidian 1.14.4).
//!
//! ⚠ Ces comportements sont **DÉDUITS** (inférés) de l'analyse statique :
//! ils ne sont pas **observés** à l'exécution (le graphe sémantique REA ne
//! conserve pas les littéraux, cf. « Traces sémériques » de l'ANALYSE.md).
//! Les tests encodent donc la sémantique déduite et la vérifient à travers
//! l'API publique **existante** de `src/vault/` : aucun parsing n'est
//! réimplémenté ici, aucun comportement n'est modifié pour faire passer un
//! test.
//!
//! Fixtures : `tests/fixtures/comportements/` (une par comportement, plus une
//! cible supplémentaire pour le comportement 4).
//!
//! Écarts connus, signalés dans les tests concernés et dans le rapport :
//! - **comportement 3** : `src/vault/` n'expose pas de `mtime` (`Note` et
//!   `NoteIndex` ne portent pas d'horodatage) et `scan_vault` est un rescane
//!   complet — il n'existe pas d'invalidation incrémentale par mtime dans
//!   l'API du vault ;
//! - **comportement 4** : la découpe linkpath/subpath et le message
//!   « file or folder not found » ne sont pas exposés par `src/vault/`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{anyhow, Result};
use obsidian_rs::engine::Engine;
use obsidian_rs::vault::frontmatter::parse_frontmatter;
use obsidian_rs::vault::index::NoteIndex;
use obsidian_rs::vault::outline::extract_outline;
use obsidian_rs::vault::tags::parse_tags;
use obsidian_rs::vault::wikilinks::parse_wikilinks;
use obsidian_rs::vault::{scan_vault, Note};

/// Suffixe unique par vault temporaire (pid + compteur atomique).
fn unique_suffix() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("{}-{n}", std::process::id())
}

/// Vault temporaire supprimé à la fin du test (y compris en cas de panique).
///
/// Les fixtures sont **copiées** dans le vault : un test peut donc modifier
/// un fichier sans jamais toucher au dépôt.
struct TempVault {
    root: PathBuf,
}

impl TempVault {
    fn new() -> Result<Self> {
        let root =
            std::env::temp_dir().join(format!("obsidian-rs-vault-behaviors-{}", unique_suffix()));
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }
}

impl Drop for TempVault {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Dossier des fixtures de ce fichier de tests.
fn dossier_fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("comportements")
}

/// Copie une fixture dans le vault temporaire et retourne son chemin absolu.
fn copier_fixture(vault: &TempVault, nom: &str) -> Result<PathBuf> {
    let destination = vault.root.join(nom);
    fs::copy(dossier_fixtures().join(nom), &destination)?;
    Ok(destination)
}

/// Note du scan dont le nom (stem du fichier) correspond, sinon erreur
/// explicite : jamais de `unwrap()` sur le résultat d'un scan.
fn note_par_nom<'a>(notes: &'a [Note], nom: &str) -> Result<&'a Note> {
    notes
        .iter()
        .find(|note| note.name == nom)
        .ok_or_else(|| anyhow!("note absente du scan : {nom}"))
}

/// **Comportement 1** — ANALYSE.md §« Comportements déduits » (frontmatter =
/// bloc YAML délimité en tête, exposé comme métadonnées structurées).
///
/// Déduit de l'analyse statique, **pas observé** à l'exécution.
#[test]
fn comportement_1_frontmatter_bloc_yaml_en_tete_expose_com_metadonnees() -> Result<()> {
    let vault = TempVault::new()?;
    let chemin = copier_fixture(&vault, "comportement-1-frontmatter-en-tete.md")?;

    // Le bloc YAML n'est un frontmatter que s'il ouvre la note.
    let source = fs::read_to_string(&chemin)?;
    let valeur = parse_frontmatter(&source)?
        .ok_or_else(|| anyhow!("frontmatter non reconnu en tête de note"))?;
    assert_eq!(valeur["title"].as_str(), Some("Comportement 1"));
    assert_eq!(valeur["source"].as_str(), Some("deduction-statique"));

    // Un bloc `---` qui n'ouvre pas la note n'est pas du frontmatter.
    let pas_en_tete = parse_frontmatter("Texte\n---\ntitle: X\n---\n")?;
    assert!(pas_en_tete.is_none());

    // Le scan expose le frontmatter comme métadonnées structurées de la note.
    let notes = scan_vault(&vault.root)?;
    let note = note_par_nom(&notes, "comportement-1-frontmatter-en-tete")?;
    assert!(
        note.metadata.frontmatter_error.is_none(),
        "frontmatter invalide : {:?}",
        note.metadata.frontmatter_error
    );
    let frontmatter = note
        .metadata
        .frontmatter
        .as_ref()
        .ok_or_else(|| anyhow!("frontmatter absent des métadonnées de la note"))?;
    assert_eq!(frontmatter["title"].as_str(), Some("Comportement 1"));
    assert_eq!(frontmatter["source"].as_str(), Some("deduction-statique"));

    Ok(())
}

/// **Comportement 2** — ANALYSE.md §« Comportements déduits » (tags d'une note
/// = union des hashtags du corps et des tags du frontmatter, fusion explicite
/// depuis le frontmatter).
///
/// Déduit de l'analyse statique, **pas observé** à l'exécution.
#[test]
fn comportement_2_tags_union_corps_et_frontmatter() -> Result<()> {
    let vault = TempVault::new()?;
    let chemin = copier_fixture(&vault, "comportement-2-tags-corps-plus-frontmatter.md")?;

    // Le corps seul : uniquement les hashtags inline (le frontmatter n'en
    // contribute aucun tout seul).
    let source = fs::read_to_string(&chemin)?;
    assert_eq!(
        parse_tags(&source),
        vec!["depuis-corps".to_string(), "commun".to_string()]
    );

    // La fusion avec le frontmatter est explicite : union, sans doublon
    // (« commun » est présent des deux côtés, « depuis-frontmatter » seulement
    // dans le frontmatter).
    let notes = scan_vault(&vault.root)?;
    let note = note_par_nom(&notes, "comportement-2-tags-corps-plus-frontmatter")?;
    assert_eq!(
        note.metadata.tags,
        vec![
            "depuis-corps".to_string(),
            "commun".to_string(),
            "depuis-frontmatter".to_string(),
        ]
    );

    Ok(())
}

/// **Comportement 3** — ANALYSE.md §« Comportements déduits » (index de
/// métadonnées par fichier, avec mtime, rafraîchi/invalidation incrémentale).
///
/// Déduit de l'analyse statique, **pas observé** à l'exécution.
///
/// ⚠ **ÉCART CONNU** : l'API publique de `src/vault/` n'expose pas de `mtime`
/// (`Note` et `NoteIndex` ne portent pas d'horodatage) et `scan_vault` est un
/// rescane complet : il n'existe pas d'invalidation incrémentale par mtime
/// dans le vault. Le contrat observable le plus proche — et le seul testable
/// ici — est donc : l'index porte les métadonnées **par fichier**, et un scan
/// suivant l'état courant du disque met l'index à jour. L'invalidation par
/// mtime existe au niveau application (`src/app/mod.rs`, cache `file_mtimes`),
/// et le moteur expose le mtime des fichiers dans `/files`.
#[test]
fn comportement_3_index_metadonnees_par_fichier_rafraichi_apres_modification() -> Result<()> {
    let vault = TempVault::new()?;
    let chemin = copier_fixture(&vault, "comportement-3-index-metadonnees-par-fichier.md")?;
    fs::write(
        vault.root.join("Autre.md"),
        "---\nrevision: 2\n---\nAutre fichier.\n",
    )?;

    // Premier index : les métadonnées sont celles de **chaque** fichier.
    let notes = scan_vault(&vault.root)?;
    assert_eq!(notes.len(), 2);
    let index = NoteIndex::build_index(&notes);
    let suivi = note_par_nom(&notes, "comportement-3-index-metadonnees-par-fichier")?;
    let autre = note_par_nom(&notes, "Autre")?;
    assert_eq!(
        suivi
            .metadata
            .frontmatter
            .as_ref()
            .and_then(|fm| fm["revision"].as_i64()),
        Some(1)
    );
    assert_eq!(
        autre
            .metadata
            .frontmatter
            .as_ref()
            .and_then(|fm| fm["revision"].as_i64()),
        Some(2)
    );

    // Modification du fichier : le fichier « Nouveau » n'existe pas encore,
    // l'index courant ne le connaît pas.
    assert!(index.resolve("Nouveau").is_empty());

    fs::write(
        &chemin,
        "---\nrevision: 3\n---\nDeuxième version, lien vers [[Nouveau]].\n",
    )?;
    fs::write(vault.root.join("Nouveau.md"), "# Nouveau\n")?;

    // Nouvel index : il reflète l'état courant des fichiers.
    let notes = scan_vault(&vault.root)?;
    assert_eq!(notes.len(), 3);
    let index = NoteIndex::build_index(&notes);
    let suivi = note_par_nom(&notes, "comportement-3-index-metadonnees-par-fichier")?;
    assert_eq!(
        suivi
            .metadata
            .frontmatter
            .as_ref()
            .and_then(|fm| fm["revision"].as_i64()),
        Some(3)
    );
    // Le lien ajouté dans le fichier modifié n'est résolu que par l'index
    // rafraîchi.
    assert_eq!(index.outgoing(suivi), vec![Some("Nouveau")]);
    assert_eq!(index.resolve("Nouveau").len(), 1);
    // Les métadonnées de l'autre fichier sont inchangées (index par fichier).
    let autre = note_par_nom(&notes, "Autre")?;
    assert_eq!(
        autre
            .metadata
            .frontmatter
            .as_ref()
            .and_then(|fm| fm["revision"].as_i64()),
        Some(2)
    );

    Ok(())
}

/// **Comportement 4** — ANALYSE.md §« Comportements déduits » (résolution de
/// lien : linkpath + subpath (ancre) → destination ; chemin de revers
/// fichier → texte de lien ; « file or folder not found » distinct).
///
/// Déduit de l'analyse statique, **pas observé** à l'exécution.
///
/// ⚠ **ÉCART CONNU** : `src/vault/` n'expose ni la découpe linkpath/subpath
/// ni un message d'erreur « file or folder not found ». Le lien parsé garde
/// la cible complète (ancre comprise) et l'index apparie le nom complet ;
/// l'absence de cible est un **état distinct** (`None`), pas une erreur.
#[test]
fn comportement_4_resolution_lien_chemin_de_revers_et_cible_absente() -> Result<()> {
    let vault = TempVault::new()?;
    copier_fixture(&vault, "comportement-4-liens-cible.md")?;
    let chemin_source = copier_fixture(&vault, "comportement-4-liens-source.md")?;

    // Les liens sont extraits avec leur cible (linkpath + subpath) et leur
    // alias (texte de lien affiché).
    let source = fs::read_to_string(&chemin_source)?;
    let liens = parse_wikilinks(&source);
    assert_eq!(liens.len(), 3);
    assert_eq!(liens[0].target, "comportement-4-liens-cible#Ancre");
    assert_eq!(liens[1].target, "comportement-4-liens-cible");
    assert_eq!(liens[1].alias.as_deref(), Some("alias"));
    assert_eq!(liens[2].target, "note-inexistante");

    let notes = scan_vault(&vault.root)?;
    let index = NoteIndex::build_index(&notes);
    let note_source = note_par_nom(&notes, "comportement-4-liens-source")?;
    let note_cible = note_par_nom(&notes, "comportement-4-liens-cible")?;

    // **linkpath → destination** : la partie avant l'ancre désigne le fichier
    // destination. (La découpe est faite par l'appelant — le moteur fait de
    // même dans `Engine::resolve_link` ; `src/vault/` ne l'expose pas encore.)
    let (linkpath, subpath) = liens[0]
        .target
        .split_once('#')
        .ok_or_else(|| anyhow!("subpath absent de la cible du lien"))?;
    let destinations = index.resolve(linkpath);
    assert_eq!(destinations.len(), 1);
    assert_eq!(destinations[0].name, "comportement-4-liens-cible");

    // **subpath (ancre) → destination** : l'ancre désigne un titre de la note
    // destination.
    let contenu_cible = fs::read_to_string(vault.root.join("comportement-4-liens-cible.md"))?;
    let plan = extract_outline(&contenu_cible);
    let titres: Vec<&str> = plan.iter().map(|entree| entree.title.as_str()).collect();
    assert!(
        titres.contains(&subpath),
        "ancre « {subpath} » absente du plan de la destination"
    );

    // **Chemin de revers** : la destination connaît la note qui pointe vers
    // elle.
    let backlinks = index.backlinks(note_cible);
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0].name, "comportement-4-liens-source");

    // **« file or folder not found »** : état distinct et identifiable — une
    // cible absente ne se confond jamais avec une cible résolue.
    let sortants = index.outgoing(note_source);
    assert_eq!(
        sortants,
        vec![
            None, // cible avec ancre : l'index du vault apparie le nom complet
            Some("comportement-4-liens-cible"),
            None, // « note-inexistante » : fichier/dossier non trouvé
        ]
    );
    assert!(index.resolve("note-inexistante").is_empty());

    Ok(())
}

/// **Comportement 4 (suite)** — ANALYSE.md §« Comportements déduits » (le
/// subpath porte l'ancre : `[[cible#Ancre]]` pointe la note `cible`).
///
/// Déduit de l'analyse statique, **pas observé** à l'exécution.
///
/// La découpe linkpath/subpath n'est pas faite par `src/vault/` : elle est
/// faite par l'appelant. On l'exerce ici par l'API publique du crate
/// (`Engine::resolve_link`), qui s'appuie sur l'index du vault. Écart connu
/// pour `src/vault/` seul : `NoteIndex::resolve` apparie le nom complet,
/// ancre comprise (cf. le test précédent).
#[test]
fn comportement_4_subpath_ancre_vers_destination_via_resolution_appelant() -> Result<()> {
    let vault = TempVault::new()?;
    copier_fixture(&vault, "comportement-4-liens-cible.md")?;
    copier_fixture(&vault, "comportement-4-liens-source.md")?;

    let engine = Engine::open(&vault.root)?;
    let source = engine
        .find_note(Path::new("comportement-4-liens-source.md"))
        .ok_or_else(|| anyhow!("note source absente du moteur"))?;

    // linkpath + subpath → la note destination (l'ancre ne change pas la cible
    // du fichier, elle désigne un titre à l'intérieur).
    let destination = engine
        .resolve_link(Some(source), "comportement-4-liens-cible#Ancre")
        .ok_or_else(|| anyhow!("lien avec ancre non résolu"))?;
    assert_eq!(destination.name, "comportement-4-liens-cible");

    // Même cible sans ancre, et avec alias : même destination.
    let destination_alias = engine
        .resolve_link(Some(source), "comportement-4-liens-cible")
        .ok_or_else(|| anyhow!("lien sans ancre non résolu"))?;
    assert_eq!(destination_alias.name, "comportement-4-liens-cible");

    // La cible absente reste distincte : aucune destination.
    assert!(engine
        .resolve_link(Some(source), "note-inexistante")
        .is_none());

    Ok(())
}
