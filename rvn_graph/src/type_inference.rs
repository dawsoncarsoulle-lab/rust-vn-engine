//! Read-only data types for presentation and connection checks. A constraint
//! such as IndexKey is not a value type: its connected value may be a String or
//! an Int. Unknown values must never acquire a type merely from their consumer.
use crate::*;
use rvn_parser::{BinOpKind, Expr};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableScope {
    Global,
    Parameter,
    Local,
}

impl GraphDocument {
    /// Names scoped to this calculation/handler, not project declarations.
    /// Parameters and `local`/`for` shadow globals. Ordinary Set is local in a
    /// calculation or screen, but global in a handler unless already shadowed.
    pub fn local_variable_names(&self) -> BTreeSet<String> {
        let mut names = self.parameter_names();
        for node in self.nodes.values() {
            if crate::widget_factory_graph_1001::is_local_assignment(node)
                || (matches!(node.kind,NodeKind::ForEach|NodeKind::ForLoopWithBreak)
                    && matches!(
                        self.kind,
                        GraphKind::Function { .. }
                            | GraphKind::UnitFunction { .. }
                            | GraphKind::Screen { .. }
                            | GraphKind::Handler { .. }
                    ))
                || (node.kind == NodeKind::SetVariable
                    && matches!(
                        self.kind,
                        GraphKind::Function { .. } | GraphKind::Screen { .. }
                    ))
            {
                if let Some(name) = self.variable_node_name(node.id) {
                    names.insert(name);
                }
            }
        }
        names
    }

    pub fn node_variable_scope(&self, node: NodeId) -> Option<VariableScope> {
        let name = self.variable_node_name(node)?;
        self.variable_scope(&name)
    }

    pub fn variable_scope(&self, name: &str) -> Option<VariableScope> {
        if self.parameter_names().contains(name) {
            Some(VariableScope::Parameter)
        } else if self.local_variable_names().contains(name) {
            Some(VariableScope::Local)
        } else if self.variables.contains_key(name) {
            Some(VariableScope::Global)
        } else {
            None
        }
    }

    fn parameter_names(&self) -> BTreeSet<String> {
        if matches!(self.kind,GraphKind::UnitFunction{..}) { return crate::unit_graph_source1003::parameters(self).map(|pins|pins.iter().map(|pin|pin.label.clone()).collect()).unwrap_or_default(); }
        self.nodes
            .values()
            .filter(|node| {
                matches!(
                    node.kind,
                    NodeKind::FunctionEntry | NodeKind::ScreenEntry | NodeKind::HandlerEntry
                )
            })
            .filter_map(|node| match node.properties.get("parameters") {
                Some(PropertyValue::StringList(names)) => Some(names),
                _ => None,
            })
            .flatten()
            .cloned()
            .collect()
    }

    /// Resolve the assignment/read identity using the same precedence as RVN:
    /// the authored name, an existing static identity wire, then its default.
    pub fn variable_node_name(&self, node: NodeId) -> Option<String> {
        let owner = self.nodes.get(&node)?;
        if !matches!(
            owner.kind,
            NodeKind::VariableGet
                | NodeKind::VariableReference
                | NodeKind::SetVariable
                | NodeKind::LocalVariable
                | NodeKind::WidgetCreate
                | NodeKind::ForEach
                | NodeKind::ForLoopWithBreak
        ) {
            return None;
        }
        if let Some(PropertyValue::String(name)) = owner.properties.get("name") {
            return Some(name.clone());
        }
        let pin = self.pin_by_key(node, "name")?;
        if let Some(edge) = self.edges.values().find(|edge| edge.input == pin.id) {
            let producer = self.nodes.get(&self.pins.get(&edge.output)?.node)?;
            return match (
                producer.kind,
                producer.properties.get("name"),
                producer.properties.get("value"),
            ) {
                (NodeKind::VariableReference, Some(PropertyValue::String(name)), _) => {
                    Some(name.clone())
                }
                (NodeKind::Literal, _, Some(PropertyValue::String(name))) => Some(name.clone()),
                _ => None,
            };
        }
        match &pin.default_value {
            Some(PropertyValue::String(name)) => Some(name.clone()),
            _ => None,
        }
    }

    /// Infer once per graph draw, then use this same map for node pins and wires.
    /// Cycles or exhausted work return Any rather than guessing or recursing
    /// forever. This never changes source, identifiers or serialized pin types.
    pub fn effective_pin_types(&self) -> BTreeMap<PinId, ValueType> {
        let mut inference = Inference::new(self);
        for id in self.pins.keys().copied() {
            let ty = inference.pin(id, 0);
            inference.types.entry(id).or_insert(ty);
        }
        inference.types
    }

    pub fn effective_pin_type(&self, pin: PinId) -> Option<ValueType> {
        self.pins.get(&pin)?;
        Some(Inference::new(self).pin(pin, 0))
    }

