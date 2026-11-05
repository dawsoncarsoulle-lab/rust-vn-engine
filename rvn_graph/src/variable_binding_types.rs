//! Source-derived narrative binding types. These are presentation/validation
//! caches like the variable registry; executable values still come from RVN.
//! A global read before Local must not use the local's same-name registry row.
use crate::{GraphDocument, NodeId, NodeKind, PinId, PropertyValue, ValueType,
    VariableBindingContext};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const NODE_CACHE: &str = "inferred_narrative_binding_types";
const GLOBAL_CACHE: &str = "inferred_narrative_global_types";

#[derive(Serialize, Deserialize)]
struct BindingTypes {
    name: String,
    pins: BTreeMap<String, ValueType>,
    // None exists only while the private source importer constructs a graph.
    // Returned documents are sealed against their type-relevant structure.
    stamp: Option<u64>,
}

impl GraphDocument {
    /// Re-derive narrative consumer bindings from this graph's lexical scopes
    /// and its source global definitions. Opening a graph must not replace a
    /// global consumer with the same-name local registry slot. A caller that
    /// also repairs legacy sockets must refresh again after those repairs.
    pub fn refresh_variable_usage_bindings(&mut self) -> Result<(), String> {
        if self.narrative.is_none() { return Ok(()); }
        let globals = self.source_global_binding_types()?;
        self.refresh_narrative_binding_types(&globals)
    }

    /// A source binding overrides the name registry, including an unresolved
    /// global/local consumer. Keep its authored sockets during registry-based
    /// legacy hydration; the refreshed binding carries the semantic type.
    pub fn variable_usage_has_source_type_binding(&self, node: NodeId) -> bool {
        self.narrative.is_some() && self.nodes.get(&node).is_some_and(|node|
            matches!(node.kind, NodeKind::VariableGet | NodeKind::SetVariable | NodeKind::LocalVariable)
                && node.properties.contains_key(NODE_CACHE))
    }

    pub(crate) fn source_binding_pin_type_with_stamp(&self, pin: PinId, stamp: u64) -> Option<ValueType> {
        if self.narrative.is_none() { return None; }
        let pin = self.pins.get(&pin)?;
        let node = self.nodes.get(&pin.node)?;
        let stored = node.properties.get(NODE_CACHE)?;
        if !matches!(node.kind, NodeKind::VariableGet | NodeKind::SetVariable | NodeKind::LocalVariable)
            || !matches!(pin.key.as_str(), "value" | "value_out") { return None; }
        let PropertyValue::String(encoded) = stored else { return Some(ValueType::Any); };
        let Ok(cache) = serde_json::from_str::<BindingTypes>(encoded) else { return Some(ValueType::Any); };
        if self.variable_node_name(node.id).as_deref() != Some(cache.name.as_str()) { return Some(ValueType::Any); }
        if cache.stamp.is_some_and(|expected|expected!=stamp) && cache.pins.contains_key(&pin.key) {
            // Topology or a type changed. An old source cache proves neither
            // domain now; project compile/reimport must refresh it first.
            return Some(ValueType::Any);
        }
        cache.pins.get(&pin.key).cloned()
    }

    pub(crate) fn source_binding_stamp(&self) -> u64 {
        if self.narrative.is_none() || !self.nodes.values().any(|node|node.properties.contains_key(NODE_CACHE)) {
            return 0;
        }
        // Scalar contents do not change the lexical binding domain. Including
        // them would rewrite unrelated cache properties on every value edit
        // and break presentation identity across save/recovery. Keep their
        // full primitive type, topology, names and all non-scalar contents.
        let scalar_type = |value: &PropertyValue| match value {
            PropertyValue::Bool(_) => PropertyValue::Bool(false),
            PropertyValue::Int(_) => PropertyValue::Int(0),
            PropertyValue::Float(_) => PropertyValue::Float(0.0),
            PropertyValue::Name(_) => PropertyValue::Name(crate::NameDefault::default()),
            other => other.clone(),
        };
        let nodes:Vec<_>=self.nodes.values().map(|node| (node.id,node.kind,
            node.properties.iter().filter(|(key,_)|key.as_str()!=NODE_CACHE)
                .map(|(key,value)|(key,if node.kind==NodeKind::Literal && key=="value" {
                    scalar_type(value)
                } else {value.clone()})).collect::<Vec<_>>())).collect();
        let pins:Vec<_>=self.pins.values().map(|pin|(pin.id,pin.node,&pin.key,pin.direction,
            &pin.value_type,pin.cardinality,pin.default_value.as_ref().map(scalar_type))).collect();
        let variables:Vec<_>=self.variables.iter().map(|(name,var)|(name,&var.name,&var.value_type)).collect();
        let encoded=serde_json::to_vec(&(self.kind.clone(),self.narrative,nodes,pins,&self.edges,variables))
            .expect("Graph binding stamp is serializable");
        encoded.into_iter().fold(0xcbf29ce484222325u64,|hash,byte|(hash^u64::from(byte)).wrapping_mul(0x100000001b3))
    }

    fn seal_source_binding_types(&mut self) -> Result<(), String> {
        let stamp=self.source_binding_stamp();
        for node in self.nodes.values_mut() {
            if let Some(PropertyValue::String(encoded))=node.properties.get_mut(NODE_CACHE) {
                let mut cache:BindingTypes=serde_json::from_str(encoded).map_err(|e|e.to_string())?;
                cache.stamp=Some(stamp);
                *encoded=serde_json::to_string(&cache).map_err(|e|e.to_string())?;
            }
        }
        Ok(())
    }

