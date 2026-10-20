//! Édition batch de propriétés frontmatter par-dessus `vault::meta`.
//!
//! [`apply_edits`] applique les éditions **séquentiellement**, en déléguant
//! chaque opération à `meta::set_frontmatter_property` ou
//! `meta::remove_frontmatter_property`. Chaque édition réussie est écrite sur
//! disque (fichier temporaire + `rename` atomique via `ops::write_note`) avant
//! que la suivante ne soit tentée.
//!
//! # Absence de transaction
//!
//! Le lot n'est pas atomique : au premier edit invalide, l'erreur est
//! retournée et les edits suivants ne sont pas exécutés, mais les edits déjà
//! appliqués **ne sont pas annulés** (aucun rollback). L'appelant qui a besoin
//! d'un tout-ou-rien doit relire l'état avant de réessayer, ou appliquer une
//! seule édition à la fois.
//!
//! Une liste d'éditions vide ne lit ni ne réécrit la note : elle réussit
//! toujours, sans même exiger que le chemin existe.

use std::path::Path;

use anyhow::{Context, Result};

use super::meta::{remove_frontmatter_property, set_frontmatter_property};

/// Édition unitaire d'une propriété frontmatter.
#[derive(Debug, Clone)]
pub enum PropertyEdit {
    /// Affecte `key` : la clé est créée si elle est absente, remplacée sinon.
    Set {
        /// Nom de la propriété.
        key: String,
        /// Valeur YAML (scalaire, séquence ou mapping).
        value: serde_yaml::Value,
    },
    /// Supprime `key` du frontmatter. Une clé absente n'est pas une erreur.
    Remove {
        /// Nom de la propriété à supprimer.
        key: String,
    },
}

