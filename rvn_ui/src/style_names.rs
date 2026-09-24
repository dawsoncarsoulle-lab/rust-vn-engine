use crate::{Document, Element, StylePatch};

fn checked_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) {
        return Err(diagnostic!(
            "Le style demande un nom de 1 à 120 caractères, sans caractère de contrôle",
            "Style names must contain 1 to 120 characters, without control characters"
        )
        .into());
    }
    Ok(name.into())
}
impl Document {
    pub fn capture_style(
        &mut self,
        page: usize,
        element: &str,
        name: &str,
    ) -> Result<String, String> {
        let name = checked_name(name)?;
        if self.styles.contains_key(&name) {
            return Err(diagnostic!("Le style « {name} » existe déjà. Choisissez un autre nom ou mettez à jour ce style.", "Style “{name}” already exists. Choose another name or update this style."));
        }
        let source = self
            .find_element(page, element)
            .ok_or(diagnostic!("Élément absent", "Missing element"))?;
        let style = StylePatch::from_element(&self.resolved_element(source));
        self.styles.insert(name.clone(), style);
        let source = self.find_element_mut(page, element).unwrap();
        source.style = Some(name.clone());
        source.overrides = Default::default();
        Ok(name)
    }
    pub fn rename_style(&mut self, old: &str, new: &str) -> Result<(), String> {
        let new = checked_name(new)?;
        if !self.styles.contains_key(old) {
            return Err(diagnostic!("Style partagé absent", "Missing shared style").into());
        }
        if new == old {
            return Ok(());
        }
        if self.styles.contains_key(&new) {
            return Err(diagnostic!(
                "Le style « {new} » existe déjà ; aucun style n’a été remplacé.",
                "Style “{new}” already exists; no style was replaced."
            ));
        }
        let style = self.styles.remove(old).unwrap();
        self.styles.insert(new.clone(), style);
        fn remap(elements: &mut [Element], old: &str, new: &str) {
            for e in elements {
                if e.style.as_deref() == Some(old) {
                    e.style = Some(new.into());
                }
                remap(&mut e.children, old, new);
            }
        }
        for p in &mut self.pages {
            remap(&mut p.elements, old, &new);
        }
        for e in self.components.values_mut() {
            remap(std::slice::from_mut(e), old, &new);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rename_updates_pages_components_and_preserves_local_exceptions() {
        let mut d = Document::defaults();
        let id = d.pages[0].elements[0].id.clone();
        d.capture_style(0, &id, "Titres clairs").unwrap();
        let mut e = d.pages[0].elements[0].clone();
        e.overrides.foreground = Some([1.0, 0.0, 0.0, 1.0]);
        d.components.insert("title".into(), e.clone());
        let before = d.resolved_element(&e);
        d.rename_style("Titres clairs", "Titres — papier").unwrap();
        let changed = &d.components["title"];
        assert_eq!(changed.style.as_deref(), Some("Titres — papier"));
        assert_eq!(d.resolved_element(changed).foreground, before.foreground);
        assert_eq!(
            d.pages[0].elements[0].style.as_deref(),
            Some("Titres — papier")
        );
        let snapshot = d.clone();
        assert!(d.capture_style(0, &id, "Titres — papier").is_err());
        assert!(d.rename_style("Titres — papier", " ").is_err());
        assert_eq!(snapshot, d);
        assert_eq!(Document::from_json(&d.to_json().unwrap()).unwrap(), d);
    }
}
