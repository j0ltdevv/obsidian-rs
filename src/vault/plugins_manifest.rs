//! Manifests de plugins Obsidian (`.obsidian/plugins/<id>/manifest.json`).
//!
//! Ce module est un module vault pur : il lit et valide les manifests à titre de
//! *données* uniquement. Il ne charge ni n'exécute aucun code (le champ `main`
//! n'est jamais traité comme exécutable), et il ne dépend d'aucun composant
//! iced.

use std::fs;
use std::path::{Path, PathBuf};

/// Manifest d'un plugin Obsidian, tel que déclaré dans
/// `.obsidian/plugins/<id>/manifest.json`.
///
/// Seuls `id`, `name` et `version` sont requis ; tous les autres champs sont
/// optionnels et valent `None` lorsqu'ils sont absents du fichier.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PluginManifest {
    /// Identifiant stable du plugin (clé du dossier de plugin).
    pub id: String,
    /// Libellé humain du plugin.
    pub name: String,
    /// Version du plugin (libre, par exemple `1.2.3`).
    pub version: String,
    /// Description courte du plugin.
    #[serde(default)]
    pub description: Option<String>,
    /// Auteur ou autrice du plugin.
    #[serde(default)]
    pub author: Option<String>,
    /// Version minimale d'Obsidian requise (champ `minAppVersion`).
    #[serde(default, rename = "minAppVersion")]
    pub min_app_version: Option<String>,
    /// `true` si le plugin ne fonctionne que sur les clients de bureau
    /// (champ `isDesktopOnly`).
    #[serde(default, rename = "isDesktopOnly")]
    pub is_desktop_only: Option<bool>,
    /// Nom du fichier principal du plugin (jamais exécuté ici).
    #[serde(default)]
    pub main: Option<String>,
}

/// Erreurs de lecture et de validation d'un manifest de plugin.
#[derive(Debug, thiserror::Error)]
pub enum PluginManifestError {
    /// Échec d'E/S (lecture d'un dossier ou d'un fichier).
    #[error("erreur d'E/S : {0}")]
    Io(#[from] std::io::Error),
    /// JSON invalide ou ne correspondant pas à la forme attendue.
    #[error("JSON de manifest invalide : {0}")]
    Json(#[from] serde_json::Error),
    /// Champs requis manquants ou vides (ou manifest absent), avec le détail.
    #[error("champs manquants ou vides : {0:?}")]
    ChampsManquants(Vec<String>),
    /// Dossier de plugin invalide (par exemple identifiant illégal).
    #[error("dossier invalide : {0}")]
    DossierInvalide(String),
}

/// Représentation intermédiaire laxiste : les champs requis sont optionnels
/// pour distinguer « absent » de « présent mais vide ».
#[derive(serde::Deserialize)]
struct RawManifest {
    id: Option<String>,
    name: Option<String>,
    version: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default, rename = "minAppVersion")]
    min_app_version: Option<String>,
    #[serde(default, rename = "isDesktopOnly")]
    is_desktop_only: Option<bool>,
    #[serde(default)]
    main: Option<String>,
}

/// Parse strictement le contenu d'un manifest de plugin.
///
/// - Le JSON doit être un objet valide ; les clés inconnues sont ignorées.
/// - `id`, `name` et `version` doivent être présents et non vides, sinon
///   [`PluginManifestError::ChampsManquants`] avec la liste de tous les champs
///   en cause.
/// - `id` ne peut contenir que des caractères alphanumériques ASCII, `_` et
///   `-`, sinon [`PluginManifestError::DossierInvalide`].
/// - Tout autre échec de désérialisation produit [`PluginManifestError::Json`].
pub fn parse_manifest(json: &str) -> Result<PluginManifest, PluginManifestError> {
    let brut: RawManifest = serde_json::from_str(json)?;

    let (id, name, version) = match (brut.id, brut.name, brut.version) {
        (Some(id), Some(name), Some(version))
            if !id.is_empty() && !name.is_empty() && !version.is_empty() =>
        {
            (id, name, version)
        }
        (id, name, version) => {
            let mut manquants = Vec::new();
            if id.as_deref().is_none_or(str::is_empty) {
                manquants.push("id".to_string());
            }
            if name.as_deref().is_none_or(str::is_empty) {
                manquants.push("name".to_string());
            }
            if version.as_deref().is_none_or(str::is_empty) {
                manquants.push("version".to_string());
            }
            return Err(PluginManifestError::ChampsManquants(manquants));
        }
    };

    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(PluginManifestError::DossierInvalide(format!(
            "id invalide : {id:?}"
        )));
    }

    Ok(PluginManifest {
        id,
        name,
        version,
        description: brut.description,
        author: brut.author,
        min_app_version: brut.min_app_version,
        is_desktop_only: brut.is_desktop_only,
        main: brut.main,
    })
}