/// Applique `edits` à la note `rel_path` sous `root`, dans l'ordre donné.
///
/// S'arrête au premier edit en erreur (les edits précédents restent écrits,
/// voir l'avertissement du module). Les modifications de clés existantes
/// préservent leur position ; les nouvelles clés sont ajoutées en fin de bloc.
///
/// # Erreurs
///
/// - `root` n'est pas un répertoire, ou `rel_path` est vide, absolu ou
///   contient un composant `..` ;
/// - la note est illisible ;
/// - le frontmatter existant n'est pas un YAML valide ou n'est pas un mapping ;
/// - une écriture atomique échoue.
pub fn apply_edits(root: &Path, rel_path: &Path, edits: &[PropertyEdit]) -> Result<()> {
    for (index, edit) in edits.iter().enumerate() {
        let position = index + 1;
        match edit {
            PropertyEdit::Set { key, value } => {
                set_frontmatter_property(root, rel_path, key, value).with_context(|| {
                    format!("edit {position} (set « {key} ») de {}", rel_path.display())
                })?;
            }
            PropertyEdit::Remove { key } => {
                remove_frontmatter_property(root, rel_path, key).with_context(|| {
                    format!(
                        "edit {position} (remove « {key} ») de {}",
                        rel_path.display()
                    )
                })?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_yaml::{Mapping, Value};

    use super::*;
    use crate::vault::meta::read_frontmatter;

    /// Vault éphémère, supprimé à la chute du scope.
    struct TempVault {
        path: PathBuf,
    }

    impl TempVault {
        fn new() -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let count = COUNTER.fetch_add(1, Ordering::SeqCst);
            // Sous `target/` plutôt que dans `/tmp` : le dossier de build est
            // déjà inscriptible et évite les quotas d'un tmpfs partagé.
            let base = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join("test-tmp");
            let path = base.join(format!(
                "obsidian-rs-properties-{}-{}",
                std::process::id(),
                count
            ));
            fs::create_dir_all(&path).expect("création du vault temporaire");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write(&self, rel_path: &str, content: &str) {
            let target = self.path.join(rel_path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).expect("création des dossiers intermédiaires");
            }
            fs::write(&target, content).expect("écriture de la note");
        }

        fn raw(&self, rel_path: &str) -> String {
            fs::read_to_string(self.path.join(rel_path)).expect("lecture du contenu brut")
        }

        fn frontmatter(&self, rel_path: &str) -> Option<Value> {
            read_frontmatter(&self.path, Path::new(rel_path)).expect("lecture du frontmatter")
        }
    }

    impl Drop for TempVault {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn set(key: &str, value: impl Into<Value>) -> PropertyEdit {
        PropertyEdit::Set {
            key: key.to_string(),
            value: value.into(),
        }
    }

    fn remove(key: &str) -> PropertyEdit {
        PropertyEdit::Remove {
            key: key.to_string(),
        }
    }

    fn mapping_frontmatter(fm: Option<Value>) -> Mapping {
        fm.expect("frontmatter présent")
            .as_mapping()
            .expect("frontmatter mapping")
            .clone()
    }

    #[test]
    fn set_et_remove_combines_dans_un_meme_lot() {
        let vault = TempVault::new();
        vault.write(
            "note.md",
            "---\ntitle: ancien\nstatus: draft\n---\n# Corps\n\nTexte.\n",
        );

        apply_edits(
            vault.path(),
            Path::new("note.md"),
            &[
                set("title", "Nouveau titre"),
                remove("status"),
                set("tags", Value::Sequence(vec!["rust".into(), "test".into()])),
            ],
        )
        .expect("application du lot");

        let map = mapping_frontmatter(vault.frontmatter("note.md"));
        assert_eq!(map.get("title"), Some(&"Nouveau titre".into()));
        assert!(!map.contains_key("status"));
        assert_eq!(
            map.get("tags"),
            Some(&Value::Sequence(vec!["rust".into(), "test".into()]))
        );
        // Le corps Markdown est intact.
        assert!(vault.raw("note.md").ends_with("# Corps\n\nTexte.\n"));
    }

    #[test]
    fn plusieurs_set_preservent_l_ordre_et_remplacent_en_place() {
        let vault = TempVault::new();
        vault.write("note.md", "---\nalpha: 1\n---\ncorps\n");

        apply_edits(
            vault.path(),
            Path::new("note.md"),
            &[set("beta", "2"), set("gamma", "3"), set("alpha", "10")],
        )
        .expect("application du lot");

        let map = mapping_frontmatter(vault.frontmatter("note.md"));
        let keys: Vec<Option<&str>> = map.keys().map(Value::as_str).collect();
        // Les nouvelles clés sont ajoutées à la fin, `alpha` garde sa position.
        assert_eq!(keys, vec![Some("alpha"), Some("beta"), Some("gamma")]);
        assert_eq!(map.get("alpha"), Some(&"10".into()));
        assert_eq!(map.get("beta"), Some(&"2".into()));
    }

    #[test]
    fn sans_frontmatter_en_cree_un_et_preserve_le_corps() {
        let vault = TempVault::new();
        vault.write("note.md", "# Note sans frontmatter\n\nParagraphe.\n");
        assert!(vault.frontmatter("note.md").is_none());

        apply_edits(
            vault.path(),
            Path::new("note.md"),
            &[set("title", "Créé"), set("draft", true)],
        )
        .expect("application du lot");

        let map = mapping_frontmatter(vault.frontmatter("note.md"));
        assert_eq!(map.get("title"), Some(&"Créé".into()));
        assert_eq!(map.get("draft"), Some(&Value::Bool(true)));
        let raw = vault.raw("note.md");
        assert!(raw.starts_with("---\n"));
        assert!(raw.contains("title: Créé\n"));
        assert!(raw.ends_with("---\n# Note sans frontmatter\n\nParagraphe.\n"));
    }

    #[test]
    fn yaml_invalide_echoue_sans_modification_du_fichier() {
        let vault = TempVault::new();
        // Séquence YAML non fermée : le parseur du frontmatter renvoie une erreur.
        vault.write("note.md", "---\ntitre: [non fermé\n---\ncorps\n");
        let avant = vault.raw("note.md");

        let err = apply_edits(
            vault.path(),
            Path::new("note.md"),
            &[set("alpha", "1"), set("beta", "2")],
        );

        assert!(err.is_err(), "un YAML invalide doit échouer");
        // Aucun edit n'a été écrit : pas de modification partielle.
        assert_eq!(vault.raw("note.md"), avant);
    }

    #[test]
    fn frontmatter_non_mapping_rejete_sans_modification() {
        let vault = TempVault::new();
        // YAML valide mais racine non mapping : `meta` refuse l'édition.
        vault.write("note.md", "---\n- liste\n- sans cle\n---\ncorps\n");
        let avant = vault.raw("note.md");

        let err = apply_edits(vault.path(), Path::new("note.md"), &[set("alpha", "1")]);

        assert!(err.is_err(), "un frontmatter non mapping doit échouer");
        assert_eq!(vault.raw("note.md"), avant);
    }

    #[test]
    fn listes_et_mappings_imbriques_sont_relus_a_l_identique() {
        let vault = TempVault::new();
        vault.write("note.md", "---\nalpha: 1\n---\ncorps\n");

        let mut lien = Mapping::new();
        lien.insert("url".into(), "https://exemple.test".into());
        lien.insert("label".into(), "Exemple".into());
        let liens = Value::Sequence(vec![Value::Mapping(lien)]);
        let tags = Value::Sequence(vec!["a".into(), "b".into()]);

        apply_edits(
            vault.path(),
            Path::new("note.md"),
            &[set("tags", tags.clone()), set("liens", liens.clone())],
        )
        .expect("application du lot");

        let map = mapping_frontmatter(vault.frontmatter("note.md"));
        assert_eq!(map.get("tags"), Some(&tags));
        assert_eq!(map.get("liens"), Some(&liens));
        // Sérialisation lisible en liste YAML, pas en JSON inline.
        let raw = vault.raw("note.md");
        assert!(raw.contains("tags:\n- a\n- b\n"), "sortie YAML : {raw}");
        assert!(raw.contains("- url: https://exemple.test"));
    }

    #[test]
    fn relecture_reflete_les_edits_et_reapplication_idempotente() {
        let vault = TempVault::new();
        vault.write("note.md", "---\nstatut: brouillon\n---\ncorps\n");
        let edits = [
            set("titre", "Final"),
            set("statut", "publié"),
            remove("absent"),
        ];

        apply_edits(vault.path(), Path::new("note.md"), &edits).expect("premier lot");
        let map = mapping_frontmatter(vault.frontmatter("note.md"));
        assert_eq!(map.get("titre"), Some(&"Final".into()));
        assert_eq!(map.get("statut"), Some(&"publié".into()));

        // Relecture via `read_frontmatter` puis réapplication : aucun changement,
        // donc aucune réécriture du fichier.
        let apres_premier = vault.raw("note.md");
        apply_edits(vault.path(), Path::new("note.md"), &edits).expect("second lot");
        assert_eq!(vault.raw("note.md"), apres_premier);
    }

    #[test]
    fn remove_d_une_cle_absente_reussit_sans_reecrire() {
        let vault = TempVault::new();
        vault.write("note.md", "---\nalpha: 1\n---\ncorps\n");
        let avant = vault.raw("note.md");

        apply_edits(vault.path(), Path::new("note.md"), &[remove("inexistante")])
            .expect("suppression d'une clé absente");

        assert_eq!(vault.raw("note.md"), avant);
        let map = mapping_frontmatter(vault.frontmatter("note.md"));
        // `alpha: 1` est un nombre YAML, pas une chaîne.
        assert_eq!(map.get("alpha"), Some(&Value::Number(1.into())));
    }

    #[test]
    fn lot_vide_ne_touche_ni_au_disque_ni_au_chemin() {
        let vault = TempVault::new();
        // Ni la note ni la racine ne sont validées sans edit à appliquer.
        apply_edits(vault.path(), Path::new("absente.md"), &[]).expect("lot vide");
        assert!(!vault.path().join("absente.md").exists());

        let racine_absente = vault.path().join("racine-absente");
        apply_edits(&racine_absente, Path::new("note.md"), &[]).expect("lot vide");
        assert!(!racine_absente.exists());
    }

    #[test]
    fn chemin_hors_vault_ou_racine_invalide_rejetes() {
        let vault = TempVault::new();
        vault.write("note.md", "---\na: b\n---\n");

        let err = apply_edits(
            vault.path(),
            Path::new("../hors-vault.md"),
            &[set("x", "y")],
        );
        assert!(
            err.is_err(),
            "un chemin remontant hors du vault doit échouer"
        );

        let racine_absente = vault.path().join("inexistante");
        let err = apply_edits(&racine_absente, Path::new("note.md"), &[set("x", "y")]);
        assert!(err.is_err(), "une racine absente doit échouer");

        // Le chemin relatif absolu est également refusé.
        let err = apply_edits(vault.path(), Path::new("/etc/passwd"), &[set("x", "y")]);
        assert!(err.is_err(), "un chemin absolu doit échouer");
    }
}
