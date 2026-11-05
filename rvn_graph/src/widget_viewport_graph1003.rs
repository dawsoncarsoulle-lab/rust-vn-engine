//! Typed object viewport effects on the existing Native Scene graph.
use crate::{GraphDocument, GraphNode, NodeKind, PinDirection, PropertyValue, ValueType, NodeId};
pub fn is_viewport_kind(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::WidgetAddToViewport | NodeKind::WidgetRemoveFromParent)
}
impl GraphDocument {
    /// Compatibility query for the Scene Add/Remove family. Handler Remove
    /// uses the kind-aware query below; this does not enable Handler Add.
    pub fn widget_viewport_authoring_context(&self) -> Result<(), String> {
        self.widget_viewport_node_context(NodeKind::WidgetAddToViewport)
    }
    pub fn widget_viewport_node_context(&self, kind: NodeKind) -> Result<(), String> {
        if kind == NodeKind::WidgetRemoveFromParent
            && matches!(&self.kind, crate::GraphKind::Handler { .. })
            && !self.inspector_evaluation.is_legacy() {
            // Only static authoring eligibility. Actual callback/class/source/
            // epoch and object admission remain in the Engine transaction.
            return self.validate_narrative_metadata();
        }
        self.widget_factory_authoring_context().map(|_| ()).map_err(|_| "Ajouter au viewport exige une scène Native ; Retirer du parent admet aussi un vrai Handler Native. Init, fonctions et Screen ne sont pas pris en charge.".into())
    }
}
pub(crate) fn validate_node(graph: &GraphDocument, node: &GraphNode) -> Result<(), String> {
    if !is_viewport_kind(node.kind) { return Ok(()); }
    graph.widget_viewport_node_context(node.kind)?;
    let input = graph.pin_by_key(node.id, "object").ok_or("Widget viewport Object pin absent")?;
    if input.direction != PinDirection::Input || input.value_type != ValueType::WidgetObject || input.default_value.is_some() {
        return Err("Widget viewport Object must be a connected typed WidgetObject output, including actual nullable instances".into());
    }
    let mut incoming = graph.edges.values().filter(|edge| edge.input == input.id);
    let edge = incoming.next().ok_or("Widget viewport requires a connected Widget object")?;
    if incoming.next().is_some() { return Err("Widget viewport Object has ambiguous inputs".into()); }
    let output = graph.pins.get(&edge.output).ok_or("Widget viewport producer output absent")?;
    if output.direction != PinDirection::Output || graph.nodes.get(&output.node).is_none_or(|owner| !owner.pins.contains(&output.id))
        || graph.effective_pin_type(output.id) != Some(ValueType::WidgetObject) {
        return Err("Widget viewport requires an actual WidgetObject, not a class/string/Any conversion".into());
    }
    if node.kind == NodeKind::WidgetAddToViewport {
        let z_order = graph.pin_by_key(node.id, "z_order").ok_or("Widget viewport ZOrder pin absent")?;
        if z_order.direction != PinDirection::Input || z_order.value_type != ValueType::Int { return Err("Widget ZOrder requires its actual integer input".into()); }
        let mut edges = graph.edges.values().filter(|edge| edge.input == z_order.id);
        if let Some(edge) = edges.next() {
            let output = graph.pins.get(&edge.output).ok_or("Widget ZOrder source absent")?;
            if edges.next().is_some() || output.direction != PinDirection::Output || graph.effective_pin_type(output.id) != Some(ValueType::Int) {
                return Err("Widget ZOrder requires an actual integer producer; no Any/float cast".into());
            }
        } else if !matches!(z_order.default_value.as_ref(), Some(PropertyValue::Int(value)) if i32::try_from(*value).is_ok()) {
            return Err("Widget ZOrder default must be an exact int32".into());
        }
    }
    Ok(())
}
pub(crate) fn validate(graph: &GraphDocument) -> Vec<(NodeId, String)> {
    graph.nodes.values().filter(|node| is_viewport_kind(node.kind)).filter_map(|node| validate_node(graph,node).err().map(|error|(node.id,error))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GraphKind;
    use rvn_parser::Expr;
    fn connect(graph:&mut GraphDocument,from:NodeId,output:&str,to:NodeId,input:&str){
        graph.connect(graph.pin_by_key(from,output).unwrap().id,graph.pin_by_key(to,input).unwrap().id).unwrap();
    }
    fn seed()->(GraphDocument,NodeId){
        let parsed=rvn_parser::parse("scene label Play\nset instance=0\n\"Ready\"\nreturn\n").unwrap();
        let mut graph=crate::import_script(&parsed).unwrap().into_iter().find(|graph|matches!(&graph.kind,GraphKind::Label{name}if name=="Play")).unwrap();
        graph.enable_native_frame_preparation().unwrap();
        let assignment=graph.nodes.values().find(|node|node.kind==NodeKind::SetVariable).unwrap().id;
        let factory=graph.replace_assignment_with_widget_factory(assignment,&Expr::Call{name:"__rvn_widget_class_none_v1".into(),args:vec![]},&Expr::ListLit(vec![])).unwrap();
        (graph,factory)
    }
    #[test]
    fn viewport1003_native_scene_roundtrip_preserves_real_factory_object_and_effect_ids(){
        for(kind,layer)in [(NodeKind::WidgetAddToViewport,i32::MIN),(NodeKind::WidgetAddToViewport,i32::MAX),(NodeKind::WidgetRemoveFromParent,0)]{
            let(mut graph,factory)=seed();let ready=graph.nodes.values().find(|node|node.kind==NodeKind::Dialogue).unwrap().id;
            let output=graph.pin_by_key(factory,"exec_out").unwrap().id;
            let edge=graph.edges.values().find(|edge|edge.output==output).unwrap().id;graph.remove_edge(edge);
            let effect=graph.add_catalog_node(kind,[300.,0.]).unwrap();
            connect(&mut graph,factory,"exec_out",effect,"exec_in");connect(&mut graph,effect,"exec_out",ready,"exec_in");connect(&mut graph,factory,"value_out",effect,"object");
            if kind==NodeKind::WidgetAddToViewport{graph.set_pin_default(effect,"z_order",PropertyValue::Int(i64::from(layer))).unwrap();}
            assert!(!graph.validate().iter().any(|diagnostic|diagnostic.severity==crate::DiagnosticSeverity::Error));
            let compiled=crate::prepare_native_frame_graph(&graph).unwrap();
            let imported=crate::import_script(&compiled.ast).unwrap().into_iter().find(|item|item.graph_id==graph.graph_id).unwrap();
            assert_eq!(imported.nodes.keys().collect::<Vec<_>>(),graph.nodes.keys().collect::<Vec<_>>());
            assert_eq!(imported.pins.keys().collect::<Vec<_>>(),graph.pins.keys().collect::<Vec<_>>());
            assert_eq!(imported.edges,graph.edges);
            let again=crate::prepare_native_frame_graph(&imported).unwrap();assert_eq!(again.ast,compiled.ast);
        }
    }
    #[test]
    fn viewport1003_unsupported_context_construction_refuses_without_allocating_or_promoting(){
        for source in ["label Play\n\"Ready\"\nreturn\n","init{set value=0}","handler click(event){set value=0}","screen panel(){return []}","function pure(){return 0}"]{
            for mut graph in crate::import_script(&rvn_parser::parse(source).unwrap()).unwrap(){
                let before=serde_json::to_vec(&graph).unwrap();
                for kind in [NodeKind::WidgetAddToViewport,NodeKind::WidgetRemoveFromParent]{assert!(graph.add_catalog_node(kind,[0.,0.]).is_err());assert_eq!(serde_json::to_vec(&graph).unwrap(),before);}
            }
        }
    }
    #[test]
    fn handler_remove1003_native_handler_admits_remove_but_keeps_add_and_other_contexts_closed() {
        let mut graph = crate::import_script(&rvn_parser::parse("handler click(event){set hit=1}").unwrap()).unwrap()
            .into_iter().find(|graph| matches!(graph.kind, GraphKind::Handler { .. })).unwrap();
        assert!(graph.widget_viewport_node_context(NodeKind::WidgetRemoveFromParent).is_err());
        graph.enable_native_frame_preparation().unwrap();
        let remove = graph.add_catalog_node(NodeKind::WidgetRemoveFromParent, [100., 100.]).unwrap();
        assert!(validate_node(&graph, &graph.nodes[&remove]).is_err(), "eligibility never supplies a missing typed object");
        let before = serde_json::to_vec(&graph).unwrap();
        assert!(graph.add_catalog_node(NodeKind::WidgetAddToViewport, [200., 100.]).is_err());
        assert_eq!(serde_json::to_vec(&graph).unwrap(), before);
    }
    #[test]
    fn viewport1003_object_requires_real_type_and_layer_default_requires_exact_int32(){
        let(mut graph,factory)=seed();let effect=graph.add_catalog_node(NodeKind::WidgetAddToViewport,[300.,0.]).unwrap();
        assert!(validate_node(&graph,&graph.nodes[&effect]).is_err());
        connect(&mut graph,factory,"value_out",effect,"object");
        assert!(validate_node(&graph,&graph.nodes[&effect]).is_ok());
        graph.set_pin_default(effect,"z_order",PropertyValue::Int(i64::from(i32::MAX)+1)).unwrap();
        assert!(validate_node(&graph,&graph.nodes[&effect]).is_err());
        graph.set_pin_default(effect,"z_order",PropertyValue::Float(0.)).unwrap();
        assert!(validate_node(&graph,&graph.nodes[&effect]).is_err());
        graph.set_pin_default(effect,"z_order",PropertyValue::Int(0)).unwrap();
        let input=graph.pin_by_key(effect,"object").unwrap().id;
        let edge=graph.edges.values().find(|edge|edge.input==input).unwrap().id;graph.remove_edge(edge);
        let unknown=graph.add_catalog_node(NodeKind::FunctionCall,[100.,100.]).unwrap();
        graph.set_property(unknown,"function",PropertyValue::String("untyped_result".into())).unwrap();
        connect(&mut graph,unknown,"result",effect,"object");
        assert_eq!(graph.effective_pin_type(graph.pin_by_key(unknown,"result").unwrap().id),Some(ValueType::Any));
        assert!(validate_node(&graph,&graph.nodes[&effect]).is_err());
    }
}
