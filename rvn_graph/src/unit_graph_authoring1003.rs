//! Real Unit authoring over the owning source project; no identity allocator.
use crate::*;
impl SourceWorkspace {
    pub fn prepare_unit_call_node(&self, graphs:&[GraphDocument],caller:GraphId,
        callee:GraphId,position:[f64;2])->Result<(GraphDocument,NodeId),String>{
        self.with_authoring_admission(|workspace|{
            let declaration=graphs.iter().find(|graph|graph.graph_id==callee).ok_or("Unit declaration disappeared")?;
            let root=crate::unit_graph_source1003::entry(declaration)?;
            let view=crate::entry_parameter_identity1003::EntryParameters::from_admitted_projection(workspace,graphs,callee,root)?;
            let parameters=view.pins()?;
            let mut candidate=graphs.iter().find(|graph|graph.graph_id==caller).ok_or("Caller is outside this source owner")?.clone();
            if !matches!(candidate.kind,GraphKind::Init|GraphKind::Label{..}|GraphKind::UnitFunction{..}){
                return Err("Unit authoring currently supports Init, narrative and Unit bodies; Widget Handler/Screen/value Function remain unsupported".into());
            }
            let GraphKind::UnitFunction{name}=&declaration.kind else{return Err("Actual Unit callee required".into());};
            candidate.check_allocation_capacity(1,2+parameters.len(),0)?;
            let node=candidate.add_catalog_node(NodeKind::UnitCall,position).map_err(|error|error.to_string())?;
            candidate.set_property(node,"function",PropertyValue::String(name.clone())).map_err(|error|error.to_string())?;
            for parameter in parameters {
                let identity=parameter.parameter_id.clone().ok_or("Parameter identity missing")?;
                let input=candidate.add_pin(node,format!("argument_{}",identity.as_str()),&parameter.label,
                    PinDirection::Input,ValueType::Any,PinCardinality::One).map_err(|error|error.to_string())?;
                candidate.pins.get_mut(&input).ok_or("Prepared argument disappeared")?.parameter_id=Some(identity);
            }
            // Validate actual graph/container gates before returning an editable
            // call. NativeFrame schemas12/13 cannot silently become schema14.
            candidate.validate_narrative_metadata()?;
            crate::unit_graph_source1003::resolve_call(graphs,&candidate,node)?;
            // Real unconnected arguments are editable drafts. Source export
            // refuses missing values; no semantic default is fabricated.
            Ok((candidate,node))
        })
    }
}
