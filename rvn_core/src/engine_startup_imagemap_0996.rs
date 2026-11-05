//! Dedicated source-owned Startup Imagemap path. No default input answer.
use super::*;
/// Explicit backend opt-in to the same prepared publication transaction.
/// No generic Renderer/Raw-AST constructor supplies this capability.
/// Implementations must validate their real publication backend, then route
/// selection only from its installed and retained input image/geometry.
pub trait StartupInputBatchSink:StartupBatchSink {
    fn validate_startup_input_host(&self)->Result<(),String>;
}
/// Never serialized. The retained observation belongs to the actual waiting
/// owner; Bevy must also admit the exact completed-image receipt before using it.
pub struct StartupImagemapSource {pub(super) source:std::sync::Arc<StartupObservationSource>}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PortableStartupImagemap {
    pub(super) header:Vec<wire::Route>,
    pub(super) header_origin:crate::program::InstructionOrigin,
    pub(super) gather:Vec<crate::eval::debugger_eval::PortableRecord>,
    // None: reached input wait. Some: the actual selected authored branch.
    // Both are inert restore data, never a host/publication/input grant.
    pub(super) selected:Option<usize>,
}
impl StartupState {
    pub(in crate::engine) fn waiting_imagemap(&self)->bool{self.imagemaps.last().is_some_and(|frame|frame.selected.is_none())}
}
impl<R:StartupInputBatchSink> Engine<R> {
    /// Backend/source admission only. No Init statement or producer executes.
    pub fn prepare_input_startup(program:ParsedProgram,renderer:R,rollback_depth:usize,persistent_vars:HashMap<String,Value>,text_context:TextRuntimeContext,overlay:Option<(crate::debugger_provenance::Overlay,u64)>)->Result<EngineStartup<R>,RuntimeError>{
        renderer.validate_startup_input_host().map_err(|message|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction(message)),0))?;
        let program=std::sync::Arc::new(program);
        let input=crate::consumer_schedule::StartupInputProgram::from_frozen(program.clone()).map_err(|error|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(error),0))?;
        Self::prepare_owned_startup_in(program,renderer,rollback_depth,persistent_vars,text_context,overlay,(0,0,0),Some(std::sync::Arc::new(input)))
    }
}
impl<R:StartupInputBatchSink> Engine<R> {
    pub fn prepare_widget_class_input_startup(program:ParsedProgram,classes:crate::widget_class_source_0950::AdmittedWidgetClasses,renderer:R,rollback_depth:usize,persistent_vars:HashMap<String,Value>,text_context:TextRuntimeContext,overlay:Option<(crate::debugger_provenance::Overlay,u64)>)->Result<EngineStartup<R>,RuntimeError>{
        if !classes.matches_program(&program).map_err(|message|RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction(message)),0))?{
            return Err(RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction("Widget classes differ from the actual input Startup programme".into())),0));
        }
        Self::prepare_input_startup(program,renderer,rollback_depth,persistent_vars,text_context,overlay)?.with_widget_classes(std::sync::Arc::new(classes))
    }
}
impl<R:Renderer> Engine<R> {
    pub(in crate::engine) fn exec_startup_imagemap_header(&mut self,statement:&Statement)->Result<(),RuntimeError>{
        let input=self.consumer_program.startup_input.as_ref().ok_or_else(||self.scene_resource_error("Native Init Imagemap requires an actual capable Startup owner".into()))?;
        let startup=self.startup.as_ref().ok_or_else(||self.scene_resource_error("Init Imagemap has no actual pending owner".into()))?;
        if startup.waiting_imagemap()||startup.waiting_video.is_some(){return Err(self.scene_resource_error("Another actual Init wait is still pending".into()));}
        let root=startup.current().ok_or_else(||self.scene_resource_error("Init Imagemap root is absent".into()))?;
        let header=startup.route();let definition=root.definition_root_ordinal;
        if startup.statement(&self.script).map_err(|e|self.scene_resource_error(e))?!=statement{return Err(self.scene_resource_error("Init Imagemap differs from its actual source cursor".into()));}
        let Some((_,_,hotspots))=Self::consumer_imagemap_parts(statement)else{return Err(self.scene_resource_error("Init input is not the actual v2 Imagemap".into()));};
        if hotspots.len()>crate::consumer_schedule::MAX_TERMINALS{return Err(self.scene_resource_error("Init Imagemap exceeds the actual hotspot bound".into()));}
        if !hotspots.is_empty(){
            let map=self.functions.debug_source_map.as_ref().ok_or_else(||self.scene_resource_error("Init Imagemap source map is absent".into()))?;
            imagemap_route::prepare(input.program(),map,definition,&header,0).map_err(|e|self.scene_resource_error(e))?;
        }
        let(next,value)=self.gather_scheduled_action_inputs(statement)?;
        if !matches!(value,Value::List(ref fields)if fields.is_empty()){return Err(self.scene_resource_error("Init Imagemap static header has unexpected computed ports".into()));}
        // A zero-hotspot authored map executes its actual gather once and has
        // no input wait. The outer real Startup cursor advances normally.
        if hotspots.is_empty(){self.state=next;return Ok(());}
        let probe=self.functions.debug_probe.as_ref().ok_or_else(||self.scene_resource_error("Init Imagemap gather lost its actual continuation trace".into()))?;
        let gather=Probe::completed_loop_records(probe,&header).map_err(|e|self.eval_err(e,"Init Imagemap retained header"))?;
        let origin=self.startup_origin_for_route(&header)?;
        let frame=PortableStartupImagemap{header,header_origin:origin,gather,selected:None};
        crate::save::consumer_checkpoint::preflight_serialized(&frame,crate::save::consumer_checkpoint::Save15::MAX_BYTES).map_err(|e|self.scene_resource_error(e.to_string()))?;
        if self.startup.as_ref().unwrap().imagemaps.len()>=64{return Err(self.scene_resource_error("Init input continuation exceeds the actual nested source budget".into()));}
        self.state=next;self.startup.as_mut().unwrap().imagemaps.push(frame);Ok(())
    }
    pub(in crate::engine) fn startup_imagemap_interaction(&self)->Result<Option<Interaction>,RuntimeError>{
        let Some(startup)=&self.startup else{return Ok(None);};
        if !startup.waiting_imagemap(){return Ok(None);}
        let statement=startup.statement(&self.script).map_err(|e|self.scene_resource_error(e))?;
        let(background,hover,hotspots)=Self::consumer_imagemap_parts(statement).ok_or_else(||self.scene_resource_error("Init input wait lost its actual frozen Imagemap".into()))?;
        Ok(Some(Interaction::Imagemap{background:background.into(),hover_image:hover.map(str::to_owned),hotspots:hotspots.to_vec()}))
    }
}
impl<R:Renderer> EngineStartup<R> {
    /// Read-only admission of an already installed UI source, used by the
    /// painted blocking grid. This does not dispatch input or issue a handle.
    pub fn ui_input_source_current(&self,source:&crate::ui_input::UiInputSource)->bool{self.engine.ui_input_source_current(source)}
    pub fn capture_ui_input_source(&self,screen:&str)->Option<crate::ui_input::UiInputSource>{self.engine.capture_ui_input_source(screen)}
    /// Resolves only the actual process-local widget handle on this owner.
    /// This is backend provenance, never focus or executable input authority.
    pub fn widget_backend_owner_for_alias(&self,alias:&str)->Result<crate::widget_backend_0956::WidgetBackendOwner,RuntimeError>{
        let handle=self.engine.widget_instance_handle(alias).ok_or_else(||self.engine.scene_resource_error("Startup widget backend has no actual class instance".into()))?;
        self.engine.widget_backend_owner(&handle)
    }
    pub fn waiting_for_imagemap(&self)->bool{self.session.is_none()&&self.engine.startup.as_ref().is_some_and(StartupState::waiting_imagemap)}
    pub fn imagemap_interaction(&self)->Result<Option<Interaction>,RuntimeError>{self.engine.startup_imagemap_interaction()}
    /// Must be called with the observation retained by the admitted completed
    /// image. A fresh/latest observation cannot substitute for that receipt.
    pub fn imagemap_input_source(&self,source:std::sync::Arc<StartupObservationSource>)->Result<StartupImagemapSource,RuntimeError>{
        if !self.waiting_for_imagemap()||!self.accepts_observation_source(&source){return Err(self.engine.scene_resource_error("Completed image does not belong to the actual waiting Init Imagemap".into()));}
        Ok(StartupImagemapSource{source})
    }
    pub fn accepts_imagemap_input_source(&self,source:&StartupImagemapSource)->bool{self.waiting_for_imagemap()&&self.accepts_observation_source(&source.source)}
}

