//! Export/import d'un vault entier en archive ZIP.
//!
//! Module purement I/O fichier (aucune dépendance à iced). L'export archive
//! tout le contenu du vault (fichiers et dossiers) en préservant les chemins
//! relatifs ; l'import restaure la hiérarchie dans une racine cible, sans
//! jamais écraser un fichier existant.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Archive tout le contenu de `vault_root` (fichiers et dossiers) dans
/// `dest` (un fichier `.zip`), en stockant les chemins relatifs au root.
///
/// Les dossiers cachés (dont le nom commence par `.`, par exemple
/// `.obsidian` et `.trash`) sont ignorés, comme lors du scan. L'écriture
/// est atomique : l'archive est d'abord écrite dans un fichier `.zip.tmp`
/// du même dossier, puis déplacée vers `dest` via `rename`.
pub fn export_vault_zip(vault_root: &Path, dest: &Path) -> Result<()> {
    require_root(vault_root)?;
    let dest_dir = dest
        .parent()
        .ok_or_else(|| anyhow!("chemin sans dossier parent : {}", dest.display()))?;
    fs::create_dir_all(dest_dir)
        .with_context(|| format!("création du dossier de destination {}", dest_dir.display()))?;
    let (tmp, file) = open_unique_temp(dest_dir, dest)?;
    let result = (|| -> Result<()> {
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        walk_export(vault_root, vault_root, &mut writer, options, dest, &tmp)?;
        writer
            .finish()
            .with_context(|| format!("finalisation de l'archive {}", tmp.display()))?;
        Ok(())
    })();
    if let Err(err) = result {
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    fs::rename(&tmp, dest)
        .with_context(|| format!("déplacement atomique vers {}", dest.display()))?;
    Ok(())
}

/// Parcourt récursivement `dir` et écrit chaque entrée dans `writer` avec son
/// chemin relatif au root, en ignorant les dossiers cachés. Évite aussi
/// d'inclure la sortie elle-même si elle vit dans le vault.
fn walk_export(
    root: &Path,
    dir: &Path,
    writer: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    dest: &Path,
    tmp: &Path,
) -> Result<()> {
    for entry in
        fs::read_dir(dir).with_context(|| format!("lecture du dossier {}", dir.display()))?
    {
        let entry = entry.with_context(|| format!("entrée illisible dans {}", dir.display()))?;
        let path = entry.path();
        // La sortie (ou son temporaire) peut vivre dans le vault : on la
        // saute pour éviter l'auto-inclusion.
        if path == *tmp || path == *dest {
            continue;
        }
        let file_type = entry
            .file_type()
            .with_context(|| format!("type de fichier illisible pour {}", path.display()))?;
        if file_type.is_dir() {
            let is_hidden = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with('.'));
            if is_hidden {
                continue;
            }
            let rel = relative_zip_path(root, &path)?;
            writer
                .add_directory(&rel, options)
                .with_context(|| format!("écriture du dossier {}", rel))?;
            walk_export(root, &path, writer, options, dest, tmp)?;
        } else if file_type.is_file() {
            let rel = relative_zip_path(root, &path)?;
            writer
                .start_file(&rel, options)
                .with_context(|| format!("écriture du fichier {}", rel))?;
            let mut source = File::open(&path)
                .with_context(|| format!("ouverture du fichier {}", path.display()))?;
            std::io::copy(&mut source, writer)
                .with_context(|| format!("copie du contenu de {}", path.display()))?;
        }
    }
    Ok(())
}

/// Calcul le chemin relatif de `path` sous `root`, avec des séparateurs `/`
/// (convention ZIP), indépendamment du séparateur du système.
fn relative_zip_path(root: &Path, path: &Path) -> Result<String> {
    let rel = path
        .strip_prefix(root)
        .with_context(|| format!("chemin hors du vault : {}", path.display()))?;
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            _ => bail!("chemin non relatif attendu : {}", path.display()),
        }
    }
    Ok(parts.join("/"))
}

