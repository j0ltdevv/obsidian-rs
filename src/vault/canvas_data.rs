//! Données d'un canevas (canvas) : cartes positionnées et arêtes entre elles.
//!
//! Un canevas est une collection de cartes (`Card`) disposées librement dans un
//! espace 2D, reliées par des arêtes (`CanvasEdge`). Les identifiants sont
//! stables (`CardId`, compteur interne) et indépendants de l'ordre d'insertion,
//! afin que les vues puissent les conserver entre deux rafraîchissements.
//!
//! L'ordre d'insertion définit l'ordre z : une carte ajoutée plus tard est
//! dessinée par-dessus les précédentes, et `hit_test` renvoie donc la carte
//! la plus récente sous le curseur.

use std::collections::HashMap;

use iced::{Color, Point};

/// Identifiant unique et stable d'une carte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CardId(pub u64);

impl CardId {
    /// Retourne l'identifiant sous forme d'entier.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// Dimensions et position d'une carte, en coordonnées du canevas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Le point est-il à l'intérieur du rectangle, bords compris ?
    fn contains(&self, point: Point) -> bool {
        let x = point.x;
        let y = point.y;
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

/// Largeur et hauteur par défaut d'une carte nouvellement créée.
pub const DEFAULT_CARD_WIDTH: f32 = 200.0;
/// Hauteur par défaut d'une carte nouvellement créée.
pub const DEFAULT_CARD_HEIGHT: f32 = 100.0;
/// Décalage entre deux cartes créées d'affilée, pour éviter le chevauchement
/// total des premières cartes.
pub const CASCADE_STEP: f32 = 20.0;

/// Erreur retournée par les opérations mutantes sur un canevas.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CanvasError {
    /// La carte référencée n'existe pas dans le canevas.
    #[error("carte introuvable : {0:?}")]
    CardNotFound(CardId),
    /// Une arête relie une carte à elle-même.
    #[error("arête réflexive sur la carte {0:?}")]
    SelfLoop(CardId),
}

/// Une carte du canevas : rectangle positionné, titre, couleur et contenu.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    /// Identifiant stable de la carte.
    pub id: CardId,
    /// Abscisse du coin supérieur gauche, en coordonnées du canevas.
    pub x: f32,
    /// Ordonnée du coin supérieur gauche, en coordonnées du canevas.
    pub y: f32,
    /// Largeur de la carte.
    pub width: f32,
    /// Hauteur de la carte.
    pub height: f32,
    /// Titre affiché en tête de la carte.
    pub title: String,
    /// Contenu textuel de la carte (corps).
    pub text: String,
    /// Couleur de fond, par défaut absente (couleur du thème).
    pub color: Option<Color>,
}

impl Card {
    /// Crée une carte aux dimensions par défaut, positionnée en `(0, 0)`.
    pub fn new(id: CardId, title: &str) -> Self {
        Self::new_at(id, title, 0.0, 0.0, DEFAULT_CARD_WIDTH, DEFAULT_CARD_HEIGHT)
    }

    /// Crée une carte à une position et des dimensions explicites.
    pub fn new_at(id: CardId, title: &str, x: f32, y: f32, width: f32, height: f32) -> Self {
        Card {
            id,
            x,
            y,
            width,
            height,
            title: title.to_string(),
            text: String::new(),
            color: None,
        }
    }

    /// Retourne le rectangle occupé par la carte.
    pub fn rect(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }

    /// Déplace la carte en `(x, y)`.
    pub fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    /// Redimensionne la carte ; une dimension négative est ramenée à 0.
    pub fn set_size(&mut self, width: f32, height: f32) {
        self.width = width.max(0.0);
        self.height = height.max(0.0);
    }

    /// Change le titre de la carte.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    /// Change le contenu de la carte.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
    }

    /// Change (ou retire) la couleur de fond.
    pub fn set_color(&mut self, color: Option<Color>) {
        self.color = color;
    }
}

/// Arête orientée (ou non) entre deux cartes, éventuellement étiquetée.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasEdge {
    /// Carte source de l'arête.
    pub from: CardId,
    /// Carte cible de l'arête.
    pub to: CardId,
    /// Étiquette optionnelle affichée le long de l'arête.
    pub label: Option<String>,
}

impl CanvasEdge {
    /// Crée une arête sans étiquette.
    pub fn new(from: CardId, to: CardId) -> Self {
        CanvasEdge {
            from,
            to,
            label: None,
        }
    }

    /// Crée une arête avec une étiquette.
    pub fn with_label(from: CardId, to: CardId, label: &str) -> Self {
        CanvasEdge {
            from,
            to,
            label: Some(label.to_string()),
        }
    }
}

