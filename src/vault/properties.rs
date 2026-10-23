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

use std::cmp::Ordering;
use std::fmt;
use std::path::Path;

use anyhow::{Context, Result};
use serde_yaml::Value;

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

// ---------------------------------------------------------------------------
// Valeurs typées (panneau Propriétés)
// ---------------------------------------------------------------------------

/// Valeur typée d'une propriété, telle que l'affiche et l'édite le panneau
/// Propriétés.
///
/// L'aller-retour est **sans perte** : pour toute valeur YAML `v`,
/// `PropertyValue::from_yaml(&v).to_yaml() == v` (à l'exception des nombres
/// qui ne tiennent pas dans un `f64`, voir [`PropertyValue::from_yaml`]).
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    /// Propriété absente, ou explicitement vide (`null`, chaîne vide).
    Empty,
    /// Chaîne libre. Un nombre saisi à la main reste ici tant que l'utilisateur
    /// ne l'a pas validé (voir [`PropertyValue::validate_number`]).
    Text(String),
    /// Nombre (entier ou décimal) issu d'une valeur YAML numérique.
    Number(f64),
    /// Case à cocher.
    Checkbox(bool),
    /// Date ISO `AAAA-MM-JJ`.
    Date(IsoDate),
    /// Liste ordonnée de valeurs.
    List(Vec<PropertyValue>),
    /// Valeur YAML non scalaire (mapping, valeur étiquetée) conservée telle
    /// quelle : c'est ce qui garantit l'absence de perte à la réécriture.
    Raw(Value),
}

impl PropertyValue {
    /// Convertit une valeur YAML en valeur typée.
    ///
    /// - `null` → [`PropertyValue::Empty`] ;
    /// - booléen → [`PropertyValue::Checkbox`] ;
    /// - nombre → [`PropertyValue::Number`] ; un nombre non fini (`.nan`,
    ///   `.inf`) est conservé tel quel en [`PropertyValue::Raw`] pour rester
    ///   réversible ;
    /// - chaîne strictement datée `AAAA-MM-JJ` → [`PropertyValue::Date`],
    ///   toute autre chaîne → [`PropertyValue::Text`] (un `"42"` reste du
    ///   texte : seule la validation explicite en fait un nombre) ;
    /// - séquence → [`PropertyValue::List`], élément par élément ;
    /// - mapping ou valeur étiquetée → [`PropertyValue::Raw`].
    ///
    /// Un nombre YAML plus grand qu'un `i64` perd sa précision exacte en
    /// passant par `f64` ; c'est le seul cas où l'aller-retour peut différer.
    pub fn from_yaml(value: &Value) -> Self {
        match value {
            Value::Null => PropertyValue::Empty,
            Value::Bool(flag) => PropertyValue::Checkbox(*flag),
            Value::Number(number) => match number.as_f64() {
                Some(value) if value.is_finite() => PropertyValue::Number(value),
                // `.nan` / `.inf` : on garde la valeur d'origine intacte.
                _ => PropertyValue::Raw(value.clone()),
            },
            Value::String(text) => match parse_iso_date(text) {
                Ok(date) => PropertyValue::Date(date),
                Err(_) => PropertyValue::Text(text.clone()),
            },
            Value::Sequence(items) => {
                PropertyValue::List(items.iter().map(PropertyValue::from_yaml).collect())
            }
            Value::Mapping(_) | Value::Tagged(_) => PropertyValue::Raw(value.clone()),
        }
    }

    /// Convertit la valeur typée en valeur YAML, sans perte.
    ///
    /// Un nombre entier est réécrit en entier (`42`, pas `42.0`), une date est
    /// réécrite en chaîne ISO identique : c'est ce qui rend l'aller-retour
    /// stable et le fichier lisible.
    pub fn to_yaml(&self) -> Value {
        match self {
            PropertyValue::Empty => Value::Null,
            PropertyValue::Text(text) => Value::String(text.clone()),
            // Un `f64` non fini ne peut pas s'écrire en YAML numérique : on
            // retombe sur `Null` plutôt que de paniquer (ce cas n'est produit
            // que si l'appelant construit la valeur à la main).
            PropertyValue::Number(number) => number_to_yaml(*number),
            PropertyValue::Checkbox(flag) => Value::Bool(*flag),
            PropertyValue::Date(date) => Value::String(date.to_string()),
            PropertyValue::List(items) => {
                Value::Sequence(items.iter().map(PropertyValue::to_yaml).collect())
            }
            PropertyValue::Raw(value) => value.clone(),
        }
    }

