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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

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