    /// The value used by an unconnected operand. Authored defaults are retained
    /// even while hidden by a wire. A genuinely vacant arithmetic/comparison
    /// operand acquires a default only when another operand proves its domain.
    /// This is the same read-only projection used by inference and codegen;
    /// merely drawing a graph never creates or saves an authored default.
    pub fn effective_pin_default_value(&self, pin: PinId) -> Option<PropertyValue> {
        let model = self.pins.get(&pin)?;
        if model.direction != PinDirection::Input || model.value_type.is_execution() {
            return None;
        }
        if let Some(value) = &model.default_value {
            return Some(value.clone());
        }
        Inference::new(self).default_value(pin, 0)
    }

    /// Resolve a whole draw's defaults together, sharing the bounded inference
    /// cache instead of recursively resolving a large graph once per field.
    pub fn effective_pin_default_values(&self) -> BTreeMap<PinId, PropertyValue> {
        let mut inference = Inference::new(self);
        self.pins
            .keys()
            .copied()
            .filter_map(|pin| inference.default_value(pin, 0).map(|value| (pin, value)))
            .collect()
    }

    /// Wire values use the producer's effective type, never the consumer's
    /// requested type (which would conceal required conversions/conflicts).
    pub fn effective_edge_type(&self, edge: EdgeId) -> Option<ValueType> {
        let edge = self.edges.get(&edge)?;
        self.effective_pin_type(edge.output)
    }

    /// Authoring compatibility constraint, distinct from the connected value's
    /// presentation type. Includes current registry types and legacy IndexKey.
    pub fn pin_constraint_type(&self, pin: PinId) -> Option<ValueType> {
        self.pin_constraint_type_with_binding_stamp(pin,self.source_binding_stamp())
    }

    pub(crate) fn pin_constraint_type_with_binding_stamp(&self,pin:PinId,stamp:u64) -> Option<ValueType> {
        if let Some(value_type) = self.source_binding_pin_type_with_stamp(pin,stamp) { return Some(value_type); }
        let pin = self.pins.get(&pin)?;
        let node = self.nodes.get(&pin.node)?;
        if node.kind == NodeKind::Reroute {
            return Some(reroute_constraint(self, node.id));
        }
        // Old presentation caches used an integer-only Index pin. The current
        // operator accepts integer positions and string dictionary keys.
        if node.kind == NodeKind::Index && pin.key == "index" {
            return Some(ValueType::IndexKey);
        }
        if matches!(
            node.kind,
            NodeKind::SetVariable | NodeKind::LocalVariable | NodeKind::VariableGet
        ) && matches!(pin.key.as_str(), "value" | "value_out")
        {
            if let Some(variable) = self
                .variable_node_name(node.id)
                .and_then(|name| self.variables.get(&name))
            {
                return Some(variable.value_type.clone());
            }
        }
        Some(pin.value_type.clone())
    }
}

struct Inference<'a> {
    graph: &'a GraphDocument,
    types: BTreeMap<PinId, ValueType>,
    active: BTreeSet<PinId>,
    operand_contexts: BTreeMap<NodeId, Option<ValueType>>,
    active_operand_contexts: BTreeSet<NodeId>,
    incoming: BTreeMap<PinId, Option<PinId>>,
    work: usize,
    binding_stamp: u64,
    reroutes: crate::reroute_net::RerouteTopology,
    reroute_types: BTreeMap<(usize, bool), ValueType>,
    active_reroutes: BTreeSet<(usize, bool)>,
    exhausted: bool,
}

pub(crate) fn reroute_constraint(graph: &GraphDocument, node: NodeId) -> ValueType {
    Inference::new(graph).reroute_type(node, 0, true)
}

pub(crate) fn reroute_edit_constraint(graph: &GraphDocument, node: NodeId) -> Option<ValueType> {
    let mut inference=Inference::new(graph);
    let ty=inference.reroute_type(node,0,true);
    (!inference.exhausted).then_some(ty)
}

pub(crate) fn reroute_exec_connection_resolvable(graph:&GraphDocument,output:PinId,input:PinId)->bool {
    let mut inference=Inference::new(graph);
    inference.reroutes=graph.reroute_topology_with_link(Some((output,input)));
    for pin in [output,input] {
        let Some(model)=graph.pins.get(&pin)else{return false;};
        if graph.nodes.get(&model.node).is_some_and(|n|n.kind==NodeKind::Reroute)
            && inference.reroute_type(model.node,0,true)!=ValueType::Execution{return false;}
    }
    !inference.exhausted
}

pub(crate) fn reroute_constraints(graph: &GraphDocument) -> BTreeMap<PinId, ValueType> {
    let mut inference=Inference::new(graph);
    let nodes: Vec<_>=inference.reroutes.by_node.keys().copied().collect();
    let mut result=BTreeMap::new();
    for node in nodes {
        let ty=inference.reroute_type(node,0,true);
        for key in ["value","value_out"] {
            if let Some(pin)=graph.pin_by_key(node,key) { result.insert(pin.id,ty.clone()); }
        }
    }
    result
}