/// Canevas : cartes positionnées et arêtes les reliant.
#[derive(Debug, Default, PartialEq)]
pub struct CanvasBoard {
    /// Cartes, indexées par identifiant.
    cards: HashMap<CardId, Card>,
    /// Identifiant à attribuer à la prochaine carte.
    next_id: u64,
    /// Arêtes, dans l'ordre d'insertion (dédoublonnées).
    edges: Vec<CanvasEdge>,
}

impl CanvasBoard {
    /// Crée un canevas vide.
    pub fn new() -> Self {
        CanvasBoard::default()
    }

    /// Nombre de cartes présentes.
    pub fn card_count(&self) -> usize {
        self.cards.len()
    }

    /// Nombre d'arêtes présentes.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Ajoute une carte aux dimensions par défaut, en cascade par rapport aux
    /// précédentes, et retourne son identifiant.
    ///
    /// La position est `(n * CASCADE_STEP, n * CASCADE_STEP)` où `n` est le
    /// nombre de cartes déjà présentes, ce qui évite que les premières cartes
    /// ne se superposent exactement.
    pub fn add_card(&mut self, title: &str) -> CardId {
        let id = CardId(self.next_id);
        self.next_id += 1;
        let offset = (self.cards.len() as u64) as f32 * CASCADE_STEP;
        let card = Card::new_at(
            id,
            title,
            offset,
            offset,
            DEFAULT_CARD_WIDTH,
            DEFAULT_CARD_HEIGHT,
        );
        self.cards.insert(id, card);
        id
    }

    /// Ajoute une carte à une position et des dimensions explicites.
    pub fn add_card_at(&mut self, title: &str, x: f32, y: f32, width: f32, height: f32) -> CardId {
        let id = CardId(self.next_id);
        self.next_id += 1;
        let card = Card::new_at(id, title, x, y, width, height);
        self.cards.insert(id, card);
        id
    }

    /// Rend une carte par son identifiant.
    pub fn card(&self, id: CardId) -> Option<&Card> {
        self.cards.get(&id)
    }

    /// Rend une carte mutable par son identifiant.
    pub fn card_mut(&mut self, id: CardId) -> Option<&mut Card> {
        self.cards.get_mut(&id)
    }

    /// Itère sur les cartes, dans l'ordre croissant des identifiants.
    pub fn cards(&self) -> Vec<&Card> {
        let mut cards: Vec<&Card> = self.cards.values().collect();
        cards.sort_by_key(|card| card.id.0);
        cards
    }

    /// Itère sur les arêtes, dans l'ordre d'insertion.
    pub fn edges(&self) -> &[CanvasEdge] {
        &self.edges
    }

    /// Retire une carte et les arêtes qui y sont incidentes.
    ///
    /// Retourne la carte retirée, ou `None` si l'identifiant est inconnu.
    /// Les arêtes incidentes (entrantes et sortantes) sont également retirées.
    pub fn remove_card(&mut self, id: CardId) -> Option<Card> {
        let card = self.cards.remove(&id)?;
        self.edges.retain(|edge| edge.from != id && edge.to != id);
        Some(card)
    }

    /// Relie deux cartes par une arête.
    ///
    /// Une arête déjà présente entre les mêmes deux cartes (même orientation)
    /// n'est pas dupliquée : l'appel est idempotent. Les arêtes sont orientées
    /// (`from` → `to`), et une arête réflexive est refusée.
    pub fn connect(&mut self, from: CardId, to: CardId) -> Result<(), CanvasError> {
        if from == to {
            return Err(CanvasError::SelfLoop(from));
        }
        if !self.cards.contains_key(&from) || !self.cards.contains_key(&to) {
            return Err(CanvasError::CardNotFound(
                if !self.cards.contains_key(&from) {
                    from
                } else {
                    to
                },
            ));
        }
        if !self
            .edges
            .iter()
            .any(|edge| edge.from == from && edge.to == to)
        {
            self.edges.push(CanvasEdge::new(from, to));
        }
        Ok(())
    }

