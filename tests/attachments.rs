//! Tests d'intégration des pièces jointes (scan, renommage, suppression).

use obsy::vault::attachments::scan_attachments;

#[test]
fn scan_detecte_les_pieces_jointes_et_ignore_les_notes() {
    let mut dir = std::env::temp_dir();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    dir.push(format!("obsy-att-{unique}"));
    std::fs::create_dir_all(dir.join("attachments")).unwrap();
    std::fs::write(dir.join("note.md"), "# t").unwrap();
    std::fs::write(dir.join("attachments/photo.png"), "png").unwrap();
    std::fs::write(dir.join("attachments/doc.pdf"), "pdf").unwrap();
    let atts = scan_attachments(&dir).unwrap();
    assert_eq!(atts.len(), 2);
    assert!(atts.iter().any(|a| a.name == "photo.png"));
    assert!(atts.iter().any(|a| a.name == "doc.pdf"));
    assert!(!atts.iter().any(|a| a.name == "note.md"));
    let _ = std::fs::remove_dir_all(&dir);
}
