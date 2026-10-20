//! Lecture/écriture Markdown, liens, index.

pub mod canvas_data;
pub mod commands;
pub mod daily_notes;
pub mod dataview;
pub mod editor;
pub mod frontmatter;
pub mod graph;
pub mod history;
pub mod index;
pub mod keymap;
pub mod live_preview;
pub mod meta;
pub mod note;
pub mod ops;
pub mod outline;
pub mod properties;
pub mod quick_switcher;
pub mod scan;
pub mod search;
pub mod search_ops;
pub mod tags;
pub mod tasks;
pub mod templates;
pub mod themes;
pub mod wikilinks;

pub use note::{Note, NoteMetadata, Wikilink};
pub use scan::scan_vault;