    /// Interprète une saisie utilisateur du panneau Propriétés.
    ///
    /// Règles, dans cet ordre :
    ///
    /// 1. vide ou blancs seuls → [`PropertyValue::Empty`] ;
    /// 2. `true` / `false` (casse libre) → [`PropertyValue::Checkbox`] ;
    /// 3. saisie de forme `AAAA-MM-JJ` → date, **strictement validée**
    ///    (mois, jour, année bissextile) et rejetée sinon ;
    /// 4. tout le reste → [`PropertyValue::Text`], espaces de bord retirés.
    ///    Un nombre tapé à la main (`"42"`) reste donc du texte jusqu'à
    ///    [`PropertyValue::validate_number`].
    pub fn from_user_input(raw: &str) -> Result<Self, PropertyError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(PropertyValue::Empty);
        }
        if trimmed.eq_ignore_ascii_case("true") {
            return Ok(PropertyValue::Checkbox(true));
        }
        if trimmed.eq_ignore_ascii_case("false") {
            return Ok(PropertyValue::Checkbox(false));
        }
        if looks_like_date(trimmed) {
            return parse_iso_date(trimmed).map(PropertyValue::Date);
        }
        Ok(PropertyValue::Text(trimmed.to_string()))
    }

    /// Convertit un texte numérique en nombre.
    ///
    /// C'est le geste explicite de validation qui distingue `"42"` (texte
    /// conservé tel quel) de `42` (nombre). Les autres types sont renvoyés
    /// inchangés : la validation est idempotente.
    pub fn validate_number(&self) -> Result<Self, PropertyError> {
        match self {
            PropertyValue::Text(text) => text
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .map(PropertyValue::Number)
                .ok_or_else(|| PropertyError::NotANumber {
                    input: text.clone(),
                }),
            other => Ok(other.clone()),
        }
    }

    /// Représentation affichable dans le panneau, sur une ligne.
    ///
    /// Une liste est jointe par `, `, une case est rendue `true` / `false`
    /// (comme Obsidian), une valeur vide ne rend rien.
    pub fn display_text(&self) -> String {
        match self {
            PropertyValue::Empty => String::new(),
            PropertyValue::Text(text) => text.clone(),
            PropertyValue::Number(number) => format_number(*number),
            PropertyValue::Checkbox(flag) => {
                if *flag {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            PropertyValue::Date(date) => date.to_string(),
            PropertyValue::List(items) => items
                .iter()
                .map(PropertyValue::display_text)
                .collect::<Vec<_>>()
                .join(", "),
            PropertyValue::Raw(value) => raw_to_text(value),
        }
    }

    /// Vrai si la propriété n'a pas de contenu affichable.
    pub fn is_empty(&self) -> bool {
        match self {
            PropertyValue::Empty => true,
            PropertyValue::Text(text) => text.is_empty(),
            PropertyValue::List(items) => items.iter().all(PropertyValue::is_empty),
            _ => false,
        }
    }

    /// Rang de tri par type, utilisé par [`sorted_values`] : valeurs vides,
    /// puis textes, nombres, booléens, dates, listes et valeurs brutes.
    /// L'ordre ne dépend que du type, jamais de la locale.
    fn sort_rank(&self) -> u8 {
        match self {
            PropertyValue::Empty => 0,
            PropertyValue::Text(_) => 1,
            PropertyValue::Number(_) => 2,
            PropertyValue::Checkbox(_) => 3,
            PropertyValue::Date(_) => 4,
            PropertyValue::List(_) => 5,
            PropertyValue::Raw(_) => 6,
        }
    }

    /// Compare deux valeurs pour un affichage trié et déterministe.
    ///
    /// À type égal : dates chronologiquement, nombres numériquement, booléens
    /// `false` avant `true`, textes sans tenir compte de la casse puis, à
    /// égalité, dans l'ordre exact des octets. Deux listes de même type
    /// restent dans leur ordre d'origine (tri stable).
    pub fn display_cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (PropertyValue::Date(a), PropertyValue::Date(b)) => a.cmp(b),
            (PropertyValue::Number(a), PropertyValue::Number(b)) => {
                a.partial_cmp(b).unwrap_or(Ordering::Equal)
            }
            (PropertyValue::Checkbox(a), PropertyValue::Checkbox(b)) => a.cmp(b),
            (PropertyValue::Text(a), PropertyValue::Text(b)) => a
                .to_lowercase()
                .cmp(&b.to_lowercase())
                .then_with(|| a.cmp(b)),
            (a, b) => a.sort_rank().cmp(&b.sort_rank()),
        }
    }
}

