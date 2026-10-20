//! Lecture/écriture Markdown, liens, index.

pub mod aliases;
pub mod attachments;
pub mod block_refs;
pub mod bookmarks;
pub mod callouts;
pub mod canvas_data;
pub mod commands;
pub mod css_snippets;
pub mod dataview;
pub mod editor;
pub mod embeds;
pub mod frontmatter;
pub mod graph;
pub mod history;
pub mod index;
pub mod keymap;
pub mod live_preview;
pub mod math;
pub mod meta;
pub mod note;
pub mod ops;
pub mod outline;
pub mod plugins_manifest;
pub mod properties;
pub mod quick_switcher;
pub mod scan;
pub mod search;
pub mod search_ops;
pub mod tag_tree;
pub mod tags;
pub mod tasks;
pub mod templates;
pub mod themes;
pub mod wikilinks;

pub use note::{Note, NoteMetadata, Wikilink};
pub use scan::scan_vault;
