//! Editable starting layouts, not opaque renderer skins.
use crate::*;

impl Document {
    pub fn from_template(preset: ThemePreset) -> Self {
        let mut doc = Self::defaults();
        doc.add_dialogue_page();
        doc.add_choices_page();
        doc.add_quick_actions_page();
        let card = if preset == ThemePreset::ScienceFiction {
            doc.add_detailed_save_card()
        } else {
            doc.add_save_card()
        };
        let gallery_card = doc.add_gallery_card();
        for page in &mut doc.pages {
            match page.role {
                Some(PageRole::Title | PageRole::Pause) => {
                    let mut panel = Element::new("navigation_panel".into(), Kind::Panel);
                    panel.rect = match preset {
                        ThemePreset::Sobre => [580.0, 160.0, 760.0, 760.0],
                        ThemePreset::Illustre => [80.0, 100.0, 620.0, 880.0],
                        ThemePreset::ScienceFiction => [1200.0, 120.0, 600.0, 840.0],
                    };
                    panel.style = Some("panel".into());
                    let [x, y, w, _] = panel.rect;
                    for e in &mut page.elements {
                        if e.kind == Kind::Text {
                            e.rect = [x + 40.0, y + 32.0, w - 80.0, 90.0];
                            e.font_size = 48.0;
                        } else if e.kind == Kind::Button {
                            let i = e.focus_order;
                            e.rect = [x + 40.0, y + 160.0 + i as f32 * 90.0, w - 80.0, 66.0];
                        }
                    }
                    page.elements.insert(0, panel);
                    if preset == ThemePreset::Illustre {
                        let mut image = Element::new("illustration".into(), Kind::Image);
                        image.name = "Illustration — choisissez votre image".into();
                        image.rect = [0.0, 0.0, 1920.0, 1080.0];
                        image.layout_options.image_fit = ImageFit::Cover;
                        image.normal = [0.0; 4];
                        image.locked = false;
                        page.elements.insert(0, image);
                    }
                    if preset == ThemePreset::ScienceFiction {
                        let mut accent = Element::new("accent".into(), Kind::Panel);
                        accent.rect = [1176.0, 120.0, 4.0, 840.0];
                        accent.overrides.normal = Some([0.0, 0.7, 0.85, 1.0]);
                        page.elements.push(accent);
                        let mut title = Element::new("story_title".into(), Kind::Text);
                        title.text = "VOTRE HISTOIRE".into();
                        title.rect = [100.0, 420.0, 1000.0, 180.0];
                        title.font_size = 80.0;
                        page.elements.push(title);
                    }
                }
                Some(PageRole::Save | PageRole::Load) => {
                    for e in &mut page.elements {
                        if e.kind == Kind::SaveList {
                            e.rect = [450.0, 230.0, 1370.0, 740.0];
                            e.list.template = Some(card.clone());
                            e.list.columns = 3;
                            e.list.rows = 2;
                            e.list.slots = 60;
                            e.layout_options.gap = if preset == ThemePreset::ScienceFiction {
                                4.0
                            } else {
                                28.0
                            };
                        }
                        if e.kind == Kind::Button {
                            e.rect = [80.0, 940.0, 300.0, 64.0];
                        }
                    }
                    let mut side = Element::new("sidebar".into(), Kind::Panel);
                    side.rect = [40.0, 210.0, 350.0, 690.0];
                    side.style = Some("panel".into());
                    page.elements.insert(0, side);
                    for (i, (label, action)) in [
                        ("Historique", Action::History),
                        ("Sauvegarder", Action::Save),
                        ("Charger", Action::Load),
                        ("Réglages", Action::Settings),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let mut e = Element::new(format!("nav_{i}"), Kind::Button);
                        e.text = label.into();
                        e.action = action;
                        e.rect = [65.0, 260.0 + i as f32 * 110.0, 300.0, 76.0];
                        e.focus_order = i as i32;
                        page.elements.push(e);
                    }
                }
                Some(PageRole::Dialogue) => {
                    if preset == ThemePreset::Illustre {
                        for e in &mut page.elements {
                            e.rect[0] += 100.0;
                            e.rect[2] -= 200.0;
                        }
                    }
                    if preset == ThemePreset::ScienceFiction {
                        for e in &mut page.elements {
                            if e.kind == Kind::Panel {
                                e.overrides.radius = Some(0.0);
                                e.overrides.border_width = Some(2.0);
                                e.overrides.border_color = Some([0.0, 0.6, 0.7, 1.0]);
                            }
                        }
                    }
                }
                Some(PageRole::Gallery) => {
                    for e in &mut page.elements {
                        if e.kind == Kind::Gallery {
                            e.list.template = Some(gallery_card.clone());
                            e.list.columns = 3;
                            e.list.rows = 2;
                            e.layout_options.gap = 24.0;
                        }
                    }
                }
                Some(PageRole::Choices) => {
                    let e = &mut page.elements[0];
                    e.rect = match preset {
                        ThemePreset::Sobre => [460.0, 150.0, 1000.0, 580.0],
                        ThemePreset::Illustre => [740.0, 160.0, 1040.0, 590.0],
                        ThemePreset::ScienceFiction => [140.0, 180.0, 1100.0, 560.0],
                    };
                }
                _ => {}
            }
        }
        if let Some(template) = doc.components.get_mut(&card) {
            template.overrides.radius = Some(if preset == ThemePreset::ScienceFiction {
                0.0
            } else {
                10.0
            });
            template.overrides.border_width = Some(if preset == ThemePreset::Sobre {
                1.0
            } else {
                2.0
            });
            template.overrides.border_color = Some(match preset {
                ThemePreset::Sobre => [0.26, 0.28, 0.32, 1.0],
                ThemePreset::Illustre => [0.5, 0.4, 0.28, 1.0],
                ThemePreset::ScienceFiction => [0.0, 0.6, 0.7, 1.0],
            });
        }
        doc.apply_theme(preset);
        // Overlay roles must never obscure the scene behind them.
        for page in &mut doc.pages {
            if matches!(
                page.role,
                Some(PageRole::Dialogue | PageRole::Choices | PageRole::QuickActions)
            ) {
                page.background = [0.0; 4];
            }
        }
        doc
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_templates_roundtrip_and_fit() {
        for preset in [
            ThemePreset::Sobre,
            ThemePreset::Illustre,
            ThemePreset::ScienceFiction,
        ] {
            let doc = Document::from_template(preset);
            doc.validate().unwrap();
            assert_eq!(doc.pages.len(), 11);
            assert_eq!(Document::from_json(&doc.to_json().unwrap()).unwrap(), doc);
            for size in [[1280.0, 720.0], [1920.0, 1080.0], [2560.0, 1080.0]] {
                for i in 0..doc.pages.len() {
                    for e in doc.layout_page(i, size) {
                        assert!(e.rect.iter().all(|v| v.is_finite()));
                        assert!(
                            e.rect[0] >= 0.0
                                && e.rect[1] >= 0.0
                                && e.rect[0] + e.rect[2] <= size[0] + 0.1
                                && e.rect[1] + e.rect[3] <= size[1] + 0.1,
                            "{preset:?} {} {} {:?}",
                            doc.pages[i].id,
                            e.id,
                            e.rect
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn layouts_differ_but_themes_preserve_actions_and_geometry() {
        let mut d = Document::from_template(ThemePreset::Illustre);
        let before = d.pages.clone();
        d.apply_theme(ThemePreset::ScienceFiction);
        for (a, b) in before.iter().zip(&d.pages) {
            assert_eq!(a.graphs, b.graphs);
            for (a, b) in a.elements.iter().zip(&b.elements) {
                assert_eq!(a.rect, b.rect);
                assert_eq!(a.action, b.action);
                assert_eq!(a.text, b.text);
            }
        }
        assert_ne!(
            Document::from_template(ThemePreset::Sobre).pages[0].elements[0].rect,
            Document::from_template(ThemePreset::ScienceFiction).pages[0].elements[0].rect
        );
    }
}
