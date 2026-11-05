//! Authored Text defaults retain source and identity; translations are derived.
use crate::*;
use rvn_parser::localized_text::{DeferredText, LocalizedText, TextIdentity, TextIdentityEdit, TextLocalization, TextReference};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphTextEdit {
    Source(String), Namespace(String), Key(String), Localizable(bool), Notes(String),
}

impl GraphDocument {
    pub fn pin_text_table_reference(&self, pin: PinId) -> Result<Option<&rvn_parser::localized_text::StringTableReference>, String> {
        Ok(self.pins.get(&pin).ok_or("Missing Text pin")?.text_table_reference.as_ref())
    }

    /// The reference is the persistent value. Names are an explicit authored
    /// snapshot supplied by the binding transaction, never a display cache.
    /// Rebinding the same pair is a no-op even when its current source changed.
    pub fn set_pin_text_table_reference(&mut self, pin: PinId,
        reference: rvn_parser::localized_text::StringTableReference, arguments: &[String]) -> Result<bool, String> {
        self.validate_narrative_metadata()?;
        reference.validate()?;
        rvn_parser::localized_text::validate_text_asset_id(&reference.table)?;
        if self.text_owner.is_none() { return Err("A table Text pin needs an explicitly owned asset".into()); }
        let model = self.pins.get(&pin).ok_or("Missing Text pin")?;
        let node = model.node;
        let kind = self.nodes.get(&node).ok_or("Missing Text node")?.kind;
        let supported = matches!((kind, model.key.as_str(), model.direction),
            (NodeKind::TextValue, "value", PinDirection::Output) | (NodeKind::FormatText, "format", PinDirection::Input));
        if !supported || model.value_type != ValueType::InterpolatedText {
            return Err("Unsupported table Text pin".into());
        }
        if model.direction == PinDirection::Input && self.edges.values().any(|edge| edge.input == pin) {
            return Err("Disconnect the Text input before binding a table reference".into());
        }
        if kind == NodeKind::TextValue && !arguments.is_empty() {
            return Err("A literal Text table reference has no argument pins".into());
        }
        if model.text_table_reference.as_ref() == Some(&reference) { return Ok(false); }
        let mut trial = self.clone();
        trial.pins.get_mut(&pin).unwrap().text_localization = None;
        trial.pins.get_mut(&pin).unwrap().text_table_reference = None;
        if kind == NodeKind::FormatText {
            trial.set_format_argument_names(node, arguments).map_err(|error| error.to_string())?;
            trial.set_pin_default(node, "format", PropertyValue::String(String::new())).map_err(|error| error.to_string())?;
            trial.nodes.get_mut(&node).unwrap().properties.insert("deferred_text".into(), PropertyValue::Bool(true));
        } else {
            trial.set_property(node, "value", PropertyValue::String(String::new())).map_err(|error| error.to_string())?;
            trial.pins.get_mut(&pin).unwrap().default_value = None;
        }
        trial.nodes.get_mut(&node).unwrap().properties.remove("@text_explicit_notes");
        trial.pins.get_mut(&pin).unwrap().text_table_reference = Some(reference);
        trial.schema_version = trial.schema_version.max(crate::GRAPH_TABLE_TEXT_SCHEMA_VERSION);
        trial.validate_narrative_metadata()?;
        *self = trial;
        Ok(true)
    }

    /// Read the single authoritative source, never the rendered translation.
    pub fn pin_text(&self, pin: PinId) -> Result<Option<LocalizedText>, String> {
        let pin = self.pins.get(&pin).ok_or("Missing Text pin")?;
        let Some(localization) = &pin.text_localization else { return Ok(None) };
        let node = self.nodes.get(&pin.node).ok_or("Missing Text node")?;
        let source = if node.kind == NodeKind::TextValue && pin.key == "value"
            && pin.direction == PinDirection::Output {
            node.properties.get("value")
        } else if node.kind == NodeKind::FormatText && pin.key == "format"
            && pin.direction == PinDirection::Input {
            pin.default_value.as_ref()
        } else { return Err("Unsupported authored Text pin".into()) };
        let Some(PropertyValue::String(source)) = source else { return Err("Text source must be a string".into()) };
        Ok(Some(LocalizedText { source: source.clone(), localization: localization.clone() }))
    }

