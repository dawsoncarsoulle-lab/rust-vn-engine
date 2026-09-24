//! These documents were created, saved and packaged through the native editor.
use rvn_ui::{Document, PageRole, ThemePackage};
use std::{collections::BTreeSet, path::PathBuf};

#[test]
fn editor_created_designs_are_portable_complete_and_roundtrip() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/menu-designs");
    let mut layouts = Vec::new();
    for name in ["Sobre", "Illustré", "Science-fiction"] {
        let project = root.join(name);
        let doc =
            Document::from_json(&std::fs::read_to_string(project.join("menus.rvnui")).unwrap())
                .unwrap();
        doc.validate_files(&project.join("assets"), &BTreeSet::from(["start".into()]))
            .unwrap();
        assert_eq!(doc.pages.len(), 11);
        assert!(doc.pages.iter().any(|p| p.role == Some(PageRole::Dialogue)));
        assert!(doc.pages.iter().any(|p| p.role == Some(PageRole::Choices)));
        assert_eq!(Document::from_json(&doc.to_json().unwrap()).unwrap(), doc);
        let package = ThemePackage::from_json(
            &std::fs::read_to_string(root.join(format!("{name}.rvnuitheme"))).unwrap(),
        )
        .unwrap();
        assert_eq!(
            package.document, doc,
            "The package must contain the saved editable design"
        );
        for (path, bytes) in &package.resources {
            assert_eq!(
                &std::fs::read(project.join("assets").join(path)).unwrap(),
                bytes
            );
        }
        let original = Document::defaults();
        let mut second_project = original.clone();
        second_project
            .merge_design(package.document.clone())
            .unwrap();
        second_project.merge_design(package.document).unwrap();
        assert_eq!(
            &second_project.pages[..original.pages.len()],
            &original.pages
        );
        second_project.validate().unwrap();
        for size in [[1280.0, 720.0], [1920.0, 1080.0], [2560.0, 1080.0]] {
            for page in 0..doc.pages.len() {
                for element in doc.layout_page(page, size) {
                    assert!(element.rect.iter().all(|v| v.is_finite()));
                    let [x, y, width, height] = element.rect;
                    assert!(
                        width > 0.0 && height > 0.0,
                        "{name}: {} has no visible area",
                        element.id
                    );
                    assert!(
                        x >= -0.1
                            && y >= -0.1
                            && x + width <= size[0] + 0.1
                            && y + height <= size[1] + 0.1,
                        "{name}: {} lies outside {size:?}: {:?}",
                        element.id,
                        element.rect
                    );
                }
            }
        }
        layouts.push(doc.pages[0].elements.clone());
    }
    assert_ne!(layouts[0], layouts[1]);
    assert_ne!(layouts[1], layouts[2]);
}