impl<R:Renderer> Engine<R> {
    pub(in crate::engine) fn validate_startup_imagemap_wait(&self)->Result<StartupOperation,RuntimeError>{
        let startup=self.startup.as_ref().ok_or_else(||self.scene_resource_error("Init input has no actual pending owner".into()))?;
        if !startup.waiting_imagemap()||startup.waiting_video.is_some()||self.consumer_program.startup_input.is_none(){return Err(self.scene_resource_error("Init input has no actual admitted waiting Imagemap".into()));}
        let operation=self.startup_operation()?;
        let frame=startup.imagemaps.last().unwrap();
        if operation.route!=frame.header||operation.origin!=frame.header_origin||self.startup_imagemap_interaction()?.is_none(){return Err(self.scene_resource_error("Waiting Imagemap differs from its original Init source/cursor".into()));}
        Ok(operation)
    }
}
impl<R:Renderer> EngineStartup<R> {
    pub(super) fn imagemap_wait_snapshot(&self)->wire::Snapshot{
        let operation=self.engine.validate_startup_imagemap_wait().expect("Admitted Init input wait keeps its exact actual source");
        let mut snapshot=self.engine.debug_current_snapshot(&self.watches).expect("Admitted bounded watches");
        snapshot.site=Some(operation.entry_site());snapshot.origin=Some(operation.origin);
        snapshot.evaluating=false;snapshot.running=false;snapshot.armed=None;snapshot.breakpoints=self.breakpoints.clone();snapshot
    }
}

