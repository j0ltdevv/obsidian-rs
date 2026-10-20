# Câblage iced 0.14 — Intégration UI Phase 2

Guide de référence pour le câblage de la Phase 2 (recherche rapide, édition du
frontmatter, vue graphe, raccourcis clavier) sur le projet `obsidian-rs`
(Rust + iced `0.14`).

> **API vérifiée** : toutes les signatures ci-dessous sont extraites des sources
> `cargo` résolues (iced `0.14.0`, `iced_widget` `0.14.2`, `iced_core`
> `0.14.0`), pas de la doc en ligne. Les extraits sont prêts à copier, mais
> adaptez les chemins/types au code réel (le builder Phase 2 est en cours sur
> `src/ui/mod.rs` et `src/app/mod.rs` — ce document ne les remplace pas).

---

## 1. Raccourcis clavier (globaux)

### 1.1 API vérifiée

- `iced::event::listen_with(f)` renvoie une `Subscription<Message>`.
  La signature en 0.14 est un **pointeur de fonction** — pas une closure qui
  capture :

  ```rust
  pub fn listen_with<Message>(
      f: fn(Event, event::Status, window::Id) -> Option<Message>,
  ) -> Subscription<Message>
  where
      Message: 'static + MaybeSend,
  ```

- `listen_with` est appelé pour **chaque événement géré par le runtime**, y
  compris ceux déjà `Status::Captured` par un widget (c'est la différence avec
  `listen()` qui, lui, ne livre que les événements `Status::Ignored`).
  `listen_with` filtre en interne `RedrawRequested` et `SystemThemeChanged`.
  (`listen_raw` existe aussi : même signature, mais il livre aussi les
  `RedrawRequested` — à éviter pour des raccourcis, risque de boucle.)
- `iced::event::{Event, Status}` ; `iced::keyboard::{Event, Key, Modifiers}`
  et `iced::keyboard::key` (module `Named`).
- `Keyboard::Event::KeyPressed { key, modified_key, physical_key, location, modifiers, text, repeat }`.
- `Key` (enum) : `Named(Named) | Character(C) | Unidentified`. Comparer via
  `Key::as_ref() -> Key<&str>` pour éviter les soucis avec `SmolStr`.
- `key::Named::Escape` existe.
- `Modifiers` (bitflags) : `SHIFT`, `CTRL`, `ALT`, `LOGO`, `NONE`, et les
  méthodes `shift()`, `control()`, `alt()`, `logo()`, `command()`.

### 1.2 Désambiguation Command / Control

Sur **macOS**, le raccourci « commande » est `LOGO` (⌘). Ailleurs, il est
`CTRL`. La constante `Modifiers::COMMAND` et la méthode `Modifiers::command()`
encapsulent ce choix de plateforme (`LOGO` sur macOS, `CTRL` sinon).

```rust
use iced::event::{self, Event, Status};
use iced::keyboard::{key, Key, Modifiers};

fn keyboard_shortcut(event: Event, _status: Status, _window: iced::window::Id) -> Option<Message> {
    let Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key, modifiers, repeat, ..
    }) = event
    else {
        return None;
    };

    // Ignore l'auto-répétition (tenir Ctrl+K enfoncé n'ouvre pas 10×).
    if repeat {
        return None;
    }

    // Raccourci « commande » (Ctrl sur Linux/Windows, ⌘ sur macOS).
    if modifiers == Modifiers::COMMAND {
        match key.as_ref() {
            Key::Character(c) if c.eq_ignore_ascii_case("k") => {
                return Some(Message::OpenSwitcher);
            }
            Key::Character(c) if c.eq_ignore_ascii_case("s") => {
                return Some(Message::SaveNote);
            }
            Key::Character(c) if c.eq_ignore_ascii_case("g") => {
                return Some(Message::ToggleGraph);
            }
            _ => {}
        }
    }

    // Échap ferme la recherche rapide (sans modificateur).
    if modifiers.is_empty() && key.as_ref() == Key::Named(key::Named::Escape) {
        return Some(Message::CloseSwitcher);
    }

    None
}

pub fn subscription() -> iced::Subscription<Message> {
    event::listen_with(keyboard_shortcut)
}
```

Points d'attention :

- **Égalité stricte** `modifiers == Modifiers::COMMAND` : `Ctrl+Shift+K` ne
  déclenche donc pas `Ctrl+K`. Si vous préférez tolérer `Shift`, testez
  `modifiers.command() && !modifiers.alt()` (et décidez pour `Shift`).
- **Ne pas capturer d'état** : `listen_with` prend une `fn`, pas une closure.
  Tout ce qui dépend de l'état doit être décidé dans `update` (le message
  retourné est simple ; ex. `CloseSwitcher` alors que rien n'est ouvert est
  inoffensif car `update` l'ignore).
- **État actuel du dépôt** : `app::subscription` (dans `src/app/mod.rs`)
  utilise `event::listen_raw` et teste `modifiers.control()` ; sur macOS le
  raccourci « commande » (⌘) ne déclenchera donc rien — passer par
  `Modifiers::COMMAND` / `command()` pour être portable.

Enchaînement dans `main` :

