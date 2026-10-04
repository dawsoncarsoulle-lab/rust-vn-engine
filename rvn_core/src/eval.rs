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
    definitions: HashMap<String, (Vec<String>, Vec<Statement>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiCommand {
    Menu { request: rvn_ui::source_menus::MenuRequest, source: Option<String> },
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
    Focus {
        name: String,
        element: String,
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
        let mut computation = Computation::new(library, globals);
        computation.random = *random;
        let command = computation.ui_command(statement, globals)?;
        *random = computation.random;
        Ok(command)
    }
}

impl FunctionLibrary {
    pub fn has_function(&self, name: &str, parameters: usize) -> bool {
        self.definitions
            .get(name)
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
        if !self.has_function(name, 3) {
            return Err(EvalError::InvalidFunction(format!(
                "Canvas draw '{name}' must reference an RVN function with three parameters"
            )));
        }
        let mut computation = Computation::new(self, globals);
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
        let mut library = Self::default();
        for statement in script {
            if let Statement::Function {
                name,
                parameters,
                body,
            } = statement
            {
                if !rvn_parser::is_binding_name(name)
                    || library.definitions.contains_key(name)
                    || is_builtin(name)
                {
                    return Err(EvalError::InvalidFunction(format!(
                        "nom déjà défini : {name}"
                    )));
                }
                if parameters.len() > 128
                    || parameters
                        .iter()
                        .any(|name| !rvn_parser::is_binding_name(name))
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
                validate_computation(body, true)?;
                library
                    .definitions
                    .insert(name.clone(), (parameters.clone(), body.clone()));
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
        let mut computation = Computation::new(self, globals);
        computation.random = *random;
        let value = computation.eval(expression, globals, 0)?;
        *random = computation.random;
        Ok(value)
    }

    pub fn eval_bool(
        &self,
        expression: &Expr,
        globals: &HashMap<String, Value>,
    ) -> EvalResult<bool> {
        Ok(truthy(&self.eval(expression, globals)?))
    }

    pub fn eval_bool_with_random(
        &self,
        expression: &Expr,
        globals: &HashMap<String, Value>,
        random: &mut RandomState,
    ) -> EvalResult<bool> {
        Ok(truthy(&self.eval_with_random(expression, globals, random)?))
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
        let mut computation = Computation::new(self, globals);
        computation.random = *random;
        let mut result = String::new();
        for segment in &text.0 {
            match segment {
                TextSegment::Lit(text) => result.push_str(text),
                TextSegment::Interp(expression) => {
                    result.push_str(&computation.eval(expression, globals, 0)?.to_string())
                }
            }
        }
        *random = computation.random;
        Ok(result)
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
        validate_computation(block, false)?;
        let mut values = globals.clone();
        let mut computation = Computation::new(self, globals);
        computation.random = *random;
        computation.block(block, &mut values)?;
        *random = computation.random;
        Ok(values)
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
        validate_computation(body, true)?;
        if parameters.len() != arguments.len() {
            return Err(EvalError::InvalidFunction(
                "nombre d’arguments d’écran incorrect".into(),
            ));
        }
        let mut locals = globals.clone();
        locals.extend(parameters.iter().cloned().zip(arguments.iter().cloned()));
        let mut computation = Computation::new(self, globals);
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
        validate_handler(body)?;
        if parameters.len() != arguments.len() {
            return Err(EvalError::InvalidFunction(
                "nombre d’arguments du gestionnaire incorrect".into(),
            ));
        }
        let mut locals = globals.clone();
        locals.extend(parameters.iter().cloned().zip(arguments.iter().cloned()));
        let mut computation = Computation::new(self, globals);
        computation.steps = budget.steps;
        computation.value_work = budget.value_work;
        computation.handler_locals = Some(parameters.iter().cloned().collect());
        computation.random = *random;
        let result = computation.block(body, &mut locals);
        budget.steps = computation.steps;
        budget.value_work = computation.value_work;
        result?;
        *random = computation.random;
        Ok((computation.globals, computation.ui_commands))
    }
}

fn validate_computation(block: &[Statement], returns: bool) -> EvalResult<()> {
    for statement in block {
        match statement {
            Statement::SetVar { name, .. } | Statement::LocalVar { name, .. } if rvn_parser::is_binding_name(name) => {}
            Statement::FunctionReturn { .. } if returns => {}
            Statement::If { then_branch, else_branch, .. } => {
                validate_computation(then_branch, returns)?;
                validate_computation(else_branch, returns)?;
            }
            Statement::While { body, .. } => validate_computation(body, returns)?,
            Statement::ForEach { name,body,.. } if rvn_parser::is_binding_name(name) => validate_computation(body,returns)?,
            _ => return Err(EvalError::InvalidFunction("une expression ne peut pas modifier l’état persistant ou effectuer une opération narrative".into())),
        }
    }
    Ok(())
}

pub fn validate_screen(block: &[Statement]) -> EvalResult<()> {
    validate_computation(block, true)
}

pub fn validate_handler(block: &[Statement]) -> EvalResult<()> {
    for statement in block {
        match statement {
            Statement::UiOpen { .. }
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
                validate_handler(then_branch)?;
                validate_handler(else_branch)?;
            }
            Statement::While { body, .. } => validate_handler(body)?,
            Statement::ForEach { name, body, .. } if rvn_parser::is_binding_name(name) => {
                validate_handler(body)?
            }
            other => validate_computation(std::slice::from_ref(other), true)?,
        }
    }
    Ok(())
}

pub fn is_builtin(name: &str) -> bool {
    rvn_parser::builtin_arity(name).is_some()
}

struct Computation<'a> {
    library: &'a FunctionLibrary,
    globals: HashMap<String, Value>,
    steps: usize,
    value_work: usize,
    calls: usize,
    random: RandomState,
    handler_locals: Option<std::collections::HashSet<String>>,
    ui_commands: Vec<UiCommand>,
    pure_canvas: bool,
}

impl<'a> Computation<'a> {
    fn new(library: &'a FunctionLibrary, globals: &HashMap<String, Value>) -> Self {
        Self {
            library,
            globals: globals.clone(),
            steps: 0,
            value_work: 0,
            calls: 0,
            random: RandomState::default(),
            handler_locals: None,
            ui_commands: Vec::new(),
            pure_canvas: false,
        }
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

    fn block(
        &mut self,
        block: &[Statement],
        variables: &mut HashMap<String, Value>,
    ) -> EvalResult<Option<Value>> {
        for statement in block {
            self.tick()?;
            match statement {
                Statement::SetVar { name, value } => {
                    let value = self.eval(value, variables, 0)?;
                    if self.calls == 0
                        && !self
                            .handler_locals
                            .as_ref()
                            .is_some_and(|locals| locals.contains(name))
                    {
                        self.globals.insert(name.clone(), value.clone());
                    }
                    variables.insert(name.clone(), value);
                }
                Statement::LocalVar { name, value } => {
                    let value = self.eval(value, variables, 0)?;
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
                Statement::UiOpen { .. }
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
                Statement::FunctionReturn { value } => {
                    return Ok(Some(self.eval(value, variables, 0)?))
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let branch = if truthy(&self.eval(condition, variables, 0)?) {
                        then_branch
                    } else {
                        else_branch
                    };
                    if let Some(result) = self.block(branch, variables)? {
                        return Ok(Some(result));
                    }
                }
                Statement::While { condition, body } => {
                    while truthy(&self.eval(condition, variables, 0)?) {
                        self.tick()?;
                        if let Some(result) = self.block(body, variables)? {
                            return Ok(Some(result));
                        }
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
                    if self.calls == 0 {
                        if let Some(locals) = &mut self.handler_locals {
                            locals.insert(name.clone());
                        }
                    }
                    for item in items {
                        self.tick()?;
                        variables.insert(name.clone(), item);
                        if self.calls == 0 && self.handler_locals.is_none() {
                            self.globals.insert(name.clone(), variables[name].clone());
                        }
                        if let Some(result) = self.block(body, variables)? {
                            return Ok(Some(result));
                        }
                    }
                }
                _ => {
                    return Err(EvalError::InvalidFunction(
                        "instruction non calculable".into(),
                    ))
                }
            }
        }
        Ok(None)
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
                        .map_err(|error|EvalError::InvalidMenuRequest(error.to_string()))?)
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
            Statement::UiClose { name } => UiCommand::Close {
                name: string(self.eval(name, vars, 0)?)?,
            },
            Statement::UiFocus { name, element } => UiCommand::Focus {
                name: string(self.eval(name, vars, 0)?)?,
                element: string(self.eval(element, vars, 0)?)?,
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
                || !self
                    .library
                    .definitions
                    .get(function)
                    .is_some_and(|(parameters, _)| parameters.len() == 1)
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
        let Some((parameters, body)) = self.library.definitions.get(name) else {
            return eval_call(name, arguments, &self.globals, &mut self.random);
        };
        if arguments.len() != parameters.len() {
            return Err(EvalError::InvalidFunction(format!(
                "{name} attend {} argument(s), reçu {}",
                parameters.len(),
                arguments.len()
            )));
        }
        if self.calls >= MAX_FUNCTION_DEPTH {
            return Err(EvalError::ExecutionLimit {
                limit: "64 appels de fonction imbriqués",
            });
        }
        for value in self.globals.values() {
            self.value_work = self
                .value_work
                .checked_add(crate::value_limits::value_work(value)?)
                .ok_or(EvalError::NumericOverflow)?;
            if self.value_work > 1_000_000 {
                return Err(EvalError::ExecutionLimit {
                    limit: "1 000 000 copies ou caractères de valeurs par calcul",
                });
            }
        }
        // Lexical scope: never inherit another function's parameters or locals.
        let mut locals = self.globals.clone();
        locals.extend(parameters.iter().cloned().zip(arguments));
        self.calls += 1;
        let result = self.block(body, &mut locals);
        self.calls -= 1;
        result?
            .ok_or_else(|| EvalError::InvalidFunction(format!("{name} n’a retourné aucune valeur")))
    }

    fn eval(
        &mut self,
        expr: &Expr,
        vars: &HashMap<String, Value>,
        depth: usize,
    ) -> EvalResult<Value> {
        self.tick()?;
        if depth > 256 {
            return Err(EvalError::ExecutionLimit {
                limit: "256 niveaux d’expression",
            });
        }
        let depth = depth + 1;
        let result = match expr {
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

            Expr::Not(inner) => Ok(Value::Bool(!truthy(&self.eval(inner, vars, depth)?))),

            Expr::And(l, r) => {
                if !truthy(&self.eval(l, vars, depth)?) {
                    return Ok(Value::Bool(false));
                }
                Ok(Value::Bool(truthy(&self.eval(r, vars, depth)?)))
            }
            Expr::Or(l, r) => {
                if truthy(&self.eval(l, vars, depth)?) {
                    return Ok(Value::Bool(true));
                }
                Ok(Value::Bool(truthy(&self.eval(r, vars, depth)?)))
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

fn is_menu_request_constructor(name:&str)->bool {
    matches!(name,"menu_action"|"menu_start_scene"|"menu_open_page"|"menu_save_page"|"menu_language"
        |"menu_choose"|"menu_gallery_cg"|"menu_gallery_tab"|"menu_confirm"|"menu_cancel"
        |"menu_slot"|"menu_protect"|"menu_number"|"menu_bool"|"menu_advance"|"menu_skip_typewriter")
}
fn eval_call(
    name: &str,
    evaluated: Vec<Value>,
    vars: &HashMap<String, Value>,
    random: &mut RandomState,
) -> EvalResult<Value> {
    if let Some(arity) = rvn_parser::builtin_arity(name) {
        if !arity.contains(&evaluated.len()) {
            let message=format!("{name} : nombre d’arguments invalide");
            return Err(if is_menu_request_constructor(name){EvalError::InvalidMenuRequest(message)}else{EvalError::InvalidFunction(message)});
        }
    }
    match name {
        "menu_action" | "menu_start_scene" | "menu_open_page" | "menu_save_page" | "menu_language"
        | "menu_choose" | "menu_gallery_cg" | "menu_gallery_tab" | "menu_confirm" | "menu_cancel"
        | "menu_slot" | "menu_protect" | "menu_number" | "menu_bool" | "menu_advance" | "menu_skip_typewriter" => {
            let args = evaluated.iter().map(crate::ui::value_to_json).collect::<EvalResult<Vec<_>>>()
                .map_err(|error|EvalError::InvalidMenuRequest(error.to_string()))?;
            let value = rvn_ui::source_menus::construct(name, &args).map_err(EvalError::InvalidMenuRequest)?;
            crate::ui::value_from_json(&value).map_err(|error|EvalError::InvalidMenuRequest(error.to_string()))
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
        // RVN text templates currently share the string value representation.
        // These explicit graph casts preserve the payload verbatim; they must
        // never stringify numbers or silently re-interpret interpolation.
        "string_to_text" | "text_to_string" => match evaluated.as_slice() {
            [Value::Str(value)] => Ok(Value::Str(value.clone())),
            _ => Err(EvalError::TypeMismatch {
                op: name.into(),
                left: "string".into(),
                right: "non-string value".into(),
            }),
        },
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
    Ok(truthy(&eval_expr(expr, vars)?))
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,
        Value::Int(n) => *n != 0,
        Value::Float(f) => *f != 0.0,
        Value::Str(s) => !s.is_empty(),
        Value::List(items) => !items.is_empty(),
        Value::Dict(items) => !items.is_empty(),
    }
}

pub fn eval_interpolated(
    text: &InterpolatedText,
    vars: &HashMap<String, Value>,
) -> EvalResult<String> {
    FunctionLibrary::default().interpolate(text, vars)
}

// ─── Opérateurs binaires ──────────────────────────────────────────────────────

fn eval_binop(op: &BinOpKind, lv: Value, rv: Value) -> EvalResult<Value> {
    match op {
        BinOpKind::Add => match (&lv, &rv) {
            (Value::Int(a), Value::Int(b)) => a
                .checked_add(*b)
                .map(Value::Int)
                .ok_or(EvalError::NumericOverflow),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f32 + b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + *b as f32)),
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
