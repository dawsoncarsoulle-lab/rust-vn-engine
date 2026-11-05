//! Parameter projections prepared under the existing owning workspace admission.
//! Publication remains SourceWorkspace::save's atomic source/sidecar transaction.
use crate::*;
use crate::entry_parameter_identity1003::EntryParameters;

impl SourceWorkspace {
    pub fn prepare_unit_parameter_rename(&self, graphs:&[GraphDocument], declaration:GraphId,
        identity:&ParameterId, name:&str)->Result<Vec<GraphDocument>,String> {
        self.with_authoring_admission(|workspace|{
            let graph=graphs.iter().find(|graph|graph.graph_id==declaration).ok_or("Unit declaration is absent from this owner")?;
            let root=crate::unit_graph_source1003::entry(graph)?;
            let view=EntryParameters::from_admitted_projection(workspace,graphs,declaration,root)?;
            let parameters=view.pins()?;let bindings=view.bindings()?;
            let binding=parameters.iter().zip(&bindings).find(|(pin,_)|pin.parameter_id.as_ref()==Some(identity))
                .map(|(_,binding)|binding).ok_or("Opaque parameter is absent from this declaration")?;
            let prepared=view.rename_projection(binding,name)?;
            crate::unit_graph_source1003::validate_project(&prepared)?;
            Ok(prepared)
        })
    }

    pub fn prepare_unit_parameter_reorder(&self, graphs:&[GraphDocument], declaration:GraphId,
        order:&[ParameterId])->Result<Vec<GraphDocument>,String> {
        self.with_authoring_admission(|workspace|{
            let graph=graphs.iter().find(|graph|graph.graph_id==declaration).ok_or("Unit declaration is absent from this owner")?;
            let root=crate::unit_graph_source1003::entry(graph)?;
            let view=EntryParameters::from_admitted_projection(workspace,graphs,declaration,root)?;
            let parameters=view.pins()?;let bindings=view.bindings()?;
            let order=order.iter().map(|identity|parameters.iter().zip(&bindings)
                .find(|(pin,_)|pin.parameter_id.as_ref()==Some(identity)).map(|(_,binding)|binding.clone())
                .ok_or_else(||"Parameter permutation contains an absent/foreign identity".to_owned()))
                .collect::<Result<Vec<_>,_>>()?;
            let prepared=view.reorder_projection(&order)?;
            crate::unit_graph_source1003::validate_project(&prepared)?;
            Ok(prepared)
        })
    }
}

impl SourceWorkspace {
    /// Read-only source-context proof over the current actual Unit entry pin.
    /// This exposes no mutation, new parameter identity or cross-owner handle.
    pub fn unit_parameter_is_referenced(&self,graphs:&[GraphDocument],declaration:GraphId,
        identity:&ParameterId)->Result<bool,String>{
        self.with_authoring_admission(|workspace|{
            let graph=graphs.iter().find(|graph|graph.graph_id==declaration).ok_or("Unit declaration is outside this source owner")?;
            let root=crate::unit_graph_source1003::entry(graph)?;
            let view=EntryParameters::from_admitted_projection(workspace,graphs,declaration,root)?;
            let pin=view.pins()?.into_iter().find(|pin|pin.parameter_id.as_ref()==Some(identity))
                .ok_or("Parameter identity does not designate an actual current entry pin")?;
            crate::unit_parameter_references1003::source_uses_parameter(workspace,graphs,declaration,pin)
        })
    }
}
