//! Saved interface instances and bounded, transactional event evaluation.
use crate::{
    eval::{EvalError, FunctionLibrary, UiCommand},
    random::RandomState,
};
use rvn_parser::{Statement, Value};
use rvn_ui::custom_canvas::{CanvasDrawing, CanvasFrame, MAX_CANVASES, MAX_GLOBAL_DRAW_PRIMITIVES};
use rvn_ui::programmable::{Component, ComponentKind, ScreenEventKind, ScreenView};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UiState {
    pub screens: Vec<ScreenInstance>,
    pub next_order: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScreenInstance {
    pub name: String,
    pub arguments: Vec<Value>,
    pub modal: bool,
    pub layer: i32,
    pub order: u64,
    pub focus: Option<String>,
    pub values: BTreeMap<String, Value>,
    pub random: RandomState,
    #[serde(default)]
    pub canvas_states: BTreeMap<String, CanvasState>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CanvasState {
    pub state: Value,
    pub elapsed: f64,
}

/// One shared budget for all canvas drawing callbacks or one tick transaction,
/// not a fresh 100k allowance for every nested component/function.
#[derive(Debug, Clone, Default)]
pub struct CanvasBudget {
    pub(crate) steps: usize,
    pub(crate) value_work: usize,
    primitive_count: usize,
}

pub fn validate_canvas_state(value: &Value) -> Result<(), EvalError> {
    let Value::Dict(items) = value else {
        return Err(invalid("Canvas local state must be a dictionary"));
    };
    crate::value_limits::value_work(value)?;
    let json = value_to_json(value)?;
    fn valid(value: &serde_json::Value, depth: usize) -> bool {
        if depth > 16 {
            return false;
        }
        match value {
            serde_json::Value::Object(items) => {
                items.len() <= 128
                    && items
                        .iter()
                        .all(|(key, value)| key.len() <= 4096 && valid(value, depth + 1))
            }
            serde_json::Value::Array(items) => {
                items.len() <= 512 && items.iter().all(|value| valid(value, depth + 1))
            }
            serde_json::Value::Null => false,
            _ => true,
        }
    }
    if items.len() > 128
        || serde_json::to_string(&json).map_or(true, |text| text.len() > 65_536)
        || !valid(&json, 0)
    {
        return Err(invalid(
            "Canvas local state exceeds 64 KiB, 128 dictionary entries or 16 nested levels",
        ));
    }
    Ok(())
}

pub fn canvas_frame_value(frame: CanvasFrame) -> Value {
    Value::Dict(BTreeMap::from([
        ("width".into(), Value::Float(frame.width)),
        ("height".into(), Value::Float(frame.height)),
        ("time".into(), Value::Float(frame.time as f32)),
    ]))
}

pub fn evaluate_canvas(
    functions: &FunctionLibrary,
    component: &Component,
    state: &Value,
    frame: CanvasFrame,
    globals: &HashMap<String, Value>,
    budget: &mut CanvasBudget,
) -> Result<CanvasDrawing, EvalError> {
    if component.kind != ComponentKind::Canvas {
        return Err(invalid("A canvas drawing requires a canvas component"));
    }
    validate_canvas_state(state)?;
    validate_canvas_frame(frame)?;
    let Some(name) = component.draw.as_deref() else {
        return Ok(CanvasDrawing::default());
    };
    let props = value_from_json(
        &serde_json::to_value(&component.props).map_err(|error| invalid(error.to_string()))?,
    )?;
    let value = functions.canvas_function(
        name,
        vec![state.clone(), props, canvas_frame_value(frame)],
        globals,
        budget,
    )?;
    let drawing = CanvasDrawing::parse(value_to_json(&value)?).map_err(invalid)?;
    budget.primitive_count = budget
        .primitive_count
        .checked_add(drawing.primitive_count())
        .ok_or(EvalError::NumericOverflow)?;
    if budget.primitive_count > MAX_GLOBAL_DRAW_PRIMITIVES {
        return Err(invalid(
            "All open canvases exceed the global 4096 drawing primitive limit",
        ));
    }
    Ok(drawing)
}

fn validate_canvas_frame(frame: CanvasFrame) -> Result<(), EvalError> {
    if !frame.width.is_finite()
        || !frame.height.is_finite()
        || !(0.0..=16_384.0).contains(&frame.width)
        || !(0.0..=16_384.0).contains(&frame.height)
        || !frame.time.is_finite()
        || !(0.0..=1.0e12).contains(&frame.time)
    {
        return Err(invalid(
            "Invalid canvas frame dimensions or simulation time",
        ));
    }
    Ok(())
}

/// Constructors return ordinary RVN dictionaries. Source/Blueprint conversion
/// therefore preserves the actual function expressions rather than snapshots.
pub fn canvas_construct(name: &str, args: &[Value]) -> Result<Value, EvalError> {
    let values = args
        .iter()
        .map(value_to_json)
        .collect::<Result<Vec<_>, _>>()?;
    let value = match (name, values.as_slice()) {
        ("canvas_rect", [rect, color, radius]) => {
            serde_json::json!({"kind":"rect","rect":rect,"color":color,"radius":radius})
        }
        ("canvas_ellipse", [rect, color]) => {
            serde_json::json!({"kind":"ellipse","rect":rect,"color":color})
        }
        ("canvas_line", [points, color, width]) => {
            serde_json::json!({"kind":"line","points":points,"color":color,"width":width})
        }
        ("canvas_polygon", [points, color]) => {
            serde_json::json!({"kind":"polygon","points":points,"color":color})
        }
        ("canvas_text", [text, position, color, size]) => {
            serde_json::json!({"kind":"text","text":text,"position":position,"color":color,"size":size})
        }
        ("canvas_image", [asset, rect]) => {
            serde_json::json!({"kind":"image","asset":asset,"rect":rect})
        }
        ("canvas_hit", [id, rect]) => serde_json::json!({"kind":"hit","id":id,"rect":rect}),
        ("canvas_group", [transform, clip, children]) => {
            let mut value =
                serde_json::json!({"kind":"group","transform":transform,"children":children});
            if !clip.as_array().is_some_and(Vec::is_empty) {
                value
                    .as_object_mut()
                    .unwrap()
                    .insert("clip".into(), clip.clone());
            }
            value
        }
        _ => {
            return Err(invalid(format!(
                "Invalid canvas primitive constructor arguments: {name}"
            )))
        }
    };
    CanvasDrawing::parse(serde_json::Value::Array(vec![value.clone()])).map_err(invalid)?;
    value_from_json(&value)
}

#[derive(Debug, Clone)]
pub struct UiInput {
    pub screen: String,
    pub element: String,
    pub kind: ScreenEventKind,
    pub value: Option<Value>,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UiLibrary {
    screens: HashMap<String, (Vec<String>, Vec<Statement>)>,
    handlers: HashMap<String, (Vec<String>, Vec<Statement>)>,
}

fn invalid(message: impl Into<String>) -> EvalError {
    EvalError::InvalidFunction(message.into())
}

pub fn value_to_json(value: &Value) -> Result<serde_json::Value, EvalError> {
    Ok(match value {
        Value::Int(value) => (*value).into(),
        Value::Float(value) => serde_json::Number::from_f64(f64::from(*value))
            .map(serde_json::Value::Number)
            .ok_or_else(|| invalid("nombre d’interface non fini"))?,
        Value::Bool(value) => (*value).into(),
        Value::Str(value) => value.clone().into(),
        Value::List(items) => {
            serde_json::Value::Array(items.iter().map(value_to_json).collect::<Result<_, _>>()?)
        }
        Value::Dict(items) => serde_json::Value::Object(
            items
                .iter()
                .map(|(name, value)| Ok((name.clone(), value_to_json(value)?)))
                .collect::<Result<_, EvalError>>()?,
        ),
    })
}

pub fn value_from_json(value: &serde_json::Value) -> Result<Value, EvalError> {
    Ok(match value {
        serde_json::Value::Bool(value) => Value::Bool(*value),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                Value::Int(value)
            } else {
                let value = value
                    .as_f64()
                    .ok_or_else(|| invalid("nombre d’interface invalide"))?
                    as f32;
                if !value.is_finite() {
                    return Err(invalid("nombre d’interface hors limites"));
                }
                Value::Float(value)
            }
        }
        serde_json::Value::String(value) => Value::Str(value.clone()),
        serde_json::Value::Array(items) => Value::List(
            items
                .iter()
                .map(value_from_json)
                .collect::<Result<_, _>>()?,
        ),
        serde_json::Value::Object(items) => Value::Dict(
            items
                .iter()
                .map(|(name, value)| Ok((name.clone(), value_from_json(value)?)))
                .collect::<Result<_, EvalError>>()?,
        ),
        serde_json::Value::Null => {
            return Err(invalid("une valeur de contrôle ne peut pas être null"))
        }
    })
}

impl UiLibrary {
    pub fn has_handler(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }
    pub fn media_event(
        &self,
        name: &str,
        event: Value,
        functions: &FunctionLibrary,
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> Result<(HashMap<String, Value>, Vec<UiCommand>), EvalError> {
        let (parameters, body) = self
            .handlers
            .get(name)
            .ok_or_else(|| invalid(format!("Unknown video event handler '{name}'")))?;
        functions.handle_event(parameters, body, &[event], globals, random)
    }
    pub fn from_script(script: &[Statement]) -> Result<Self, EvalError> {
        let mut library = Self::default();
        let mut names = HashSet::new();
        for statement in script {
            let (name, parameters, body, handler) = match statement {
                Statement::Screen {
                    name,
                    parameters,
                    body,
                } => (name, parameters, body, false),
                Statement::Handler {
                    name,
                    parameters,
                    body,
                } => (name, parameters, body, true),
                _ => continue,
            };
            if !rvn_parser::is_binding_name(name)
                || !names.insert(name)
                || script
                    .iter()
                    .any(|s| matches!(s, Statement::Function { name: other, .. } if other == name))
                || crate::eval::is_builtin(name)
            {
                return Err(invalid(format!(
                    "nom d’écran ou gestionnaire déjà défini ou invalide : {name}"
                )));
            }
            if parameters.len() > 128
                || parameters.iter().collect::<HashSet<_>>().len() != parameters.len()
                || parameters
                    .iter()
                    .any(|name| !rvn_parser::is_binding_name(name))
            {
                return Err(invalid(format!("paramètres invalides : {name}")));
            }
            if handler {
                if parameters.len() != 1 {
                    return Err(invalid(format!(
                        "le gestionnaire {name} doit accepter un paramètre événement"
                    )));
                }
                crate::eval::validate_handler(body)?;
                library
                    .handlers
                    .insert(name.clone(), (parameters.clone(), body.clone()));
            } else {
                // Validate before opening; evaluating needs game variables, so
                // calculation-only validation is separate from execution.
                crate::eval::validate_screen(body)?;
                library
                    .screens
                    .insert(name.clone(), (parameters.clone(), body.clone()));
            }
        }
        Ok(library)
    }

    pub fn views(
        &self,
        state: &UiState,
        functions: &FunctionLibrary,
        globals: &HashMap<String, Value>,
    ) -> Result<Vec<ScreenView>, EvalError> {
        self.resolve_views(state, functions, globals, true)
    }

    /// Resolve controls, layout and canvas frame/state metadata without running
    /// drawing callbacks. Simulation and target discovery need this structure,
    /// not a new copy of the same geometry for every check.
    pub(crate) fn structural_views(
        &self,
        state: &UiState,
        functions: &FunctionLibrary,
        globals: &HashMap<String, Value>,
    ) -> Result<Vec<ScreenView>, EvalError> {
        self.resolve_views(state, functions, globals, false)
    }

    fn resolve_views(
        &self,
        state: &UiState,
        functions: &FunctionLibrary,
        globals: &HashMap<String, Value>,
        draw: bool,
    ) -> Result<Vec<ScreenView>, EvalError> {
        if state.screens.len() > 32 {
            return Err(invalid("32 interfaces simultanées maximum"));
        }
        let mut names = HashSet::new();
        let mut orders = HashSet::new();
        let mut views = Vec::new();
        let mut canvas_count = 0;
        let mut budget = CanvasBudget::default();
        for instance in &state.screens {
            if !names.insert(&instance.name)
                || !orders.insert(instance.order)
                || instance.order >= state.next_order
                || !(-1000..=1000).contains(&instance.layer)
            {
                return Err(invalid("état d’interface sauvegardé invalide"));
            }
            if instance.values.len() > 512
                || instance.values.iter().any(|(id, value)| {
                    id.is_empty()
                        || id.len() > 128
                        || match value {
                            Value::Str(text) => text.len() > 65_536,
                            Value::Float(number) => !number.is_finite(),
                            Value::Int(_) | Value::Bool(_) => false,
                            Value::List(_) | Value::Dict(_) => true,
                        }
                })
            {
                return Err(invalid(
                    "valeurs de contrôles sauvegardées invalides ou hors limites",
                ));
            }
            if instance.arguments.len() > 128 {
                return Err(invalid("128 arguments d’interface maximum"));
            }
            if instance.canvas_states.len() > MAX_CANVASES {
                return Err(invalid("Too many saved canvas instances"));
            }
            for (id, canvas) in &instance.canvas_states {
                if id.is_empty()
                    || id.len() > 128
                    || !canvas.elapsed.is_finite()
                    || !(0.0..=1.0e12).contains(&canvas.elapsed)
                {
                    return Err(invalid("Invalid saved canvas identity or simulation time"));
                }
                validate_canvas_state(&canvas.state)?;
            }
            for argument in &instance.arguments {
                crate::value_limits::value_work(argument)?;
            }
            let (parameters, body) = self
                .screens
                .get(&instance.name)
                .ok_or_else(|| invalid(format!("écran inconnu : {}", instance.name)))?;
            let mut random = instance.random;
            let description = functions.describe_screen(
                parameters,
                body,
                &instance.arguments,
                globals,
                &mut random,
            )?;
            let mut root = Component::parse(value_to_json(&description)?).map_err(invalid)?;
            let mut error = None;
            root.visit_mut(&mut |component| {
                if let Some(binding) = &component.binding {
                    match globals.get(binding).map(value_to_json) {
                        Some(Ok(value)) => component.value = value,
                        _ => error = Some(invalid(format!("variable liée inconnue : {binding}"))),
                    }
                } else if component.is_control() {
                    if let Some(value) = instance.values.get(&component.id) {
                        match value_to_json(value) {
                            Ok(value) => component.value = value,
                            Err(problem) => error = Some(problem),
                        }
                    }
                }
                if let Err(problem) = component.validate_value(&component.value) {
                    error = Some(invalid(problem));
                }
                for handler in component.events.values() {
                    if !self.handlers.contains_key(handler) {
                        error = Some(invalid(format!(
                            "gestionnaire d’interface inconnu : {handler}"
                        )));
                    }
                }
            });
            if let Some(error) = error {
                return Err(error);
            }
            let rects =
                rvn_ui::programmable::layout_rects(&root, [1920.0, 1080.0]).map_err(invalid)?;
            let mut canvas_error = None;
            root.visit_mut(&mut |component| {
                if component.kind != ComponentKind::Canvas || canvas_error.is_some() {
                    return;
                }
                canvas_count += 1;
                if canvas_count > MAX_CANVASES {
                    canvas_error = Some(invalid(
                        "32 canvas instances maximum across all open screens",
                    ));
                    return;
                }
                if component
                    .draw
                    .as_ref()
                    .is_some_and(|name| !functions.has_function(name, 3))
                {
                    canvas_error = Some(invalid(format!(
                        "Canvas draw must name a function with three parameters: {}",
                        component.id
                    )));
                    return;
                }
                let initial = value_from_json(&serde_json::to_value(&component.state).unwrap());
                let state = instance
                    .canvas_states
                    .get(&component.id)
                    .map(|entry| Ok(entry.state.clone()))
                    .unwrap_or(initial);
                let state = match state {
                    Ok(state) => state,
                    Err(error) => {
                        canvas_error = Some(error);
                        return;
                    }
                };
                let time = instance
                    .canvas_states
                    .get(&component.id)
                    .map_or(0.0, |entry| entry.elapsed);
                let rect = rects
                    .iter()
                    .find(|rect| rect.id == component.id)
                    .map(|rect| rect.rect)
                    .unwrap_or([
                        0.0,
                        0.0,
                        component.width.unwrap_or(300.0),
                        component.height.unwrap_or(180.0),
                    ]);
                let frame = CanvasFrame {
                    width: rect[2],
                    height: rect[3],
                    time,
                };
                component.canvas_frame = Some(frame);
                if draw {
                    match evaluate_canvas(functions, component, &state, frame, globals, &mut budget)
                    {
                        Ok(drawing) => component.drawing = Some(drawing),
                        Err(error) => canvas_error = Some(error),
                    }
                } else if let Err(error) =
                    validate_canvas_state(&state).and_then(|_| validate_canvas_frame(frame))
                {
                    canvas_error = Some(error);
                };
            });
            if let Some(error) = canvas_error {
                return Err(error);
            }
            let focus_order = root.focus_order();
            let focus = instance
                .focus
                .clone()
                .filter(|id| focus_order.contains(id))
                .or_else(|| focus_order.first().cloned());
            views.push(ScreenView {
                name: instance.name.clone(),
                modal: instance.modal,
                layer: instance.layer,
                order: instance.order,
                focus,
                root,
            });
        }
        views.sort_by_key(|view| (view.layer, view.order));
        Ok(views)
    }

    /// Hydrate newly generated IDs and forget removed instances only as part
    /// of an explicit engine transaction. Reading/previewing never mutates a save.
    pub fn reconcile_canvas_states(
        &self,
        state: &mut UiState,
        functions: &FunctionLibrary,
        globals: &HashMap<String, Value>,
    ) -> Result<(), EvalError> {
        let views = self.structural_views(state, functions, globals)?;
        self.reconcile_canvas_states_from_views(state, &views)
    }

    /// Reuse an already validated description of this trial state. Missing
    /// canvases used their authored initial state and time zero in that view,
    /// exactly the state hydrated here; removed IDs only need to be forgotten.
    pub(crate) fn reconcile_canvas_states_from_views(
        &self,
        state: &mut UiState,
        views: &[ScreenView],
    ) -> Result<(), EvalError> {
        for view in views {
            let instance = state
                .screens
                .iter_mut()
                .find(|screen| screen.name == view.name)
                .unwrap();
            let mut ids = HashSet::new();
            let mut error = None;
            view.root.visit(&mut |component| {
                if component.kind != ComponentKind::Canvas {
                    return;
                }
                ids.insert(component.id.clone());
                if !instance.canvas_states.contains_key(&component.id) {
                    match value_from_json(&serde_json::to_value(&component.state).unwrap()) {
                        Ok(initial) => {
                            instance.canvas_states.insert(
                                component.id.clone(),
                                CanvasState {
                                    state: initial,
                                    elapsed: 0.0,
                                },
                            );
                        }
                        Err(problem) => error = Some(problem),
                    }
                }
            });
            if let Some(error) = error {
                return Err(error);
            }
            instance.canvas_states.retain(|id, _| ids.contains(id));
        }
        Ok(())
    }

    /// Commands operate on a caller-owned trial state. Lifecycle events are
    /// fed back through the same bounded event path before the trial commits.
    pub fn command(
        &self,
        state: &mut UiState,
        command: UiCommand,
        functions: &FunctionLibrary,
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> Result<Option<(UiInput, Component)>, EvalError> {
        match command {
            UiCommand::AccessibilityConfigure { .. }
            | UiCommand::AccessibilitySpeak { .. }
            | UiCommand::AccessibilityStop => Err(EvalError::InvalidFunction(
                "Accessibility commands must be applied by the engine transaction".into(),
            )),
            UiCommand::VideoPlay { .. }
            | UiCommand::VideoPause { .. }
            | UiCommand::VideoResume { .. }
            | UiCommand::VideoStop { .. }
            | UiCommand::VideoSkip { .. }
            | UiCommand::VideoSeek { .. }
            | UiCommand::VideoVolume { .. } => {
                return Err(invalid("Video commands must be handled by the game engine"))
            }
            UiCommand::MotionPlay { .. } | UiCommand::MotionStop { .. } => {
                return Err(invalid(
                    "Animation commands must be handled by the game engine",
                ))
            }
            UiCommand::CharacterCompose { .. } | UiCommand::CharacterAttributes { .. } => {
                return Err(invalid(
                    "Composition commands must be handled by the game engine",
                ))
            }
            UiCommand::Open {
                name,
                arguments,
                modal,
                layer,
            } => {
                if !self.screens.contains_key(&name) {
                    return Err(invalid(format!("écran inconnu : {name}")));
                }
                let old_order = state
                    .screens
                    .iter()
                    .find(|screen| screen.name == name)
                    .map(|screen| screen.order);
                let order = if let Some(order) = old_order {
                    order
                } else {
                    let order = state.next_order;
                    state.next_order = order.checked_add(1).ok_or(EvalError::NumericOverflow)?;
                    order
                };
                state.screens.retain(|screen| screen.name != name);
                state.screens.push(ScreenInstance {
                    name: name.clone(),
                    arguments,
                    modal,
                    layer,
                    order,
                    focus: None,
                    values: BTreeMap::new(),
                    random: RandomState::seeded(random.next_u64()),
                    canvas_states: BTreeMap::new(),
                });
                self.reconcile_canvas_states(state, functions, globals)?;
                let view = self
                    .views(state, functions, globals)?
                    .into_iter()
                    .find(|view| view.name == name)
                    .unwrap();
                if let Some(screen) = state.screens.iter_mut().find(|screen| screen.name == name) {
                    screen.focus = view.focus;
                }
                Ok(Some((
                    UiInput {
                        screen: name,
                        element: view.root.id.clone(),
                        kind: ScreenEventKind::Open,
                        value: None,
                        key: None,
                    },
                    view.root,
                )))
            }
            UiCommand::Close { name } => {
                let view = self
                    .views(state, functions, globals)?
                    .into_iter()
                    .find(|view| view.name == name);
                state.screens.retain(|screen| screen.name != name);
                Ok(view.map(|view| {
                    (
                        UiInput {
                            screen: name,
                            element: view.root.id.clone(),
                            kind: ScreenEventKind::Close,
                            value: None,
                            key: None,
                        },
                        view.root,
                    )
                }))
            }
            UiCommand::Focus { name, element } => {
                let view = self
                    .structural_views(state, functions, globals)?
                    .into_iter()
                    .find(|view| view.name == name)
                    .ok_or_else(|| invalid(format!("écran fermé : {name}")))?;
                if !view.root.focus_order().contains(&element) {
                    return Err(invalid(format!("élément non focalisable : {element}")));
                }
                state
                    .screens
                    .iter_mut()
                    .find(|screen| screen.name == name)
                    .unwrap()
                    .focus = Some(element.clone());
                let component = view.root.find(&element).unwrap().clone();
                Ok(Some((
                    UiInput {
                        screen: name,
                        element,
                        kind: ScreenEventKind::Focus,
                        value: None,
                        key: None,
                    },
                    component,
                )))
            }
            UiCommand::SetState {
                name,
                element,
                state: local,
            } => {
                validate_canvas_state(&local)?;
                self.reconcile_canvas_states(state, functions, globals)?;
                let instance = state
                    .screens
                    .iter_mut()
                    .find(|screen| screen.name == name)
                    .ok_or_else(|| {
                        invalid(format!(
                            "Cannot set canvas state in a closed screen: {name}"
                        ))
                    })?;
                let canvas = instance.canvas_states.get_mut(&element).ok_or_else(|| {
                    invalid(format!("Canvas instance is absent: {name}/{element}"))
                })?;
                canvas.state = local;
                Ok(None)
            }
        }
    }

    pub fn event(
        &self,
        state: &mut UiState,
        globals: &mut HashMap<String, Value>,
        event: &UiInput,
        component: &Component,
        functions: &FunctionLibrary,
        random: &mut RandomState,
    ) -> Result<Vec<UiCommand>, EvalError> {
        self.event_budgeted(
            state,
            globals,
            event,
            component,
            functions,
            random,
            &mut CanvasBudget::default(),
        )
    }

    pub fn event_budgeted(
        &self,
        state: &mut UiState,
        globals: &mut HashMap<String, Value>,
        event: &UiInput,
        component: &Component,
        functions: &FunctionLibrary,
        random: &mut RandomState,
        budget: &mut CanvasBudget,
    ) -> Result<Vec<UiCommand>, EvalError> {
        if matches!(
            event.kind,
            ScreenEventKind::PointerDown
                | ScreenEventKind::PointerMove
                | ScreenEventKind::PointerUp
                | ScreenEventKind::PointerCancel
                | ScreenEventKind::Wheel
                | ScreenEventKind::Tick
        ) && component.kind != ComponentKind::Canvas
        {
            return Err(invalid("Pointer and tick events require a canvas"));
        }
        if event.kind == ScreenEventKind::Change {
            if !component.is_control() {
                return Err(invalid("changement de valeur sur un élément sans contrôle"));
            }
            let value = event
                .value
                .clone()
                .ok_or_else(|| invalid("valeur de contrôle manquante"))?;
            component
                .validate_value(&value_to_json(&value)?)
                .map_err(invalid)?;
            if let Some(binding) = &component.binding {
                globals.insert(binding.clone(), value.clone());
            } else {
                state
                    .screens
                    .iter_mut()
                    .find(|screen| screen.name == event.screen)
                    .ok_or_else(|| invalid("écran fermé"))?
                    .values
                    .insert(event.element.clone(), value);
            }
        }
        if event.kind == ScreenEventKind::Focus {
            if !component.is_focusable() {
                return Err(invalid("élément non focalisable"));
            }
            state
                .screens
                .iter_mut()
                .find(|screen| screen.name == event.screen)
                .ok_or_else(|| invalid("écran fermé"))?
                .focus = Some(event.element.clone());
        }
        let handler = component.events.get(&event.kind).or_else(|| {
            if event.kind == ScreenEventKind::Activate {
                component.events.get(&ScreenEventKind::Click)
            } else {
                None
            }
        });
        let Some(handler) = handler else {
            return Ok(Vec::new());
        };
        let (parameters, body) = self
            .handlers
            .get(handler)
            .ok_or_else(|| invalid(format!("gestionnaire inconnu : {handler}")))?;
        let mut value = BTreeMap::from([
            ("screen".into(), Value::Str(event.screen.clone())),
            ("element".into(), Value::Str(event.element.clone())),
            (
                "kind".into(),
                Value::Str(
                    serde_json::to_value(event.kind)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .into(),
                ),
            ),
            (
                "value".into(),
                event.value.clone().unwrap_or(Value::Str(String::new())),
            ),
            (
                "key".into(),
                Value::Str(event.key.clone().unwrap_or_default()),
            ),
            (
                "data".into(),
                Value::Dict(
                    component
                        .event_data
                        .iter()
                        .map(|(name, value)| Ok((name.clone(), value_from_json(value)?)))
                        .collect::<Result<_, EvalError>>()?,
                ),
            ),
        ]);
        if component.kind == ComponentKind::Canvas {
            let entry = state
                .screens
                .iter()
                .find(|screen| screen.name == event.screen)
                .and_then(|screen| screen.canvas_states.get(&event.element));
            let local = match entry {
                Some(entry) => entry.state.clone(),
                None => value_from_json(&serde_json::to_value(&component.state).unwrap())?,
            };
            let props = value_from_json(&serde_json::to_value(&component.props).unwrap())?;
            let mut frame = component.canvas_frame.unwrap_or(CanvasFrame {
                width: component.width.unwrap_or(300.0),
                height: component.height.unwrap_or(180.0),
                time: 0.0,
            });
            if let Some(entry) = entry {
                frame.time = entry.elapsed;
            }
            value.insert("state".into(), local);
            value.insert("props".into(), props);
            value.insert("frame".into(), canvas_frame_value(frame));
        }
        let (next, commands) = functions.handle_event_budgeted(
            parameters,
            body,
            &[Value::Dict(value)],
            globals,
            random,
            budget,
        )?;
        *globals = next;
        Ok(commands)
    }
}