impl<'a> Inference<'a> {
    fn reroute_type(&mut self, node: NodeId, depth: usize, constraint: bool) -> ValueType {
        let Some(index)=self.reroutes.by_node.get(&node).copied() else { return ValueType::Any; };
        let key=(index,constraint);
        if let Some(ty)=self.reroute_types.get(&key) { return ty.clone(); }
        if depth>128 || self.work>=100_000 { self.exhausted=true; return ValueType::Any; }
        if !self.active_reroutes.insert(key) { return ValueType::Any; }
        let net=&self.reroutes.nets[index];
        let valid=net.valid;
        let too_deep=net.too_deep;
        let cost=net.nodes.len()+net.producers.len()+net.consumers.len();
        let producers: Vec<_>=net.producers.iter().copied().collect();
        let consumers: Vec<_>=net.consumers.iter().copied().collect();
        self.work=self.work.saturating_add(cost);
        let mut ty=ValueType::Any;
        // Preserve the engine's existing bounded-inference contract even
        // though a connected knot chain is now flattened before inference.
        if too_deep || self.work>100_000 { self.exhausted=true; }
        if valid && !self.exhausted {
            // Actual producers win. Consumer constraints can type a cable,
            // but never manufacture a value for an unknown connected producer.
            for source in &producers {
                let candidate=self.pin(*source,depth+1);
                if candidate!=ValueType::Any { ty=candidate; break; }
            }
            if ty==ValueType::Any && (constraint || producers.is_empty()) {
                for consumer in consumers {
                    let candidate=self.graph.pin_constraint_type_with_binding_stamp(consumer,self.binding_stamp).unwrap_or(ValueType::Any);
                    if candidate!=ValueType::Any { ty=candidate; break; }
                }
            }
        }
        self.active_reroutes.remove(&key);
        self.reroute_types.insert(key,ty.clone());
        ty
    }

    fn new(graph: &'a GraphDocument) -> Self {
        let mut incoming = BTreeMap::new();
        for edge in graph.edges.values() {
            incoming
                .entry(edge.input)
                .and_modify(|source| *source = None)
                .or_insert(Some(edge.output));
        }
        Self {
            graph,
            types: BTreeMap::new(),
            active: BTreeSet::new(),
            operand_contexts: BTreeMap::new(),
            active_operand_contexts: BTreeSet::new(),
            incoming,
            work: 0,
            binding_stamp: graph.source_binding_stamp(),
            reroutes: graph.reroute_topology(),
            reroute_types: BTreeMap::new(),
            active_reroutes: BTreeSet::new(),
            exhausted: false,
        }
    }

    fn pin(&mut self, id: PinId, depth: usize) -> ValueType {
        if let Some(ty) = self.types.get(&id) {
            return ty.clone();
        }
        if self
            .graph
            .pins
            .get(&id)
            .is_some_and(|pin| pin.value_type.is_execution()
                && self.graph.nodes.get(&pin.node).is_some_and(|n| n.kind != NodeKind::Reroute))
        {
            self.types.insert(id, ValueType::Execution);
            return ValueType::Execution;
        }
        if depth > 128 || self.work >= 100_000 {
            self.exhausted=true;
            return ValueType::Any;
        }
        if !self.active.insert(id) { return ValueType::Any; }
        self.work += 1;
        let ty = self.infer(id, depth + 1);
        self.active.remove(&id);
        self.types.insert(id, ty.clone());
        ty
    }