/// Extrait les entrées de `zip_path` dans `vault_root` (création des
/// sous-dossiers au besoin). Refuse tout chemin qui sortirait de
/// `vault_root` (`../`, chemins absolus, composant `..`). Retourne le nombre
/// de fichiers extraits.
///
/// Aucune écriture silencieuse : si un fichier destination existe déjà, un
/// suffixe `-importé` est ajouté avant l'extension (puis `-importé-N` si
/// nécessaire) pour garantir une destination libre.
pub fn import_vault_zip(zip_path: &Path, vault_root: &Path) -> Result<usize> {
    require_root(vault_root)?;
    let file = File::open(zip_path)
        .with_context(|| format!("ouverture de l'archive {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("lecture de l'archive {}", zip_path.display()))?;
    let mut extracted = 0usize;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .with_context(|| format!("entrée {} de l'archive", i))?;
        let raw_name = entry.name();
        let is_dir = raw_name.ends_with('/');
        let name = raw_name.trim_end_matches('/');
        let rel = confine_zip_entry(name)?;
        let target = vault_root.join(&rel);
        if is_dir {
            fs::create_dir_all(&target)
                .with_context(|| format!("création du dossier {}", target.display()))?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!(
                        "création des dossiers intermédiaires pour {}",
                        target.display()
                    )
                })?;
            }
            let target = unique_target(&target)?;
            write_atomic_bytes(&target, &mut entry)?;
            extracted += 1;
        }
    }
    Ok(extracted)
}

/// Vérifie qu'un nom d'entrée ZIP est un chemin relatif sûr : rejette tout
/// chemin vide, absolu, tout composant `..` (remontée hors du vault), puis
/// retourne la forme normalisée (composants `.` redondants supprimés).
fn confine_zip_entry(name: &str) -> Result<PathBuf> {
    if name.is_empty() {
        bail!("nom d'entrée ZIP vide");
    }
    let mut normalized = PathBuf::new();
    for component in Path::new(name).components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("chemin d'entrée ZIP non confiné : {name}");
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        bail!("nom d'entrée ZIP vide après normalisation");
    }
    Ok(normalized)
}

/// Retourne une destination libre pour `target` : si le fichier existe déjà,
/// un suffixe `-importé` (puis `-importé-N`) est ajouté avant l'extension.
fn unique_target(target: &Path) -> Result<PathBuf> {
    if fs::symlink_metadata(target).is_err() {
        return Ok(target.to_path_buf());
    }
    let dir = target
        .parent()
        .ok_or_else(|| anyhow!("chemin sans dossier parent : {}", target.display()))?;
    let stem = target
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = target
        .extension()
        .map(|ext| ext.to_string_lossy().into_owned());
    for i in 0..10_000u32 {
        let candidate_name = match &ext {
            Some(ext) if i == 0 => format!("{stem}-importé.{ext}"),
            Some(ext) => format!("{stem}-importé-{i}.{ext}"),
            None if i == 0 => format!("{stem}-importé"),
            None => format!("{stem}-importé-{i}"),
        };
        let candidate = dir.join(candidate_name);
        if fs::symlink_metadata(&candidate).is_err() {
            return Ok(candidate);
        }
    }
    bail!(
        "impossible de trouver un nom libre pour {}",
        target.display()
    );
}

/// Écrit le contenu de `reader` dans `target` de façon atomique : fichier
/// temporaire dans le même dossier, puis `rename`. En cas d'erreur, le
/// temporaire est nettoyé.
fn write_atomic_bytes(target: &Path, mut reader: impl Read) -> Result<()> {
    let dir = target
        .parent()
        .ok_or_else(|| anyhow!("chemin sans dossier parent : {}", target.display()))?;
    let (temp, mut file) = open_unique_temp(dir, target)?;
    let result = (|| -> Result<()> {
        std::io::copy(&mut reader, &mut file)
            .with_context(|| format!("écriture du fichier temporaire {}", temp.display()))?;
        file.flush()
            .with_context(|| format!("flush du fichier temporaire {}", temp.display()))?;
        Ok(())
    })();
    if let Err(err) = result {
        let _ = fs::remove_file(&temp);
        return Err(err);
    }
    fs::rename(&temp, target)
        .with_context(|| format!("déplacement atomique vers {}", target.display()))?;
    Ok(())
}