    pub(crate) fn validate_text_pins(&self) -> Result<(), String> {
        for pin in self.pins.values().filter(|pin| pin.text_table_reference.is_some()) {
            if !matches!(self.schema_version, crate::GRAPH_TABLE_TEXT_SCHEMA_VERSION | crate::GRAPH_NAME_TABLE_SCHEMA_VERSION | crate::GRAPH_NAME_TEXT_SCHEMA_VERSION | crate::GRAPH_NOMINAL_ENUM_SCHEMA_VERSION) || self.text_owner.is_none() {
                return Err("Table Text pins require explicitly owned graph schema 8".into());
            }
            let reference = pin.text_table_reference.as_ref().unwrap();
            reference.validate()?;
            rvn_parser::localized_text::validate_text_asset_id(&reference.table)?;
            if pin.text_localization.is_some() || pin.value_type != ValueType::InterpolatedText {
                return Err("A table Text pin cannot also own inline localization".into());
            }
            let node = self.nodes.get(&pin.node).ok_or("Missing table Text node")?;
            let empty_source = Some(PropertyValue::String(String::new()));
            let valid = match (node.kind, pin.key.as_str(), pin.direction) {
                (NodeKind::TextValue, "value", PinDirection::Output) => node.properties.get("value") == empty_source.as_ref() && pin.default_value.is_none(),
                (NodeKind::FormatText, "format", PinDirection::Input) => pin.default_value == empty_source
                    && !self.edges.values().any(|edge| edge.input == pin.id)
                    && node.properties.get("deferred_text") == Some(&PropertyValue::Bool(true)),
                _ => false,
            };
            if !valid { return Err("A table Text reference cannot retain a second source or a connected default".into()); }
            DeferredText::StringTable { reference: reference.clone() }.storage_work()?;
        }
        for pin in self.pins.values().filter(|p| p.text_localization.is_some()) {
            let owner = self.text_owner.as_ref().ok_or("Authored Text requires a persistent asset owner")?;
            let text = self.pin_text(pin.id)?.ok_or("Missing authored Text")?;
            if pin.value_type != ValueType::InterpolatedText {
                return Err("Authored Text requires a Text pin type".into());
            }
            if pin.direction == PinDirection::Input && self.edges.values().any(|e| e.input == pin.id) {
                return Err("Connected Text pins cannot retain an authored literal".into());
            }
            if text.localization.identity().is_some_and(|id| id.owner != owner.locale_owner()) {
                return Err("Text identity belongs to a different asset".into());
            }
            DeferredText::Literal { text }.storage_work()?;
        }
        for node in self.nodes.values().filter(|n| n.properties.contains_key("deferred_text")) {
            if node.kind != NodeKind::FormatText
                || node.properties.get("deferred_text") != Some(&PropertyValue::Bool(true))
                || self.text_owner.is_none() {
                return Err("Deferred Format Text requires an opted-in owning asset".into());
            }
            let pin = self.pin_by_key(node.id, "format").ok_or("Missing Format Text pattern")?;
            if pin.text_localization.is_none() && pin.text_table_reference.is_none() && !self.edges.values().any(|e| e.input == pin.id) {
                return Err("Deferred Format Text requires a Text pattern".into());
            }
        }
        Ok(())
    }

    /// One atomic source+metadata+argument-layout edit. Legacy setters cannot
    /// silently change the identity of an opted-in Text value.
    pub fn set_pin_text(&mut self, pin: PinId, text: LocalizedText) -> Result<bool, String> {
        self.validate_narrative_metadata()?;
        let model = self.pins.get(&pin).ok_or("Missing Text pin")?;
        if model.text_table_reference.is_some() { return Err("Convert the table reference to an independent literal before editing inline Text".into()); }
        let node = model.node;
        let kind = self.nodes.get(&node).ok_or("Missing Text node")?.kind;
        let key = model.key.clone();
        if self.pin_text(pin)?.as_ref() == Some(&text) { return Ok(false) }
        if model.direction == PinDirection::Input && self.edges.values().any(|e| e.input == pin) {
            return Err("Disconnect the Text input before editing its literal".into());
        }
        let mut trial = self.clone();
        trial.pins.get_mut(&pin).unwrap().text_localization = None;
        match (kind, key.as_str(), model.direction) {
            (NodeKind::TextValue, "value", PinDirection::Output) => {
                trial.set_property(node, "value", PropertyValue::String(text.source.clone())).map_err(|e| e.to_string())?;
            }
            (NodeKind::FormatText, "format", PinDirection::Input) => {
                trial.set_format_text_pattern(node, text.source.clone()).map_err(|e| e.to_string())?;
                trial.nodes.get_mut(&node).unwrap().properties.insert("deferred_text".into(), PropertyValue::Bool(true));
            }
            _ => return Err("Unsupported authored Text pin".into()),
        }
        trial.pins.get_mut(&pin).unwrap().text_localization = Some(text.localization);
        trial.validate_narrative_metadata()?;
        *self = trial;
        Ok(true)
    }