    fn infer(&mut self, id: PinId, depth: usize) -> ValueType {
        let Some(pin) = self.graph.pins.get(&id) else {
            return ValueType::Any;
        };
        let Some(node) = self.graph.nodes.get(&pin.node) else {
            return ValueType::Any;
        };
        if node.kind == NodeKind::Reroute {
            return self.reroute_type(node.id, depth, false);
        }
        if pin.value_type.is_execution() {
            return ValueType::Execution;
        }
        if pin.direction == PinDirection::Input {
            let declared = self.graph.pin_constraint_type_with_binding_stamp(id,self.binding_stamp).unwrap_or(ValueType::Any);
            if let Some(Some(source)) = self.incoming.get(&id) {
                let value = self.pin(*source, depth);
                // Show the value actually carried by every accepted legacy
                // connection, including numeric widening and unknown sources.
                // The separate constraint remains intact for authoring checks.
                if declared.accepts(&value) && node.kind.accepts_data_source(&value) {
                    return value;
                }
                return declared;
            }
            if self.incoming.get(&id) == Some(&None) {
                return ValueType::Any;
            }
            if node.kind == NodeKind::ConvertNumberToText && pin.key == "value" {
                // This is a numeric union, not an operation requesting Float
                // precision. Integer-to-String uses an integer-colored input.
                let value = self.input_value(node.id, "value", depth);
                return numeric_type(value);
            }
            if matches!(
                declared,
                ValueType::Any | ValueType::IndexKey | ValueType::List(_)
            ) {
                let value = match self.incoming.get(&id) {
                    Some(Some(source)) => self.pin(*source, depth),
                    Some(None) => ValueType::Any,
                    None => self.default_type(id, depth),
                };
                // Narrow only wildcard/union constraints, not fixed typed pins.
                return match declared {
                    ValueType::List(_)
                        if !matches!(value, ValueType::List(_)) || !declared.accepts(&value) =>
                    {
                        declared
                    }
                    ValueType::IndexKey if !declared.accepts(&value) => declared,
                    ValueType::IndexKey if value == ValueType::Any => ValueType::Any,
                    _ => value,
                };
            }
            return declared;
        }
        match node.kind {
            NodeKind::Literal => node
                .properties
                .get("value")
                .or(pin.default_value.as_ref())
                .map(property_type)
                .unwrap_or_else(|| pin.value_type.clone()),
            NodeKind::VariableGet => self.graph.pin_constraint_type_with_binding_stamp(id,self.binding_stamp).unwrap_or(ValueType::Any),
            NodeKind::SetVariable | NodeKind::LocalVariable if pin.key == "value_out" => {
                let declared = self.graph.pin_constraint_type_with_binding_stamp(id,self.binding_stamp).unwrap_or(ValueType::Any);
                if declared == ValueType::Any {
                    self.input_value(node.id, "value", depth)
                } else {
                    declared
                }
            }
            NodeKind::ForEach if node.native_loop_contract == Some(crate::NativeLoopContract::DemandV1) && pin.key == "array_element" => {
                match self.input_value(node.id, "collection", depth) { ValueType::List(item) => *item, _ => ValueType::Any }
            },
            NodeKind::Reroute => self.input_value(node.id, "value", depth),
            NodeKind::MathAdd
            | NodeKind::MathSubtract
            | NodeKind::MathMultiply
            | NodeKind::MathDivide => {
                let keys: Vec<_> = node
                    .pins
                    .iter()
                    .filter_map(|id| self.graph.pins.get(id))
                    .filter(|pin| {
                        pin.direction == PinDirection::Input && !pin.value_type.is_execution()
                    })
                    .map(|pin| pin.key.clone())
                    .collect();
                let mut types = keys.iter().map(|key| self.input_value(node.id, key, depth));
                let first = types.next().unwrap_or(ValueType::Any);
                types.fold(first, |left, right| {
                    arithmetic_type(
                        node.kind == NodeKind::MathAdd && !node.uses_blueprint_operator_policy(),
                        &left,
                        &right,
                    )
                })
            }
            NodeKind::MathNegate => numeric_type(self.input_value(node.id, "value", depth)),
            NodeKind::BinaryOperator => {
                let left = self.input_value(node.id, "left", depth);
                let right = self.input_value(node.id, "right", depth);
                match node
                    .properties
                    .get("operator")
                    .or_else(|| node.properties.get("op"))
                {
                    Some(PropertyValue::String(op))
                        if matches!(
                            op.as_str(),
                            "==" | "!=" | "<" | "<=" | ">" | ">=" | "and" | "or"
                        ) =>
                    {
                        ValueType::Bool
                    }
                    Some(PropertyValue::String(op))
                        if matches!(op.as_str(), "+" | "-" | "*" | "/") =>
                    {
                        arithmetic_type(
                            op == "+" && !node.uses_blueprint_operator_policy(),
                            &left,
                            &right,
                        )
                    }
                    _ => ValueType::Any,
                }
            }
            NodeKind::UnaryOperator => match node
                .properties
                .get("operator")
                .or_else(|| node.properties.get("op"))
            {
                Some(PropertyValue::String(op)) if op == "not" => ValueType::Bool,
                Some(PropertyValue::String(op)) if op == "-" => {
                    numeric_type(self.input_value(node.id, "value", depth))
                }
                _ => ValueType::Any,
            },
            NodeKind::ListLiteral => {
                let values = self.sequence_types(node.id, depth);
                ValueType::List(Box::new(common_type(values)))
            }
            NodeKind::Index => self.index_type(node.id, depth),
            NodeKind::FunctionCall => {
                if self.graph.library_call1027(node.id).is_some(){return pin.value_type.clone();}
                let Some(PropertyValue::String(function)) = node.properties.get("function") else {
                    return ValueType::Any;
                };
                let types = self.sequence_types(node.id, depth);
                match function.as_str() {
                    "dict_at" | "dict_get" => {
                        let result = self.dictionary_item(node.id, 0, 1, depth);
                        if function == "dict_get" {
                            common_type([result, types.get(2).cloned().unwrap_or(ValueType::Any)])
                        } else {
                            result
                        }
                    }
                    _ => builtin_type(function, &types),
                }
            }
            _ => pin.value_type.clone(),
        }
    }

    fn input_value(&mut self, node: NodeId, key: &str, depth: usize) -> ValueType {
        let Some(pin) = self.graph.pin_by_key(node, key) else {
            return ValueType::Any;
        };
        match self.incoming.get(&pin.id) {
            Some(Some(source)) => self.pin(*source, depth),
            Some(None) => ValueType::Any,
            None => self.default_type(pin.id, depth),
        }
    }

