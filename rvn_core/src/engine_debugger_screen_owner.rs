//! Source and operation checks for the ephemeral Screen factory witness.
use super::*;
impl<R:Renderer> Engine<R> {
    fn is_debug_screen_construction(&self,operation:&DebugOperation)->bool {
        if !matches!(operation,DebugOperation::Statement){return false;}
        match self.script.get(self.state.pc){
            Some(Statement::UiOpen{..})=>true,
            Some(Statement::ScheduledConsumer{body,..})=>matches!(body.as_slice(),[Statement::UiOpen{..}]),
            _=>false,
        }
    }
    pub(crate) fn debug_screen_construction_source(&self,session:&DebugSession)->Result<Option<DebugStateSource>,RuntimeError> {
        if !self.accepts_debug_session(session)||session.complete||session.error.is_some(){return Err(self.scene_resource_error("Screen construction has no current DebugSession operation".into()));}
        if !self.is_debug_screen_construction(&session.operation){return Ok(None);}
        self.consumer_program.validate_emitted(&self.script).map_err(|e|self.eval_err(e,"Screen construction source"))?;
        // Unfrozen legacy ui.open needs no construction permit. Its existing
        // live debugger path remains available; no native portable permission
        // is issued merely because the current instruction is ui.open.
        if self.consumer_program.source.is_none(){return Ok(None);}
        self.debug_state_source().map(Some)
    }
    pub(crate) fn validate_debug_screen_factory(
        &self,next:&GameState,binding:&crate::ui_consumer::BoundInstanceConsumerFrame,
        parameters:&[String],body:&[Statement],arguments:&[Value],
    )->Result<(),RuntimeError> {
        use crate::consumer_schedule::HostDefinitionKind;
        if !self.is_debug_screen_construction(&DebugOperation::Statement)||binding.owner.kind!=HostDefinitionKind::Screen {
            return Err(self.scene_resource_error("Screen factory is outside its actual UiOpen operation".into()));
        }
        self.consumer_program.validate_emitted(&self.script).map_err(|e|self.eval_err(e,"Screen construction source"))?;
        self.consumer_program.validate_instance_binding(binding).map_err(|e|self.eval_err(e,"Screen construction frame"))?;
        let name=&binding.owner.definition;
        let actual=self.consumer_program.capture_instance_owner(next,&binding.owner.target,name,HostDefinitionKind::Screen).map_err(|e|self.eval_err(e,"Screen construction owner"))?;
        if actual!=binding.owner{return Err(self.scene_resource_error("Screen factory target/source differs from actual candidate".into()));}
        let resident=crate::ui_residency::resident_interface(next,&actual.target).map_err(|e|self.scene_resource_error(e))?;
        let (actual_parameters,actual_body)=self.ui_library.screen_definition(name).ok_or_else(||self.scene_resource_error("Screen factory definition disappeared".into()))?;
        if resident.instance.arguments!=arguments||actual_parameters!=parameters||actual_body!=body||parameters.len()!=arguments.len(){
            return Err(self.scene_resource_error("Screen factory description/arguments differ from actual candidate".into()));
        }
        Ok(())
    }
    // Mapping is data derived from frozen source, not permission. The Request
    // witness is still required independently at capture and record replay.
    pub(super) fn debug_statement_screen_namespaces(&self,operation:&DebugOperation)->Result<std::collections::BTreeMap<String,std::collections::BTreeMap<String,String>>,String> {
        if !self.is_debug_screen_construction(operation){return Ok(Default::default());}
        let binding=self.consumer_program.source.as_ref().ok_or("Screen construction has no frozen namespaces")?;
        let mut roots=std::collections::BTreeMap::new();
        for(root,definition)in &binding.definitions{
            let name=&definition.definition;
            let kind=match definition.kind{crate::consumer_schedule::HostDefinitionKind::Screen=>"screen",crate::consumer_schedule::HostDefinitionKind::Handler=>"handler"};
            let scopes=definition.scopes.keys().map(|scope|(scope.clone(),format!("host-source:{}:{kind}:{}:{name}:{scope}",definition.source_ordinal,name.len()))).collect();
            roots.insert(root.clone(),scopes);
        }
        Ok(roots)
    }
    // Called only by the genuine, infallible PreparedSave15 publication after
    // the session is rebound to its actual new Engine generation/anchor. The
    // prepared session was validated before any live effect. Reissue a NEW
    // capability; old Request clones cannot become current through mutation.
    pub(super) fn publish_screen_construction_session(&self,session:&mut DebugSession){
        let source=DebugStateSource{instance:self.instance_id(),epoch:self.runtime_epoch(),anchor:session.anchor,
            text_resources:self.functions.text_resources().revision_stamp(),locale_revision:self.locale.as_ref().map(LocaleManager::revision_stamp)};
        Probe::publish_screen_construction(&session.probe,session.continuation.as_mut(),source);
    }
}
#[cfg(test)]
impl DebugSession {
    pub(crate) fn screen_probe_for_test(&self)->&SharedProbe{&self.probe}
}
#[cfg(test)]
impl<R:Renderer> Engine<R> {
    pub(crate) fn screen_debug_pending_for_test(&self,session:&DebugSession)->Result<Continuation,String>{
        if !self.accepts_debug_session(session){return Err("Foreign test session".into());}
        Probe::pending(&session.probe,&self.functions).map_err(|e|e.to_string())?.ok_or_else(||"Actual Screen request absent".into())
    }
}
