//! Notes journalières (Daily Notes) : nom de fichier, contenu et création.
//!
//! Une note journalière porte le nom `AAAA-MM-JJ.md`, au format de la date
//! demandée. Le contenu par défaut est un gabarit minimal (titre daté +
//! section « Tâches ») que l'appelant peut remplacer par un gabarit `{{...}}`.

use std::fs;
use std::path::{Path, PathBuf};

use super::ops::create_note;
use super::templates::{default_variables, expand_template};

/// Erreurs de création de note journalière.
#[derive(Debug, thiserror::Error)]
pub enum DailyNotesError {
    /// Le format de la date n'est pas `AAAA-MM-JJ`.
    #[error("format de date invalide : {0}")]
    InvalidDate(String),
    /// Une note pour cette date existe déjà dans le vault.
    #[error("le fichier existe déjà : {0}")]
    Exists(String),
    /// Échec de l'écriture atomique (racine, dossiers, fichiers).
    #[error("écriture impossible : {0}")]
    Write(String),
}

/// Gabarit par défaut d'une note journalière.
const DEFAULT_DAILY_TEMPLATE: &str = "# {{date}}\n\n## Tâches\n\n- [ ] ";

/// `true` si `date` est exactement au format `AAAA-MM-JJ` (10 caractères,
/// chiffres aux bonnes positions, tirets aux positions 5 et 8).
fn is_daily_date_format(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10 {
        return false;
    }
    let chiffres = [0, 1, 2, 3, 5, 6, 8, 9]
        .iter()
        .all(|i| bytes[*i].is_ascii_digit());
    chiffres && bytes[4] == b'-' && bytes[7] == b'-'
}

/// Nom de fichier pour une date formatée "YYYY-MM-DD" (`None` si le format est
/// invalide). Le nom est `AAAA-MM-JJ.md`.
pub fn daily_filename(date: &str) -> Option<String> {
    if !is_daily_date_format(date) {
        return None;
    }
    Some(format!("{date}.md"))
}

/// Construit le contenu d'une note journalière (template par défaut +
/// variables).
///
/// Sans gabarit (ou avec un gabarit vide), le contenu est le gabarit par
/// défaut : `# AAAA-MM-JJ` + ligne vide + `## Tâches` + ligne vide + `- [ ] `.
/// Si un `tpl` non vide est fourni, il est expansé avec `{{title}}` = date et
/// `{{date}}` = date.
pub fn daily_content(date: &str, tpl: Option<&str>) -> String {
    let gabarit = match tpl {
        Some(t) if !t.trim().is_empty() => t,
        _ => DEFAULT_DAILY_TEMPLATE,
    };
    expand_template(gabarit, &default_variables(date, date))
}

