//! Vue graphe du vault : nœuds = notes, arêtes = wikilinks résolus.
//!
//! Le graphe est dessiné avec un `iced::widget::canvas`. Les positions sont
//! calculées de façon déterministe (pas de moteur physique) :
//!   1. placement radial par degré décroissant sur des cercles concentriques,
//!   2. quelques passes de répulsion pour éloigner les nœuds proches,
//!   3. recentrage du barycentre sur le canevas.
//!
//! Les clics sont traités dans `canvas::Program::update` : la même fonction
//! de positionnement est réutilisée pour retrouver le nœud sous le curseur,
//! puis `Message::OpenNote` est émis avec le chemin de la note.

use std::collections::HashMap;
use std::path::PathBuf;

use iced::widget::canvas;
use iced::widget::{container, text};
use iced::{Color, Element, Font, Length, Point, Rectangle, Size};

use crate::app::{App, Message};
use crate::vault::graph::{build_graph, NodeId};
use crate::vault::index::NoteIndex;
use crate::vault::note::Note;

/// Rayon d'un nœud.
const NODE_RADIUS: f32 = 12.0;
/// Zone de clic autour d'un nœud (rayon + tolérance).
const HIT_RADIUS: f32 = NODE_RADIUS + 4.0;
/// Espacement minimal entre deux nœuds (après répulsion).
const MIN_NODE_DISTANCE: f32 = 34.0;
/// Nombre de passes de répulsion.
const REPULSION_PASSES: usize = 3;
/// Couleur des nœuds.
const NODE_COLOR: Color = Color::from_rgb(0.25, 0.45, 0.75);
/// Couleur des arêtes.
const EDGE_COLOR: Color = Color::from_rgb(0.7, 0.75, 0.8);
/// Épaisseur des arêtes.
const EDGE_WIDTH: f32 = 1.5;
/// Police des noms.
const LABEL_FONT: Font = Font::MONOSPACE;
/// Taille des noms (pixels).
const LABEL_SIZE: u16 = 11;
/// Rayon des cercles concentriques (part du rayon disponible).
const INNER_RING_FRACTION: f32 = 0.35;
const MIDDLE_RING_FRACTION: f32 = 0.65;
const OUTER_RING_FRACTION: f32 = 0.95;

/// Scène du graphe : positions des nœuds, noms, et arêtes.
#[derive(Debug, Clone)]
pub struct GraphScene {
    /// Positions des nœuds (indice de note → point dans le canevas).
    positions: HashMap<usize, Point>,
    /// Noms des notes (indice → nom).
    names: HashMap<usize, String>,
    /// Chemins des notes (indice → chemin relatif), pour l'ouverture au clic.
    paths: HashMap<usize, PathBuf>,
    /// Arêtes du graphe.
    edges: Vec<(NodeId, NodeId)>,
}

impl GraphScene {
    /// Construit la scène depuis le graphe et les dimensions du canevas.
    pub fn layout(notes: &[Note], index: &NoteIndex, size: Size) -> Self {
        let graph = build_graph(notes, index);
        let mut positions: HashMap<usize, Point> = HashMap::new();
        let mut names: HashMap<usize, String> = HashMap::new();
        let mut paths: HashMap<usize, PathBuf> = HashMap::new();

        if notes.is_empty() || size.width <= 0.0 || size.height <= 0.0 {
            return Self {
                positions,
                names,
                paths: HashMap::new(),
                edges: graph.edges,
            };
        }

        // Degrés des nœuds, triés décroissant (les plus connectés au centre).
        let mut ranked: Vec<usize> = (0..notes.len()).collect();
        ranked.sort_by_key(|&a| std::cmp::Reverse(graph.degree(a)));

        let center = Point::new(size.width / 2.0, size.height / 2.0);
        let available = size.width.min(size.height) / 2.0;

        // Anneaux : degré 0 → extérieur, degré 1 → milieu, degré ≥ 2 → intérieur.
        let ring_of = |degree: usize| match degree {
            0 => 2,
            1 => 1,
            _ => 0,
        };

        for (rank, &node) in ranked.iter().enumerate() {
            names.insert(node, notes[node].name.clone());
            paths.insert(node, notes[node].path.clone());
            let degree = graph.degree(node);
            let ring = ring_of(degree);
            let ring_radius = match ring {
                0 => available * INNER_RING_FRACTION,
                1 => available * MIDDLE_RING_FRACTION,
                _ => available * OUTER_RING_FRACTION,
            };
            // Position angulaire uniforme au sein de l'anneau.
            let count = ranked
                .iter()
                .filter(|&&n| ring_of(graph.degree(n)) == ring)
                .count();
            let angle = 2.0 * std::f32::consts::PI * (rank % count) as f32 / count as f32
                - std::f32::consts::FRAC_PI_2;
            positions.insert(
                node,
                Point::new(
                    center.x + ring_radius * angle.cos(),
                    center.y + ring_radius * angle.sin(),
                ),
            );
        }

        // Passes de répulsion : éloigne les nœuds trop proches.
        for _ in 0..REPULSION_PASSES {
            for i in 0..notes.len() {
                for j in (i + 1)..notes.len() {
                    let Some(a) = positions.get(&i) else {
                        continue;
                    };
                    let Some(b) = positions.get(&j) else {
                        continue;
                    };
                    let dx = b.x - a.x;
                    let dy = b.y - a.y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist > 0.0 && dist < MIN_NODE_DISTANCE {
                        let push = (MIN_NODE_DISTANCE - dist) * 0.5;
                        let nx = dx / dist * push;
                        let ny = dy / dist * push;
                        if let Some(ai) = positions.get_mut(&i) {
                            ai.x -= nx;
                            ai.y -= ny;
                        }
                        if let Some(bj) = positions.get_mut(&j) {
                            bj.x += nx;
                            bj.y += ny;
                        }
                    }
                }
            }
        }

        // Répulsion douce depuis le centre pour éviter les superpositions au
        // milieu (les nœuds de degré élevé ont tendance à s'accumuler).
        for _ in 0..REPULSION_PASSES {
            for pos in positions.values_mut() {
                let dx = pos.x - center.x;
                let dy = pos.y - center.y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > 0.0 && dist < 18.0 {
                    let push = (18.0 - dist) * 0.5;
                    let nx = dx / dist * push;
                    let ny = dy / dist * push;
                    pos.x += nx;
                    pos.y += ny;
                }
            }
        }

        // Recentrage du barycentre sur le centre du canevas.
        let n = positions.len() as f32;
        if n > 0.0 {
            let sum_x: f32 = positions.values().map(|p| p.x).sum();
            let sum_y: f32 = positions.values().map(|p| p.y).sum();
            let centroid = Point::new(sum_x / n, sum_y / n);
            for pos in positions.values_mut() {
                pos.x += center.x - centroid.x;
                pos.y += center.y - centroid.y;
            }
        }

        Self {
            positions,
            names,
            paths,
            edges: graph.edges,
        }
    }

