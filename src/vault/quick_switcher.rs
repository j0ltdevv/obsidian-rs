//! Modèle du *Quick Switcher* : sélection d'une note par requête texte.
//!
//! Le modèle garde deux listes :
//!
//! - `recents`, les notes ouvertes récemment, de la plus récente à la moins
//!   récente (sans doublon) ;
//! - `results`, la liste affichée, construite à partir des récents qui
//!   correspondent à la requête, puis des autres résultats du moteur dans
//!   l'ordre retourné par celui-ci.
//!
//! `limit` plafonne à la fois `results` et `recents`.
//!
//! Le module fournit aussi un classement additif ([`hits_for`]) qui va plus
//! loin qu'un simple filtre : portée de requête (`#tag`, `path:…`, `/dossier`,
//! `>` pour le mode commande), correspondance par alias, et score par palier
//! (préfixe > début de mot > sous-chaîne > sous-séquence) avec départage
//! déterministe par chemin.

use super::aliases::extract_aliases;
use super::note::Note;

/// Modèle du Quick Switcher.
///
/// Les index de `results` et `recents` renvoient aux notes du vault (par
/// exemple l'index retourné par `scan_vault`). Le modèle ne valide pas ces
/// index : c'est à l'appelant de garantir qu'ils existent.
#[derive(Debug, PartialEq, Eq)]
pub struct SwitcherModel {
    /// Requête en cours de saisie.
    pub query: String,
    /// Notes affichées, dans l'ordre de présentation.
    pub results: Vec<usize>,
    /// Notes ouvertes récemment, de la plus récente à la moins récente.
    pub recents: Vec<usize>,
    /// Nombre maximal de résultats et de récents retenus.
    pub limit: usize,
}

impl SwitcherModel {
    /// Crée un modèle vide. `limit` plafonne `results` et `recents`.
    pub fn new(limit: usize) -> Self {
        SwitcherModel {
            query: String::new(),
            results: Vec::new(),
            recents: Vec::new(),
            limit,
        }
    }

    /// Met à jour la requête et reconstruit `results`.
    ///
    /// `search` est le moteur de recherche du vault : il reçoit la requête et
    /// retourne les index des notes correspondantes, déjà triés par pertinence.
    /// Il n'est interrogé qu'une fois par mise à jour.
    ///
    /// `results` place d'abord les récents qui correspondent à la requête
    /// (dans l'ordre d'utilisation), puis les autres résultats du moteur dans
    /// leur ordre, sans doublon. Une requête vide est une correspondance
    /// universelle : `results` devient alors les récents suivis des autres
    /// notes dans l'ordre du vault.
    pub fn update_query(&mut self, q: &str, search: impl Fn(&str) -> Vec<usize>) {
        self.query.clear();
        self.query.push_str(q);
        self.results.clear();

        if self.limit == 0 {
            return;
        }

        let hits = search(q);
        let mut seen = Vec::with_capacity(self.limit.min(self.recents.len() + hits.len()));

        // Les récents en tête, dans l'ordre d'utilisation.
        for &index in &self.recents {
            if self.results.len() >= self.limit {
                return;
            }
            if q.is_empty() || hits.contains(&index) {
                self.results.push(index);
                seen.push(index);
            }
        }

        // Le reste, dans l'ordre du moteur.
        for index in hits {
            if self.results.len() >= self.limit {
                break;
            }
            if !seen.contains(&index) {
                seen.push(index);
                self.results.push(index);
            }
        }
    }

    /// Enregistre l'ouverture d'une note : elle devient la plus récente.
    ///
    /// Si la note était déjà dans `recents`, elle est déplacée en tête. Les
    /// doublons sont supprimés et la liste est tronquée à `limit`.
    pub fn record_open(&mut self, index: usize) {
        if self.limit == 0 {
            return;
        }
        if let Some(pos) = self.recents.iter().position(|&i| i == index) {
            self.recents.remove(pos);
        } else if self.recents.len() >= self.limit {
            // La liste est pleine et la note est nouvelle : on éjecte la moins
            // récente pour faire de la place.
            self.recents.pop();
        }
        self.recents.insert(0, index);
    }