/// Trie (de façon stable) les valeurs d'une propriété multivaluée pour
/// l'affichage. L'ordre d'entrée des valeurs de même type est conservé.
pub fn sorted_values(values: &[PropertyValue]) -> Vec<PropertyValue> {
    let mut sorted = values.to_vec();
    sorted.sort_by(PropertyValue::display_cmp);
    sorted
}

/// Date ISO `AAAA-MM-JJ`, sans dépendance de calendrier externe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IsoDate {
    /// Année (1 à 9999).
    pub year: i32,
    /// Mois (1 à 12).
    pub month: u8,
    /// Jour (1 au dernier jour du mois).
    pub day: u8,
}

impl IsoDate {
    /// Nombre de jours du mois, année bissextile comprise.
    fn days_in_month(year: i32, month: u8) -> u8 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if is_leap_year(year) => 29,
            2 => 28,
            _ => 0,
        }
    }
}

impl fmt::Display for IsoDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        )
    }
}

/// Erreur de saisie d'une propriété typée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyError {
    /// La saisie ressemblait à une date mais n'en est pas une valide.
    InvalidDate {
        /// Saisie refusée, telle que tapée.
        input: String,
    },
    /// La valeur n'est pas un nombre exploitable.
    NotANumber {
        /// Saisie refusée, telle que tapée.
        input: String,
    },
}

impl fmt::Display for PropertyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PropertyError::InvalidDate { input } => write!(
                formatter,
                "date invalide « {input} » (format attendu AAAA-MM-JJ)"
            ),
            PropertyError::NotANumber { input } => {
                write!(formatter, "« {input} » n'est pas un nombre")
            }
        }
    }
}

impl std::error::Error for PropertyError {}

/// Année bissextile grégorienne.
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Vrai si la saisie a la forme d'une date (10 caractères, `-` en 5e et 8e
/// position, chiffres ailleurs) : sert uniquement à décider s'il faut valider
/// comme une date plutôt que de la garder en texte.
fn looks_like_date(input: &str) -> bool {
    let octets = input.as_bytes();
    octets.len() == 10
        && octets[4] == b'-'
        && octets[7] == b'-'
        && octets
            .iter()
            .enumerate()
            .all(|(index, octet)| index == 4 || index == 7 || octet.is_ascii_digit())
}

