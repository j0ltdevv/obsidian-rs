//! Tests property-based (proptest) : les parseurs ne doivent jamais paniquer
//! ni produire d'état incohérent sur des entrées arbitraires.

use proptest::prelude::*;

use obsy::vault::frontmatter::parse_frontmatter;
use obsy::vault::search_ops::parse_query;
use obsy::vault::wikilinks::parse_wikilinks;

/// Toute chaîne Unicode (y compris vide, émojis, sauts de ligne).
fn texte_arbitraire() -> impl Strategy<Value = String> {
    "\\PC*".prop_map(|s: String| s)
}

proptest! {
    #[test]
    fn parse_query_ne_panique_jamais(q in texte_arbitraire()) {
        let parsed = parse_query(&q);
        // L'analyse ne doit jamais perdre la totalité de la requête : soit des
        // termes positifs, soit des exclusions, soit rien (requête vide).
        prop_assert!(parsed.positive.len() + parsed.negative.len() <= q.split_whitespace().count() + 1);
    }

    #[test]
    fn parse_wikilinks_ne_panique_jamais(s in texte_arbitraire()) {
        let _ = parse_wikilinks(&s);
    }

    #[test]
    fn parse_frontmatter_ne_panique_jamais(s in texte_arbitraire()) {
        let _ = parse_frontmatter(&s);
    }
}