    pub(crate) fn cache_source_binding_types(&mut self, node: NodeId, name: &str,
        pins: BTreeMap<String, ValueType>) -> Result<(), String>
    {
        let owner = self.nodes.get_mut(&node).ok_or("Missing binding type owner")?;
        if pins.is_empty() { owner.properties.remove(NODE_CACHE); }
        else {
            owner.properties.insert(NODE_CACHE.into(), PropertyValue::String(
                serde_json::to_string(&BindingTypes {name:name.into(),pins,stamp:None}).map_err(|e|e.to_string())?));
        }
        Ok(())
    }

    pub(crate) fn source_global_binding_types(&self) -> Result<BTreeMap<String, ValueType>, String> {
        let Some(PropertyValue::String(encoded)) = self.nodes.values()
            .find_map(|node|node.properties.get(GLOBAL_CACHE)) else { return Ok(BTreeMap::new()); };
        serde_json::from_str(encoded).map_err(|e|format!("Invalid source binding type cache: {e}"))
    }

    pub(crate) fn cache_source_global_types(&mut self, root: NodeId,
        globals: &BTreeMap<String, ValueType>) -> Result<(), String>
    {
        if self.narrative.is_some() {
            let root=self.nodes.get_mut(&root).ok_or("Missing narrative entry")?;
            if globals.is_empty() { root.properties.remove(GLOBAL_CACHE); }
            else { root.properties.insert(GLOBAL_CACHE.into(),
                PropertyValue::String(serde_json::to_string(globals).map_err(|e|e.to_string())?)); }
        }
        Ok(())
    }

    /// Refresh against genuine graph consumers and source global definitions.
    /// Globals and locals can have different full ValueTypes under one name.
    /// An ambiguous inherited fragment stays Any unless both domains prove the
    /// same type. Missing/unreachable consumers acquire no global identity.
    pub(crate) fn refresh_narrative_binding_types(&mut self,
        globals: &BTreeMap<String, ValueType>) -> Result<(), String>
    {
        self.refresh_narrative_binding_types_context(globals, None, None)
    }

    pub(crate) fn refresh_narrative_binding_types_in_project(&mut self,
        globals: &BTreeMap<String, ValueType>, project: &[GraphDocument]) -> Result<(), String>
    {
        self.refresh_narrative_binding_types_context(globals, Some(project), None)
    }

    pub(crate) fn refresh_narrative_binding_types_library1027(&mut self,globals:&BTreeMap<String,ValueType>,project:&[GraphDocument],resources:&crate::LibraryGraphResources1027<'_>)->Result<(),String>{
        self.refresh_narrative_binding_types_context(globals,Some(project),Some(resources))
    }

    fn refresh_narrative_binding_types_context(&mut self,
        globals: &BTreeMap<String, ValueType>, project: Option<&[GraphDocument]>,library:Option<&crate::LibraryGraphResources1027<'_>>) -> Result<(), String>
    {
        if self.narrative.is_none() {
            for node in self.nodes.values_mut() {
                node.properties.remove(NODE_CACHE);
                node.properties.remove(GLOBAL_CACHE);
            }
            return Ok(());
        }
        let names: std::collections::BTreeSet<_> = self.nodes.values()
            .filter_map(|node|self.variable_node_name(node.id)).collect();
        for name in names {
            let projection=if let Some(resources)=library {
                self.variable_binding_projection_library1027(&name,project.ok_or("Original scoped binding project is absent")?,resources)
            }else{match project {
                Some(project) => self.variable_binding_projection_in_project(&name, project),
                None => self.variable_binding_projection(&name),
            }}.map_err(|e|e.to_string())?;
            let local=self.variables.get(&name).map(|v|v.value_type.clone()).unwrap_or(ValueType::Any);
            let global=globals.get(&name).cloned().unwrap_or(ValueType::Any);
            let has_local=!projection.local_declarations.is_empty();
            let projected=|context|match context {
                VariableBindingContext::ProjectGlobal=>(has_local && global!=local).then(||global.clone()),
                VariableBindingContext::ActivationLocal=>None,
                VariableBindingContext::GlobalOrLocal=>if has_local && global==local {None} else {Some(ValueType::Any)},
            };
            let nodes:Vec<_>=self.nodes.values().filter(|node|matches!(node.kind,
                NodeKind::VariableGet|NodeKind::SetVariable|NodeKind::LocalVariable)
                &&self.variable_node_name(node.id).as_deref()==Some(name.as_str())).map(|node|node.id).collect();
            for node in nodes {
                let mut pins=BTreeMap::new();
                for id in &self.nodes[&node].pins {
                    let pin=&self.pins[id];
                    let context=if pin.key=="value" && self.nodes[&node].kind!=NodeKind::VariableGet {
                        projection.write_contexts.get(&node)
                    } else if matches!(pin.key.as_str(),"value"|"value_out") {
                        projection.read_contexts.get(id)
                    } else {None};
                    if let Some(ty)=context.and_then(|context|projected(*context)) {pins.insert(pin.key.clone(),ty);}
                }
                self.cache_source_binding_types(node,&name,pins)?;
            }
        }
        if let Some(root)=self.nodes.values().find(|node|node.kind==NodeKind::Label).map(|node|node.id) {
            self.cache_source_global_types(root,globals)?;
        }
        self.seal_source_binding_types()?;
        Ok(())
    }
}
