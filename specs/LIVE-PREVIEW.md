# Objectif

Documenter l'extension du moteur de rendu Markdown (`live_preview.rs`) aux
segments stylés de callouts, maths et embeds, en réutilisant les modules de
parsing existants (`callouts`, `math`, `embeds`).

## Comportements

1. `SegmentStyle` gagne `Callout`, `CalloutTitle`, `MathInline`, `MathDisplay`, `Embed`, `EmbedImage`, `EmbedPdf` ; les variantes restent sans payload (Copy) et le texte du segment porte l'information affichable. — INFERRED
2. Une ligne appartenant à un callout (plage `start_line..=end_line` extraite par `callouts::extract_callouts`) est rendue en segments `Callout`, jamais en `Blockquote`. — INFERRED
3. La ligne d'ouverture d'un callout avec titre produit un segment `CalloutTitle` dont le texte est la ligne sans le préfixe `> ` (marqueur `[!type]` et tiret de repli conservés) ; sans titre, elle produit un segment `Callout`. — INFERRED
4. Chaque ligne de corps d'un callout produit un segment `Callout` dont le texte est la ligne sans le préfixe `> ` (retour à la ligne final retiré). — INFERRED
5. Une plage de maths display (`math::extract_math`) produit un segment `MathDisplay` unique, texte = source brute (sauts de ligne internes compris) ; les lignes de la plage sont consommées. — INFERRED
6. Les `$…$` inline d'une ligne ordinaire produisent des segments `MathInline` (texte = source) ; un `$` non fermé reste littéral ; `$$` ne déclenche pas l'inline. — INFERRED
7. Un embed `![[cible#section|alias]]` produit un segment `Embed` (texte = cible, sans `![[ ]]`, sans section ni alias), `EmbedImage` si la cible est une image, `EmbedPdf` si c'est un PDF — la classification réutilise `embeds::embed_kind`. — INFERRED
8. Les embeds ne sont pas confondus avec les wikilinks `[[…]]` existants ; `![alt](url)` (image markdown) n'est pas un embed. — INFERRED
9. Les blocs de code sont ignorés par les parsers de callouts, maths et embeds. — INFERRED
10. L'ordre des segments suit l'ordre du document ; la signature de `render_markdown` est inchangée. — INFERRED

## Limites et hors périmètre

- Aucun rendu iced : les segments sont des données pures, converties en widgets par l'UI (hors périmètre de ce module).
- L'alias et la section d'un embed ne sont pas reflétés dans le texte du segment (seule la cible l'est).
- L'état replié d'un callout est conservé dans le texte du titre (`[!type]-`) mais n'a pas de segment dédié.
- `$` échappé (`\$`) non traité.
- Les blocs de code à tildes `~~~` ne sont pas ignorés par le rendu inline (comportement existant, inchangé).

## Label

Phase 2 : comportements déduits des modules de parsing livrés et du format
Obsidian, sans observation REA — chacun est étiqueté **INFERRED**.
