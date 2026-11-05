//! Portable source/owner DTOs. Live namespaces and callback handles are never
//! accepted as portable authority; the current frozen program is recomputed.
use rvn_parser::{ParsedProgram,Statement};
use serde::{Serialize,Deserialize};
use crate::save::SaveError;
use crate::debugger_provenance::SourceMap;

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableConsumerProgram {
    pub version:u32,
    pub emitted:[u8;32],
    pub content:[u8;32],
    pub source_bytes_in_inclusion_order:Vec<String>,
    pub root_source_ordinals:Vec<usize>,
    /// Complete AST witness, compared to the current reparsed source before use.
    /// This client-supplied witness is never executed or used to hydrate tasks.
    pub script:Vec<Statement>,
}
impl PortableConsumerProgram {
    pub fn from_program(program:&ParsedProgram)->Result<Self,SaveError> {
        preflight_program(program)?;
        let map=SourceMap::from_program(program).map_err(SaveError::Incompatible)?;
        let root_source_ordinals=program.origins.iter().map(|origin|
            program.sources.iter().position(|source|source.path==origin.source)
                .ok_or_else(||SaveError::Incompatible("Consumer origin has no actual frozen source ordinal".into())))
            .collect::<Result<Vec<_>,_>>()?;
        Ok(Self {version:1,emitted:map.emitted,content:map.content,
            source_bytes_in_inclusion_order:program.sources.iter().map(|source|source.source.clone()).collect(),
            root_source_ordinals,script:program.script.clone()})
    }
    pub fn validate_current(&self,program:&ParsedProgram)->Result<(),SaveError> {
        if self.version!=1 || *self!=Self::from_program(program)? {
            return Err(SaveError::Incompatible("Portable consumer programme differs from actual frozen bytes/order/AST/source ordinals".into()));
        }
        Ok(())
    }
}

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum PortableConsumerOwner {
    Game,
    Scene {activation:crate::scene::ActivationId},
    Function {source_ordinal:usize,definition:String,stack_frame:usize},
    Host {target:crate::scene_resources::TargetIdentity,source_ordinal:usize,definition:String,#[serde(default,skip_serializing_if="Option::is_none")] definition_kind:Option<crate::consumer_schedule::HostDefinitionKind>},
}
impl PortableConsumerOwner {
    /// Host lifetime is admitted only when an actual instance adapter exists.
    /// This DTO shape never maps unknown or missing instances to Game.
    pub fn require_story_frame(&self,state:&crate::GameState)->Result<(),SaveError> {
        match self {
            Self::Game=>Ok(()),
            Self::Scene {activation} if state.scene_execution.as_ref().is_some_and(|execution|
                execution.active.as_ref().is_some_and(|active|active.id==*activation)
                ||execution.frames.iter().any(|frame|frame.parked.as_ref().is_some_and(|active|active.id==*activation)))=>Ok(()),
            Self::Scene {..}=>Err(SaveError::Incompatible("Portable consumer scene owner is not in the actual active/parked state".into())),
            Self::Function {..}=>Err(SaveError::Incompatible("Function consumer owner requires its validated saved continuation stack".into())),
            Self::Host {..}=>Err(SaveError::Incompatible("Host/widget consumer owner requires actual TargetIdentity/incarnation/handler lifecycle adapter".into())),
        }
    }
}

use std::collections::{BTreeMap,BTreeSet};
use crate::consumer_schedule::{BoundConsumerFrame,ConsumerOwner,ConsumerProgram,TextTableTerminals};
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableInspectorScope {source_ordinal:usize,scope:String,terminals:BTreeMap<String,TextTableTerminals>}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableFrame {owner:PortableConsumerOwner,scopes:Vec<PortableInspectorScope>}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableStory {base:crate::GameState,frames:Vec<PortableFrame>,
    #[serde(default,skip_serializing_if="Vec::is_empty")] projections:Vec<PortableUiProjection>}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableUiProjection {owner:PortableConsumerOwner,root:Option<rvn_ui::programmable::Component>,#[serde(default,skip_serializing_if="rvn_ui::programmable::ScreenHostProperties::is_default")] host:rvn_ui::programmable::ScreenHostProperties}
fn visit_ui_instances(state:&mut crate::GameState,mut visit:impl FnMut(&mut crate::ui::ScreenInstance)->Result<(),SaveError>)->Result<(),SaveError>{
    for instance in &mut state.ui.screens{visit(instance)?;}
    if let Some(execution)=&mut state.scene_execution{for active in execution.active.iter_mut().chain(execution.frames.iter_mut().filter_map(|frame|frame.parked.as_mut())){for instance in &mut active.tasks.suspended_ui.screens{visit(instance)?;}}}
    Ok(())
}
fn host_owner(owner:&crate::ui_consumer::InstanceConsumerOwner)->PortableConsumerOwner{PortableConsumerOwner::Host{target:owner.target.clone(),source_ordinal:owner.source_ordinal,definition:owner.definition.clone(),definition_kind:Some(owner.kind.clone())}}
fn restore_host_owner(owner:&PortableConsumerOwner,state:&crate::GameState,binding:&ConsumerProgram)->Result<crate::ui_consumer::InstanceConsumerOwner,SaveError>{
    let PortableConsumerOwner::Host{target,source_ordinal,definition,definition_kind}=owner else{return Err(incompatible("Actual Host owner is required"));};
    let resident=crate::ui_residency::resident_interface(state,target).map_err(|s|incompatible(&s))?;
    let kind=definition_kind.as_ref().ok_or_else(||incompatible("Portable Host frame has no exact Screen/Handler definition kind"))?;
    let source=binding.host_definition(kind,definition).filter(|source|source.native_plans&&source.source_ordinal==*source_ordinal).ok_or_else(||incompatible("Host owner has no actual loaded source definition/ordinal"))?;
    if source.kind==crate::consumer_schedule::HostDefinitionKind::Screen&&resident.instance.definition()!=definition.as_str(){return Err(incompatible("Host Screen definition differs from real resident instance"));}
    Ok(crate::ui_consumer::InstanceConsumerOwner{target:resident.identity,source_ordinal:*source_ordinal,definition:definition.clone(),kind:source.kind.clone()})
}
struct ScopeBinding {ordinal:usize,scope:String,live:String,producers:BTreeSet<String>}
fn incompatible(message:&str)->SaveError{SaveError::Incompatible(message.into())}
fn scope_bindings(program:&ParsedProgram,script:&[Statement],binding:&ConsumerProgram)->Result<Vec<ScopeBinding>,SaveError>{
    let source=binding.source.as_ref().ok_or_else(||incompatible("Save15 requires actual frozen source binding"))?;
    let mut mappings:Vec<ScopeBinding>=Vec::new();
    for(pc,statement)in script.iter().enumerate(){
        let mut plans=Vec::new();visit_statement_plans(std::slice::from_ref(statement),&mut |plan|{plans.push(plan);Ok(())})?;
        if plans.is_empty(){continue;}
        let origin=source.origins.get(pc).ok_or_else(||incompatible("Emitted consumer has no frozen source origin"))?;
        let ordinal=program.sources.iter().position(|s|s.path==origin.source).ok_or_else(||incompatible("Emitted consumer has no real source ordinal"))?;
        for plan in plans {
            let Statement::ScheduledConsumer{scope,steps,..}=plan else{unreachable!()};
            let live=binding.bound_scope(scope,pc).map_err(|e|incompatible(&e.to_string()))?;
            let producers=steps.iter().filter_map(|step|match step{rvn_parser::ConsumerStep::InspectTextTable{producer,..}=>Some(producer.clone()),_=>None}).collect::<BTreeSet<_>>();
            if let Some(old)=mappings.iter_mut().find(|m|m.ordinal==ordinal&&m.scope==*scope){if old.live!=live{return Err(incompatible("Logical inspector scope maps to differing live namespaces"));}old.producers.extend(producers);}
            else{mappings.push(ScopeBinding{ordinal,scope:scope.clone(),live,producers});}
        }
    }
    Ok(mappings)
}
fn export_frame(frame:&BoundConsumerFrame,mappings:&[ScopeBinding])->Result<PortableFrame,SaveError>{
    frame.frame.validate().map_err(|e|incompatible(&e.to_string()))?;
    let encoded=serde_json::to_value(&frame.frame)?;
    let mut scopes=Vec::new();
    for(live,terminals)in encoded.get("scopes").and_then(|v|v.as_object()).ok_or_else(||incompatible("Consumer frame has no typed terminal storage"))?{
        let mapping=mappings.iter().find(|m|m.live==*live).ok_or_else(||incompatible("Consumer frame namespace has no actual frozen source scope"))?;
        let terminals:BTreeMap<String,TextTableTerminals>=serde_json::from_value(terminals.clone())?;
        if terminals.keys().any(|p|!mapping.producers.contains(p)){return Err(incompatible("Consumer terminal has no inspector in loaded plans"));}
        scopes.push(PortableInspectorScope{source_ordinal:mapping.ordinal,scope:mapping.scope.clone(),terminals});
    }
    Ok(PortableFrame{owner:match frame.owner{ConsumerOwner::Game=>PortableConsumerOwner::Game,ConsumerOwner::Scene{activation}=>PortableConsumerOwner::Scene{activation}},scopes})
}
fn visit_story_slots(state:&mut crate::GameState,mut visit:impl FnMut(&mut Option<BoundConsumerFrame>)->Result<(),SaveError>)->Result<(),SaveError>{
    visit(&mut state.consumer_frame)?;
    if let Some(execution)=&mut state.scene_execution {
        if let Some(active)=&mut execution.active{visit(&mut active.consumer_frame)?;}
        for frame in &mut execution.frames{if let Some(parked)=&mut frame.parked{visit(&mut parked.consumer_frame)?;}}
    }Ok(())
}
impl PortableStory {
    pub(crate) fn capture(state:&crate::GameState,program:&ParsedProgram,script:&[Statement],binding:&ConsumerProgram)->Result<Self,SaveError>{
        Self::capture_with_widget_values(state,program,script,binding,None)
    }
    pub(crate) fn capture_with_widget_values(state:&crate::GameState,program:&ParsedProgram,script:&[Statement],binding:&ConsumerProgram,admission:Option<&crate::widget_reference_values_0974::WidgetValueAdmission>)->Result<Self,SaveError>{
        preflight_story_state(state)?;
        crate::game_tasks::validate_portable(state).map_err(|e|incompatible(&e))?;
        binding.validate_state_binding(state).map_err(|e|incompatible(&e.to_string()))?;
        let mappings=scope_bindings(program,script,binding)?;let mut base=state.clone();let mut frames=Vec::new();let mut projections=Vec::new();
        crate::widget_reference_values_0974::visit_state_values_mut(&mut base,&mut |value|match admission{
            Some(admission)=>admission.value(value,crate::widget_reference_values_0974::ReferencePhase::Export),
            None=>crate::widget_reference_values_0974::reject_widget_values(value)
        }).map_err(SaveError::Incompatible)?;
        for dialogue in base.consumer_dialogue.iter_mut().chain(base.scene_execution.as_mut().and_then(|e|e.display.as_mut()).and_then(|d|d.consumer_dialogue.as_mut())).chain(base.last_dialogue.as_mut().and_then(|d|d.consumer_dialogue.as_mut())) {dialogue.program=[0;32];}
        for data in base.consumer_imagemap.iter_mut().chain(base.scene_execution.as_mut().and_then(|e|e.display.as_mut()).and_then(|d|d.consumer_imagemap.as_mut())){data.program=[0;32];}
        for choice in base.consumer_choice.iter_mut().chain(base.scene_execution.as_mut().and_then(|e|e.display.as_mut()).and_then(|d|d.consumer_choice.as_mut())){choice.program=[0;32];}
        visit_story_slots(&mut base,|slot|{if let Some(frame)=slot.take(){frames.push(export_frame(&frame,&mappings)?);}Ok(())})?;
        visit_ui_instances(&mut base,|instance|{
            instance.standard_macro_frame=None; // explicit cold snapshot policy, including in-memory DTOs
            for(_,bound)in std::mem::take(&mut instance.consumer_frames){
                binding.validate_instance_binding(&bound).map_err(|e|incompatible(&e.to_string()))?;
                let encoded=serde_json::to_value(&bound.frame)?;
                let scopes=encoded.get("scopes").and_then(|value|value.as_object()).ok_or_else(||incompatible("Host frame lacks typed scope map"))?.iter().map(|(scope,terminals)|Ok(PortableInspectorScope{source_ordinal:bound.owner.source_ordinal,scope:scope.clone(),terminals:serde_json::from_value(terminals.clone())?})).collect::<Result<Vec<_>,SaveError>>()?;
                frames.push(PortableFrame{owner:host_owner(&bound.owner),scopes});
            }
            if let Some(projection)=instance.consumer_projection.take(){projections.push(PortableUiProjection{owner:host_owner(&projection.owner),root:projection.root,host:projection.host});}
            Ok(())
        })?;
        remap_native_loop_namespaces(&mut base,program,script,binding,true)?;
        remap_source_origins(&mut base,program,true)?;
        // Preserve the historical serde validation/omission boundary after
        // mapping typed compiler positions; user values are never remapped.
        base=serde_json::from_value(serde_json::to_value(&base)?)?;
        Ok(Self{base,frames,projections})
    }
    pub(crate) fn restore(&self,program:&ParsedProgram,script:&[Statement],binding:&ConsumerProgram)->Result<crate::GameState,SaveError>{
        if self.frames.len()>16384+2+crate::scene::MAX_NARRATIVE_FRAMES{return Err(incompatible("Too many portable activation frames"));}
        preflight_story_state(&self.base)?;
        crate::game_tasks::validate_portable(&self.base).map_err(|e|incompatible(&e))?;
        let mut base=self.base.clone();remap_source_origins(&mut base,program,false)?;remap_native_loop_namespaces(&mut base,program,script,binding,false)?;base=serde_json::from_value(serde_json::to_value(&base)?)?;visit_story_slots(&mut base,|slot|if slot.is_some(){Err(incompatible("Portable base cannot carry live salted frames"))}else{Ok(())})?;
        if self.projections.len()>16384{return Err(incompatible("Too many portable native UI projections"));}
        visit_ui_instances(&mut base,|instance|if !instance.consumer_frames.is_empty()||instance.consumer_projection.is_some(){Err(incompatible("Portable base cannot carry live native UI frames/projections"))}else{Ok(())})?;
        let mut projected=Vec::new();
        for projection in &self.projections{
            let owner=restore_host_owner(&projection.owner,&base,binding)?;
            if owner.kind!=crate::consumer_schedule::HostDefinitionKind::Screen||projected.contains(&owner.target){return Err(incompatible("Portable native projection has duplicate/wrong definition owner"));}projected.push(owner.target.clone());
            projection.host.validate().map_err(|s|incompatible(&s))?;
            if let Some(root)=&projection.root{root.validate().map_err(|s|incompatible(&s))?;}
            let instance=crate::ui_consumer::instance_mut(&mut base,&owner.target).map_err(|s|incompatible(&s))?;
            instance.consumer_projection=Some(crate::ui_consumer::InstanceProjection{program:binding.identity().map_err(|e|incompatible(&e.to_string()))?,owner,root:projection.root.clone(),host:projection.host.clone()});
        }
        let mappings=scope_bindings(program,script,binding)?;let mut attached=Vec::new();
        for dialogue in base.consumer_dialogue.iter_mut().chain(base.scene_execution.as_mut().and_then(|e|e.display.as_mut()).and_then(|d|d.consumer_dialogue.as_mut())).chain(base.last_dialogue.as_mut().and_then(|d|d.consumer_dialogue.as_mut())) {if dialogue.program!=[0;32]{return Err(incompatible("Portable Dialogue cannot carry a live salted programme binding"));}dialogue.program=binding.identity().map_err(|e|incompatible(&e.to_string()))?;}
        for data in base.consumer_imagemap.iter_mut().chain(base.scene_execution.as_mut().and_then(|e|e.display.as_mut()).and_then(|d|d.consumer_imagemap.as_mut())){if data.program!=[0;32]{return Err(incompatible("Portable Imagemap cannot carry a live salted program binding"));}data.program=binding.identity().map_err(|e|incompatible(&e.to_string()))?;}
        for choice in base.consumer_choice.iter_mut().chain(base.scene_execution.as_mut().and_then(|e|e.display.as_mut()).and_then(|d|d.consumer_choice.as_mut())){if choice.program!=[0;32]{return Err(incompatible("Portable Choice cannot carry a live salted programme binding"));}choice.program=binding.identity().map_err(|e|incompatible(&e.to_string()))?;}
        for frame in &self.frames {
            if !matches!(frame.owner,PortableConsumerOwner::Host{..}){frame.owner.require_story_frame(&base)?;}
            if attached.contains(&frame.owner){return Err(incompatible("Duplicate portable frame owner"));}attached.push(frame.owner.clone());
            if matches!(frame.owner,PortableConsumerOwner::Host{..}){
                let owner=restore_host_owner(&frame.owner,&base,binding)?;
                if frame.scopes.iter().any(|scope|scope.source_ordinal!=owner.source_ordinal){return Err(incompatible("Portable Host scope belongs to another source ordinal"));}
                let mut scopes=BTreeMap::new();for scope in &frame.scopes{if scopes.insert(scope.scope.clone(),scope.terminals.clone()).is_some(){return Err(incompatible("Repeated portable Host scope"));}}
                let current:crate::consumer_schedule::ConsumerFrame=serde_json::from_value(serde_json::json!({"version":1,"scopes":scopes}))?;
                let bound=crate::ui_consumer::BoundInstanceConsumerFrame{program:binding.identity().map_err(|e|incompatible(&e.to_string()))?,owner:owner.clone(),frame:current};
                binding.validate_instance_binding(&bound).map_err(|e|incompatible(&e.to_string()))?;
                let instance=crate::ui_consumer::instance_mut(&mut base,&owner.target).map_err(|s|incompatible(&s))?;
                if instance.consumer_frames.insert(owner.storage_key(),bound).is_some(){return Err(incompatible("Repeated portable Host definition frame"));}
                continue;
            }
            let owner=match frame.owner{PortableConsumerOwner::Game=>ConsumerOwner::Game,PortableConsumerOwner::Scene{activation}=>ConsumerOwner::Scene{activation},_=>return Err(incompatible("Portable frame owner needs its actual host/continuation adapter"))};
            let mut scopes=BTreeMap::new();
            for scope in &frame.scopes {
                let map=mappings.iter().find(|m|m.ordinal==scope.source_ordinal&&m.scope==scope.scope).ok_or_else(||incompatible("Portable terminal namespace has no current source ordinal/scope"))?;
                if scope.terminals.keys().any(|p|!map.producers.contains(p))||scopes.insert(map.live.clone(),scope.terminals.clone()).is_some(){return Err(incompatible("Portable terminals differ from actual inspector plan"));}
            }
            let current:crate::consumer_schedule::ConsumerFrame=serde_json::from_value(serde_json::json!({"version":1,"scopes":scopes}))?;
            current.validate().map_err(|e|incompatible(&e.to_string()))?;
            let bound=BoundConsumerFrame{program:binding.identity().map_err(|e|incompatible(&e.to_string()))?,owner,frame:current};
            match owner {
                ConsumerOwner::Game=>base.consumer_frame=Some(bound),
                ConsumerOwner::Scene{activation}=>{
                    let execution=base.scene_execution.as_mut().ok_or_else(||incompatible("Missing actual scene activation"))?;
                    if let Some(active)=execution.active.as_mut().filter(|a|a.id==activation){active.consumer_frame=Some(bound);}
                    else {let parked=execution.frames.iter_mut().find_map(|f|f.parked.as_mut().filter(|a|a.id==activation)).ok_or_else(||incompatible("Missing actual parked caller"))?;parked.consumer_frame=Some(bound);}
                }
            }
        }
        binding.validate_state_binding(&base).map_err(|e|incompatible(&e.to_string()))?;Ok(base)
    }
}

/// The first executable Save15 boundary. The public wire preserves the whole
/// real player state and immutable text/provider receipts; live handles are absent.
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Save15 {
    pub format_version:u32,
    pub slot:u32,
    pub label:String,
    pub source:PortableConsumerProgram,
    pub text_tables:crate::string_table::StringTableContentSnapshot,
    pub locale:Option<crate::locale::LocaleContentSnapshot>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub(crate) widget_classes:Option<crate::widget_class_source_0950::WidgetClassCatalogReceipt>,
    /// Checkpoint16 nominal references. Absent on exact historical Save15.
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub(crate) enum_references1025:Option<Vec<rvn_parser::enum_asset::EnumValue>>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub native_media_checkpoint:Option<Vec<rvn_media::checkpoint::NativeCheckpoint>>,
    pub(crate) story:PortableStory,
    pub(crate) persistent_context:BTreeMap<String,rvn_parser::Value>,
    pub(crate) debugger:Option<PortableDebugState>,
}
/// Explicit extension of the same source-admitted checkpoint, not a legacy maximum.
pub const ENUM_NOMINAL_CHECKPOINT_FORMAT_VERSION:u32=16;
impl Save15 {
    pub const MAX_BYTES:usize=64*1024*1024;
    pub(crate) fn seal_enum_values1025(&mut self)->Result<(),SaveError>{
        let values=crate::save_enum_values1025::collect(self)?;
        self.format_version=if values.is_empty(){15}else{ENUM_NOMINAL_CHECKPOINT_FORMAT_VERSION};
        self.enum_references1025=if values.is_empty(){None}else{Some(values)};Ok(())
    }
    pub(crate) fn validate_enum_resources1025(&self,resources:&rvn_parser::enum_asset_admission1025::EnumAssets1025)->Result<(),SaveError>{
        resources.validate_values(self.enum_references1025.iter().flatten()).map_err(SaveError::Incompatible)
    }
    fn validate_enum_wire1025(&self)->Result<(),SaveError>{
        let actual=crate::save_enum_values1025::collect(self)?;
        match (self.format_version,&self.enum_references1025){
            (15,None) if actual.is_empty()=>Ok(()),
            (ENUM_NOMINAL_CHECKPOINT_FORMAT_VERSION,Some(expected)) if !actual.is_empty()&&*expected==actual=>Ok(()),
            _=>Err(incompatible("Checkpoint15 cannot carry Enum; checkpoint16 requires the exact sorted nominal Value inventory")),
        }
    }
    pub fn encode(&self)->Result<String,SaveError>{
        self.validate_wire()?;let size=preflight_serialized(self,Self::MAX_BYTES)?;
        let mut writer=BoundedJsonWriter{bytes:Vec::with_capacity(size),limit:Self::MAX_BYTES};
        serde_json::to_writer(&mut writer,self)?;let json=String::from_utf8(writer.bytes).map_err(|_|incompatible("Save15 encoder produced non-UTF8"))?;
        preflight_json(&json)?;Ok(json)
    }
    pub fn decode(json:&str)->Result<Self,SaveError>{
        preflight_json(json)?;let mut decoder=serde_json::Deserializer::from_str(json);decoder.disable_recursion_limit();
        let save=Self::deserialize(&mut decoder)?;decoder.end()?;save.validate_wire()?;Ok(save)
    }
    fn validate_startup_trace_wire(&self)->Result<(),SaveError>{
        let Some(debugger)=&self.debugger else{return Ok(());};
        if let Some(startup)=&debugger.startup{startup.validate_callback_wire()?;}
        if let Some(widget)=&debugger.widget_materialization{widget.validate_wire().map_err(|error|incompatible(&error))?;}
        if [debugger.startup.is_some(),debugger.host_input.is_some(),debugger.frame_operation.is_some(),debugger.widget_materialization.is_some()].into_iter().filter(|present|*present).count()>1{return Err(incompatible("Saved debugger cannot combine independently owned operations"));}
        let waiting=debugger.startup.as_ref().and_then(|startup|startup.waiting_video_name());
        let imagemap=debugger.startup.as_ref().is_some_and(|startup|startup.has_imagemap_wait());
        let has_wait=waiting.is_some()||imagemap;
        let frame_entry = debugger.is_frame_entry_phase()
            && self.story.base.scene_execution.is_none()
            && (self.story.base.game_tasks.is_some() || self.story.base.game_audio.is_some());
        let widget_entry=debugger.is_widget_materialization_entry_phase();
        if (debugger.trace.is_none() && !has_wait && !frame_entry && !widget_entry)
            || (has_wait && debugger.trace.is_some()) {
            return Err(incompatible("Only an actual standalone frame/Widget materialization entry or executed initializer input/media wait may omit its computation trace"));
        }
        if has_wait{
            if waiting.is_some_and(|name|name.len()>128||!rvn_ui::composition::valid_name(name))||debugger.host_input.is_some()||debugger.frame_operation.is_some()||debugger.widget_materialization.is_some()
                ||debugger.entry_stop||debugger.stopped.is_some()||debugger.last_site.is_none()||debugger.work!=(0,0){
                return Err(incompatible("Executed initializer wait has a contradictory debugger phase"));
            }
        }
        Ok(())
    }
    fn validate_wire(&self)->Result<(),SaveError>{self.validate_startup_trace_wire()?;if let Some(rows)=&self.native_media_checkpoint{if rows.len()>8{return Err(incompatible("Native checkpoint exceeds8clips"));}let mut ids=std::collections::HashSet::new();for row in rows{row.validate().map_err(|e|incompatible(&e))?;if !ids.insert(&row.id){return Err(incompatible("Duplicate native media checkpoint"));}}}validate_witness_ast(&self.source.script)?;self.validate_enum_wire1025()?;self.text_tables.restore_digest().map_err(|e|incompatible(&e))?;if let Some(locale)=&self.locale{locale.restore_digest().map_err(|e|incompatible(&e))?;}if self.label.len()>8192||self.source.version!=1{return Err(incompatible("Invalid Save15 source/label version or bound"));}Ok(())}
}
impl crate::save::SaveManager {
    /// Explicit writer shares existing slot protection and atomic publication.
    pub fn save15_slot(&self,save:&Save15)->Result<(),SaveError>{self.check_slot(save.slot)?;self.check_writable(save.slot)?;self.write15_path(&self.slot_path(save.slot),&save.encode()?)}
    pub fn save15_autosave(&self,save:&Save15)->Result<(),SaveError>{self.write15_path(&self.autosave_path(),&save.encode()?)}
    pub fn save15_quicksave(&self,save:&Save15)->Result<(),SaveError>{self.write15_path(&self.quicksave_path(),&save.encode()?)}
    pub fn load15_slot(&self,slot:u32)->Result<Save15,SaveError>{self.check_slot(slot)?;self.read15_path(&self.slot_path(slot))}
    pub fn load15_autosave(&self)->Result<Save15,SaveError>{self.read15_path(&self.autosave_path())}
    pub fn load15_quicksave(&self)->Result<Save15,SaveError>{self.read15_path(&self.quicksave_path())}
    #[cfg(not(target_arch="wasm32"))]
    fn write15_path(&self,path:&std::path::Path,json:&str)->Result<(),SaveError>{self.write_atomic(path,json)}
    #[cfg(target_arch="wasm32")]
    fn write15_path(&self,path:&std::path::Path,json:&str)->Result<(),SaveError>{let name=path.file_stem().and_then(|s|s.to_str()).unwrap_or("save");Self::local_storage()?.set_item(&self.storage_key(name),json).map_err(|_|incompatible("Save15 localStorage publication failed"))}
    #[cfg(target_arch="wasm32")]
    fn read15_path(&self,path:&std::path::Path)->Result<Save15,SaveError>{let name=path.file_stem().and_then(|s|s.to_str()).unwrap_or("save");let json=Self::local_storage()?.get_item(&self.storage_key(name)).map_err(|_|incompatible("Save15 localStorage read failed"))?.ok_or(SaveError::SlotVide(0))?;Save15::decode(&json)}
    #[cfg(not(target_arch="wasm32"))]
    fn read15_path(&self,path:&std::path::Path)->Result<Save15,SaveError>{use std::io::Read;let mut bytes=Vec::new();std::fs::File::open(path)?.take(Save15::MAX_BYTES as u64+1).read_to_end(&mut bytes)?;let json=std::str::from_utf8(&bytes).map_err(|_|incompatible("Save15 is not UTF8"))?;Save15::decode(json)}
}

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableDebugState {
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub(crate) startup:Option<crate::engine::PortableStartup>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub(crate) host_input:Option<PortableHostInput>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub(crate) widget_materialization:Option<crate::engine::PortableWidgetMaterialization>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub(crate) frame_operation:Option<crate::engine::frame_continuation::PortableFrameOperation>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub(crate) trace:Option<crate::eval::debugger_eval::PortableTrace>,
    pub(crate) namespaces:Vec<PortableConsumerNamespace>,
    pub(crate) pc:usize,
    pub(crate) breakpoints:Vec<crate::editor_debugger::Breakpoint>,
    pub(crate) watches:Vec<crate::editor_debugger::Watch>,
    pub(crate) entry_stop:bool,
    pub(crate) stopped:Option<PortableStoppedVisit>,
    pub(crate) last_site:Option<crate::editor_debugger::Site>,
    pub(crate) work:(usize,usize),
}

impl PortableDebugState {
    pub(crate) fn is_widget_materialization_entry_phase(&self)->bool{
        self.startup.is_none()&&self.host_input.is_none()&&self.frame_operation.is_none()&&self.widget_materialization.is_some()
            &&self.entry_stop&&self.stopped.is_none()&&self.last_site.is_none()&&self.work==(0,0)
    }
    pub(crate) fn is_frame_entry_phase(&self) -> bool {
        self.startup.is_none() && self.host_input.is_none() && self.widget_materialization.is_none()
            && self.frame_operation.as_ref().is_some_and(|f| f.is_game_frame())
            && self.entry_stop && self.stopped.is_none() && self.last_site.is_none()
            && self.work == (0, 0)
    }
}

/// Logical stopped operation, never a persisted input capability. Its address
/// is admitted against the restored real registry and frozen event definition.
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableHostInput {
    pub(crate) target:crate::scene_resources::TargetIdentity,
    pub(crate) screen_ordinal:usize,
    pub(crate) handler:String,
    pub(crate) handler_ordinal:usize,
    pub(crate) screen:String,
    pub(crate) element:String,
    pub(crate) kind:rvn_ui::programmable::ScreenEventKind,
    pub(crate) value:Option<rvn_parser::Value>,
    pub(crate) key:Option<String>,
}

// Only typed runtime positions participate. User props and dictionaries are
// never interpreted as compiler metadata by their field names.
pub(crate) fn remap_instruction_origin(origin:&mut crate::program::InstructionOrigin,program:&ParsedProgram,export:bool)->Result<(),SaveError>{
    if origin.source.is_none()&&origin.span.is_none()&&origin.route.is_empty(){return Ok(());}
    let ordinal=if export{
        match &origin.source{
            Some(path)=>program.sources.iter().position(|source|source.path.as_ref()==Some(path))
                .ok_or_else(||incompatible("Saved origin has no actual frozen source ordinal"))?,
            None=>{
                let Some(crate::program::AstRoutePart::Root{index})=origin.route.first() else{return Err(incompatible("Unqualified memory origin has no actual authored root"));};
                let root=program.origins.get(*index).ok_or_else(||incompatible("Memory origin root is outside actual frozen program"))?;
                if root.source.is_some(){return Err(incompatible("Memory origin cannot adopt a file-backed authored root"));}
                let mut matches=program.sources.iter().enumerate().filter(|(_,source)|source.path.is_none());
                let ordinal=matches.next().ok_or_else(||incompatible("Memory origin has no actual captured source"))?.0;
                if matches.next().is_some(){return Err(incompatible("Memory origin source ordinal is ambiguous"));}
                ordinal
            }
        }
    }else{
        let Some(path)=&origin.source else{return Ok(());};
        path.to_str().and_then(|path|path.strip_prefix("rvn-source-ordinal:")).and_then(|ordinal|ordinal.parse::<usize>().ok())
            .ok_or_else(||incompatible("Portable origin carries an unqualified live path"))?
    };
    let actual=program.sources.get(ordinal).ok_or_else(||incompatible("Portable origin source ordinal is outside frozen closure"))?;
    origin.source=if export{Some(format!("rvn-source-ordinal:{ordinal}").into())}else{actual.path.clone()};
    Ok(())
}
fn remap_source_origins(state:&mut crate::GameState,program:&ParsedProgram,export:bool)->Result<(),SaveError>{
    fn position(position:&mut crate::scene::ResumePosition,program:&ParsedProgram,export:bool)->Result<(),SaveError>{
        if let Some(origin)=&mut position.origin{remap_instruction_origin(origin,program,export)?;}Ok(())
    }
    fn registry(registry:&mut crate::scene_resources::ResourceRegistry,program:&ParsedProgram,export:bool)->Result<(),SaveError>{
        for task in &mut registry.tasks{if let Some(wait)=&mut task.wait{position(wait,program,export)?;}}
        for wait in &mut registry.waits{position(&mut wait.position,program,export)?;}
        for wait in &mut registry.cancelled_waits{position(&mut wait.position,program,export)?;}
        Ok(())
    }
    if let Some(execution)=&mut state.scene_execution{
        position(&mut execution.live_position,program,export)?;
        if let Some(display)=&mut execution.display{position(&mut display.position,program,export)?;}
        for frame in &mut execution.frames{position(&mut frame.continuation,program,export)?;}
        execution.visit_native_loop_frames_mut(|frame|->Result<(),SaveError>{
            position(&mut frame.header,program,export)?;
            if let Some(site)=&mut frame.break_site{position(&mut site.position,program,export)?;}
            Ok(())
        })?;
        registry(&mut execution.resources,program,export)?;
    }
    if let Some(interfaces)=&mut state.ui.instance_registry{registry(interfaces,program,export)?;}
    Ok(())
}

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableConsumerNamespace {source_ordinal:usize,scope:String}
impl PortableConsumerNamespace {fn key(&self)->Result<String,SaveError>{use sha2::{Digest,Sha256};let hash=Sha256::digest(serde_json::to_vec(&(self.source_ordinal,&self.scope))?);Ok(hash.iter().map(|b|format!("{b:02x}")).collect())}}
fn remap_native_loop_namespaces(state:&mut crate::GameState,program:&ParsedProgram,script:&[Statement],binding:&ConsumerProgram,export:bool)->Result<(),SaveError>{
    let Some(execution)=&mut state.scene_execution else{return Ok(());};
    if !execution.has_native_loop_frames(){return Ok(());}
    // Existing source-ordinal/declared-scope mapping, never user dictionaries.
    let(_,live_to_logical)=portable_namespaces(program,script,binding)?;
    let mapping=if export{live_to_logical}else{
        let mut reverse=BTreeMap::new();for(live,logical)in live_to_logical{if reverse.insert(logical,live).is_some(){return Err(incompatible("Native loop portable namespace is ambiguous"));}}reverse
    };
    execution.visit_native_loop_frames_mut(|frame|{
        for record in &mut frame.gather{
            record.remap_loop_namespaces(&mapping).map_err(|error|incompatible(&error.to_string()))?;
        }
        Ok(())
    })
}
pub(crate) fn portable_namespaces(program:&ParsedProgram,script:&[Statement],binding:&ConsumerProgram)->Result<(Vec<PortableConsumerNamespace>,BTreeMap<String,String>),SaveError>{
    let mappings=scope_bindings(program,script,binding)?;let mut namespaces=mappings.iter().map(|m|PortableConsumerNamespace{source_ordinal:m.ordinal,scope:m.scope.clone()}).collect::<Vec<_>>();namespaces.sort_by(|a,b|(a.source_ordinal,&a.scope).cmp(&(b.source_ordinal,&b.scope)));
    let map=mappings.into_iter().map(|m|{let logical=PortableConsumerNamespace{source_ordinal:m.ordinal,scope:m.scope};Ok((m.live,logical.key()?))}).collect::<Result<BTreeMap<_,_>,SaveError>>()?;Ok((namespaces,map))
}

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableStoppedVisit {pub(crate) entry:bool,pub(crate) site:Option<crate::editor_debugger::Site>}

/// Only actual statement variants identify plans. User dictionary keys, strings,
/// expressions and retained values never participate in plan discovery.
pub(crate) fn visit_statement_plans<'a>(block:&'a [Statement],visit:&mut impl FnMut(&'a Statement)->Result<(),SaveError>)->Result<(),SaveError>{visit_statement_nodes(block,&mut |statement|if matches!(statement,Statement::ScheduledConsumer{..}){visit(statement)}else{Ok(())})}
fn visit_statement_nodes<'a>(block:&'a [Statement],visit:&mut impl FnMut(&'a Statement)->Result<(),SaveError>)->Result<(),SaveError>{
    let mut pending=vec![(block,0usize)];let mut nodes=0usize;
    while let Some((body,depth))=pending.pop(){if depth>256{return Err(incompatible("Portable statement traversal depth budget"));}for statement in body{nodes=nodes.checked_add(1).ok_or_else(||incompatible("Portable statement count overflow"))?;if nodes>100_000{return Err(incompatible("Portable statement traversal count budget"));}visit(statement)?;match statement{
        Statement::ScheduledConsumer{body,..}=>{pending.push((body,depth+1));},
        Statement::Function{body,..}|Statement::Init{body}|Statement::Screen{body,..}|Statement::Handler{body,..}|Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}=>pending.push((body,depth+1)),
        Statement::If{then_branch,else_branch,..}=>{pending.push((else_branch,depth+1));pending.push((then_branch,depth+1));},
        Statement::StandardMacroDispatch{dispatch}=>for branch in dispatch.branches.iter().rev(){pending.push((&branch.body,depth+1));},
        Statement::Sequence{branches}=>for body in branches.iter().rev(){pending.push((body,depth+1));},
        Statement::Choice{options}=>for option in options.iter().rev(){pending.push((&option.body,depth+1));},
        Statement::Imagemap{hotspots,..}=>for hotspot in hotspots.iter().rev(){pending.push((&hotspot.body,depth+1));},
        _=>{},
    }}}Ok(())
}

pub(crate) const MAX_JSON_DEPTH:usize=640;
const MAX_JSON_NODES:usize=8_000_000;
/// Count exact JSON bytes through a borrowed serializer before any output buffer
/// or source/player/frame clone. The count writer retains no serialized data.
pub(crate) fn preflight_serialized<T:Serialize+?Sized>(value:&T,limit:usize)->Result<usize,SaveError>{
    struct Count{bytes:usize,limit:usize,depth:usize,nodes:usize,string:bool,escape:bool,token:bool}
    impl std::io::Write for Count{fn write(&mut self,data:&[u8])->std::io::Result<usize>{self.bytes=self.bytes.checked_add(data.len()).filter(|n|*n<=self.limit).ok_or_else(||std::io::Error::other("Portable JSON byte budget exceeded before allocation"))?;
        for &byte in data{if self.string{if self.escape{self.escape=false;}else if byte==b'\\'{self.escape=true;}else if byte==b'"'{self.string=false;}continue;}
            match byte{b'"'=>{self.string=true;self.token=false;self.nodes+=1;},b'{'|b'['=>{self.depth+=1;self.nodes+=1;self.token=false;if self.depth>MAX_JSON_DEPTH{return Err(std::io::Error::other("Portable JSON depth budget exceeded before allocation"));}},b'}'|b']'=>{self.depth-=1;self.token=false;},b','|b':'|b' '|b'\t'|b'\n'|b'\r'=>self.token=false,_=>{if !self.token{self.nodes+=1;self.token=true;}}}
            if self.nodes>MAX_JSON_NODES{return Err(std::io::Error::other("Portable JSON node budget exceeded before allocation"));}
        }Ok(data.len())}fn flush(&mut self)->std::io::Result<()>{Ok(())}}
    let mut writer=Count{bytes:0,limit,depth:0,nodes:0,string:false,escape:false,token:false};serde_json::to_writer(&mut writer,value)?;Ok(writer.bytes)
}
struct BoundedJsonWriter{bytes:Vec<u8>,limit:usize}
impl std::io::Write for BoundedJsonWriter{fn write(&mut self,data:&[u8])->std::io::Result<usize>{let length=self.bytes.len().checked_add(data.len()).filter(|n|*n<=self.limit).ok_or_else(||std::io::Error::other("Portable JSON byte budget exceeded"))?;if length>self.bytes.capacity(){self.bytes.try_reserve_exact(length-self.bytes.len()).map_err(std::io::Error::other)?;}self.bytes.extend_from_slice(data);Ok(data.len())}fn flush(&mut self)->std::io::Result<()>{Ok(())}}
/// Scan the wire before serde allocation/recursive deserialization. String
/// escapes remain data, so braces and plan-like keys inside strings are inert.
fn preflight_json(json:&str)->Result<(),SaveError>{
    if json.len()>Save15::MAX_BYTES{return Err(incompatible("Save15 exceeds portable byte budget"));}
    let(mut depth,mut nodes,mut string,mut escape,mut token)=(0usize,0usize,false,false,false);
    for byte in json.bytes(){if string{if escape{escape=false;}else if byte==b'\\'{escape=true;}else if byte==b'"'{string=false;}continue;}
        match byte{
            b'"'=>{string=true;token=false;nodes+=1;},
            b'{'|b'['=>{depth=depth.checked_add(1).ok_or_else(||incompatible("Save15 JSON depth overflow"))?;if depth>MAX_JSON_DEPTH{return Err(incompatible("Save15 JSON depth budget exceeded before decode"));}nodes+=1;token=false;},
            b'}'|b']'=>{depth=depth.checked_sub(1).ok_or_else(||incompatible("Save15 JSON container mismatch"))?;token=false;},
            b','|b':'|b' '|b'\t'|b'\n'|b'\r'=>token=false,
            _=>{if !token{nodes+=1;token=true;}},
        }
        if nodes>MAX_JSON_NODES{return Err(incompatible("Save15 JSON node budget exceeded before decode"));}
    }
    if string||escape||depth!=0{return Err(incompatible("Save15 JSON wire is incomplete"));}Ok(())
}
pub(crate) fn preflight_story_state(state:&crate::GameState)->Result<(),SaveError>{
    crate::scene_resources::validate_current_resource_state(state).map_err(SaveError::Incompatible)?;
    let mut values=crate::scene::NarrativeBudget::default();values.values(&state.vars).map_err(SaveError::Incompatible)?;
    if let Some(frame)=&state.consumer_frame{frame.frame.validate().map_err(|e|incompatible(&e.to_string()))?;}
    if let Some(execution)=&state.scene_execution{
        if execution.frames.len()>crate::scene::MAX_NARRATIVE_FRAMES{return Err(incompatible("Portable caller count exceeds runtime bound"));}
        for activation in execution.active.iter().chain(execution.frames.iter().filter_map(|f|f.parked.as_ref())){values.values(&activation.locals).map_err(SaveError::Incompatible)?;if let Some(frame)=&activation.consumer_frame{frame.frame.validate().map_err(|e|incompatible(&e.to_string()))?;}}
    }preflight_serialized(state,Save15::MAX_BYTES)?;Ok(())
}

#[cfg(test)]
mod codec_tests {
    use super::*;
    #[test]
    fn typed_origin_mapping_promotes_captured_memory_source_without_touching_user_dictionary() {
        use rvn_parser::Value;
        let program=rvn_parser::parse_program("scene label start\n\"hello\"\n").unwrap();
        let mut engine=crate::Engine::from_program(program.clone(),crate::TerminalRenderer,8).unwrap();
        let data=Value::Dict(std::collections::BTreeMap::from([
            ("source".into(),Value::Str("user-owned source data".into())),
            ("span".into(),Value::Str("user-owned span".into())),
            ("route".into(),Value::List(vec![Value::Int(3)])),
            ("generated".into(),Value::Bool(false)),
        ]));
        engine.state.vars.insert("user".into(),data.clone());
        remap_source_origins(&mut engine.state,&program,true).unwrap();
        assert_eq!(engine.state.vars["user"],data);
        assert_eq!(engine.state.scene_execution.as_ref().unwrap().live_position.origin.as_ref().unwrap().source.as_deref(),Some(std::path::Path::new("rvn-source-ordinal:0")));
        let mut relocated=program;
        let path=Some(std::path::PathBuf::from("C:/actual/cold-source.rvn"));
        relocated.sources[0].path=path.clone();
        for origin in &mut relocated.origins{origin.source=path.clone();}
        remap_source_origins(&mut engine.state,&relocated,false).unwrap();
        assert_eq!(engine.state.vars["user"],data);
        assert_eq!(engine.state.scene_execution.as_ref().unwrap().live_position.origin.as_ref().unwrap().source,path);
    }
    #[test]
    fn consumer_save15_borrowed_byte_preflight_rejects_before_buffer_or_clone(){
        let data="x".repeat(100);assert!(preflight_serialized(&data,50).unwrap_err().to_string().contains("before allocation"));assert_eq!(preflight_serialized(&data,102).unwrap(),102);
    }
    #[test]
    fn consumer_save15_json_escaped_structure_is_data_and_extreme_depth_rejects(){
        preflight_json(r#"{"key":"[\\\"{ScheduledConsumer}]"}"#).unwrap();let bad=format!("{}0{}","[".repeat(1025),"]".repeat(1025));assert!(preflight_json(&bad).unwrap_err().to_string().contains("depth budget"));
    }
}

pub(crate) fn preflight_program(program:&ParsedProgram)->Result<(),SaveError>{
        validate_witness_ast(&program.script)?;
        preflight_serialized(&program.script,16*1024*1024)?;
        if program.sources.len()>100_000||program.origins.len()>100_000{return Err(incompatible("Portable source containers exceed bound before cloning"));}
        let mut source_bytes=0usize;for source in &program.sources{source_bytes=source_bytes.checked_add(source.source.len()).filter(|n|*n<=Save15::MAX_BYTES).ok_or_else(||incompatible("Portable source bytes exceed bound before cloning"))?;}
        Ok(())
}
fn validate_witness_ast(script:&[Statement])->Result<(),SaveError>{
    let mut expressions=Vec::new();
    visit_statement_nodes(script,&mut |statement|{match statement{
        // The historical parser accepts decimal f32 overflow. Runtime keeps
        // that route, but JSON cannot round-trip infinity as a typed float.
        Statement::Timer{duration_secs,..} if !duration_secs.is_finite()=>return Err(incompatible("Non-finite source timer is not a portable Save15 witness")),
        Statement::SpriteEffect{scale,rotation,..} if scale.is_some_and(|v|!v.is_finite())||rotation.is_some_and(|v|!v.is_finite())=>return Err(incompatible("Non-finite source sprite style is not a portable Save15 witness")),
        Statement::SpriteAnimate{params,..} if params.iter().any(|param|matches!(param.value,rvn_parser::AnimationValue::Float(value) if !value.is_finite()))=>return Err(incompatible("Non-finite source sprite animation is not a portable Save15 witness")),
Statement::MusicVolumeValue{level:value}|Statement::TypewriterSetValue{enabled:value}|Statement::TypewriterSpeedValue{chars_per_sec:value}|Statement::LocalVar{value,..}=>{expressions.push((value,0usize));},
Statement::WidgetAddToViewport{object,z_order}=>{expressions.push((object,0usize));expressions.push((z_order,0usize));},
Statement::WidgetRemoveFromParent{object}=>{expressions.push((object,0usize));},
Statement::UiOpen{name,arguments,modal,layer,..}=>{expressions.push((name,0usize));expressions.push((arguments,0usize));expressions.push((modal,0usize));expressions.push((layer,0usize));},
Statement::UiClose{name,..}=>{expressions.push((name,0usize));},
Statement::UiFocus{name,element,..}=>{expressions.push((name,0usize));expressions.push((element,0usize));},
Statement::UiSetState{name,element,state,..}=>{expressions.push((name,0usize));expressions.push((element,0usize));expressions.push((state,0usize));},
Statement::MenuExecute{request,..}=>{expressions.push((request,0usize));},
Statement::MotionPlay{target,definition,..}=>{expressions.push((target,0usize));expressions.push((definition,0usize));},
Statement::MotionStop{target,..}=>{expressions.push((target,0usize));},
Statement::MotionWait{target,..}=>{expressions.push((target,0usize));},
Statement::CharacterCompose{character,definition,..}=>{expressions.push((character,0usize));expressions.push((definition,0usize));},
Statement::CharacterAttributes{character,attributes,..}=>{expressions.push((character,0usize));expressions.push((attributes,0usize));},
Statement::VideoPlay{name,definition,..}=>{expressions.push((name,0usize));expressions.push((definition,0usize));},
Statement::VideoPause{name,..}=>{expressions.push((name,0usize));},
Statement::VideoResume{name,..}=>{expressions.push((name,0usize));},
Statement::VideoStop{name,..}=>{expressions.push((name,0usize));},
Statement::VideoSkip{name,..}=>{expressions.push((name,0usize));},
Statement::VideoSeek{name,seconds,..}=>{expressions.push((name,0usize));expressions.push((seconds,0usize));},
Statement::VideoVolume{name,volume,..}=>{expressions.push((name,0usize));expressions.push((volume,0usize));},
Statement::VideoWait{name,..}=>{expressions.push((name,0usize));},
Statement::AccessibilityConfigure{settings,..}=>{expressions.push((settings,0usize));},
Statement::AccessibilitySpeak{text,..}=>{expressions.push((text,0usize));},
Statement::FunctionReturn{value,..}=>{expressions.push((value,0usize));},
Statement::ForLoopWithBreak{first,last,..}=>{expressions.push((first,0usize));expressions.push((last,0usize));},
Statement::While{condition,..}=>{expressions.push((condition,0usize));},
Statement::ForEach{collection,..}=>{expressions.push((collection,0usize));},
Statement::SetVar{value,..}=>{expressions.push((value,0usize));},
Statement::If{condition,..}=>{expressions.push((condition,0usize));},
        Statement::StandardMacroDispatch{dispatch}=>{dispatch.validate().map_err(|error|incompatible(&error))?;for value in dispatch.operand_expressions(){expressions.push((value,0usize));}},
        Statement::ScheduledConsumer{steps,..}=>for step in steps{let(value,depth)=match step{rvn_parser::ConsumerStep::Capture{value,..}|rvn_parser::ConsumerStep::InspectTextTable{value,..}=>(value,0usize)};expressions.push((value,depth));},
        Statement::Dialogue{text,..}=>for segment in &text.0{if let rvn_parser::TextSegment::Interp(expr)=segment{expressions.push((expr,0));}},
        Statement::Choice{options}=>for option in options{if let Some(condition)=&option.condition{expressions.push((condition,0));}for segment in &option.label.0{if let rvn_parser::TextSegment::Interp(expr)=segment{expressions.push((expr,0));}}},
        _=>{},
    }Ok(())})?;
    let mut nodes=0usize;
    while let Some((expression,depth))=expressions.pop(){if depth>256{return Err(incompatible("Source AST expression nesting exceeds runtime256 before serialization"));}nodes+=1;if nodes>1_000_000{return Err(incompatible("Source AST expression count exceeds portable bound"));}match expression{
        rvn_parser::Expr::Neg(value)|rvn_parser::Expr::Not(value)=>expressions.push((value,depth+1)),
        rvn_parser::Expr::BinOp{left,right,..}|rvn_parser::Expr::And(left,right)|rvn_parser::Expr::Or(left,right)=>{expressions.push((right,depth+1));expressions.push((left,depth+1));},
        rvn_parser::Expr::Index{target,index}=>{expressions.push((index,depth+1));expressions.push((target,depth+1));},
        rvn_parser::Expr::Call{args,..}|rvn_parser::Expr::ListLit(args)=>for arg in args{expressions.push((arg,depth+1));},_=>{},
    }}Ok(())
}

impl PortableDebugState {
 pub(crate) fn export_widget_values(&mut self,admission:&crate::widget_reference_values_0974::WidgetValueAdmission)->Result<(),String>{
  use crate::widget_reference_values_0974::ReferencePhase;
  if let Some(trace)=&mut self.trace { trace.export_widget_values(admission)?; }
  if let Some(startup)=&mut self.startup { startup.visit_widget_value_roots_mut(&mut |value|admission.value(value,ReferencePhase::Export))?; }
  if let Some(frame)=&mut self.frame_operation { frame.visit_widget_value_roots_mut(&mut |value|admission.value(value,ReferencePhase::Export))?; }
  if let Some(input)=&mut self.host_input { if let Some(value)=&mut input.value { admission.value(value,ReferencePhase::Export)?; } }
  Ok(())
 }
 pub(crate) fn hydrate_widget_values(&mut self,admission:std::sync::Arc<crate::widget_reference_values_0974::WidgetValueAdmission>)->Result<(),String>{
  use crate::widget_reference_values_0974::ReferencePhase;
  if let Some(trace)=&mut self.trace { trace.admit_widget_values(admission.clone())?; }
  if let Some(startup)=&mut self.startup { startup.visit_widget_value_roots_mut(&mut |value|admission.value(value,ReferencePhase::Hydrate))?; }
  if let Some(frame)=&mut self.frame_operation { frame.visit_widget_value_roots_mut(&mut |value|admission.value(value,ReferencePhase::Hydrate))?; }
  if let Some(input)=&mut self.host_input { if let Some(value)=&mut input.value { admission.value(value,ReferencePhase::Hydrate)?; } }
  Ok(())
 }
}
