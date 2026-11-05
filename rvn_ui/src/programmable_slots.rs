//! Typed slots on the existing Component tree. No runtime, evaluator or registry.
use super::{Alignment, Component, ComponentKind as Kind, ComponentRect};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutContract {
    WidgetV1,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SlotEdges {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}
impl SlotEdges {
    pub fn uniform(value: f32) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }
    fn total(self) -> [f32; 2] {
        [self.left + self.right, self.top + self.bottom]
    }
    fn validate(self) -> Result<(), String> {
        if [self.left, self.top, self.right, self.bottom]
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 16384.0)
        {
            return Err("Invalid finite bounded parent-slot padding".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "snake_case", deny_unknown_fields)]
pub enum BoxSlotSize {
    Auto {
        #[serde(default = "default_box_weight", skip_serializing_if = "is_default_box_weight")]
        weight: f32,
    },
    Fill {
        #[serde(default = "default_box_weight")]
        weight: f32,
    },
}
fn default_box_weight() -> f32 { 1.0 }
fn is_default_box_weight(weight: &f32) -> bool { *weight == 1.0 }
impl Default for BoxSlotSize {
    fn default() -> Self { Self::Auto { weight: default_box_weight() } }
}
impl BoxSlotSize {
    /// Serialized slot state; Auto retains this value but ignores it in layout.
    pub fn weight(self) -> f32 { match self { Self::Auto { weight } | Self::Fill { weight } => weight } }
    pub fn into_auto(self) -> Self { Self::Auto { weight: self.weight() } }
    pub fn into_fill(self) -> Self { Self::Fill { weight: self.weight() } }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ParentSlot {
    VerticalBox {
        #[serde(default)]
        padding: SlotEdges,
        #[serde(default)]
        horizontal: Alignment,
        #[serde(default)]
        vertical: Alignment,
        #[serde(default)]
        size: BoxSlotSize,
    },
    HorizontalBox {
        #[serde(default)]
        padding: SlotEdges,
        #[serde(default)]
        horizontal: Alignment,
        #[serde(default)]
        vertical: Alignment,
        #[serde(default)]
        size: BoxSlotSize,
    },
    Button {
        padding: SlotEdges,
        horizontal: Alignment,
        vertical: Alignment,
    },
}
impl ParentSlot {
    pub fn for_parent(kind: Kind) -> Result<Self, String> {
        match kind {
            Kind::Column => Ok(Self::VerticalBox {
                padding: SlotEdges::default(),
                horizontal: Alignment::Stretch,
                vertical: Alignment::Stretch,
                size: BoxSlotSize::default(),
            }),
            Kind::Row => Ok(Self::HorizontalBox {
                padding: SlotEdges::default(),
                horizontal: Alignment::Stretch,
                vertical: Alignment::Stretch,
                size: BoxSlotSize::default(),
            }),
            Kind::Button => Ok(Self::Button {
                padding: SlotEdges {
                    left: 4.0,
                    right: 4.0,
                    top: 2.0,
                    bottom: 2.0,
                },
                horizontal: Alignment::Center,
                vertical: Alignment::Center,
            }),
            _ => Err("This Widget parent has no admitted child-slot contract".into()),
        }
    }
    fn properties(&self) -> (SlotEdges, Alignment, Alignment) {
        match self {
            Self::VerticalBox {
                padding,
                horizontal,
                vertical,
                ..
            }
            | Self::HorizontalBox {
                padding,
                horizontal,
                vertical,
                ..
            }
            | Self::Button {
                padding,
                horizontal,
                vertical,
            } => (*padding, *horizontal, *vertical),
        }
    }
    fn box_size(&self) -> Option<BoxSlotSize> {
        match self {
            Self::VerticalBox { size, .. } | Self::HorizontalBox { size, .. } => Some(*size),
            Self::Button { .. } => None,
        }
    }
    fn validate_for(&self, parent: Kind) -> Result<(), String> {
        let matches = matches!(
            (self, parent),
            (Self::VerticalBox { .. }, Kind::Column)
                | (Self::HorizontalBox { .. }, Kind::Row)
                | (Self::Button { .. }, Kind::Button)
        );
        if !matches {
            return Err("Parent-slot type differs from its actual parent".into());
        }
        self.properties().0.validate()?;
        if let Some(size) = self.box_size() {
            let weight = size.weight();
            if !weight.is_finite() || weight < 0.0 || weight > 16384.0 {
                return Err("Invalid nonnegative bounded Box fill weight".into());
            }
        }
        Ok(())
    }
}

pub(super) fn validate_tree_contract(root: &Component) -> Result<(), String> {
    let widget = root.layout_contract.is_some();
    let mut count = 0usize;
    fn walk(
        node: &Component,
        parent: Option<Kind>,
        widget: bool,
        depth: usize,
        count: &mut usize,
    ) -> Result<(), String> {
        *count += 1;
        if depth > 32 || *count > 512 {
            return Err("Screen limit: 512 components and 32 nesting levels".into());
        }
        if depth > 0 && node.layout_contract.is_some() {
            return Err("Layout contract belongs only to the definition root".into());
        }
        if !widget {
            if node.parent_slot.is_some() {
                return Err(
                    "Typed parent slot requires an explicit root Widget layout contract".into(),
                );
            }
        } else {
            match (parent, node.parent_slot.as_ref()) {
                (None, None) => {}
                (Some(parent), Some(slot)) => slot.validate_for(parent)?,
                (None, Some(_)) => return Err("The Widget root has no parent slot".into()),
                (Some(_), None) => {
                    return Err("A Widget child needs its actual parent's typed slot".into())
                }
            }
            match node.kind{
                Kind::Column | Kind::Row=>{},
                Kind::Button if node.children.len()<=1&&node.text.is_empty()&&node.text_key.is_none()&&node.placeholder.is_empty()&&node.placeholder_key.is_none()=>{},
                Kind::Text if node.children.is_empty()=>{},
                Kind::Button=>return Err("Widget Button accepts one real child; its text belongs to that child".into()),
                _=>return Err("WidgetV1 currently implements Column/VerticalBox, Row/HorizontalBox, Button and Text; other Widget families remain pending".into()),
            }
            // These fields belong to the historical reference-pixel layout.
            // Refuse ambiguity instead of accepting controls with no effect.
            if node.rect.is_some()
                || node.width.is_some()
                || node.height.is_some()
                || node.min_width.is_some()
                || node.max_width.is_some()
                || node.min_height.is_some()
                || node.max_height.is_some()
                || node.padding != 0.0
                || node.margin != [0.0; 4]
                || node.spacing != 0.0
                || node.scroll != super::ScrollAxis::None
                || node.wrap
                || node.justify != super::Justification::Start
                || node.align != Alignment::Stretch
            {
                return Err("Widget placement uses parent slots and external viewport bounds, not legacy rectangle/flex fields".into());
            }
        }
        for child in &node.children {
            walk(child, Some(node.kind), widget, depth + 1, count)?;
        }
        Ok(())
    }
    walk(root, None, widget, 0, &mut count)
}

impl Component {
    /// Stage the whole edit. Capacity/type/identity failures leave the old tree
    /// and the proposed child untouched, including its old parent's content.
    pub fn add_widget_child(&mut self, parent: &str, child: &Component) -> Result<(), String> {
        self.validate()?;
        if self.layout_contract.is_none() {
            return Err("Widget editing requires its explicit layout contract".into());
        }
        let owner = self.find(parent).ok_or("Widget parent is absent")?;
        if owner.kind == Kind::Button && !owner.children.is_empty() {
            return Err("Occupied Button refuses AddChild before reparenting".into());
        }
        let slot = ParentSlot::for_parent(owner.kind)?;
        let mut child = child.clone();
        child.layout_contract = None;
        child.parent_slot = Some(slot);
        let mut trial = self.clone();
        trial.find_mut(parent).unwrap().children.push(child);
        trial.validate()?;
        *self = trial;
        Ok(())
    }
    /// Same-parent movement is a no-op. Cross-parent movement replaces the slot
    /// with the receiving parent's defaults in one validated document edit.
    pub fn reparent_widget_child(&mut self, child: &str, parent: &str) -> Result<(), String> {
        self.validate()?;
        if self.layout_contract.is_none() {
            return Err("Widget editing requires its explicit layout contract".into());
        }
        if self.id == child {
            return Err("Reparenting the definition root requires an explicit root edit".into());
        }
        let source = self.find(child).ok_or("Widget child is absent")?;
        if source.find(parent).is_some() {
            return Err("Widget cannot be reparented into itself or a descendant".into());
        }
        fn owner<'a>(node: &'a Component, child: &str) -> Option<&'a str> {
            for item in &node.children {
                if item.id == child {
                    return Some(&node.id);
                }
                if let Some(id) = owner(item, child) {
                    return Some(id);
                }
            }
            None
        }
        if owner(self, child) == Some(parent) {
            return Ok(());
        }
        let target = self.find(parent).ok_or("Widget parent is absent")?;
        if target.kind == Kind::Button && !target.children.is_empty() {
            return Err("Occupied Button refuses AddChild before reparenting".into());
        }
        let slot = ParentSlot::for_parent(target.kind)?;
        fn remove(node: &mut Component, id: &str) -> Option<Component> {
            if let Some(i) = node.children.iter().position(|node| node.id == id) {
                return Some(node.children.remove(i));
            }
            node.children.iter_mut().find_map(|node| remove(node, id))
        }
        let mut trial = self.clone();
        let mut moved = remove(&mut trial, child).ok_or("Widget source relation is absent")?;
        moved.parent_slot = Some(slot);
        trial.find_mut(parent).unwrap().children.push(moved);
        trial.validate()?;
        *self = trial;
        Ok(())
    }
    /// Unlike AddChild, SetContent deliberately replaces an occupied child.
    /// The removed definition subtree is returned as its own Widget layout root.
    /// This does not allocate, detach or destroy a runtime instance.
    pub fn set_widget_button_content(
        &mut self,
        parent: &str,
        child: Option<&Component>,
    ) -> Result<Option<Component>, String> {
        self.validate()?;
        if self.layout_contract.is_none() {
            return Err("Widget editing requires its explicit layout contract".into());
        }
        if self.find(parent).ok_or("Widget Button is absent")?.kind != Kind::Button {
            return Err("SetContent requires a Button".into());
        }
        let mut trial = self.clone();
        let target = trial.find_mut(parent).unwrap();
        let mut old = target.children.pop();
        if let Some(old) = &mut old {
            old.parent_slot = None;
            old.layout_contract = self.layout_contract;
        }
        if let Some(child) = child {
            let mut child = child.clone();
            child.layout_contract = None;
            child.parent_slot = Some(ParentSlot::for_parent(Kind::Button)?);
            target.children.push(child);
        }
        trial.validate()?;
        *self = trial;
        Ok(old)
    }
}

