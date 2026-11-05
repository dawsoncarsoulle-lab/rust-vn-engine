// Proposed as a child module of menu_documents::confirmation. Independent Rust.
// Not installed. Syntax alone does not establish types, routing or native parity.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Clone, PartialEq)]
struct Operation {
    token: u64,
    pending: Option<(usize, SaveMenuMode)>,
    action: Option<(Action, String, VnState)>,
    quick_return: Option<VnState>,
}
impl Operation {
    fn current(world: &World) -> Option<Self> {
        let gate = world.get_resource::<SaveConfirmation>()?;
        let source = world.get_resource::<crate::source_menus::SourceMenus>()?;
        if source.active(rvn_ui::PageRole::Confirm) { return None; }
        Some(Self {
            token: source.current_confirmation_token(gate)?,
            pending: gate.pending,
            action: gate.action_pending.clone(),
            quick_return: gate.quick_return.clone(),
        })
    }
}

#[derive(Clone, PartialEq)]
struct Target {
    root: Entity,
    id: String,
    order: Option<i32>,
    approve: Option<bool>,
    semantic: [u8; 32],
}
#[derive(Clone)]
pub(crate) struct Frame {
    operation: Operation,
    page: String,
    targets: HashMap<Entity, Target>,
}

fn root(world: &World, mut entity: Entity) -> Option<Entity> {
    for _ in 0..128 {
        if world.get::<ConfirmationRoot>(entity).is_some() { return Some(entity); }
        entity = world.get::<Parent>(entity)?.get();
    }
    None
}
fn visible_now(world: &World, mut entity: Entity) -> bool {
    for _ in 0..128 {
        if world.get::<Visibility>(entity) == Some(&Visibility::Hidden) { return false; }
        let Some(parent) = world.get::<Parent>(entity) else { return true; };
        entity = parent.get();
    }
    false
}
fn document(world: &World) -> Option<(Document, usize)> {
    let menus = world.get_resource::<Menus>()?;
    let mut doc = menus.doc.clone().unwrap_or_else(Document::defaults);
    let index = doc.pages.iter().position(|page| page.role == Some(rvn_ui::PageRole::Confirm))
        .unwrap_or_else(|| {
            doc.pages.push(Document::defaults().pages.into_iter()
                .find(|page| page.role == Some(rvn_ui::PageRole::Confirm)).unwrap());
            doc.pages.len() - 1
        });
    Some((menus.session.present(&doc), index))
}
fn intents(doc: &Document, page: usize) -> Option<HashMap<String, [u8; 32]>> {
    let graphs: [u8; 32] = Sha256::digest(serde_json::to_vec(&doc.pages[page].graphs).ok()?).into();
    let mut intents = HashMap::new();
    for element in doc.layout_page(page, doc.reference) {
        if !element.visible || !element.enabled { continue; }
        let events = doc.pages[page].graphs.iter()
            .filter(|graph| graph.target.as_deref() == Some(element.id.as_str()))
            .map(|graph| graph.event.clone());
        let pointer = interaction_policy(element.kind, events).0;
        if !pointer && !matches!(element.action, Action::Confirm | Action::Back) { continue; }
        let semantic = serde_json::to_vec(&(
            &doc.pages[page].id, &element.id, element.kind, &element.action,
            &element.binding, &element.local_control, &element.visibility_binding, graphs,
        )).ok()?;
        intents.insert(element.id, Sha256::digest(semantic).into());
    }
    Some(intents)
}
fn target(world: &World, entity: Entity, intents: &HashMap<String, [u8; 32]>) -> Option<Target> {
    let source = world.get::<ConfirmationTarget>(entity)?;
    // Geometry and decorative colors belong to the captured pixels. Intent,
    // inherited availability and document graphs remain authoritative at dispatch.
    Some(Target {
        root: root(world, entity)?, id: source.0.clone(), order: source.1,
        approve: world.get::<ConfirmationButton>(entity).map(|button| button.0),
        semantic: *intents.get(&source.0)?,
    })
}