    fn default_type(&mut self, id: PinId, depth: usize) -> ValueType {
        let Some(pin) = self.graph.pins.get(&id) else {
            return ValueType::Any;
        };
        if let Some(value) = &pin.default_value {
            return crate::blueprint_policy::authored_default_type(self.graph, pin, value);
        }
        self.operand_context(pin.node, depth)
            .unwrap_or(ValueType::Any)
    }

    fn default_value(&mut self, id: PinId, depth: usize) -> Option<PropertyValue> {
        let pin = self.graph.pins.get(&id)?;
        if pin.direction != PinDirection::Input || pin.value_type.is_execution() {
            return None;
        }
        if let Some(value) = &pin.default_value {
            return Some(value.clone());
        }
        // A connected unknown is not a vacant operand. Never back-infer its
        // producer from another operand or from a typed downstream consumer.
        if self.incoming.contains_key(&id) {
            return None;
        }
        let ty = self.operand_context(pin.node, depth)?;
        match ty {
            ValueType::Int => Some(PropertyValue::Int(0)),
            ValueType::Float => Some(PropertyValue::Float(0.0)),
            ValueType::String | ValueType::InterpolatedText => {
                Some(PropertyValue::String(String::new()))
            }
            ValueType::Bool => Some(PropertyValue::Bool(false)),
            _ => None,
        }
    }

    fn operand_context(&mut self, node: NodeId, depth: usize) -> Option<ValueType> {
        if let Some(context) = self.operand_contexts.get(&node) {
            return context.clone();
        }
        let model = self.graph.nodes.get(&node)?;
        let domain = operand_domain(model)?;
        if depth > 128 || self.work >= 100_000 {
            return None;
        }
        if !self.active_operand_contexts.insert(node) {
            return None;
        }
        self.work += 1;
        let inputs: Vec<_> = model
            .pins
            .iter()
            .filter_map(|id| self.graph.pins.get(id))
            .filter(|pin| pin.direction == PinDirection::Input && !pin.value_type.is_execution())
            .map(|pin| pin.id)
            .collect();
        let mut evidence = Vec::new();
        let mut ambiguous = false;
        for input in inputs {
            match self.incoming.get(&input) {
                Some(Some(source)) => evidence.push(self.pin(*source, depth + 1)),
                Some(None) => {
                    ambiguous = true;
                    break;
                }
                None => {
                    if let Some(value) = &self.graph.pins[&input].default_value {
                        evidence.push(crate::blueprint_policy::authored_default_type(
                            self.graph,
                            &self.graph.pins[&input],
                            value,
                        ));
                    }
                }
            }
        }
        let context = if ambiguous {
            None
        } else {
            operand_context_type(domain, &evidence)
        };
        self.active_operand_contexts.remove(&node);
        self.operand_contexts.insert(node, context.clone());
        context
    }

    fn sequence_types(&mut self, node: NodeId, depth: usize) -> Vec<ValueType> {
        let Some(owner) = self.graph.nodes.get(&node) else {
            return Vec::new();
        };
        if let Some(PropertyValue::Int(count)) = owner.properties.get("input_count") {
            return (0..(*count).clamp(0, 128))
                .map(|index| self.input_value(node, &format!("item_{index}"), depth))
                .collect();
        }
        match owner
            .properties
            .get(if owner.kind == NodeKind::FunctionCall {
                "args"
            } else {
                "items"
            }) {
            Some(PropertyValue::StringList(values)) => {
                vec![ValueType::String; values.len().min(128)]
            }
            _ => Vec::new(),
        }
    }

    fn input_producer(&self, node: NodeId, key: &str) -> Option<NodeId> {
        let pin = self.graph.pin_by_key(node, key)?;
        self.incoming
            .get(&pin.id)?
            .and_then(|id| self.graph.pins.get(&id).map(|pin| pin.node))
    }

    fn string_input(&self, node: NodeId, key: &str) -> Option<String> {
        let pin = self.graph.pin_by_key(node, key)?;
        if let Some(Some(output)) = self.incoming.get(&pin.id) {
            let producer = self.graph.nodes.get(&self.graph.pins.get(output)?.node)?;
            return match (producer.kind, producer.properties.get("value")) {
                (NodeKind::Literal | NodeKind::TextValue, Some(PropertyValue::String(text))) => {
                    Some(text.clone())
                }
                _ => None,
            };
        }
        match &pin.default_value {
            Some(PropertyValue::String(text)) => Some(text.clone()),
            _ => None,
        }
    }

    fn index_type(&mut self, node: NodeId, depth: usize) -> ValueType {
        let target = self.input_value(node, "target", depth);
        let index = self.input_value(node, "index", depth);
        match (target, index) {
            (ValueType::List(inner), ValueType::Int) => *inner,
            (ValueType::String | ValueType::InterpolatedText, ValueType::Int) => ValueType::String,
            (_, ValueType::String | ValueType::InterpolatedText) => {
                let key = self.string_input(node, "index");
                self.input_producer(node, "target")
                    .map(|target| self.dictionary_value(target, key.as_deref(), depth))
                    .unwrap_or(ValueType::Any)
            }
            _ => ValueType::Any,
        }
    }