    /// Persistent literals counted once per pin/constructor node, regardless
    /// of wire fan-out or derived variable type caches. Runtime computations
    /// with nonliteral constructor arguments are not authored FText values.
    pub fn authored_text_literals(&self) -> Result<Vec<LocalizedText>, String> {
        self.validate_text_pins()?;
        let mut result = Vec::new();
        for pin in self.pins.values().filter(|p| p.text_localization.is_some()) {
            result.push(self.pin_text(pin.id)?.unwrap());
        }
        for node in self.nodes.values().filter(|n| n.kind == NodeKind::FunctionCall
            && n.properties.get("function") == Some(&PropertyValue::String("__rvn_localizable_text".into()))) {
            let Some(output) = node.pins.iter().filter_map(|p| self.pins.get(p))
                .find(|p| p.direction == PinDirection::Output) else { continue };
            let expression = transpile_value_output(self, node.id, &output.key).map_err(|e| e.to_string())?;
            let parsed = rvn_parser::parse(&format!("set value = {expression}")).map_err(|e| e.to_string())?;
            let [rvn_parser::Statement::SetVar { value: rvn_parser::Expr::Call { args, .. }, .. }] = parsed.as_slice() else { continue };
            let strings: Option<Vec<_>> = args.iter().map(|a| match a {
                rvn_parser::Expr::Str(s) => Some(s.clone()), _ => None,
            }).collect();
            let Some(strings) = strings else { continue };
            if !(4..=5).contains(&strings.len()) { return Err("Invalid authored Text constructor".into()) }
            let text = LocalizedText { source: strings[0].clone(), localization: TextLocalization::Localizable {
                identity: TextIdentity { owner: strings[1].clone(), namespace: strings[2].clone(), key: strings[3].clone() },
                developer_notes: strings.get(4).cloned().unwrap_or_default(),
            }};
            DeferredText::Literal { text: text.clone() }.storage_work()?;
            result.push(text);
        }
        Ok(result)
    }

    /// Adopt an old literal explicitly; legacy templates remain untouched until
    /// this transaction is requested. Allocation uses the caller's asset policy.
    pub fn adopt_pin_text(&mut self, pin: PinId, mut generate_key: impl FnMut() -> String) -> Result<bool, String> {
        if self.pin_text(pin)?.is_some() { return Ok(false) }
        let model = self.pins.get(&pin).ok_or("Missing Text pin")?;
        if model.text_table_reference.is_some() { return Err("A table reference cannot be adopted as an inline literal".into()); }
        let source = match (self.nodes[&model.node].kind, model.key.as_str()) {
            (NodeKind::TextValue, "value") => self.nodes[&model.node].properties.get("value"),
            (NodeKind::FormatText, "format") => model.default_value.as_ref(),
            _ => return Err("Unsupported authored Text pin".into()),
        };
        let Some(PropertyValue::String(source)) = source else { return Err("Text source must be a string".into()) };
        let mut text = LocalizedText { source: source.clone(), localization: TextLocalization::CultureInvariant };
        let owner = self.text_owner.as_ref().ok_or("Text needs an owning asset")?.locale_owner();
        let values = self.authored_text_literals()?;
        let references: Vec<_> = values.iter().filter_map(|text| text.localization.identity().map(|identity| TextReference { identity, source: &text.source })).collect();
        text.set_localizable(true, &owner, &references, &mut generate_key)?;
        self.set_pin_text(pin, text)
    }

