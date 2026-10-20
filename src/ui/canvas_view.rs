//! Vue Canvas : planche de cartes (une par note) reliées par les wikilinks
//! résolus.
//!
//! Les cartes sont dessinées avec un `iced::widget::canvas` : rectangles
//! arrondis avec titre, arêtes entre cartes reliées, clic sur une carte →
//! `Message::OpenNote(chemin)`. Les positions sont stables (déterministes)
//! d'un rendu à l'autre : chaque carte occupe la position de la note
//! correspondante (une cascade depuis (0, 0)).

use std::collections::HashMap;
use std::path::PathBuf;

use iced::widget::canvas;
use iced::widget::canvas::{Frame, Path, Program, Stroke};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size};

use crate::app::Message;
use crate::vault::canvas_data::{CanvasBoard, CardId};

/// Largeur d'une carte.
const CARD_WIDTH: f32 = 160.0;
/// Hauteur d'une carte.
const CARD_HEIGHT: f32 = 72.0;
/// Couleur de fond des cartes.
const CARD_BG: Color = Color::from_rgb(0.92, 0.94, 0.97);
/// Couleur du cadre des cartes.
const CARD_BORDER: Color = Color::from_rgb(0.6, 0.68, 0.78);
/// Couleur du titre.
const CARD_TITLE: Color = Color::from_rgb(0.15, 0.28, 0.45);
/// Couleur des arêtes.
const EDGE_COLOR: Color = Color::from_rgb(0.7, 0.75, 0.8);
/// Épaisseur des arêtes.
const EDGE_WIDTH: f32 = 1.5;
/// Rayon d'arrondi des cartes.
const CORNER_RADIUS: f32 = 6.0;

/// Scène : positions, titres, chemins et arêtes.
#[derive(Debug, Clone)]
pub struct CanvasScene {
    /// id de carte → position (coin supérieur gauche).
    positions: HashMap<CardId, Point>,
    /// Titre de chaque carte.
    titles: HashMap<CardId, String>,
    /// Chemin relatif de la note représentée par chaque carte.
    paths: HashMap<CardId, PathBuf>,
    /// Arêtes (paires de CardId).
    edges: Vec<(CardId, CardId)>,
}

impl CanvasScene {
    /// Construit la scène depuis la planche et les dimensions du canevas.
    pub fn layout(board: &CanvasBoard, size: Size) -> Self {
        let mut positions = HashMap::new();
        let mut titles = HashMap::new();
        let paths = HashMap::new();
        for card in board.cards() {
            positions.insert(card.id, Point::new(card.x, card.y));
            titles.insert(card.id, card.title.clone());
        }
        // Recentrage : décale tout le bloc pour que le contenu apparaisse
        // centré quand il est plus petit que le canevas.
        let max_x = positions
            .values()
            .map(|p| p.x + CARD_WIDTH)
            .fold(0.0_f32, f32::max);
        let off_x = ((size.width - max_x) / 2.0).max(0.0);
        if off_x > 0.0 {
            for point in positions.values_mut() {
                point.x += off_x;
            }
        }
        let edges = board.edges().iter().map(|e| (e.from, e.to)).collect();
        Self {
            positions,
            titles,
            paths,
            edges,
        }
    }

    /// Mémorise le chemin associé à chaque carte (indice → note).
    pub fn attach_paths(&mut self, paths: &[(u64, PathBuf)]) {
        for &(raw, ref path) in paths {
            self.paths.insert(CardId(raw), path.clone());
        }
    }

    /// Carte sous le curseur, ou `None`.
    fn card_at(&self, point: Point) -> Option<CardId> {
        self.positions.iter().find_map(|(&id, &origin)| {
            let rect = Rectangle::new(origin, Size::new(CARD_WIDTH, CARD_HEIGHT));
            rect.contains(point).then_some(id)
        })
    }
}

/// Programme canvas : dessine la scène et capte les clics.
struct CanvasProgram {
    scene: CanvasScene,
}

impl Program<Message> for CanvasProgram {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        // Arêtes d'abord (sous les cartes).
        for (from, to) in &self.scene.edges {
            let (Some(&a), Some(&b)) =
                (self.scene.positions.get(from), self.scene.positions.get(to))
            else {
                continue;
            };
            let a = Point::new(a.x + CARD_WIDTH / 2.0, a.y + CARD_HEIGHT / 2.0);
            let b = Point::new(b.x + CARD_WIDTH / 2.0, b.y + CARD_HEIGHT / 2.0);
            let path = Path::line(a, b);
            frame.stroke(
                &path,
                Stroke::default()
                    .with_color(EDGE_COLOR)
                    .with_width(EDGE_WIDTH),
            );
        }

        // Cartes.
        for (id, &origin) in &self.scene.positions {
            let size = Size::new(CARD_WIDTH, CARD_HEIGHT);
            let path = Path::new(|builder| {
                builder.rounded_rectangle(origin, size, CORNER_RADIUS.into());
            });
            frame.fill(&path, CARD_BG);
            frame.stroke(
                &path,
                Stroke::default().with_color(CARD_BORDER).with_width(1.0),
            );
            if let Some(title) = self.scene.titles.get(id) {
                frame.fill_text(canvas::Text {
                    content: title.clone(),
                    position: Point::new(origin.x + 8.0, origin.y + 8.0),
                    size: 13.0.into(),
                    color: CARD_TITLE,
                    ..Default::default()
                });
            }
        }

        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if !matches!(
            event,
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left))
        ) {
            return None;
        }
        let point = cursor.position_over(bounds)?;
        let id = self.scene.card_at(point)?;
        let path = self.scene.paths.get(&id)?.clone();
        Some(canvas::Action::publish(Message::OpenNote(path)))
    }
}

/// Vue canvas : rend la planche.
///
/// `report` associe à chaque carte (par son id numérique, aligné sur l'indice
/// de la note) le chemin de la note à ouvrir au clic.
pub fn canvas_board_view<'a>(
    board: &'a CanvasBoard,
    report: &[(u64, PathBuf)],
) -> Element<'a, Message> {
    let mut scene = CanvasScene::layout(board, Size::new(800.0, 600.0));
    scene.attach_paths(report);
    canvas(CanvasProgram { scene })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
