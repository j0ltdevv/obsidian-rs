# Objectif

Documenter l'extension du modèle de données `CanvasBoard` (`vault::canvas_data`)
au-delà du canevas de base : regroupement des cartes en groupes nommés,
export/import JSON (roundtrip intégral) et auto-layout en grille. Cette spec
concerne uniquement la couche de données pures — aucune UI.

## Comportements

### 1. Groupes de cartes

1. Une carte porte un champ `group: Option<String>` (par défaut `None`),
   initialisé dans `Card::new`/`Card::new_at`. — INFERRED
2. `Card::set_group(Option<String>)` change (ou retire) le groupe ;
   `Card::group()` retourne `Option<&str>` (le groupe, s'il existe). —
   INFERRED
3. `CanvasBoard::cards_in_group(group)` retourne les cartes du groupe donné,
   triées par id croissant ; un groupe inexistant retourne une liste vide. —
   INFERRED
4. `CanvasBoard::groups()` retourne les groupes distincts présents, triés
   alphabétiquement et sans doublons. — INFERRED

### 2. Export/import JSON

5. `Card`, `CanvasEdge` et `CanvasBoard` implémentent `Serialize`/
   `Deserialize` (serde). — INFERRED
6. La couleur (`iced::Color`) n'est pas sérialisable telle quelle : elle est
   convertie en triplet RGB `[r, g, b]` de `u8` (attributs
   `#[serde(skip_serializing_if = "Option::is_none", default)]`), et
   reconstruite via `Color::from_rgb8`. — INFERRED
7. Le champ `group` d'une carte est optionnel en JSON : s'il est absent, la
   carte est chargée avec `group = None`. — INFERRED
8. `CanvasBoard::to_json()` sérialise le canevas (cartes triées par id, puis
   arêtes dans l'ordre d'insertion) ; `next_id` n'est pas persisté. — INFERRED
9. `CanvasBoard::from_json(json)` déserialise un canevas : les id des cartes
   et arêtes sont conservés à l'identique, et `next_id` est recalculé pour
   rester supérieur à tout id existant (0 si le canevas est vide). — INFERRED
10. Le roundtrip `from_json(to_json(board)) == board` est garanti pour un
    canevas quelconque (cartes, arêtes, couleurs, groupes, étiquettes). —
    INFERRED

### 3. Auto-layout en grille

11. `CanvasBoard::auto_layout()` réorganise toutes les cartes en grille :
    colonnes de largeur `DEFAULT_CARD_WIDTH`, lignes de hauteur
    `DEFAULT_CARD_HEIGHT`, espacement `AUTO_LAYOUT_SPACING` (24 px), avec
    `AUTO_LAYOUT_COLUMNS` (4) colonnes. — INFERRED
12. Les cartes sont placées ligne par ligne dans l'ordre croissant de leurs id
    (ordre préservé), sans que deux rectangles ne se chevauchent. — INFERRED

## Limites et hors périmètre

- La couleur est persistée en RGB 8 bits sans canal alpha : une couleur
  translucide devient opaque après un roundtrip JSON.
- `auto_layout` ne conserve ni l'ordre z ni les positions personnalisées ; il
  écrase toutes les positions.
- Aucune validation d'intégrité à l'import : un JSON avec des id dupliqués
  n'est pas détecté (le dernier gagne, `next_id` reste cohérent).
- Le nombre de colonnes de la grille est une constante (`AUTO_LAYOUT_COLUMNS`)
  indépendante de la largeur disponible du canevas.

## Label

Phase 3 : tous les comportements ci-dessus sont déduits du module
`vault::canvas_data` (testé par les tests unitaires du module), sans observation
REA — chacun est étiqueté **INFERRED**.