// Account for borrowed producer data before cloning the document, operation,
// targets or presentation. The enclosing PhysicalFrame has the shared 2 MiB cap.
pub(crate) fn cost(world: &World, cost: &mut crate::editor_preview_input::PreviewCost) -> Option<()> {
    let gate = match world.get_resource::<SaveConfirmation>() {
        Some(gate) if gate.active() => gate,
        _ => return Some(()),
    };
    if world.get_resource::<crate::source_menus::SourceMenus>()
        .is_some_and(|source| source.active(rvn_ui::PageRole::Confirm)) { return Some(()); }
    let menus = world.get_resource::<Menus>()?;
    if let Some(doc) = &menus.doc { cost.serialized(doc)?; }
    else { cost.serialized(&Document::defaults())?; }
    for ((page, id), state) in &menus.session.presentation {
        cost.serialized(&(page, id, state.visible, state.enabled, &state.text, &state.image))?;
    }
    cost.serialized(&gate.action_pending.as_ref().map(|(action, page, state)|
        (action, page, format!("{state:?}"))))?;
    let stack = world.get_resource::<bevy::ui::UiStack>()?;
    if stack.uinodes.len() > 4096 { return None; }
    for &entity in &stack.uinodes {
        if let Some(source) = world.get::<ConfirmationTarget>(entity) {
            cost.add(std::mem::size_of::<Target>().checked_add(source.0.len())?)?;
        }
    }
    Some(())
}
impl Frame {
    pub(crate) fn capture(world: &World) -> Option<Self> {
        // Also bound this owner's use in isolation. The enclosing snapshot
        // accounts for the same borrowed data in its shared budget.
        let mut producer_cost = crate::editor_preview_input::PreviewCost::new(2 * 1024 * 1024);
        cost(world, &mut producer_cost)?;
        let operation = Operation::current(world)?;
        let (doc, index) = document(world)?;
        let intents = intents(&doc, index)?;
        let stack = world.get_resource::<bevy::ui::UiStack>()?;
        let targets = stack.uinodes.iter().filter_map(|&entity|
            target(world, entity, &intents).map(|target| (entity, target))).collect();
        Some(Self { operation, page: doc.pages[index].id.clone(), targets })
    }
    pub(crate) fn contains(&self, entity: Entity) -> bool { self.targets.contains_key(&entity) }
    pub(crate) fn current(&self, world: &World, entity: Entity) -> bool {
        // Live producer data may have grown since capture. Reject before any
        // operation or document clone, rather than trusting the old frame's cap.
        let mut producer_cost = crate::editor_preview_input::PreviewCost::new(2 * 1024 * 1024);
        if cost(world, &mut producer_cost).is_none() { return false; }
        if Operation::current(world).as_ref() != Some(&self.operation) { return false; }
        let Some(captured) = self.targets.get(&entity) else { return false; };
        if !visible_now(world, entity)
            || !world.get::<ViewVisibility>(entity).is_some_and(|visible| visible.get())
            || world.get::<Node>(entity).is_none()
            || world.get::<GlobalTransform>(entity).is_none()
            || !world.get_resource::<bevy::ui::UiStack>()
                .is_some_and(|stack| stack.uinodes.contains(&entity)) { return false; }
        let Some((doc, index)) = document(world) else { return false; };
        let Some(intents) = intents(&doc, index) else { return false; };
        self.page == doc.pages[index].id
            && target(world, entity, &intents).as_ref() == Some(captured)
    }
    // The caller supplies the entity picked from this completed image and its
    // frozen clipping/stack. It must never re-pick with a newer layout.
    pub(crate) fn project(&self, world: &mut World, picked: Option<Entity>) {
        let picked = picked.filter(|&entity| self.current(world, entity));
        let pressed = world.get_resource::<ButtonInput<MouseButton>>()
            .is_some_and(|mouse| mouse.pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Left));
        let mut query = world.query_filtered::<(Entity, &mut Interaction), With<ConfirmationTarget>>();
        for (entity, mut interaction) in query.iter_mut(world) {
            let next = if picked != Some(entity) { Interaction::None }
                else if pressed { Interaction::Pressed } else { Interaction::Hovered };
            if *interaction != next { *interaction = next; }
        }
    }
}

// Keep private confirmation components inside their owner module.
pub(crate) fn clear_interactions(world: &mut World) {
    let mut query = world.query_filtered::<&mut Interaction, With<ConfirmationTarget>>();
    for mut interaction in query.iter_mut(world) {
        if *interaction != Interaction::None { *interaction = Interaction::None; }
    }
}