impl<R:Renderer> Engine<R> {
    pub(in crate::engine) fn validate_startup_imagemap_gathers(&self)->Result<(),RuntimeError>{
        let Some(startup)=&self.startup else{return Ok(());};
        if startup.imagemaps.is_empty(){
            if startup.route().iter().any(|part|matches!(part,wire::Route::Option(_))){return Err(self.scene_resource_error("Init selected hotspot has no retained actual input continuation".into()));}
            return Ok(());
        }
        let input=self.consumer_program.startup_input.as_ref().ok_or_else(||self.scene_resource_error("Saved Init input requires its capable source-owned Startup constructor".into()))?;
        let root=startup.current().ok_or_else(||self.scene_resource_error("Saved Init input lost its actual declaration".into()))?;
        let map=self.functions.debug_source_map.as_ref().ok_or_else(||self.scene_resource_error("Saved Init input source map absent".into()))?;
        let current=startup.route();let mut selected=Vec::new();
        for(index,frame)in startup.imagemaps.iter().enumerate(){
            let cursor=imagemap_route::prepare(input.program(),map,root.definition_root_ordinal,&frame.header,frame.selected.unwrap_or(0)).map_err(|e|self.scene_resource_error(e))?;
            if cursor.parent_origin()!=&frame.header_origin{return Err(self.scene_resource_error("Saved Init input origin differs from its actual source".into()));}
            match frame.selected{
                Some(chosen)=>{
                    let Some((route,_))=cursor.continuation()else{return Err(self.scene_resource_error("An empty selected hotspot cannot retain a child continuation".into()));};
                    let prefix=&route[..route.len()-1];
                    if !current.starts_with(prefix){return Err(self.scene_resource_error("Saved selected hotspot is outside its real active branch".into()));}
                    selected.push((frame.header.clone(),chosen));
                },
                None if index+1==startup.imagemaps.len()&&frame.header==current&&startup.waiting_video.is_none()=>{},
                None=>return Err(self.scene_resource_error("Saved Init input wait is not its actual innermost current header".into())),
            }
            let statement=startup.statement_at(&self.script,&frame.header).map_err(|e|self.scene_resource_error(e))?;
            let Statement::ScheduledConsumer{scope,..}=statement else{return Err(self.scene_resource_error("Saved input header has no real consumer".into()));};
            self.consumer_program.validate_plan_binding(statement,root.pc,true).map_err(|e|self.eval_err(e,"Saved Init input plan"))?;
            let scope=self.consumer_program.bound_scope(scope,root.pc).map_err(|e|self.eval_err(e,"Saved Init input scope"))?;
            let fields=self.functions.validate_retained_imagemap_input(statement,&scope,&frame.header,&frame.gather,self.locale.as_ref()).map_err(|e|self.eval_err(e,"Saved actual Imagemap header"))?;
            if !matches!(fields,Value::List(ref fields)if fields.is_empty()){return Err(self.scene_resource_error("Saved Init input header has foreign computed fields".into()));}
        }
        let mut actual=Vec::new();
        for(index,part)in current.iter().enumerate(){if let wire::Route::Option(chosen)=part{
            if index<2||current[index-2..index]!=[wire::Route::Body,wire::Route::Statement(0)]{return Err(self.scene_resource_error("Init hotspot route lost its real consumer parent".into()));}
            actual.push((current[..index-2].to_vec(),*chosen));
        }}
        if selected!=actual{return Err(self.scene_resource_error("Saved Init input stack differs from its exact selected source routes".into()));}
        Ok(())
    }
    pub(in crate::engine) fn rebind_startup_imagemap_gathers(&mut self,program:&ParsedProgram)->Result<(),crate::save::SaveError>{
        if self.startup.as_ref().is_none_or(|startup|startup.imagemaps.is_empty()){return Ok(());}
        let(_,map)=crate::save::consumer_checkpoint::portable_namespaces(program,&self.script,&self.consumer_program)?;
        let inverse=map.into_iter().map(|(live,logical)|(logical,live)).collect();
        for frame in &mut self.startup.as_mut().unwrap().imagemaps{
            crate::save::consumer_checkpoint::remap_instruction_origin(&mut frame.header_origin,program,false)?;
            for record in &mut frame.gather{record.remap_loop_namespaces(&inverse).map_err(|e|crate::save::SaveError::Incompatible(e.to_string()))?;}
        }
        self.validate_startup_imagemap_gathers().map_err(|e|crate::save::SaveError::Incompatible(e.to_string()))
    }
    pub(super) fn select_startup_imagemap(&mut self,selected:usize)->Result<(),RuntimeError>{
        self.validate_startup_imagemap_wait()?;self.validate_startup_imagemap_gathers()?;
        let input=self.consumer_program.startup_input.as_ref().unwrap();let startup=self.startup.as_ref().unwrap();let root=startup.current().unwrap();let header=startup.route();
        let cursor=imagemap_route::prepare(input.program(),self.functions.debug_source_map.as_ref().unwrap(),root.definition_root_ordinal,&header,selected).map_err(|e|self.scene_resource_error(e))?;
        let startup=self.startup.as_mut().unwrap();startup.imagemaps.last_mut().unwrap().selected=Some(selected);
        match cursor.continuation(){Some((route,_))=>startup.set_route(route.to_vec()),None=>startup.advance_from(&self.script,header)}.map_err(|e|self.scene_resource_error(e))?;
        if self.startup.as_ref().is_some_and(|startup|startup.current().is_none()){self.startup=None;}
        self.state.pc=self.startup.as_ref().and_then(|startup|startup.current().map(|root|root.pc)).unwrap_or(self.init_scan_end);
        self.sync_narrative_position();Ok(())
    }
}

#[cfg(test)]
#[path="engine_startup_imagemap_tests_0997.rs"]
mod tests;