/// Énumère les plugins d'un vault en lisant `.obsidian/plugins/*/manifest.json`.
///
/// Retourne une entrée `(chemin_relatif, manifest)` par plugin, où
/// `chemin_relatif` est relatif à la racine du vault (par exemple
/// `.obsidian/plugins/mon-plugin`), triée par nom de dossier.
///
/// - Si le dossier `.obsidian/plugins` n'existe pas, la liste est vide (ce
///   n'est pas une erreur).
/// - Les fichiers présents directement sous `.obsidian/plugins` (non-dossiers)
///   sont ignorés.
/// - Le premier problème rencontré arrête le parcours : un dossier de plugin
///   sans `manifest.json` (ou dont `manifest.json` n'est pas un fichier
///   régulier) produit [`PluginManifestError::ChampsManquants`] mentionnant le
///   dossier ; un manifest invalide propage l'erreur du parsing.
pub fn load_manifest(
    vault_root: &Path,
) -> Result<Vec<(PathBuf, PluginManifest)>, PluginManifestError> {
    let plugins_dir = vault_root.join(".obsidian").join("plugins");
    if !plugins_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entrees: Vec<_> = fs::read_dir(&plugins_dir)?.collect::<Result<_, _>>()?;
    entrees.sort_by_key(|entree| entree.file_name());

    let mut resultat = Vec::new();
    for entree in entrees {
        let dossier = entree.path();
        if !dossier.is_dir() {
            continue;
        }

        let manifest_path = dossier.join("manifest.json");
        if !manifest_path.is_file() {
            return Err(PluginManifestError::ChampsManquants(vec![format!(
                "fichier manifest.json manquant dans le dossier {}",
                dossier.display()
            )]));
        }

        let json = fs::read_to_string(&manifest_path)?;
        let manifest = parse_manifest(&json)?;

        let relatif = match dossier.strip_prefix(vault_root) {
            Ok(rel) => rel.to_path_buf(),
            Err(_) => dossier.clone(),
        };
        resultat.push((relatif, manifest));
    }

    Ok(resultat)
}

