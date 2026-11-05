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
    /// Opt-in real instance lifetime registry for native Legacy Screen contexts.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub instance_registry:Option<crate::scene_resources::ResourceRegistry>,
    /// Host presenters never consume the narrative allocation sequence.
    #[serde(skip)]
    pub host_next_order: u64,
}
impl UiState {
    /// Cold GameSave boundary only. Normal Clone/story parking/debug are exact.
    pub(crate) fn clear_transient_macro_cells(&mut self) {
        for instance in &mut self.screens {instance.standard_macro_frame=None;}
    }
    pub(crate) fn cold_save_story_only(&self)->Self {
        let mut next=self.story_only();next.clear_transient_macro_cells();next
    }
    pub(crate) fn story_only(&self) -> Self {
        let mut state = self.clone();
        state.screens.retain(|screen| screen.host_role.is_none());
        state.host_next_order = 0;
        if let Some(registry)=&mut state.instance_registry{registry.targets.retain(|record|!matches!(record.owner,crate::scene_tasks::TaskOwner::Host{..}));registry.tasks.retain(|task|!matches!(task.key.owner,crate::scene_tasks::TaskOwner::Host{..}));}
        state
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScreenInstance {
    pub name: String,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub widget_class:Option<crate::widget_class_source_0950::WidgetInstanceBinding>,
    /// System presenters are host-owned, never restored from a story save.
    #[serde(skip)]
    pub host_role: Option<rvn_ui::PageRole>,
    #[serde(skip)]
    pub host_root: bool,
    pub arguments: Vec<Value>,
    pub modal: bool,
    pub layer: i32,
    pub order: u64,
    pub focus: Option<String>,
    /// Validated owner class/instance projection published atomically with its
    /// optional child tree. This is never a substitute for live input authority.
    #[serde(default,skip_serializing_if="rvn_ui::programmable::ScreenHostProperties::is_default")]
    pub owner_properties: rvn_ui::programmable::ScreenHostProperties,
    /// Desired intent only; physical acquisition is source/user/wrapper gated.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub owner_focus_request:Option<rvn_ui::programmable::HostFocusRequest>,
    pub values: BTreeMap<String, Value>,
    pub random: RandomState,
    #[serde(default)]
    pub canvas_states: BTreeMap<String, CanvasState>,
    #[serde(default,skip_serializing_if="BTreeMap::is_empty")]
    pub consumer_frames:BTreeMap<String,crate::ui_consumer::BoundInstanceConsumerFrame>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub consumer_projection:Option<crate::ui_consumer::InstanceProjection>,
    /// Transient UObject-style cells: keep exact clones/warm rollback/detach;
    /// omit cold GameSave under the explicit Root StandardMacros4 policy.
    #[serde(skip)]
    pub standard_macro_frame:Option<crate::widget_class_source_0950::standard_macros::WidgetMacroFrame>,
}
impl ScreenInstance{
    /// Runtime alias and authored class definition are deliberately distinct.
    pub fn definition(&self)->&str{self.widget_class.as_ref().map_or(self.name.as_str(),|binding|binding.class.definition.as_str())}
    pub fn viewport_attached(&self)->bool{self.widget_class.as_ref().is_none_or(|binding|binding.viewport_attached)}
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
    functions: &FunctionLibrary, component: &Component, state: &Value,
    frame: CanvasFrame, globals: &HashMap<String, Value>, budget: &mut CanvasBudget,
) -> Result<CanvasDrawing, EvalError> {
    evaluate_canvas_with_locale(functions, component, state, frame, globals, budget, None)
}

pub fn evaluate_canvas_with_locale(
    functions: &FunctionLibrary,
    component: &Component,
    state: &Value,
    frame: CanvasFrame,
    globals: &HashMap<String, Value>,
    budget: &mut CanvasBudget,
    locale: Option<&crate::locale::LocaleManager>,
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
    let value = functions.canvas_function_with_locale(
        name,
        vec![state.clone(), props, canvas_frame_value(frame)],
        globals,
        budget,
        locale,
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

pub(crate) fn contains_native_plan(body:&[Statement])->bool{
    body.iter().any(|statement|matches!(statement,Statement::ScheduledConsumer{version:2|3,..})||crate::consumer_schedule::statement_children(statement).into_iter().any(contains_native_plan))
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
        Value::Text(_) => return Err(invalid("A Text value requires explicit text_to_string conversion at the JSON interface boundary")),
        Value::Name(_) => return Err(invalid("A Name value requires explicit __rvn_name_to_string conversion at the JSON interface boundary")),
        Value::Enum(_) => return Err(invalid("Enum is a nominal runtime value, not a JSON interface property")),
        Value::Widget(_) | Value::WidgetClass(_) => return Err(invalid("Widget references are typed runtime values, not JSON interface properties")),
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

/// A transient evaluation context; locale is neither global nor saved UI state.
pub struct UiEvaluation<'a> {
    library: &'a UiLibrary,
    locale: Option<&'a crate::locale::LocaleManager>,
}
impl std::ops::Deref for UiEvaluation<'_> {
    type Target = UiLibrary;
    fn deref(&self) -> &Self::Target { self.library }
}

pub(crate) fn valid_instance_layer(instance:&ScreenInstance)->bool{
    (instance.widget_class.is_some()&&instance.host_role.is_none())||(-1000..=1000).contains(&instance.layer)
}
pub(crate) fn effective_screen_layer(instance:&ScreenInstance,modal_host_roles:&std::collections::BTreeSet<rvn_ui::PageRole>)->i64{
    let layer=i64::from(instance.layer);
    if let Some(role)=instance.host_role{
        let narrative=matches!(role,rvn_ui::PageRole::Dialogue|rvn_ui::PageRole::Choices|rvn_ui::PageRole::QuickActions);
        if modal_host_roles.contains(&role){layer+if role==rvn_ui::PageRole::Confirm{9000}else if narrative{3000}else{6000}}
        else if narrative{layer-3000}else{layer}
    }else{layer}
}
fn combined_screen_layer(instance:&ScreenInstance,roles:&std::collections::BTreeSet<rvn_ui::PageRole>,slot:i32)->Result<i32,String>{
    // AddToViewport supplies this object's GLOBAL viewport order. The
    // authored descriptor slot is the same viewport scope, not an independent
    // Canvas child Z layer. Do not add that legacy Screen fallback a second time.
    if instance.widget_class.is_some()&&instance.host_role.is_none(){return Ok(instance.layer);}
    // Composition is performed in a wider domain. Neither an extreme int32
    // Widget ZOrder nor an invalid old role state can panic/wrap before refusal.
    i32::try_from(effective_screen_layer(instance,roles)+i64::from(slot))
        .map_err(|_|"Combined Widget viewport ZOrder is outside int32; no wrap or saturation".into())
}
pub fn published_owner_hosts(state:&UiState,children:&[ScreenView])->Result<Vec<rvn_ui::programmable::ScreenHostView>,String>{
    let roles=state.screens.iter().filter(|instance|instance.viewport_attached()&&instance.modal).filter_map(|instance|instance.host_role).collect();
    if state.screens.len()>32||children.len()>state.screens.len(){return Err("Invalid published UserWidget owner/child count".into());}
    let mut names=HashSet::new();let mut orders=HashSet::new();let mut hosts=Vec::new();
    for instance in &state.screens{
        instance.owner_properties.validate()?;
        if let Some(request)=&instance.owner_focus_request{request.validate()?;}
        if !names.insert(instance.name.as_str())||!orders.insert((instance.host_role.is_some(),instance.order))
            || instance.name.is_empty()||instance.name.len()>128||!valid_instance_layer(instance)
            || instance.order>=if instance.host_role.is_some(){state.host_next_order}else{state.next_order}{return Err("Invalid/duplicate published UserWidget owner identity".into());}
        if !instance.viewport_attached(){if children.iter().any(|child|child.name==instance.name){return Err("Detached Widget instance cannot publish a child".into());}continue;}
        let child=children.iter().find(|child|child.name==instance.name).cloned();
        let layer=combined_screen_layer(instance,&roles,instance.owner_properties.viewport_slot.z_order)?;
        if child.as_ref().is_some_and(|child|child.order!=instance.order||child.layer!=layer||child.modal!=instance.modal){return Err("UserWidget child and owner come from different publication identities".into());}
        hosts.push(rvn_ui::programmable::ScreenHostView{name:instance.name.clone(),modal:instance.modal,layer,order:instance.order,properties:instance.owner_properties.clone(),child});
    }
    let mut child_names=HashSet::new();
    for child in children{
        if !names.contains(child.name.as_str())||!child_names.insert(child.name.as_str()){return Err("Published child lacks a unique actual UserWidget owner".into());}
        child.root.validate()?;
    }
    hosts.sort_by_key(|host|(host.layer,host.order));Ok(hosts)
}
pub(crate) fn top_modal_owner(state:&UiState)->Result<Option<(i32,u64)>,String>{
    let roles=state.screens.iter().filter(|instance|instance.viewport_attached()&&instance.modal).filter_map(|instance|instance.host_role).collect();
    state.screens.iter().filter(|instance|instance.viewport_attached()&&instance.modal).try_fold(None::<(i32,u64)>,|highest,instance|{
        let current=(combined_screen_layer(instance,&roles,instance.owner_properties.viewport_slot.z_order)?,instance.order);
        Ok::<_,String>(Some(highest.map_or(current,|prior|prior.max(current))))
    })
}
impl UiLibrary {
    pub(crate) fn prepare_owner_properties(&self,state:&mut UiState,functions:&FunctionLibrary,globals:&HashMap<String,Value>,locale:Option<&crate::locale::LocaleManager>)->Result<(),EvalError>{
        let hosts=self.with_locale(locale).resolve_views(state,functions,globals,false,true)?;
        for host in hosts{let instance=state.screens.iter_mut().find(|instance|instance.name==host.name&&instance.order==host.order).ok_or_else(||invalid("UserWidget owner disappeared during its pure preparation"))?;instance.owner_properties=host.properties;}
        Ok(())
    }
    pub(crate) fn screen_definition(&self,name:&str)->Option<&(Vec<String>,Vec<Statement>)>{self.screens.get(name)}
    pub(crate) fn event_handler<'a>(&self,event:&UiInput,component:&'a Component)->Option<&'a str>{component.events.get(&event.kind).or_else(||if event.kind==ScreenEventKind::Activate{component.events.get(&ScreenEventKind::Click)}else{None}).map(String::as_str)}
    pub fn host_views(&self,state:&UiState,functions:&FunctionLibrary,globals:&HashMap<String,Value>)->Result<Vec<rvn_ui::programmable::ScreenHostView>,EvalError>{self.with_locale(None).host_views(state,functions,globals)}
    pub fn screen_arity(&self, name: &str) -> Option<usize> { self.screens.get(name).map(|(parameters, _)| parameters.len()) }
    pub fn has_handler(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
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
                    .any(|name| !rvn_parser::is_variable_binding_name(name))
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

    pub fn with_locale<'a>(&'a self, locale: Option<&'a crate::locale::LocaleManager>) -> UiEvaluation<'a> {
        UiEvaluation { library: self, locale }
    }
    pub fn media_event(
        &self,
        name: &str,
        event: Value,
        functions: &FunctionLibrary,
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> Result<(HashMap<String, Value>, Vec<UiCommand>), EvalError> {
        self.with_locale(None).media_event(name, event, functions, globals, random)
    }

    pub fn media_event_budgeted(
        &self,
        name: &str,
        event: Value,
        functions: &FunctionLibrary,
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
        budget: &mut CanvasBudget,
    ) -> Result<(HashMap<String, Value>, Vec<UiCommand>), EvalError> {
        self.with_locale(None).media_event_budgeted(name, event, functions, globals, random, budget)
    }

    pub fn views(
        &self,
        state: &UiState,
        functions: &FunctionLibrary,
        globals: &HashMap<String, Value>,
    ) -> Result<Vec<ScreenView>, EvalError> {
        self.with_locale(None).views(state, functions, globals)
    }

    pub(crate) fn structural_views(
        &self,
        state: &UiState,
        functions: &FunctionLibrary,
        globals: &HashMap<String, Value>,
    ) -> Result<Vec<ScreenView>, EvalError> {
        self.with_locale(None).structural_views(state, functions, globals)
    }

    pub fn reconcile_canvas_states(
        &self,
        state: &mut UiState,
        functions: &FunctionLibrary,
        globals: &HashMap<String, Value>,
    ) -> Result<(), EvalError> {
        self.with_locale(None).reconcile_canvas_states(state, functions, globals)
    }

    pub(crate) fn reconcile_canvas_states_from_views(
        &self,
        state: &mut UiState,
        views: &[ScreenView],
    ) -> Result<(), EvalError> {
        self.with_locale(None).reconcile_canvas_states_from_views(state, views)
    }

    pub fn command(
        &self,
        state: &mut UiState,
        command: UiCommand,
        functions: &FunctionLibrary,
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> Result<Option<(UiInput, Component)>, EvalError> {
        self.with_locale(None).command(state, command, functions, globals, random)
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
        self.with_locale(None).event(state, globals, event, component, functions, random)
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
        self.with_locale(None).event_budgeted(state, globals, event, component, functions, random, budget)
    }
}

impl UiEvaluation<'_> {
    pub(crate) fn owner_event_result_budgeted_frame(&self,name:&str,event:Value,functions:&FunctionLibrary,globals:&HashMap<String,Value>,random:&mut RandomState,budget:&mut CanvasBudget,frame:Option<&mut crate::consumer_schedule::ConsumerFrame>)->Result<(HashMap<String,Value>,Vec<UiCommand>,Option<Value>),EvalError>{
        self.owner_event_result_budgeted_instance_frames(name,event,functions,globals,random,budget,frame,None)
    }
    pub(crate) fn owner_event_result_budgeted_instance_frames(&self,name:&str,event:Value,functions:&FunctionLibrary,globals:&HashMap<String,Value>,random:&mut RandomState,budget:&mut CanvasBudget,frame:Option<&mut crate::consumer_schedule::ConsumerFrame>,macros:Option<&mut crate::widget_class_source_0950::standard_macros::WidgetMacroFrame>)->Result<(HashMap<String,Value>,Vec<UiCommand>,Option<Value>),EvalError>{
        let(parameters,body)=self.handlers.get(name).ok_or_else(||invalid(format!("Unknown owner event handler '{name}'")))?;
        functions.debugger_scope(format!("handler:{name}"),||functions.handle_event_result_in_instance_frames_budgeted_with_locale(
            parameters,body,&[event],globals,random,budget,self.locale,frame,macros))
    }

    pub fn media_event(
        &self,
        name: &str,
        event: Value,
        functions: &FunctionLibrary,
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> Result<(HashMap<String, Value>, Vec<UiCommand>), EvalError> {
        self.media_event_budgeted(
            name,
            event,
            functions,
            globals,
            random,
            &mut CanvasBudget::default(),
        )
    }
    pub fn media_event_budgeted(
        &self,
        name: &str,
        event: Value,
        functions: &FunctionLibrary,
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
        budget: &mut CanvasBudget,
    ) -> Result<(HashMap<String, Value>, Vec<UiCommand>), EvalError> {
        self.media_event_budgeted_frame(name,event,functions,globals,random,budget,None)
    }
    pub(crate) fn media_event_budgeted_frame(&self,name:&str,event:Value,functions:&FunctionLibrary,globals:&HashMap<String,Value>,random:&mut RandomState,budget:&mut CanvasBudget,frame:Option<&mut crate::consumer_schedule::ConsumerFrame>)->Result<(HashMap<String,Value>,Vec<UiCommand>),EvalError>{
        let (parameters, body) = self
            .handlers
            .get(name)
            .ok_or_else(|| invalid(format!("Unknown video event handler '{name}'")))?;
        functions.debugger_scope(format!("handler:{name}"),||match frame{Some(frame)=>functions.handle_event_in_consumer_frame_budgeted_with_locale(parameters,body,&[event],globals,random,budget,self.locale,frame),None=>functions.handle_event_budgeted_with_locale(parameters,body,&[event],globals,random,budget,self.locale)})
    }
    pub fn views(
        &self,
        state: &UiState,
        functions: &FunctionLibrary,
        globals: &HashMap<String, Value>,
    ) -> Result<Vec<ScreenView>, EvalError> {
        self.resolve_views(state, functions, globals, true,false).map(|hosts|hosts.into_iter().filter_map(|host|host.child).collect())
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
        self.resolve_views(state, functions, globals, false,false).map(|hosts|hosts.into_iter().filter_map(|host|host.child).collect())
    }

    pub fn host_views(&self,state:&UiState,functions:&FunctionLibrary,globals:&HashMap<String,Value>)->Result<Vec<rvn_ui::programmable::ScreenHostView>,EvalError>{self.resolve_views(state,functions,globals,true,false)}
    pub(crate) fn initialized_child_views(&self,state:&UiState,functions:&FunctionLibrary,globals:&HashMap<String,Value>)->Result<Vec<ScreenView>,EvalError>{self.resolve_views(state,functions,globals,false,true).map(|hosts|hosts.into_iter().filter_map(|host|host.child).collect())}

    fn resolve_views(
        &self,
        state: &UiState,
        functions: &FunctionLibrary,
        globals: &HashMap<String, Value>,
        draw: bool,
        include_detached:bool,
    ) -> Result<Vec<rvn_ui::programmable::ScreenHostView>, EvalError> {
        if state.screens.len() > 32 {
            return Err(invalid("32 interfaces simultanées maximum"));
        }
        let mut names = HashSet::new();
        let mut orders = HashSet::new();
        let mut views = Vec::new();
        let mut canvas_count = 0;
        let mut budget = CanvasBudget::default();
        let modal_host_roles: std::collections::BTreeSet<_> = state.screens.iter().filter(|screen| screen.viewport_attached()&&screen.modal).filter_map(|screen| screen.host_role).collect();
        for instance in &state.screens {
            instance.owner_properties.validate().map_err(invalid)?;
            if let Some(request)=&instance.owner_focus_request{request.validate().map_err(invalid)?;}
            if !names.insert(&instance.name)
                || !orders.insert((instance.host_role.is_some(), instance.order))
                || instance.order >= if instance.host_role.is_some() { state.host_next_order } else { state.next_order }
                || !valid_instance_layer(instance)
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
                            Value::Enum(_) | Value::Name(_) | Value::Text(_) | Value::Widget(_) | Value::WidgetClass(_) | Value::List(_) | Value::Dict(_) => true,
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
                .get(instance.definition())
                .ok_or_else(|| invalid(format!("écran inconnu : {}", instance.name)))?;
            let mut random = instance.random;
            let description=if let Some(projection)=&instance.consumer_projection{rvn_ui::programmable::ScreenDescription{host:projection.host.clone(),root:projection.root.clone()}}else{
                if contains_native_plan(body){return Err(invalid("Native Screen projection must be prepared once by its actual instance lifecycle"));}
                let description = functions.debugger_scope(format!("screen:{}",instance.definition()),||functions.describe_screen_with_locale(
                    parameters,body,&instance.arguments,globals,&mut random,self.locale,
                ))?;
                rvn_ui::programmable::parse_screen_description(value_to_json(&description)?).map_err(invalid)?
            };
            description.host.validate().map_err(invalid)?;
            for handler in description.host.events.values(){if !self.handlers.contains_key(handler){return Err(invalid(format!("Unknown UserWidget owner handler: {handler}")));}}
            if !instance.viewport_attached()&&!include_detached{continue;}
            // VN host-role bands and authored viewport Z form one logical
            // order. The backend maps this total order to its physical ranks.
            let layer=combined_screen_layer(instance,&modal_host_roles,description.host.viewport_slot.z_order).map_err(invalid)?;
            let mut host=rvn_ui::programmable::ScreenHostView{name:instance.name.clone(),modal:instance.modal,layer,order:instance.order,properties:description.host,child:None};
            let Some(mut root) = description.root else { views.push(host);continue; };
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
                    match evaluate_canvas_with_locale(functions, component, &state, frame, globals, &mut budget, self.locale)
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
            host.child=Some(ScreenView {
                name: instance.name.clone(),
                modal: instance.modal,
                // The system modal is above every authored story layer. Keep
                // this same effective layer for drawing and all hit tests.
                layer,
                order: instance.order,
                focus,
                root,
            });
            views.push(host);
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
        // These are child Canvas/focus IDs, never UserWidget owner focus or
        // owner lifetime. Drop only detached child state in this transaction.
        for instance in &mut state.screens {
            if !instance.viewport_attached(){continue;}
            if !views.iter().any(|view| view.name == instance.name) {
                instance.canvas_states.clear();
                instance.focus = None;
            }
        }
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
        self.command_deferred(state,command,functions,globals,random,false)
    }
    pub(crate) fn command_deferred(&self,state:&mut UiState,command:UiCommand,functions:&FunctionLibrary,globals:&HashMap<String,Value>,random:&mut RandomState,defer_description:bool)->Result<Option<(UiInput,Component)>,EvalError>{
        match command {
            UiCommand::WidgetRemoveFromParent { .. } => Err(invalid("Typed Widget removal requires the actual Engine object transaction")),
            UiCommand::Menu { .. } => Err(EvalError::InvalidMenuRequest("Menu requests must be validated by the host authority".into())),
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
                host_role,
                ..
            } => {
                if !self.screens.contains_key(&name) {
                    return Err(invalid(format!("écran inconnu : {name}")));
                }
                if state.screens.iter().any(|instance|instance.name==name&&instance.widget_class.is_some()){
                    return Err(invalid("Legacy ui.open cannot replace a typed Widget instance"));
                }
                if state.screens.iter().any(|screen| screen.name == name && screen.host_role != host_role) {
                    return Err(invalid("An interface name is owned by another presenter"));
                }
                let old_order = state
                    .screens
                    .iter()
                    .find(|screen| screen.name == name)
                    .map(|screen| screen.order);
                let order = if let Some(order) = old_order {
                    order
                } else {
                    let counter = if host_role.is_some() { &mut state.host_next_order } else { &mut state.next_order };
                    let order = *counter;
                    *counter = order.checked_add(1).ok_or(EvalError::NumericOverflow)?;
                    order
                };
                state.screens.retain(|screen| screen.name != name);
                state.screens.push(ScreenInstance {
                    host_root: false,
                    name: name.clone(),
                    widget_class:None,
                    host_role,
                    arguments,
                    modal,
                    layer,
                    order,
                    focus: None,
                    owner_properties:Default::default(),owner_focus_request:None,
                    values: BTreeMap::new(),
                    random: RandomState::seeded(if host_role.is_some() {
                        name.bytes().fold(order ^ 0x72676e6d656e75, |seed, byte| seed.wrapping_mul(1099511628211) ^ u64::from(byte))
                    } else { random.next_u64() }),
                    canvas_states: BTreeMap::new(),
                    consumer_frames:BTreeMap::new(),consumer_projection:None,standard_macro_frame:None,
                });
                if defer_description{return Ok(None);}
                self.reconcile_canvas_states(state, functions, globals)?;
                let view = self
                    .views(state, functions, globals)?
                    .into_iter()
                    .find(|view| view.name == name);
                let Some(view) = view else { return Ok(None); };
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
            UiCommand::FocusTarget{name,target,user}=>{
                target.validate().map_err(invalid)?;
                if user>4095{return Err(invalid("UserWidget focus user exceeds its domain bound"));}
                // Describe through the existing VM before mutating any intent.
                let host=self.host_views(state,functions,globals)?.into_iter().find(|host|host.name==name).ok_or_else(||invalid(format!("écran fermé : {name}")))?;
                if let rvn_ui::programmable::HostFocusTarget::Element(id)=&target{
                    if !host.child.as_ref().is_some_and(|view|view.root.focus_order().contains(id)){return Err(invalid(format!("élément non focalisable : {id}")));}
                }
                let instance=state.screens.iter_mut().find(|screen|screen.name==name).ok_or_else(||invalid("Described owner disappeared"))?;
                let serial=instance.owner_focus_request.as_ref().map_or(Ok(1),|request|request.serial.checked_add(1).ok_or(EvalError::NumericOverflow))?;
                instance.owner_focus_request=Some(rvn_ui::programmable::HostFocusRequest{target,user,serial});
                // No Component Focus event or fabricated element is generated.
                // The real accepted backend acquisition owns its notification.
                Ok(None)
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
        self.event_budgeted_frame(state,globals,event,component,functions,random,budget,None,None)
    }
    pub(crate) fn event_budgeted_frame(&self,state:&mut UiState,globals:&mut HashMap<String,Value>,event:&UiInput,component:&Component,functions:&FunctionLibrary,random:&mut RandomState,budget:&mut CanvasBudget,frame:Option<&mut crate::consumer_schedule::ConsumerFrame>,macros:Option<&mut crate::widget_class_source_0950::standard_macros::WidgetMacroFrame>)->Result<Vec<UiCommand>,EvalError>{
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
        let (next,mut commands,_)=functions.debugger_scope(format!("handler:{handler}"),||functions.handle_event_result_in_instance_frames_budgeted_with_locale(
            parameters,body,&[Value::Dict(value)],globals,random,budget,self.locale,frame,macros))?;
        for command in &mut commands {
            if let UiCommand::Menu { source, .. } = command { *source = Some(event.screen.clone()); }
            if let UiCommand::Open { host_role, inherit_host: true, .. } = command {
                *host_role = state.screens.iter().find(|screen| screen.name == event.screen).and_then(|screen| screen.host_role);
            }
        }
        *globals = next;
        Ok(commands)
    }
}

#[cfg(test)]
#[path="ui_empty_root_tests_0942.rs"]
mod empty_root_tests_0942;