```rust
iced::application(app::boot, app::update, app::view)
    .title("obsidian-rs")
    .window_size((720, 480))
    .centered()
    .subscription(app::subscription)
```

---

## 2. Switcher (overlay centré)

### 2.1 Principe

Superposer un panneau centré au-dessus de la vue courante : un `Stack` dont le
premier enfant est la vue normale et le second l'overlay (fond assombri +
panneau). Le widget `opaque(...)` bloque la propagation des événements souris
vers l'arrière-plan (c'est son but déclaré dans `iced_widget`).

Imports nécessaires (à ajouter en tête de `src/ui/mod.rs`) :

```rust
use iced::widget::{opaque, stack, text_input};
```

### 2.2 Vue racine avec overlay

```rust
pub fn view(app: &App) -> Element<'_, Message> {
    let body = match &app.notes {
        Ok(notes) => workspace_view(app, notes),
        Err(error) => error_view(error),
    };

    let mut root = Column::new().push(title_bar());
    if let Some(error) = &app.error {
        root = root.push(error_banner(error));
    }
    let root = root.push(body).height(Length::Fill);

    if app.switcher.is_some() {
        stack([root, opaque(switcher_overlay(app))])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        root.into()
    }
}
```

### 2.3 Overlay : panneau centré

API vérifiée :

- `container(...).center_x(Length::Fill).center_y(Length::Fill)` — **signature
  0.14** : `center_x(self, width: impl Into<Length>)` (et `center_y`),
  l'argument est obligatoire. Il existe aussi `center(width, height)` qui
  enchaîne les deux.
- `text_input(placeholder: &str, value: &str)` — le placeholder et la valeur
  sont des **arguments du constructeur** (pas des méthodes builder), puis
  `.on_input(closure)`, `.on_submit(message)`, `.width(...)`, `.padding(...)`.
- `scrollable(...).height(Length::Fill)` pour la liste des résultats.

```rust
fn switcher_overlay<'a>(app: &'a App) -> Element<'a, Message> {
    let Some(state) = &app.switcher else {
        return stack([]).into();
    };

    let mut results = Column::new().spacing(2);
    for &index in &state.results {
        let name = app
            .notes
            .as_ref()
            .ok()
            .and_then(|notes| notes.get(index))
            .map(|note| note.name.clone())
            .unwrap_or_else(|| format!("#{index}"));
        results = results.push(
            button(text(name))
                .width(Length::Fill)
                .on_press(Message::SwitcherSelect(index)),
        );
    }
    if state.results.is_empty() {
        results = results.push(text("Aucun résultat.").size(13));
    }

    let panel = container(
        column![
            text("Recherche rapide").size(16).font(bold_font()),
            text_input("Filtrer les notes…", &state.query)
                .on_input(Message::SwitcherQuery)
                .padding([7, 8]),
            scrollable(results).height(Length::FillPortion(3)),
            row![
                text("Ctrl+K : ouvrir · Échap : fermer")
                    .size(12)
                    .width(Length::Fill),
                button(text("Fermer"))
                    .on_press(Message::CloseSwitcher)
                    .style(primary_button_style),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(8)
        .padding(16),
    )
    .width(Length::Fixed(440.0))
    .height(Length::Shrink)
    .style(panel_style);

    container(panel)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(overlay_style) // fond semi-transparent
        .into()
}
```

Messages déjà gérés par `update` (`src/app/mod.rs`) : `OpenSwitcher`,
`CloseSwitcher`, `SwitcherQuery(String)`, `SwitcherSelect(usize)` (indice dans
`App::notes`). La fermeture se fait par Échap (section 1) ou par le bouton
« Fermer ».

---

## 3. Propriétés éditables du frontmatter

### 3.1 État fourni (vérifié)

- `App::open: Option<OpenNote>` ; `OpenNote::metadata.frontmatter` :
  `Option<serde_yaml::Value>` (mapping).
- `OpenNote::property_drafts: PropertyDrafts` avec :
  - `values: BTreeMap<String, String>` — brouillon de la valeur, par clé
    existante (initialisé depuis le frontmatter via `property_display`),
  - `new_key: String`, `new_value: String` — ligne d'ajout.
- Messages : `PropertyValueChanged { key, value }`,
  `SetProperty { key, value }`, `RemoveProperty { key }`,
  `NewPropertyKeyChanged(String)`, `NewPropertyValueChanged(String)`,
  `AddProperty { key, value }`.
- Conversion YAML : `App` fournit `parse_property_value(text) -> serde_yaml::Value`
  (nombres/booléens/listes si YAML valide, sinon chaîne) et
  `property_display(value) -> String` (chaînes brutes, autres valeurs
  sérialisées). L'UI envoie des **chaînes** ; la conversion a lieu dans
  `update` (via `set_property` → `set_frontmatter_property`).

### 3.2 Boucle sur les entrées + ligne d'ajout