    fn dictionary_item(
        &mut self,
        node: NodeId,
        dict: usize,
        key: usize,
        depth: usize,
    ) -> ValueType {
        let key = self.string_input(node, &format!("item_{key}"));
        self.input_producer(node, &format!("item_{dict}"))
            .map(|target| self.dictionary_value(target, key.as_deref(), depth))
            .unwrap_or(ValueType::Any)
    }

    fn dictionary_value(&mut self, node: NodeId, key: Option<&str>, depth: usize) -> ValueType {
        if depth > 128 || self.work >= 100_000 {
            return ValueType::Any;
        }
        self.work += 1;
        let Some(owner) = self.graph.nodes.get(&node) else {
            return ValueType::Any;
        };
        if owner.kind == NodeKind::Reroute {
            return self
                .input_producer(node, "value")
                .map(|node| self.dictionary_value(node, key, depth + 1))
                .unwrap_or(ValueType::Any);
        }
        if owner.kind != NodeKind::FunctionCall
            || owner.properties.get("function") != Some(&PropertyValue::String("dict".into()))
        {
            return ValueType::Any;
        }
        let Some(PropertyValue::Int(count)) = owner.properties.get("input_count") else {
            return ValueType::Any;
        };
        if *count < 0 || *count > 128 || count % 2 != 0 {
            return ValueType::Any;
        }
        let mut values = Vec::new();
        let mut keys = BTreeSet::new();
        for index in (0..*count).step_by(2) {
            let Some(name) = self.string_input(node, &format!("item_{index}")) else {
                return ValueType::Any;
            };
            if !keys.insert(name.clone()) {
                return ValueType::Any;
            }
            let value = self.input_value(node, &format!("item_{}", index + 1), depth + 1);
            if key == Some(name.as_str()) {
                values.push((true, value));
            } else {
                values.push((false, value));
            }
        }
        if key.is_some() {
            values
                .into_iter()
                .find(|(matches, _)| *matches)
                .map(|(_, value)| value)
                .unwrap_or(ValueType::Any)
        } else {
            common_type(values.into_iter().map(|(_, value)| value))
        }
    }
}

#[derive(Clone, Copy)]
enum OperandDomain {
    Add,
    Numeric,
    Equality,
    Ordered,
    BlueprintEquality,
    Append,
    Boolean,
}

fn operand_domain(node: &GraphNode) -> Option<OperandDomain> {
    use NodeKind as N;
    use OperandDomain as D;
    if let Some(domain) = crate::blueprint_policy::blueprint_operand_domain(node) {
        use crate::blueprint_policy::BlueprintOperandDomain as B;
        return Some(match domain {
            B::Numeric => D::Numeric,
            B::Equality => D::BlueprintEquality,
            B::Append => D::Append,
            B::Boolean => D::Boolean,
        });
    }
    match node.kind {
        N::MathAdd => Some(D::Add),
        N::MathSubtract | N::MathMultiply | N::MathDivide | N::MathNegate => Some(D::Numeric),
        N::MathEqual | N::MathNotEqual => Some(D::Equality),
        N::MathLess | N::MathLessEqual | N::MathGreater | N::MathGreaterEqual => Some(D::Ordered),
        N::BinaryOperator => match node
            .properties
            .get("operator")
            .or_else(|| node.properties.get("op"))
        {
            Some(PropertyValue::String(op)) => match op.as_str() {
                "+" => Some(D::Add),
                "-" | "*" | "/" => Some(D::Numeric),
                "==" | "!=" => Some(D::Equality),
                "<" | "<=" | ">" | ">=" => Some(D::Ordered),
                _ => None,
            },
            _ => None,
        },
        N::UnaryOperator
            if node
                .properties
                .get("operator")
                .or_else(|| node.properties.get("op"))
                == Some(&PropertyValue::String("-".into())) =>
        {
            Some(D::Numeric)
        }
        _ => None,
    }
}

/// Empty strings on these operands are real string values, not the catalog's
/// generic "please provide an expression" placeholder used on other nodes.
pub(crate) fn allows_empty_string_operand(node: &GraphNode) -> bool {
    matches!(
        operand_domain(node),
        Some(
            OperandDomain::Add
                | OperandDomain::Equality
                | OperandDomain::Ordered
                | OperandDomain::BlueprintEquality
                | OperandDomain::Append
        )
    )
}

pub(crate) fn has_adaptive_operand_defaults(node: &GraphNode) -> bool {
    operand_domain(node).is_some()
}