/// Chemin du dossier d'un plugin : `vault_root/.obsidian/plugins/<id>`.
///
/// Concaténation brute : `id` n'est ni validé ni nettoyé ici ; l'appelant doit
/// garantir que l'identifiant est sûr avant de l'utiliser dans un chemin.
pub fn plugin_dir(vault_root: &Path, id: &str) -> PathBuf {
    vault_root.join(".obsidian").join("plugins").join(id)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use PluginManifestError::{ChampsManquants, DossierInvalide, Json};

    static COMPTEUR: AtomicUsize = AtomicUsize::new(0);

    /// Vault éphémère, supprimé à la fin du scope.
    struct VaultTemp(PathBuf);

    impl VaultTemp {
        fn nouveau() -> Self {
            let compte = COMPTEUR.fetch_add(1, Ordering::SeqCst);
            let chemin = std::env::temp_dir().join(format!(
                "obsidian-rs-plugins-manifest-{}-{}",
                std::process::id(),
                compte
            ));
            let _ = fs::remove_dir_all(&chemin);
            fs::create_dir_all(&chemin).expect("création du vault temporaire");
            VaultTemp(chemin)
        }

        fn chemin(&self) -> &Path {
            &self.0
        }

        fn ecrit_plugin(&self, id: &str, manifest: &str) -> PathBuf {
            let dossier = self.chemin().join(".obsidian").join("plugins").join(id);
            fs::create_dir_all(&dossier).expect("création du dossier de plugin");
            fs::write(dossier.join("manifest.json"), manifest).expect("écriture du manifest");
            dossier
        }
    }

    impl Drop for VaultTemp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const MANIFEST_COMPLET: &str = r#"{
        "id": "mon-plugin",
        "name": "Mon Plugin",
        "version": "1.2.3",
        "description": "Fait des choses.",
        "author": "Moi",
        "minAppVersion": "1.0.0",
        "isDesktopOnly": true,
        "main": "main.js"
    }"#;

    #[test]
    fn manifest_complet_valide() {
        let manifest = parse_manifest(MANIFEST_COMPLET).expect("manifest valide");
        assert_eq!(manifest.id, "mon-plugin");
        assert_eq!(manifest.name, "Mon Plugin");
        assert_eq!(manifest.version, "1.2.3");
        assert_eq!(manifest.description.as_deref(), Some("Fait des choses."));
        assert_eq!(manifest.author.as_deref(), Some("Moi"));
        assert_eq!(manifest.min_app_version.as_deref(), Some("1.0.0"));
        assert_eq!(manifest.is_desktop_only, Some(true));
        assert_eq!(manifest.main.as_deref(), Some("main.js"));
    }

    #[test]
    fn manifest_minimal_valide() {
        let manifest = parse_manifest(r#"{"id":"simple","name":"Simple","version":"0.1.0"}"#)
            .expect("manifest minimal valide");
        assert_eq!(manifest.id, "simple");
        assert_eq!(manifest.name, "Simple");
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.description, None);
        assert_eq!(manifest.author, None);
        assert_eq!(manifest.min_app_version, None);
        assert_eq!(manifest.is_desktop_only, None);
        assert_eq!(manifest.main, None);
    }

    #[test]
    fn champ_requis_manquant() {
        let erreur = parse_manifest(r#"{"id":"p","version":"1"}"#).expect_err("name manquant");
        match erreur {
            ChampsManquants(champs) => assert_eq!(champs, vec!["name"]),
            autre => panic!("erreur inattendue : {autre:?}"),
        }
    }

    #[test]
    fn champ_requis_vide() {
        let cas = [
            (r#"{"id":"","name":"P","version":"1"}"#, "id"),
            (r#"{"id":"p","name":"","version":"1"}"#, "name"),
            (r#"{"id":"p","name":"P","version":""}"#, "version"),
        ];
        for (json, attendu) in cas {
            let erreur = parse_manifest(json).expect_err("champ requis vide");
            match erreur {
                ChampsManquants(champs) => assert_eq!(champs, vec![attendu]),
                autre => panic!("erreur inattendue : {autre:?}"),
            }
        }
    }

    #[test]
    fn plusieurs_champs_requis_manquants() {
        let erreur = parse_manifest(r#"{"id":"p"}"#).expect_err("name et version manquants");
        match erreur {
            ChampsManquants(champs) => assert_eq!(champs, vec!["name", "version"]),
            autre => panic!("erreur inattendue : {autre:?}"),
        }

        let erreur = parse_manifest(r#"{}"#).expect_err("tous les champs manquants");
        match erreur {
            ChampsManquants(champs) => assert_eq!(champs, vec!["id", "name", "version"]),
            autre => panic!("erreur inattendue : {autre:?}"),
        }
    }

    #[test]
    fn id_invalide_rejette() {
        for id in [
            "mon plugin",
            "mon-plugin!",
            "mon-plugin€",
            "a/b",
            "a\nb",
            "é",
        ] {
            let json = format!(r#"{{"id":{id:?},"name":"P","version":"1"}}"#);
            let erreur = parse_manifest(&json).expect_err("id invalide");
            assert!(
                matches!(erreur, DossierInvalide(_)),
                "id {id:?} → erreur : {erreur:?}"
            );
        }
    }

    #[test]
    fn id_avec_tiret_et_underscore_valide() {
        let manifest = parse_manifest(r#"{"id":"mon-plugin_2","name":"P","version":"1"}"#)
            .expect("id avec tiret et underscore");
        assert_eq!(manifest.id, "mon-plugin_2");
    }

    #[test]
    fn json_invalide() {
        for json in ["", "pas du json", "[1, 2]", r#"{"id": 42}"#, "null"] {
            let erreur = parse_manifest(json).expect_err("json invalide");
            assert!(
                matches!(erreur, Json(_)),
                "json {json:?} → erreur : {erreur:?}"
            );
        }
    }

    #[test]
    fn renommage_champs_obsidian() {
        let manifest = parse_manifest(
            r#"{"id":"p","name":"P","version":"1","minAppVersion":"0.9.0","isDesktopOnly":false}"#,
        )
        .expect("champs renommés");
        assert_eq!(manifest.min_app_version.as_deref(), Some("0.9.0"));
        assert_eq!(manifest.is_desktop_only, Some(false));
    }

    #[test]
    fn serialisation_deserialisation_circulaire() {
        let manifest = parse_manifest(MANIFEST_COMPLET).expect("manifest valide");
        let json = serde_json::to_string(&manifest).expect("sérialisation");
        let relu = parse_manifest(&json).expect("re-parsing");
        assert_eq!(manifest, relu);
    }

    #[test]
    fn load_manifest_dossier_plugins_absent() {
        let vault = VaultTemp::nouveau();
        let plugins = load_manifest(vault.chemin()).expect("liste vide");
        assert!(plugins.is_empty());
    }

    #[test]
    fn load_manifest_un_plugin() {
        let vault = VaultTemp::nouveau();
        vault.ecrit_plugin("mon-plugin", MANIFEST_COMPLET);

        let plugins = load_manifest(vault.chemin()).expect("lecture du manifest");
        assert_eq!(plugins.len(), 1);
        let (relatif, manifest) = &plugins[0];
        assert_eq!(*relatif, PathBuf::from(".obsidian/plugins/mon-plugin"));
        assert_eq!(manifest.id, "mon-plugin");
    }

    #[test]
    fn load_manifest_plusieurs_plugins_tries() {
        let vault = VaultTemp::nouveau();
        vault.ecrit_plugin("zeta", r#"{"id":"zeta","name":"Z","version":"1"}"#);
        vault.ecrit_plugin("alpha", r#"{"id":"alpha","name":"A","version":"1"}"#);

        let plugins = load_manifest(vault.chemin()).expect("lecture des manifests");
        let ids: Vec<_> = plugins.iter().map(|(_, m)| m.id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "zeta"]);
    }

    #[test]
    fn load_manifest_dossier_sans_manifest() {
        let vault = VaultTemp::nouveau();
        let dossier = vault
            .chemin()
            .join(".obsidian")
            .join("plugins")
            .join("orphelin");
        fs::create_dir_all(&dossier).expect("création du dossier orphelin");

        let erreur = load_manifest(vault.chemin()).expect_err("manifest manquant");
        match erreur {
            ChampsManquants(detail) => {
                assert!(
                    detail.iter().any(|d| d.contains("orphelin")),
                    "détail : {detail:?}"
                );
            }
            autre => panic!("erreur inattendue : {autre:?}"),
        }
    }

    #[test]
    fn load_manifest_manifest_est_un_dossier() {
        let vault = VaultTemp::nouveau();
        let dossier = vault
            .chemin()
            .join(".obsidian")
            .join("plugins")
            .join("etrange");
        fs::create_dir_all(dossier.join("manifest.json"))
            .expect("création du dossier manifest.json");

        let erreur = load_manifest(vault.chemin()).expect_err("manifest non-fichier");
        assert!(matches!(erreur, ChampsManquants(_)), "erreur : {erreur:?}");
    }

    #[test]
    fn load_manifest_ignore_les_fichiers_directs() {
        let vault = VaultTemp::nouveau();
        let plugins_dir = vault.chemin().join(".obsidian").join("plugins");
        fs::create_dir_all(&plugins_dir).expect("création de plugins/");
        fs::write(plugins_dir.join("notes.txt"), "rien").expect("écriture du fichier parasite");
        fs::write(plugins_dir.join("manifest.json"), "{}").expect("écriture du manifest parasite");

        let plugins = load_manifest(vault.chemin()).expect("aucun plugin");
        assert!(plugins.is_empty());
    }

    #[test]
    fn load_manifest_manifest_invalide() {
        let vault = VaultTemp::nouveau();
        vault.ecrit_plugin("casse", r#"{"id":"casse","name":"C"}"#); // version manquante

        let erreur = load_manifest(vault.chemin()).expect_err("manifest invalide");
        assert!(matches!(erreur, ChampsManquants(_)), "erreur : {erreur:?}");
    }

    #[test]
    fn plugin_dir_retourne_le_chemin_attendu() {
        let vault = VaultTemp::nouveau();
        assert_eq!(
            plugin_dir(vault.chemin(), "ma-ext"),
            vault
                .chemin()
                .join(".obsidian")
                .join("plugins")
                .join("ma-ext")
        );
    }
}