/// Crée la note journalière dans le vault (réutilise l'écriture atomique
/// d'`ops`). Retourne le chemin relatif créé sous la racine.
///
/// - Date invalide → `Err(InvalidDate)`, rien n'est écrit.
/// - Le fichier existe déjà → `Err(Exists)`.
/// - Échec d'écriture → `Err(Write)`.
pub fn create_daily_note(
    vault_root: &Path,
    date: &str,
    tpl: Option<&str>,
) -> Result<PathBuf, DailyNotesError> {
    let nom = daily_filename(date).ok_or_else(|| DailyNotesError::InvalidDate(date.to_string()))?;
    let rel = PathBuf::from(nom.clone());
    let contenu = daily_content(date, tpl);
    let cible = vault_root.join(&rel);
    // `symlink_metadata` refuse aussi les liens symboliques morts. Le résidu
    // TOCTOU (fichier apparu entre la vérification et le `rename`) est
    // accepté, comme dans `ops::create_note`.
    if fs::symlink_metadata(&cible).is_ok() {
        return Err(DailyNotesError::Exists(nom));
    }
    create_note(vault_root, &rel, &contenu)
        .map_err(|err| DailyNotesError::Write(err.to_string()))?;
    Ok(rel)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use DailyNotesError::{Exists, InvalidDate, Write};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// Vault éphémère, supprimé à la fin du scope.
    struct TempVault(PathBuf);

    impl TempVault {
        fn new() -> Self {
            let count = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "obsidian-rs-daily-notes-{}-{}",
                std::process::id(),
                count
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("création du vault temporaire");
            TempVault(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempVault {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const CONTENU_DEFAUT: &str = "# 2026-10-20\n\n## Tâches\n\n- [ ] ";

    #[test]
    fn nom_de_fichier_valide() {
        assert_eq!(
            daily_filename("2026-10-20"),
            Some("2026-10-20.md".to_string())
        );
    }

    #[test]
    fn nom_de_fichier_rejette_un_format_invalide() {
        for date in [
            "2026-10-2",
            "2026-10",
            "2026/10/20",
            "2026-10-201",
            "abcd-10-20",
            "2026-1o-20",
            "",
        ] {
            assert_eq!(daily_filename(date), None, "date : {date}");
        }
    }

    #[test]
    fn contenu_par_defaut_exact() {
        assert_eq!(daily_content("2026-10-20", None), CONTENU_DEFAUT);
    }

    #[test]
    fn contenu_avec_template_expand_date_et_title() {
        let tpl = "Jour : {{date}}\nTitre : {{title}}\n{{inconnu}}";
        assert_eq!(
            daily_content("2026-10-20", Some(tpl)),
            "Jour : 2026-10-20\nTitre : 2026-10-20\n{{inconnu}}"
        );
    }

    #[test]
    fn template_vide_tombe_sur_le_contenu_par_defaut() {
        assert_eq!(daily_content("2026-10-20", Some("")), CONTENU_DEFAUT);
        assert_eq!(daily_content("2026-10-20", Some("   ")), CONTENU_DEFAUT);
    }

    #[test]
    fn creation_cree_le_fichier_sous_le_vault() {
        let vault = TempVault::new();
        let rel = create_daily_note(vault.path(), "2026-10-20", None)
            .expect("création de la note journalière");
        let fichier = vault.path().join(&rel);
        assert!(fichier.is_file(), "fichier créé : {fichier:?}");
        assert_eq!(
            fs::read_to_string(&fichier).expect("lecture de la note"),
            CONTENU_DEFAUT
        );
    }

    #[test]
    fn chemin_retourne_est_le_chemin_relatif() {
        let vault = TempVault::new();
        let rel = create_daily_note(vault.path(), "2026-10-20", None)
            .expect("création de la note journalière");
        assert_eq!(rel, PathBuf::from("2026-10-20.md"));
        assert!(vault.path().join(&rel).is_file());
    }

    #[test]
    fn creation_refuse_un_fichier_existant() {
        let vault = TempVault::new();
        create_daily_note(vault.path(), "2026-10-20", None).expect("première création");
        let erreur = create_daily_note(vault.path(), "2026-10-20", None)
            .expect_err("le second appel doit être refusé");
        assert!(matches!(erreur, Exists(_)), "erreur : {erreur:?}");
    }

    #[test]
    fn idempotence_deuxieme_appel_refuse() {
        let vault = TempVault::new();
        let premier =
            create_daily_note(vault.path(), "2026-11-01", None).expect("première création");
        let second = create_daily_note(vault.path(), "2026-11-01", None)
            .expect_err("la seconde création doit être refusée");
        assert!(matches!(second, Exists(_)), "erreur : {second:?}");
        // Le premier fichier n'est pas modifié.
        assert_eq!(
            fs::read_to_string(vault.path().join(&premier)).expect("lecture"),
            "# 2026-11-01\n\n## Tâches\n\n- [ ] "
        );
    }

    #[test]
    fn deux_dates_distinctes_cree_deux_notes() {
        let vault = TempVault::new();
        let un = create_daily_note(vault.path(), "2026-10-20", None).expect("première note");
        let deux = create_daily_note(vault.path(), "2026-10-21", None).expect("seconde note");
        assert_ne!(un, deux);
        assert!(vault.path().join(&un).is_file());
        assert!(vault.path().join(&deux).is_file());
    }

    #[test]
    fn date_invalide_refusee_sans_ecriture() {
        let vault = TempVault::new();
        let erreur =
            create_daily_note(vault.path(), "20/10/2026", None).expect_err("date invalide");
        assert!(matches!(erreur, InvalidDate(_)), "erreur : {erreur:?}",);
        let entrees: Vec<_> = fs::read_dir(vault.path())
            .expect("lecture du vault")
            .collect();
        assert!(entrees.is_empty(), "aucun fichier ne doit être créé");
    }

    #[test]
    fn contenu_ecrit_reflete_le_template() {
        let vault = TempVault::new();
        let rel = create_daily_note(vault.path(), "2026-12-25", Some("{{date}} !"))
            .expect("création avec gabarit");
        assert_eq!(
            fs::read_to_string(vault.path().join(&rel)).expect("lecture"),
            "2026-12-25 !"
        );
    }

    #[test]
    fn racine_absente_refuse_l_ecriture() {
        let absent = std::env::temp_dir().join("obsidian-rs-dn-racine-absente");
        let _ = fs::remove_dir_all(&absent);
        let erreur = create_daily_note(&absent, "2026-10-20", None).expect_err("racine absente");
        assert!(matches!(erreur, Write(_)), "erreur : {erreur:?}");
        assert!(!absent.join("2026-10-20.md").exists(), "aucun fichier créé");
    }
}
