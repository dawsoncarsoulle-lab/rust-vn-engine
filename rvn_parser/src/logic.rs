use crate::{Expr, InterpolatedText, Statement, TextSegment};
use logos::Logos;
use std::collections::BTreeMap;

pub fn is_binding_name(name: &str) -> bool {
    let mut lexer = crate::lexer::Token::lexer(name);
    !name.starts_with("__rvn_")
        && matches!(lexer.next(),Some(Ok(crate::lexer::Token::Ident(found))) if found==name)
        && lexer.next().is_none()
}

/// Single catalogue used by the runtime, CLI and Blueprint compiler.
pub fn builtin_arity(name: &str) -> Option<std::ops::RangeInclusive<usize>> {
    Some(match name {
        "menu_action" | "menu_start_scene" | "menu_open_page" | "menu_save_page" | "menu_language"
        | "menu_choose" | "menu_gallery_cg" | "menu_gallery_tab" | "menu_confirm" | "menu_cancel" => 1..=1,
        "menu_slot" | "menu_protect" | "menu_number" | "menu_bool" => 2..=2,
        "menu_advance" | "menu_skip_typewriter" => 0..=0,
        "min" | "max" => 0..=128,
        "dict" => 0..=128,
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
        "abs" | "floor" | "ceil" | "to_int" | "string_to_text" | "text_to_string" | "len"
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
        external: bool,
        errors: &mut Vec<LogicDiagnostic>,
    ) {
        match expr {
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
                } else if !external {
                    errors.push(LogicDiagnostic {
                        name: name.clone(),
                        code: "unknown-function",
                        english: format!("Unknown RVN function '{name}'"),
                        french: format!("Fonction RVN inconnue : {name}"),
                    });
                }
                for argument in args {
                    expression(argument, functions, external, errors);
                }
            }
            Expr::BinOp { left, right, .. } | Expr::And(left, right) | Expr::Or(left, right) => {
                expression(left, functions, external, errors);
                expression(right, functions, external, errors);
            }
            Expr::Index { target, index } => {
                expression(target, functions, external, errors);
                expression(index, functions, external, errors);
            }
            Expr::Neg(value) | Expr::Not(value) => expression(value, functions, external, errors),
            Expr::ListLit(items) => {
                for item in items {
                    expression(item, functions, external, errors);
                }
            }
            _ => {}
        }
    }
    fn text(
        text: &InterpolatedText,
        functions: &BTreeMap<&str, usize>,
        external: bool,
        errors: &mut Vec<LogicDiagnostic>,
    ) {
        for segment in &text.0 {
            if let TextSegment::Interp(value) = segment {
                expression(value, functions, external, errors);
            }
        }
    }
    fn walk(
        block: &[Statement],
        functions: &BTreeMap<&str, usize>,
        external: bool,
        errors: &mut Vec<LogicDiagnostic>,
    ) {
        for statement in block {
            match statement {
                Statement::Init { body }
                | Statement::Function { body, .. }
                | Statement::Screen { body, .. }
                | Statement::Handler { body, .. } => walk(body, functions, external, errors),
                Statement::SetVar { value, .. }
                | Statement::LocalVar { value, .. }
                | Statement::FunctionReturn { value } => {
                    expression(value, functions, external, errors)
                }
                Statement::UiOpen {
                    name,
                    arguments,
                    modal,
                    layer,
                    ..
                } => {
                    for value in [name, arguments, modal, layer] {
                        expression(value, functions, external, errors);
                    }
                }
                Statement::UiClose { name } => expression(name, functions, external, errors),
                Statement::MenuExecute { request } => expression(request, functions, external, errors),
                Statement::UiFocus { name, element } => {
                    expression(name, functions, external, errors);
                    expression(element, functions, external, errors);
                }
                Statement::UiSetState {
                    name,
                    element,
                    state,
                } => {
                    for value in [name, element, state] {
                        expression(value, functions, external, errors);
                    }
                }
                Statement::MotionPlay { target, definition } => {
                    expression(target, functions, external, errors);
                    expression(definition, functions, external, errors);
                }
                Statement::CharacterCompose {
                    character,
                    definition,
                } => {
                    expression(character, functions, external, errors);
                    expression(definition, functions, external, errors);
                }
                Statement::CharacterAttributes {
                    character,
                    attributes,
                } => {
                    expression(character, functions, external, errors);
                    expression(attributes, functions, external, errors);
                }
                Statement::VideoPlay { name, definition } => {
                    expression(name, functions, external, errors);
                    expression(definition, functions, external, errors);
                }
                Statement::VideoSeek { name, seconds } => {
                    expression(name, functions, external, errors);
                    expression(seconds, functions, external, errors);
                }
                Statement::VideoVolume { name, volume } => {
                    expression(name, functions, external, errors);
                    expression(volume, functions, external, errors);
                }
                Statement::AccessibilityConfigure { settings } => {
                    expression(settings, functions, external, errors)
                }
                Statement::AccessibilitySpeak { text } => {
                    expression(text, functions, external, errors)
                }
                Statement::VideoPause { name }
                | Statement::VideoResume { name }
                | Statement::VideoStop { name }
                | Statement::VideoSkip { name }
                | Statement::VideoWait { name } => expression(name, functions, external, errors),
                Statement::MotionStop { target } | Statement::MotionWait { target } => {
                    expression(target, functions, external, errors)
                }
                Statement::While { condition, body } => {
                    expression(condition, functions, external, errors);
                    walk(body, functions, external, errors);
                }
                Statement::ForEach {
                    collection, body, ..
                } => {
                    expression(collection, functions, external, errors);
                    walk(body, functions, external, errors);
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    expression(condition, functions, external, errors);
                    walk(then_branch, functions, external, errors);
                    walk(else_branch, functions, external, errors);
                }
                Statement::Dialogue { text: value, .. } => text(value, functions, external, errors),
                Statement::Choice { options } => {
                    for option in options {
                        text(&option.label, functions, external, errors);
                        if let Some(condition) = &option.condition {
                            expression(condition, functions, external, errors);
                        }
                        walk(&option.body, functions, external, errors);
                    }
                }
                Statement::Imagemap { hotspots, .. } => {
                    for hotspot in hotspots {
                        walk(&hotspot.body, functions, external, errors);
                    }
                }
                _ => {}
            }
        }
    }
    walk(script, &functions, allow_external, &mut errors);
    fn interfaces(
        block: &[Statement],
        screens: &BTreeMap<&str, usize>,
        external: bool,
        errors: &mut Vec<LogicDiagnostic>,
    ) {
        for statement in block {
            match statement {
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
                | Statement::Screen { body, .. }
                | Statement::Handler { body, .. }
                | Statement::While { body, .. }
                | Statement::ForEach { body, .. } => interfaces(body, screens, external, errors),
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
                Statement::SetVar { value, .. }
                | Statement::LocalVar { value, .. }
                | Statement::FunctionReturn { value } => visit_expr(value, visit),
                Statement::Init { body }
                | Statement::Function { body, .. }
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
            Statement::SetVar { .. }
            | Statement::LocalVar { .. }
            | Statement::FunctionReturn { .. } => true,
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => calculation_block(then_branch) && calculation_block(else_branch),
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