/// Analyse une date stricte `AAAA-MM-JJ`.
///
/// Refuse tout autre format, les années hors 1..=9999, les mois hors 1..=12 et
/// les jours inexistants (`2024-02-30`, `2023-02-29`, `2024-13-40`).
pub fn parse_iso_date(input: &str) -> Result<IsoDate, PropertyError> {
    if !looks_like_date(input) {
        return Err(PropertyError::InvalidDate {
            input: input.to_string(),
        });
    }
    // `looks_like_date` garantit la longueur et la position des séparateurs.
    let year = input[0..4].parse::<i32>();
    let month = input[5..7].parse::<u8>();
    let day = input[8..10].parse::<u8>();
    let (Ok(year), Ok(month), Ok(day)) = (year, month, day) else {
        return Err(PropertyError::InvalidDate {
            input: input.to_string(),
        });
    };
    if !(1..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || day == 0
        || day > IsoDate::days_in_month(year, month)
    {
        return Err(PropertyError::InvalidDate {
            input: input.to_string(),
        });
    }
    Ok(IsoDate { year, month, day })
}

/// Écrit un nombre en YAML : entier quand il l'est, décimal sinon.
fn number_to_yaml(number: f64) -> Value {
    if !number.is_finite() {
        return Value::Null;
    }
    if number.fract() == 0.0 && number >= i64::MIN as f64 && number <= i64::MAX as f64 {
        Value::Number(serde_yaml::Number::from(number as i64))
    } else {
        Value::Number(serde_yaml::Number::from(number))
    }
}

/// Affiche un nombre sans décimale superflue (`42`, `42.5`).
fn format_number(number: f64) -> String {
    if number.fract() == 0.0 && number.abs() < 1e15 {
        format!("{}", number as i64)
    } else {
        format!("{number}")
    }
}

/// Rend une valeur YAML non scalaire sous forme lisible sur une ligne.
fn raw_to_text(value: &Value) -> String {
    match value {
        Value::Mapping(map) => map
            .iter()
            .map(|(key, item)| format!("{}: {}", raw_to_text(key), raw_to_text(item)))
            .collect::<Vec<_>>()
            .join(", "),
        Value::Tagged(tagged) => format!("!{} {}", tagged.tag, raw_to_text(&tagged.value)),
        Value::Sequence(items) => items.iter().map(raw_to_text).collect::<Vec<_>>().join(", "),
        other => PropertyValue::from_yaml(other).display_text(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_yaml::Mapping;

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

#[cfg(test)]
mod tests_valeurs {
    use serde_yaml::Mapping;

    use super::*;

    fn date(year: i32, month: u8, day: u8) -> PropertyValue {
        PropertyValue::Date(IsoDate { year, month, day })
    }

    #[test]
    fn chaque_type_yaml_est_reconnu() {
        assert_eq!(PropertyValue::from_yaml(&Value::Null), PropertyValue::Empty);
        assert_eq!(
            PropertyValue::from_yaml(&Value::Bool(true)),
            PropertyValue::Checkbox(true)
        );
        assert_eq!(
            PropertyValue::from_yaml(&Value::from(42)),
            PropertyValue::Number(42.0)
        );
        assert_eq!(
            PropertyValue::from_yaml(&Value::from(42.5)),
            PropertyValue::Number(42.5)
        );
        assert_eq!(
            PropertyValue::from_yaml(&Value::from("texte libre")),
            PropertyValue::Text("texte libre".to_string())
        );
        // Une chaîne non scalaire reste du texte.
        assert_eq!(
            PropertyValue::from_yaml(&Value::from("42")),
            PropertyValue::Text("42".to_string())
        );
        // Une chaîne strictement datée devient une date.
        assert_eq!(
            PropertyValue::from_yaml(&Value::from("2024-02-29")),
            date(2024, 2, 29)
        );
        // Date impossible : reste du texte, aucun panic.
        assert_eq!(
            PropertyValue::from_yaml(&Value::from("2024-13-40")),
            PropertyValue::Text("2024-13-40".to_string())
        );
        assert_eq!(
            PropertyValue::from_yaml(&Value::Sequence(vec![
                Value::from(1),
                Value::Bool(false),
                Value::from("lien"),
            ])),
            PropertyValue::List(vec![
                PropertyValue::Number(1.0),
                PropertyValue::Checkbox(false),
                PropertyValue::Text("lien".to_string()),
            ])
        );
        // Mapping : conservé tel quel pour ne rien perdre.
        let mut map = Mapping::new();
        map.insert("url".into(), "https://exemple.test".into());
        let value = Value::Mapping(map);
        assert_eq!(
            PropertyValue::from_yaml(&value),
            PropertyValue::Raw(value.clone())
        );
    }

    #[test]
    fn aller_retour_yaml_sans_perte() {
        let mut map = Mapping::new();
        map.insert("a".into(), Value::from(1));
        map.insert("b".into(), Value::from("x"));

        let valeurs = vec![
            Value::Null,
            Value::Bool(false),
            Value::from(0),
            Value::from(42),
            Value::from(-3),
            Value::from(42.5),
            Value::from("texte"),
            Value::from(""),
            Value::from("42"),
            Value::from("2024-01-02"),
            Value::from("2024-13-40"),
            Value::Mapping(map),
            Value::Sequence(vec![
                Value::from("a"),
                Value::Sequence(vec![Value::from(2), Value::Null]),
                Value::Bool(true),
            ]),
        ];

        for valeur in valeurs {
            let typee = PropertyValue::from_yaml(&valeur);
            assert_eq!(typee.to_yaml(), valeur, "aller-retour de {valeur:?}");
            // Deux passages donnent le même type : la conversion est stable.
            assert_eq!(PropertyValue::from_yaml(&typee.to_yaml()), typee);
        }
    }

    #[test]
    fn un_nombre_entier_est_reecrit_en_entier() {
        assert_eq!(PropertyValue::Number(42.0).to_yaml(), Value::from(42));
        assert_eq!(PropertyValue::Number(42.5).to_yaml(), Value::from(42.5));
        assert_eq!(PropertyValue::Number(42.0).display_text(), "42");
        assert_eq!(PropertyValue::Number(42.5).display_text(), "42.5");
    }

    #[test]
    fn dates_valides_acceptees() {
        for (entree, attendu) in [
            (
                "2024-01-02",
                IsoDate {
                    year: 2024,
                    month: 1,
                    day: 2,
                },
            ),
            (
                "2024-02-29",
                IsoDate {
                    year: 2024,
                    month: 2,
                    day: 29,
                },
            ),
            (
                "2000-02-29",
                IsoDate {
                    year: 2000,
                    month: 2,
                    day: 29,
                },
            ),
            (
                "1999-12-31",
                IsoDate {
                    year: 1999,
                    month: 12,
                    day: 31,
                },
            ),
        ] {
            assert_eq!(parse_iso_date(entree), Ok(attendu), "date {entree}");
            assert_eq!(attendu.to_string(), entree);
        }
    }

    #[test]
    fn dates_impossibles_rejetees() {
        for entree in [
            "2024-13-40",
            "2024-02-30",
            "2023-02-29",
            "1900-02-29",
            "2024-00-10",
            "2024-04-31",
            "0000-01-01",
            "2024-1-1",
            "2024-01-02T00:00",
            "2024/01/02",
            "",
        ] {
            assert_eq!(
                parse_iso_date(entree),
                Err(PropertyError::InvalidDate {
                    input: entree.to_string()
                }),
                "date refusée : {entree}"
            );
        }
        // Le message d'erreur nomme la saisie et le format attendu.
        let message = PropertyError::InvalidDate {
            input: "2024-02-30".to_string(),
        }
        .to_string();
        assert!(message.contains("2024-02-30"), "message : {message}");
        assert!(message.contains("AAAA-MM-JJ"), "message : {message}");
    }

    #[test]
    fn saisie_utilisateur_type_selon_la_forme() {
        assert_eq!(
            PropertyValue::from_user_input("  "),
            Ok(PropertyValue::Empty)
        );
        assert_eq!(PropertyValue::from_user_input(""), Ok(PropertyValue::Empty));
        assert_eq!(
            PropertyValue::from_user_input("  TRUE "),
            Ok(PropertyValue::Checkbox(true))
        );
        assert_eq!(
            PropertyValue::from_user_input("false"),
            Ok(PropertyValue::Checkbox(false))
        );
        assert_eq!(
            PropertyValue::from_user_input(" 2024-02-29 "),
            Ok(date(2024, 2, 29))
        );
        // Un nombre tapé à la main reste du texte…
        assert_eq!(
            PropertyValue::from_user_input("42"),
            Ok(PropertyValue::Text("42".to_string()))
        );
        // …jusqu'à validation explicite.
        let valide = PropertyValue::from_user_input("42")
            .expect("saisie valide")
            .validate_number()
            .expect("nombre valide");
        assert_eq!(valide, PropertyValue::Number(42.0));
        // Une date impossible est refusée, pas transformée en texte.
        assert_eq!(
            PropertyValue::from_user_input("2024-02-30"),
            Err(PropertyError::InvalidDate {
                input: "2024-02-30".to_string()
            })
        );
    }

    #[test]
    fn validation_numerique_idempotente_et_erreurs() {
        let texte = PropertyValue::Text("  3.5 ".to_string());
        assert_eq!(texte.validate_number(), Ok(PropertyValue::Number(3.5)));
        let nombre = PropertyValue::Number(3.5);
        assert_eq!(nombre.validate_number(), Ok(nombre.clone()));
        assert_eq!(
            PropertyValue::Text("douze".to_string()).validate_number(),
            Err(PropertyError::NotANumber {
                input: "douze".to_string()
            })
        );
        // `NaN` et l'infini ne sont pas des nombres acceptables en propriété.
        assert!(PropertyValue::Text("NaN".to_string())
            .validate_number()
            .is_err());
    }

    #[test]
    fn affichage_des_valeurs() {
        assert_eq!(PropertyValue::Empty.display_text(), "");
        assert_eq!(PropertyValue::Checkbox(true).display_text(), "true");
        assert_eq!(PropertyValue::Checkbox(false).display_text(), "false");
        assert_eq!(date(2024, 3, 9).display_text(), "2024-03-09");
        assert_eq!(
            PropertyValue::List(vec![
                PropertyValue::Text("a".to_string()),
                PropertyValue::Number(2.0),
                PropertyValue::Empty,
            ])
            .display_text(),
            "a, 2, "
        );
    }

    #[test]
    fn valeurs_vides_detectees() {
        assert!(PropertyValue::Empty.is_empty());
        assert!(PropertyValue::Text(String::new()).is_empty());
        assert!(PropertyValue::List(vec![PropertyValue::Empty]).is_empty());
        assert!(!PropertyValue::Text("x".to_string()).is_empty());
        assert!(!PropertyValue::Checkbox(false).is_empty());
        assert!(!PropertyValue::Number(0.0).is_empty());
    }

    #[test]
    fn tri_stable_et_deterministe() {
        let valeurs = vec![
            PropertyValue::Text("banane".to_string()),
            PropertyValue::Number(10.0),
            PropertyValue::Text("Abricot".to_string()),
            PropertyValue::Checkbox(true),
            date(2023, 5, 1),
            PropertyValue::Empty,
            PropertyValue::Number(2.0),
            date(2024, 1, 1),
            PropertyValue::Text("abricot".to_string()),
        ];
        let triees = sorted_values(&valeurs);
        assert_eq!(
            triees,
            vec![
                PropertyValue::Empty,
                // Textes : casse ignorée, puis ordre exact (`Abricot` < `abricot`
                // < `banane`).
                PropertyValue::Text("Abricot".to_string()),
                PropertyValue::Text("abricot".to_string()),
                PropertyValue::Text("banane".to_string()),
                PropertyValue::Number(2.0),
                PropertyValue::Number(10.0),
                PropertyValue::Checkbox(true),
                date(2023, 5, 1),
                date(2024, 1, 1),
            ]
        );
        // Le tri ne modifie pas l'entrée et reste idempotent.
        assert_eq!(valeurs.len(), 9);
        assert_eq!(sorted_values(&triees), triees);
    }

    #[test]
    fn tri_conserve_l_ordre_des_listes() {
        let premiere = PropertyValue::List(vec![PropertyValue::Text("b".to_string())]);
        let seconde = PropertyValue::List(vec![PropertyValue::Text("a".to_string())]);
        let triees = sorted_values(&[premiere.clone(), seconde.clone()]);
        assert_eq!(triees, vec![premiere, seconde]);
    }
}