    /// Retourne l'indice de la note sous le curseur, si un nœud est touché.
    pub fn hit_test(&self, point: Point) -> Option<usize> {
        for (&node, &pos) in &self.positions {
            let dx = point.x - pos.x;
            let dy = point.y - pos.y;
            if dx * dx + dy * dy <= HIT_RADIUS * HIT_RADIUS {
                return Some(node);
            }
        }
        None
    }

    /// Dessine le graphe (arêtes, nœuds, noms) dans le cadre donné.
    pub fn draw(&self, renderer: &iced::Renderer, bounds: Rectangle) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // Arêtes (en arrière-plan) : un segment par lien.
        for &(a, b) in &self.edges {
            if let (Some(pa), Some(pb)) = (self.positions.get(&a), self.positions.get(&b)) {
                frame.stroke(
                    &canvas::Path::line(*pa, *pb),
                    canvas::Stroke {
                        style: canvas::Style::Solid(EDGE_COLOR),
                        width: EDGE_WIDTH,
                        line_cap: canvas::LineCap::Round,
                        line_join: canvas::LineJoin::Round,
                        ..Default::default()
                    },
                );
            }
        }

        // Nœuds.
        for pos in self.positions.values() {
            frame.fill(
                &canvas::Path::circle(*pos, NODE_RADIUS),
                canvas::Fill {
                    style: canvas::Style::Solid(NODE_COLOR),
                    ..Default::default()
                },
            );
        }

        // Noms, centrés sous chaque nœud (décalés pour ne pas masquer le
        // nœud ; omis si le nœud est trop près du bord).
        for (node, pos) in &self.positions {
            if pos.y + 32.0 > bounds.y + bounds.height - 4.0 {
                continue;
            }
            let label = self.names.get(node).cloned().unwrap_or_default();
            frame.fill_text(canvas::Text {
                content: label,
                position: Point::new(pos.x, pos.y + NODE_RADIUS + 10.0),
                size: (LABEL_SIZE as f32).into(),
                font: LABEL_FONT,
                color: Color::from_rgb(0.3, 0.3, 0.35),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                ..Default::default()
            });
        }

        vec![frame.into_geometry()]
    }
}

/// Programme iced pour le canevas du graphe.
///
/// La scène est construite à chaque vue (le canevas est recréé à chaque
/// frame, ce qui est acceptable pour un graphe de petite taille).
#[derive(Debug)]
pub struct GraphProgram {
    scene: GraphScene,
}

impl GraphProgram {
    pub fn new(scene: GraphScene) -> Self {
        Self { scene }
    }
}

impl canvas::Program<Message> for GraphProgram {
    type State = ();

    /// Gère le clic gauche : ouvre la note dont le nœud est sous le curseur.
    /// Le chemin de la note est re-trouvé avec les mêmes positions que le
    /// dessin (`GraphScene::hit_test`), garantissant la cohérence.
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
        let node = self.scene.hit_test(point)?;
        let path = self.scene.paths.get(&node)?.clone();
        Some(canvas::Action::publish(Message::OpenNote(path)))
    }

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        self.scene.draw(renderer, bounds)
    }
}

/// Vue graphe : canevas (dessin + clics via `canvas::Program::update`).
pub fn graph_view<'a>(app: &'a App, notes: &'a [Note], size: Size) -> Element<'a, Message> {
    let Some(index) = &app.index else {
        return container(text("Index absent")).into();
    };
    let scene = GraphScene::layout(notes, index, size);

    let canvas = canvas(GraphProgram::new(scene))
        .width(Length::Fill)
        .height(Length::Fill);

    container(canvas)
        .style(super::panel_style)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