/// Same typed-tree traversal as layout, including Row/Column slot axes.
/// Owner autosize never measures a hidden replacement child or a second tree.
pub fn measure_widget_desired(root:&Component,text_measure:&mut impl FnMut(&Component)->Result<[f32;2],String>)->Result<[f32;2],String>{
 root.validate()?;if root.layout_contract.is_none(){return Err("Typed Widget measurement requires its explicit contract".into());}
 measure_widget_node(root,text_measure,&mut BTreeMap::new())
}
fn box_axis(kind: Kind) -> Option<usize> {
        match kind { Kind::Row => Some(0), Kind::Column => Some(1), _ => None }
    }
fn measure_widget_node(
        node: &Component,
        measure_text: &mut impl FnMut(&Component) -> Result<[f32; 2], String>,
        out: &mut BTreeMap<String, [f32; 2]>,
    ) -> Result<[f32; 2], String> {
        if !node.visible {
            out.insert(node.id.clone(), [0.0; 2]);
            return Ok([0.0; 2]);
        }
        let mut size = [0.0f32; 2];
        if node.kind == Kind::Text {
            size = measure_text(node)?;
            if size
                .iter()
                .any(|v| !v.is_finite() || *v < 0.0 || *v > 16384.0)
            {
                return Err("Invalid measured Widget text size".into());
            }
        } else {
            for child in node.children.iter().filter(|child| child.visible) {
                let measured = measure_widget_node(child, measure_text, out)?;
                let padding = child.parent_slot.as_ref().unwrap().properties().0.total();
                let padded = [measured[0] + padding[0], measured[1] + padding[1]];
                if let Some(axis) = box_axis(node.kind) {
                    size[1 - axis] = size[1 - axis].max(padded[1 - axis]);
                    size[axis] += padded[axis];
                } else {
                    size = padded;
                }
            }
        }
        if size.iter().any(|v| !v.is_finite() || v.abs() > 16384.0) {
            return Err("Invalid measured Widget desired size".into());
        }
        out.insert(node.id.clone(), size);
        Ok(size)
    }