/// Vérifie que `root` existe et est un dossier.
fn require_root(root: &Path) -> Result<()> {
    if !root.is_dir() {
        bail!("la racine du vault n'existe pas : {}", root.display());
    }
    Ok(())
}

/// Ouvre un fichier temporaire unique (`.{nom}.tmp-{pid}[-n]`) dans `dir`, en
/// `create_new` pour garantir l'unicité même en cas d'accès concurrent.
fn open_unique_temp(dir: &Path, target: &Path) -> Result<(PathBuf, File)> {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "archive".to_string());
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
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("création du fichier temporaire {}", temp.display()));
            }
        }
    }
    bail!(
        "impossible de créer un fichier temporaire unique dans {}",
        dir.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Crée un vault temporaire unique et retourne (root, chemin de nettoyage).
    fn temp_vault(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "obsidian_{prefix}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("horloge système avant epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("création du vault temporaire");
        dir
    }

    #[test]
    fn export_cree_archive_et_ignore_dossiers_caches() {
        let root = temp_vault("export");
        fs::create_dir_all(root.join("notes")).expect("dossier notes");
        fs::create_dir_all(root.join(".obsidian")).expect("dossier caché");
        fs::create_dir_all(root.join(".trash")).expect("dossier caché");
        fs::write(root.join("notes").join("a.md"), "A").expect("écriture a");
        fs::write(root.join(".obsidian").join("app.json"), "{}").expect("écriture cachée");
        fs::write(root.join(".trash").join("old.md"), "vieux").expect("écriture cachée");
        let dest = root.join("export.zip");

        export_vault_zip(&root, &dest).expect("export");

        assert!(dest.is_file(), "l'archive doit exister");
        let file = File::open(&dest).expect("ouverture archive");
        let mut archive = zip::ZipArchive::new(file).expect("lecture archive");
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).expect("entrée").name().to_string())
            .collect();
        assert!(
            names.contains(&"notes/".to_string()),
            "dossier notes présent : {names:?}"
        );
        assert!(
            names.contains(&"notes/a.md".to_string()),
            "note présente : {names:?}"
        );
        assert!(
            !names
                .iter()
                .any(|n| n.contains(".obsidian") || n.contains(".trash")),
            "dossiers cachés exclus : {names:?}"
        );
        fs::remove_dir_all(&root).expect("nettoyage");
    }

    #[test]
    fn import_restaure_la_hierarchie() {
        let root = temp_vault("import");
        let zip_path = root.join("vault.zip");
        {
            let file = File::create(&zip_path).expect("création archive");
            let mut writer = ZipWriter::new(file);
            let options = SimpleFileOptions::default();
            writer
                .add_directory("sous/dossier", options)
                .expect("dossier");
            writer
                .start_file("sous/dossier/note.md", options)
                .expect("note");
            writer.write_all(b"# Titre\n").expect("contenu");
            writer
                .start_file("racine.md", options)
                .expect("note racine");
            writer.write_all(b"Racine").expect("contenu");
            writer.finish().expect("finalisation");
        }
        let vault = temp_vault("import-cible");

        let count = import_vault_zip(&zip_path, &vault).expect("import");
        assert_eq!(count, 2);
        assert_eq!(
            fs::read_to_string(vault.join("sous/dossier/note.md")).expect("lecture note"),
            "# Titre\n"
        );
        assert_eq!(
            fs::read_to_string(vault.join("racine.md")).expect("lecture racine"),
            "Racine"
        );
        assert!(vault.join("sous/dossier").is_dir());
        fs::remove_dir_all(&root).expect("nettoyage");
        fs::remove_dir_all(&vault).expect("nettoyage");
    }

    #[test]
    fn import_refuse_les_chemins_non_confines() {
        let root = temp_vault("malveillant");
        let zip_path = root.join("malveillant.zip");
        {
            let file = File::create(&zip_path).expect("création archive");
            let mut writer = ZipWriter::new(file);
            let options = SimpleFileOptions::default();
            writer
                .start_file("../evasion.txt", options)
                .expect("entrée");
            writer.write_all(b"fuite").expect("contenu");
            writer.start_file("/absolu.txt", options).expect("entrée");
            writer.write_all(b"absolu").expect("contenu");
            writer
                .start_file("a/../../sortie.txt", options)
                .expect("entrée");
            writer.write_all(b"sortie").expect("contenu");
            writer.finish().expect("finalisation");
        }
        let vault = temp_vault("malveillant-cible");

        let err = import_vault_zip(&zip_path, &vault).expect_err("import doit échouer");
        assert!(
            err.to_string().contains("non confiné"),
            "message d'erreur : {err}"
        );
        assert!(
            !vault.join("evasion.txt").exists() && !vault.join("sortie.txt").exists(),
            "aucun fichier ne doit avoir été écrit hors du vault"
        );
        fs::remove_dir_all(&root).expect("nettoyage");
        fs::remove_dir_all(&vault).expect("nettoyage");
    }

    #[test]
    fn roundtrip_export_import() {
        let root = temp_vault("roundtrip");
        fs::create_dir_all(root.join("notes/sous")).expect("dossiers");
        fs::create_dir_all(root.join("vide")).expect("dossier vide");
        fs::write(root.join("notes/sous").join("b.md"), "# B\n").expect("écriture b");
        fs::write(root.join("notes").join("a.md"), "# A\n").expect("écriture a");
        fs::write(root.join("lisez-moi.md"), "Bonjour").expect("écriture lisez-moi");
        let zip_path = root.join("roundtrip.zip");
        export_vault_zip(&root, &zip_path).expect("export");

        let cible = temp_vault("roundtrip-cible");
        let count = import_vault_zip(&zip_path, &cible).expect("import");
        assert_eq!(count, 3);
        assert_eq!(
            fs::read_to_string(cible.join("notes/sous/b.md")).expect("lecture b"),
            "# B\n"
        );
        assert_eq!(
            fs::read_to_string(cible.join("notes/a.md")).expect("lecture a"),
            "# A\n"
        );
        assert_eq!(
            fs::read_to_string(cible.join("lisez-moi.md")).expect("lecture lisez-moi"),
            "Bonjour"
        );
        assert!(cible.join("vide").is_dir(), "dossier vide préservé");
        fs::remove_dir_all(&root).expect("nettoyage");
        fs::remove_dir_all(&cible).expect("nettoyage");
    }

    #[test]
    fn import_sans_ecrasement() {
        let root = temp_vault("ecrasement");
        let zip_path = root.join("vault.zip");
        {
            let file = File::create(&zip_path).expect("création archive");
            let mut writer = ZipWriter::new(file);
            let options = SimpleFileOptions::default();
            writer.start_file("note.md", options).expect("note");
            writer.write_all(b"nouveau contenu").expect("contenu");
            writer.finish().expect("finalisation");
        }
        let vault = temp_vault("ecrasement-cible");
        fs::write(vault.join("note.md"), "contenu original").expect("écriture existante");

        let count = import_vault_zip(&zip_path, &vault).expect("import");
        assert_eq!(count, 1);
        assert_eq!(
            fs::read_to_string(vault.join("note.md")).expect("lecture originale"),
            "contenu original",
            "le fichier existant ne doit pas être écrasé"
        );
        assert_eq!(
            fs::read_to_string(vault.join("note-importé.md")).expect("lecture importée"),
            "nouveau contenu"
        );
        fs::remove_dir_all(&root).expect("nettoyage");
        fs::remove_dir_all(&vault).expect("nettoyage");
    }
}
