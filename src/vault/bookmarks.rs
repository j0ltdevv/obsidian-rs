//! Marque-pages : store JSON dans `.obsidian/bookmarks.json`. Écriture
//! atomique (temporaire + rename).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Nom du fichier de marque-pages dans `.obsidian/`.
pub const BOOKMARKS_FILE: &str = "bookmarks.json";

/// Un marque-page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bookmark {
    /// Identifiant unique et stable.
    pub id: String,
    /// Nom affiché.
    pub name: String,
    /// Chemin relatif de la cible (note, vue…).
    pub path: PathBuf,
    /// Horodatage de création (secondes epoch).
    pub created_at: u64,
}

/// Store de marque-pages adossé à un fichier JSON.
#[derive(Debug)]
pub struct BookmarkStore {
    file: PathBuf,
    items: Vec<Bookmark>,
    counter: u64,
}

impl BookmarkStore {
    /// Charge le store depuis `.obsidian/bookmarks.json`. Fichier absent ou
    /// vide → store vide ; JSON invalide → erreur.
    pub fn load(vault_root: &Path) -> anyhow::Result<Self> {
        let file = vault_root.join(".obsidian").join(BOOKMARKS_FILE);
        if !file.exists() {
            return Ok(Self {
                file,
                items: Vec::new(),
                counter: 0,
            });
        }
        let raw = fs::read_to_string(&file)?;
        if raw.trim().is_empty() {
            return Ok(Self {
                file,
                items: Vec::new(),
                counter: 0,
            });
        }
        let items: Vec<Bookmark> = serde_json::from_str(&raw)?;
        let counter = items.len() as u64;
        Ok(Self {
            file,
            items,
            counter,
        })
    }

    /// Écrit le store de façon atomique (temporaire + rename), en créant
    /// `.obsidian/` si nécessaire.
    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.file.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = self.file.with_extension("json.tmp");
        let data = serde_json::to_string_pretty(&self.items)?;
        fs::write(&tmp, data)?;
        fs::rename(&tmp, &self.file)?;
        Ok(())
    }

    /// Ajoute un marque-page et le retourne (id unique).
    pub fn add(&mut self, name: &str, path: &Path) -> Bookmark {
        self.counter += 1;
        let id = format!("bm-{}-{}", now_secs(), self.counter);
        let bookmark = Bookmark {
            id,
            name: name.to_string(),
            path: path.to_path_buf(),
            created_at: now_secs(),
        };
        self.items.push(bookmark.clone());
        bookmark
    }

    /// Supprime un marque-page par id. Retourne `true` si retiré.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|b| b.id != id);
        self.items.len() != before
    }

    /// Liste (ordonnée) des marque-pages.
    pub fn list(&self) -> &[Bookmark] {
        &self.items
    }

    /// Marque-page par id.
    pub fn get(&self, id: &str) -> Option<&Bookmark> {
        self.items.iter().find(|b| b.id == id)
    }
}

/// Secondes epoch courantes.
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Sandbox {
        dir: PathBuf,
    }

    impl Sandbox {
        fn new() -> Self {
            let mut dir = std::env::temp_dir();
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            dir.push(format!("obsidian-rs-bm-{unique}"));
            fs::create_dir_all(&dir).unwrap();
            Self { dir }
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn load_fichier_absent_retourne_vide() {
        let sb = Sandbox::new();
        let store = BookmarkStore::load(&sb.dir).unwrap();
        assert!(store.list().is_empty());
    }

    #[test]
    fn roundtrip_add_save_load() {
        let sb = Sandbox::new();
        let mut store = BookmarkStore::load(&sb.dir).unwrap();
        let bm = store.add("Ma note", Path::new("Dossier/Note.md"));
        store.save().unwrap();

        let reloaded = BookmarkStore::load(&sb.dir).unwrap();
        assert_eq!(reloaded.list().len(), 1);
        assert_eq!(reloaded.list()[0].id, bm.id);
        assert_eq!(reloaded.list()[0].name, "Ma note");
        assert_eq!(reloaded.list()[0].path, PathBuf::from("Dossier/Note.md"));
    }

    #[test]
    fn remove_retire_par_id() {
        let sb = Sandbox::new();
        let mut store = BookmarkStore::load(&sb.dir).unwrap();
        let a = store.add("A", Path::new("a.md"));
        let b = store.add("B", Path::new("b.md"));
        assert!(store.remove(&a.id));
        assert!(!store.remove(&a.id));
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.list()[0].id, b.id);
    }

    #[test]
    fn json_corrompu_erreur() {
        let sb = Sandbox::new();
        fs::create_dir_all(sb.dir.join(".obsidian")).unwrap();
        fs::write(sb.dir.join(".obsidian").join(BOOKMARKS_FILE), "{invalide").unwrap();
        assert!(BookmarkStore::load(&sb.dir).is_err());
    }

    #[test]
    fn add_genere_des_id_distincts() {
        let sb = Sandbox::new();
        let mut store = BookmarkStore::load(&sb.dir).unwrap();
        let a = store.add("A", Path::new("a.md"));
        let b = store.add("B", Path::new("b.md"));
        assert_ne!(a.id, b.id);
    }
}
