//! Child of the existing source importer. It reconstructs real catalogue
//! nodes/wires and reuses the complete existing identity reconciliation.
use super::*;
use crate::native_frame_preparation::NativeNodeIdentity;
use rvn_parser::scheduled_source::standard_macro_contract::{Control, DataOutput, Dispatch, Site};
#[derive(Default)]
pub(super) struct MacroDefinition {
    operand: Option<PropertyValue>,
    deferred: Option<(Expr, Vec<rvn_parser::ConsumerStep>)>,
    branches: Option<Vec<rvn_parser::scheduled_source::standard_macro_contract::Branch>>,
}
fn identity(
    endpoint: &rvn_parser::scheduled_source::DeclaredEndpoint,
) -> Result<NativeNodeIdentity, String> {
    let kind: NodeKind = serde_json::from_value(serde_json::Value::String(endpoint.kind.clone()))
        .map_err(|e| e.to_string())?;
    Ok(NativeNodeIdentity {
        node: NodeId::new(endpoint.node),
        kind,
        ports: endpoint
            .ports
            .iter()
            .map(|p| {
                (
                    p.role.clone(),
                    PinId::new(p.pin),
                    p.incoming.map(EdgeId::new),
                )
            })
            .collect(),
        static_inputs: Default::default(),
        dynamic_inputs: endpoint
            .ports
            .iter()
            .filter_map(|p| p.source_pin.map(|id| (p.role.clone(), PinId::new(id))))
            .collect(),
        legacy_string_sequence: false,
        exec_links_v1: endpoint.exec_links_v1,
        exec_inputs: endpoint
            .ports
            .iter()
            .filter_map(|p| p.exec_incoming.clone().map(|links| (p.role.clone(), links)))
            .collect(),
    })
}
impl Builder<'_> {
    fn macro_node(&mut self, site: &Site, pos: [f64; 2]) -> Result<NodeId, String> {
        site.validate()?;
        if !matches!(self.graph.kind, GraphKind::Handler { .. }) {
            return Err("Macro source is not an actual Handler graph".into());
        }
        let scope = site.scope_source();
        if self
            .native_scope
            .as_ref()
            .is_some_and(|prior| prior != &scope)
        {
            return Err("Macro source root differs inside one actual Handler".into());
        }
        let root = self
            .graph
            .nodes
            .values()
            .find(|node| node.kind == NodeKind::HandlerEntry)
            .ok_or("Actual Handler entry missing")?
            .id;
        let root_identity = identity(&site.root)?;
        if let Some(prior) = self.native_nodes.get(&root) {
            if prior != &root_identity {
                return Err("Macro and ordinary source disagree on actual root pins".into());
            }
        } else {
            self.native_nodes.insert(root, root_identity);
        }
        self.graph.graph_id = GraphId::new(site.node.graph);
        self.graph.enable_native_frame_preparation()?;
        if site.root.exec_links_v1 {
            self.graph
                .nodes
                .get_mut(&root)
                .unwrap()
                .properties
                .insert("@source_exec_links_v1".into(), PropertyValue::Bool(true));
        }
        self.native_scope = Some(scope);
        let declared = identity(&site.node)?;
        if let Some((node, prior)) = self
            .native_nodes
            .iter()
            .find(|(_, prior)| prior.node == declared.node)
        {
            if prior != &declared
                || crate::standard_macros_1020::kind(self.graph.nodes[node].kind).is_none()
            {
                return Err(
                    "Macro expansion collides with a different actual node/declaration".into(),
                );
            }
            return Ok(*node);
        }
        let node = self.node(declared.kind, pos)?;
        self.native_nodes.insert(node, declared);
        Ok(node)
    }
    pub(super) fn macro_dispatch(
        &mut self,
        dispatch: &Dispatch,
        prev: (NodeId, String),
        x: f64,
        y: f64,
    ) -> Result<f64, String> {
        dispatch.validate()?;
        let node = self.macro_node(&dispatch.site, [x, y])?;
        let family = dispatch.site.kind()?;
        let control = family.control_role(dispatch.control)?;
        self.connect(prev.0, &prev.1, node, control)?;
        if let Some(value) = &dispatch.operand {
            let key = if family == rvn_parser::scheduled_source::standard_macro_contract::Kind::DoN
            {
                "n"
            } else {
                "start_closed"
            };
            let declared = dispatch
                .site
                .node
                .ports
                .iter()
                .find(|port| port.role == key)
                .unwrap();
            if let Some(plan) = &dispatch.deferred_operand_v1 {
                let candidate = (value.clone(), plan.steps.clone());
                if self.macro_definitions.get(&node).and_then(|d| d.deferred.as_ref()).is_some_and(|prior| prior != &candidate)
                    || self.macro_definitions.get(&node).is_some_and(|d| d.operand.is_some()) {
                    return Err("Repeated macro expansion changes its deferred operand".into());
                }
                let already_defined = self.macro_definitions.get(&node).and_then(|d|d.deferred.as_ref()).is_some();
                let prior = std::mem::take(&mut self.native_active);
                let result = (|| {
                    // Every expansion still validates the exact producer plan.
                    // The one declared macro input wire was constructed on its
                    // first definition; Enter/Reset do not create another edge.
                    self.native_operand_steps(&plan.steps,x,y)?;
                    if !already_defined { self.input_expr(node,key,value)?; }
                    Ok::<(),String>(())
                })();
                self.native_active = prior;
                result?;
                self.macro_definitions.entry(node).or_default().deferred = Some(candidate);
            } else {
            if declared.incoming.is_some() || declared.source_pin.is_some()
                || self.macro_definitions.get(&node).is_some_and(|d| d.deferred.is_some()) {
                return Err("Connected macro operand requires its deferred source plan".into());
            }
            let property = match value {
                Expr::Bool(value) if key == "start_closed" => PropertyValue::Bool(*value),
                value if key == "n" => PropertyValue::Int(i64::from(
                    rvn_parser::scheduled_source::standard_macro_contract::literal_int32(value)?,
                )),
                _ => {
                    return Err(
                        "Unconnected macro operand must retain its actual typed pin default".into(),
                    )
                }
            };
            let definition = self.macro_definitions.entry(node).or_default();
            if definition
                .operand
                .as_ref()
                .is_some_and(|prior| prior != &property)
            {
                return Err("Repeated macro expansion changes its actual pin default".into());
            }
            definition.operand = Some(property.clone());
            self.graph
                .set_pin_default(node, key, property)
                .map_err(err)?;
            }
        }
        if dispatch.control == Control::Enter {
            if let Some(prior) = self
                .macro_definitions
                .get(&node)
                .and_then(|definition| definition.branches.as_ref())
            {
                if prior != &dispatch.branches {
                    return Err("Repeated actual macro node changed its execution outputs".into());
                }
            } else {
                self.macro_definitions.entry(node).or_default().branches =
                    Some(dispatch.branches.clone());
                for branch in &dispatch.branches {
                    let role = dispatch
                        .site
                        .node
                        .ports
                        .iter()
                        .find(|port| port.pin == branch.pin)
                        .ok_or("Macro output pin missing")?
                        .role
                        .clone();
                    self.lane += 960.0;
                    self.sequence_impl(
                        &branch.body,
                        (node, role),
                        x + 960.0,
                        self.lane,
                        Some(node),
                        false,
                    )?;
                }
            }
        }
        Ok(x + 960.0)
    }
    pub(super) fn macro_data(
        &mut self,
        site: &Site,
        output: DataOutput,
        pin: u64,
        pos: [f64; 2],
    ) -> Result<ExpressionRef, String> {
        if site.data_pin(output)? != pin {
            return Err("Macro live read changed its actual output pin".into());
        }
        let node = self.macro_node(site, pos)?;
        let role = site
            .node
            .ports
            .iter()
            .find(|port| port.pin == pin)
            .ok_or("Macro data pin absent")?
            .role
            .clone();
        Ok(ExpressionRef {
            node,
            output: self
                .graph
                .pin_by_key(node, &role)
                .ok_or("Actual macro catalogue data output missing")?
                .id,
        })
    }
}
