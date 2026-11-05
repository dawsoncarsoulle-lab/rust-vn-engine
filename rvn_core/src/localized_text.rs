//! Pure display projection of portable text values. No gameplay evaluation,
//! filesystem access, global locale, random state or saved display cache.
use crate::locale::LocaleManager;
use rvn_parser::localized_text::{DeferredText, TextArgument, TextLocalization};
use std::collections::BTreeMap;

/// Additive RVN constructors. Historical string_to_text remains a string alias
/// until a graph explicitly opts in to portable text metadata.
pub fn construct(name: &str, arguments: &[rvn_parser::Value]) -> crate::eval::EvalResult<rvn_parser::Value> {
    use rvn_parser::{Value, localized_text::{LocalizedText, TextIdentity}};
    let invalid = |message: &str| crate::eval::EvalError::InvalidFunction(message.into());
    for value in arguments { crate::value_limits::value_work(value)?; }
    let text = match (name, arguments) {
        ("__rvn_empty_text", []) => DeferredText::Literal { text: LocalizedText {
            source: String::new(), localization: TextLocalization::Empty,
        } },
        ("__rvn_invariant_text", [Value::Str(source)]) => DeferredText::invariant(source.clone()),
        // Explicit Name conversion preserves display spelling, never its folded
        // identity, interpolation, a locale key or a rendered translation.
        ("__rvn_invariant_text", [Value::Name(source)]) => DeferredText::invariant(source.as_str()),
        ("__rvn_string_table_text", [Value::Str(table), Value::Str(key)]) => {
            let reference = rvn_parser::localized_text::StringTableReference { table: table.clone(), key: key.clone() };
            reference.validate().map_err(crate::eval::EvalError::InvalidFunction)?;
            DeferredText::StringTable { reference }
        },
        ("__rvn_string_table_text_name", [Value::Name(table), Value::Str(key)]) => {
            let reference = rvn_parser::localized_text::StringTableReference { table: table.as_str().into(), key: key.clone() };
            reference.validate_name().map_err(crate::eval::EvalError::InvalidFunction)?;
            DeferredText::NameStringTable { reference }
        },
        ("__rvn_localizable_text", [Value::Str(source), Value::Str(owner), Value::Str(namespace), Value::Str(key), rest @ ..])
            if rest.is_empty() || matches!(rest, [Value::Str(_)]) => {
                let identity = TextIdentity { owner: owner.clone(), namespace: namespace.clone(), key: key.clone() };
                identity.validate().map_err(crate::eval::EvalError::InvalidFunction)?;
                let notes = match rest { [Value::Str(notes)] => notes.clone(), _ => String::new() };
                DeferredText::Literal { text: LocalizedText { source: source.clone(), localization:
                    TextLocalization::Localizable { identity, developer_notes: notes } } }
            },
        ("__rvn_format_text", [pattern, Value::Dict(arguments)]) => {
            let pattern = match pattern {
                Value::Text(text) => text.clone(),
                Value::Str(source) => Box::new(DeferredText::invariant(source.clone())),
                _ => return Err(invalid("format_text requires a Text or string pattern")),
            };
            let arguments = arguments.iter().map(|(key, value)| {
                let argument = match value {
                    Value::Str(value) => TextArgument::String(value.clone()),
                    Value::Text(value) => TextArgument::Text(value.clone()),
                    Value::Int(value) => TextArgument::Int(*value),
                    Value::Float(value) => TextArgument::Float(*value),
                    Value::Bool(value) => TextArgument::Bool(*value),
                    Value::Name(_) => return Err(invalid("A Name format argument requires explicit __rvn_name_to_string conversion")),
                    Value::Enum(_) | Value::Widget(_) | Value::WidgetClass(_) | Value::List(_) | Value::Dict(_) => return Err(invalid("Format arguments require supported scalar or Text values")),
                };
                Ok((key.clone(), argument))
            }).collect::<Result<_, _>>()?;
            DeferredText::Format { pattern, arguments }
        },
        _ => return Err(invalid("Invalid portable Text constructor arguments")),
    };
    let value = Value::Text(Box::new(text));
    crate::value_limits::value_work(&value)?;
    Ok(value)
}