fn operand_context_type(domain: OperandDomain, evidence: &[ValueType]) -> Option<ValueType> {
    use ValueType as T;
    if matches!(domain, OperandDomain::Append) {
        return Some(T::String);
    }
    if matches!(domain, OperandDomain::Boolean) {
        return Some(T::Bool);
    }
    let known: Vec<_> = evidence.iter().filter(|ty| !matches!(ty, T::Any)).collect();
    if known.is_empty() {
        return None;
    }
    if matches!(domain, OperandDomain::Equality | OperandDomain::BlueprintEquality) {
        if let ValueType::Enum(asset) = known[0] {
            if known.iter().all(|ty| **ty == ValueType::Enum(*asset)) { return Some(ValueType::Enum(*asset)); }
        }
    }
    if matches!(domain, OperandDomain::Add)
        && known
            .iter()
            .any(|ty| matches!(ty, T::String | T::InterpolatedText))
    {
        return Some(T::String);
    }
    if known.iter().all(|ty| matches!(ty, T::Int | T::Float)) {
        return Some(if known.iter().any(|ty| matches!(ty, T::Float)) {
            T::Float
        } else {
            T::Int
        });
    }
    if matches!(domain, OperandDomain::Equality | OperandDomain::Ordered)
        && known
            .iter()
            .all(|ty| matches!(ty, T::String | T::InterpolatedText))
    {
        return Some(T::String);
    }
    if matches!(domain, OperandDomain::BlueprintEquality) {
        if known.iter().all(|ty| matches!(ty, T::String)) {
            return Some(T::String);
        }
        if known.iter().all(|ty| matches!(ty, T::InterpolatedText)) {
            return Some(T::InterpolatedText);
        }
    }
    if matches!(
        domain,
        OperandDomain::Equality | OperandDomain::BlueprintEquality
    ) && known.iter().all(|ty| matches!(ty, T::Bool))
    {
        return Some(T::Bool);
    }
    None
}

pub(crate) fn property_type(value: &PropertyValue) -> ValueType {
    match value {
        PropertyValue::Bool(_) => ValueType::Bool,
        PropertyValue::Int(_) => ValueType::Int,
        PropertyValue::Float(_) => ValueType::Float,
        PropertyValue::String(_) => ValueType::String,
        PropertyValue::Name(_) => ValueType::Name,
        PropertyValue::Enum(value) => ValueType::Enum(value.asset),
        PropertyValue::StringList(items) => ValueType::List(Box::new(if items.is_empty() {
            ValueType::Any
        } else {
            ValueType::String
        })),
    }
}

fn common_type(types: impl IntoIterator<Item = ValueType>) -> ValueType {
    let mut types = types.into_iter();
    let Some(first) = types.next() else {
        return ValueType::Any;
    };
    if types.all(|ty| ty == first) {
        first
    } else {
        ValueType::Any
    }
}

fn numeric_type(ty: ValueType) -> ValueType {
    if matches!(ty, ValueType::Int | ValueType::Float) {
        ty
    } else {
        ValueType::Any
    }
}

fn arithmetic_type(add: bool, left: &ValueType, right: &ValueType) -> ValueType {
    use ValueType as T;
    if matches!(left, T::Name | T::Enum(_)) || matches!(right, T::Name | T::Enum(_)) { return T::Any; }
    if add
        && (matches!(left, T::String | T::InterpolatedText)
            || matches!(right, T::String | T::InterpolatedText))
    {
        return T::String;
    }
    match (left, right) {
        (T::Int, T::Int) => T::Int,
        (T::Int | T::Float, T::Int | T::Float) => T::Float,
        _ => T::Any,
    }
}

fn builtin_type(name: &str, args: &[ValueType]) -> ValueType {
    use ValueType as T;
    match name {
        "__rvn_widget_class_v1" | "__rvn_widget_class_none_v1" | "__rvn_widget_object_class_v1" => T::WidgetClass,
        "__rvn_widget_create_v1" => T::WidgetObject,
        "__rvn_literal_name" | "__rvn_string_to_name" | "__rvn_string_table_id_name" | "__rvn_text_table_id" => T::Name,
        "__rvn_text_table_valid" => T::Bool,
        "__rvn_text_table_key" => T::String,
        "__rvn_string_table_text_name" => T::InterpolatedText,
        "__rvn_string_table_registered_name" | "__rvn_string_table_entry_registered_name" => T::Bool,
        "__rvn_string_table_namespace_name" | "__rvn_string_table_source_name" | "__rvn_string_table_metadata_name" => T::String,
        "__rvn_registered_string_tables_name" | "__rvn_string_table_metadata_ids_name" => T::List(Box::new(T::Name)),
        "__rvn_string_table_keys_name" => T::List(Box::new(T::String)),
        "__rvn_name_to_string" => T::String,
        "len" | "floor" | "ceil" | "to_int" | "__rvn_bool_to_int" | "random" | "rand" => T::Int,
        "mouse_x" | "mouse_y" => T::Float,
        "__rvn_string_table_registered" | "__rvn_string_table_entry_registered" => T::Bool,
        "__rvn_string_table_id" | "__rvn_string_table_namespace" | "__rvn_string_table_source" | "__rvn_string_table_metadata" => T::String,
        "__rvn_string_table_asset" => T::Asset(crate::AssetKind::StringTable),
        "__rvn_registered_string_tables" | "__rvn_string_table_keys" | "__rvn_string_table_metadata_ids" => T::List(Box::new(T::String)),
        "contains" | "key_pressed" | "mouse_clicked" | "__rvn_int_to_bool" => T::Bool,
        "upper" | "lower" | "capitalize" | "trim" | "replace" | "substring" | "make_color"
        | "make_color_rgb" | "text_to_string" => T::String,
        "string_to_text" | "__rvn_localizable_text" | "__rvn_invariant_text" | "__rvn_empty_text" | "__rvn_format_text" | "__rvn_string_table_text" => T::InterpolatedText,
        "split" | "dict_keys" => T::List(Box::new(T::String)),
        "abs" => numeric_type(args.first().cloned().unwrap_or(T::Any)),
        // min/max return Int when their chosen numeric value is integral, even
        // for Float inputs. No concrete promise is safe without evaluating it.
        "min" | "max" => {
            if !args.is_empty() && args.iter().all(|ty| *ty == T::Int) {
                T::Int
            } else {
                T::Any
            }
        }
        "list_remove" | "list_slice" | "__rvn_iterable" => match args.first() {
            Some(T::List(inner)) => T::List(inner.clone()),
            _ => T::Any,
        },
        _ => T::Any,
    }
}

