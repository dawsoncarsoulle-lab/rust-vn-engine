//! An authored sibling order is part of the existing Screen expression.
//! This does not allocate widgets, change parent slots or create a second tree.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetSiblingPlacement {
    Before,
    After,
}

impl GraphDocument {
    /// Place an existing Widget immediately before/after its existing sibling.
    /// The destination is an identity, not a potentially stale list index.
    /// Cross-parent movement remains the separate typed reparent operation.
    pub fn interface_widget_reorder_sibling(
        &mut self,
        returned: NodeId,
        node: NodeId,
        sibling: NodeId,
        placement: WidgetSiblingPlacement,
    ) -> Result<bool, String> {
        use rvn_ui::programmable::ComponentKind;
        let tree = widget_context(self, returned)?;
        if node == sibling {
            return Err("A Widget cannot be its own reorder destination".into());
        }
        let moving = widget_find(&tree, node).ok_or("Widget to reorder is absent")?;
        let anchor = widget_find(&tree, sibling).ok_or("Widget sibling destination is absent")?;
        widget_kind(moving)?;
        widget_kind(anchor)?;
        let (parent, from) = widget_parent(&tree, node)
            .ok_or("The Widget root cannot be reordered as a child")?;
        let (sibling_parent, anchor_index) = widget_parent(&tree, sibling)
            .ok_or("The Widget root cannot be a sibling destination")?;
        if parent != sibling_parent {
            return Err("Widget reorder requires siblings of the same parent".into());
        }
        let container = widget_find(&tree, parent).ok_or("Widget parent is absent")?;
        if !matches!(widget_kind(container)?, ComponentKind::Column | ComponentKind::Row) {
            return Err("This Widget parent has no admitted sibling reorder contract".into());
        }
        if container.computed_children.is_some() {
            return Err("Computed Widget children remain Blueprint expressions".into());
        }
        widget_owned_branch(self, &tree, node)?;
        // The anchor's identity must also be unambiguous in the authored tree.
        widget_owned_branch(self, &tree, sibling)?;

        // Determine the insertion position after removal, including the forward
        // move correction. The slot belongs to the child and is never rewritten.
        let anchor_after_removal = anchor_index - usize::from(from < anchor_index);
        let to = anchor_after_removal + usize::from(placement == WidgetSiblingPlacement::After);
        if from == to {
            return Ok(false);
        }
        let mut next = self.clone();
        next.interface_componentize(parent)?;
        next.move_list_input(parent, "children", from, to)?;
        widget_validate_edit(&next, returned)?;
        let actual = next.interface_tree(returned)?;
        let actual_parent = widget_find(&actual, parent).ok_or("Reordered Widget parent is absent")?;
        if actual_parent.children.len() != container.children.len()
            || actual_parent.children.get(to).map(|child| child.node) != Some(node)
        {
            return Err("Widget reorder did not retain its original child identities".into());
        }
        *self = next;
        Ok(true)
    }
}