/// Emit an independent RVN value expression, preserving source, metadata,
/// argument types and nesting. No rendered translation enters generated code.
pub fn source_expression(text: &DeferredText) -> Result<String, String> {
    text.storage_work()?;
    fn emit(text: &DeferredText) -> Result<String, String> {
        let quote = |value: &str| serde_json::to_string(value).map_err(|error| error.to_string());
        let result = match text {
            DeferredText::StringTable { reference } => format!("__rvn_string_table_text({}, {})", quote(&reference.table)?, quote(&reference.key)?),
            DeferredText::NameStringTable { reference } => format!("__rvn_string_table_text_name(__rvn_literal_name({}), {})", quote(&reference.table)?, quote(&reference.key)?),
            DeferredText::Literal { text } => match &text.localization {
                TextLocalization::Empty => "__rvn_empty_text()".into(),
                TextLocalization::CultureInvariant => format!("__rvn_invariant_text({})", quote(&text.source)?),
                TextLocalization::Localizable { identity, developer_notes } => format!(
                    "__rvn_localizable_text({}, {}, {}, {}, {})", quote(&text.source)?, quote(&identity.owner)?,
                    quote(&identity.namespace)?, quote(&identity.key)?, quote(developer_notes)?),
            },
            DeferredText::Format { pattern, arguments } => {
                let mut entries = Vec::with_capacity(arguments.len());
                for (name, value) in arguments {
                    let value = match value {
                        TextArgument::String(value) => quote(value)?,
                        TextArgument::Text(value) => emit(value)?,
                        TextArgument::Int(value) if *value == i64::MIN => "(-9223372036854775807 - 1)".into(),
                        TextArgument::Int(value) => value.to_string(),
                        TextArgument::Bool(value) => value.to_string(),
                        TextArgument::Float(value) => {
                            let mut value = value.to_string();
                            if !value.contains(['.', 'e', 'E']) { value.push_str(".0"); }
                            value
                        },
                    };
                    entries.push(format!("{}: {value}", quote(name)?));
                }
                format!("__rvn_format_text({}, {{{}}})", emit(pattern)?, entries.join(", "))
            },
        };
        if result.len() > 8 * 1_048_576 { return Err("Generated Text expression exceeds 8 MiB".into()); }
        Ok(result)
    }
    emit(text)
}

/// Scalar formatting is a host contract. Keep the existing VN number rendering
/// as default until culture-specific numeric formatting is qualified separately.
pub trait TextScalarFormatter {
    fn integer(&self, value: i64) -> String;
    fn float(&self, value: f32) -> String;
    fn boolean(&self, value: bool) -> String;
}

pub struct VnTextScalars;
impl TextScalarFormatter for VnTextScalars {
    fn integer(&self, value: i64) -> String { value.to_string() }
    fn float(&self, value: f32) -> String { value.to_string() }
    fn boolean(&self, value: bool) -> String { value.to_string() }
}

pub fn render_text(
    text: &DeferredText,
    locale: Option<&LocaleManager>,
    scalars: &impl TextScalarFormatter,
) -> Result<String, String> {
    render_text_with_resources(text, locale, scalars, None)
}

pub fn render_text_with_resources(
    text: &DeferredText,
    locale: Option<&LocaleManager>,
    scalars: &impl TextScalarFormatter,
    resources: Option<&crate::string_table::StringTableResources>,
) -> Result<String, String> {
    render_text_with_resources_work(text, locale, scalars, resources).map(|(display, _)| display)
}

pub(crate) fn render_text_with_resources_work(
    text: &DeferredText, locale: Option<&LocaleManager>, scalars: &impl TextScalarFormatter,
    resources: Option<&crate::string_table::StringTableResources>,
) -> Result<(String, usize), String> {
    text.storage_work()?;
    let mut budget = TextBudget { nodes: 0, bytes: 0, lookup_bytes: 0, copies: 0 };
    let display = render(text, locale, scalars, resources, &mut budget, 0)?;
    Ok((display, budget.nodes + budget.copies + (budget.bytes + budget.lookup_bytes) / 32))
}

struct TextBudget { nodes: usize, bytes: usize, lookup_bytes: usize, copies: usize }
impl TextBudget {
    fn charge_lookup(&mut self, bytes: usize) -> Result<(), String> {
        self.lookup_bytes = self.lookup_bytes.checked_add(bytes).ok_or("Text source lookup work overflow")?;
        if self.lookup_bytes > crate::value_limits::MAX_VALUE_BYTES {
            return Err("Text source lookup exceeds 1 MiB of work".into());
        }
        Ok(())
    }

    fn visit(&mut self, depth: usize) -> Result<(), String> {
        self.nodes += 1;
        if depth > 64 || self.nodes > 65_536 {
            return Err("Text display exceeds 64 levels or 65,536 values".into());
        }
        Ok(())
    }
    fn charge(&mut self, bytes: usize) -> Result<(), String> {
        self.bytes = self.bytes.checked_add(bytes).ok_or("Text display size overflow")?;
        if self.bytes > crate::value_limits::MAX_VALUE_BYTES {
            return Err("Text display exceeds 1 MiB of work".into());
        }
        Ok(())
    }
}

