//! Vue graphe du vault : nœuds = notes, arêtes = wikilinks résolus.
//!
//! Le graphe est dessiné avec un `iced::widget::canvas`. Les positions sont
//! calculées de façon déterministe (pas de moteur physique) :
//!   1. placement radial par degré décroissant sur des cercles concentriques,
//!   2. quelques passes de répulsion pour éloigner les nœuds proches,
//!   3. recentrage du barycentre sur le canevas.
//!
//! Le glisser-déposer des nœuds est traité dans `canvas::Program::update` :
//! la même fonction de positionnement est réutilisée pour retrouver le nœud
//! sous le curseur (`Message::GraphDragStart` / `GraphDragApply`).

use std::collections::HashMap;

use iced::widget::canvas;
use iced::widget::{button, container, text, Row};
use iced::{Color, Element, Font, Length, Point, Rectangle, Size};

use crate::app::{App, Message};
use crate::vault::graph::{build_graph, GraphFilters, NodeId};
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

/// Couleurs de groupes (par dossier) pour les nœuds.
const FOLDER_PALETTE: [Color; 6] = [
    Color::from_rgb(0.25, 0.45, 0.75),
    Color::from_rgb(0.75, 0.40, 0.25),
    Color::from_rgb(0.25, 0.60, 0.40),
    Color::from_rgb(0.60, 0.35, 0.65),
    Color::from_rgb(0.70, 0.60, 0.20),
    Color::from_rgb(0.35, 0.50, 0.70),
];

/// Clé de groupe d'une note : son dossier parent ("" pour la racine).
fn folder_key(note: &Note) -> String {
    note.path
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_default()
}

/// Couleur déterministe d'un groupe (dossier).
fn folder_color(key: &str) -> Color {
    let mut hash: u32 = 5381;
    for b in key.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(u32::from(b));
    }
    FOLDER_PALETTE[(hash as usize) % FOLDER_PALETTE.len()]
}

/// Arêtes dont les deux extrémités sont présentes dans `positions`.
fn visible_edges(
    edges: &[(NodeId, NodeId)],
    positions: &HashMap<usize, Point>,
) -> Vec<(NodeId, NodeId)> {
    edges
        .iter()
        .copied()
        .filter(|(a, b)| positions.contains_key(a) && positions.contains_key(b))
        .collect()
}

/// Options de vue du graphe (zoom, pan, affichage).
#[derive(Debug, Clone, Copy)]
pub struct ViewOpts {
    /// Facteur de zoom (1.0 = 100 %).
    pub zoom: f32,
    /// Décalage de pan (px).
    pub pan: (f32, f32),
    /// Affiche les noms.
    pub show_labels: bool,
    /// Affiche un quadrillage.
    pub show_grid: bool,
}

#[derive(Debug, Clone)]
pub struct GraphScene {
    /// Positions des nœuds (indice de note → point dans le canevas).
    positions: HashMap<usize, Point>,
    /// Noms des notes (indice → nom).
    names: HashMap<usize, String>,
    /// Arêtes du graphe.
    edges: Vec<(NodeId, NodeId)>,
    /// Couleurs de groupe par nœud (dossier).
    colors: HashMap<usize, Color>,
    /// Facteur de zoom (1.0 = 100 %).
    zoom: f32,
    /// Décalage de pan (px).
    pan: (f32, f32),
    /// Affiche les noms.
    show_labels: bool,
    /// Affiche un quadrillage.
    show_grid: bool,
}

