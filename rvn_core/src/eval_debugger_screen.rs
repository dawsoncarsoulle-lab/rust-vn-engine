//! Ephemeral authority from a real stopped ui.open candidate. Nothing in this
//! module is deserialized. Cold restore must run that same preparation again.
use super::*;

#[derive(Clone,PartialEq)]
struct Description {
    binding:crate::ui_consumer::BoundInstanceConsumerFrame,
    parameters:Vec<String>,body:Vec<Statement>,arguments:Vec<Value>,
}
#[derive(Clone)]
pub(super) struct ConstructionWitness {
    operation:Arc<crate::engine::DebugStateSource>,
    description:Arc<Description>,
}
impl PartialEq for ConstructionWitness {
    fn eq(&self,other:&Self)->bool {
        Arc::ptr_eq(&self.operation,&other.operation)&&self.description==other.description
    }
}
impl ConstructionWitness {
    pub(super) fn matches(&self,request:&Request)->bool {
        let description=&self.description;
        request.scope.as_deref()==Some(format!("screen:{}",description.binding.owner.definition).as_str())
            &&matches!(&request.root,Root::Block(body)if body==&description.body)
            &&request.state.consumer_frame==description.binding.frame
            &&description.parameters.iter().zip(&description.arguments).all(|(name,value)|request.variables.get(name)==Some(value))
    }
}
impl std::fmt::Debug for Request {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
        // Keep the previous exact Debug spelling for existing source anchors.
        f.debug_struct("Request").field("root",&self.root).field("variables",&self.variables)
            .field("state",&self.state).field("scope",&self.scope).field("route_prefix",&self.route_prefix).finish()
    }
}
impl std::fmt::Debug for Probe {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
        f.debug_struct("Probe").field("cursor",&self.cursor).field("records",&self.records)
            .field("requested",&self.requested).field("stale",&self.stale).field("scope",&self.scope)
            .field("bytes",&self.bytes).field("sealed",&self.sealed).field("route_prefix",&self.route_prefix).finish()
    }
}
impl Probe {
    pub(crate) fn begin_screen_preparation<R:crate::renderer::Renderer>(probe:&SharedProbe,engine:&crate::Engine<R>,session:&crate::engine::DebugSession)->EvalResult<()> {
        // This factory only yields a source for an accepted, actual UiOpen
        // DebugOperation::Statement, never for a name supplied by a caller.
        let actual=match engine.debug_screen_construction_source(session){Ok(actual)=>actual,Err(error)=>{Self::revoke_screen_construction(probe);return Err(invalid(&error.to_string()));}};
        let mut probe=probe.lock().map_err(|_|invalid("Screen construction trace unavailable"))?;
        probe.screen_active=None;
        match actual {
            Some(source)=>{
                if !probe.screen_operation.as_ref().is_some_and(|old|engine.accepts_debug_state_source(old)) {
                    probe.screen_operation=Some(Arc::new(source));
                }
            },
            None=>probe.screen_operation=None,
        }
        Ok(())
    }
    pub(crate) fn revoke_screen_construction(probe:&SharedProbe) {
        let mut probe=probe.lock().unwrap_or_else(|p|p.into_inner());
        probe.screen_operation=None;probe.screen_active=None;
    }
    pub(crate) fn validate_screen_construction<R:crate::renderer::Renderer>(probe:&SharedProbe,engine:&crate::Engine<R>)->EvalResult<()> {
        let probe=probe.lock().map_err(|_|invalid("Screen construction trace unavailable"))?;
        if probe.screen_operation.as_ref().is_some_and(|source|!engine.accepts_debug_state_source(source)) {
            return Err(invalid("Screen construction operation is stale or foreign"));
        }
        Ok(())
    }
    pub(crate) fn prepare_screen_publication(probe:&SharedProbe,continuation:Option<&Continuation>)->EvalResult<()> {
        let probe=probe.lock().map_err(|_|invalid("Screen publication trace unavailable"))?;
        if probe.screen_active.is_some(){return Err(invalid("Screen factory is still active at restore publication"));}
        for request in probe.records.iter().map(|record|&record.request).chain(probe.requested.iter()).chain(continuation.map(|c|&c.request)){
            if request.screen.is_some()&&!probe.admits_screen_request(request){return Err(invalid("Screen publication contains a foreign/stale factory witness"));}
        }
        if continuation.is_some_and(|c|probe.requested.as_ref()!=Some(&c.request)){return Err(invalid("Screen publication lost its actual pending request"));}
        Ok(())
    }
    pub(crate) fn publish_screen_construction(probe:&SharedProbe,continuation:Option<&mut Continuation>,source:crate::engine::DebugStateSource){
        // This only rebinds the private session owned by the admitted restore,
        // never arbitrary decoded roots. Validated before renderer publication.
        Self::prepare_screen_publication(probe,continuation.as_deref()).expect("owned Screen publication preflighted");
        let mut probe=probe.lock().unwrap_or_else(|p|p.into_inner());
        let Some(old)=probe.screen_operation.take()else{return;};let source=Arc::new(source);
        fn rebind(request:&mut Request,old:&Arc<crate::engine::DebugStateSource>,source:&Arc<crate::engine::DebugStateSource>){
            if let Some(witness)=&request.screen{
                assert!(Arc::ptr_eq(old,&witness.operation),"owned Screen witness was preflighted");
                request.screen=Some(Arc::new(ConstructionWitness{operation:source.clone(),description:witness.description.clone()}));
            }
        }
        for record in &mut probe.records{rebind(&mut record.request,&old,&source);}
        if let Some(request)=&mut probe.requested{rebind(request,&old,&source);}
        if let Some(current)=continuation{rebind(&mut current.request,&old,&source);}
        probe.screen_operation=Some(source);
    }
    pub(super) fn admits_screen_request(&self,request:&Request)->bool {
        request.screen.as_ref().is_some_and(|witness|self.screen_operation.as_ref()
            .is_some_and(|source|Arc::ptr_eq(source,&witness.operation))&&witness.matches(request))
    }
    pub(crate) fn screen_construction<R:crate::renderer::Renderer,T>(
        probe:&SharedProbe,engine:&crate::Engine<R>,next:&crate::GameState,
        binding:&crate::ui_consumer::BoundInstanceConsumerFrame,parameters:&[String],body:&[Statement],arguments:&[Value],
        operation:impl FnOnce()->T,
    )->EvalResult<T> {
        let source=probe.lock().map_err(|_|invalid("Screen construction trace unavailable"))?.screen_operation.clone();
        let Some(source)=source else{return Ok(operation());};
        if !engine.accepts_debug_state_source(&source){return Err(invalid("Screen factory belongs to a stale operation"));}
        engine.validate_debug_screen_factory(next,binding,parameters,body,arguments).map_err(|e|invalid(&e.to_string()))?;
        // Validate the existing observation budget before retaining the one
        // description shared by all Request clones. No AST enters Save15.
        measured(&format_args!("{:?}{:?}{:?}{:?}",binding,parameters,body,arguments))?;
        let witness=Arc::new(ConstructionWitness{operation:source,description:Arc::new(Description{
            binding:binding.clone(),parameters:parameters.to_vec(),body:body.to_vec(),arguments:arguments.to_vec(),
        })});
        struct Guard(SharedProbe,Option<Arc<ConstructionWitness>>);
        impl Drop for Guard{fn drop(&mut self){self.0.lock().unwrap_or_else(|p|p.into_inner()).screen_active=self.1.take();}}
        let previous={let mut p=probe.lock().map_err(|_|invalid("Screen construction trace unavailable"))?;std::mem::replace(&mut p.screen_active,Some(witness))};
        let _guard=Guard(probe.clone(),previous);Ok(operation())
    }
}

#[cfg(test)]
#[path="eval_debugger_screen_tests.rs"]
mod tests;