    pub fn edit_pin_text(&mut self, pin: PinId, edit: GraphTextEdit,
        mut generate_key: impl FnMut() -> String) -> Result<bool, String> {
        let mut text = self.pin_text(pin)?.ok_or("Adopt the Text literal before editing metadata")?;
        let owner = self.text_owner.as_ref().ok_or("Text needs an owning asset")?.locale_owner();
        let values = self.authored_text_literals()?;
        let references: Vec<_> = values.iter().filter_map(|t| t.localization.identity().map(|identity| TextReference { identity, source: &t.source })).collect();
        let changed = match edit {
            GraphTextEdit::Source(source) => text.edit_source(source, &owner, &references, &mut generate_key)?,
            GraphTextEdit::Namespace(value) => text.edit_identifier(TextIdentityEdit::Namespace, value, &references, &mut generate_key)?,
            GraphTextEdit::Key(value) => text.edit_identifier(TextIdentityEdit::Key, value, &references, &mut generate_key)?,
            GraphTextEdit::Localizable(enabled) => text.set_localizable(enabled, &owner, &references, &mut generate_key)?,
            GraphTextEdit::Notes(notes) => {
                let TextLocalization::Localizable { developer_notes, .. } = &mut text.localization else { return Err("Invariant Text has no developer notes".into()) };
                if *developer_notes == notes { false } else { *developer_notes = notes; true }
            }
        };
        if !changed { return Ok(false) }
        self.set_pin_text(pin, text)
    }
}

pub(crate) fn emit_table_text(reference: &rvn_parser::localized_text::StringTableReference) -> Result<String, String> {
    reference.validate()?;
    rvn_parser::localized_text::validate_text_asset_id(&reference.table)?;
    let q = |value: &str| serde_json::to_string(value).expect("String serialization");
    Ok(format!("__rvn_string_table_text({}, {})", q(&reference.table), q(&reference.key)))
}

/// Only an already opted-in source scope uses the dedicated pin form. Dynamic
/// arguments and historical opaque host IDs remain genuine constructor calls.
pub(crate) fn table_text_expression(expr: &rvn_parser::Expr)
    -> Result<Option<rvn_parser::localized_text::StringTableReference>, String> {
    let rvn_parser::Expr::Call { name, args } = expr else { return Ok(None); };
    if name != "__rvn_string_table_text" { return Ok(None); }
    let [rvn_parser::Expr::Str(table), rvn_parser::Expr::Str(key)] = args.as_slice() else { return Ok(None); };
    if rvn_parser::localized_text::validate_text_asset_id(table).is_err() { return Ok(None); }
    let reference = rvn_parser::localized_text::StringTableReference { table: table.clone(), key: key.clone() };
    reference.validate()?;
    Ok(Some(reference))
}

pub(crate) fn emit_authored_text(text: &LocalizedText, explicit_notes: bool) -> String {
    let q = |value: &str| serde_json::to_string(value).expect("String serialization");
    match &text.localization {
        TextLocalization::Empty => "__rvn_empty_text()".into(),
        TextLocalization::CultureInvariant => format!("__rvn_invariant_text({})", q(&text.source)),
        TextLocalization::Localizable { identity, developer_notes } if !explicit_notes && developer_notes.is_empty() => format!(
            "__rvn_localizable_text({}, {}, {}, {})", q(&text.source), q(&identity.owner), q(&identity.namespace), q(&identity.key)),
        TextLocalization::Localizable { identity, developer_notes } => format!(
            "__rvn_localizable_text({}, {}, {}, {}, {})", q(&text.source), q(&identity.owner),
            q(&identity.namespace), q(&identity.key), q(developer_notes)),
    }
}

/// Decode only literal constructor data in an explicitly owned asset. This
/// never evaluates an expression or guesses ownership from source content.
pub(crate) fn authored_text_expression(expr: &rvn_parser::Expr, owner: &TextOwnerId)
    -> Result<Option<LocalizedText>, String> {
    let rvn_parser::Expr::Call { name, args } = expr else { return Ok(None) };
    let strings: Option<Vec<_>> = args.iter().map(|a| match a {
        rvn_parser::Expr::Str(s) => Some(s.clone()), _ => None,
    }).collect();
    let Some(strings) = strings else { return Ok(None) };
    let text = match (name.as_str(), strings.as_slice()) {
        ("__rvn_empty_text", []) => LocalizedText { source: String::new(), localization: TextLocalization::Empty },
        ("__rvn_invariant_text", [source]) => LocalizedText { source: source.clone(), localization: TextLocalization::CultureInvariant },
        ("__rvn_localizable_text", [source, asset, namespace, key])
        | ("__rvn_localizable_text", [source, asset, namespace, key, _]) if *asset == owner.locale_owner() =>
            LocalizedText { source: source.clone(), localization: TextLocalization::Localizable {
                identity: TextIdentity {owner: asset.clone(), namespace: namespace.clone(), key: key.clone()},
                developer_notes: strings.get(4).cloned().unwrap_or_default(),
            } },
        _ => return Ok(None),
    };
    DeferredText::Literal { text: text.clone() }.storage_work()?;
    Ok(Some(text))
}