    /// Ajoute ou remplace l'étiquette d'une arête existante.
    ///
    /// Retourne `Ok(true)` si l'arête a été trouvée et modifiée, `Ok(false)`
    /// si aucune arête `from` → `to` n'existe, ou une erreur si l'une des
    /// cartes est inconnue.
    pub fn set_edge_label(
        &mut self,
        from: CardId,
        to: CardId,
        label: Option<&str>,
    ) -> Result<bool, CanvasError> {
        if !self.cards.contains_key(&from) || !self.cards.contains_key(&to) {
            return Err(CanvasError::CardNotFound(
                if !self.cards.contains_key(&from) {
                    from
                } else {
                    to
                },
            ));
        }
        for edge in &mut self.edges {
            if edge.from == from && edge.to == to {
                edge.label = label.map(str::to_string);
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Retire l'arête allant de `from` à `to`, si elle existe.
    ///
    /// Retourne le nombre d'arêtes retirées (0 ou 1, les doublons étant
    /// impossibles).
    pub fn disconnect(&mut self, from: CardId, to: CardId) -> usize {
        let before = self.edges.len();
        self.edges
            .retain(|edge| !(edge.from == from && edge.to == to));
        before - self.edges.len()
    }

    /// Déplace une carte en `(x, y)`.
    ///
    /// Retourne `Ok(())` si la carte existe, une erreur sinon.
    pub fn move_card(&mut self, id: CardId, x: f32, y: f32) -> Result<(), CanvasError> {
        self.card_mut(id)
            .ok_or(CanvasError::CardNotFound(id))
            .map(|card| card.set_position(x, y))
    }

    /// Redimensionne une carte.
    pub fn resize_card(&mut self, id: CardId, width: f32, height: f32) -> Result<(), CanvasError> {
        self.card_mut(id)
            .ok_or(CanvasError::CardNotFound(id))
            .map(|card| card.set_size(width, height))
    }

    /// Change le titre d'une carte.
    pub fn set_card_title(&mut self, id: CardId, title: &str) -> Result<(), CanvasError> {
        self.card_mut(id)
            .ok_or(CanvasError::CardNotFound(id))
            .map(|card| card.set_title(title))
    }

    /// Change le contenu d'une carte.
    pub fn set_card_text(&mut self, id: CardId, text: &str) -> Result<(), CanvasError> {
        self.card_mut(id)
            .ok_or(CanvasError::CardNotFound(id))
            .map(|card| card.set_text(text))
    }

    /// Change la couleur de fond d'une carte.
    pub fn set_card_color(&mut self, id: CardId, color: Option<Color>) -> Result<(), CanvasError> {
        self.card_mut(id)
            .ok_or(CanvasError::CardNotFound(id))
            .map(|card| card.set_color(color))
    }

    /// Identifiant de la carte située sous le point donné, s'il y en a une.
    ///
    /// Les bords du rectangle sont considérés comme appartenant à la carte.
    /// Lorsque plusieurs cartes se superposent, c'est la plus récente (celle
    /// dont l'identifiant est le plus élevé) qui est retournée, conformément
    /// à l'ordre z du dessin. Un canevas vide, ou un point hors de toute
    /// carte, renvoie `None`.
    pub fn hit_test(&self, point: Point) -> Option<CardId> {
        self.cards
            .values()
            .filter(|card| card.rect().contains(point))
            .max_by_key(|card| card.id.0)
            .map(|card| card.id)
    }

    /// Rend les arêtes incidentes à une carte (entrantes et sortantes).
    pub fn incident_edges(&self, id: CardId) -> Option<Vec<&CanvasEdge>> {
        if !self.cards.contains_key(&id) {
            return None;
        }
        Some(
            self.edges
                .iter()
                .filter(|edge| edge.from == id || edge.to == id)
                .collect(),
        )
    }

    /// Rend les arêtes sortantes d'une carte.
    pub fn outgoing_edges(&self, id: CardId) -> Option<Vec<&CanvasEdge>> {
        if !self.cards.contains_key(&id) {
            return None;
        }
        Some(self.edges.iter().filter(|edge| edge.from == id).collect())
    }

    /// Rend les arêtes entrantes d'une carte.
    pub fn incoming_edges(&self, id: CardId) -> Option<Vec<&CanvasEdge>> {
        if !self.cards.contains_key(&id) {
            return None;
        }
        Some(self.edges.iter().filter(|edge| edge.to == id).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_board() -> CanvasBoard {
        let mut board = CanvasBoard::new();
        board.add_card("A");
        board.add_card("B");
        board.add_card("C");
        board
    }

    #[test]
    fn nouveau_canevas_est_vide() {
        let board = CanvasBoard::new();
        assert_eq!(board.card_count(), 0);
        assert_eq!(board.edge_count(), 0);
        assert_eq!(board.hit_test(Point::new(10.0, 10.0)), None);
    }

    #[test]
    fn add_card_attribue_des_ids_stables_et_incremente() {
        let mut board = CanvasBoard::new();
        let a = board.add_card("A");
        let b = board.add_card("B");
        assert_eq!(a, CardId(0));
        assert_eq!(b, CardId(1));
        assert_eq!(board.card_count(), 2);
    }

    #[test]
    fn add_card_place_les_premieres_en_cascade() {
        let mut board = CanvasBoard::new();
        let a = board.add_card("A");
        let b = board.add_card("B");
        assert_eq!(board.card(a).unwrap().x, 0.0);
        assert_eq!(board.card(a).unwrap().y, 0.0);
        assert_eq!(board.card(b).unwrap().x, CASCADE_STEP);
        assert_eq!(board.card(b).unwrap().y, CASCADE_STEP);
        assert_eq!(board.card(b).unwrap().width, DEFAULT_CARD_WIDTH);
        assert_eq!(board.card(b).unwrap().height, DEFAULT_CARD_HEIGHT);
    }

    #[test]
    fn add_card_at_positionne_et_dimensionne_explicitement() {
        let mut board = CanvasBoard::new();
        let id = board.add_card_at("X", 5.0, 7.0, 42.0, 13.0);
        let card = board.card(id).unwrap();
        assert_eq!(card.id, CardId(0));
        assert_eq!(card.x, 5.0);
        assert_eq!(card.y, 7.0);
        assert_eq!(card.width, 42.0);
        assert_eq!(card.height, 13.0);
        assert_eq!(card.title, "X");
    }

    #[test]
    fn add_card_accepte_les_titres_vides() {
        let mut board = CanvasBoard::new();
        let id = board.add_card("");
        assert_eq!(board.card(id).unwrap().title, "");
        assert_eq!(board.card_count(), 1);
    }

    #[test]
    fn card_et_card_mut_rendent_une_carte_par_son_id() {
        let board = make_board();
        assert_eq!(board.card(CardId(1)).unwrap().title, "B");
        assert!(board.card(CardId(99)).is_none());

        let mut board = make_board();
        board.card_mut(CardId(0)).unwrap().set_title("A2");
        assert_eq!(board.card(CardId(0)).unwrap().title, "A2");
    }

    #[test]
    fn cards_rend_les_cartes_triées_par_id() {
        let board = make_board();
        let ids: Vec<u64> = board.cards().iter().map(|c| c.id.0).collect();
        assert_eq!(ids, vec![0, 1, 2]);
    }

    #[test]
    fn hit_test_point_interieur_rend_la_carte() {
        let board = make_board();
        assert_eq!(board.hit_test(Point::new(10.0, 10.0)), Some(CardId(0)));
    }

    #[test]
    fn hit_test_bords_inclusifs() {
        let mut board = CanvasBoard::new();
        board.add_card("A"); // (0, 0, 200, 100), seule sur la planche
                             // coin supérieur gauche inclus
        assert_eq!(board.hit_test(Point::new(0.0, 0.0)), Some(CardId(0)));
        // bord droit inclus
        assert_eq!(board.hit_test(Point::new(200.0, 0.0)), Some(CardId(0)));
        // bord inférieur inclus
        assert_eq!(board.hit_test(Point::new(0.0, 100.0)), Some(CardId(0)));
        // coin inférieur droit inclus
        assert_eq!(board.hit_test(Point::new(200.0, 100.0)), Some(CardId(0)));
    }

    #[test]
    fn hit_test_point_hors_toute_carte_rend_none() {
        let board = make_board();
        assert_eq!(board.hit_test(Point::new(1000.0, 1000.0)), None);
        assert_eq!(board.hit_test(Point::new(-1.0, -1.0)), None);
    }

    #[test]
    fn hit_test_chevauchement_rend_la_plus_recente() {
        let mut board = CanvasBoard::new();
        // deux cartes superposées à la même position
        board.add_card_at("base", 0.0, 0.0, 100.0, 100.0);
        board.add_card_at("dessus", 0.0, 0.0, 100.0, 100.0);
        assert_eq!(board.hit_test(Point::new(50.0, 50.0)), Some(CardId(1)));
    }

    #[test]
    fn connect_cree_une_arete_et_rejette_les_cas_invalides() {
        let mut board = make_board();
        assert!(board.connect(CardId(0), CardId(1)).is_ok());
        assert_eq!(board.edge_count(), 1);
        assert_eq!(board.edges()[0].from, CardId(0));
        assert_eq!(board.edges()[0].to, CardId(1));
        assert_eq!(board.edges()[0].label, None);

        // idempotence : pas de doublon
        assert!(board.connect(CardId(0), CardId(1)).is_ok());
        assert_eq!(board.edge_count(), 1);

        // boucle sur soi-même refusée
        assert_eq!(
            board.connect(CardId(0), CardId(0)),
            Err(CanvasError::SelfLoop(CardId(0)))
        );
        assert_eq!(board.edge_count(), 1);

        // carte inconnue refusée
        assert_eq!(
            board.connect(CardId(0), CardId(99)),
            Err(CanvasError::CardNotFound(CardId(99)))
        );
    }

    #[test]
    fn set_edge_label_et_disconnect() {
        let mut board = make_board();
        board.connect(CardId(0), CardId(1)).unwrap();
        assert_eq!(
            board.set_edge_label(CardId(0), CardId(1), Some("lien")),
            Ok(true)
        );
        assert_eq!(board.edges()[0].label, Some("lien".to_string()));
        assert_eq!(board.set_edge_label(CardId(0), CardId(1), None), Ok(true));
        assert_eq!(board.edges()[0].label, None);

        // arête inexistante
        assert_eq!(
            board.set_edge_label(CardId(0), CardId(2), Some("x")),
            Ok(false)
        );
        assert_eq!(board.edge_count(), 1);

        // carte inconnue → erreur
        assert_eq!(
            board.set_edge_label(CardId(0), CardId(99), Some("x")),
            Err(CanvasError::CardNotFound(CardId(99)))
        );

        assert_eq!(board.disconnect(CardId(0), CardId(1)), 1);
        assert_eq!(board.edge_count(), 0);
        // déconnexion d'une arête absente
        assert_eq!(board.disconnect(CardId(0), CardId(1)), 0);
    }

    #[test]
    fn move_card_et_resize_card_modifient_la_carte() {
        let mut board = make_board();
        assert!(board.move_card(CardId(0), 42.0, 7.0).is_ok());
        let card = board.card(CardId(0)).unwrap();
        assert_eq!(card.x, 42.0);
        assert_eq!(card.y, 7.0);

        assert!(board.resize_card(CardId(0), 5.0, 8.0).is_ok());
        let card = board.card(CardId(0)).unwrap();
        assert_eq!(card.width, 5.0);
        assert_eq!(card.height, 8.0);

        // dimension négative ramenée à 0
        assert!(board.resize_card(CardId(0), -3.0, -1.0).is_ok());
        assert_eq!(board.card(CardId(0)).unwrap().width, 0.0);
        assert_eq!(board.card(CardId(0)).unwrap().height, 0.0);

        // carte inconnue → erreur
        assert_eq!(
            board.move_card(CardId(99), 1.0, 1.0),
            Err(CanvasError::CardNotFound(CardId(99)))
        );
    }

    #[test]
    fn set_card_title_text_et_color() {
        let mut board = make_board();
        assert!(board.set_card_title(CardId(0), "Nouveau titre").is_ok());
        assert!(board.set_card_text(CardId(0), "corps").is_ok());
        assert!(board.set_card_color(CardId(0), Some(Color::BLACK)).is_ok());

        let card = board.card(CardId(0)).unwrap();
        assert_eq!(card.title, "Nouveau titre");
        assert_eq!(card.text, "corps");
        assert_eq!(card.color, Some(Color::BLACK));

        assert_eq!(
            board.set_card_title(CardId(99), "x"),
            Err(CanvasError::CardNotFound(CardId(99)))
        );
    }

    #[test]
    fn remove_card_retire_la_carte_et_ses_aretes() {
        let mut board = make_board();
        board.connect(CardId(0), CardId(1)).unwrap();
        board.connect(CardId(1), CardId(2)).unwrap();
        board.connect(CardId(0), CardId(2)).unwrap();

        let removed = board.remove_card(CardId(1));
        assert!(removed.is_some());
        assert_eq!(board.card_count(), 2);
        assert_eq!(board.edge_count(), 1); // seule 0 -> 2 reste
        assert!(board.card(CardId(1)).is_none());

        // retirer une carte inconnue ne fait rien
        assert!(board.remove_card(CardId(99)).is_none());
        assert_eq!(board.card_count(), 2);
    }

    #[test]
    fn incident_edges_rend_les_aretes_d_une_carte() {
        let mut board = make_board();
        board.connect(CardId(0), CardId(1)).unwrap();
        board.connect(CardId(2), CardId(1)).unwrap();

        let inc = board.incident_edges(CardId(1)).unwrap();
        assert_eq!(inc.len(), 2);

        assert_eq!(board.outgoing_edges(CardId(1)).unwrap().len(), 0);
        assert_eq!(board.incoming_edges(CardId(1)).unwrap().len(), 2);

        // carte inconnue
        assert!(board.incident_edges(CardId(99)).is_none());
    }
}
