//! Explicit legacy conversion. Unconsumed properties are reported, not dropped silently.
use crate::*;

pub struct ThemeImport {
    pub document: Document,
    pub warnings: Vec<String>,
}
struct Reader {
    value: toml::Value,
    used: BTreeSet<String>,
}
impl Reader {
    fn take(&mut self, path: &str) -> Option<toml::Value> {
        let mut value = &self.value;
        for key in path.split('.') {
            value = value.get(key)?;
        }
        self.used.insert(path.into());
        Some(value.clone())
    }
    fn text(&mut self, path: &str) -> Result<Option<String>, String> {
        self.take(path)
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| diagnostic!("{path} : texte attendu", "{path}: expected text"))
            })
            .transpose()
    }
    fn number(&mut self, path: &str) -> Result<Option<f32>, String> {
        self.take(path)
            .map(|v| {
                v.as_float()
                    .or_else(|| v.as_integer().map(|n| n as f64))
                    .filter(|n| n.is_finite())
                    .map(|n| n as f32)
                    .ok_or_else(|| diagnostic!("{path} : nombre fini attendu", "{path}: expected a finite number"))
            })
            .transpose()
    }
    fn boolean(&mut self, path: &str) -> Result<Option<bool>, String> {
        self.take(path)
            .map(|v| {
                v.as_bool()
                    .ok_or_else(|| diagnostic!("{path} : booléen attendu", "{path}: expected a boolean"))
            })
            .transpose()
    }
    fn color(&mut self, path: &str) -> Result<Option<[f32; 4]>, String> {
        self.text(path)?
            .map(|s| {
                parse_color(&s)
                    .ok_or_else(|| diagnostic!("{path} : couleur #RRGGBB ou #RRGGBBAA attendue", "{path}: expected a #RRGGBB or #RRGGBBAA color"))
            })
            .transpose()
    }
    fn warnings(&self) -> Vec<String> {
        fn leaves(v: &toml::Value, prefix: &str, out: &mut Vec<String>) {
            if let Some(table) = v.as_table() {
                for (k, v) in table {
                    leaves(
                        v,
                        &if prefix.is_empty() {
                            k.clone()
                        } else {
                            format!("{prefix}.{k}")
                        },
                        out,
                    )
                }
            } else {
                out.push(prefix.into())
            }
        }
        let mut out = vec![];
        leaves(&self.value, "", &mut out);
        out.into_iter()
            .filter(|p| !self.used.contains(p))
            .map(|p| diagnostic!("Non converti : {p}", "Not converted: {p}"))
            .collect()
    }
}
fn parse_color(s: &str) -> Option<[f32; 4]> {
    let s = s.strip_prefix('#')?;
    if !matches!(s.len(), 6 | 8) || !s.is_ascii() {
        return None;
    }
    let mut out = [1.0; 4];
    for (i, value) in out.iter_mut().enumerate().take(s.len() / 2) {
        *value = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()? as f32 / 255.0;
    }
    Some(out)
}
fn anchor(s: &str) -> Result<[f32; 2], String> {
    Ok(match s {
        "top_left" => [0.0, 0.0],
        "top_center" => [0.5, 0.0],
        "top_right" => [1.0, 0.0],
        "center_left" => [0.0, 0.5],
        "center" => [0.5, 0.5],
        "center_right" => [1.0, 0.5],
        "bottom_left" => [0.0, 1.0],
        "bottom_center" => [0.5, 1.0],
        "bottom_right" => [1.0, 1.0],
        _ => return Err(diagnostic!("Ancrage inconnu : {s}", "Unknown anchor: {s}")),
    })
}
fn text_style(r: &mut Reader, prefix: &str, e: &mut Element) -> Result<(), String> {
    if let Some(v) = r.number(&format!("{prefix}.font_size"))? {
        e.font_size = v;
        e.overrides.font_size = Some(v);
    }
    if let Some(v) = r.color(&format!("{prefix}.color"))? {
        e.overrides.foreground = Some(v);
    }
    if let Some(v) = r.text(&format!("{prefix}.font_path"))? {
        e.overrides.font = Some(v);
    }
    Ok(())
}
impl Document {
    pub fn import_theme_report(source: &str) -> Result<ThemeImport, String> {
        let mut r = Reader {
            value: toml::from_str(source).map_err(|e| e.to_string())?,
            used: BTreeSet::new(),
        };
        let mut doc = Document::defaults();
        let dialogue = doc.add_dialogue_page();
        let choices = doc.add_choices_page();
        // Legacy UI dimensions are authored against the engine's 1280×720 canvas.
        for page in &mut doc.pages {
            for e in &mut page.elements {
                e.rect = e.rect.map(|v| v * 2.0 / 3.0);
                e.font_size *= 2.0 / 3.0;
            }
        }
        doc.reference = [1280.0, 720.0];
        let mut warnings = vec![];
        let title = &mut doc.pages[0];
        if let Some(text) = r.text("title_screen.title.text")? {
            title.elements[0].text = text;
        }
        text_style(&mut r, "title_screen.title", &mut title.elements[0])?;
        let a = anchor(
            &r.text("title_screen.title.anchor")?
                .unwrap_or_else(|| "top_center".into()),
        )?;
        let x = r
            .number("title_screen.title.offset_x")?
            .or(r
                .number("title_screen.title.x")?
                .map(|v| (v - 0.5) * 1280.0))
            .unwrap_or(0.0);
        let y = r
            .number("title_screen.title.offset_y")?
            .or(r.number("title_screen.title.y")?.map(|v| v * 720.0))
            .unwrap_or(96.0);
        title.elements[0].rect = [x, a[1] * 720.0 + y, 1280.0, 90.0];
        title.elements[0].layout_options.text_align = if a[0] == 0.0 {
            TextAlign::Left
        } else if a[0] == 1.0 {
            TextAlign::Right
        } else {
            TextAlign::Center
        };
        title.elements[0].anchors = [0.0, a[1], 1.0, a[1]];
        let background = if r
            .value
            .get("title_screen")
            .and_then(|v| v.get("background"))
            .is_some_and(|v| v.is_str())
        {
            r.text("title_screen.background")?
        } else {
            r.text("title_screen.background.path")?
                .or(r.text("title_screen.background.image")?)
        };
        if let Some(path) = background {
            let mut e = Element::new("background".into(), Kind::Image);
            e.asset = Some(path);
            e.rect = [0.0, 0.0, 1280.0, 720.0];
            e.anchors = [0.0, 0.0, 1.0, 1.0];
            e.normal = [0.0; 4];
            e.style = None;
            e.layout_options.image_fit = match r
                .text("title_screen.background.mode")?
                .as_deref()
                .unwrap_or("cover")
            {
                "cover" => ImageFit::Cover,
                "contain" => ImageFit::Contain,
                "stretch" => ImageFit::Stretch,
                other => return Err(diagnostic!("Mode d’image inconnu : {other}", "Unknown image mode: {other}")),
            };
            title.elements.insert(0, e);
        }
        let keys = [
            "continue", "new_game", "load", "settings", "gallery", "quit",
        ];
        let order = match r.take("title_screen.button_order") {
            Some(v) => v
                .as_array()
                .ok_or(diagnostic!("button_order : liste attendue", "button_order: expected a list"))?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_string)
                        .ok_or(diagnostic!("button_order : texte attendu", "button_order: expected text").to_string())
                })
                .collect::<Result<Vec<_>, _>>()?,
            None => keys.iter().map(|s| s.to_string()).collect(),
        };
        let width = r.number("title_screen.buttons.width")?.unwrap_or(300.0);
        let height = r.number("title_screen.buttons.height")?.unwrap_or(48.0);
        let gap = r
            .number("title_screen.buttons.spacing")?
            .or(r.number("title_screen.button_spacing")?)
            .unwrap_or(10.0);
        let a = anchor(
            &r.text("title_screen.buttons.anchor")?
                .unwrap_or_else(|| "center".into()),
        )?;
        let x = r
            .number("title_screen.buttons.offset_x")?
            .or(r
                .number("title_screen.button_x")?
                .map(|v| (v - a[0]) * 1280.0))
            .unwrap_or(0.0);
        let y = r
            .number("title_screen.buttons.offset_y")?
            .or(r
                .number("title_screen.button_y")?
                .map(|v| (v - a[1]) * 720.0))
            .unwrap_or(0.0);
        let mut buttons = Vec::new();
        let mut seen = BTreeSet::new();
        for key in order {
            if !seen.insert(key.clone()) {
                return Err(diagnostic!("Bouton dupliqué : {key}", "Duplicate button: {key}"));
            }
            let action = match key.as_str() {
                "continue" => Action::Continue,
                "new_game" => Action::NewGame,
                "load" => Action::Load,
                "settings" => Action::Settings,
                "gallery" => Action::Gallery,
                "quit" => Action::Quit,
                _ => {
                    warnings.push(diagnostic!("Bouton non converti : {key}", "Button not converted: {key}"));
                    continue;
                }
            };
            let mut e = title
                .elements
                .iter()
                .find(|e| e.kind == Kind::Button && e.action == action)
                .cloned()
                .ok_or(diagnostic!("Bouton standard absent", "Missing standard button"))?;
            let shown = r
                .boolean(&format!("title_screen.buttons.visibility.{key}"))?
                .or(r.boolean(&format!("title_screen.show_{key}"))?)
                .unwrap_or(true);
            if let Some(text) = r.text(&format!("title_screen.buttons.labels.{key}"))? {
                e.text = text;
            }
            text_style(&mut r, "title_screen.buttons", &mut e)?;
            if let Some(v) = r.color("title_screen.buttons.style.background_color")? {
                e.overrides.normal = Some(v);
            }
            if let Some(v) = r.color("title_screen.buttons.style.hover_color")? {
                e.overrides.hover = Some(v);
            }
            if let Some(v) = r.color("title_screen.buttons.style.pressed_color")? {
                e.overrides.pressed = Some(v);
            }
            if let Some(v) = r.color("title_screen.buttons.style.text_color")? {
                e.overrides.foreground = Some(v);
            }
            e.visible = shown;
            if shown {
                let index = buttons.iter().filter(|e: &&Element| e.visible).count();
                e.rect = [
                    a[0] * 1280.0 - width * a[0] + x,
                    a[1] * 720.0 + y + index as f32 * (height + gap),
                    width,
                    height,
                ];
                e.focus_order = index as i32;
                e.anchors = [a[0], a[1], a[0], a[1]];
            }
            buttons.push(e);
        }
        let bottom = buttons
            .iter()
            .filter(|e| e.visible)
            .map(|e| e.rect[1] + e.rect[3])
            .fold(0.0, f32::max);
        if bottom > 720.0 {
            let shift = bottom - 720.0;
            for e in &mut buttons {
                e.rect[1] -= shift;
            }
            warnings.push(
                diagnostic!("Boutons du titre : groupe remonté pour garder le dernier bouton accessible.", "Title buttons: group moved up to keep the last button accessible.")
                    .into(),
            );
        }
        title.elements.retain(|e| e.kind != Kind::Button);
        title.elements.extend(buttons);
        if r.boolean("title_screen.enabled")? == Some(false) {
            title.role = None;
            warnings.push(
                diagnostic!("Écran titre désactivé : la page importée n’a pas de rôle de démarrage.", "Title screen disabled: the imported page has no startup role.").into(),
            );
        }
        let box_height = r.number("textbox.height")?.unwrap_or(174.0);
        let padding = r.number("textbox.padding")?.unwrap_or(24.0);
        let page = &mut doc.pages[dialogue];
        page.elements[0].rect = [0.0, 720.0 - box_height, 1280.0, box_height];
        page.elements[0].anchors = [0.0, 1.0, 1.0, 1.0];
        if let Some(v) = r.color("textbox.background_color")? {
            page.elements[0].overrides.normal = Some(v);
        }
        if let Some(v) = r.text("textbox.image_path")? {
            page.elements[0].asset = Some(v);
            page.elements[0].layout_options.image_fit = ImageFit::Stretch;
        }
        text_style(&mut r, "text.name", &mut page.elements[1])?;
        text_style(&mut r, "text.dialogue", &mut page.elements[2])?;
        let name_height = page.elements[1].font_size * 1.35;
        let content_height = (box_height - padding * 2.0 - name_height).max(40.0);
        if padding * 2.0 + name_height + 40.0 > box_height {
            warnings.push(
                diagnostic!("Boîte de dialogue : marges verticales réduites pour garder le texte accessible.", "Dialogue box: vertical padding reduced to keep the text accessible.")
                    .into(),
            );
        }
        let top = (padding).min((box_height - name_height - content_height).max(0.0) * 0.5);
        page.elements[1].rect = [
            padding,
            720.0 - box_height + top,
            1280.0 - padding * 2.0,
            name_height,
        ];
        page.elements[2].rect = [
            padding,
            720.0 - box_height + top + name_height,
            1280.0 - padding * 2.0,
            content_height,
        ];
        for e in &mut page.elements[1..] {
            e.anchors = [0.0, 1.0, 1.0, 1.0];
        }
        let e = &mut doc.pages[choices].elements[0];
        text_style(&mut r, "choice", e)?;
        if let Some(v) = r.color("choice.background_color")? {
            e.overrides.normal = Some(v);
        }
        if let Some(v) = r.color("choice.text_color")? {
            e.overrides.foreground = Some(v);
        }
        // The visual format stores offsets from anchors, not absolute bounds.
        // Convert the legacy absolute rectangles once, after positioning them.
        for page in &mut doc.pages {
            for e in &mut page.elements {
                e.rect[0] -= e.anchors[0] * 1280.0;
                e.rect[1] -= e.anchors[1] * 720.0;
                e.rect[2] -= (e.anchors[2] - e.anchors[0]) * 1280.0;
                e.rect[3] -= (e.anchors[3] - e.anchors[1]) * 720.0;
            }
        }
        warnings.extend(r.warnings());
        doc.validate()?;
        Ok(ThemeImport {
            document: doc,
            warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stretched_offsets_are_validated_against_the_parent() {
        let mut d = Document::defaults();
        let mut root = Element::new("root".into(), Kind::Panel);
        root.rect = [0.0, 0.0, 100.0, 100.0];
        let mut child = Element::new("fill".into(), Kind::Panel);
        child.rect = [10.0, 10.0, -20.0, -20.0];
        child.anchors = [0.0, 0.0, 1.0, 1.0];
        root.children.push(child);
        d.pages[0].elements = vec![root];
        d.validate().unwrap();
        assert_eq!(
            d.layout_page(0, d.reference)[1].rect,
            [10.0, 10.0, 80.0, 80.0]
        );
        d.pages[0].elements[0].rect[2] = 10.0;
        assert!(d.validate().is_err());
    }
    #[test]
    fn imports_real_theme_and_reports_unconverted_fields() {
        let source = include_str!("../../rvn_cli/template/default/theme.toml");
        let report = Document::import_theme_report(source).unwrap();
        let d = report.document;
        assert_eq!(d.reference, [1280.0, 720.0]);
        assert!(d.pages[0]
            .elements
            .iter()
            .any(|e| e.asset.as_deref() == Some("backgrounds/title_forest.png")));
        let b = d.pages[0]
            .elements
            .iter()
            .find(|e| e.action == Action::Settings)
            .unwrap();
        assert_eq!(b.text, "Paramètres");
        assert_eq!(b.rect[2], 304.0);
        assert_eq!(d.resolved_element(b).font_size, 22.0);
        let page = d
            .pages
            .iter()
            .find(|p| p.role == Some(PageRole::Dialogue))
            .unwrap();
        assert_eq!(page.elements[0].asset.as_deref(), Some("ui/textbox2.png"));
        assert_eq!(d.resolved_element(&page.elements[2]).font_size, 22.0);
        assert!(report
            .warnings
            .iter()
            .any(|s| s.contains("choice.number_color")));
        assert!(report
            .warnings
            .iter()
            .any(|s| s.contains("title_screen.music")));
        for size in [[1280.0, 720.0], [1920.0, 1080.0], [2560.0, 1080.0]] {
            for i in [
                0,
                d.pages
                    .iter()
                    .position(|p| p.role == Some(PageRole::Dialogue))
                    .unwrap(),
            ] {
                for e in d.layout_page(i, size) {
                    assert!(
                        e.rect[0] >= 0.0
                            && e.rect[1] >= 0.0
                            && e.rect[0] + e.rect[2] <= size[0] + 0.1
                            && e.rect[1] + e.rect[3] <= size[1] + 0.1,
                        "{} {:?}",
                        e.id,
                        e.rect
                    );
                }
            }
        }
    }
    #[test]
    fn malformed_values_are_not_silently_replaced() {
        assert!(Document::import_theme_report("[text.dialogue]\nfont_size='big'").is_err());
        assert!(Document::import_theme_report("[choice]\ntext_color='#zz0000'").is_err());
        let r = Document::import_theme_report("[future]\nfoo=1").unwrap();
        assert!(r.warnings.iter().any(|s| s.contains("future.foo")));
    }
}
