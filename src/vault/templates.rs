//! Expansion de gabarits `{{variable}}`.
//!
//! Seul le marquage par doubles accolades est interprété. Les accolades simples,
//! les clés inconnues et les séquences `{{` non fermées sont recopiées telles
//! quelles, ce qui garantit que l'expansion est une passe unique et sans effet
//! de bord sur le contenu du document.
/// Remplace les variables `{{clé}}` de `tpl` par leur valeur.
///
/// # Règles
///
/// - Seule la forme `{{clé}}` est reconnue comme variable. Les accolades simples
///   `{` et `}` (par exemple dans du code ou des ensembles) sont inchangées.
/// - Une variable dont la clé n'est pas dans `vars` est conservée littéralement
///   dans le résultat.
/// - Une séquence `{{` sans `}}` fermante est conservée telle quelle.
/// - Les valeurs sont insérées telles quelles : elles ne sont **pas**
///   ré-évaluées, même si elles contiennent `{{...}}`.
/// - La recherche des clés est sensible à la casse ; en cas de clé dupliquée
///   dans `vars`, la première occurrence l'emporte.
///
/// # Règles
///
/// - Voir la doc complète ci-dessus pour le comportement.
///
/// Les exemples exécutables sont fournis sous forme de tests unitaires
/// (`#[cfg(test)]`) à la fin du fichier, conformément à la convention du repo.
pub fn expand_template(tpl: &str, vars: &[(String, String)]) -> String {
    let mut resultat = String::with_capacity(tpl.len());
    let mut reste = tpl;

    while let Some(ouverture) = reste.find("{{") {
        let interieur = &reste[ouverture + 2..];
        if let Some(fermeture) = interieur.find("}}") {
            let cle = &interieur[..fermeture];
            // La première correspondance l'emporte ; une clé absente laisse la
            // variable intacte (pas de suppression, pas de vide).
            if let Some(valeur) = vars.iter().find(|(k, _)| *k == cle) {
                // Recopie le texte qui précède la variable, puis sa valeur.
                resultat.push_str(&reste[..ouverture]);
                resultat.push_str(valeur.1.as_str());
            } else {
                // Inconnue : conserve `{{clé}}` dans son intégralité.
                resultat.push_str(&reste[..ouverture + 2 + fermeture + 2]);
            }

            // Passe après la variable fermante ; la valeur insérée n'est pas
            // re-parcourue, donc pas de ré-évaluation.
            reste = &reste[ouverture + 2 + fermeture + 2..];
        } else {
            // Pas de fermeture : garde `{{` et avance pour éviter une boucle
            // infinie sur une séquence comme `{{{`.
            resultat.push_str(&reste[..ouverture + 2]);
            reste = &reste[ouverture + 2..];
        }
    }

    resultat.push_str(reste);
    resultat
}

/// Variables par défaut injectées dans les gabarits de notes.
///
/// - `{{title}}` : le titre de la note.
/// - `{{date}}` : la date du jour (au format de l'appelant).
pub fn default_variables(title: &str, date: &str) -> Vec<(String, String)> {
    vec![
        ("title".to_string(), title.to_string()),
        ("date".to_string(), date.to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remplace_la_variable_par_sa_valeur() {
        let vars = [("name".into(), "Alice".into())];
        assert_eq!(expand_template("Bonjour {{name}}", &vars), "Bonjour Alice");
    }

    #[test]
    fn cle_inconnue_et_accolades_simples_conservees() {
        assert_eq!(expand_template("{{inconnu}} {x}", &[]), "{{inconnu}} {x}");
    }

    #[test]
    fn variables_par_defaut_title_et_date() {
        let par_defaut = default_variables("Accueil", "2026-10-20");
        assert_eq!(expand_template("# {{title}}", &par_defaut), "# Accueil");
        assert_eq!(expand_template("le {{date}}", &par_defaut), "le 2026-10-20");
    }

    #[test]
    fn sequence_ouverte_sans_fermeture_conservee() {
        assert_eq!(expand_template("a {{b", &[]), "a {{b");
    }

    #[test]
    fn valeur_non_reevaluee() {
        let vars = [("x".into(), "{{y}}".into())];
        assert_eq!(expand_template("{{x}}", &vars), "{{y}}");
    }

    #[test]
    fn premiere_occurrence_l_emporte() {
        let vars = [("k".into(), "v1".into()), ("k".into(), "v2".into())];
        assert_eq!(expand_template("{{k}}", &vars), "v1");
    }

    #[test]
    fn casse_sensible() {
        let vars = [("Name".into(), "X".into())];
        assert_eq!(expand_template("{{name}}", &vars), "{{name}}");
    }

    #[test]
    fn gabarit_vide() {
        assert_eq!(expand_template("", &[]), "");
    }
}
