//! Bounded native adapter for actual existing video instances. The historical
//! unscheduled MethodCall diagnostic is deliberately not redirected here.
use super::*;

#[derive(Clone,Copy)]
enum VideoMethod{Pause,Resume,Stop,Skip}
fn signature(command:&Statement)->Result<(&str,VideoMethod),&'static str>{
    let Statement::MethodCall{target,method,arg,transition}=command else{
        return Err("Native MethodCall requires its actual source statement");
    };
    let method=match method.as_str(){
        "pause"=>VideoMethod::Pause,"resume"=>VideoMethod::Resume,
        "stop"=>VideoMethod::Stop,"skip"=>VideoMethod::Skip,
        _=>return Err("Unsupported native MethodCall signature; diagnostic-only dispatch is not execution"),
    };
    if !rvn_ui::composition::valid_name(target){return Err("Native MethodCall target is not a valid video instance alias");}
    if arg.is_some(){return Err("Native video instance methods require zero arguments");}
    if !matches!(transition,Transition::None){return Err("Native video instance methods do not accept a transition");}
    Ok((target,method))
}
pub(crate) fn validate_native_signature(command:&Statement)->Result<(),&'static str>{signature(command).map(|_|())}

impl<R:Renderer> Engine<R>{
    fn current_native_method<'a>(&self,statement:&'a Statement)->Result<(&'a str,VideoMethod,crate::scene_resources::ResourceToken),RuntimeError>{
        let (target,method)=signature(statement).map_err(|e|self.scene_resource_error(e.into()))?;
        // Resolve the real Track installed by video.play. A string, a class
        // name, a saved descriptor, or a matching value is not an instance.
        let track=self.state.videos.tracks.get(target).ok_or_else(||
            self.scene_resource_error(format!("Native MethodCall target has no actual video instance: {target}")))?;
        track.validate().map_err(|e|self.scene_resource_error(e))?;
        let token={
            let execution=self.state.scene_execution.as_ref().ok_or_else(||
                self.scene_resource_error("Native MethodCall requires actual narrative video resource ownership".into()))?;
            use crate::scene_resources::{ResourceTaskKind,TargetKind};
            let task=execution.resources.task(ResourceTaskKind::Video,target,execution.owner())
                .ok_or_else(||self.scene_resource_error("Native MethodCall video target has no live owner".into()))?;
            let token=execution.resources.effect_token(self.runtime_epoch,&task.key,execution.owner())
                .ok_or_else(||self.scene_resource_error("Native MethodCall video target is parked or retired".into()))?;
            if token.target.kind!=TargetKind::Video||token.target.alias!=target
                ||!execution.resources.accepts(&token,self.runtime_epoch,execution.owner()){
                return Err(self.scene_resource_error("Native MethodCall video incarnation is no longer current".into()));
            }
            token
        };
        Ok((target,method,token))
    }
    pub(super) fn exec_native_method_call(&mut self,statement:Statement)->Result<(),RuntimeError>{
        let (target,method,_)=self.current_native_method(&statement)?;
        // Reuse the exact command paths: task renewal/removal, skip callback,
        // wait release, Startup observation and host projection happen there.
        let name=target.to_owned();
        let command=match method{
            VideoMethod::Pause=>crate::eval::UiCommand::VideoPause{name},
            VideoMethod::Resume=>crate::eval::UiCommand::VideoResume{name},
            VideoMethod::Stop=>crate::eval::UiCommand::VideoStop{name},
            VideoMethod::Skip=>crate::eval::UiCommand::VideoSkip{name},
        };
        let mut next=self.state.clone();
        self.apply_ui_commands(&mut next,vec![command])?;
        next.pc+=1;
        // This is called inside the existing source-bound classic candidate.
        // Its publication fence revalidates the genuine source/owner once;
        // no direct renderer callback or second object registry is introduced.
        self.commit_ui_state(next)
    }
}

// This borrow is minted from the retained Statement and its current video
// resource, never from Save15 labels or a fabricated UI/Host instance.
pub(super) struct StatementMediaScopePermit<'a,R:Renderer>{
    engine:&'a Engine<R>,source:DebugStateSource,token:crate::scene_resources::ResourceToken,
    root:String,namespaces:std::collections::BTreeMap<String,std::collections::BTreeMap<String,String>>,
}
impl<R:Renderer> crate::ui_consumer::PortableScopePermit for StatementMediaScopePermit<'_,R>{
    fn admits(&self,scope:&str)->bool{
        scope==self.root&&self.engine.accepts_debug_state_source(&self.source)
            &&self.engine.video_task_current(&self.token)
    }
}
impl<R:Renderer> StatementMediaScopePermit<'_,R>{
    pub(super) fn namespaces(&self)->&std::collections::BTreeMap<String,std::collections::BTreeMap<String,String>>{&self.namespaces}
}
impl<R:Renderer> Engine<R>{
    pub(super) fn statement_media_scope_permit<'a>(&'a self,program:&rvn_parser::ParsedProgram,operation:&DebugOperation)->Result<Option<StatementMediaScopePermit<'a,R>>,crate::save::SaveError>{
        use crate::save::SaveError;
        if !matches!(operation,DebugOperation::Statement){return Ok(None);}
        let Some(Statement::ScheduledConsumer{version:2,body,..})=self.script.get(self.state.pc)else{return Ok(None);};
        let [command@Statement::MethodCall{..}]=body.as_slice()else{return Ok(None);};
        let (target,method,token)=self.current_native_method(command).map_err(|error|SaveError::Incompatible(error.to_string()))?;
        if !matches!(method,VideoMethod::Skip){return Ok(None);}
        self.validate_save15_program(program)?;
        let track=&self.state.videos.tracks[target];
        if track.finished()||track.clip.looping||!track.clip.skippable{return Ok(None);}
        let Some(handler)=&track.clip.on_end else{return Ok(None);};
        // Native instance frames still require their actual resident adapter.
        // This slice covers a Scene/Game-owned video callback without a UI target.
        // The exact resource token came from the same read-only target checks
        // as execution. No video/interface description or cache promotion is
        // needed: target=None means video_projection_binding is also None.
        if track.clip.target.is_some()||matches!(token.task.key.owner,crate::scene_tasks::TaskOwner::Host{..}){return Ok(None);}
        let binding=self.consumer_program.source.as_ref().ok_or_else(||SaveError::Incompatible("Statement media callback has no frozen source".into()))?;
        let kind=crate::consumer_schedule::HostDefinitionKind::Handler;
        let definition=binding.definition(&kind,handler).ok_or_else(||SaveError::Incompatible("Statement media callback handler has no frozen definition".into()))?;
        if definition.native_plans{return Ok(None);}
        let root=kind.qualified(handler);
        let scopes=definition.scopes.keys().map(|scope|(scope.clone(),format!("statement-media-source:{}:{}:{}:{}",definition.source_ordinal,root.len(),root,scope))).collect();
        let namespaces=std::collections::BTreeMap::from([(root.clone(),scopes)]);
        Ok(Some(StatementMediaScopePermit{engine:self,source:self.debug_state_source().map_err(|error|SaveError::Incompatible(error.to_string()))?,token,root,namespaces}))
    }
}
