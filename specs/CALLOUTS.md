# Objectif

Documenter l'extraction des callouts — blocs de citation spéciaux `> [!TYPE]`
à la manière d'Obsidian — depuis un document Markdown, telle qu'implémentée
par le module `vault::callouts` (type `Callout`, fonction `extract_callouts`).

## Comportements

1. Un callout débute par une ligne de citation `> [!TYPE]` : le type est le
   texte entre `[!` et `]` (exclu), trimé et normalisé en **minuscules**
   (`> [!WARNING]` → `kind == "warning"`). Un type vide ne forme pas un
   callout. — INFERRED
2. La syntaxe `> [!TYPE]-` — tiret **immédiatement** après `]` — indique un
   callout replié (`collapsed: true`). Sans tiret, `collapsed` est faux. —
   INFERRED
3. Le **titre** optionnel est le reste de la ligne d'ouverture après `]`,
   trimé ; une ligne sans texte après `]` (ou après le tiret de repli) n'a
   pas de titre (`title == None`). — INFERRED
4. Le corps d'un callout est constitué des lignes de citation qui suivent
   l'ouverture : `> ` suivi du contenu, ou `>` seul pour une ligne vide de
   citation. Le marqueur `> ` est retiré de chaque ligne stockée. — INFERRED
5. Une ligne non-citation (texte ordinaire, ligne vide, fence, …) termine le
   callout. Le callout s'étend donc de sa ligne d'ouverture jusqu'à la
   première ligne non-citation, y compris la fin du document s'il n'est
   jamais fermé. — INFERRED
6. Une citation imbriquée (`> > texte`) à l'intérieur du corps est conservée
   telle quelle : seul le premier marqueur `> ` est retiré. — INFERRED
7. Les blockquotes ordinaires (sans `[!...]` juste après le préfixe `> `)
   ne produisent aucune entrée. — INFERRED
8. Les blocs de code sont ignorés : les fences (``` ou ~~~, fermées ou non,
   ainsi que les lignes qu'elles contiennent) et les lignes indentées de
   quatre espaces ou plus (ou commençant par une tabulation) ne peuvent ni
   ouvrir ni prolonger un callout. — INFERRED
9. Les indices `start_line` et `end_line` sont **0-based**, et `end_line`
   est l'index de la dernière ligne du callout (**inclus**). Un callout sans
   corps a `start_line == end_line`. — INFERRED
10. Deux callouts doivent être séparés par une ligne non-citation (par
    exemple une ligne vide) : sinon, la seconde ouverture est absorbée comme
    ligne de corps de la première. — INFERRED
11. Le marqueur `[!` doit apparaître immédiatement après le préfixe de
    citation : `> texte [!note]` n'est pas un callout. — INFERRED

## Limites et hors périmètre

- Pas de callouts imbriqués : une ligne `> [!TYPE]` à l'intérieur du corps
  d'un callout est une ligne de corps comme une autre, elle n'ouvre pas un
  nouveau callout.
- Pas de rendu ni de repli effectif : seuls l'état `collapsed` et les
  métadonnées sont extraits.
- Le type n'est pas validé contre une liste connue : n'importe quel texte non
  vide est accepté (normalisé en minuscules).
- Les lignes de corps sont stockées sans le préfixe `> `, mais leur
  indentation interne est conservée ; les blancs de fin de ligne sont retirés.
  Une ligne `>` seule devient une ligne vide dans le corps.
- Le marqueur de repli doit coller au `]` : `> [!note] -` donne un titre `-`,
  pas un callout replié.
- Le code indenté (quatre espaces ou plus, ou tabulation) est ignoré de la
  même façon que les fences, même s'il contient une ligne `> [!TYPE]`.

## Label

Phase 0 : tous les comportements ci-dessus sont déduits du format Obsidian
des callouts et de la structure Markdown des citations, sans observation REA —
chacun est étiqueté **INFERRED**.