fn render(
    text: &DeferredText, locale: Option<&LocaleManager>, scalars: &impl TextScalarFormatter,
    resources: Option<&crate::string_table::StringTableResources>,
    budget: &mut TextBudget, depth: usize,
) -> Result<String, String> {
    budget.visit(depth)?;
    match text {
        DeferredText::StringTable { reference } => {
            let (display, lookup_work) = resources.ok_or("A string-table Text needs an explicit resource context")?.project_with_lookup_work(reference, locale)?;
            budget.charge_lookup(lookup_work)?;
            budget.charge(reference.table.len())?;
            budget.charge(reference.key.len())?;
            budget.charge(display.len())?;
            Ok(display)
        },
        DeferredText::NameStringTable { reference } => {
            let (display, lookup_work, copies) = resources.ok_or("A string-table Text needs an explicit resource context")?.project_name_with_lookup_work(reference, locale)?;
            budget.copies = budget.copies.checked_add(copies).ok_or("Name Text copy work overflow")?;
            if budget.copies > 1_000_000 { return Err("Name Text exceeds 1,000,000 value-copy units".into()); }
            budget.charge_lookup(lookup_work)?;
            budget.charge(display.len())?;
            Ok(display)
        },
        DeferredText::Literal { text } => {
            if matches!(text.localization, TextLocalization::Empty) && !text.source.is_empty() {
                return Err("Empty text metadata requires an empty source".into());
            }
            if let Some(identity) = text.localization.identity() { identity.validate()?; }
            let (display, lookup_work) = locale.map(|locale| locale.resolve_text_with_lookup_work(&text.source, Some(&text.localization)))
                .unwrap_or((&text.source, 0));
            budget.charge_lookup(lookup_work)?;
            budget.charge(text.source.len())?;
            budget.charge(display.len())?;
            Ok(display.into())
        }
        DeferredText::Format { pattern, arguments } => {
            if arguments.len() > crate::value_limits::MAX_COLLECTION_ITEMS {
                return Err("A formatted text exceeds 4,096 arguments".into());
            }
            let pattern = render(pattern, locale, scalars, resources, budget, depth + 1)?;
            let mut values = BTreeMap::new();
            for (name, argument) in arguments {
                budget.visit(depth + 1)?;
                budget.charge(name.len())?;
                let value = match argument {
                    TextArgument::Text(text) => render(text, locale, scalars, resources, budget, depth + 1)?,
                    TextArgument::String(value) => { budget.charge(value.len())?; value.clone() },
                    TextArgument::Int(value) => scalars.integer(*value),
                    TextArgument::Float(value) if value.is_finite() => scalars.float(*value),
                    TextArgument::Float(_) => return Err("A text argument must be finite".into()),
                    TextArgument::Bool(value) => scalars.boolean(*value),
                };
                budget.charge(value.len())?;
                values.insert(name.as_str(), value);
            }
            let result = rvn_parser::localized_text::substitute_named(&pattern, &values,
                crate::value_limits::MAX_VALUE_BYTES.saturating_sub(budget.bytes))?;
            budget.charge(result.len())?;
            Ok(result)
        }
    }
}


/// Pure top-level history queries. Availability, locale and nested Format
/// references do not determine whether a Text itself is from a String Table.
pub fn inspect_table_identity(name: &str, values: &[rvn_parser::Value]) -> crate::eval::EvalResult<rvn_parser::Value> {
    use rvn_parser::{Name, Value};
    use crate::eval::EvalError;
    let [value] = values else { return Err(EvalError::InvalidFunction("Text identity expects one Text".into())); };
    crate::value_limits::value_work(value)?;
    let reference = match value {
        Value::Text(text) => match text.as_ref() {
            DeferredText::StringTable { reference } | DeferredText::NameStringTable { reference } => Some(reference),
            _ => None,
        },
        // Historical string_to_text and old saves can retain String values.
        // Their safe invariant-text adapter has no table identity.
        Value::Str(_) => None,
        _ => return Err(EvalError::InvalidFunction("Text identity requires Text (or a retained legacy text String)".into())),
    };
    let result = match name {
        "__rvn_text_table_valid" => Value::Bool(reference.is_some()),
        "__rvn_text_table_id" => Value::Name(match reference {
            Some(reference) => Name::new(&reference.table).map_err(|error| EvalError::InvalidFunction(format!("Legacy Text table identity is not representable as Name: {error}")))?,
            None => Name::default(),
        }),
        "__rvn_text_table_key" => Value::Str(reference.map_or("", |reference| reference.key.as_str()).into()),
        _ => return Err(EvalError::InvalidFunction("Unknown Text identity projection".into())),
    };
    crate::value_limits::value_work(&result)?;
    Ok(result)
}
