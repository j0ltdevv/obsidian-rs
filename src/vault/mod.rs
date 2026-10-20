//! Lecture/écriture Markdown, liens, index.

pub mod editor;
pub mod frontmatter;
pub mod graph;
pub mod history;
pub mod index;
pub mod keymap;
pub mod meta;
pub mod note;
pub mod ops;
pub mod properties;
pub mod quick_switcher;
pub mod scan;
pub mod search;
pub mod tags;
pub mod themes;
pub mod wikilinks;

pub use note::{Note, NoteMetadata, Wikilink};
pub use scan::scan_vault;
