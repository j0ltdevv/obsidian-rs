# Objectif

Documenter les canevas (canvas) : la structure de données `CanvasBoard` (cartes
identifiées, arêtes, tests d'intersection) et la vue iced qui les rend sous forme
de canevas (`iced::widget::canvas`), où chaque carte est un rectangle cliquable
portant son titre.

## Comportements

### 1. `CanvasBoard` — cartes et identifiants

1. Un canevas est vide à la création : `card_count() == 0`, `edge_count() == 0`,
   `hit_test` retourne `None`. — INFERRED
2. `CardId` est un entier `u64` opaque (`pub struct CardId(pub u64)`), stable et
   indépendant de l'ordre d'insertion ; il sert de clé d'indexation. — INFERRED
3. `add_card(title)` attribue l'identifiant `n` (avec `n` = nombre de cartes
   déjà présentes), puis incrémente le compteur interne : les ids sont donc
   croissants et démarrent à `0` (`CardId(0)`, `CardId(1)`…). — INFERRED
4. La position par défaut d'une nouvelle carte est `(n * CASCADE_STEP, n *
   CASCADE_STEP)` avec `CASCADE_STEP = 20.0`, ce qui décale en diagonale chaque
   carte par rapport aux précédentes ; les dimensions par défaut sont
   `DEFAULT_CARD_WIDTH = 200.0` et `DEFAULT_CARD_HEIGHT = 100.0`. — INFERRED
5. `add_card_at(title, x, y, width, height)` crée une carte à une position et
   des dimensions explicites, avec le même schéma d'id croissant. — INFERRED
6. Les titres vides sont acceptés (`add_card("")` crée une carte au titre vide).
   — INFERRED
7. `cards()` retourne les cartes triées par id croissant ; `edges()` retourne
   les arêtes dans l'ordre d'insertion. — INFERRED

### 2. `CanvasBoard` — arêtes

8. `connect(from, to)` crée une arête orientée `from → to` (sans étiquette). —
   INFERRED
9. Les arêtes sont dédoublonnées : appeler deux fois `connect` avec la même paire
   orientée est idempotent (`edge_count` reste à 1). — INFERRED
10. Une boucle (`from == to`) est refusée : `Err(CanvasError::SelfLoop(from))`,
    sans arête créée. — INFERRED
11. Une carte inconnue (source ou cible) est refusée :
    `Err(CanvasError::CardNotFound(id))` avec l'id de la carte manquante, sans
    arête créée. — INFERRED
12. `set_edge_label(from, to, label)` ajoute/remplace l'étiquette d'une arête
    existante (`Ok(true)`), retourne `Ok(false)` si l'arête n'existe pas, et
    `Err(CanvasError::CardNotFound)` si l'une des cartes est inconnue. — INFERRED
13. `disconnect(from, to)` retire l'arête orientée `from → to` et retourne le
    nombre d'arêtes retirées (0 ou 1, les doublons étant impossibles). — INFERRED
14. `remove_card(id)` retire la carte et toutes ses arêtes incidentes (entrantes
    et sortantes) ; une carte inconnue ne fait rien (`None`). — INFERRED

### 3. `CanvasBoard` — `hit_test`

15. `hit_test(point)` retourne le `CardId` de la carte dont le rectangle contient
    le point, ou `None` si aucune carte n'est touchée. — INFERRED
16. Les bords du rectangle sont inclusifs (le coin supérieur gauche `(x, y)` et
    le coin inférieur droit `(x + width, y + height)` comptent comme
    appartenant à la carte). — INFERRED
17. Un point hors de toute carte (loin ou négatif) retourne `None`. — INFERRED
18. En cas de chevauchement, c'est la carte la plus récente (id le plus élevé)
    qui est retournée, conformément à l'ordre z du dessin. — INFERRED

### 4. Cartes : rectangle + titre

19. Une carte est un rectangle positionné (`x`, `y`, `width`, `height`) portant
    un titre et un contenu textuel ; sa couleur de fond est optionnelle
    (`Option<Color>`, par défaut absente — la couleur du thème s'applique). —
    INFERRED
20. L'ordre d'insertion définit l'ordre z : une carte ajoutée plus tard est
    dessinée par-dessus les précédentes, et `hit_test` renvoie donc la carte la
    plus récente sous le curseur. — INFERRED
21. Les dimensions négatives sont ramenées à 0 par `set_size`. — INFERRED
22. Le titre, la position, la taille, le contenu et la couleur peuvent être
    modifiés après création, chacun retournant une erreur `CardNotFound` si
    l'identifiant est inconnu. — INFERRED

### 5. Vue graphe — rendu dans un canevas iced

23. La vue graphe (`ui::graph::graph_view`) est rendue dans un
    `iced::widget::canvas` dont la taille est fournie par `responsive` : le
    canevas a besoin de ses dimensions réelles pour placer les éléments. —
    INFERRED
24. Chaque nœud est dessiné comme un cercle de rayon `NODE_RADIUS` (12.0) à la
    position calculée de la note, et porte son nom (texte centré sous le nœud,
    décalé de `NODE_RADIUS + 10.0`). — INFERRED
25. Les arêtes sont des segments entre les positions des deux nœuds qu'elles
    relient, tracés en arrière-plan. — INFERRED
26. Un nœud trop proche du bord inférieur du canevas n'affiche pas son nom, pour
    éviter qu'il ne soit coupé. — INFERRED
27. La scène est reconstruite à chaque vue (le canevas est recréé à chaque
    frame), ce qui est acceptable pour un graphe de petite taille. — INFERRED

### 6. Clics dans le canevas

28. Seuls les clics gauche sont traités (`iced::Event::Mouse::ButtonPressed` avec
    `Button::Left`) ; les autres événements sont ignorés. — INFERRED
29. Le clic est validé uniquement si le curseur est dans les limites du canevas
    (`cursor.position_over(bounds)`) et si `hit_test` trouve un élément. —
    INFERRED
30. L'élément touché est identifié par le même indice que le dessin (graph :
    `GraphScene::paths`), garantissant la cohérence entre rendu et interaction ;
    `Message::OpenNote(path)` est alors publié. — INFERRED
31. Si l'index est absent, la vue affiche « Index absent » au lieu du canevas. —
    INFERRED

## Limites et hors périmètre

- `CanvasBoard` est une structure de données pure, sans dépendance UI : elle ne
  sait ni dessiner ni gérer les clics ; le rendu et l'interaction sont dans
  `ui::graph.rs`.
- Les arêtes sont orientées (`from → to`) et non dédoublonnées dans le sens
  inverse : `connect(a, b)` et `connect(b, a)` créent deux arêtes distinctes.
- Pas de moteur de physique : le placement des nœuds est déterministe (anneaux
  par degré, passes de répulsion, recentrage du barycentre), mais il n'est pas
  persisté entre deux rafraîchissements.
- La feature `canvas` de iced est optionnelle ; elle est déjà déclarée dans
  `Cargo.toml` (`features = ["canvas"]`).
- La vue graphe ne gère pas le survol, le déplacement de nœud, le zoom ni la
  sélection multiple.

## Label

Phase 3 : tous les comportements ci-dessus sont déduits des modules
`vault::canvas_data` et `ui::graph`, sans observation REA — chacun est étiqueté
**INFERRED**.