    /// Retourne l'index de la note à la position `pos` dans `results`.
    pub fn select(&self, pos: usize) -> Option<usize> {
        self.results.get(pos).copied()
    }

    /// Retourne la liste des récents.
    pub fn recents(&self) -> &[usize] {
        &self.recents
    }

    /// Met à jour la requête avec le classement par paliers et alias.
    ///
    /// Contrairement à [`SwitcherModel::update_query`], l'ordre des récents
    /// n'est pas prioritaire : `results` suit exactement [`hits_for`] (score
    /// décroissant, puis chemin croissant). La requête brute est conservée
    /// telle quelle dans `query`, préfixes de portée compris. Retourne les
    /// correspondances classées, tronquées à `limit`.
    pub fn update_query_ranked(&mut self, raw: &str, notes: &[Note]) -> Vec<ScoredHit> {
        self.query.clear();
        self.query.push_str(raw);
        let hits = hits_for(notes, raw, self.limit);
        self.results = hits.iter().map(|h| h.index).collect();
        hits
    }
}

// ---------------------------------------------------------------------------
// Classement additif
// ---------------------------------------------------------------------------

/// Palier de classement : l'étiquette commence par le terme recherché.
pub const RANK_PREFIX: u32 = 4;
/// Palier de classement : le terme commence un mot de l'étiquette.
pub const RANK_WORD: u32 = 3;
/// Palier de classement : le terme apparaît tel quel, en milieu de mot.
pub const RANK_SUBSTRING: u32 = 2;
/// Palier de classement : les caractères du terme apparaissent dans l'ordre.
pub const RANK_SUBSEQUENCE: u32 = 1;

/// Portée extraite du début d'une requête de switcher.
///
/// Les préfixes reconnus sont, dans l'ordre : `>` (mode commande), `#tag`
/// (filtre par tag), `path:dossier` et `/dossier` (filtre par préfixe de
/// chemin). Tout le reste est une requête sur les noms de notes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryScope {
    /// Aucune portée : la requête porte sur toutes les notes.
    All,
    /// Filtre par tag (`#a/b`), comparaison insensible à la casse ; un tag
    /// parent inclut ses enfants (`#a` retient `#a/b`).
    Tag(String),
    /// Filtre par préfixe de chemin relatif, séparateurs normalisés en `/`.
    Path(String),
    /// Mode commande : la requête désigne une commande, pas une note.
    Commands,
}

/// Correspondance d'une étiquette avec un terme recherché.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelMatch {
    /// Palier atteint ([`RANK_PREFIX`] à [`RANK_SUBSEQUENCE`]).
    pub score: u32,
    /// Indices (en caractères) des caractères de l'étiquette qui correspondent.
    pub matched: Vec<usize>,
}

/// Résultat classé : une note et la qualité de sa correspondance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoredHit {
    /// Index de la note dans la liste fournie à [`hits_for`].
    pub index: usize,
    /// Palier de classement ; `0` pour une requête vide.
    pub score: u32,
    /// Indices des caractères correspondants dans l'étiquette gagnante
    /// (nom de note, ou alias si `via_alias`).
    pub matched: Vec<usize>,
    /// Vrai si la correspondance vient d'un alias du frontmatter.
    pub via_alias: bool,
}

/// Normalise un caractère pour la comparaison : minuscule, sans diacritique
/// latin courant. La transformation reste **un caractère vers un caractère**,
/// afin que les indices de correspondance restent alignés sur l'étiquette
/// d'origine. Les ligatures (`œ`, `æ`) ne sont pas décomposées.
fn normaliser(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'a',
        'ç' | 'Ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
        'ñ' | 'Ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
        'ý' | 'ÿ' | 'Ý' | 'Ÿ' => 'y',
        _ => c.to_lowercase().next().unwrap_or(c),
    }
}

