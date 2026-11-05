use crate::error::EvalErrorKind;
use crate::random::RandomState;
use rvn_parser::{BinOpKind, Expr, InterpolatedText, Statement, TextSegment, Value};
use std::collections::HashMap;

pub type EvalError = EvalErrorKind;
pub type EvalResult<T> = Result<T, EvalError>;

fn video_number(value: Value) -> EvalResult<f64> {
    match value {
        Value::Int(value) => Ok(value as f64),
        Value::Float(value) if value.is_finite() => Ok(f64::from(value)),
        _ => Err(EvalError::InvalidFunction(
            "Video time or volume needs a finite number".into(),
        )),
    }
}

// ─── ÉVALUATEUR ──────────────────────────────────────────────────────────────

pub const MAX_COMPUTATION_STEPS: usize = 100_000;
pub const MAX_FUNCTION_DEPTH: usize = 64;

#[derive(Debug, Clone, Default)]
pub struct FunctionLibrary {
    pub(crate) enum_assets1025: std::sync::Arc<crate::enum_runtime_resources1025::EnumAssets1025>,
    pub(crate) text_resources: std::sync::Arc<crate::string_table::StringTableResources>,
    // Installed only by the existing source-admitted Engine startup, before Init.
    pub(crate) widget_classes: std::sync::Arc<crate::widget_class_source_0950::AdmittedWidgetClasses>,
    pub(crate) widget_macro_sites:std::sync::Arc<crate::widget_class_source_0950::standard_macros::MacroSites>,
    definitions: HashMap<FunctionSymbol, (Vec<String>, Vec<Statement>)>,
    library_metadata: HashMap<FunctionSymbol, qualified_libraries1023::LibraryMetadata>,
    library_assets: Vec<rvn_parser::function_library_asset1023::FunctionLibraryAsset>,
    // Result/effect metadata for the same definition table, not another VM.
    unit_functions: std::collections::HashSet<FunctionSymbol>,
    pub(crate) uses_widget_object_class: bool,
    pub(crate) debug_probe: Option<debugger_eval::SharedProbe>,
    pub(crate) debug_origins: Vec<crate::editor_debugger::DebugOrigin>,
    pub(crate) debug_source_map: Option<crate::debugger_provenance::SourceMap>,
    pub(crate) debug_source_map_installed:bool,
}
#[path="eval_function_libraries1023.rs"]
mod qualified_libraries1023;
use qualified_libraries1023::FunctionSymbol;
#[path="eval_debugger.rs"]
pub(crate) mod debugger_eval;
#[path="eval_native_loops.rs"]
mod native_loops;
#[path="eval_unit_sequenced1003.rs"]
mod unit_sequenced;
use unit_sequenced::BlockExit;
#[path="eval_widget_object_class_1002.rs"]
mod widget_objects;
#[path="eval_standard_macros_1020.rs"]
mod standard_macros_1020;

#[cfg(test)]
#[path="eval_function_native_bindings_tests_0770.rs"]
mod function_native_bindings_tests;

#[derive(Debug, Clone, PartialEq)]
pub enum UiCommand {
    Menu {
        request: rvn_ui::source_menus::MenuRequest,
        source: Option<String>,
    },
    AccessibilityConfigure {
        settings: rvn_ui::accessibility::AccessibilitySettings,
    },
    AccessibilitySpeak {
        text: String,
    },
    AccessibilityStop,
    VideoPlay {
        name: String,
        definition: rvn_ui::video::VideoClip,
    },
    VideoPause {
        name: String,
    },
    VideoResume {
        name: String,
    },
    VideoStop {
        name: String,
    },
    VideoSkip {
        name: String,
    },
    VideoSeek {
        name: String,
        seconds: f64,
    },
    VideoVolume {
        name: String,
        volume: f64,
    },
    CharacterCompose {
        character: String,
        definition: rvn_ui::composition::Composition,
    },
    CharacterAttributes {
        character: String,
        attributes: rvn_ui::composition::Attributes,
    },
    MotionPlay {
        target: crate::motion::MotionTarget,
        definition: rvn_ui::motion::Motion,
    },
    MotionStop {
        target: crate::motion::MotionTarget,
    },
    Open {
        name: String,
        arguments: Vec<Value>,
        modal: bool,
        layer: i32,
        host_role: Option<rvn_ui::PageRole>,
        inherit_host: bool,
    },
    Close {
        name: String,
    },
    /// An already evaluated typed object, never a Screen alias or a second
    /// runtime. The Engine alone may stage this callback effect.
    WidgetRemoveFromParent {
        object: Value,
    },
    Focus {
        name: String,
        element: String,
    },
    FocusTarget {
        name:String,
        target:rvn_ui::programmable::HostFocusTarget,
        user:u32,
    },
    SetState {
        name: String,
        element: String,
        state: Value,
    },
}