impl GraphScene {
    /// Construit la scène depuis le graphe, les dimensions du canevas et les
    /// filtres d'affichage (graphe local, tag, dossier). Les décalages
    /// persistants de glisser-déposer (`drags`) sont appliqués aux positions
    /// finales des nœuds.
    pub fn layout(
        notes: &[Note],
        index: &NoteIndex,
        size: Size,
        filters: &GraphFilters,
        drags: &HashMap<usize, (f32, f32)>,
        mtimes: &std::collections::HashMap<std::path::PathBuf, std::time::SystemTime>,
        view: &ViewOpts,
    ) -> Self {
        let graph = build_graph(notes, index);
        let mut positions: HashMap<usize, Point> = HashMap::new();
        let mut names: HashMap<usize, String> = HashMap::new();
        let mut colors: HashMap<usize, Color> = HashMap::new();

        if notes.is_empty() || size.width <= 0.0 || size.height <= 0.0 {
            let edges = visible_edges(&graph.edges, &positions);
            return Self {
                positions,
                names,
                colors,
                edges,
                zoom: view.zoom,
                pan: view.pan,
                show_labels: view.show_labels,
                show_grid: view.show_grid,
            };
        }

        // Notes visibles selon les filtres.
        let allowed: Vec<usize> = (0..notes.len())
            .filter(|&i| filters.allows(&graph, notes, i, mtimes))
            .collect();
        if allowed.is_empty() {
            let edges = Vec::new();
            return Self {
                positions,
                names,
                colors,
                edges,
                zoom: view.zoom,
                pan: view.pan,
                show_labels: view.show_labels,
                show_grid: view.show_grid,
            };
        }

        // Degrés des nœuds visibles, triés décroissant (connectés au centre).
        let mut ranked = allowed.clone();
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
            colors.insert(node, folder_color(&folder_key(&notes[node])));
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
            for i in 0..allowed.len() {
                for j in (i + 1)..allowed.len() {
                    let Some(a) = positions.get(&allowed[i]) else {
                        continue;
                    };
                    let Some(b) = positions.get(&allowed[j]) else {
                        continue;
                    };
                    let dx = b.x - a.x;
                    let dy = b.y - a.y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist > 0.0 && dist < MIN_NODE_DISTANCE {
                        let push = (MIN_NODE_DISTANCE - dist) * 0.5;
                        let nx = dx / dist * push;
                        let ny = dy / dist * push;
                        if let Some(ai) = positions.get_mut(&allowed[i]) {
                            ai.x -= nx;
                            ai.y -= ny;
                        }
                        if let Some(bj) = positions.get_mut(&allowed[j]) {
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

        // Décalages persistants du glisser-déposer : appliqués après le
        // placement, pour que `hit_test` et le dessin utilisent les positions
        // finales (offsets additifs, déjà stockés par l'application).
        for (node, (dx, dy)) in drags {
            if let Some(pos) = positions.get_mut(node) {
                pos.x += dx;
                pos.y += dy;
            }
        }

        let edges = visible_edges(&graph.edges, &positions);
        Self {
            positions,
            names,
            colors,
            edges,
            zoom: view.zoom,
            pan: view.pan,
            show_labels: view.show_labels,
            show_grid: view.show_grid,
        }
    }

    /// Convertit une coordonnée « monde » en coordonnée écran (zoom + pan).
    fn to_screen(&self, p: Point) -> Point {
        Point::new(p.x * self.zoom + self.pan.0, p.y * self.zoom + self.pan.1)
    }

    /// Convertit une coordonnée écran en coordonnée « monde ».
    fn to_world(&self, p: Point) -> Point {
        Point::new(
            (p.x - self.pan.0) / self.zoom,
            (p.y - self.pan.1) / self.zoom,
        )
    }

    /// Retourne l'indice de la note sous le curseur, si un nœud est touché.
    pub fn hit_test(&self, point: Point) -> Option<usize> {
        let world = self.to_world(point);
        for (&node, &pos) in &self.positions {
            let dx = world.x - pos.x;
            let dy = world.y - pos.y;
            if dx * dx + dy * dy <= HIT_RADIUS * HIT_RADIUS {
                return Some(node);
            }
        }
        None
    }

    /// Dessine le graphe (arêtes, nœuds, noms) dans le cadre donné. Si un
    /// glisser-déposer est en cours (`dragged`), le nœud concerné est dessiné
    /// à sa position décalée.
    pub fn draw(
        &self,
        renderer: &iced::Renderer,
        bounds: Rectangle,
        dragged: Option<DragState>,
    ) -> Vec<canvas::Geometry> {
        let dragged_delta = dragged.map(|d| (d.node, d.delta));
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // Quadrillage optionnel (espacement fixe à l'écran).
        if self.show_grid {
            let step = 40.0;
            let mut x = self.pan.0 % step;
            while x < bounds.width {
                if x > 0.0 {
                    frame.stroke(
                        &canvas::Path::line(Point::new(x, 0.0), Point::new(x, bounds.height)),
                        canvas::Stroke {
                            style: canvas::Style::Solid(Color::from_rgba(0.5, 0.5, 0.5, 0.15)),
                            width: 1.0,
                            ..Default::default()
                        },
                    );
                }
                x += step;
            }
            let mut y = self.pan.1 % step;
            while y < bounds.height {
                if y > 0.0 {
                    frame.stroke(
                        &canvas::Path::line(Point::new(0.0, y), Point::new(bounds.width, y)),
                        canvas::Stroke {
                            style: canvas::Style::Solid(Color::from_rgba(0.5, 0.5, 0.5, 0.15)),
                            width: 1.0,
                            ..Default::default()
                        },
                    );
                }
                y += step;
            }
        }

        // Arêtes (en arrière-plan) : un segment par lien.
        for &(a, b) in &self.edges {
            if let (Some(pa), Some(pb)) = (self.positions.get(&a), self.positions.get(&b)) {
                frame.stroke(
                    &canvas::Path::line(self.to_screen(*pa), self.to_screen(*pb)),
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

        // Nœuds, colorés par groupe (dossier).
        for (node, pos) in &self.positions {
            let base = match dragged_delta {
                Some((dragged_node, delta)) if dragged_node == *node => {
                    Point::new(pos.x + delta.x / self.zoom, pos.y + delta.y / self.zoom)
                }
                _ => *pos,
            };
            let draw_pos = self.to_screen(base);
            let color = self.colors.get(node).copied().unwrap_or(NODE_COLOR);
            frame.fill(
                &canvas::Path::circle(draw_pos, NODE_RADIUS * self.zoom),
                canvas::Fill {
                    style: canvas::Style::Solid(color),
                    ..Default::default()
                },
            );
        }

        // Noms, si activés.
        if self.show_labels {
            for (node, pos) in &self.positions {
                let base = match dragged_delta {
                    Some((dragged_node, delta)) if dragged_node == *node => {
                        Point::new(pos.x + delta.x / self.zoom, pos.y + delta.y / self.zoom)
                    }
                    _ => *pos,
                };
                let draw_pos = self.to_screen(base);
                if draw_pos.y + 32.0 > bounds.height - 4.0 {
                    continue;
                }
                let label = self.names.get(node).cloned().unwrap_or_default();
                frame.fill_text(canvas::Text {
                    content: label,
                    position: Point::new(draw_pos.x, draw_pos.y + NODE_RADIUS * self.zoom + 10.0),
                    size: (LABEL_SIZE as f32 * self.zoom).into(),
                    font: LABEL_FONT,
                    color: Color::from_rgb(0.3, 0.3, 0.35),
                    align_x: iced::alignment::Horizontal::Center.into(),
                    align_y: iced::alignment::Vertical::Center,
                    ..Default::default()
                });
            }
        }

        vec![frame.into_geometry()]
    }
}

// État de session d'un glisser-déposer de nœud.
#[derive(Debug, Clone, Copy)]
pub struct DragState {
    /// Nœud déplacé (usize::MAX si pan).
    node: usize,
    /// Position du curseur au moment du clic (dans le canevas).
    origin: Point,
    /// Décalage courant du curseur depuis l'origine.
    delta: Point,
    /// Vrai si c'est un pan (glisser sur le fond), faux si nœud.
    is_pan: bool,
}

// Programme iced pour le canevas du graphe.
//
// La scène est reconstruite à chaque vue ; le programme persistant est
// conservé dans `App::graph_program` (la scène y est remplacée via
// `update_scene`). L'état du glisser-déposer en cours vit dans le
// `State` du widget canevas, que iced préserve entre deux frames.
#[derive(Debug, Clone)]
pub struct GraphProgram {
    scene: GraphScene,
}

impl GraphProgram {
    pub fn new(scene: GraphScene) -> Self {
        Self { scene }
    }

    /// Remplace la scène (positions recalées à chaque frame).
    pub fn update_scene(&mut self, scene: GraphScene) {
        self.scene = scene;
    }
}

impl canvas::Program<Message> for GraphProgram {
    type State = Option<DragState>;

    /// Gère le glisser-déposer des nœuds :
    /// - clic gauche sur un nœud : début du drag (événement capturé +
    ///   `Message::GraphDragStart`) ;
    /// - mouvement de curseur pendant un drag : mise à jour locale du
    ///   décalage, avec simple demande de redessin (pas de message par frame,
    ///   pour éviter les boucles) ;
    /// - relâchement : fin du drag (`Message::GraphDragApply` avec le
    ///   décalage total, additif côté application).
    fn update(
        &self,
        drag: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                // Coordonnées locales au canevas (le curseur est global).
                let p = cursor.position_over(bounds)?;
                let point = Point::new(p.x - bounds.x, p.y - bounds.y);
                let node = self.scene.hit_test(point);
                let is_pan = node.is_none();
                let node_id = node.unwrap_or(usize::MAX);
                *drag = Some(DragState {
                    node: node_id,
                    origin: point,
                    delta: Point::new(0.0, 0.0),
                    is_pan,
                });
                if is_pan {
                    Some(canvas::Action::request_redraw().and_capture())
                } else {
                    Some(
                        canvas::Action::publish(Message::GraphDragStart {
                            node: node_id,
                            origin: (point.x, point.y),
                        })
                        .and_capture(),
                    )
                }
            }
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                let d = drag.as_mut()?;
                let local = Point::new(position.x - bounds.x, position.y - bounds.y);
                d.delta = Point::new(local.x - d.origin.x, local.y - d.origin.y);
                Some(canvas::Action::request_redraw())
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                let d = drag.take()?;
                if d.is_pan {
                    Some(
                        canvas::Action::publish(Message::GraphPan {
                            dx: d.delta.x,
                            dy: d.delta.y,
                        })
                        .and_capture(),
                    )
                } else {
                    Some(
                        canvas::Action::publish(Message::GraphDragApply {
                            node: d.node,
                            // Le delta persistant est en espace monde.
                            dx: d.delta.x / self.scene.zoom,
                            dy: d.delta.y / self.scene.zoom,
                        })
                        .and_capture(),
                    )
                }
            }
            iced::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) => {
                let scroll = match delta {
                    iced::mouse::ScrollDelta::Lines { y, .. } => *y,
                    iced::mouse::ScrollDelta::Pixels { y, .. } => *y / 50.0,
                };
                Some(canvas::Action::publish(Message::GraphZoom(scroll)))
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        drag: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        self.scene.draw(renderer, bounds, *drag)
    }
}

/// Vue graphe : canevas (dessin + glisser-déposer via `canvas::Program::update`).
pub fn graph_view<'a>(app: &'a App, notes: &'a [Note], size: Size) -> Element<'a, Message> {
    let Some(index) = &app.index else {
        return container(text("Index absent")).into();
    };
    let scene = GraphScene::layout(
        notes,
        index,
        size,
        &app.graph_filters,
        &app.graph_drags,
        &app.file_mtimes,
        &ViewOpts {
            zoom: app.graph_zoom,
            pan: app.graph_pan,
            show_labels: app.graph_show_labels,
            show_grid: app.graph_show_grid,
        },
    );

    // Réutilise le programme persistant s'il a déjà été construit (la scène
    // est simplement remplacée), sinon en crée un. Le canevas reçoit une
    // copie : l'état du glisser-déposer en cours vit dans le `State` du
    // widget, que iced préserve entre les frames.
    let program = match &app.graph_program {
        Some(existing) => {
            let mut program = existing.clone();
            program.update_scene(scene);
            program
        }
        None => GraphProgram::new(scene),
    };

    let canvas = canvas(program).width(Length::Fill).height(Length::Fill);

    // Barre d'outils : filtre local (autour de la note ouverte) et effacement.
    let local_label = if app.graph_filters.local_note.is_some() {
        "Local ✓"
    } else {
        "Local"
    };
    let depth = app.graph_filters.local_depth.max(1);
    let mut toolbar = Row::new().spacing(8).padding([4, 8]);
    toolbar = toolbar.push(
        button(text(local_label))
            .on_press(Message::GraphToggleLocal)
            .style(super::ghost_button_style),
    );
    toolbar = toolbar.push(
        button(text(format!("Profondeur {depth}")))
            .on_press(Message::GraphCycleDepth)
            .style(super::ghost_button_style),
    );
    let period_label = match &app.graph_filters.modified_after {
        None => "Depuis : tout",
        Some(since) => {
            let now = std::time::SystemTime::now();
            const DAY: u64 = 86_400;
            if *since
                <= now
                    .checked_sub(std::time::Duration::from_secs(90 * DAY))
                    .unwrap_or(std::time::UNIX_EPOCH)
            {
                "Depuis : 90 j"
            } else if *since
                <= now
                    .checked_sub(std::time::Duration::from_secs(30 * DAY))
                    .unwrap_or(std::time::UNIX_EPOCH)
            {
                "Depuis : 30 j"
            } else {
                "Depuis : 7 j"
            }
        }
    };
    toolbar = toolbar.push(
        button(text(period_label))
            .on_press(Message::GraphCyclePeriod)
            .style(super::ghost_button_style),
    );
    toolbar = toolbar.push(
        button(text(if app.graph_show_labels {
            "Noms ✓"
        } else {
            "Noms"
        }))
        .on_press(Message::GraphToggleLabels)
        .style(super::ghost_button_style),
    );
    toolbar = toolbar.push(
        button(text(if app.graph_show_grid {
            "Quadrillage ✓"
        } else {
            "Quadrillage"
        }))
        .on_press(Message::GraphToggleGrid)
        .style(super::ghost_button_style),
    );
    toolbar = toolbar.push(
        button(text("Effacer"))
            .on_press(Message::GraphClearFilters)
            .style(super::ghost_button_style),
    );
    let filter_desc;
    if let Some(path) = &app.graph_filters.local_note {
        filter_desc = format!("local : {} (profondeur {depth})", path.display());
    } else if let Some(tag) = &app.graph_filters.tag {
        filter_desc = format!("tag : #{tag}");
    } else if let Some(folder) = &app.graph_filters.folder {
        filter_desc = format!("dossier : {}", folder.display());
    } else {
        filter_desc = "tout le vault".to_string();
    }
    toolbar = toolbar.push(text(filter_desc).size(13));

    container(iced::widget::column![toolbar, canvas])
        .style(super::panel_style)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use crate::vault::note::{Note, NoteMetadata};
    use iced::widget::canvas::Program;

    fn note(name: &str) -> Note {
        Note {
            path: PathBuf::from(format!("{name}.md")),
            name: name.to_string(),
            metadata: NoteMetadata {
                frontmatter: None,
                frontmatter_error: None,
                wikilinks: Vec::new(),
                tags: Vec::new(),
            },
        }
    }

    fn scene_size() -> Size {
        Size::new(800.0, 600.0)
    }

    #[test]
    fn layout_applies_persistent_drags() {
        let notes = vec![note("alpha"), note("beta")];
        let index = NoteIndex::build_index(&notes);
        let size = scene_size();
        let no_drags = HashMap::new();

        let base = GraphScene::layout(
            &notes,
            &index,
            size,
            &GraphFilters::default(),
            &no_drags,
            &HashMap::new(),
            &ViewOpts {
                zoom: 1.0,
                pan: (0.0, 0.0),
                show_labels: true,
                show_grid: false,
            },
        );

        let drags = HashMap::from([(0usize, (12.0, -7.0))]);
        let dragged = GraphScene::layout(
            &notes,
            &index,
            size,
            &GraphFilters::default(),
            &drags,
            &HashMap::new(),
            &ViewOpts {
                zoom: 1.0,
                pan: (0.0, 0.0),
                show_labels: true,
                show_grid: false,
            },
        );

        let base_0 = base.positions[&0];
        assert_eq!(
            dragged.positions[&0],
            Point::new(base_0.x + 12.0, base_0.y - 7.0)
        );
        // Les autres nœuds ne bougent pas.
        assert_eq!(dragged.positions[&1], base.positions[&1]);
    }

    #[test]
    fn hit_test_follows_dragged_position() {
        let notes = vec![note("alpha"), note("beta")];
        let index = NoteIndex::build_index(&notes);
        let size = scene_size();
        let no_drags = HashMap::new();

        let base = GraphScene::layout(
            &notes,
            &index,
            size,
            &GraphFilters::default(),
            &no_drags,
            &HashMap::new(),
            &ViewOpts {
                zoom: 1.0,
                pan: (0.0, 0.0),
                show_labels: true,
                show_grid: false,
            },
        );
        let drags = HashMap::from([(0usize, (50.0, 40.0))]);
        let dragged = GraphScene::layout(
            &notes,
            &index,
            size,
            &GraphFilters::default(),
            &drags,
            &HashMap::new(),
            &ViewOpts {
                zoom: 1.0,
                pan: (0.0, 0.0),
                show_labels: true,
                show_grid: false,
            },
        );

        // Le curseur sur la position décalée touche le nœud déplacé.
        let moved = dragged.positions[&0];
        assert_eq!(dragged.hit_test(moved), Some(0));

        // L'ancienne position n'est plus touchée (décalage > rayon de clic).
        let old = base.positions[&0];
        assert_ne!(dragged.hit_test(old), Some(0));
    }

    #[test]
    fn drag_lifecycle_publishes_start_and_apply() {
        let notes = vec![note("alpha"), note("beta")];
        let index = NoteIndex::build_index(&notes);
        let size = scene_size();
        let scene = GraphScene::layout(
            &notes,
            &index,
            size,
            &GraphFilters::default(),
            &HashMap::new(),
            &HashMap::new(),
            &ViewOpts {
                zoom: 1.0,
                pan: (0.0, 0.0),
                show_labels: true,
                show_grid: false,
            },
        );
        let program = GraphProgram::new(scene);
        let mut drag = None;
        let bounds = Rectangle::new(Point::ORIGIN, size);
        let origin = program.scene.positions[&0];

        // Press au-dessus du nœud 0 : capture + `GraphDragStart`.
        let press =
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left));
        let action = program
            .update(
                &mut drag,
                &press,
                bounds,
                iced::mouse::Cursor::Available(origin),
            )
            .expect("un clic sur un nœud doit produire une action");
        let (message, _, status) = action.into_inner();
        assert!(matches!(
            message,
            Some(Message::GraphDragStart { node: 0, origin: (x, y) })
                if (x - origin.x).abs() < 0.001 && (y - origin.y).abs() < 0.001
        ));
        assert_eq!(status, iced::event::Status::Captured);
        assert!(drag.is_some());

        // Move : delta local mis à jour, pas de message, demande de redessin.
        let moved_to = Point::new(origin.x + 40.0, origin.y + 10.0);
        let moved = iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: moved_to });
        let action = program
            .update(
                &mut drag,
                &moved,
                bounds,
                iced::mouse::Cursor::Available(moved_to),
            )
            .expect("un mouvement pendant un drag doit produire une action");
        let (message, redraw, _) = action.into_inner();
        assert!(message.is_none());
        assert!(matches!(redraw, iced::window::RedrawRequest::NextFrame));
        let d = drag.expect("le drag doit être en cours");
        assert!((d.delta.x - 40.0).abs() < 0.001);
        assert!((d.delta.y - 10.0).abs() < 0.001);

        // Release : `GraphDragApply` avec le décalage total, drag terminé.
        let release = iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        ));
        let action = program
            .update(
                &mut drag,
                &release,
                bounds,
                iced::mouse::Cursor::Available(moved_to),
            )
            .expect("le relâchement doit produire une action");
        let (message, _, status) = action.into_inner();
        assert!(matches!(
            message,
            Some(Message::GraphDragApply { node: 0, dx, dy })
                if (dx - 40.0).abs() < 0.001 && (dy - 10.0).abs() < 0.001
        ));
        assert_eq!(status, iced::event::Status::Captured);
        assert!(drag.is_none());
    }
}
