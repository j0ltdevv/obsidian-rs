//! Tests de golden files : chaque fixture `tests/fixtures/*.md` doit produire
//! exactement les segments attendus dans `tests/fixtures/*.json`.

use obsy::vault::live_preview::{render_markdown, SegmentStyle};

#[derive(serde::Deserialize)]
struct SegmentAttendu {
    style: String,
    text: String,
    line: usize,
}

fn style_name(style: SegmentStyle) -> &'static str {
    match style {
        SegmentStyle::Plain => "Plain",
        SegmentStyle::H1 => "H1",
        SegmentStyle::H2 => "H2",
        SegmentStyle::H3 => "H3",
        SegmentStyle::H4 => "H4",
        SegmentStyle::H5 => "H5",
        SegmentStyle::H6 => "H6",
        SegmentStyle::Bold => "Bold",
        SegmentStyle::Italic => "Italic",
        SegmentStyle::BoldItalic => "BoldItalic",
        SegmentStyle::Strikethrough => "Strikethrough",
        SegmentStyle::InlineCode => "InlineCode",
        SegmentStyle::CodeBlock => "CodeBlock",
        SegmentStyle::Blockquote => "Blockquote",
        SegmentStyle::Link => "Link",
        SegmentStyle::ExternalUrl => "ExternalUrl",
        SegmentStyle::Wikilink => "Wikilink",
        SegmentStyle::Tag => "Tag",
        SegmentStyle::TaskCheckboxTodo => "TaskCheckboxTodo",
        SegmentStyle::TaskCheckboxDone => "TaskCheckboxDone",
        SegmentStyle::ListBullet => "ListBullet",
        SegmentStyle::ListNumbered => "ListNumbered",
        SegmentStyle::HorizontalRule => "HorizontalRule",
        SegmentStyle::Table => "Table",
        SegmentStyle::Callout => "Callout",
        SegmentStyle::CalloutTitle => "CalloutTitle",
        SegmentStyle::MathInline => "MathInline",
        SegmentStyle::MathDisplay => "MathDisplay",
        SegmentStyle::Embed => "Embed",
        SegmentStyle::EmbedImage => "EmbedImage",
        SegmentStyle::EmbedPdf => "EmbedPdf",
        SegmentStyle::Footnote => "Footnote",
    }
}

#[test]
fn fixtures_golden_rendues_identiques() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut mds: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
        .collect();
    mds.sort_by_key(|e| e.file_name());
    assert!(!mds.is_empty(), "aucune fixture trouvée");
    for entry in mds {
        let md_path = entry.path();
        let json_path = md_path.with_extension("json");
        let source = std::fs::read_to_string(&md_path).unwrap();
        let expected: Vec<SegmentAttendu> =
            serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap()).unwrap();
        let actual = render_markdown(&source);
        let actual: Vec<(String, String, usize)> = actual
            .into_iter()
            .map(|s| (style_name(s.style).to_string(), s.text, s.line))
            .collect();
        let expected: Vec<(String, String, usize)> = expected
            .into_iter()
            .map(|s| (s.style, s.text, s.line))
            .collect();
        assert_eq!(actual, expected, "golden mismatch: {}", md_path.display());
    }
}