impl UiCommand {
    pub fn evaluate(
        library: &FunctionLibrary,
        statement: &Statement,
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> EvalResult<Self> {
        Self::evaluate_scoped(library, statement, globals, globals, random)
    }

    pub fn evaluate_scoped(
        library: &FunctionLibrary,
        statement: &Statement,
        environment: &HashMap<String, Value>,
        lexical_globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> EvalResult<Self> {
        Self::evaluate_scoped_with_locale(library, statement, environment, lexical_globals, random, None)
    }

    pub fn evaluate_scoped_with_locale(
        library: &FunctionLibrary, statement: &Statement, environment: &HashMap<String, Value>,
        lexical_globals: &HashMap<String, Value>, random: &mut RandomState,
        locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<Self> {
        Self::evaluate_scoped_with_widget_objects(None,library,statement,environment,lexical_globals,random,locale)
    }

    pub(crate) fn evaluate_scoped_with_widget_objects(
        widget_objects:Option<&crate::widget_reference_values_0974::WidgetValueAdmission>,
        library: &FunctionLibrary, statement: &Statement, environment: &HashMap<String, Value>,
        lexical_globals: &HashMap<String, Value>, random: &mut RandomState,
        locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<Self> {
        let mut computation = Computation::new(library, lexical_globals);
        computation.widget_objects=widget_objects;
        computation.locale = locale;
        computation.random = *random;
        let command = computation.ui_command(statement, environment)?;
        *random = computation.random;
        Ok(command)
    }
}

#[path="eval_enum1025.rs"]
mod enum_values1025;

impl FunctionLibrary {
    pub fn with_enum_assets1025(mut self,resources:std::sync::Arc<crate::enum_runtime_resources1025::EnumAssets1025>)->Self{self.enum_assets1025=resources;self}
    pub fn text_resources(&self) -> &crate::string_table::StringTableResources { &self.text_resources }
    pub fn text_resources_mut(&mut self) -> &mut crate::string_table::StringTableResources {
        std::sync::Arc::make_mut(&mut self.text_resources)
    }
    pub fn has_function(&self, name: &str, parameters: usize) -> bool {
        !self.legacy_unit(name) && self.legacy_definition(name)
            .is_some_and(|(names, _)| names.len() == parameters)
    }

    /// A drawing callback is a pure user function. Input and random queries are
    /// rejected transitively, and all callbacks share a caller-owned budget.
    pub fn canvas_function(
        &self,
        name: &str,
        arguments: Vec<Value>,
        globals: &HashMap<String, Value>,
        budget: &mut crate::ui::CanvasBudget,
    ) -> EvalResult<Value> {
        self.canvas_function_with_locale(name, arguments, globals, budget, None)
    }

    pub fn canvas_function_with_locale(
        &self,
        name: &str,
        arguments: Vec<Value>,
        globals: &HashMap<String, Value>,
        budget: &mut crate::ui::CanvasBudget,
        locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<Value> {
        if !self.has_function(name, 3) {
            return Err(EvalError::InvalidFunction(format!(
                "Canvas draw '{name}' must reference an RVN function with three parameters"
            )));
        }
        let mut computation = Computation::new(self, globals);
        computation.locale = locale;
        computation.pure_canvas = true;
        computation.steps = budget.steps;
        computation.value_work = budget.value_work;
        let result = computation
            .tick()
            .and_then(|_| computation.call(name, arguments));
        budget.steps = computation.steps;
        budget.value_work = computation.value_work;
        result
    }
    pub fn from_script(script: &[Statement]) -> EvalResult<Self> {
        Self::from_script_with_library_assets(script,&[])
    }
    fn from_legacy_script(script: &[Statement]) -> EvalResult<Self> {
        rvn_parser::scheduled_source::native_loop_contract::validate_authored_bindings(script)
            .map_err(EvalError::InvalidFunction)?;
        let mut library = Self::default();
        library.uses_widget_object_class=widget_objects::script_uses_getter(script);
        for statement in script {
            if let Statement::Function { name, parameters, body }
                | Statement::UnitFunction { name, parameters, body } = statement
            {
                let unit=matches!(statement,Statement::UnitFunction{..});
                if !rvn_parser::is_binding_name(name)
                    || library.definitions.contains_key(&FunctionSymbol::legacy(name))
                    || is_builtin(name)
                {
                    return Err(EvalError::InvalidFunction(format!(
                        "nom déjà défini : {name}"
                    )));
                }
                if parameters.len() > 128
                    || parameters
                        .iter()
                        .any(|name| !rvn_parser::is_variable_binding_name(name))
                    || parameters
                        .iter()
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        != parameters.len()
                {
                    return Err(EvalError::InvalidFunction(format!(
                        "paramètres invalides : {name}"
                    )));
                }
                crate::flow_control::validate(body,false,0)?;
                if unit {
                    unit_sequenced::validate_body(body,0)?;
                    if script.iter().any(|s|matches!(s,Statement::Screen{name:other,..}|Statement::Handler{name:other,..}if other==name)) {
                        return Err(EvalError::InvalidFunction(format!("Unit function name collides with another callable: {name}")));
                    }
                    library.unit_functions.insert(FunctionSymbol::legacy(name));
                } else { validate_computation(body, true)?; }
                library
                    .definitions
                    .insert(FunctionSymbol::legacy(name), (parameters.clone(), body.clone()));
            }
        }
        Ok(library)
    }

    pub fn eval(&self, expression: &Expr, globals: &HashMap<String, Value>) -> EvalResult<Value> {
        self.eval_with_random(expression, globals, &mut RandomState::default())
    }

    pub fn eval_with_random(
        &self,
        expression: &Expr,
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> EvalResult<Value> {
        self.eval_scoped_with_random(expression, globals, globals, random)
    }

    /// The expression sees the caller's locals; called functions close over
    /// game globals. Passing one merged map to both would use dynamic scope.
    pub fn eval_scoped_with_random(
        &self,
        expression: &Expr,
        environment: &HashMap<String, Value>,
        lexical_globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> EvalResult<Value> {
        self.eval_scoped_with_locale(expression, environment, lexical_globals, random, None)
    }

    pub fn eval_scoped_with_locale(
        &self, expression: &Expr, environment: &HashMap<String, Value>,
        lexical_globals: &HashMap<String, Value>, random: &mut RandomState,
        locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<Value> {
        self.eval_scoped_with_locale_with_widget_objects(None, expression, environment, lexical_globals, random, locale)
    }

    pub(crate) fn eval_scoped_with_locale_with_widget_objects(
        &self, widget_objects: Option<&crate::widget_reference_values_0974::WidgetValueAdmission>, expression: &Expr, environment: &HashMap<String, Value>,
        lexical_globals: &HashMap<String, Value>, random: &mut RandomState,
        locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<Value> {
        let mut computation = Computation::new(self, lexical_globals);
        computation.widget_objects=widget_objects;
        computation.locale = locale;
        computation.random = *random;
        let value = computation.eval(expression, environment, 0)?;
        *random = computation.random;
        Ok(value)
    }

    /// Evaluate one expression using the caller's cumulative operation and
    /// value-copy budget. Failure consumes budget but commits no random draw.
    pub fn eval_with_random_budgeted(
        &self,
        expression: &Expr,
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
        budget: &mut crate::ui::CanvasBudget,
    ) -> EvalResult<Value> {
        self.eval_scoped_with_random_budgeted(expression, globals, globals, random, budget)
    }

    /// Preserve lexical function scope while sharing the enclosing transaction
    /// budget with callbacks and subsequent command selectors.
    pub fn eval_scoped_with_random_budgeted(
        &self,
        expression: &Expr,
        environment: &HashMap<String, Value>,
        lexical_globals: &HashMap<String, Value>,
        random: &mut RandomState,
        budget: &mut crate::ui::CanvasBudget,
    ) -> EvalResult<Value> {
        let mut computation = Computation::new(self, lexical_globals);
        computation.random = *random;
        computation.steps = budget.steps;
        computation.value_work = budget.value_work;
        let result = computation.eval(expression, environment, 0);
        budget.steps = computation.steps;
        budget.value_work = computation.value_work;
        if result.is_ok() {
            *random = computation.random;
        }
        result
    }

    pub fn eval_bool(
        &self,
        expression: &Expr,
        globals: &HashMap<String, Value>,
    ) -> EvalResult<bool> {
        self.eval_bool_scoped_with_locale(expression, globals, globals, &mut RandomState::default(), None)
    }

    pub fn eval_bool_with_random(
        &self,
        expression: &Expr,
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> EvalResult<bool> {
        self.eval_bool_scoped_with_locale(expression, globals, globals, random, None)
    }

    pub fn eval_bool_scoped_with_random(
        &self,
        expression: &Expr,
        environment: &HashMap<String, Value>,
        lexical_globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> EvalResult<bool> {
        self.eval_bool_scoped_with_locale(expression, environment, lexical_globals, random, None)
    }

    pub fn eval_bool_scoped_with_locale(
        &self, expression: &Expr, environment: &HashMap<String, Value>,
        lexical_globals: &HashMap<String, Value>, random: &mut RandomState,
        locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<bool> {
        self.eval_bool_scoped_with_locale_with_widget_objects(None, expression, environment, lexical_globals, random, locale)
    }

    pub(crate) fn eval_bool_scoped_with_locale_with_widget_objects(
        &self, widget_objects: Option<&crate::widget_reference_values_0974::WidgetValueAdmission>, expression: &Expr, environment: &HashMap<String, Value>,
        lexical_globals: &HashMap<String, Value>, random: &mut RandomState,
        locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<bool> {
        let mut computation = Computation::new(self, lexical_globals);
        computation.widget_objects=widget_objects;
        computation.locale = locale;
        computation.random = *random;
        let value = computation.eval(expression, environment, 0)?;
        let result = truthy_bounded(&value, Some(self.text_resources()), &mut computation.value_work)?;
        *random = computation.random;
        Ok(result)
    }

    pub fn eval_bool_with_locale(
        &self, expression: &Expr, globals: &HashMap<String, Value>,
        random: &mut RandomState, locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<bool> {
        self.eval_bool_scoped_with_locale(expression, globals, globals, random, locale)
    }

    pub(crate) fn eval_bool_with_locale_with_widget_objects(
        &self, widget_objects:Option<&crate::widget_reference_values_0974::WidgetValueAdmission>, expression: &Expr, globals: &HashMap<String, Value>,
        random: &mut RandomState, locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<bool> {
        self.eval_bool_scoped_with_locale_with_widget_objects(widget_objects, expression, globals, globals, random, locale)
    }

    pub fn interpolate(
        &self,
        text: &InterpolatedText,
        globals: &HashMap<String, Value>,
    ) -> EvalResult<String> {
        self.interpolate_with_random(text, globals, &mut RandomState::default())
    }

    pub fn interpolate_with_random(
        &self,
        text: &InterpolatedText,
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> EvalResult<String> {
        self.interpolate_scoped_with_random(text, globals, globals, random)
    }

    pub fn interpolate_with_locale(
        &self, text: &InterpolatedText, globals: &HashMap<String, Value>,
        random: &mut RandomState, locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<String> {
        self.interpolate_scoped_with_locale(text, globals, globals, random, locale)
    }

    pub(crate) fn interpolate_with_locale_with_widget_objects(
        &self, widget_objects:Option<&crate::widget_reference_values_0974::WidgetValueAdmission>, text: &InterpolatedText, globals: &HashMap<String, Value>,
        random: &mut RandomState, locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<String> {
        self.interpolate_scoped_with_locale_with_widget_objects(widget_objects, text, globals, globals, random, locale)
    }

    pub fn interpolate_scoped_with_random(
        &self,
        text: &InterpolatedText,
        environment: &HashMap<String, Value>,
        lexical_globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> EvalResult<String> {
        self.interpolate_scoped_with_locale(text, environment, lexical_globals, random, None)
    }

    pub fn interpolate_scoped_with_locale(
        &self, text: &InterpolatedText, environment: &HashMap<String, Value>,
        lexical_globals: &HashMap<String, Value>, random: &mut RandomState,
        locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<String> {
        self.interpolate_scoped_with_locale_with_widget_objects(None, text, environment, lexical_globals, random, locale)
    }

    pub(crate) fn interpolate_scoped_with_locale_with_widget_objects(
        &self, widget_objects: Option<&crate::widget_reference_values_0974::WidgetValueAdmission>, text: &InterpolatedText, environment: &HashMap<String, Value>,
        lexical_globals: &HashMap<String, Value>, random: &mut RandomState,
        locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<String> {
        let mut computation = Computation::new(self, lexical_globals);
        computation.widget_objects=widget_objects;
        computation.locale = locale;
        computation.random = *random;
        let mut result = String::new();
        for segment in &text.0 {
            let display = match segment {
                TextSegment::Lit(text) => text.clone(),
                TextSegment::Interp(expression) => {
                    let value = computation.eval(expression, environment, 0)?;
                    display_value(&value, locale, Some(self.text_resources()), &mut computation.value_work)?
                }
            };
            push_display(&mut result, &display)?;
        }
        *random = computation.random;
        Ok(result)
    }
    pub(crate) fn render_retained_choice_label(&self,text:&InterpolatedText,values:&[Value],locale:Option<&crate::locale::LocaleManager>,translated:Option<&str>)->EvalResult<String>{
        let original=text.0.iter().filter_map(|p|if let TextSegment::Interp(e)=p{Some(e)}else{None}).collect::<Vec<_>>();
        if original.len()!=values.len(){return Err(EvalError::InvalidFunction("Retained choice ports differ from its authored label".into()));}
        let template=translated.map(rvn_parser::parse_interpolated_str).transpose().map_err(|e|EvalError::InvalidFunction(e.to_string()))?;let template=template.as_ref().unwrap_or(text);let mut result=String::new();let mut work=0;
        let mut used=Vec::<(&Expr,usize)>::new();
        for part in &template.0{let display=match part{TextSegment::Lit(v)=>v.clone(),TextSegment::Interp(expr)=>{
            let position=used.iter().position(|(e,_)|*e==expr).unwrap_or_else(||{used.push((expr,0));used.len()-1});let ordinal=used[position].1;let positions=original.iter().enumerate().filter_map(|(i,e)|(*e==expr).then_some(i)).collect::<Vec<_>>();let index=positions.get(ordinal).copied().or_else(||positions.first().copied()).ok_or_else(||EvalError::InvalidFunction("Translated choice label introduces an input outside the frozen consumer ports".into()))?;used[position].1+=1;display_value(&values[index],locale,Some(self.text_resources()),&mut work)?
        }};push_display(&mut result,&display)?;}Ok(result)
    }

    /// Atomic computation: callers commit the returned variables only on success.
    pub fn execute(
        &self,
        block: &[Statement],
        globals: &HashMap<String, Value>,
    ) -> EvalResult<HashMap<String, Value>> {
        self.execute_with_random(block, globals, &mut RandomState::default())
    }

    pub fn execute_with_random(
        &self,
        block: &[Statement],
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> EvalResult<HashMap<String, Value>> {
        self.execute_with_locale(block, globals, random, None)
    }

    pub fn execute_with_locale(
        &self, block: &[Statement], globals: &HashMap<String, Value>,
        random: &mut RandomState, locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<HashMap<String, Value>> {
        self.execute_with_locale_with_widget_objects(None, block, globals, random, locale)
    }

    pub(crate) fn execute_with_locale_with_widget_objects(
        &self, widget_objects: Option<&crate::widget_reference_values_0974::WidgetValueAdmission>, block: &[Statement], globals: &HashMap<String, Value>,
        random: &mut RandomState, locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<HashMap<String, Value>> {
        if crate::consumer_schedule::contains_scheduled_consumer(block) {
            return Err(EvalError::InvalidFunction("Scheduled consumer v1 requires an explicitly owned ConsumerFrame; legacy computation entry refused".into()));
        }
        crate::flow_control::validate(block,false,0)?;
        validate_computation(block, false)?;
        let mut values = globals.clone();
        let mut computation = Computation::new(self, globals);
        computation.widget_objects=widget_objects;
        computation.locale = locale;
        computation.random = *random;
        computation.block(block, &mut values)?;
        *random = computation.random;
        Ok(values)
    }

    /// Execute against an explicitly caller-owned activation frame. Variables,
    /// RNG and terminal state commit together only after the whole block succeeds.
    /// Engine/Scene/Save attachment is a separate dependency, not implicit here.
    pub fn execute_in_consumer_frame(
        &self, block: &[Statement], globals: &HashMap<String, Value>,
        random: &mut RandomState, frame: &mut crate::consumer_schedule::ConsumerFrame,
    ) -> EvalResult<HashMap<String, Value>> {
        let storage = frame.storage()?;
        crate::flow_control::validate(block,false,0)?;
        validate_computation(block,false)?;
        let mut values = globals.clone();
        let mut computation = Computation::new(self,globals);
        computation.random = *random;
        computation.value_work = storage.bytes;
        computation.consumer_frame = frame.clone();
        computation.consumer_storage = storage;
        computation.block(block,&mut values)?;
        *random = computation.random;
        *frame = computation.consumer_frame;
        Ok(values)
    }

    /// Evaluate one real Engine consumer against separate environment/lexical
    /// globals. The returned value, RNG and frame remain candidate-owned until
    /// Engine publishes the complete state/renderer transaction.
    pub(crate) fn eval_engine_consumer(
        &self, statement: &Statement, environment: &HashMap<String,Value>,
        lexical_globals: &HashMap<String,Value>, random: &mut RandomState,
        frame: &mut crate::consumer_schedule::ConsumerFrame,
        bound_scope: &str,
        locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<Value> {
        self.eval_engine_consumer_with_widget_objects(None, statement, environment, lexical_globals, random, frame, bound_scope, locale)
    }

    pub(crate) fn eval_engine_consumer_with_widget_objects(
        &self, widget_objects: Option<&crate::widget_reference_values_0974::WidgetValueAdmission>, statement: &Statement, environment: &HashMap<String,Value>,
        lexical_globals: &HashMap<String,Value>, random: &mut RandomState,
        frame: &mut crate::consumer_schedule::ConsumerFrame,
        bound_scope: &str,
        locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<Value> {
        self.eval_engine_consumer_inner(widget_objects,statement,environment,lexical_globals,random,frame,bound_scope,locale,false).map(|(value,_)|value)
    }

    pub(crate) fn eval_engine_consumer_record(
        &self, statement: &Statement, environment: &HashMap<String,Value>,
        lexical_globals: &HashMap<String,Value>, random: &mut RandomState,
        frame: &mut crate::consumer_schedule::ConsumerFrame,
        bound_scope: &str,
        locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<(Value,debugger_eval::PortableRecord)> {
        self.eval_engine_consumer_record_with_widget_objects(None, statement, environment, lexical_globals, random, frame, bound_scope, locale)
    }

    pub(crate) fn eval_engine_consumer_record_with_widget_objects(
        &self, widget_objects: Option<&crate::widget_reference_values_0974::WidgetValueAdmission>, statement: &Statement, environment: &HashMap<String,Value>,
        lexical_globals: &HashMap<String,Value>, random: &mut RandomState,
        frame: &mut crate::consumer_schedule::ConsumerFrame,
        bound_scope: &str,
        locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<(Value,debugger_eval::PortableRecord)> {
        let(value,record)=self.eval_engine_consumer_inner(widget_objects,statement,environment,lexical_globals,random,frame,bound_scope,locale,true)?;
        Ok((value,record.ok_or_else(||EvalError::InvalidFunction("Real Native input evaluation produced no receipt".into()))?))
    }

    fn eval_engine_consumer_inner(
        &self, widget_objects: Option<&crate::widget_reference_values_0974::WidgetValueAdmission>, statement: &Statement, environment: &HashMap<String,Value>,
        lexical_globals: &HashMap<String,Value>, random: &mut RandomState,
        frame: &mut crate::consumer_schedule::ConsumerFrame,
        bound_scope: &str,
        locale: Option<&crate::locale::LocaleManager>, retain:bool,
    ) -> EvalResult<(Value,Option<debugger_eval::PortableRecord>)> {
        let Statement::ScheduledConsumer { steps, body, .. } = statement
            else { return Err(EvalError::InvalidFunction("Expected an Engine scheduled consumer".into())); };
        let dialogue=rvn_parser::scheduled_source::dialogue_text_binding(statement)
            .map_err(EvalError::InvalidFunction)?;
        let inputs=body.first().map(crate::consumer_schedule::direct_expressions).unwrap_or_default();
        let scalar=body.first().is_some_and(crate::consumer_schedule::scalar_engine_input);
        if matches!(body.as_slice(),[Statement::FunctionReturn{..}]){return Err(EvalError::InvalidFunction("Function return requires its real function activation".into()));}
        let storage = frame.storage()?;
        let mut computation = Computation::new(self,lexical_globals);
        computation.widget_objects=widget_objects;
        computation.locale = locale;
        computation.random = *random;
        computation.value_work = storage.bytes;
        computation.consumer_storage = storage;
        computation.consumer_frame = frame.clone();
        let mut variables = environment.clone();
        let native_phase=rvn_parser::scheduled_source::native_loop_contract::binding(statement)
            .map_err(EvalError::InvalidFunction)?
            .map(|binding|rvn_parser::scheduled_source::native_loop_contract::owned_engine_phase(&binding,&variables))
            .transpose().map_err(EvalError::InvalidFunction)?;
        let evaluate=|computation:&mut Computation<'_>,variables:&mut HashMap<String,Value>|->EvalResult<Value>{
            Ok(if let Some(result)=computation.debugger_consumer(statement,bound_scope,variables){result?}
            else if let Some(phase)=native_phase {computation.native_loop_input(statement,bound_scope,phase,variables)?}
            else {computation.scheduled_inner(bound_scope,steps,if let Some(binding)=dialogue{ConsumerBody::RetainedDialogue(binding.producer)}else if let[Statement::Choice{options}]=body.as_slice(){ConsumerBody::Choice(options)}else if scalar{ConsumerBody::Value(inputs[0])}else{ConsumerBody::Inputs(inputs)},variables)?
                .ok_or_else(|| EvalError::InvalidFunction("Engine consumer did not produce a value".into()))?})
        };
        let(result,record)=if retain {
            let(value,record)=computation.retain_native_consumer_input(statement,bound_scope,&mut variables,evaluate)?;
            (value,Some(record))
        }else{(evaluate(&mut computation,&mut variables)?,None)};
        *random = computation.random;
        *frame = computation.consumer_frame;
        Ok((result,record))
    }

    /// Screens have function-local state and cannot call narrative operations.
    pub fn describe_screen(
        &self,
        parameters: &[String],
        body: &[Statement],
        arguments: &[Value],
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> EvalResult<Value> {
        self.describe_screen_with_locale(parameters, body, arguments, globals, random, None)
    }

    pub fn describe_screen_with_locale(
        &self,
        parameters: &[String],
        body: &[Statement],
        arguments: &[Value],
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
        locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<Value> {
        if crate::ui::contains_native_plan(body){return Err(EvalError::InvalidFunction("Native Screen description requires its actual retained instance owner".into()));}
        crate::flow_control::validate(body,false,0)?;
        validate_computation(body, true)?;
        if parameters.len() != arguments.len() {
            return Err(EvalError::InvalidFunction(
                "nombre d’arguments d’écran incorrect".into(),
            ));
        }
        let mut locals = globals.clone();
        locals.extend(parameters.iter().cloned().zip(arguments.iter().cloned()));
        let mut computation = Computation::new(self, globals);
        computation.locale = locale;
        computation.calls = 1;
        computation.random = *random;
        let value = computation.block(body, &mut locals)?.ok_or_else(|| {
            EvalError::InvalidFunction("l’écran doit retourner une description d’interface".into())
        })?;
        *random = computation.random;
        Ok(value)
    }

    /// All updates and UI commands are returned together, never partially committed.
    pub fn handle_event(
        &self,
        parameters: &[String],
        body: &[Statement],
        arguments: &[Value],
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> EvalResult<(HashMap<String, Value>, Vec<UiCommand>)> {
        self.handle_event_budgeted(
            parameters,
            body,
            arguments,
            globals,
            random,
            &mut crate::ui::CanvasBudget::default(),
        )
    }

    pub fn handle_event_budgeted(
        &self,
        parameters: &[String],
        body: &[Statement],
        arguments: &[Value],
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
        budget: &mut crate::ui::CanvasBudget,
    ) -> EvalResult<(HashMap<String, Value>, Vec<UiCommand>)> {
        self.handle_event_budgeted_with_locale(parameters, body, arguments, globals, random, budget, None)
    }

    pub(crate) fn handle_event_in_consumer_frame_budgeted_with_locale(
        &self,parameters:&[String],body:&[Statement],arguments:&[Value],globals:&HashMap<String,Value>,random:&mut RandomState,
        budget:&mut crate::ui::CanvasBudget,locale:Option<&crate::locale::LocaleManager>,frame:&mut crate::consumer_schedule::ConsumerFrame,
    )->EvalResult<(HashMap<String,Value>,Vec<UiCommand>)>{
        self.handle_event_result_in_consumer_frame_budgeted_with_locale(parameters,body,arguments,globals,random,budget,locale,frame).map(|(variables,commands,_)|(variables,commands))
    }
    /// Same computation/retained frame, preserving the existing handler return
    /// data for owner event replies. The compatibility API still discards it.
    pub(crate) fn handle_event_result_in_consumer_frame_budgeted_with_locale(
        &self,parameters:&[String],body:&[Statement],arguments:&[Value],globals:&HashMap<String,Value>,random:&mut RandomState,
        budget:&mut crate::ui::CanvasBudget,locale:Option<&crate::locale::LocaleManager>,frame:&mut crate::consumer_schedule::ConsumerFrame,
    )->EvalResult<(HashMap<String,Value>,Vec<UiCommand>,Option<Value>)>{
        validate_handler(body)?;if parameters.len()!=arguments.len(){return Err(EvalError::InvalidFunction("Handler argument count differs".into()));}
        let storage=frame.storage()?;let mut locals=globals.clone();locals.extend(parameters.iter().cloned().zip(arguments.iter().cloned()));
        let mut computation=Computation::new(self,globals);computation.locale=locale;computation.steps=budget.steps;
        computation.value_work=budget.value_work.checked_add(storage.bytes).ok_or(EvalError::NumericOverflow)?;
        computation.handler_locals=Some(parameters.iter().cloned().collect());computation.random=*random;
        computation.consumer_frame=frame.clone();computation.consumer_storage=storage;
        let result=computation.block(body,&mut locals);budget.steps=computation.steps;budget.value_work=computation.value_work;let returned=result?;
        *random=computation.random;*frame=computation.consumer_frame;Ok((computation.globals,computation.ui_commands,returned))
    }
    pub(crate) fn describe_screen_in_consumer_frame_with_locale(
        &self,parameters:&[String],body:&[Statement],arguments:&[Value],globals:&HashMap<String,Value>,random:&mut RandomState,
        budget:&mut crate::ui::CanvasBudget,locale:Option<&crate::locale::LocaleManager>,frame:&mut crate::consumer_schedule::ConsumerFrame,
    )->EvalResult<Value>{
        validate_screen(body)?;if parameters.len()!=arguments.len(){return Err(EvalError::InvalidFunction("Screen argument count differs".into()));}
        let storage=frame.storage()?;let mut locals=globals.clone();locals.extend(parameters.iter().cloned().zip(arguments.iter().cloned()));
        let mut computation=Computation::new(self,globals);computation.locale=locale;computation.calls=1;computation.random=*random;
        computation.steps=budget.steps;computation.value_work=budget.value_work.checked_add(storage.bytes).ok_or(EvalError::NumericOverflow)?;
        computation.consumer_frame=frame.clone();computation.consumer_storage=storage;
        let result=computation.block(body,&mut locals);budget.steps=computation.steps;budget.value_work=computation.value_work;
        let value=result?.ok_or_else(||EvalError::InvalidFunction("Screen must return a real component description".into()))?;
        *random=computation.random;*frame=computation.consumer_frame;Ok(value)
    }

    pub fn handle_event_budgeted_with_locale(
        &self,parameters:&[String],body:&[Statement],arguments:&[Value],globals:&HashMap<String,Value>,random:&mut RandomState,
        budget:&mut crate::ui::CanvasBudget,locale:Option<&crate::locale::LocaleManager>,
    )->EvalResult<(HashMap<String,Value>,Vec<UiCommand>)>{
        self.handle_event_result_budgeted_with_locale(parameters,body,arguments,globals,random,budget,locale).map(|(variables,commands,_)|(variables,commands))
    }
    pub(crate) fn handle_event_result_budgeted_with_locale(
        &self,
        parameters: &[String],
        body: &[Statement],
        arguments: &[Value],
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
        budget: &mut crate::ui::CanvasBudget,
        locale: Option<&crate::locale::LocaleManager>,
    ) -> EvalResult<(HashMap<String, Value>, Vec<UiCommand>,Option<Value>)> {
        if crate::ui::contains_native_plan(body){return Err(EvalError::InvalidFunction("Native Handler requires its actual retained instance owner".into()));}
        validate_handler(body)?;
        if parameters.len() != arguments.len() {
            return Err(EvalError::InvalidFunction(
                "nombre d’arguments du gestionnaire incorrect".into(),
            ));
        }
        let mut locals = globals.clone();
        locals.extend(parameters.iter().cloned().zip(arguments.iter().cloned()));
        let mut computation = Computation::new(self, globals);
        computation.locale = locale;
        computation.steps = budget.steps;
        computation.value_work = budget.value_work;
        computation.handler_locals = Some(parameters.iter().cloned().collect());
        computation.random = *random;
        let result = computation.block(body, &mut locals);
        budget.steps = computation.steps;
        budget.value_work = computation.value_work;
        let returned=result?;
        *random = computation.random;
        Ok((computation.globals, computation.ui_commands,returned))
    }
}

fn validate_computation(block: &[Statement], returns: bool) -> EvalResult<()> {
    for statement in block {
        if rvn_parser::widget_viewport_effect_1003::parts(statement).is_some(){return Err(EvalError::InvalidFunction("Widget viewport effect requires actual Scene execution, not a pure/Handler computation".into()));}
        if rvn_parser::widget_create_effect::parts(statement).map_err(EvalError::InvalidFunction)?.is_some(){
            return Err(EvalError::InvalidFunction("Widget factory needs the actual Scene Consumer effect/result port; pure functions, Screens and Handler fallback are unsupported".into()));
        }
        match statement {
            Statement::ScheduledConsumer { version, scope, consumer, steps, body } => {
                crate::consumer_schedule::validate_plan(*version,scope,consumer,steps,body)?;
                validate_computation(body,returns)?;
            },
            Statement::SetVar { name, .. } | Statement::LocalVar { name, .. } if rvn_parser::is_variable_binding_name(name) => {}
            Statement::FunctionReturn { .. } if returns => {}
            Statement::If { then_branch, else_branch, .. } => {
                validate_computation(then_branch, returns)?;
                validate_computation(else_branch, returns)?;
            }
            Statement::Sequence { branches } => {for branch in branches {validate_computation(branch,returns)?;}},
            Statement::ForLoopWithBreak {name,body,..} if rvn_parser::is_variable_binding_name(name) => validate_computation(body,returns)?,
            Statement::LoopBreak => {}, // lexical legality checked by flow_control::validate at the public boundary
            Statement::While { body, .. } => validate_computation(body, returns)?,
            Statement::ForEach { name,body,.. } if rvn_parser::is_variable_binding_name(name) => validate_computation(body,returns)?,
            _ => return Err(EvalError::InvalidFunction("une expression ne peut pas modifier l’état persistant ou effectuer une opération narrative".into())),
        }
    }
    Ok(())
}

pub fn validate_screen(block: &[Statement]) -> EvalResult<()> {
    crate::flow_control::validate(block,false,0)?;
    validate_computation(block, true)
}

pub fn validate_handler(block: &[Statement]) -> EvalResult<()> {
    crate::flow_control::validate(block,false,0)?;
    validate_handler_inner(block)
}
fn validate_handler_inner(block: &[Statement]) -> EvalResult<()> {
    for statement in block {
        match statement {
            Statement::StandardMacroDispatch{dispatch}=>{
                dispatch.validate().map_err(EvalError::InvalidFunction)?;
                for branch in &dispatch.branches {validate_handler_inner(&branch.body)?;}
            },
            Statement::ScheduledConsumer { version, scope, consumer, steps, body } => {
                crate::consumer_schedule::validate_plan(*version,scope,consumer,steps,body)?;
                validate_handler_inner(body)?;
            },
            Statement::WidgetAddToViewport { .. } => return Err(EvalError::InvalidFunction("Widget attachment requires its actual Engine-owned Scene consumer".into())),
            Statement::WidgetRemoveFromParent { .. } | Statement::UiOpen { .. }
            | Statement::MenuExecute { .. }
            | Statement::UiClose { .. }
            | Statement::UiFocus { .. }
            | Statement::UiSetState { .. }
            | Statement::MotionPlay { .. }
            | Statement::MotionStop { .. }
            | Statement::CharacterCompose { .. }
            | Statement::CharacterAttributes { .. }
            | Statement::VideoPlay { .. }
            | Statement::VideoPause { .. }
            | Statement::VideoResume { .. }
            | Statement::VideoStop { .. }
            | Statement::VideoSkip { .. }
            | Statement::VideoSeek { .. }
            | Statement::VideoVolume { .. }
            | Statement::AccessibilityConfigure { .. }
            | Statement::AccessibilitySpeak { .. }
            | Statement::AccessibilityStop => {}
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                validate_handler_inner(then_branch)?;
                validate_handler_inner(else_branch)?;
            }
            Statement::While { body, .. } => validate_handler_inner(body)?,
            Statement::ForLoopWithBreak {name,body,..} if rvn_parser::is_variable_binding_name(name)=>validate_handler_inner(body)?,
            Statement::Sequence {branches}=>{for b in branches{validate_handler_inner(b)?;}},
            Statement::ForEach { name, body, .. } if rvn_parser::is_variable_binding_name(name) => {
                validate_handler_inner(body)?
            }
            other => validate_computation(std::slice::from_ref(other), true)?,
        }
    }
    Ok(())
}

pub fn is_builtin(name: &str) -> bool {
    rvn_parser::builtin_arity(name).is_some()
}

enum ConsumerBody<'a> { Block(&'a [Statement]), Value(&'a Expr), Inputs(Vec<&'a Expr>),Choice(&'a [rvn_parser::ChoiceOption]),RetainedDialogue(String) }
// Dynamic call-stack ownership for synchronous body evaluation only. A
// historical range masks an enclosing Native range without changing its
// legacy immediate Break contract. No saved authority or VM is introduced.
enum ComputationRangeBreak { Legacy, Native { requested: bool } }
struct Computation<'a> {
    // Set only while entering the actual canonical definition frame.
    current_function1023: Option<FunctionSymbol>,
    range_break: bool,
    range_break_owners: Vec<ComputationRangeBreak>,
    library: &'a FunctionLibrary,
    widget_objects: Option<&'a crate::widget_reference_values_0974::WidgetValueAdmission>,
    globals: HashMap<String, Value>,
    steps: usize,
    value_work: usize,
    calls: usize,
    random: RandomState,
    handler_locals: Option<std::collections::HashSet<String>>,
    unit_locals: Option<std::collections::HashSet<String>>,
    // Facts from reached Unit global assignments only. The actual owner,
    // not this evaluator, decides which injected bindings are read-only.
    unit_global_writes: std::collections::HashSet<String>,
    ui_commands: Vec<UiCommand>,
    pure_canvas: bool,
    widget_macro_frame:Option<crate::widget_class_source_0950::standard_macros::WidgetMacroFrame>,
    consumer_frame: crate::consumer_schedule::ConsumerFrame,
    consumer_storage: crate::consumer_schedule::FrameStorage,
    active_consumer: Option<crate::consumer_schedule::ActiveConsumer>,
    locale: Option<&'a crate::locale::LocaleManager>,
}

impl<'a> Computation<'a> {
    fn new(library: &'a FunctionLibrary, globals: &HashMap<String, Value>) -> Self {
        Self {
            current_function1023: None,
            range_break: false,
            range_break_owners: Vec::new(),
            library,
            widget_objects: None,
            globals: globals.clone(),
            steps: 0,
            value_work: 0,
            calls: 0,
            random: RandomState::default(),
            handler_locals: None,
            unit_locals: None,
            unit_global_writes: Default::default(),
            ui_commands: Vec::new(),
            pure_canvas: false,
            widget_macro_frame:None,
            consumer_frame: Default::default(),
            consumer_storage: Default::default(),
            active_consumer: None,
            locale: None,
        }
    }

    fn scheduled(&mut self, scope: &str,
        steps: &[rvn_parser::ConsumerStep], body: &[Statement], variables: &mut HashMap<String, Value>) -> EvalResult<Option<Value>> {
        if let [Statement::ForEach{name,collection,body}]=body {
            let value=self.scheduled_inner(scope,steps,ConsumerBody::Value(collection),variables)?.ok_or_else(||EvalError::InvalidFunction("Scheduled ForEach has no collection".into()))?;
            let Value::List(items)=value else{return Err(EvalError::InvalidFunction("for attend une liste".into()));};
            if self.calls==0{if let Some(locals)=&mut self.handler_locals{locals.insert(name.clone());}}
            for value in items{self.tick()?;variables.insert(name.clone(),value);let result=self.block(body,variables)?;if result.is_some()||self.range_break{return Ok(result);}}return Ok(None);
        }
        if let [Statement::ForLoopWithBreak{name,first,last,body}]=body {
            let value=self.scheduled_inner(scope,steps,ConsumerBody::Inputs(vec![first,last]),variables)?.ok_or_else(||EvalError::InvalidFunction("Scheduled range has no bounds".into()))?;
            let Value::List(values)=value else{unreachable!()};let(first,count)=crate::flow_control::bounds(&values[0],&values[1])?;
            if self.calls==0{if let Some(locals)=&mut self.handler_locals{locals.insert(name.clone());}}
            for offset in 0..count{self.tick()?;variables.insert(name.clone(),Value::Int(first.checked_add(offset).ok_or(EvalError::NumericOverflow)?));let result=self.legacy_range_body(body,variables)?;if result.is_some(){return Ok(result);}if self.range_break{self.range_break=false;break;}}return Ok(None);
        }
        if let [Statement::While{condition,body}]=body {
            loop {
                self.tick()?;
                let value=self.scheduled_inner(scope,steps,ConsumerBody::Value(condition),variables)?.ok_or_else(||EvalError::InvalidFunction("Scheduled While has no condition value".into()))?;
                if !truthy_bounded(&value,Some(self.library.text_resources()),&mut self.value_work)?{return Ok(None);}
                let result=self.block(body,variables)?;if result.is_some()||self.range_break{return Ok(result);}
            }
        }
        self.scheduled_inner(scope,steps,ConsumerBody::Block(body),variables)
    }

    fn scheduled_inner(&mut self, scope: &str,
        steps: &[rvn_parser::ConsumerStep], body: ConsumerBody<'_>, variables: &mut HashMap<String,Value>) -> EvalResult<Option<Value>> {
        // Public block/screen/handler entries validate plans before effects;
        // immutable function definitions validate them at library construction.
        // Rewalking a lazy expression here would do unbudgeted work per visit.
        if self.active_consumer.is_some() { return Err(EvalError::InvalidFunction("Nested consumer must use its own activation".into())); }
        self.active_consumer = Some(crate::consumer_schedule::ActiveConsumer {
            scope: scope.into(), captures: Default::default(), inspectors: Default::default(),
        });
        let result = (|| {
            self.evaluate_producer_steps(scope,steps,variables)?;
            match body {ConsumerBody::RetainedDialogue(producer)=>{
                    let value=self.active_consumer.as_ref().and_then(|a|a.captures.get(&producer))
                        .ok_or_else(||EvalError::InvalidFunction("Dynamic Dialogue lost its actual captured port".into()))?;
                    let value=crate::consumer_schedule::retained_dialogue_port(value,&mut self.value_work)?;
                    Ok(Some(Value::List(vec![value])))
                },ConsumerBody::Choice(options)=>{
                    let mut rows=Vec::new();for option in options{self.tick()?;let visible=match &option.condition{Some(c)=>truthy_bounded(&self.eval(c,variables,0)?,Some(self.library.text_resources()),&mut self.value_work)?,None=>true};let mut values=Vec::new();if visible{for part in &option.label.0{if let TextSegment::Interp(expr)=part{values.push(self.eval(expr,variables,0)?);}}}rows.push(Value::List(vec![Value::Bool(visible),Value::List(values)]));}Ok(Some(Value::List(rows)))
                },ConsumerBody::Block([Statement::If{condition,then_branch,else_branch}]) => {
                    let selected=truthy_bounded(&self.eval(condition,variables,0)?,Some(self.library.text_resources()),&mut self.value_work)?;
                    self.active_consumer=None;self.block(if selected{then_branch}else{else_branch},variables)
                },ConsumerBody::Block([Statement::Sequence{branches}])=>{self.active_consumer=None;let mut result=None;for branch in branches{result=self.block(branch,variables)?;if result.is_some()||self.range_break{break;}}Ok(result)},
                ConsumerBody::Block(body) => self.block(body,variables),
                ConsumerBody::Value(value) => self.eval(value,variables,0).map(Some),ConsumerBody::Inputs(inputs)=>{let values=inputs.into_iter().map(|value|self.eval(value,variables,0)).collect::<EvalResult<Vec<_>>>()?;Ok(Some(Value::List(values)))} }
        })();
        self.active_consumer = None;
        result
    }

    /// Same evaluator/RNG/work counters, with caller-owned lexical scratch.
    fn evaluate_producer_steps(&mut self, scope:&str, steps:&[rvn_parser::ConsumerStep], variables:&mut HashMap<String,Value>)->EvalResult<()> {
            for step in steps {
                self.tick()?;
                match step {
                    rvn_parser::ConsumerStep::Capture { producer, value } => {
                        let value = self.eval(value, variables, 0)?;
                        self.active_consumer.as_mut().unwrap().captures.insert(producer.clone(), value);
                    },
                    rvn_parser::ConsumerStep::InspectTextTable { producer, value } => {
                        let value = self.eval(value, variables, 0)?;
                        let copied = self.consumer_frame.inspect(&mut self.consumer_storage, scope, producer, value)?;
                        self.value_work = self.value_work.checked_add(copied).ok_or(EvalError::NumericOverflow)?;
                        if self.value_work > 1_000_000 {
                            return Err(EvalError::ExecutionLimit { limit: "1 000 000 copies ou caractères de valeurs par calcul" });
                        }
                        self.active_consumer.as_mut().unwrap().inspectors.insert(producer.clone());
                    },
                }
            }
        Ok(())
    }

    fn deferred_macro_value(&mut self, dispatch:&rvn_parser::scheduled_source::standard_macro_contract::Dispatch, variables:&mut HashMap<String,Value>)->EvalResult<Value> {
        let value=dispatch.operand.as_ref().ok_or_else(||EvalError::InvalidFunction("Macro operand expression missing".into()))?;
        if dispatch.deferred_operand_v1.is_none(){return self.eval(value,variables,0);}
        let prior=self.active_consumer.take();
        self.active_consumer=Some(crate::consumer_schedule::ActiveConsumer{scope:dispatch.site.scope_source(),captures:Default::default(),inspectors:Default::default()});
        let result=(||{self.evaluate_producer_steps(&dispatch.site.scope_source(),dispatch.operand_steps(),variables)?;self.eval(value,variables,0)})();
        self.active_consumer=prior;
        result
    }

    fn tick(&mut self) -> EvalResult<()> {
        self.steps += 1;
        if self.steps > MAX_COMPUTATION_STEPS {
            return Err(EvalError::ExecutionLimit {
                limit: "100 000 opérations",
            });
        }
        Ok(())
    }

    fn block(&mut self,block:&[Statement],variables:&mut HashMap<String,Value>)->EvalResult<Option<Value>> {
        match self.block_outcome(block,variables)? {
            BlockExit::Continue=>Ok(None),BlockExit::Value(value)=>Ok(Some(value)),
            BlockExit::Unit=>Err(EvalError::InvalidFunction("Unit completion cannot satisfy a legacy value/block result".into())),
        }
    }

    fn block_outcome(
        &mut self,
        block: &[Statement],
        variables: &mut HashMap<String, Value>,
    ) -> EvalResult<BlockExit> {
        if self.library.debug_probe.is_some() && unit_sequenced::contains_unit_runtime(block) {
            return Err(EvalError::InvalidFunction("Unit computation debugger activation is not yet source-admitted".into()));
        }
        if let Some(result)=self.debugger_block(block,variables){return result.map(BlockExit::from_legacy);}
        for statement in block {
            if self.range_break { return Ok(BlockExit::Continue); }
            self.tick()?;
            match statement {
                Statement::StandardMacroDispatch{dispatch}=>{
                    if let Some(value)=self.macro_dispatch(dispatch,variables)? {return Ok(BlockExit::Value(value));}
                },
                Statement::SetVar { name, value } => {
                    let value = self.eval(value, variables, 0)?;
                    let publish=self.unit_locals.as_ref().map_or_else(
                        ||self.calls==0 && !self.handler_locals.as_ref().is_some_and(|locals|locals.contains(name)),
                        |locals|!locals.contains(name));
                    if publish {
                        if self.unit_locals.is_some(){self.unit_global_writes.insert(name.clone());}
                        self.globals.insert(name.clone(), value.clone());
                    }
                    variables.insert(name.clone(), value);
                }
                Statement::LocalVar { name, value } => {
                    let value = self.eval(value, variables, 0)?;
                    if let Some(locals)=&mut self.unit_locals { locals.insert(name.clone()); }
                    if self.calls == 0 {
                        if let Some(locals) = &mut self.handler_locals {
                            locals.insert(name.clone());
                        } else {
                            return Err(EvalError::InvalidFunction(
                                "local hors d’un calcul ou gestionnaire".into(),
                            ));
                        }
                    }
                    variables.insert(name.clone(), value);
                }
                Statement::WidgetAddToViewport { .. } => return Err(EvalError::InvalidFunction("Widget attachment requires its actual Engine-owned Scene consumer".into())),
                Statement::FunctionReturn { value } => {
                    if self.unit_locals.is_some(){return Err(EvalError::InvalidFunction("A unit function cannot return a value".into()));}
                    return Ok(BlockExit::Value(self.eval(value, variables, 0)?))
                }
                Statement::UnitReturn => {
                    if self.unit_locals.is_none(){return Err(EvalError::InvalidFunction("Unit return outside its actual invocation".into()));}
                    return Ok(BlockExit::Unit);
                }
                Statement::UnitCall{name,arguments}=>{
                    if self.unit_locals.is_none(){return Err(EvalError::InvalidFunction("Unit call requires its explicit candidate entry".into()));}
                    self.invoke_unit_expressions(name,arguments,variables)?;
                    self.refresh_unit_globals(variables)?;
                },
                Statement::LibraryUnitCall{call}=>{
                    if self.unit_locals.is_none(){return Err(EvalError::InvalidFunction("Unit call requires its explicit candidate entry".into()));}
                    self.invoke_library_unit_expressions1023(call,variables)?;
                    self.refresh_unit_globals(variables)?;
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let branch = if truthy_bounded(&self.eval(condition, variables, 0)?, Some(self.library.text_resources()), &mut self.value_work)? {
                        then_branch
                    } else {
                        else_branch
                    };
                    let result=self.block_outcome(branch,variables)?;
                    if !matches!(result,BlockExit::Continue){return Ok(result);}
                }
                Statement::LoopBreak => { self.request_computation_range_break(); return Ok(BlockExit::Continue); }
                Statement::Sequence { branches } => {
                    for branch in branches {
                        let result=self.block_outcome(branch,variables)?;
                        if !matches!(result,BlockExit::Continue){return Ok(result);}
                        if self.range_break {return Ok(BlockExit::Continue);}
                    }
                }
                Statement::ForLoopWithBreak {name,first,last,body} => {
                    let first=self.eval(first,variables,0)?;let last=self.eval(last,variables,0)?;
                    let (first,count)=crate::flow_control::bounds(&first,&last)?;
                    if let Some(locals)=&mut self.unit_locals{locals.insert(name.clone());}
                    if self.calls==0 {if let Some(locals)=&mut self.handler_locals {locals.insert(name.clone());}}
                    for cursor in 0..count {
                        self.tick()?;let value=Value::Int(first.checked_add(cursor).ok_or(EvalError::NumericOverflow)?);
                        variables.insert(name.clone(),value.clone());
                        if self.calls==0&&self.handler_locals.is_none(){self.globals.insert(name.clone(),value);}
                        let result=self.range_outcome(body,variables)?;
                        if !matches!(result,BlockExit::Continue){return Ok(result);}
                        if self.range_break {self.range_break=false;break;}
                    }
                }
                Statement::While { condition, body } => {
                    while truthy_bounded(&self.eval(condition, variables, 0)?, Some(self.library.text_resources()), &mut self.value_work)? {
                        self.tick()?;
                        let result=self.block_outcome(body,variables)?;
                        if !matches!(result,BlockExit::Continue){return Ok(result);}
                        if self.range_break { break; }
                    }
                }
                Statement::ForEach {
                    name,
                    collection,
                    body,
                } => {
                    let Value::List(items) = self.eval(collection, variables, 0)? else {
                        return Err(EvalError::InvalidFunction("for attend une liste".into()));
                    };
                    if let Some(locals)=&mut self.unit_locals{locals.insert(name.clone());}
                    if self.calls == 0 {
                        if let Some(locals) = &mut self.handler_locals { locals.insert(name.clone()); }
                    }
                    for item in items {
                        self.tick()?;
                        variables.insert(name.clone(), item);
                        if self.calls == 0 && self.handler_locals.is_none() {
                            self.globals.insert(name.clone(), variables[name].clone());
                        }
                        let result=self.block_outcome(body,variables)?;
                        if !matches!(result,BlockExit::Continue){return Ok(result);}
                        if self.range_break { break; }
                    }
                }
                _ => {
                    let result=self.block_host_outcome(statement,variables)?;
                    if !matches!(result,BlockExit::Continue){return Ok(result);}
                }
            }
        }
        Ok(BlockExit::Continue)
    }

    // Keep Host/scheduled command temporaries off the recursively entered
    // lexical block frame. This is the same Computation and owner; the caller
    // already performs the one tick/debug visit and owns all shared budgets.
    fn block_host_outcome(&mut self,statement:&Statement,variables:&mut HashMap<String,Value>)->EvalResult<BlockExit> {
        match statement {
                Statement::ScheduledConsumer { version, scope, steps, body, .. } => {
                    let result=if *version==3 {self.scheduled_native_loop(statement,variables)?}
                        else {self.scheduled(scope,steps,body,variables)?};
                    if let Some(value) = result { return Ok(BlockExit::Value(value)); }
                },
            Statement::WidgetRemoveFromParent { .. } | Statement::UiOpen { .. }
                | Statement::MenuExecute { .. }
                | Statement::UiClose { .. }
                | Statement::UiFocus { .. }
                | Statement::UiSetState { .. }
                | Statement::MotionPlay { .. }
                | Statement::MotionStop { .. }
                | Statement::CharacterCompose { .. }
                | Statement::CharacterAttributes { .. }
                | Statement::VideoPlay { .. }
                | Statement::VideoPause { .. }
                | Statement::VideoResume { .. }
                | Statement::VideoStop { .. }
                | Statement::VideoSkip { .. }
                | Statement::VideoSeek { .. }
                | Statement::VideoVolume { .. }
                | Statement::AccessibilityConfigure { .. }
                | Statement::AccessibilitySpeak { .. }
                | Statement::AccessibilityStop
                    if self.calls == 0 && self.handler_locals.is_some() =>
                {
                    let command = self.ui_command(statement, variables)?;
                    if self.ui_commands.len() >= 128 {
                        return Err(EvalError::ExecutionLimit {
                            limit: "128 commandes d’interface par événement",
                        });
                    }
                    self.ui_commands.push(command);
                }
                _ => {
                    return Err(EvalError::InvalidFunction(
                        "instruction non calculable".into(),
                    ))
                }
        }
        Ok(BlockExit::Continue)
    }

    fn ui_command(
        &mut self,
        statement: &Statement,
        vars: &HashMap<String, Value>,
    ) -> EvalResult<UiCommand> {
        let string = |value: Value| match value {
            Value::Str(value) if !value.is_empty() => Ok(value),
            _ => Err(EvalError::InvalidFunction(
                "nom d’écran ou d’élément : texte non vide attendu".into(),
            )),
        };
        Ok(match statement {
            Statement::MenuExecute { request } => UiCommand::Menu {
                source: None,
                request: rvn_ui::source_menus::MenuRequest::parse(
                    crate::ui::value_to_json(&self.eval(request, vars, 0)?)
                        .map_err(|error| EvalError::InvalidMenuRequest(error.to_string()))?,
                )
                .map_err(EvalError::InvalidMenuRequest)?,
            },
            Statement::AccessibilityConfigure { settings } => UiCommand::AccessibilityConfigure {
                settings: rvn_ui::accessibility::AccessibilitySettings::parse(
                    crate::ui::value_to_json(&self.eval(settings, vars, 0)?)?,
                )
                .map_err(EvalError::InvalidFunction)?,
            },
            Statement::AccessibilitySpeak { text } => {
                let text = string(self.eval(text, vars, 0)?)?;
                rvn_ui::accessibility::validate_speech(&text)
                    .map_err(EvalError::InvalidFunction)?;
                UiCommand::AccessibilitySpeak { text }
            }
            Statement::AccessibilityStop => UiCommand::AccessibilityStop,
            Statement::VideoPlay { name, definition } => UiCommand::VideoPlay {
                name: string(self.eval(name, vars, 0)?)?,
                definition: rvn_ui::video::VideoClip::parse(crate::ui::value_to_json(
                    &self.eval(definition, vars, 0)?,
                )?)
                .map_err(EvalError::InvalidFunction)?,
            },
            Statement::VideoPause { name } => UiCommand::VideoPause {
                name: string(self.eval(name, vars, 0)?)?,
            },
            Statement::VideoResume { name } => UiCommand::VideoResume {
                name: string(self.eval(name, vars, 0)?)?,
            },
            Statement::VideoStop { name } => UiCommand::VideoStop {
                name: string(self.eval(name, vars, 0)?)?,
            },
            Statement::VideoSkip { name } => UiCommand::VideoSkip {
                name: string(self.eval(name, vars, 0)?)?,
            },
            Statement::VideoSeek { name, seconds } => UiCommand::VideoSeek {
                name: string(self.eval(name, vars, 0)?)?,
                seconds: video_number(self.eval(seconds, vars, 0)?)?,
            },
            Statement::VideoVolume { name, volume } => UiCommand::VideoVolume {
                name: string(self.eval(name, vars, 0)?)?,
                volume: video_number(self.eval(volume, vars, 0)?)?,
            },
            Statement::CharacterCompose {
                character,
                definition,
            } => UiCommand::CharacterCompose {
                character: string(self.eval(character, vars, 0)?)?,
                definition: rvn_ui::composition::Composition::parse(crate::ui::value_to_json(
                    &self.eval(definition, vars, 0)?,
                )?)
                .map_err(EvalError::InvalidFunction)?,
            },
            Statement::CharacterAttributes {
                character,
                attributes,
            } => UiCommand::CharacterAttributes {
                character: string(self.eval(character, vars, 0)?)?,
                attributes: serde_json::from_value(crate::ui::value_to_json(
                    &self.eval(attributes, vars, 0)?,
                )?)
                .map_err(|e| {
                    EvalError::InvalidFunction(format!(
                        "Character attributes need a dictionary of text values: {e}"
                    ))
                })?,
            },
            Statement::MotionPlay { target, definition } => UiCommand::MotionPlay {
                target: crate::motion::MotionTarget::parse(&string(self.eval(target, vars, 0)?)?)
                    .map_err(EvalError::InvalidFunction)?,
                definition: rvn_ui::motion::Motion::parse(crate::ui::value_to_json(
                    &self.eval(definition, vars, 0)?,
                )?)
                .map_err(EvalError::InvalidFunction)?,
            },
            Statement::MotionStop { target } => UiCommand::MotionStop {
                target: crate::motion::MotionTarget::parse(&string(self.eval(target, vars, 0)?)?)
                    .map_err(EvalError::InvalidFunction)?,
            },
            Statement::UiOpen {
                name,
                arguments,
                modal,
                layer,
                story,
            } => {
                let name = string(self.eval(name, vars, 0)?)?;
                let Value::List(arguments) = self.eval(arguments, vars, 0)? else {
                    return Err(EvalError::InvalidFunction(
                        "arguments de ui.open : liste attendue".into(),
                    ));
                };
                let Value::Bool(modal) = self.eval(modal, vars, 0)? else {
                    return Err(EvalError::InvalidFunction(
                        "modalité de ui.open : booléen attendu".into(),
                    ));
                };
                let Value::Int(layer) = self.eval(layer, vars, 0)? else {
                    return Err(EvalError::InvalidFunction(
                        "couche de ui.open : entier attendu".into(),
                    ));
                };
                let layer = i32::try_from(layer).map_err(|_| EvalError::NumericOverflow)?;
                if !(-1000..=1000).contains(&layer) {
                    return Err(EvalError::InvalidFunction(
                        "couche d’interface entre -1000 et 1000 attendue".into(),
                    ));
                }
                UiCommand::Open {
                    name,
                    arguments,
                    modal,
                    layer,
                    host_role: None,
                    inherit_host: !*story,
                }
            }
            Statement::WidgetRemoveFromParent { object } => UiCommand::WidgetRemoveFromParent {
                object: self.eval(object, vars, 0)?,
            },
            Statement::UiClose { name } => UiCommand::Close {
                name: string(self.eval(name, vars, 0)?)?,
            },
            Statement::UiFocus { name, element } => {
                let name=string(self.eval(name,vars,0)?)?;
                match self.eval(element,vars,0)?{
                    Value::Str(element)=>UiCommand::Focus{name,element},
                    value=>{let(target,user)=rvn_ui::programmable::parse_host_focus_target(crate::ui::value_to_json(&value)?).map_err(EvalError::InvalidFunction)?;UiCommand::FocusTarget{name,target,user}}
                }
            },
            Statement::UiSetState {
                name,
                element,
                state,
            } => {
                let state = self.eval(state, vars, 0)?;
                crate::ui::validate_canvas_state(&state)?;
                UiCommand::SetState {
                    name: string(self.eval(name, vars, 0)?)?,
                    element: string(self.eval(element, vars, 0)?)?,
                    state,
                }
            }
            _ => {
                return Err(EvalError::InvalidFunction(
                    "commande d’interface attendue".into(),
                ))
            }
        })
    }

    fn call(&mut self, name: &str, arguments: Vec<Value>) -> EvalResult<Value> {
        for argument in &arguments{crate::enum_runtime_resources1025::validate_value(&self.library.enum_assets1025,argument)?;}
        if self.library.legacy_unit(name){return Err(EvalError::InvalidFunction(format!("Unit function '{name}' has no expression value")));}
        if let Some(result)=self.debugger_call(name,&arguments){return result;}
        if name==enum_values1025::CONSTRUCTOR{return enum_values1025::construct(&self.library.enum_assets1025,&arguments);}
        if self.pure_canvas
            && matches!(
                name,
                "random" | "rand" | "mouse_clicked" | "mouse_x" | "mouse_y" | "key_pressed"
            )
        {
            return Err(EvalError::InvalidFunction(format!(
                "Pure canvas drawing cannot query '{name}'; use saved state and event handlers"
            )));
        }
        if name == "motion_curve" {
            let [Value::Str(function), Value::Int(samples)] = arguments.as_slice() else {
                return Err(EvalError::InvalidFunction(
                    "motion_curve(nom de fonction, 2–257 échantillons) attendu".into(),
                ));
            };
            if !(2..=257).contains(samples)
                || !self.library.has_function(function,1)
            {
                return Err(EvalError::InvalidFunction(
                    "La courbe attend une fonction RVN à un paramètre et 2–257 échantillons".into(),
                ));
            }
            let random = self.random.clone();
            let result = crate::motion::sample_curve(*samples as usize, |time| {
                self.call(function, vec![Value::Float(time)])
            });
            if self.random != random {
                self.random = random;
                return Err(EvalError::InvalidFunction(
                    "Une courbe d’animation doit être déterministe : random est interdit".into(),
                ));
            }
            return result;
        }
        if matches!(name, "__rvn_captured_v1" | "__rvn_terminal_v1") {
            let active = self.active_consumer.as_ref().ok_or_else(|| EvalError::InvalidFunction("Scheduled port access outside its consumer activation".into()))?;
            return match (name, arguments.as_slice()) {
                ("__rvn_captured_v1", [Value::Str(id)]) => active.captures.get(id).cloned()
                    .ok_or_else(|| EvalError::InvalidFunction("Unknown scheduled capture".into())),
                ("__rvn_terminal_v1", [Value::Str(id), Value::Str(port)]) if active.inspectors.contains(id) => {
                    let terminal = self.consumer_frame.terminals(&active.scope,id).ok_or_else(|| EvalError::InvalidFunction("Unknown scheduled terminal".into()))?;
                    match port.as_str() {
                        "valid" => Ok(Value::Bool(terminal.valid)),
                        "table_id" => Ok(Value::Name(terminal.table_id.clone())),
                        "key" => Ok(Value::Str(terminal.key.clone())),
                        _ => Err(EvalError::InvalidFunction("Unknown typed scheduled port".into())),
                    }
                },
                _ => Err(EvalError::InvalidFunction("Invalid scheduled port arguments".into())),
            };
        }
        let Some((parameters, body)) = self.library.legacy_definition(name) else {
            return eval_call(name, arguments, &self.globals, &mut self.random, self.locale, Some(self.library.text_resources()), &self.library.widget_classes, self.widget_objects, &mut self.value_work);
        };
        self.call_definition1023(FunctionSymbol::legacy(name), arguments)
    }

    fn eval(
        &mut self,
        expr: &Expr,
        vars: &HashMap<String, Value>,
        depth: usize,
    ) -> EvalResult<Value> {
        if let Expr::Call{name,..}=expr {if self.library.legacy_unit(name){return Err(EvalError::InvalidFunction(format!("Unit function '{name}' has no expression value")));}}
        if let Some(result)=self.debugger_expression(expr,vars,depth){return result;}
        self.tick()?;
        if depth > 256 {
            return Err(EvalError::ExecutionLimit {
                limit: "256 niveaux d’expression",
            });
        }
        let depth = depth + 1;
        let result = match expr {
            Expr::StandardMacroData{site,output,pin}=>self.macro_data(site,*output,*pin),
            Expr::Int(n) => Ok(Value::Int(*n)),
            Expr::Float(f) => Ok(Value::Float(*f)),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Str(s) => Ok(Value::Str(s.clone())),

            Expr::Var(name) => vars
                .get(name)
                .cloned()
                .ok_or_else(|| EvalError::UndefinedVar(name.clone())),

            Expr::Neg(inner) => match self.eval(inner, vars, depth)? {
                Value::Int(n) => n
                    .checked_neg()
                    .map(Value::Int)
                    .ok_or(EvalError::NumericOverflow),
                Value::Float(f) => Ok(Value::Float(-f)),
                other => Err(EvalError::TypeMismatch {
                    op: "-".into(),
                    left: type_name(&other).into(),
                    right: "".into(),
                }),
            },

            Expr::Not(inner) => Ok(Value::Bool(!truthy_bounded(&self.eval(inner, vars, depth)?, Some(self.library.text_resources()), &mut self.value_work)?)),

            Expr::And(l, r) => {
                if !truthy_bounded(&self.eval(l, vars, depth)?, Some(self.library.text_resources()), &mut self.value_work)? {
                    return Ok(Value::Bool(false));
                }
                Ok(Value::Bool(truthy_bounded(&self.eval(r, vars, depth)?, Some(self.library.text_resources()), &mut self.value_work)?))
            }
            Expr::Or(l, r) => {
                if truthy_bounded(&self.eval(l, vars, depth)?, Some(self.library.text_resources()), &mut self.value_work)? {
                    return Ok(Value::Bool(true));
                }
                Ok(Value::Bool(truthy_bounded(&self.eval(r, vars, depth)?, Some(self.library.text_resources()), &mut self.value_work)?))
            }

            Expr::BinOp { op, left, right } => {
                let left = self.eval(left, vars, depth)?;
                let right = self.eval(right, vars, depth)?;
                eval_binop(op, left, right)
            }
            Expr::Call { name, args } => {
                let values = args
                    .iter()
                    .map(|argument| self.eval(argument, vars, depth))
                    .collect::<EvalResult<_>>()?;
                self.call(name, values)
            }
            Expr::LibraryCall { call } => self.library_value1023(call, vars, depth),
            Expr::ListLit(items) => {
                let evaluated: Vec<Value> = items
                    .iter()
                    .map(|e| self.eval(e, vars, depth))
                    .collect::<Result<_, _>>()?;
                Ok(Value::List(evaluated))
            }
            Expr::Index { target, index } => {
                let target_val = self.eval(target, vars, depth)?;
                let index_val = self.eval(index, vars, depth)?;
                match (&target_val, &index_val) {
                    (Value::Dict(items), Value::Str(key)) => items
                        .get(key)
                        .cloned()
                        .ok_or_else(|| EvalError::UndefinedVar(format!("dictionary key {key}"))),
                    (Value::List(items), Value::Int(i)) => {
                        let idx = *i as usize;
                        items
                            .get(idx)
                            .cloned()
                            .ok_or_else(|| EvalError::TypeMismatch {
                                op: "index".into(),
                                left: format!("index {} out of bounds (len {})", idx, items.len()),
                                right: "".into(),
                            })
                    }
                    (Value::Str(string), Value::Int(i)) => {
                        let chars: Vec<char> = string.chars().collect();
                        let idx = *i as usize;
                        chars
                            .get(idx)
                            .map(|c| Value::Str(c.to_string()))
                            .ok_or_else(|| EvalError::TypeMismatch {
                                op: "index".into(),
                                left: format!("index {} out of bounds (len {})", idx, chars.len()),
                                right: "".into(),
                            })
                    }
                    _ => Err(EvalError::TypeMismatch {
                        op: "index".into(),
                        left: "list or string".into(),
                        right: "int".into(),
                    }),
                }
            }
        }?;
        crate::enum_runtime_resources1025::validate_value(&self.library.enum_assets1025,&result)?;
        self.value_work = self
            .value_work
            .checked_add(crate::value_limits::value_work(&result)?)
            .ok_or(EvalError::NumericOverflow)?;
        if self.value_work > 1_000_000 {
            return Err(EvalError::ExecutionLimit {
                limit: "1 000 000 copies ou caractères de valeurs par calcul",
            });
        }
        Ok(result)
    }
}

pub fn eval_expr(expr: &Expr, vars: &HashMap<String, Value>) -> EvalResult<Value> {
    FunctionLibrary::default().eval(expr, vars)
}

fn is_menu_request_constructor(name: &str) -> bool {
    matches!(
        name,
        "menu_action"
            | "menu_start_scene"
            | "menu_open_page"
            | "menu_save_page"
            | "menu_language"
            | "menu_choose"
            | "menu_gallery_cg"
            | "menu_gallery_tab"
            | "menu_confirm"
            | "menu_cancel"
            | "menu_slot"
            | "menu_protect"
            | "menu_number"
            | "menu_bool"
            | "menu_advance"
            | "menu_skip_typewriter"
    )
}
fn eval_call(
    name: &str,
    evaluated: Vec<Value>,
    vars: &HashMap<String, Value>,
    random: &mut RandomState,
    locale: Option<&crate::locale::LocaleManager>,
    resources: Option<&crate::string_table::StringTableResources>,
    widget_classes: &crate::widget_class_source_0950::AdmittedWidgetClasses,
    widget_objects: Option<&crate::widget_reference_values_0974::WidgetValueAdmission>,
    value_work: &mut usize,
) -> EvalResult<Value> {
    if let Some(arity) = rvn_parser::builtin_arity(name) {
        if !arity.contains(&evaluated.len()) {
            let message = format!("{name} : nombre d’arguments invalide");
            return Err(if is_menu_request_constructor(name) {
                EvalError::InvalidMenuRequest(message)
            } else {
                EvalError::InvalidFunction(message)
            });
        }
    }
    match name {
        "__rvn_widget_object_class_v1" => {
            let [value]=evaluated.as_slice() else { return Err(EvalError::InvalidFunction("Get Class requires one Widget object".into())); };
            crate::widget_reference_values_0974::object_class::evaluate(widget_objects,value).map_err(EvalError::InvalidFunction)
        },
        "__rvn_widget_create_v1" => return Err(EvalError::InvalidFunction("Widget factory syntax requires its reached source-owned Scene Consumer; it is not a pure producer or Handler result fallback".into())),
        "__rvn_widget_class_v1" => {
            let [Value::Str(uuid)] = evaluated.as_slice() else {
                return Err(EvalError::InvalidFunction("Widget class lookup requires one asset UUID string".into()));
            };
            let class = crate::widget_class_getter_0981::resolve_class_data(widget_classes, Some(uuid))
                .map_err(EvalError::InvalidFunction)?;
            Ok(Value::WidgetClass(Box::new(class)))
        },
        "__rvn_widget_class_none_v1" => {
            // Typed class None is distinct from a Widget object None or text.
            Ok(Value::WidgetClass(Box::new(None)))
        },
        "__rvn_string_table_asset" => {
            let [Value::Str(id)] = evaluated.as_slice() else {
                return Err(EvalError::InvalidFunction(format!("{name} expects a string-table ID")));
            };
            if !id.is_empty() {
                rvn_parser::localized_text::validate_string_table_id(id).map_err(EvalError::InvalidFunction)?;
            }
            // Resolve only against the caller's published snapshot, without IO.
            Ok(if resources.is_some_and(|r| r.ready_table(id).is_some()) {
                crate::string_table::asset_reference(id)
            } else { Value::Str(String::new()) })
        }
        "__rvn_string_table_id_name" | "__rvn_string_table_registered_name"
        | "__rvn_string_table_entry_registered_name" | "__rvn_string_table_namespace_name"
        | "__rvn_string_table_source_name" | "__rvn_string_table_metadata_name"
        | "__rvn_registered_string_tables_name" | "__rvn_string_table_keys_name"
        | "__rvn_string_table_metadata_ids_name" =>
            eval_name_string_table_query(name, &evaluated, resources, value_work),
        "__rvn_string_table_id" => {
            let id = crate::string_table::asset_reference_id(&evaluated[0])
                .map_err(EvalError::InvalidFunction)?;
            // Retained identity survives unload; querying it never reloads content.
            Ok(Value::Str(id.into()))
        }
        "__rvn_string_table_registered" | "__rvn_string_table_entry_registered" | "__rvn_string_table_namespace"
        | "__rvn_string_table_source" | "__rvn_string_table_metadata" | "__rvn_registered_string_tables"
        | "__rvn_string_table_keys" | "__rvn_string_table_metadata_ids" =>
            eval_string_table_query(name, &evaluated, resources),
        "menu_action"
        | "menu_start_scene"
        | "menu_open_page"
        | "menu_save_page"
        | "menu_language"
        | "menu_choose"
        | "menu_gallery_cg"
        | "menu_gallery_tab"
        | "menu_confirm"
        | "menu_cancel"
        | "menu_slot"
        | "menu_protect"
        | "menu_number"
        | "menu_bool"
        | "menu_advance"
        | "menu_skip_typewriter" => {
            let args = evaluated
                .iter()
                .map(crate::ui::value_to_json)
                .collect::<EvalResult<Vec<_>>>()
                .map_err(|error| EvalError::InvalidMenuRequest(error.to_string()))?;
            let value = rvn_ui::source_menus::construct(name, &args)
                .map_err(EvalError::InvalidMenuRequest)?;
            crate::ui::value_from_json(&value)
                .map_err(|error| EvalError::InvalidMenuRequest(error.to_string()))
        }
        "canvas_rect" | "canvas_ellipse" | "canvas_line" | "canvas_polygon" | "canvas_text"
        | "canvas_image" | "canvas_group" | "canvas_hit" => {
            crate::ui::canvas_construct(name, &evaluated)
        }
        "layered_image" | "image_layer" | "image_layers" => {
            crate::composition::construct(name, &evaluated)
        }
        "video_clip" => crate::video::construct(&evaluated),
        "motion_tween" | "motion_sequence" | "motion_parallel" | "motion_pause"
        | "motion_repeat" | "motion_frames" | "motion_spline" | "motion_bezier" => {
            crate::motion::construct(name, &evaluated)
        }
        "component" => match evaluated.as_slice() {
            [Value::Str(id), Value::Str(kind), Value::Dict(properties), Value::List(children)]
                if !id.is_empty() && children.len() <= 512 =>
            {
                if ["id", "kind", "children"]
                    .iter()
                    .any(|key| properties.contains_key(*key))
                {
                    return Err(EvalError::InvalidFunction("component : id, kind et children sont des arguments explicites, pas des propriétés".into()));
                }
                let mut component = properties.clone();
                component.insert("id".into(), Value::Str(id.clone()));
                component.insert("kind".into(), Value::Str(kind.clone()));
                component.insert("children".into(), Value::List(children.clone()));
                Ok(Value::Dict(component))
            }
            _ => Err(EvalError::InvalidFunction(
                "component(id texte, type texte, propriétés dictionnaire, enfants liste) attendu"
                    .into(),
            )),
        },
        "dict" | "dict_at" | "dict_get" | "dict_set" | "dict_remove" | "dict_keys"
        | "dict_values" => eval_dictionary_operation(name, &evaluated),
        "__rvn_for_bounds" => match evaluated.as_slice() {
            [first,last]=>{let (first,count)=crate::flow_control::bounds(first,last)?;Ok(Value::List(vec![Value::Int(first),Value::Int(count)]))},
            _=>Err(EvalError::InvalidFunction("ForLoopWithBreak attend deux bornes".into())),
        },
        "__rvn_iterable" => match evaluated.as_slice() {
            [Value::List(items)] => Ok(Value::List(items.clone())),
            _ => Err(EvalError::InvalidFunction("for attend une liste".into())),
        },
        "list_append" | "list_insert" | "list_remove" | "list_set" | "list_concat"
        | "list_slice" => eval_list_operation(name, &evaluated),
        "make_color" | "make_color_rgb" => {
            let nums = expect_numbers(&evaluated, "make_color")?;
            if nums.len() != 4 || nums.iter().any(|n| !n.is_finite()) {
                return Err(EvalError::TypeMismatch {
                    op: "make_color".into(),
                    left: "4 finite RGBA numbers".into(),
                    right: "".into(),
                });
            }
            let bytes: Vec<_> = nums
                .into_iter()
                .enumerate()
                .map(|(channel, n)| {
                    let maximum = if name == "make_color_rgb" && channel < 3 {
                        255.0
                    } else {
                        1.0
                    };
                    (n.clamp(0.0, maximum) / maximum * 255.0).round() as u8
                })
                .collect();
            Ok(Value::Str(format!(
                "#{:02x}{:02x}{:02x}{:02x}",
                bytes[0], bytes[1], bytes[2], bytes[3]
            )))
        }
        "min" => {
            let nums = expect_numbers(&evaluated, "min")?;
            Ok(nums
                .into_iter()
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(to_value)
                .unwrap_or(Value::Int(0)))
        }
        "max" => {
            let nums = expect_numbers(&evaluated, "max")?;
            Ok(nums
                .into_iter()
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(to_value)
                .unwrap_or(Value::Int(0)))
        }
        "abs" => match evaluated.as_slice() {
            [Value::Int(n)] => n
                .checked_abs()
                .map(Value::Int)
                .ok_or(EvalError::NumericOverflow),
            [Value::Float(f)] => Ok(Value::Float(f.abs())),
            _ => Err(EvalError::TypeMismatch {
                op: "abs".into(),
                left: "number".into(),
                right: "".into(),
            }),
        },
        "floor" => match evaluated.as_slice() {
            [Value::Float(f)] => Ok(Value::Int(*f as i64)),
            [Value::Int(n)] => Ok(Value::Int(*n)),
            _ => Err(EvalError::TypeMismatch {
                op: "floor".into(),
                left: "number".into(),
                right: "".into(),
            }),
        },
        "__rvn_bool_to_int" => match evaluated.as_slice() {
            [Value::Bool(value)] => Ok(Value::Int(i64::from(*value))),
            _ => Err(EvalError::TypeMismatch {
                op: name.into(),
                left: "bool".into(),
                right: "one boolean argument required".into(),
            }),
        },
        "__rvn_int_to_bool" => match evaluated.as_slice() {
            [Value::Int(value)] => Ok(Value::Bool(*value != 0)),
            _ => Err(EvalError::TypeMismatch {
                op: name.into(),
                left: "int".into(),
                right: "one integer argument required".into(),
            }),
        },
        "__rvn_literal_name" | "__rvn_string_to_name" => match evaluated.as_slice() {
            [Value::Str(value)] => rvn_parser::Name::new(value)
                .map(Value::Name)
                .map_err(|error| EvalError::InvalidFunction(error.to_string())),
            _ => Err(EvalError::TypeMismatch {
                op: name.into(), left: "string".into(), right: "name".into(),
            }),
        },
        "__rvn_name_to_string" => match evaluated.as_slice() {
            [Value::Name(value)] => Ok(Value::Str(value.as_str().into())),
            _ => Err(EvalError::TypeMismatch {
                op: name.into(), left: "name".into(), right: "string".into(),
            }),
        },
        "to_int" => match evaluated.as_slice() {
            // Comme la conversion String → Integer de Blueprint, un texte
            // vide ou non numérique produit 0 plutôt qu’une erreur fatale.
            [Value::Str(value)] => Ok(Value::Int(value.trim().parse::<i64>().unwrap_or(0))),
            [Value::Int(value)] => Ok(Value::Int(*value)),
            [Value::Float(value)] => Ok(Value::Int(*value as i64)),
            _ => Err(EvalError::TypeMismatch {
                op: "to_int".into(),
                left: "string or number".into(),
                right: "int".into(),
            }),
        },
        // Legacy string casts remain exact; typed Text has an explicit display
        // conversion using the caller's current locale.
        "string_to_text" | "text_to_string" => match evaluated.as_slice() {
            [Value::Str(value)] => Ok(Value::Str(value.clone())),
            [Value::Text(value)] if name == "text_to_string" =>
                display_text(value, locale, resources, value_work).map(Value::Str),
            _ => Err(EvalError::TypeMismatch {
                op: name.into(),
                left: "string".into(),
                right: "non-string value".into(),
            }),
        },
        "__rvn_text_table_valid" | "__rvn_text_table_id" | "__rvn_text_table_key" =>
            crate::localized_text::inspect_table_identity(name, &evaluated),
        "__rvn_localizable_text" | "__rvn_invariant_text" | "__rvn_empty_text" | "__rvn_format_text" | "__rvn_string_table_text" | "__rvn_string_table_text_name" =>
            crate::localized_text::construct(name, &evaluated),
        "ceil" => match evaluated.as_slice() {
            [Value::Float(f)] => Ok(Value::Int(
                (*f as i64) + if *f > 0.0 && f.fract() > 0.0 { 1 } else { 0 },
            )),
            [Value::Int(n)] => Ok(Value::Int(*n)),
            _ => Err(EvalError::TypeMismatch {
                op: "ceil".into(),
                left: "number".into(),
                right: "".into(),
            }),
        },
        "random" | "rand" => match evaluated.as_slice() {
            [Value::Int(lo), Value::Int(hi)] => {
                let lo = *lo;
                let hi = *hi;
                let val = random.integer(lo, hi).ok_or_else(|| {
                    EvalError::InvalidFunction("random : minimum supérieur au maximum".into())
                })?;
                Ok(Value::Int(val))
            }
            _ => Err(EvalError::TypeMismatch {
                op: "random".into(),
                left: "int, int".into(),
                right: "".into(),
            }),
        },
        "len" => match evaluated.as_slice() {
            [Value::Str(s)] => Ok(Value::Int(s.chars().count() as i64)),
            [Value::List(items)] => Ok(Value::Int(items.len() as i64)),
            [Value::Dict(items)] => Ok(Value::Int(items.len() as i64)),
            _ => Err(EvalError::TypeMismatch {
                op: "len".into(),
                left: "string or list".into(),
                right: "".into(),
            }),
        },
        "contains" => match evaluated.as_slice() {
            [Value::List(items), item] => Ok(Value::Bool(items.iter().any(|i| i == item))),
            [Value::Dict(items), Value::Str(key)] => Ok(Value::Bool(items.contains_key(key))),
            [Value::Str(haystack), Value::Str(needle)] => {
                Ok(Value::Bool(haystack.contains(needle.as_str())))
            }
            _ => Err(EvalError::TypeMismatch {
                op: "contains".into(),
                left: "list+value or string+string".into(),
                right: "".into(),
            }),
        },
        "upper" => match evaluated.as_slice() {
            [Value::Str(s)] => Ok(Value::Str(s.to_uppercase())),
            _ => Err(EvalError::TypeMismatch {
                op: "upper".into(),
                left: "string".into(),
                right: "".into(),
            }),
        },
        "lower" => match evaluated.as_slice() {
            [Value::Str(s)] => Ok(Value::Str(s.to_lowercase())),
            _ => Err(EvalError::TypeMismatch {
                op: "lower".into(),
                left: "string".into(),
                right: "".into(),
            }),
        },
        "capitalize" => match evaluated.as_slice() {
            [Value::Str(s)] => {
                let mut c = s.chars();
                match c.next() {
                    Some(first) => Ok(Value::Str(
                        first.to_uppercase().collect::<String>() + c.as_str(),
                    )),
                    None => Ok(Value::Str(String::new())),
                }
            }
            _ => Err(EvalError::TypeMismatch {
                op: "capitalize".into(),
                left: "string".into(),
                right: "".into(),
            }),
        },
        "trim" => match evaluated.as_slice() {
            [Value::Str(s)] => Ok(Value::Str(s.trim().to_string())),
            _ => Err(EvalError::TypeMismatch {
                op: "trim".into(),
                left: "string".into(),
                right: "".into(),
            }),
        },
        "replace" => match evaluated.as_slice() {
            [Value::Str(s), Value::Str(from), Value::Str(to)] => {
                let count = s.matches(from.as_str()).count();
                let length = s
                    .len()
                    .checked_sub(count.saturating_mul(from.len()))
                    .and_then(|length| {
                        count
                            .checked_mul(to.len())
                            .and_then(|extra| length.checked_add(extra))
                    })
                    .ok_or(EvalError::NumericOverflow)?;
                if length > crate::value_limits::MAX_VALUE_BYTES {
                    return Err(EvalError::ExecutionLimit {
                        limit: "1 Mio de texte par valeur",
                    });
                }
                Ok(Value::Str(s.replace(from.as_str(), to.as_str())))
            }
            _ => Err(EvalError::TypeMismatch {
                op: "replace".into(),
                left: "string, string, string".into(),
                right: "".into(),
            }),
        },
        "substring" => match evaluated.as_slice() {
            [Value::Str(s), Value::Int(start), Value::Int(end)] => {
                let chars: Vec<char> = s.chars().collect();
                let start = (*start as usize).min(chars.len());
                let end = (*end as usize).min(chars.len());
                if start <= end {
                    Ok(Value::Str(chars[start..end].iter().collect()))
                } else {
                    Ok(Value::Str(String::new()))
                }
            }
            _ => Err(EvalError::TypeMismatch {
                op: "substring".into(),
                left: "string, int, int".into(),
                right: "".into(),
            }),
        },
        "split" => match evaluated.as_slice() {
            [Value::Str(s), Value::Str(sep)] => {
                let parts: Vec<Value> = s
                    .split(sep.as_str())
                    .map(|p| Value::Str(p.to_string()))
                    .collect();
                Ok(Value::List(parts))
            }
            _ => Err(EvalError::TypeMismatch {
                op: "split".into(),
                left: "string, string".into(),
                right: "".into(),
            }),
        },
        "key_pressed" => match evaluated.as_slice() {
            [Value::Str(key)] => {
                let input_key = vars.get("__input_key");
                Ok(Value::Bool(input_key == Some(&Value::Str(key.clone()))))
            }
            _ => Err(EvalError::TypeMismatch {
                op: "key_pressed".into(),
                left: "string".into(),
                right: "".into(),
            }),
        },
        "mouse_clicked" => {
            let clicked = vars
                .get("__input_mouse_clicked")
                .and_then(|v| {
                    if let Value::Bool(b) = v {
                        Some(*b)
                    } else {
                        None
                    }
                })
                .unwrap_or(false);
            Ok(Value::Bool(clicked))
        }
        "mouse_x" => {
            let x = vars
                .get("__input_mouse_x")
                .and_then(|v| {
                    if let Value::Float(f) = v {
                        Some(*f)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0);
            Ok(Value::Float(x))
        }
        "mouse_y" => {
            let y = vars
                .get("__input_mouse_y")
                .and_then(|v| {
                    if let Value::Float(f) = v {
                        Some(*f)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0);
            Ok(Value::Float(y))
        }
        _ => Err(EvalError::UndefinedVar(format!(
            "unknown function `{name}`"
        ))),
    }
}

/// Read-only queries use the caller's resource snapshot, never file IO or the
/// localized/display projection. Empty source and missing entry stay distinct.
fn eval_string_table_query(name: &str, args: &[Value], resources: Option<&crate::string_table::StringTableResources>) -> EvalResult<Value> {
    let args = args.iter().map(|value| match value {
        Value::Str(value) => Ok(value.as_str()),
        _ => Err(EvalError::InvalidFunction(format!("{name} expects string identifiers"))),
    }).collect::<EvalResult<Vec<_>>>()?;
    let table = resources.and_then(|resources| args.first().and_then(|id| resources.ready_table(id)));
    let entry = table.and_then(|table| args.get(1).and_then(|key| table.entries.get(*key)));
    fn text(value: &str) -> EvalResult<Value> {
        if value.len() > crate::value_limits::MAX_VALUE_BYTES {
            return Err(EvalError::ExecutionLimit { limit: "1 Mio de texte par valeur" });
        }
        Ok(Value::Str(value.into()))
    }
    fn list<'a>(items: impl Iterator<Item = &'a str>) -> EvalResult<Value> {
        // Inspect borrowed identifiers before allocating the copied result.
        let mut result = Vec::new();
        let mut bytes = 0usize;
        for item in items {
            if result.len() >= crate::value_limits::MAX_COLLECTION_ITEMS {
                return Err(EvalError::ExecutionLimit { limit: "4 096 éléments par collection" });
            }
            bytes = bytes.checked_add(item.len()).ok_or(EvalError::NumericOverflow)?;
            if bytes > crate::value_limits::MAX_VALUE_BYTES {
                return Err(EvalError::ExecutionLimit { limit: "1 Mio de texte par valeur" });
            }
            result.push(Value::Str(item.into()));
        }
        Ok(Value::List(result))
    }
    match name {
        "__rvn_string_table_registered" => Ok(Value::Bool(table.is_some())),
        "__rvn_string_table_entry_registered" => Ok(Value::Bool(entry.is_some())),
        "__rvn_string_table_namespace" => text(table.map_or("", |table| table.namespace.as_str())),
        "__rvn_string_table_source" => text(entry.map_or("", |entry| entry.source.as_str())),
        "__rvn_string_table_metadata" => text(entry.and_then(|entry| args.get(2).and_then(|name| entry.metadata_value(name))).unwrap_or("")),
        "__rvn_registered_string_tables" => list(resources.into_iter().flat_map(|resources| resources.registered_tables().map(|table| table.id.as_str()))),
        "__rvn_string_table_keys" => list(table.into_iter().flat_map(|table| table.entries.keys().map(String::as_str))),
        "__rvn_string_table_metadata_ids" => list(entry.into_iter().flat_map(|entry| entry.metadata.keys().map(String::as_str))),
        _ => Err(EvalError::InvalidFunction(format!("Unknown string-table query {name}"))),
    }
}

/// Collection edits return a new value. Explicit assignment makes them
/// participate in ordinary saves and rollback without hidden shared mutation.
fn eval_dictionary_operation(name: &str, values: &[Value]) -> EvalResult<Value> {
    let invalid =
        || EvalError::InvalidFunction(format!("{name} : dictionnaire et clé texte attendus"));
    if name == "dict" {
        if values.len() % 2 != 0 {
            return Err(invalid());
        }
        let mut dictionary = std::collections::BTreeMap::new();
        for pair in values.chunks_exact(2) {
            let Value::Str(key) = &pair[0] else {
                return Err(invalid());
            };
            if dictionary.insert(key.clone(), pair[1].clone()).is_some() {
                return Err(EvalError::InvalidFunction(format!("clé dupliquée : {key}")));
            }
        }
        return Ok(Value::Dict(dictionary));
    }
    let Some(Value::Dict(dictionary)) = values.first() else {
        return Err(invalid());
    };
    match (name, values) {
        ("dict_keys", [_]) => Ok(Value::List(
            dictionary.keys().cloned().map(Value::Str).collect(),
        )),
        ("dict_values", [_]) => Ok(Value::List(dictionary.values().cloned().collect())),
        ("dict_get", [_, Value::Str(key), fallback]) => {
            Ok(dictionary.get(key).unwrap_or(fallback).clone())
        }
        ("dict_at", [_, Value::Str(key)]) => dictionary
            .get(key)
            .cloned()
            .ok_or_else(|| EvalError::UndefinedVar(format!("dictionary key {key}"))),
        ("dict_set", [_, Value::Str(key), value]) => {
            let mut result = dictionary.clone();
            result.insert(key.clone(), value.clone());
            Ok(Value::Dict(result))
        }
        ("dict_remove", [_, Value::Str(key)]) => {
            let mut result = dictionary.clone();
            result.remove(key);
            Ok(Value::Dict(result))
        }
        _ => Err(invalid()),
    }
}

fn eval_list_operation(name: &str, values: &[Value]) -> EvalResult<Value> {
    let error = |expected: &str| EvalError::TypeMismatch {
        op: name.into(),
        left: expected.into(),
        right: values.iter().map(type_name).collect::<Vec<_>>().join(", "),
    };
    let Some(Value::List(original)) = values.first() else {
        return Err(error("list as first argument"));
    };
    let mut items = original.clone();
    let index = |value: &Value, allow_end: bool| -> EvalResult<usize> {
        let Value::Int(raw) = value else {
            return Err(error("integer index"));
        };
        let position = usize::try_from(*raw).map_err(|_| error("non-negative index"))?;
        if position > original.len() || (!allow_end && position == original.len()) {
            return Err(error("index within collection bounds"));
        }
        Ok(position)
    };
    match (name, values) {
        ("list_append", [_, item]) => items.push(item.clone()),
        ("list_insert", [_, at, item]) => items.insert(index(at, true)?, item.clone()),
        ("list_remove", [_, at]) => {
            items.remove(index(at, false)?);
        }
        ("list_set", [_, at, item]) => items[index(at, false)?] = item.clone(),
        ("list_concat", [_, Value::List(other)]) => items.extend(other.iter().cloned()),
        ("list_slice", [_, from, to]) => {
            let start = index(from, true)?;
            let end = index(to, true)?;
            if start > end {
                return Err(error("slice start <= end"));
            }
            items = original[start..end].to_vec();
        }
        _ => {
            return Err(error(match name {
                "list_append" => "list, value",
                "list_insert" | "list_set" => "list, index, value",
                "list_remove" => "list, index",
                "list_concat" => "list, list",
                _ => "list, start, end",
            }))
        }
    }
    Ok(Value::List(items))
}

fn expect_numbers(vals: &[Value], _fn_name: &str) -> EvalResult<Vec<f64>> {
    vals.iter()
        .map(|v| match v {
            Value::Int(n) => Ok(*n as f64),
            Value::Float(f) => Ok(*f as f64),
            _ => Err(EvalError::TypeMismatch {
                op: "numeric".into(),
                left: "number".into(),
                right: "".into(),
            }),
        })
        .collect()
}

fn to_value(f: f64) -> Value {
    if f.fract() == 0.0 && f.is_finite() {
        Value::Int(f as i64)
    } else {
        Value::Float(f as f32)
    }
}

pub fn eval_bool(expr: &Expr, vars: &HashMap<String, Value>) -> EvalResult<bool> {
    truthy(&eval_expr(expr, vars)?, None)
}

fn truthy(value: &Value, resources: Option<&crate::string_table::StringTableResources>) -> EvalResult<bool> {
    truthy_bounded(value, resources, &mut 0)
}

pub(crate) fn truthy_bounded(value: &Value, resources: Option<&crate::string_table::StringTableResources>, work: &mut usize) -> EvalResult<bool> {
    Ok(match value {
        Value::Bool(b) => *b,
        Value::Int(n) => *n != 0,
        Value::Float(f) => *f != 0.0,
        Value::Str(s) => !s.is_empty(),
        Value::Name(name) => !name.is_none(),
        Value::Enum(_) => return Err(EvalError::InvalidFunction("Enum has no implicit boolean conversion".into())),
        Value::WidgetClass(_) | Value::Widget(_) => return Err(EvalError::InvalidFunction("Widget references require explicit runtime validation, not boolean coercion".into())),
        Value::Text(text) => if text.has_string_table().map_err(EvalError::InvalidFunction)? {
            !display_text(text, None, resources, work)?.is_empty()
        } else { text.source_display().is_ok_and(|s| !s.is_empty()) },
        Value::List(items) => !items.is_empty(),
        Value::Dict(items) => !items.is_empty(),
    })
}

fn display_text(text: &rvn_parser::localized_text::DeferredText, locale: Option<&crate::locale::LocaleManager>,
    resources: Option<&crate::string_table::StringTableResources>, work: &mut usize) -> EvalResult<String> {
    let referenced = text.has_string_table().map_err(EvalError::InvalidFunction)?;
    let (display, cost) = crate::localized_text::render_text_with_resources_work(text, locale,
        &crate::localized_text::VnTextScalars, resources).map_err(EvalError::InvalidFunction)?;
    if referenced {
        *work = work.checked_add(cost).ok_or(EvalError::NumericOverflow)?;
        if *work > 1_000_000 { return Err(EvalError::ExecutionLimit { limit: "1 000 000 copies ou caractères de valeurs par calcul" }); }
    }
    Ok(display)
}

fn display_value(value: &Value, locale: Option<&crate::locale::LocaleManager>, resources: Option<&crate::string_table::StringTableResources>, work: &mut usize) -> EvalResult<String> {
    crate::value_limits::value_work(value)?;
    match value {
        Value::Text(text) => display_text(text, locale, resources, work),
        Value::Enum(_) => Err(EvalError::InvalidFunction("Enum has no implicit string or text conversion".into())),
        Value::WidgetClass(_) | Value::Widget(_) => Err(EvalError::InvalidFunction("Widget references cannot be converted to text".into())),
        Value::List(items) => {
            let mut result = String::from("[");
            for (index, value) in items.iter().enumerate() {
                if index > 0 { push_display(&mut result, ", ")?; }
                push_display(&mut result, &display_value(value, locale, resources, work)?)?;
            }
            push_display(&mut result, "]")?;
            Ok(result)
        },
        Value::Dict(items) => {
            let mut result = String::from("{");
            for (index, (key, value)) in items.iter().enumerate() {
                if index > 0 { push_display(&mut result, ", ")?; }
                push_display(&mut result, &format!("{key:?}: "))?;
                push_display(&mut result, &display_value(value, locale, resources, work)?)?;
            }
            push_display(&mut result, "}")?;
            Ok(result)
        },
        _ => Ok(value.to_string()),
    }
}

fn push_display(result: &mut String, text: &str) -> EvalResult<()> {
    if text.len() > crate::value_limits::MAX_VALUE_BYTES.saturating_sub(result.len()) {
        return Err(EvalError::ExecutionLimit { limit: "1 Mio de texte affiché" });
    }
    result.push_str(text);
    Ok(())
}

pub fn eval_interpolated(
    text: &InterpolatedText,
    vars: &HashMap<String, Value>,
) -> EvalResult<String> {
    FunctionLibrary::default().interpolate(text, vars)
}

// ─── Opérateurs binaires ──────────────────────────────────────────────────────

fn eval_binop(op: &BinOpKind, lv: Value, rv: Value) -> EvalResult<Value> {
    if let Some(result)=enum_values1025::binary(op,&lv,&rv){return result;}
    if matches!(op,BinOpKind::Add)&&(enum_values1025::contains(&lv)||enum_values1025::contains(&rv)){return Err(type_err(op,&lv,&rv));}
    fn has_widget_reference(value:&Value)->bool{
        match value {Value::Widget(_)|Value::WidgetClass(_)=>true,Value::List(values)=>values.iter().any(has_widget_reference),Value::Dict(values)=>values.values().any(has_widget_reference),_=>false}
    }
    if matches!(op,BinOpKind::Add){
        crate::value_limits::value_work(&lv)?;
        crate::value_limits::value_work(&rv)?;
        if has_widget_reference(&lv)||has_widget_reference(&rv){return Err(type_err(op,&lv,&rv));}
    }
    match op {
        BinOpKind::Add => match (&lv, &rv) {
            (Value::Int(a), Value::Int(b)) => a
                .checked_add(*b)
                .map(Value::Int)
                .ok_or(EvalError::NumericOverflow),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f32 + b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + *b as f32)),
            (Value::Text(_), _) | (_, Value::Text(_)) => Err(type_err(op, &lv, &rv)),
            (Value::Name(_), _) | (_, Value::Name(_)) => Err(type_err(op, &lv, &rv)),
            (Value::WidgetClass(_) | Value::Widget(_), _) | (_, Value::WidgetClass(_) | Value::Widget(_)) => Err(type_err(op, &lv, &rv)),
            (Value::Str(a), Value::Str(b)) => Ok(Value::Str(format!("{a}{b}"))),
            (Value::Str(a), _) => Ok(Value::Str(format!("{a}{rv}"))),
            (_, Value::Str(b)) => Ok(Value::Str(format!("{lv}{b}"))),
            _ => Err(type_err(op, &lv, &rv)),
        },
        BinOpKind::Sub => numeric_binop(op, &lv, &rv, i64::checked_sub, |a, b| a - b),
        BinOpKind::Mul => numeric_binop(op, &lv, &rv, i64::checked_mul, |a, b| a * b),
        BinOpKind::Div => match (&lv, &rv) {
            (_, Value::Int(0)) => Err(EvalError::DivisionByZero),
            (_, Value::Float(f)) if *f == 0.0 => Err(EvalError::DivisionByZero),
            _ => numeric_binop(op, &lv, &rv, i64::checked_div, |a, b| a / b),
        },
        BinOpKind::Eq => Ok(Value::Bool(values_eq(&lv, &rv))),
        BinOpKind::Ne => Ok(Value::Bool(!values_eq(&lv, &rv))),
        BinOpKind::Lt => cmp_values(op, &lv, &rv, |o| o.is_lt()),
        BinOpKind::Le => cmp_values(op, &lv, &rv, |o| o.is_le()),
        BinOpKind::Gt => cmp_values(op, &lv, &rv, |o| o.is_gt()),
        BinOpKind::Ge => cmp_values(op, &lv, &rv, |o| o.is_ge()),
    }
}

fn numeric_binop(
    op: &BinOpKind,
    lv: &Value,
    rv: &Value,
    ii: impl Fn(i64, i64) -> Option<i64>,
    ff: impl Fn(f32, f32) -> f32,
) -> EvalResult<Value> {
    match (lv, rv) {
        (Value::Int(a), Value::Int(b)) => {
            ii(*a, *b).map(Value::Int).ok_or(EvalError::NumericOverflow)
        }
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(ff(*a, *b))),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(ff(*a as f32, *b))),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(ff(*a, *b as f32))),
        _ => Err(type_err(op, lv, rv)),
    }
}

fn values_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Int(x), Value::Float(y)) => (*x as f32) == *y,
        (Value::Float(x), Value::Int(y)) => *x == (*y as f32),
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Name(x), Value::Name(y)) => x == y,
        (Value::Enum(x), Value::Enum(y)) => x == y,
        (Value::WidgetClass(x), Value::WidgetClass(y)) => x == y,
        (Value::Widget(x), Value::Widget(y)) => x == y,
        (Value::Text(x), Value::Text(y)) => x == y,
        (Value::List(a), Value::List(b)) => a == b,
        (Value::Dict(a), Value::Dict(b)) => a == b,
        _ => false,
    }
}

fn cmp_values(
    op: &BinOpKind,
    lv: &Value,
    rv: &Value,
    check: impl Fn(std::cmp::Ordering) -> bool,
) -> EvalResult<Value> {
    let ord = match (lv, rv) {
        (Value::Int(a), Value::Int(b)) => a.partial_cmp(b),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
        (Value::Int(a), Value::Float(b)) => (*a as f32).partial_cmp(b),
        (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f32)),
        (Value::Str(a), Value::Str(b)) => Some(a.as_str().cmp(b.as_str())),
        _ => return Err(type_err(op, lv, rv)),
    };
    Ok(Value::Bool(ord.map(check).unwrap_or(false)))
}

fn type_err(op: &BinOpKind, lv: &Value, rv: &Value) -> EvalError {
    EvalError::TypeMismatch {
        op: op.to_string(),
        left: type_name(lv).into(),
        right: type_name(rv).into(),
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Bool(_) => "bool",
        Value::Int(_) => "int",
        Value::Float(_) => "float",
        Value::Str(_) => "string",
        Value::Name(_) => "name",
        Value::Enum(_) => "enum",
        Value::WidgetClass(_) => "widget_class",
        Value::Widget(_) => "widget",
        Value::Text(_) => "text",
        Value::List(_) => "list",
        Value::Dict(_) => "dictionary",
    }
}

// ─── TESTS ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rvn_parser::{BinOpKind, Expr, InterpolatedText, TextSegment};

    fn vars(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn test_int_literal() {
        assert_eq!(eval_expr(&Expr::Int(42), &vars(&[])), Ok(Value::Int(42)));
    }
    #[test]
    fn test_bool_literal() {
        assert_eq!(
            eval_expr(&Expr::Bool(true), &vars(&[])),
            Ok(Value::Bool(true))
        );
    }
    #[test]
    fn test_str_literal() {
        assert_eq!(
            eval_expr(&Expr::Str("hi".into()), &vars(&[])),
            Ok(Value::Str("hi".into()))
        );
    }

    #[test]
    fn test_text_to_integer_conversion() {
        let expression = Expr::Call {
            name: "to_int".into(),
            args: vec![Expr::Str(" 15 ".into())],
        };
        assert_eq!(eval_expr(&expression, &vars(&[])), Ok(Value::Int(15)));
        let invalid = Expr::Call {
            name: "to_int".into(),
            args: vec![Expr::Str("pas un nombre".into())],
        };
        assert_eq!(eval_expr(&invalid, &vars(&[])), Ok(Value::Int(0)));
    }

    #[test]
    fn test_var_lookup() {
        let v = vars(&[("score", Value::Int(10))]);
        assert_eq!(
            eval_expr(&Expr::Var("score".into()), &v),
            Ok(Value::Int(10))
        );
    }

    #[test]
    fn test_var_undefined_error() {
        let err = eval_expr(&Expr::Var("x".into()), &vars(&[])).unwrap_err();
        assert!(matches!(&err, EvalError::UndefinedVar(n) if n == "x"));
        assert!(err.to_string().contains("aide"));
    }

    #[test]
    fn test_add_int() {
        let e = Expr::BinOp {
            op: BinOpKind::Add,
            left: Box::new(Expr::Int(3)),
            right: Box::new(Expr::Int(4)),
        };
        assert_eq!(eval_expr(&e, &vars(&[])), Ok(Value::Int(7)));
    }

    #[test]
    fn test_division_by_zero() {
        let e = Expr::BinOp {
            op: BinOpKind::Div,
            left: Box::new(Expr::Int(1)),
            right: Box::new(Expr::Int(0)),
        };
        assert_eq!(eval_expr(&e, &vars(&[])), Err(EvalError::DivisionByZero));
    }

    #[test]
    fn test_and_short_circuit() {
        let e = Expr::And(Box::new(Expr::Bool(false)), Box::new(Expr::Var("x".into())));
        assert_eq!(eval_expr(&e, &vars(&[])), Ok(Value::Bool(false)));
    }

    #[test]
    fn test_interpolation_simple() {
        let text = InterpolatedText(vec![
            TextSegment::Lit("Bonjour ".into()),
            TextSegment::Interp(Expr::Var("prenom".into())),
            TextSegment::Lit(" !".into()),
        ]);
        let v = vars(&[("prenom", Value::Str("Sarah".into()))]);
        assert_eq!(eval_interpolated(&text, &v), Ok("Bonjour Sarah !".into()));
    }

    #[test]
    fn test_interpolation_undefined_var_error() {
        let text = InterpolatedText(vec![TextSegment::Interp(Expr::Var("inconnu".into()))]);
        let err = eval_interpolated(&text, &vars(&[])).unwrap_err();
        assert!(matches!(err, EvalError::UndefinedVar(_)));
    }
}

// Independent Rust implementation. No Epic implementation copied.
// Legacy String intrinsics remain unchanged; these versioned boundaries are strict.
fn eval_name_string_table_query(
    name: &str,
    args: &[Value],
    resources: Option<&crate::string_table::StringTableResources>,
    value_work: &mut usize,
) -> EvalResult<Value> {
    use rvn_parser::Name;
    let invalid = |message: &str| EvalError::InvalidFunction(format!("{name}: {message}"));
    let expected = match name {
        "__rvn_registered_string_tables_name" => 0,
        "__rvn_string_table_id_name" | "__rvn_string_table_registered_name"
        | "__rvn_string_table_namespace_name" | "__rvn_string_table_keys_name" => 1,
        "__rvn_string_table_entry_registered_name" | "__rvn_string_table_source_name"
        | "__rvn_string_table_metadata_ids_name" => 2,
        "__rvn_string_table_metadata_name" => 3,
        _ => return Err(invalid("unknown typed string-table query")),
    };
    if args.len() != expected { return Err(invalid("wrong argument count")); }
    if name == "__rvn_string_table_id_name" {
        let id = crate::string_table::asset_reference_id(&args[0])
            .map_err(EvalError::InvalidFunction)?;
        let result = Name::new(id).map_err(|error| invalid(&error.to_string()))?;
        // Null object -> None. Retained non-null identity survives asset unload.
        if !id.is_empty() && result.is_none() { return Err(invalid("legacy table ID resolves to None")); }
        return Ok(Value::Name(result));
    }
    let query_id = if expected > 0 {
        let Value::Name(id) = &args[0] else { return Err(invalid("TableId requires Name; convert String explicitly")); };
        Some(id)
    } else { None };
    let key = if expected > 1 {
        let Value::Str(key) = &args[1] else { return Err(invalid("entry Key requires String")); };
        Some(key.as_str())
    } else { None };
    let metadata_id = if expected > 2 {
        let Value::Name(id) = &args[2] else { return Err(invalid("MetaDataId requires Name; convert String explicitly")); };
        Some(id)
    } else { None };

    // Compatibility boundary: old authored IDs are not rewritten or discarded.
    // Validate the published snapshot before projecting it into Name identity.
    // A future indexed implementation can cache this against revision_stamp().
    let mut identities = std::collections::HashSet::new();
    let mut table = None;
    for candidate in resources.into_iter().flat_map(|r| r.registered_tables()) {
        // Snapshot scans construct temporary Name identity strings. Charge
        // their copy work to the caller, including unsuccessful queries.
        let work = candidate.id.len().checked_mul(2).ok_or(EvalError::NumericOverflow)? / 32 + 1;
        *value_work = value_work.checked_add(work).ok_or(EvalError::NumericOverflow)?;
        if *value_work > 1_000_000 { return Err(EvalError::ExecutionLimit { limit: "1 000 000 copies de valeurs" }); }
        let id = Name::new(&candidate.id).map_err(|error| invalid(&format!("legacy table ID {}: {error}", candidate.id)))?;
        if id.is_none() { return Err(invalid(&format!("legacy table ID {} resolves to None", candidate.id))); }
        let matches = query_id == Some(&id);
        if !identities.insert(id) {
            return Err(invalid(&format!("ambiguous Name table identity {}", candidate.id)));
        }
        if matches { table = Some(candidate); }
    }
    let entry = table.and_then(|table| key.and_then(|key| table.entries.get(key)));

    if matches!(name, "__rvn_registered_string_tables_name" | "__rvn_string_table_metadata_ids_name") {
        // Borrow IDs first; bound both stored spelling and ASCII identity
        // before allocating the typed result. Never allocate a String list.
        let mut raw_ids = Vec::new();
        let mut bytes = 0usize;
        fn admit<'a>(raw: &'a str, raw_ids: &mut Vec<&'a str>, bytes: &mut usize) -> EvalResult<()> {
            if raw_ids.len() >= crate::value_limits::MAX_COLLECTION_ITEMS {
                return Err(EvalError::ExecutionLimit { limit: "4 096 éléments par collection" });
            }
            // Empty-base suffix aliases canonicalize to the four-byte None.
            let runtime_len = if crate::string_table::is_absent_metadata_name(raw) { 4 } else { raw.len() };
            *bytes = bytes.checked_add(runtime_len.checked_mul(2).ok_or(EvalError::NumericOverflow)?)
                .ok_or(EvalError::NumericOverflow)?;
            if *bytes > crate::value_limits::MAX_VALUE_BYTES {
                return Err(EvalError::ExecutionLimit { limit: "1 Mio de texte par valeur" });
            }
            raw_ids.push(raw);
            Ok(())
        }
        if name == "__rvn_registered_string_tables_name" {
            for table in resources.into_iter().flat_map(|r| r.registered_tables()) { admit(&table.id, &mut raw_ids, &mut bytes)?; }
        } else {
            for id in entry.into_iter().flat_map(|entry| entry.metadata.keys()) { admit(id, &mut raw_ids, &mut bytes)?; }
        }
        let mut names = Vec::with_capacity(raw_ids.len());
        for raw in raw_ids {
            let id = Name::new(raw).map_err(|error| invalid(&error.to_string()))?;
            names.push(Value::Name(id));
        }
        let mut identities = std::collections::HashSet::new();
        for item in &names {
            let Value::Name(id) = item else { unreachable!() };
            if !identities.insert(id) { return Err(invalid("ambiguous Name identifier collection")); }
        }
        return Ok(Value::List(names));
    }
    // Delegate bounded result creation to the existing String query implementation.
    // Feed the exact retained UUID/display identity, never a folded path.
    let mut legacy_args = Vec::with_capacity(expected);
    if expected > 0 { legacy_args.push(Value::Str(table.map_or("", |t| t.id.as_str()).into())); }
    if let Some(key) = key { legacy_args.push(Value::Str(key.into())); }
    if let Some(id) = metadata_id { legacy_args.push(Value::Str(id.as_str().into())); }
    let legacy_name = name.strip_suffix("_name").unwrap();
    let result = eval_string_table_query(legacy_name, &legacy_args, resources)?;
    match name {
        // A missing/None table cannot resolve the legacy empty key. Evaluate
        // missing outputs directly if such a table were permitted historically.
        "__rvn_string_table_registered_name" => Ok(Value::Bool(table.is_some())),
        "__rvn_string_table_entry_registered_name" => Ok(Value::Bool(entry.is_some())),
        _ => Ok(result),
    }
}

pub(crate) fn validate_enum_program_literals1027(script:&[rvn_parser::Statement],resources:&crate::enum_runtime_resources1025::EnumAssets1025)->EvalResult<()>{enum_values1025::validate_program_literals1027(script,resources)}