pub(crate) fn expression_type(
    expr: &Expr,
    variables: &BTreeMap<String, VariableDefinition>,
) -> ValueType {
    expression_type_with_resolver(expr, variables, &|_| None)
}

/// Pure source analysis. The resolver may supply a type for an expression
/// whose genuine source producer has already been inspected. It never supplies
/// a value, declaration, execution result or publication capability.
pub(crate) fn expression_type_with_resolver(
    expr: &Expr,
    variables: &BTreeMap<String, VariableDefinition>,
    resolver: &impl Fn(&Expr) -> Option<ValueType>,
) -> ValueType {
    fn infer(expr: &Expr, vars: &BTreeMap<String, VariableDefinition>, depth: usize,
        resolver: &impl Fn(&Expr) -> Option<ValueType>) -> ValueType {
        if depth > 128 {
            return ValueType::Any;
        }
        if let Some(ty) = resolver(expr) { return ty; }
        if let Ok(Some(value)) = crate::enum_graph1026::literal_expression1026(expr) { return ValueType::Enum(value.asset); }
        match expr {
            Expr::LibraryCall{..}=>ValueType::Any, // no type claim without the original qualified signature resolver
            Expr::StandardMacroData{output,..}=>match output {
                rvn_parser::scheduled_source::standard_macro_contract::DataOutput::Counter=>ValueType::Int,
                rvn_parser::scheduled_source::standard_macro_contract::DataOutput::IsA=>ValueType::Bool,
            },
            Expr::Int(_) => ValueType::Int,
            Expr::Float(_) => ValueType::Float,
            Expr::Bool(_) => ValueType::Bool,
            Expr::Str(_) => ValueType::String,
            Expr::Var(name) => vars
                .get(name)
                .map(|var| var.value_type.clone())
                .unwrap_or(ValueType::Any),
            Expr::ListLit(items) => ValueType::List(Box::new(common_type(
                items.iter().map(|item| infer(item, vars, depth + 1, resolver)),
            ))),
            Expr::And(_, _) | Expr::Or(_, _) | Expr::Not(_) => ValueType::Bool,
            Expr::Neg(value) => numeric_type(infer(value, vars, depth + 1, resolver)),
            Expr::BinOp { op, left, right } => match op {
                BinOpKind::Eq
                | BinOpKind::Ne
                | BinOpKind::Lt
                | BinOpKind::Le
                | BinOpKind::Gt
                | BinOpKind::Ge => ValueType::Bool,
                _ => arithmetic_type(
                    *op == BinOpKind::Add,
                    &infer(left, vars, depth + 1, resolver),
                    &infer(right, vars, depth + 1, resolver),
                ),
            },
            Expr::Index { target, index } => match (
                infer(target, vars, depth + 1, resolver),
                infer(index, vars, depth + 1, resolver),
            ) {
                (ValueType::List(inner), ValueType::Int) => *inner,
                (ValueType::String | ValueType::InterpolatedText, ValueType::Int) => {
                    ValueType::String
                }
                (_, ValueType::String | ValueType::InterpolatedText) => {
                    if let (Expr::Call { name, args }, Expr::Str(key)) = (&**target, &**index) {
                        if name == "dict" && args.len() % 2 == 0 {
                            let mut seen = BTreeSet::new();
                            let mut result = ValueType::Any;
                            for pair in args.chunks_exact(2) {
                                let Expr::Str(name) = &pair[0] else {
                                    return ValueType::Any;
                                };
                                if !seen.insert(name) {
                                    return ValueType::Any;
                                }
                                if name == key {
                                    result = infer(&pair[1], vars, depth + 1, resolver);
                                }
                            }
                            return result;
                        }
                    }
                    ValueType::Any
                }
                _ => ValueType::Any,
            },
            Expr::Call { name, args } => builtin_type(
                name,
                &args
                    .iter()
                    .map(|arg| infer(arg, vars, depth + 1, resolver))
                    .collect::<Vec<_>>(),
            ),
        }
    }
    infer(expr, variables, 0, resolver)
}