/// Layout in logical widget coordinates, using one supplied text-measurement
/// function for Designer and runtime. Pure data traversal executes no graph.
pub fn layout_widget_rects(
    root: &Component,
    bounds: [f32; 4],
    text_measure: &mut impl FnMut(&Component) -> Result<[f32; 2], String>,
) -> Result<Vec<ComponentRect>, String> {
    root.validate()?;
    if root.layout_contract.is_none() {
        return Err("Typed Widget layout requires its explicit contract".into());
    }
    if bounds.iter().any(|v| !v.is_finite() || v.abs() > 16384.0)
        || bounds[2] < 0.0
        || bounds[3] < 0.0
    {
        return Err("Invalid Widget layout bounds".into());
    }
    // Both UMG box orientations share one sizing/allocation contract. The
    // authored slot variant still belongs to its actual parent, never an alias.
    let mut desired = BTreeMap::new();

    measure_widget_node(root, text_measure, &mut desired)?;
    fn aligned(
        origin: f32,
        extent: f32,
        desired: f32,
        before: f32,
        after: f32,
        align: Alignment,
    ) -> (f32, f32) {
        let available = (extent - before - after).max(0.0);
        let size = if align == Alignment::Stretch {
            available
        } else {
            desired.min(available).max(0.0)
        };
        let offset = match align {
            Alignment::Start | Alignment::Stretch => before,
            Alignment::Center => (extent - size) * 0.5 + before - after,
            Alignment::End => extent - size - after,
        };
        (origin + offset, size)
    }
    fn place(
        node: &Component,
        rect: [f32; 4],
        desired: &BTreeMap<String, [f32; 2]>,
        out: &mut Vec<ComponentRect>,
    ) {
        if !node.visible {
            return;
        }
        out.push(ComponentRect {
            id: node.id.clone(),
            rect,
        });
        let children: Vec<_> = node.children.iter().filter(|child| child.visible).collect();
        let axis = box_axis(node.kind);
        let mut cursor = rect[axis.unwrap_or(1)];
        let mut fixed = 0.0;
        let mut weights = 0.0;
        if let Some(axis) = axis {
            for child in &children {
                let slot = child.parent_slot.as_ref().unwrap();
                fixed += slot.properties().0.total()[axis];
                match slot.box_size().unwrap() {
                    BoxSlotSize::Auto { .. } => fixed += desired[&child.id][axis].max(0.0),
                    BoxSlotSize::Fill { weight } => weights += weight,
                }
            }
        }
        for child in children {
            let slot = child.parent_slot.as_ref().unwrap();
            let (p, horizontal, vertical) = slot.properties();
            let size = desired[&child.id];
            let mut allotment = rect;
            if let Some(axis) = axis {
                allotment[axis] = cursor;
                allotment[axis + 2] = match slot.box_size().unwrap() {
                    BoxSlotSize::Auto { .. } => size[axis].max(0.0) + p.total()[axis],
                    BoxSlotSize::Fill { weight } => {
                        if weights > 0.0 {
                            (rect[axis + 2] - fixed).max(0.0) * weight / weights + p.total()[axis]
                        } else {
                            p.total()[axis]
                        }
                    }
                };
            }
            let (x, width) = aligned(allotment[0], allotment[2], size[0], p.left, p.right, horizontal);
            let (y, height) = aligned(allotment[1], allotment[3], size[1], p.top, p.bottom, vertical);
            place(child, [x, y, width, height], desired, out);
            if let Some(axis) = axis { cursor += allotment[axis + 2]; }
        }
    }
    let mut out = Vec::new();
    place(root, bounds, &desired, &mut out);
    Ok(out)
}

#[cfg(test)]
#[path = "programmable_horizontal_box_tests_0954.rs"]
mod horizontal_box_tests_0954;