/// Normalise un chemin relatif pour la comparaison : séparateurs en `/`,
/// minuscules, sans préfixe `./` ni `/` initial.
fn normaliser_chemin(chemin: &str) -> String {
    chemin
        .replace('\\', "/")
        .to_lowercase()
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

/// Découpe une requête brute en portée et terme de recherche.
///
/// Le terme retourné est vide lorsque la portée absorbe toute la requête
/// (`#tag`, `path:…`, `/dossier`). Le mode commande retourne le texte qui suit
/// `>`, à charge de l'appelant de l'interpréter.
pub fn parse_query(raw: &str) -> (QueryScope, &str) {
    let t = raw.trim();
    if let Some(rest) = t.strip_prefix('>') {
        return (QueryScope::Commands, rest.trim());
    }
    if let Some(rest) = t.strip_prefix('#') {
        return (QueryScope::Tag(rest.trim().to_string()), "");
    }
    if let Some(rest) = t.strip_prefix("path:") {
        return (QueryScope::Path(normaliser_chemin(rest.trim())), "");
    }
    if let Some(rest) = t.strip_prefix('/') {
        return (QueryScope::Path(normaliser_chemin(rest.trim())), "");
    }
    (QueryScope::All, t)
}

/// Classe une étiquette face à un terme recherché.
///
/// Retourne `None` si le terme n'apparaît pas, même en sous-séquence. La
/// comparaison ignore la casse et les diacritiques latins courants. Un terme
/// vide ne correspond à rien : c'est à l'appelant de traiter ce cas.
pub fn score_label(label: &str, needle: &str) -> Option<LabelMatch> {
    let l: Vec<char> = label.chars().map(normaliser).collect();
    let n: Vec<char> = needle.chars().map(normaliser).collect();
    if n.is_empty() || l.is_empty() || n.len() > l.len() {
        return None;
    }

    // 1. Préfixe : l'étiquette commence par le terme.
    if l.starts_with(n.as_slice()) {
        return Some(LabelMatch {
            score: RANK_PREFIX,
            matched: (0..n.len()).collect(),
        });
    }

    // 2. Début de mot (précédé d'un caractère non alphanumérique) ou
    //    sous-chaîne quelconque.
    if let Some(pos) = l.windows(n.len()).position(|w| w == n.as_slice()) {
        let debut_de_mot = pos == 0 || !l[pos - 1].is_alphanumeric();
        let score = if debut_de_mot {
            RANK_WORD
        } else {
            RANK_SUBSTRING
        };
        return Some(LabelMatch {
            score,
            matched: (pos..pos + n.len()).collect(),
        });
    }

    // 3. Sous-séquence : les caractères du terme apparaissent dans l'ordre.
    let mut matched = Vec::with_capacity(n.len());
    let mut j = 0usize;
    for (i, c) in l.iter().enumerate() {
        if j < n.len() && *c == n[j] {
            matched.push(i);
            j += 1;
        }
    }
    if j == n.len() {
        return Some(LabelMatch {
            score: RANK_SUBSEQUENCE,
            matched,
        });
    }

    None
}

/// Vrai si la note entre dans la portée demandée.
fn note_dans_portee(scope: &QueryScope, note: &Note) -> bool {
    match scope {
        QueryScope::All => true,
        QueryScope::Commands => false,
        QueryScope::Tag(tag) => {
            let filtre = tag.to_lowercase();
            if filtre.is_empty() {
                return true;
            }
            let prefixe = format!("{filtre}/");
            note.metadata.tags.iter().any(|t| {
                let t = t.to_lowercase();
                t == filtre || t.starts_with(&prefixe)
            })
        }
        QueryScope::Path(dossier) => {
            if dossier.is_empty() {
                return true;
            }
            normaliser_chemin(&note.path.to_string_lossy()).starts_with(dossier)
        }
    }
}

/// Classe les notes du vault pour une requête brute.
///
/// La portée est extraite par [`parse_query`] ; les notes hors portée sont
/// écartées. Une requête vide (ou une portée qui absorbe la requête) retient
/// toutes les notes en portée, classées par chemin. Les alias du frontmatter
/// sont testés après le nom et ne remplacent celui-ci qu'à score strictement
/// supérieur, donc `via_alias` reste faux à score égal.
///
/// Le résultat est déterministe : score décroissant, puis chemin croissant,
/// puis index croissant. `limit` à `0` retourne une liste vide.
pub fn hits_for(notes: &[Note], raw: &str, limit: usize) -> Vec<ScoredHit> {
    if limit == 0 {
        return Vec::new();
    }

    let (scope, needle) = parse_query(raw);
    let mut hits: Vec<ScoredHit> = Vec::new();

    for (index, note) in notes.iter().enumerate() {
        if !note_dans_portee(&scope, note) {
            continue;
        }

        let mut meilleur: Option<ScoredHit> = None;
        if needle.is_empty() {
            meilleur = Some(ScoredHit {
                index,
                score: 0,
                matched: Vec::new(),
                via_alias: false,
            });
        } else if let Some(m) = score_label(&note.name, needle) {
            meilleur = Some(ScoredHit {
                index,
                score: m.score,
                matched: m.matched,
                via_alias: false,
            });
        }

        if !needle.is_empty() {
            for alias in extract_aliases(&note.metadata.frontmatter) {
                let Some(m) = score_label(&alias, needle) else {
                    continue;
                };
                let ameliore = match &meilleur {
                    Some(courant) => m.score > courant.score,
                    None => true,
                };
                if ameliore {
                    meilleur = Some(ScoredHit {
                        index,
                        score: m.score,
                        matched: m.matched,
                        via_alias: true,
                    });
                }
            }
        }

        if let Some(hit) = meilleur {
            hits.push(hit);
        }
    }

    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| notes[a.index].path.cmp(&notes[b.index].path))
            .then_with(|| a.index.cmp(&b.index))
    });
    hits.truncate(limit);
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::note::NoteMetadata;
    use std::cell::Cell;
    use std::path::PathBuf;

    /// Construit une note de test avec tags et alias.
    fn note(path: &str, name: &str, tags: &[&str], aliases: &[&str]) -> Note {
        let mut frontmatter = serde_yaml::Mapping::new();
        if !aliases.is_empty() {
            let sequence: Vec<serde_yaml::Value> = aliases
                .iter()
                .map(|a| serde_yaml::Value::String((*a).to_string()))
                .collect();
            frontmatter.insert(
                serde_yaml::Value::String("aliases".to_string()),
                serde_yaml::Value::Sequence(sequence),
            );
        }
        let frontmatter = if frontmatter.is_empty() {
            None
        } else {
            Some(serde_yaml::Value::Mapping(frontmatter))
        };
        Note {
            path: PathBuf::from(path),
            name: name.to_string(),
            metadata: NoteMetadata {
                frontmatter,
                frontmatter_error: None,
                wikilinks: Vec::new(),
                tags: tags.iter().map(|t| (*t).to_string()).collect(),
            },
        }
    }

    #[test]
    fn prefixe_classe_avant_sous_chaine() {
        let notes = vec![
            note("a.md", "monprojet", &[], &[]),
            note("b.md", "Projet Alpha", &[], &[]),
        ];
        let hits = hits_for(&notes, "proj", 10);

        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].index, 1, "le préfixe doit passer devant");
        assert_eq!(hits[0].score, RANK_PREFIX);
        assert_eq!(hits[1].score, RANK_SUBSTRING);
    }

    #[test]
    fn debut_de_mot_classe_avant_sous_chaine() {
        let notes = vec![
            note("a.md", "monprojet", &[], &[]),
            note("b.md", "mon projet", &[], &[]),
        ];
        let hits = hits_for(&notes, "proj", 10);

        assert_eq!(hits[0].index, 1, "début de mot avant milieu de mot");
        assert_eq!(hits[0].score, RANK_WORD);
        assert_eq!(hits[1].score, RANK_SUBSTRING);
    }

    #[test]
    fn egalite_de_score_departagee_par_chemin() {
        let notes = vec![
            note("z.md", "Alpha", &[], &[]),
            note("a.md", "Alpha", &[], &[]),
        ];
        let premier = hits_for(&notes, "alpha", 10);
        let second = hits_for(&notes, "alpha", 10);

        assert_eq!(premier, second, "le classement doit être stable");
        assert_eq!(premier[0].index, 1, "a.md avant z.md");
        assert_eq!(premier[1].index, 0);
    }

    #[test]
    fn alias_remonte_une_note_par_son_frontmatter() {
        let notes = vec![
            note("a.md", "Note banale", &[], &[]),
            note("b.md", "Autre", &[], &["Zébulon"]),
        ];
        let hits = hits_for(&notes, "zebulon", 10);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].index, 1);
        assert!(hits[0].via_alias);
    }

    #[test]
    fn le_nom_prime_sur_un_alias_de_meme_score() {
        let notes = vec![note("b.md", "Alpha", &[], &["Alpha"])];
        let hits = hits_for(&notes, "alpha", 10);

        assert_eq!(hits.len(), 1);
        assert!(!hits[0].via_alias, "le nom gagne à score égal");
    }

    #[test]
    fn portee_tag_filtre_et_inclut_les_enfants() {
        let notes = vec![
            note("a.md", "Un", &["projet/alpha"], &[]),
            note("b.md", "Deux", &["autre"], &[]),
            note("c.md", "Trois", &["Projet"], &[]),
        ];
        let hits = hits_for(&notes, "#projet", 10);
        let indices: Vec<usize> = hits.iter().map(|h| h.index).collect();

        assert_eq!(indices, vec![0, 2]);
        assert_eq!(parse_query("#projet").0, QueryScope::Tag("projet".into()));
    }

    #[test]
    fn portee_chemin_filtre_par_prefixe() {
        let notes = vec![
            note("notes/jour.md", "Jour", &[], &[]),
            note("autres/x.md", "X", &[], &[]),
        ];
        let via_prefixe = hits_for(&notes, "path:notes", 10);
        let via_slash = hits_for(&notes, "/notes", 10);

        assert_eq!(via_prefixe.len(), 1);
        assert_eq!(via_prefixe[0].index, 0);
        assert_eq!(via_slash.len(), 1);
        assert_eq!(via_slash[0].index, 0);
    }

    #[test]
    fn mode_commande_ne_retourne_aucune_note() {
        let notes = vec![note("a.md", "Alpha", &[], &[])];
        let (scope, terme) = parse_query(">ouvrir");

        assert_eq!(scope, QueryScope::Commands);
        assert_eq!(terme, "ouvrir");
        assert!(hits_for(&notes, ">ouvrir", 10).is_empty());
    }

    #[test]
    fn requete_vide_retourne_tout_par_chemin() {
        let notes = vec![
            note("z.md", "Zêta", &[], &[]),
            note("a.md", "Alpha", &[], &[]),
        ];
        let hits = hits_for(&notes, "   ", 10);

        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].index, 1);
        assert_eq!(hits[0].score, 0);
        assert_eq!(hits[1].index, 0);
    }

    #[test]
    fn accents_ignores_sans_panique() {
        assert_eq!(
            score_label("Café", "cafe").map(|m| m.score),
            Some(RANK_PREFIX)
        );
        assert_eq!(
            score_label("élève", "ELEVE").map(|m| m.score),
            Some(RANK_PREFIX)
        );
        // Sous-séquence accentuée : indices alignés sur les caractères d'origine.
        let m = score_label("Ça fête", "cft");
        assert_eq!(m.as_ref().map(|m| m.score), Some(RANK_SUBSEQUENCE));
        assert_eq!(m.map(|m| m.matched), Some(vec![0, 3, 5]));
    }

    #[test]
    fn limit_nulle_ne_classe_rien() {
        let notes = vec![note("a.md", "Alpha", &[], &[])];

        assert!(hits_for(&notes, "alpha", 0).is_empty());
    }

    #[test]
    fn update_query_ranked_alimente_results() {
        let notes = vec![
            note("a.md", "Alpha", &[], &[]),
            note("b.md", "Beta", &[], &[]),
        ];
        let mut modele = SwitcherModel::new(10);
        let hits = modele.update_query_ranked("beta", &notes);

        assert_eq!(modele.query, "beta");
        assert_eq!(modele.results, vec![1]);
        assert_eq!(hits.len(), 1);
    }

    /// Moteur de recherche factice : `notes` dans l'ordre du vault, filtrées
    /// par appartenance du numéro à la requête.
    fn fake_search(notes: &'static [usize]) -> impl Fn(&str) -> Vec<usize> {
        move |q: &str| {
            if q.is_empty() {
                return notes.to_vec();
            }
            notes
                .iter()
                .filter(|&&n| n.to_string().contains(q))
                .copied()
                .collect()
        }
    }

    /// Construit un modèle avec des récents déjà enregistrés, puis une mise à
    /// jour avec une requête vide (état de démarrage).
    fn model_with(notes: &'static [usize], recents: &[usize], limit: usize) -> SwitcherModel {
        let search = fake_search(notes);
        let mut m = SwitcherModel::new(limit);
        for &r in recents {
            m.record_open(r);
        }
        m.update_query("", &search);
        m
    }

    #[test]
    fn query_vide_renvoie_les_recents_puis_le_vault() {
        let notes: &'static [usize] = &[0, 1, 2, 3, 4];
        let m = model_with(notes, &[3, 1], 10);

        assert_eq!(m.query, "");
        // `record_open(3)` puis `record_open(1)` : 1 est le plus récent
        assert_eq!(m.results, vec![1, 3, 0, 2, 4]);
        assert_eq!(m.recents(), &[1, 3]);
    }

    #[test]
    fn mise_a_jour_requete_renvoie_resultats_moteur() {
        let notes: &'static [usize] = &[10, 20, 30, 40];
        let search = fake_search(notes);
        let mut m = SwitcherModel::new(10);
        m.record_open(30);
        m.update_query("2", &search);

        assert_eq!(m.query, "2");
        // le récent 30 ne correspond pas à "2", seul le résultat moteur reste
        assert_eq!(m.results, vec![20]);
    }

    #[test]
    fn les_recents_correspondant_a_la_requete_restent_en_tete() {
        let notes: &'static [usize] = &[10, 20, 30, 40];
        let search = fake_search(notes);
        let mut m = SwitcherModel::new(10);
        m.record_open(40);
        m.record_open(20);
        m.update_query("0", &search);

        assert_eq!(m.results, vec![20, 40, 10, 30]);
    }

    #[test]
    fn record_open_dedouble_et_met_en_tete() {
        let mut m = SwitcherModel::new(10);
        m.record_open(1);
        m.record_open(2);
        m.record_open(1);

        assert_eq!(m.recents(), &[1, 2]);
    }

    #[test]
    fn select_hors_bornes_retourne_none() {
        let notes: &'static [usize] = &[0, 1, 2];
        let m = model_with(notes, &[], 10);

        assert_eq!(m.select(3), None);
        assert_eq!(m.select(usize::MAX), None);
    }

    #[test]
    fn select_renvoie_l_index_a_la_position() {
        let notes: &'static [usize] = &[5, 7, 9];
        let m = model_with(notes, &[], 10);

        assert_eq!(m.select(0), Some(5));
        assert_eq!(m.select(2), Some(9));
    }

    #[test]
    fn limit_respectee_sur_results() {
        let notes: &'static [usize] = &[0, 1, 2, 3, 4];
        let search = fake_search(notes);
        let mut m = SwitcherModel::new(3);
        m.update_query("", &search);

        assert_eq!(m.results.len(), 3);
        assert_eq!(m.results, vec![0, 1, 2]);
    }

    #[test]
    fn limit_respectee_sur_recents() {
        let mut m = SwitcherModel::new(2);
        m.record_open(1);
        m.record_open(2);
        m.record_open(3);

        assert_eq!(m.recents(), &[3, 2]);
    }

    #[test]
    fn results_stable_a_requete_identique() {
        let notes: &'static [usize] = &[0, 1, 2];
        let search = fake_search(notes);
        let mut m = SwitcherModel::new(10);
        m.record_open(1);
        m.update_query("", &search);
        let first = m.results.clone();
        m.update_query("", &search);

        assert_eq!(m.results, first);
    }

    #[test]
    fn ordre_recents_apres_plusieurs_ouvertures() {
        let mut m = SwitcherModel::new(10);
        for i in 0..5 {
            m.record_open(i);
        }
        assert_eq!(m.recents(), &[4, 3, 2, 1, 0]);

        m.record_open(2);
        assert_eq!(m.recents(), &[2, 4, 3, 1, 0]);
    }

    #[test]
    fn recents_emprunte_sans_mutation() {
        let notes: &'static [usize] = &[0, 1, 2];
        let m = model_with(notes, &[1], 10);

        let r = m.recents();
        assert_eq!(r, &[1]);
        // seul un accès immuable est possible : la compilation garantit
        // l'absence de mutation externe à travers ce prête.
        assert_eq!(m.results, vec![1, 0, 2]);
    }

    #[test]
    fn limite_nulle_ne_retient_rien() {
        let notes: &'static [usize] = &[0, 1];
        let search = fake_search(notes);
        let mut m = SwitcherModel::new(0);
        m.record_open(0);
        m.update_query("", &search);

        assert!(m.results.is_empty());
        assert!(m.recents().is_empty());
        assert_eq!(m.select(0), None);
    }

    #[test]
    fn vault_vide() {
        let notes: &'static [usize] = &[];
        let search = fake_search(notes);
        let mut m = SwitcherModel::new(5);
        m.update_query("", &search);

        assert!(m.results.is_empty());
        assert_eq!(m.recents(), &[] as &[usize]);
        assert_eq!(m.select(0), None);
    }

    #[test]
    fn recherche_sans_correspondance() {
        let notes: &'static [usize] = &[10, 20];
        let search = fake_search(notes);
        let mut m = SwitcherModel::new(5);
        m.record_open(10);
        m.update_query("zzz", &search);

        assert!(m.results.is_empty());
    }

    #[test]
    fn nouveau_modele_est_vide() {
        let m = SwitcherModel::new(5);

        assert_eq!(m.query, "");
        assert!(m.results.is_empty());
        assert!(m.recents().is_empty());
    }

    #[test]
    fn moteur_interroge_une_seule_fois() {
        let calls = Cell::new(0);
        let search = |_: &str| {
            calls.set(calls.get() + 1);
            vec![3, 4]
        };
        let mut m = SwitcherModel::new(10);
        m.record_open(1);
        m.record_open(2);
        m.update_query("x", search);

        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn moteur_avec_doublons_est_deduplique() {
        let search = |_: &str| vec![4, 4, 5];
        let mut m = SwitcherModel::new(10);
        m.record_open(4);
        m.update_query("q", search);

        // 4 est déjà en tête comme récent correspondant, le doublon moteur
        // est ignoré, 5 suit.
        assert_eq!(m.results, vec![4, 5]);
    }

    #[test]
    fn requete_non_vide_exclut_les_recents_non_correspondants() {
        let notes: &'static [usize] = &[0, 1, 2, 3];
        let search = fake_search(notes);
        let mut m = SwitcherModel::new(10);
        m.record_open(3);
        m.record_open(2);
        m.update_query("1", &search);

        assert_eq!(m.results, vec![1]);
        // les récents sont conservés indépendamment de la requête
        assert_eq!(m.recents(), &[2, 3]);
    }

    #[test]
    fn limit_s_applique_aux_recents_correspondants() {
        let notes: &'static [usize] = &[0, 1, 2, 3];
        let search = fake_search(notes);
        let mut m = SwitcherModel::new(2);
        m.record_open(3);
        m.record_open(1);
        m.update_query("", &search);

        assert_eq!(m.results, vec![1, 3]);
    }
}
