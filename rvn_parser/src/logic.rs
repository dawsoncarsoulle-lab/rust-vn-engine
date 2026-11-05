use crate::{Expr, InterpolatedText, Statement, TextSegment};
use logos::Logos;
use std::collections::BTreeMap;

pub fn is_binding_name(name: &str) -> bool {
    let mut lexer = crate::lexer::Token::lexer(name);
    !name.starts_with("__rvn_")
        && matches!(lexer.next(),Some(Ok(crate::lexer::Token::Ident(found))) if found==name)
        && lexer.next().is_none()
}


/// A semantic variable key, independent of its spelling in RVN source.
/// Callable names keep their existing stricter identifier contract.
pub fn is_variable_binding_name(name: &str) -> bool {
    variable_key_shape(name) && !name.starts_with("__rvn_")
}

fn variable_key_shape(name: &str) -> bool {
    !name.is_empty() && !name.chars().any(|c| c.is_control() || c == '.')
}

fn variable_key_source(name: &str, reserve_internal: bool) -> Option<String> {
    if !variable_key_shape(name) || reserve_internal && name.starts_with("__rvn_") { return None; }
    let mut lexer = crate::lexer::Token::lexer(name);
    let plain = matches!(lexer.next(),Some(Ok(crate::lexer::Token::Ident(found))) if found==name)
        && lexer.next().is_none();
    Some(if plain { name.to_owned() } else {
        format!("`{}`", name.replace('\\', "\\\\").replace('`', "\\`"))
    })
}

/// Parameters and local bindings have no persistent namespace.
pub fn variable_binding_source_name(name: &str) -> Option<String> {
    variable_key_source(name, true)
}

/// Preserve the existing persistent namespace and every semantic key byte.
/// Legacy ASCII spellings (including persistent suffixes) stay unchanged.
pub fn variable_source_name(name: &str) -> Option<String> {
    if let Some(suffix) = name.strip_prefix("persistent.") {
        Some(format!("persistent.{}", variable_key_source(suffix, false)?))
    } else { variable_binding_source_name(name) }
}