```rust
fn frontmatter_editor<'a>(app: &'a App, open: &'a crate::app::OpenNote) -> Element<'a, Message> {
    let drafts = &open.property_drafts;

    let mut rows = Column::new().spacing(4);
    for (key, value) in &drafts.values {
        rows = rows.push(property_row(key, value));
    }

    rows = rows.push(
        row![
            text_input("Nouvelle clé", &drafts.new_key)
                .on_input(Message::NewPropertyKeyChanged)
                .width(Length::FillPortion(2)),
            text_input("Valeur", &drafts.new_value)
                .on_input(Message::NewPropertyValueChanged)
                .width(Length::FillPortion(3)),
            button(text("+"))
                .on_press(Message::AddProperty {
                    key: drafts.new_key.clone(),
                    value: drafts.new_value.clone(),
                })
                .style(ghost_button_style),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    );

    container(rows)
        .width(Length::Fill)
        .padding(6)
        .style(panel_style)
        .into()
}

/// Une ligne de propriété existante : clé, valeur éditée, ✓ / ✕.
fn property_row<'a>(key: &'a str, value: &'a str) -> Element<'a, Message> {
    row![
        text(key).size(13).width(Length::FillPortion(2)),
        text_input("Valeur", value)
            .on_input(move |new_value| Message::PropertyValueChanged {
                key: key.to_string(),
                value: new_value,
            })
            .width(Length::FillPortion(3)),
        button(text("✓"))
            .on_press(Message::SetProperty {
                key: key.to_string(),
                value: value.to_string(),
            })
            .style(ghost_button_style),
        button(text("×"))
            .on_press(Message::RemoveProperty { key: key.to_string() })
            .style(ghost_button_style),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}
```

Notes :

- `TextInput::on_input` accepte une closure `Fn(String) -> Message` — on peut
  capturer `key` (par `move`), contrairement à `listen_with` (section 1).
- Le bouton `+` envoie `AddProperty` avec les brouillons bruts ; `update`
  valide la clé non vide, appelle `set_property` (qui convertit via
  `parse_property_value`) et vide les brouillons en cas de succès.
- Sérialisation : `serde_yaml` est déjà une dépendance (`Cargo.toml`).

---

## 4. Graphe

### 4.1 État existant

- `src/ui/graph.rs` expose : `GraphScene::layout(notes, index, size)`,
  `GraphProgram<'a>`, et `graph_view<'a>(app, notes, size: Size) -> Element<'a, Message>`.
- Le module est déclaré dans `src/ui/mod.rs` (`mod graph;` + `use self::graph::graph_view;`).

### 4.2 Câblage

Affichage conditionnel quand `app.show_graph`, en utilisant
`iced::widget::responsive` pour obtenir la taille disponible (le canevas a
besoin de ses dimensions réelles pour placer les nœuds) :

```rust
if app.show_graph {
    return responsive(move |size| graph_view(app, notes, size)).into();
}
```

### 4.3 ⚠️ Feature `canvas` — absente de `Cargo.toml`

**Constat vérifié** : `Cargo.toml` déclare `iced = "0.14"` sans feature. Or
dans iced 0.14 :

- la feature `canvas` est **optionnelle** (`canvas = ["iced_widget/canvas"]`)
  et **n'est pas** dans les features par défaut
  (`wgpu, tiny-skia, crisp, web-colors, thread-pool, linux-theme-detection,
  x11, wayland`) ;
- `iced_widget` ne publie `pub mod canvas` que sous `#[cfg(feature = "canvas")]`.

Sans la feature, `use iced::widget::canvas;` dans `src/ui/graph.rs` ne
compile pas. Il faut ajouter :

```toml
[dependencies]
iced = { version = "0.14", features = ["canvas"] }
```

(Le `graph_view` existant utilise aussi `iced::mouse::Interaction`, `Size`,
`Point`, `Rectangle` : tout est dans le cœur de iced, sans feature
supplémentaire.)

---

## 5. Points d'attention

- **Ne pas toucher `src/app/state.rs`** (réservé). L'état requis par la
  Phase 2 existe déjà dans `src/app/mod.rs` (`App::switcher`,
  `App::show_graph`, `OpenNote::property_drafts`, …) — aucune modification
  d'état n'est nécessaire pour les cinq fonctionnalités ci-dessus.
- **État manquant éventuel** : si l'on veut *focuser* automatiquement le
  `TextInput` du switcher à l'ouverture, il faut un `Task`
  (`widget::operation::focus` / `text_input::Id`), pas un champ d'état — à
  ajouter dans `update` lors de `OpenSwitcher`. De même, la taille de fenêtre
  n'a pas besoin d'être stockée : `responsive` la fournit au layout.
- **`listen_with` prend une `fn`** (pas de closure) : tout raccourci
  dépendant de l'état doit décider dans `update`, pas dans la subscription.
- **Modificateurs** : préférer `Modifiers::COMMAND` / `Modifiers::command()`
  pour couvrir macOS (⌘) et Linux/Windows (Ctrl). Le code actuel de
  `app::subscription` utilise `modifiers.control()` — à harmoniser.
- **`container::center_x` / `center_y` exigent un argument**
  (`Length::Fill`) en 0.14 ; `center_x()` seul ne compile pas.
- **`opaque(...)`** au-dessus de l'overlay pour que les clics sur le fond ne
  traversent pas vers les widgets de la vue en dessous.