/// Single catalogue used by the runtime, CLI and Blueprint compiler.
pub fn builtin_arity(name: &str) -> Option<std::ops::RangeInclusive<usize>> {
    Some(match name {
        "__rvn_widget_create_v1" => 2..=2,
        "__rvn_enum_value_v1" => 2..=2,
        "__rvn_widget_class_v1" => 1..=1,
        "__rvn_widget_object_class_v1" => 1..=1,
        "__rvn_widget_class_none_v1" => 0..=0,
        "menu_action" | "menu_start_scene" | "menu_open_page" | "menu_save_page" | "menu_language"
        | "menu_choose" | "menu_gallery_cg" | "menu_gallery_tab" | "menu_confirm" | "menu_cancel" => 1..=1,
        "menu_slot" | "menu_protect" | "menu_number" | "menu_bool" => 2..=2,
        "menu_advance" | "menu_skip_typewriter" => 0..=0,
        "min" | "max" => 0..=128,
        "dict" => 0..=128,
        "__rvn_localizable_text" => 4..=5,
        "__rvn_empty_text" => 0..=0,
        "__rvn_invariant_text" => 1..=1,
        "__rvn_format_text" => 2..=2,
        "__rvn_string_table_text" | "__rvn_string_table_text_name" => 2..=2,
        "__rvn_text_table_valid" | "__rvn_text_table_id" | "__rvn_text_table_key" => 1..=1,
        "__rvn_captured_v1" => 1..=1,
        "__rvn_terminal_v1" => 2..=2,
        "__rvn_string_table_asset" | "__rvn_string_table_id" | "__rvn_literal_name" | "__rvn_string_to_name" | "__rvn_name_to_string" => 1..=1,
        "__rvn_string_table_id_name" | "__rvn_string_table_registered_name"
        | "__rvn_string_table_namespace_name" | "__rvn_string_table_keys_name" => 1..=1,
        "__rvn_registered_string_tables_name" => 0..=0,
        "__rvn_string_table_entry_registered_name" | "__rvn_string_table_source_name"
        | "__rvn_string_table_metadata_ids_name" => 2..=2,
        "__rvn_string_table_metadata_name" => 3..=3,
        "__rvn_registered_string_tables" => 0..=0,
        "__rvn_string_table_registered" | "__rvn_string_table_namespace" | "__rvn_string_table_keys" => 1..=1,
        "__rvn_string_table_entry_registered" | "__rvn_string_table_source" | "__rvn_string_table_metadata_ids" => 2..=2,
        "__rvn_string_table_metadata" => 3..=3,
        "dict_keys" | "dict_values" => 1..=1,
        "dict_at" | "dict_remove" => 2..=2,
        "dict_set" | "dict_get" => 3..=3,
        "make_color" | "make_color_rgb" => 4..=4,
        "component" => 4..=4,
        "canvas_rect" | "canvas_line" | "canvas_group" => 3..=3,
        "canvas_ellipse" | "canvas_polygon" | "canvas_image" | "canvas_hit" => 2..=2,
        "canvas_text" => 4..=4,
        "layered_image" => 3..=4,
        "image_layer" => 3..=3,
        "image_layers" => 2..=2,
        "video_clip" => 2..=2,
        "motion_tween" => 4..=4,
        "motion_spline" => 3..=3,
        "motion_bezier" => 4..=4,
        "motion_curve" => 2..=2,
        "motion_sequence" | "motion_parallel" | "motion_pause" => 1..=1,
        "motion_repeat" | "motion_frames" => 2..=2,
        "abs" | "floor" | "ceil" | "to_int" | "__rvn_bool_to_int" | "__rvn_int_to_bool" | "string_to_text" | "text_to_string" | "len"
        | "upper" | "lower" | "capitalize" | "trim" | "key_pressed" | "__rvn_iterable" => 1..=1,
        "random" | "rand" | "contains" | "split" | "list_append" | "list_remove"
        | "list_concat" => 2..=2,
        "replace" | "substring" | "list_insert" | "list_set" | "list_slice" => 3..=3,
        "mouse_clicked" | "mouse_x" | "mouse_y" => 0..=0,
        _ => return None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicDiagnostic {
    pub name: String,
    pub code: &'static str,
    pub english: String,
    pub french: String,
}

/// Validate names and arities without running story code or loop bodies.
/// External imports defer missing names only; local mistakes remain errors.
pub fn validate_logic(script: &[Statement], allow_external: bool) -> Vec<LogicDiagnostic> {
    let mut functions = BTreeMap::new();
    let mut declaration_names = BTreeMap::new();
    let units:BTreeMap<_,_>=script.iter().filter_map(|s|if let Statement::UnitFunction{name,parameters,..}=s{Some((name.as_str(),parameters.len()))}else{None}).collect();
    let screens: BTreeMap<_, _> = script
        .iter()
        .filter_map(|statement| {
            if let Statement::Screen {
                name, parameters, ..
            } = statement
            {
                Some((name.as_str(), parameters.len()))
            } else {
                None
            }
        })
        .collect();
    let mut errors = Vec::new();
    for statement in script {
        if let Statement::Function {
            name, parameters, ..
        }
        | Statement::UnitFunction { name, parameters, .. }
        | Statement::Screen {
            name, parameters, ..
        }
        | Statement::Handler {
            name, parameters, ..
        } = statement
        {
            if matches!(statement, Statement::Handler { .. }) && parameters.len() != 1 {
                errors.push(LogicDiagnostic {
                    name: name.clone(),
                    code: "handler-arity",
                    english: format!("Handler '{name}' must accept one event parameter"),
                    french: format!("Le gestionnaire {name} doit accepter un paramètre événement"),
                });
            }
            if matches!(statement, Statement::Function { .. }) {
                functions.insert(name.as_str(), parameters.len());
            }
            if declaration_names
                .insert(name.as_str(), parameters.len())
                .is_some()
                || builtin_arity(name).is_some()
            {
                errors.push(LogicDiagnostic {
                    name: name.clone(),
                    code: "duplicate-function",
                    english: format!("Function '{name}' is already defined"),
                    french: format!("Fonction déjà définie : {name}"),
                });
            }
        }
    }
    fn expression(
        expr: &Expr,
        functions: &BTreeMap<&str, usize>,
        declared: &BTreeMap<&str, usize>,
        external: bool,
        errors: &mut Vec<LogicDiagnostic>,
    ) {
        match expr {
            Expr::StandardMacroData{site,output,pin}=>{
                if site.data_pin(*output).as_ref()!=Ok(pin){errors.push(LogicDiagnostic{name:"StandardMacro".into(),code:"macro-source-pin",english:"Macro data output differs from its typed source declaration".into(),french:"La sortie macro diffère de sa déclaration source typée".into()});}
            },
            Expr::LibraryCall{call}=>for(_,argument)in call.arguments(){expression(argument,functions,declared,external,errors);},
            Expr::Call { name, args } => {
                let expected = functions
                    .get(name.as_str())
                    .map(|count| *count..=*count)
                    .or_else(|| builtin_arity(name));
                if let Some(expected) = expected {
                    if !expected.contains(&args.len()) {
                        errors.push(LogicDiagnostic {
                            name: name.clone(),
                            code: "function-arity",
                            english: format!(
                                "Function '{name}' expects {expected:?} arguments, received {}",
                                args.len()
                            ),
                            french: format!(
                                "{name} attend {expected:?} argument(s), reçu {}",
                                args.len()
                            ),
                        });
                    }
                } else if declared.contains_key(name.as_str()) {
                    // Imports defer only genuinely unresolved symbols. A local
                    // Unit/Screen/Handler declaration is never a value function.
                    errors.push(LogicDiagnostic {
                        name:name.clone(),code:"function-call-kind",
                        english:format!("Known callable '{name}' does not return a function expression value"),
                        french:format!("Le callable connu {name} ne fournit pas une valeur de fonction"),
                    });
                } else if !external {
                    errors.push(LogicDiagnostic {
                        name: name.clone(),
                        code: "unknown-function",
                        english: format!("Unknown RVN function '{name}'"),
                        french: format!("Fonction RVN inconnue : {name}"),
                    });
                }
                for argument in args {
                    expression(argument, functions, declared, external, errors);
                }
            }
            Expr::BinOp { left, right, .. } | Expr::And(left, right) | Expr::Or(left, right) => {
                expression(left, functions, declared, external, errors);
                expression(right, functions, declared, external, errors);
            }
            Expr::Index { target, index } => {
                expression(target, functions, declared, external, errors);
                expression(index, functions, declared, external, errors);
            }
            Expr::Neg(value) | Expr::Not(value) => expression(value, functions, declared, external, errors),
            Expr::ListLit(items) => {
                for item in items {
                    expression(item, functions, declared, external, errors);
                }
            }
            _ => {}
        }
    }
    fn text(
        text: &InterpolatedText,
        functions: &BTreeMap<&str, usize>,
        declared: &BTreeMap<&str, usize>,
        external: bool,
        errors: &mut Vec<LogicDiagnostic>,
    ) {
        for segment in &text.0 {
            if let TextSegment::Interp(value) = segment {
                expression(value, functions, declared, external, errors);
            }
        }
    }
    fn walk(
        block: &[Statement],
        functions: &BTreeMap<&str, usize>,
        units: &BTreeMap<&str, usize>,
        declared: &BTreeMap<&str, usize>,
        external: bool,
        errors: &mut Vec<LogicDiagnostic>,
    ) {
        for statement in block {
            match statement {
                Statement::FunctionLibrary{scope}=>for member in scope.members(){walk(std::slice::from_ref(member.declaration()),functions,units,declared,external,errors);},
                Statement::LibraryUnitCall{call}=>for(_,argument)in call.arguments(){expression(argument,functions,declared,external,errors);},
                Statement::Init { body }
                | Statement::Function { body, .. }
                | Statement::UnitFunction { body, .. }
                | Statement::Screen { body, .. }
                | Statement::Handler { body, .. } => walk(body, functions, units, declared, external, errors),
                Statement::StandardMacroDispatch{dispatch}=>{
                    for value in dispatch.operand_expressions(){expression(value,functions,declared,external,errors);}
                    for branch in &dispatch.branches{walk(&branch.body,functions,units,declared,external,errors);}
                },
                Statement::ScheduledConsumer { steps, body, .. } => {
                    for step in steps { match step {
                        crate::ConsumerStep::Capture { value, .. } | crate::ConsumerStep::InspectTextTable { value, .. } => expression(value, functions, declared, external, errors),
                    }}
                    walk(body, functions, units, declared, external, errors);
                }
                Statement::UnitCall{name,arguments}=>{
                    match units.get(name.as_str()) {
                        Some(expected) if *expected!=arguments.len()=>errors.push(LogicDiagnostic{name:name.clone(),code:"unit-call-arity",english:format!("Unit function '{name}' expects {expected} arguments"),french:format!("La fonction sans résultat {name} attend {expected} arguments")}),
                        None if !external || declared.contains_key(name.as_str()) || builtin_arity(name).is_some()=>errors.push(LogicDiagnostic{name:name.clone(),code:"unit-call-target",english:format!("'{name}' is not a known unit function"),french:format!("{name} n’est pas une fonction sans résultat connue")}),
                        _=>{}
                    }
                    for argument in arguments {expression(argument,functions,declared,external,errors);}
                },
                Statement::SetVar { value, .. }
                | Statement::LocalVar { value, .. }
                | Statement::FunctionReturn { value } => {
                    expression(value, functions, declared, external, errors)
                }
                Statement::UiOpen {
                    name,
                    arguments,
                    modal,
                    layer,
                    ..
                } => {
                    for value in [name, arguments, modal, layer] {
                        expression(value, functions, declared, external, errors);
                    }
                }
                Statement::WidgetAddToViewport { object, z_order } => { expression(object, functions, declared, external, errors);expression(z_order, functions, declared, external, errors); }
                Statement::WidgetRemoveFromParent { object } => expression(object, functions, declared, external, errors),
                Statement::UiClose { name } => expression(name, functions, declared, external, errors),
                Statement::MenuExecute { request } => expression(request, functions, declared, external, errors),
                Statement::UiFocus { name, element } => {
                    expression(name, functions, declared, external, errors);
                    expression(element, functions, declared, external, errors);
                }
                Statement::UiSetState {
                    name,
                    element,
                    state,
                } => {
                    for value in [name, element, state] {
                        expression(value, functions, declared, external, errors);
                    }
                }
                Statement::MotionPlay { target, definition } => {
                    expression(target, functions, declared, external, errors);
                    expression(definition, functions, declared, external, errors);
                }
                Statement::CharacterCompose {
                    character,
                    definition,
                } => {
                    expression(character, functions, declared, external, errors);
                    expression(definition, functions, declared, external, errors);
                }
                Statement::CharacterAttributes {
                    character,
                    attributes,
                } => {
                    expression(character, functions, declared, external, errors);
                    expression(attributes, functions, declared, external, errors);
                }
                Statement::VideoPlay { name, definition } => {
                    expression(name, functions, declared, external, errors);
                    expression(definition, functions, declared, external, errors);
                }
                Statement::VideoSeek { name, seconds } => {
                    expression(name, functions, declared, external, errors);
                    expression(seconds, functions, declared, external, errors);
                }
                Statement::MusicVolumeValue { level } => expression(level, functions, declared, external, errors),
                Statement::TypewriterSetValue { enabled } => expression(enabled, functions, declared, external, errors),
                Statement::TypewriterSpeedValue { chars_per_sec } => expression(chars_per_sec, functions, declared, external, errors),
                Statement::VideoVolume { name, volume } => {
                    expression(name, functions, declared, external, errors);
                    expression(volume, functions, declared, external, errors);
                }
                Statement::AccessibilityConfigure { settings } => {
                    expression(settings, functions, declared, external, errors)
                }
                Statement::AccessibilitySpeak { text } => {
                    expression(text, functions, declared, external, errors)
                }
                Statement::VideoPause { name }
                | Statement::VideoResume { name }
                | Statement::VideoStop { name }
                | Statement::VideoSkip { name }
                | Statement::VideoWait { name } => expression(name, functions, declared, external, errors),
                Statement::MotionStop { target } | Statement::MotionWait { target } => {
                    expression(target, functions, declared, external, errors)
                }
                Statement::Sequence{branches}=>for branch in branches{walk(branch,functions,units,declared,external,errors);},
                Statement::ForLoopWithBreak{first,last,body,..}=>{expression(first,functions,declared,external,errors);expression(last,functions,declared,external,errors);walk(body,functions,units,declared,external,errors);},
                Statement::While { condition, body } => {
                    expression(condition, functions, declared, external, errors);
                    walk(body, functions, units, declared, external, errors);
                }
                Statement::ForEach {
                    collection, body, ..
                } => {
                    expression(collection, functions, declared, external, errors);
                    walk(body, functions, units, declared, external, errors);
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    expression(condition, functions, declared, external, errors);
                    walk(then_branch, functions, units, declared, external, errors);
                    walk(else_branch, functions, units, declared, external, errors);
                }
                Statement::Dialogue { text: value, .. } => text(value, functions, declared, external, errors),
                Statement::Choice { options } => {
                    for option in options {
                        text(&option.label, functions, declared, external, errors);
                        if let Some(condition) = &option.condition {
                            expression(condition, functions, declared, external, errors);
                        }
                        walk(&option.body, functions, units, declared, external, errors);
                    }
                }
                Statement::Imagemap { hotspots, .. } => {
                    for hotspot in hotspots {
                        walk(&hotspot.body, functions, units, declared, external, errors);
                    }
                }
                _ => {}
            }
        }
    }
    walk(script, &functions, &units, &declaration_names, allow_external, &mut errors);
    fn interfaces(
        block: &[Statement],
        screens: &BTreeMap<&str, usize>,
        external: bool,
        errors: &mut Vec<LogicDiagnostic>,
    ) {
        for statement in block {
            match statement {
                Statement::FunctionLibrary{scope}=>for member in scope.members(){interfaces(std::slice::from_ref(member.declaration()),screens,external,errors);},
                Statement::UiOpen { name, .. }
                | Statement::UiClose { name }
                | Statement::UiFocus { name, .. }
                | Statement::UiSetState { name, .. } => {
                    if let Expr::Str(name) = name {
                        if let Some(count) = screens.get(name.as_str()) {
                            if let Statement::UiOpen {
                                arguments: Expr::ListLit(arguments),
                                ..
                            } = statement
                            {
                                if *count != arguments.len() {
                                    errors.push(LogicDiagnostic{name:name.clone(),code:"screen-arity",english:format!("Screen '{name}' expects {count} arguments, received {}",arguments.len()),french:format!("L’interface {name} attend {count} argument(s), reçu {}",arguments.len())});
                                }
                            }
                        } else if !external {
                            errors.push(LogicDiagnostic {
                                name: name.clone(),
                                code: "unknown-screen",
                                english: format!("Unknown screen '{name}'"),
                                french: format!("Interface inconnue : {name}"),
                            });
                        }
                    }
                }
                Statement::Init { body }
                | Statement::Function { body, .. }
                | Statement::UnitFunction { body, .. }
                | Statement::Screen { body, .. }
                | Statement::Handler { body, .. }
                | Statement::ScheduledConsumer { body, .. }
                | Statement::While { body, .. }
                | Statement::ForEach { body, .. }
                | Statement::ForLoopWithBreak {body,..} => interfaces(body, screens, external, errors),
                Statement::Sequence{branches}=>for branch in branches{interfaces(branch,screens,external,errors);},
                Statement::StandardMacroDispatch{dispatch}=>for branch in &dispatch.branches{interfaces(&branch.body,screens,external,errors);},
                Statement::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    interfaces(then_branch, screens, external, errors);
                    interfaces(else_branch, screens, external, errors);
                }
                Statement::Choice { options } => {
                    for option in options {
                        interfaces(&option.body, screens, external, errors);
                    }
                }
                Statement::Imagemap { hotspots, .. } => {
                    for hotspot in hotspots {
                        interfaces(&hotspot.body, screens, external, errors);
                    }
                }
                _ => {}
            }
        }
    }
    interfaces(script, &screens, allow_external, &mut errors);
    validate_canvas_callbacks(script, allow_external, &mut errors);
    errors
}

/// Inspect literal drawing references without evaluating user code. Dynamic
/// references remain checked by the runtime, never frozen by static tooling.
fn validate_canvas_callbacks(
    script: &[Statement],
    external: bool,
    errors: &mut Vec<LogicDiagnostic>,
) {
    fn property<'a>(expression: &'a Expr, key: &str) -> Option<&'a Expr> {
        let Expr::Call { name, args } = expression else {
            return None;
        };
        if name != "dict" {
            return None;
        }
        args.chunks_exact(2)
            .find_map(|pair| matches!(&pair[0],Expr::Str(found) if found==key).then_some(&pair[1]))
    }
    fn visit_expr(expression: &Expr, visit: &mut impl FnMut(&Expr)) {
        visit(expression);
        match expression {
            Expr::LibraryCall{call}=>for(_,argument)in call.arguments(){visit_expr(argument,visit);},
            Expr::Call { args, .. } | Expr::ListLit(args) => {
                for argument in args {
                    visit_expr(argument, visit);
                }
            }
            Expr::BinOp { left, right, .. } | Expr::And(left, right) | Expr::Or(left, right) => {
                visit_expr(left, visit);
                visit_expr(right, visit);
            }
            Expr::Index { target, index } => {
                visit_expr(target, visit);
                visit_expr(index, visit);
            }
            Expr::Neg(value) | Expr::Not(value) => visit_expr(value, visit),
            _ => {}
        }
    }
    fn visit_block(block: &[Statement], visit: &mut impl FnMut(&Expr)) {
        for statement in block {
            match statement {
                Statement::FunctionLibrary{scope}=>for member in scope.members(){visit_block(std::slice::from_ref(member.declaration()),visit);},
                Statement::LibraryUnitCall{call}=>for(_,argument)in call.arguments(){visit_expr(argument,visit);},
                Statement::ScheduledConsumer { steps, body, .. } => {
                    for step in steps { match step {
                        crate::ConsumerStep::Capture { value, .. } | crate::ConsumerStep::InspectTextTable { value, .. } => visit_expr(value, visit),
                    }}
                    visit_block(body, visit);
                }
                Statement::SetVar { value, .. }
                | Statement::LocalVar { value, .. }
                | Statement::FunctionReturn { value } => visit_expr(value, visit),
                Statement::Init { body }
                | Statement::Function { body, .. }
                | Statement::UnitFunction { body, .. }
                | Statement::Screen { body, .. }
                | Statement::Handler { body, .. } => visit_block(body, visit),
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    visit_expr(condition, visit);
                    visit_block(then_branch, visit);
                    visit_block(else_branch, visit);
                }
                Statement::Sequence{branches}=>for branch in branches{visit_block(branch,visit);},
                Statement::StandardMacroDispatch{dispatch}=>{
                    for value in dispatch.operand_expressions(){visit_expr(value,visit);}
                    for branch in &dispatch.branches{visit_block(&branch.body,visit);}
                },
                Statement::ForLoopWithBreak{first,last,body,..}=>{visit_expr(first,visit);visit_expr(last,visit);visit_block(body,visit);},
                Statement::While { condition, body } => {
                    visit_expr(condition, visit);
                    visit_block(body, visit);
                }
                Statement::ForEach {
                    collection, body, ..
                } => {
                    visit_expr(collection, visit);
                    visit_block(body, visit);
                }
                Statement::UiOpen {
                    name,
                    arguments,
                    modal,
                    layer,
                    ..
                } => {
                    for expression in [name, arguments, modal, layer] {
                        visit_expr(expression, visit);
                    }
                }
                Statement::WidgetAddToViewport { object, z_order } => { visit_expr(object, visit);visit_expr(z_order, visit); }
                Statement::WidgetRemoveFromParent { object } => visit_expr(object, visit),
                Statement::UiClose { name } => visit_expr(name, visit),
                Statement::MenuExecute { request } => visit_expr(request, visit),
                Statement::UiFocus { name, element } => {
                    for expression in [name, element] {
                        visit_expr(expression, visit);
                    }
                }
                Statement::UiSetState {
                    name,
                    element,
                    state,
                } => {
                    for expression in [name, element, state] {
                        visit_expr(expression, visit);
                    }
                }
                Statement::Dialogue { text, .. } => {
                    for segment in &text.0 {
                        if let TextSegment::Interp(expression) = segment {
                            visit_expr(expression, visit);
                        }
                    }
                }
                Statement::Choice { options } => {
                    for option in options {
                        for segment in &option.label.0 {
                            if let TextSegment::Interp(expression) = segment {
                                visit_expr(expression, visit);
                            }
                        }
                        if let Some(condition) = &option.condition {
                            visit_expr(condition, visit);
                        }
                        visit_block(&option.body, visit);
                    }
                }
                Statement::Imagemap { hotspots, .. } => {
                    for hotspot in hotspots {
                        visit_block(&hotspot.body, visit);
                    }
                }
                _ => {}
            }
        }
    }
    fn calculation_block(block: &[Statement]) -> bool {
        block.iter().all(|statement| match statement {
            Statement::ScheduledConsumer { body, .. } => calculation_block(body),
            Statement::SetVar { .. }
            | Statement::LocalVar { .. }
            | Statement::FunctionReturn { .. } => true,
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => calculation_block(then_branch) && calculation_block(else_branch),
            Statement::Sequence{branches}=>branches.iter().all(|b|calculation_block(b)),
            Statement::LoopBreak=>true,
            Statement::ForLoopWithBreak{body,..}=>calculation_block(body),
            Statement::While { body, .. } | Statement::ForEach { body, .. } => {
                calculation_block(body)
            }
            _ => false,
        })
    }
    let functions: BTreeMap<_, _> = script
        .iter()
        .filter_map(|statement| {
            if let Statement::Function {
                name,
                parameters,
                body,
            } = statement
            {
                Some((name.as_str(), (parameters.len(), body.as_slice())))
            } else {
                None
            }
        })
        .collect();
    let nonfunctions: std::collections::BTreeSet<_> = script
        .iter()
        .filter_map(|statement| match statement {
            Statement::Screen { name, .. } | Statement::Handler { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    let mut callbacks = std::collections::BTreeSet::new();
    visit_block(script, &mut |expression| {
        let props = match expression {
            Expr::Call { name, args }
                if name == "component"
                    && args.len() == 4
                    && matches!(&args[1],Expr::Str(kind) if kind=="canvas") =>
            {
                Some(&args[2])
            }
            _ if matches!(property(expression,"kind"),Some(Expr::Str(kind)) if kind=="canvas") => {
                Some(expression)
            }
            _ => None,
        };
        if let Some(Expr::Str(name)) = props.and_then(|props| property(props, "draw")) {
            callbacks.insert(name.clone());
        }
    });
    for callback in callbacks {
        let Some((arity, _)) = functions.get(callback.as_str()) else {
            if !external
                || nonfunctions.contains(callback.as_str())
                || builtin_arity(&callback).is_some()
            {
                errors.push(LogicDiagnostic {
                    name: callback.clone(),
                    code: "canvas-draw-function",
                    english: format!(
                        "Canvas draw '{callback}' must reference a user calculation function"
                    ),
                    french: format!(
                        "Le dessin Canvas {callback} doit référencer une fonction de calcul RVN"
                    ),
                });
            }
            continue;
        };
        if *arity != 3 {
            errors.push(LogicDiagnostic{name:callback.clone(),code:"canvas-draw-arity",english:format!("Canvas draw '{callback}' must accept state, props and frame (three parameters)"),french:format!("Le dessin Canvas {callback} doit accepter state, props et frame (trois paramètres)")});
        }
        let mut pending = vec![callback.clone()];
        let mut visited = std::collections::BTreeSet::new();
        while let Some(name) = pending.pop() {
            if !visited.insert(name.clone()) {
                continue;
            }
            let Some((_, body)) = functions.get(name.as_str()) else {
                continue;
            };
            let mut forbidden = None;
            visit_block(body, &mut |expression| {
                if let Expr::Call { name, args: _ } = expression {
                    if matches!(
                        name.as_str(),
                        "random" | "rand" | "mouse_clicked" | "mouse_x" | "mouse_y" | "key_pressed"
                    ) {
                        forbidden = Some(name.clone());
                    } else if functions.contains_key(name.as_str()) {
                        pending.push(name.clone());
                    }
                }
            });
            if !calculation_block(body) || forbidden.is_some() {
                let operation = forbidden.unwrap_or_else(|| "narrative/state operation".into());
                errors.push(LogicDiagnostic {
                    name: callback.clone(),
                    code: "canvas-draw-purity",
                    english: format!(
                        "Canvas draw '{callback}' is not pure: '{name}' uses {operation}"
                    ),
                    french: format!(
                        "Le dessin Canvas {callback} n’est pas pur : {name} utilise {operation}"
                    ),
                });
            }
        }
    }
}
