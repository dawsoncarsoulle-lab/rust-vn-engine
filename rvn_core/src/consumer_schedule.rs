//! First dependency of scheduled Blueprint pure evaluation.
//! The caller owns this frame. This module does not attach it to Engine saves,
//! SceneActivation, or a suspended debugger continuation.
use crate::eval::{EvalError, EvalResult};
use rvn_parser::{ConsumerStep, Expr, Name, Statement, Value};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[path="consumer_contract.rs"]
mod classic_contract;

pub(crate) const MAX_TERMINALS: usize = 1024;
const MAX_FRAME_BYTES: usize = 1_000_000;
fn invalid(message: &str) -> EvalError { EvalError::InvalidFunction(message.into()) }

/// Transient accounting rebuilt once from validated input, never serialized.
/// It travels with its frame when a computation enters/restores a function.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrameStorage {
    terminals: usize,
    pub bytes: usize,
}
impl FrameStorage {
    fn validate(self) -> EvalResult<Self> {
        if self.terminals > MAX_TERMINALS || self.bytes > MAX_FRAME_BYTES {
            return Err(EvalError::ExecutionLimit { limit: "1024 terminals and 1 000 000 bytes per consumer frame" });
        }
        Ok(self)
    }
}
fn terminal_bytes(producer: &str, terminal: &TextTableTerminals) -> usize {
    producer.len() + 2 * terminal.table_id.as_str().len() + terminal.key.len()
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextTableTerminals {
    pub valid: bool,
    pub table_id: Name,
    pub key: String,
}

/// Typed terminal storage for one real activation, never a process/global cache.
/// The source namespace must be unique inside this activation; future Engine
/// lowering must bind it to the exact ProgramSource/GraphInstructionOrigin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsumerFrame {
    version: u32,
    scopes: BTreeMap<String, BTreeMap<String, TextTableTerminals>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConsumerOwner { Game, Scene { activation: crate::scene::ActivationId } }

#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceRow {pub visible:bool,pub values:Vec<Value>}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundConsumerChoice {pub program:[u8;32],pub owner:ConsumerOwner,pub pc:usize,pub rows:Vec<ChoiceRow>}
impl BoundConsumerChoice {
    pub(crate) fn from_value(program:[u8;32],owner:ConsumerOwner,pc:usize,value:Value)->EvalResult<Self>{
        let Value::List(rows)=value else{return Err(invalid("Choice gather lost its actual row tuple"));};
        let rows=rows.into_iter().map(|row|match row{Value::List(mut fields)if fields.len()==2=>{let Value::List(values)=fields.pop().unwrap()else{return Err(invalid("Choice label lost its typed values"));};let Value::Bool(visible)=fields.pop().unwrap()else{return Err(invalid("Choice condition lost its decision"));};Ok(ChoiceRow{visible,values})},_=>Err(invalid("Choice row differs from real gather shape"))}).collect::<EvalResult<Vec<_>>>()?;
        Ok(Self{program,owner,pc,rows})
    }
    pub(crate) fn indices(&self)->Vec<usize>{self.rows.iter().enumerate().filter_map(|(i,row)|row.visible.then_some(i)).collect()}
}

/// Data for a real reached Imagemap input boundary. This is never an input
/// capability: the actual frozen program/site/activation is readmitted below.
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundConsumerImagemap {pub program:[u8;32],pub owner:ConsumerOwner,pub pc:usize}

/// Retained input ports of a real reached narrative Dialogue consumer.
/// This is data, not an Engine/input capability; admission rebinds the programme
/// and validates the actual site and activation before publication.
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundConsumerDialogue {
    pub program:[u8;32], pub owner:ConsumerOwner, pub pc:usize, pub values:Vec<Value>,
}
impl BoundConsumerDialogue {
    pub(crate) fn from_value(program:[u8;32],owner:ConsumerOwner,pc:usize,value:Value)->EvalResult<Self>{
        let Value::List(values)=value else{return Err(invalid("Dialogue gather lost its actual input ports"));};
        Ok(Self{program,owner,pc,values})
    }
}

/// State belongs to a real Engine game or SceneActivation, never declared keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundConsumerFrame {
    pub program: [u8; 32],
    pub owner: ConsumerOwner,
    pub frame: ConsumerFrame,
}
impl BoundConsumerFrame {
    pub(crate) fn new(program: [u8; 32], owner: ConsumerOwner) -> Self {
        Self { program, owner, frame: Default::default() }
    }
    pub(crate) fn validate_binding(&self, program: [u8;32], owner: ConsumerOwner) -> EvalResult<()> {
        if self.program != program || self.owner != owner {
            return Err(invalid("Scheduled consumer frame differs from its actual program/activation owner"));
        }
        Ok(())
    }
}

/// Frozen executable contract. Exact source provenance can be attached through
/// SourceMap at the parsed-program boundary; raw AST does not invent source bytes.
#[derive(Clone, Debug)]
pub(crate) struct ConsumerProgram {
    pub emitted: [u8;32],
    pub source: Option<ConsumerSourceBinding>,
    pub(crate) startup_input:Option<std::sync::Arc<startup_input_0996::StartupInputProgram>>,
    plans: HashSet<[u8;32]>,
    root_sites: BTreeMap<usize,[u8;32]>,
    choice_sites:BTreeMap<usize,Vec<usize>>,
    imagemap_sites:BTreeMap<usize,usize>,
    dialogue_sites:BTreeMap<usize,usize>,
    native_audio:bool,
    native_game_tasks:bool,
    historical_game_timers:BTreeSet<(u64,String)>,
    native_private_bindings:BTreeSet<String>,
}

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum HostDefinitionKind { Screen, Handler }
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct HostDefinitionSource {
    pub definition:String,
    pub source_ordinal:usize,
    pub kind:HostDefinitionKind,
    pub body:[u8;32],
    pub native_plans:bool,
    pub scopes:BTreeMap<String,BTreeSet<String>>,
}

/// Recalculable receipt from Core's existing frozen SourceMap contract.
/// Graph receives this from Core; it does not invent a competing fingerprint.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct ConsumerSourceBinding {
    pub emitted: [u8;32],
    pub closure: [u8;32],
    pub content: [u8;32],
    pub origins: Vec<crate::program::InstructionOrigin>,
    pub definitions:BTreeMap<String,HostDefinitionSource>,
}
impl HostDefinitionKind {pub fn qualified(&self,name:&str)->String{format!("{}:{name}",match self{Self::Screen=>"screen",Self::Handler=>"handler"})}}
impl ConsumerSourceBinding {
    pub fn definition(&self,kind:&HostDefinitionKind,name:&str)->Option<&HostDefinitionSource>{self.definitions.get(&kind.qualified(name))}
    pub fn from_program(program: &rvn_parser::ParsedProgram) -> EvalResult<Self> {
        let map = crate::debugger_provenance::SourceMap::from_program(program).map_err(|message|invalid(&message))?;
        let mut definitions=BTreeMap::new();
        for(index,statement)in program.script.iter().enumerate(){
            let(name,kind,body)=match statement{Statement::Screen{name,body,..}=>(name,HostDefinitionKind::Screen,body),Statement::Handler{name,body,..}=>(name,HostDefinitionKind::Handler,body),_=>continue};
            let origin=program.origins.get(index).ok_or_else(||invalid("Native definition has no frozen origin"))?;
            let ordinal=program.sources.iter().position(|source|source.path==origin.source).ok_or_else(||invalid("Native definition has no actual source ordinal"))?;
            let mut scopes:BTreeMap<String,BTreeSet<String>>=BTreeMap::new();let mut native_plans=false;let mut blocks=vec![body.as_slice()];
            while let Some(block)=blocks.pop(){for statement in block{
                if let Statement::ScheduledConsumer{version,scope,steps,..}=statement{
                    native_plans|=matches!(*version,2|3);
                    scopes.entry(scope.clone()).or_default();
                    for step in steps{if let ConsumerStep::InspectTextTable{producer,..}=step{scopes.entry(scope.clone()).or_default().insert(producer.clone());}}
                }
                blocks.extend(statement_children(statement));
            }}
            if definitions.insert(kind.qualified(name),HostDefinitionSource{definition:name.clone(),source_ordinal:ordinal,kind,body:fingerprint(body)?,native_plans,scopes}).is_some(){return Err(invalid("Repeated actual native definition"));}
        }
        Ok(Self { emitted: map.emitted, closure: map.closure, content: map.content, origins: map.origins,definitions })
    }
}
fn fingerprint(value: &impl Serialize) -> EvalResult<[u8;32]> {
    use sha2::{Digest, Sha256};
    struct Sink { bytes: usize, hash: Sha256 }
    impl std::io::Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.bytes = self.bytes.checked_add(bytes.len()).filter(|bytes| *bytes <= 16 * 1024 * 1024)
                .ok_or_else(|| std::io::Error::other("Frozen consumer program exceeds 16 MiB"))?;
            self.hash.update(bytes); Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }
    let mut sink = Sink { bytes: 0, hash: Sha256::new() };
    serde_json::to_writer(&mut sink, value).map_err(|error| invalid(&error.to_string()))?;
    Ok(sink.hash.finalize().into())
}
#[path="consumer_startup_input_0996.rs"]
mod startup_input_0996;
pub(crate) use startup_input_0996::StartupInputProgram;
impl ConsumerProgram {
    pub(crate) fn new(script: &[Statement]) -> EvalResult<Self> { Self::new_with_source(script,None) }
    pub(crate) fn new_with_source(script:&[Statement],source:Option<&ConsumerSourceBinding>)->EvalResult<Self>{
        Self::new_in(script,source,None,None)
    }
    /// Only the actual prepared Native source can admit its detached body.
    /// Raw AST/public no-source construction retains the historical gate.
    pub(crate) fn new_with_prepared_native(prepared:&crate::program::PreparedNarrative,source:Option<&ConsumerSourceBinding>)->EvalResult<Self>{
        Self::new_with_prepared_native_and_startup(prepared,source,None)
    }
    pub(crate) fn new_with_source_and_startup(script:&[Statement],source:Option<&ConsumerSourceBinding>,input:std::sync::Arc<StartupInputProgram>)->EvalResult<Self>{Self::new_in(script,source,None,Some(input))}
    pub(crate) fn new_with_prepared_native_and_startup(prepared:&crate::program::PreparedNarrative,source:Option<&ConsumerSourceBinding>,input:Option<std::sync::Arc<StartupInputProgram>>)->EvalResult<Self>{
        if prepared.native_loops.is_empty(){return Self::new_in(&prepared.script,source,None,input);}
        let source=source.ok_or_else(||invalid("Prepared Native narrative requires its actual frozen source binding"))?;
        if source.origins!=prepared.origins||source.emitted!=crate::debugger_provenance::SourceMap::emitted_identity(&prepared.script).map_err(|message|invalid(&message))?{
            return Err(invalid("Prepared Native narrative differs from the exact frozen emitted source"));
        }
        let authored=prepared.native_loops.authored().ok_or_else(||invalid("Prepared Native narrative lost its original authored source"))?;
        // Validate original lexical/context/capture contracts before admitting
        // any detached instruction. This temporary index never executes.
        Self::new_in(authored,Some(source),Some(authored),input.clone())?;
        for(pc,statement)in prepared.script.iter().enumerate(){
            if matches!(statement,Statement::ScheduledConsumer{version:3,..}){
                let body=prepared.native_loops.body(pc).ok_or_else(||invalid("Prepared Native root has no actual compiled body owner"))?;
                if prepared.origins[pc]!=body.header{return Err(invalid("Prepared Native root lost its actual compiler origin"));}
            }
        }
        Self::new_in(&prepared.script,Some(source),Some(authored),input)
    }
    fn new_in(script:&[Statement],source:Option<&ConsumerSourceBinding>,authored_native:Option<&[Statement]>,startup_input:Option<std::sync::Arc<StartupInputProgram>>)->EvalResult<Self>{
        if let Some(input)=&startup_input{input.validate(script,source)?;}
        rvn_parser::scheduled_source::native_loop_contract::validate_authored_bindings(authored_native.unwrap_or(script)).map_err(|e|invalid(&e))?;
        let mut plans = HashSet::new();
        let mut authored_plans=Vec::new();
        let mut native_private_bindings=BTreeSet::new();
        let mut root_sites = BTreeMap::new();
        let mut choice_sites=BTreeMap::new();
        let mut imagemap_sites=BTreeMap::new();
        let mut dialogue_sites=BTreeMap::new();
        let mut pending = vec![(script,false,false,false)];
        while let Some((block,lexical,init,host)) = pending.pop() {
            for statement in block {
                match statement {
                    Statement::ScheduledConsumer {version,scope,consumer,steps,body} => {
                        validate_plan(*version,scope,consumer,steps,body)?;
                        if *version==3 {
                            // Raw construction owns Init only. Compiled story
                            // admission must retain the actual authored source;
                            // lexical/host v3 still needs its own adapter.
                            let lexical_source_owner=lexical && source.is_some();
                            if source.is_none() || !(lexical_source_owner || !host && (init || authored_native.is_some())){return Err(invalid("Native demand v3 requires its actual frozen Init, prepared narrative or lexical Function/Handler/Screen owner"));}
                            let b=rvn_parser::scheduled_source::native_loop_contract::binding(statement).map_err(|e|invalid(&e))?.unwrap();
                            native_private_bindings.extend([b.index,b.cursor,b.phase]);
                        }
                        if rvn_parser::scheduled_source::dialogue_text_binding(statement).map_err(|e|invalid(&e))?.is_some() && source.is_none() {
                            return Err(invalid("Dynamic Dialogue binding requires its actual frozen source programme"));
                        }
                        if *version==2 {if let Some(command)=body.first(){
                            let site=if host{classic_contract::ClassicSite::HostDeclaration}else if lexical{classic_contract::ClassicSite::Computation}else if init{classic_contract::ClassicSite::Initializer{narrative_resources:script.iter().any(|s|matches!(s,Statement::SceneLabel{..})),game_tasks:source.is_some()&&crate::game_tasks::has_native_tasks(script)}}else{classic_contract::ClassicSite::Story};
                            if !(init&&!host&&!lexical&&matches!(command,Statement::Imagemap{..})&&startup_input.is_some()){
                                classic_contract::validate_site(command,site).map_err(|message|invalid(message))?;
                            }
                        }}
                        if *version==2 && lexical && body.first().is_some_and(crate::game_audio::is_audio_command){return Err(invalid("Native audio requires an actual story Game/Scene command owner; pure/host audio needs its own adapter"));}
                        if *version==2 && (lexical||init||host) && matches!(body.as_slice(),[Statement::Dialogue{..}]) {
                            return Err(invalid("Scheduled Dialogue requires its real narrative display/input site"));
                        }
                        if *version==2&&(lexical||init)&&matches!(body.as_slice(),[Statement::Choice{..}]){return Err(invalid("Scheduled Choice requires its real narrative input/display site"));}
                        if *version==2&&host&&source.is_none(){return Err(invalid("Native host consumer requires actual TargetIdentity/incarnation/definition frame adapter; declaration is not ownership"));}
                        if *version==2&&!host&&matches!(body.as_slice(),[Statement::MenuExecute{..}]){return Err(invalid("Native menu requires an actual trusted interface event source"));}
                        if !lexical && matches!(body.as_slice(),[Statement::FunctionReturn {..}]) {
                            return Err(invalid("Scheduled return requires a function activation"));
                        }
                        if init && !lexical && matches!(body.as_slice(),[Statement::LocalVar {..}]) {
                            return Err(invalid("Scheduled narrative local is unavailable during Init"));
                        }
                        authored_plans.push(statement);
                        pending.push((body,lexical,init,host));
                    },
                    Statement::Function {body,..} => pending.push((body,true,init,false)),
                    Statement::Screen {body,..} | Statement::Handler {body,..}=>pending.push((body,true,init,true)),
                    Statement::Init {body} => pending.push((body,lexical,true,host)),
                    Statement::While {body,..} | Statement::ForEach {body,..}
                    | Statement::ForLoopWithBreak {body,..} => pending.push((body,lexical,init,host)),
                    Statement::If {then_branch,else_branch,..} => { pending.push((then_branch,lexical,init,host)); pending.push((else_branch,lexical,init,host)); },
                    Statement::Sequence {branches} => pending.extend(branches.iter().map(|branch|(branch.as_slice(),lexical,init,host))),
                    Statement::StandardMacroDispatch{dispatch}=>pending.extend(dispatch.branches.iter().map(|branch|(branch.body.as_slice(),lexical,init,host))),
                    Statement::Choice {options} => pending.extend(options.iter().map(|option| (option.body.as_slice(),lexical,init,host))),
                    Statement::Imagemap {hotspots,..} => pending.extend(hotspots.iter().map(|hotspot| (hotspot.body.as_slice(),lexical,init,host))),
                    _ => {},
                }
            }
        }
        if !authored_plans.is_empty(){crate::save::consumer_checkpoint::preflight_serialized(script,16*1024*1024).map_err(|e|invalid(&e.to_string()))?;}
        for statement in authored_plans{plans.insert(fingerprint(statement)?);}
        for (pc, statement) in script.iter().enumerate() {
            if matches!(statement,Statement::ScheduledConsumer {..}) {
                root_sites.insert(pc,fingerprint(statement)?);
                if let Statement::ScheduledConsumer{body,..}=statement {if let[Statement::Dialogue{text,..}]=body.as_slice(){dialogue_sites.insert(pc,text.0.iter().filter(|p|matches!(p,rvn_parser::TextSegment::Interp(_))).count());}}
                if let Statement::ScheduledConsumer{body,..}=statement{if let[Statement::Choice{options}]=body.as_slice(){choice_sites.insert(pc,options.iter().map(|o|o.label.0.iter().filter(|s|matches!(s,rvn_parser::TextSegment::Interp(_))).count()).collect());}}
            }
        }
        for(pc,statement)in script.iter().enumerate(){
            if let Statement::ScheduledConsumer{body,..}=statement{if let[Statement::Imagemap{hotspots,..}]=body.as_slice(){
                if hotspots.len()>MAX_TERMINALS{return Err(invalid("Native Imagemap exceeds1024 actual hotspots"));}
                imagemap_sites.insert(pc,hotspots.len());
            }}
        }
        // Historical programs without additive Unit source retain their zero
        // digest and old size contract. Unit calls opt into the same existing
        // frozen-program fingerprint, without inventing a consumer frame.
        let unit_source=crate::eval::FunctionLibrary::contains_unit_source(script);
        if unit_source{crate::save::consumer_checkpoint::preflight_serialized(script,16*1024*1024).map_err(|e|invalid(&e.to_string()))?;}
        let emitted = if plans.is_empty()&&!unit_source { [0;32] } else { fingerprint(&script)? };
        Ok(Self { emitted, source: source.cloned(), startup_input, plans, root_sites,choice_sites,imagemap_sites,dialogue_sites,native_audio:crate::game_audio::has_native_audio(script),native_game_tasks:crate::game_tasks::has_native_tasks(script),historical_game_timers:crate::game_tasks::historical_timer_declarations(script),native_private_bindings })
    }
    pub(crate) fn validate_plan_binding(&self, statement: &Statement, pc: usize, init: bool) -> EvalResult<()> {
        let digest = fingerprint(statement)?;
        if if init { !self.plans.contains(&digest) } else { self.root_sites.get(&pc) != Some(&digest) } {
            return Err(invalid("Scheduled consumer plan differs from the frozen executable program"));
        }
        Ok(())
    }
    fn source_identity(&self) -> Option<([u8;32],[u8;32],[u8;32])> {
        self.source.as_ref().map(|source|(source.emitted,source.closure,source.content))
    }
    pub(crate) fn bound_scope(&self, declared: &str, pc: usize) -> EvalResult<String> {
        let owner_source = self.source.as_ref().and_then(|source|source.origins.get(pc)).and_then(|origin|origin.source.as_ref());
        let digest = fingerprint(&(self.emitted,self.source_identity(),owner_source,declared))?;
        Ok(digest.iter().map(|byte|format!("{byte:02x}")).collect())
    }
    pub(crate) fn identity(&self) -> EvalResult<[u8;32]> { fingerprint(&(self.emitted,self.source_identity())) }
    pub(crate) fn has_plans(&self) -> bool { !self.plans.is_empty() }
    // Unit statement calls retain the very same frozen executable digest even
    // without scheduled plans. No source grant or second program is created.
    pub(crate) fn validate_unit_emitted(&self,script:&[Statement])->EvalResult<()> {
        if self.emitted!=fingerprint(&script)? {return Err(invalid("Unit executable differs from its actual frozen Engine program"));}
        Ok(())
    }
    pub(crate) fn validate_emitted(&self, script: &[Statement]) -> EvalResult<()> {
        if self.has_plans() && self.emitted != fingerprint(&script)? {
            return Err(invalid("Scheduled executable program changed since its frozen binding"));
        }
        Ok(())
    }
    pub(crate) fn admits_historical_timer(&self,duration:f32,action:&str)->bool{
        self.historical_game_timers.contains(&(f64::from(duration).to_bits(),action.to_owned()))
    }
    pub(crate) fn validate_state_binding(&self,state: &crate::GameState) -> EvalResult<()> {
        crate::game_tasks::validate(state).map_err(|e|invalid(&e))?;
        if let Some(game)=state.game_tasks.as_ref().filter(|game|game.historical_timer){
            let timer=game.tasks.timer.as_ref().ok_or_else(||invalid("Historical Game timer lost its actual slot"))?;
            if !self.historical_game_timers.contains(&(timer.duration.to_bits(),timer.action.clone())){return Err(invalid("Historical timer provenance differs from the actual authored raw declaration"));}
        }
        if state.scene_execution.is_none() && state.game_tasks.is_some()!=self.native_game_tasks{return Err(invalid("Standalone Game task authority differs from actual frozen programme"));}
        if state.vars.keys().any(|name|self.native_private_bindings.contains(name)){
            return Err(invalid("Native lexical projection cannot be a serialized global variable"));
        }
        if let Some(execution)=&state.scene_execution{
            let carries_private=|values:&HashMap<String,Value>|values.keys().any(|name|self.native_private_bindings.contains(name));
            if carries_private(&execution.pre_scene_temporaries)
                ||execution.active.iter().chain(execution.frames.iter().filter_map(|frame|frame.parked.as_ref()))
                    .any(|active|carries_private(&active.locals)||carries_private(&active.temporaries))
                ||execution.frames.iter().filter_map(|frame|frame.pre_scene_temporaries.as_ref()).any(carries_private){
                return Err(invalid("Native source projection cannot be stored in active or parked narrative locals/temporaries"));
            }
        }
        if state.scene_execution.is_none() && state.game_audio.is_some()!=self.native_audio{return Err(invalid("Standalone Game audio registry differs from actual frozen programme"));}
        let program = self.identity()?;
        for dialogue in state.consumer_dialogue.iter().chain(state.scene_execution.as_ref().and_then(|e|e.display.as_ref()).and_then(|d|d.consumer_dialogue.as_ref())).chain(state.last_dialogue.as_ref().and_then(|d|d.consumer_dialogue.as_ref())) {self.validate_dialogue(dialogue)?;}
        if let Some(snapshot)=&state.last_dialogue {if snapshot.consumer_dialogue.as_ref().is_some_and(|dialogue|dialogue.pc!=snapshot.pc){return Err(invalid("Recorded Dialogue differs from actual frozen source position"));}}
        if let Some(dialogue)=&state.consumer_dialogue {if state.scene_execution.is_some()||dialogue.owner!=ConsumerOwner::Game{return Err(invalid("Game Dialogue has no actual Game owner"));}}
        if let Some(execution)=&state.scene_execution {if let Some(display)=&execution.display {if let Some(dialogue)=&display.consumer_dialogue {
            if display.consumer_choice.is_some()||dialogue.pc!=display.position.pc||!matches!(display.display,crate::rollback::HistoryDisplay::Dialogue{..}) {return Err(invalid("Dialogue payload differs from frozen display position/kind"));}
            if dialogue.pc==state.pc {let owner=execution.owner().map(|activation|ConsumerOwner::Scene{activation}).unwrap_or(ConsumerOwner::Game);if dialogue.owner!=owner{return Err(invalid("Live Dialogue belongs to another actual activation"));}}
        }}}
        if state.display_random_pc==Some(state.pc)&&self.dialogue_sites.contains_key(&state.pc) {
            let retained=state.consumer_dialogue.as_ref().or_else(||state.scene_execution.as_ref()?.display.as_ref()?.consumer_dialogue.as_ref());
            if !retained.is_some_and(|dialogue|dialogue.pc==state.pc) {return Err(invalid("Reached Dialogue requires its retained actual input ports"));}
            let frame=match retained.unwrap().owner {
                ConsumerOwner::Game=>state.consumer_frame.as_ref(),
                ConsumerOwner::Scene{activation}=>state.scene_execution.as_ref().and_then(|execution|execution.active.as_ref()).filter(|active|active.id==activation).and_then(|active|active.consumer_frame.as_ref()),
            };
            if frame.is_none(){return Err(invalid("Reached native Dialogue requires its actual activation frame"));}

        }
        if let Some(display)=state.scene_execution.as_ref().and_then(|e|e.display.as_ref()) {
            if self.dialogue_sites.contains_key(&display.position.pc)&&matches!(display.display,crate::rollback::HistoryDisplay::Dialogue{..})&&display.consumer_dialogue.is_none() {return Err(invalid("Frozen native Dialogue cannot omit retained actual ports"));}
        }
        if let Some(snapshot)=&state.last_dialogue {if self.dialogue_sites.contains_key(&snapshot.pc)&&snapshot.consumer_dialogue.is_none(){return Err(invalid("Recorded native Dialogue cannot omit retained actual ports"));}}
        for choice in state.consumer_choice.iter().chain(state.scene_execution.as_ref().and_then(|e|e.display.as_ref()).and_then(|d|d.consumer_choice.as_ref())){self.validate_choice(choice)?;}
        if let Some(choice)=&state.consumer_choice{if state.scene_execution.is_some()||choice.owner!=ConsumerOwner::Game{return Err(invalid("Legacy Choice has no actual Game owner"));}}
        if let Some(execution)=&state.scene_execution{if let Some(display)=&execution.display{if let Some(choice)=&display.consumer_choice{if choice.pc!=display.position.pc{return Err(invalid("Choice payload differs from its frozen display position"));}if choice.pc==state.pc{let owner=execution.owner().map(|activation|ConsumerOwner::Scene{activation}).unwrap_or(ConsumerOwner::Game);if choice.owner!=owner{return Err(invalid("Live Choice belongs to another actual activation"));}}}}}
        for data in state.consumer_imagemap.iter().chain(state.scene_execution.as_ref().and_then(|e|e.display.as_ref()).and_then(|d|d.consumer_imagemap.as_ref())){self.validate_imagemap(data)?;}
        if let Some(data)=&state.consumer_imagemap{if state.scene_execution.is_some()||data.owner!=ConsumerOwner::Game{return Err(invalid("Standalone Imagemap has no actual Game owner"));}}
        if let Some(execution)=&state.scene_execution{if let Some(display)=&execution.display{
            if let Some(data)=&display.consumer_imagemap{
                if data.pc!=display.position.pc||display.consumer_choice.is_some()||display.consumer_dialogue.is_some()||!matches!(display.display,crate::rollback::HistoryDisplay::Imagemap{..}){return Err(invalid("Imagemap payload differs from its frozen display position/kind"));}
                if data.pc==state.pc{let owner=execution.owner().map(|activation|ConsumerOwner::Scene{activation}).ok_or_else(||invalid("Live Imagemap has no actual active Scene owner"))?;if data.owner!=owner{return Err(invalid("Live Imagemap belongs to another actual activation"));}}
                if display.choice_indices!=(0..self.imagemap_sites[&data.pc]).collect::<Vec<_>>(){return Err(invalid("Imagemap indices differ from its actual frozen hotspots"));}
            }
            if self.imagemap_sites.contains_key(&display.position.pc)&&display.consumer_imagemap.is_none(){return Err(invalid("Recorded native Imagemap cannot omit its reached input visit"));}
        }}
        if state.display_random_pc==Some(state.pc)&&self.imagemap_sites.contains_key(&state.pc){
            let retained=state.consumer_imagemap.as_ref().or_else(||state.scene_execution.as_ref()?.display.as_ref()?.consumer_imagemap.as_ref());
            let data=retained.filter(|data|data.pc==state.pc).ok_or_else(||invalid("Reached Imagemap requires its actual retained visit"))?;
            let frame=match data.owner{ConsumerOwner::Game=>state.consumer_frame.as_ref(),ConsumerOwner::Scene{activation}=>state.scene_execution.as_ref().and_then(|e|e.active.as_ref()).filter(|active|active.id==activation).and_then(|active|active.consumer_frame.as_ref())};
            if frame.is_none(){return Err(invalid("Reached native Imagemap requires its actual activation frame"));}
        }
        if let Some(frame) = &state.consumer_frame { frame.validate_binding(program,ConsumerOwner::Game)?; }
        if let Some(execution) = &state.scene_execution {
            if execution.frames.len() > crate::scene::MAX_NARRATIVE_FRAMES {
                return Err(invalid("Too many narrative owners in consumer frame state"));
            }
            for active in execution.active.iter().chain(execution.frames.iter().filter_map(|frame|frame.parked.as_ref())) {
                if let Some(frame) = &active.consumer_frame { frame.validate_binding(program,ConsumerOwner::Scene {activation:active.id})?; }
            }
        }
        self.validate_instance_frames(state)?;
        Ok(())
    }
    pub(crate) fn validate_dialogue(&self,dialogue:&BoundConsumerDialogue)->EvalResult<()> {
        if dialogue.program!=self.identity()? {return Err(invalid("Dialogue display differs from actual frozen programme"));}
        let count=self.dialogue_sites.get(&dialogue.pc).ok_or_else(||invalid("Dialogue display has no actual consumer site"))?;
        if dialogue.values.len()!=*count {return Err(invalid("Dialogue ports differ from its actual authored text"));}
        let mut work=0usize;for value in &dialogue.values {work=work.checked_add(crate::value_limits::value_work(value)?).ok_or(EvalError::NumericOverflow)?;if work>1_000_000{return Err(invalid("Dialogue retained values exceed bounded display storage"));}}
        Ok(())
    }
    pub(crate) fn validate_imagemap(&self,data:&BoundConsumerImagemap)->EvalResult<()>{
        if data.program!=self.identity()?||!self.imagemap_sites.contains_key(&data.pc){return Err(invalid("Imagemap visit differs from its actual frozen program/site"));}
        Ok(())
    }
    pub(crate) fn validate_choice(&self,choice:&BoundConsumerChoice)->EvalResult<()>{
        if choice.program!=self.identity()?{return Err(invalid("Choice display differs from actual frozen programme"));}
        let counts=self.choice_sites.get(&choice.pc).ok_or_else(||invalid("Choice display has no actual consumer site"))?;
        if counts.len()!=choice.rows.len(){return Err(invalid("Choice rows differ from actual authored options"));}
        let mut work=0usize;for(row,count)in choice.rows.iter().zip(counts){if row.values.len()!=if row.visible{*count}else{0}{return Err(invalid("Choice typed ports differ from frozen visible label"));}for v in &row.values{work=work.checked_add(crate::value_limits::value_work(v)?).ok_or(EvalError::NumericOverflow)?;}if work>1_000_000{return Err(invalid("Choice retained values exceed bounded display storage"));}}
        Ok(())
    }
}

pub(crate) fn state_has_owned_consumers(state: &crate::GameState) -> bool {
    state.game_tasks.is_some() || state.consumer_imagemap.is_some() || state.scene_execution.as_ref().and_then(|e|e.display.as_ref()).is_some_and(|d|d.consumer_imagemap.is_some()) || state.game_audio.is_some() || state.consumer_dialogue.is_some() || state.last_dialogue.as_ref().is_some_and(|d|d.consumer_dialogue.is_some()) || state.scene_execution.as_ref().and_then(|e|e.display.as_ref()).is_some_and(|d|d.consumer_dialogue.is_some()) || state.consumer_choice.is_some() || state.consumer_frame.is_some() || crate::ui_consumer::has_instance_consumers(state) || state.scene_execution.as_ref().is_some_and(|execution|
        execution.active.as_ref().is_some_and(|active| active.consumer_frame.is_some())
        || execution.frames.iter().any(|frame| frame.parked.as_ref().is_some_and(|active| active.consumer_frame.is_some()||active.tasks.suspended_ui.screens.iter().any(|screen|!screen.consumer_frames.is_empty()))))
}
impl Default for ConsumerFrame {
    fn default() -> Self { Self { version: 1, scopes: BTreeMap::new() } }
}
impl ConsumerFrame {
    pub fn validate(&self) -> EvalResult<()> {
        self.storage().map(|_| ())
    }
    pub(crate) fn storage(&self) -> EvalResult<FrameStorage> {
        if self.version != 1 { return Err(invalid("Unsupported consumer frame version")); }
        if self.scopes.len() > MAX_TERMINALS { return Err(invalid("Too many consumer frame scopes")); }
        let mut count = 0usize;
        let mut bytes = 0usize;
        for (scope, terminals) in &self.scopes {
            identity(scope)?;
            bytes = bytes.checked_add(scope.len()).ok_or(EvalError::NumericOverflow)?;
            for (producer, terminal) in terminals {
                identity(producer)?;
                if terminal.key.len() > 4096 || terminal.key.contains('\0') {
                    return Err(invalid("Consumer terminal key exceeds 4096 bytes or contains NUL"));
                }
                count += 1;
                bytes = bytes.checked_add(terminal_bytes(producer, terminal))
                    .ok_or(EvalError::NumericOverflow)?;
                FrameStorage { terminals: count, bytes }.validate()?;
            }
        }
        FrameStorage { terminals: count, bytes }.validate()
    }
    pub fn terminals(&self, scope: &str, producer: &str) -> Option<&TextTableTerminals> {
        self.scopes.get(scope)?.get(producer)
    }
    pub(crate) fn inspect(&mut self, storage: &mut FrameStorage, scope: &str, producer: &str, value: Value) -> EvalResult<usize> {
        identity(scope)?;
        identity(producer)?;
        let valid = crate::localized_text::inspect_table_identity("__rvn_text_table_valid", std::slice::from_ref(&value))?;
        let Value::Bool(valid) = valid else { unreachable!("typed identity projection") };
        let updated = if valid {
            let Value::Name(table_id) = crate::localized_text::inspect_table_identity("__rvn_text_table_id", std::slice::from_ref(&value))?
                else { unreachable!("typed identity projection") };
            let Value::Str(key) = crate::localized_text::inspect_table_identity("__rvn_text_table_key", &[value])?
                else { unreachable!("typed identity projection") };
            TextTableTerminals { valid, table_id, key }
        } else {
            let mut retained = self.terminals(scope, producer).cloned().unwrap_or_default();
            retained.valid = false;
            retained
        };
        if updated.key.len() > 4096 || updated.key.contains('\0') {
            return Err(invalid("Consumer terminal key exceeds 4096 bytes or contains NUL"));
        }
        let old = self.terminals(scope, producer);
        let new_scope = !self.scopes.contains_key(scope);
        if new_scope && self.scopes.len() >= MAX_TERMINALS {
            return Err(invalid("Too many consumer frame scopes"));
        }
        let candidate = FrameStorage {
            terminals: storage.terminals + usize::from(old.is_none()),
            bytes: storage.bytes.checked_sub(old.map_or(0, |terminal| terminal_bytes(producer, terminal)))
                .and_then(|bytes| bytes.checked_add(terminal_bytes(producer, &updated)))
                .and_then(|bytes| bytes.checked_add(if new_scope { scope.len() } else { 0 }))
                .ok_or(EvalError::NumericOverflow)?,
        }.validate()?;
        let copied = scope.len() + terminal_bytes(producer, &updated);
        // Validate the complete replacement before changing either frame or counters.
        self.scopes.entry(scope.into()).or_default().insert(producer.into(), updated);
        *storage = candidate;
        Ok(copied)
    }
}

/// Copy one already evaluated port under the same bounded computation budget.
pub(crate) fn retained_dialogue_port(value:&Value,work:&mut usize)->EvalResult<Value>{
    let next=work.checked_add(crate::value_limits::value_work(value)?).ok_or(EvalError::NumericOverflow)?;
    if next>1_000_000{return Err(invalid("Dynamic Dialogue retained port exceeds computation storage"));}
    *work=next;Ok(value.clone())
}

fn identity(value: &str) -> EvalResult<()> {
    if value.is_empty() || value.len() > 256 || value.contains('\0') {
        return Err(invalid("Consumer identities require 1 to 256 non-null UTF-8 bytes"));
    }
    Ok(())
}

/// Validate dependency order even for a hand-built/deserialized AST.
pub(crate) fn validate_plan(version: u32, scope: &str, consumer: &str, steps: &[ConsumerStep], body: &[Statement]) -> EvalResult<()> {
    if !matches!(version,1|2|3) { return Err(invalid("Unsupported scheduled consumer contract version")); }
    if version==3 {
        let statement=Statement::ScheduledConsumer{version,scope:scope.into(),consumer:consumer.into(),steps:steps.to_vec(),body:body.to_vec()};
        rvn_parser::scheduled_source::native_loop_contract::binding(&statement).map_err(|e|invalid(&e))?;
        for phase in [rvn_parser::scheduled_source::native_loop_contract::Phase::First,rvn_parser::scheduled_source::native_loop_contract::Phase::Test] {
            if let Ok(root)=rvn_parser::scheduled_source::native_loop_contract::phase_expression(&statement,phase){
                rvn_parser::loop_phase_dependencies::capture_step_indices(steps,root).map_err(|e|invalid(&e))?;
            }
        }
    }
    identity(scope)?; identity(consumer)?;
    if steps.len() > MAX_TERMINALS { return Err(invalid("Too many scheduled producers")); }
    if version==1 && !matches!(body, [Statement::SetVar {..} | Statement::LocalVar {..} | Statement::FunctionReturn {..}]) {
        return Err(invalid("Scheduled consumer v1 requires exactly one set, local or return consumer"));
    }
    if version==2 && !v2_body_supported(body) {return Err(invalid("Scheduled consumer v2 requires one direct/control consumer, or an empty zero-step BranchEnd"));}
    if body.is_empty()&&!steps.is_empty(){return Err(invalid("Empty BranchEnd cannot gather producers"));}
    let mut captures = HashSet::new();
    let mut inspectors = HashSet::new();
    let mut nodes = 0usize;
    for step in steps {
        let (producer, value) = match step {
            ConsumerStep::Capture { producer, value } | ConsumerStep::InspectTextTable { producer, value } => (producer, value),
        };
        identity(producer)?;
        if captures.contains(producer) || inspectors.contains(producer) { return Err(invalid("Duplicate scheduled producer identity")); }
        validate_refs(value, &captures, &inspectors, 0, &mut nodes)?;
        match step {
            ConsumerStep::Capture {..} => { captures.insert(producer.clone()); }
            ConsumerStep::InspectTextTable {..} => { inspectors.insert(producer.clone()); }
        }
    }
    if let Some(statement)=body.first(){for value in direct_expressions(statement){validate_refs(value,&captures,&inspectors,0,&mut nodes)?;}
        let mut children=statement_children(statement);let empty=HashSet::new();
        while let Some(block)=children.pop(){for statement in block{if matches!(statement,Statement::ScheduledConsumer{..}|Statement::StandardMacroDispatch{..}){continue;}
            for value in direct_expressions(statement){validate_refs(value,&empty,&empty,0,&mut nodes)?;}children.extend(statement_children(statement));
        }}
    }Ok(())
}

fn v2_body_supported(body:&[Statement])->bool{rvn_parser::scheduled_consumer_body_supported(2,body)}
pub(crate) fn direct_expressions(statement:&Statement)->Vec<&Expr>{
 if let Some(inputs)=rvn_parser::widget_viewport_effect_1003::direct_inputs(statement){return inputs;}
 if let Ok(Some(input))=rvn_parser::widget_create_effect::parts(statement){return rvn_parser::widget_create_effect::direct_inputs(&input);}
 match statement{
 Statement::LibraryUnitCall{call}=>call.arguments().iter().map(|(_,value)|value).collect(),
 Statement::UnitCall{arguments,..}=>arguments.iter().collect(),
 Statement::StandardMacroDispatch{dispatch}=>dispatch.operand_expressions(),
 Statement::MusicVolumeValue{level:value}|Statement::TypewriterSetValue{enabled:value}|Statement::TypewriterSpeedValue{chars_per_sec:value}|Statement::SetVar{value,..}|Statement::LocalVar{value,..}|Statement::FunctionReturn{value}=>vec![value],
 Statement::If{condition,..}|Statement::While{condition,..}=>vec![condition],Statement::ForEach{collection,..}=>vec![collection],Statement::ForLoopWithBreak{first,last,..}=>vec![first,last],
 Statement::UiOpen{name,arguments,modal,layer,..}=>vec![name,arguments,modal,layer],Statement::UiClose{name}|Statement::VideoPause{name}|Statement::VideoResume{name}|Statement::VideoStop{name}|Statement::VideoSkip{name}|Statement::VideoWait{name}=>vec![name],
 Statement::UiFocus{name,element}=>vec![name,element],Statement::UiSetState{name,element,state}=>vec![name,element,state],Statement::MenuExecute{request}=>vec![request],Statement::MotionPlay{target,definition}=>vec![target,definition],Statement::MotionStop{target}|Statement::MotionWait{target}=>vec![target],Statement::CharacterCompose{character,definition}=>vec![character,definition],Statement::CharacterAttributes{character,attributes}=>vec![character,attributes],Statement::VideoPlay{name,definition}=>vec![name,definition],Statement::VideoSeek{name,seconds}=>vec![name,seconds],Statement::VideoVolume{name,volume}=>vec![name,volume],Statement::AccessibilityConfigure{settings}=>vec![settings],Statement::AccessibilitySpeak{text}=>vec![text],
 Statement::Dialogue{text,..}=>text.0.iter().filter_map(|v|if let rvn_parser::TextSegment::Interp(expr)=v{Some(expr)}else{None}).collect(),
 Statement::Choice{options}=>options.iter().flat_map(|o|o.condition.iter().chain(o.label.0.iter().filter_map(|v|if let rvn_parser::TextSegment::Interp(e)=v{Some(e)}else{None}))).collect(),_=>vec![]}}
pub(crate) fn statement_children(statement:&Statement)->Vec<&[Statement]>{match statement{
 Statement::FunctionLibrary{scope}=>scope.members().iter().map(|member|std::slice::from_ref(member.declaration())).collect(),
 Statement::UnitFunction{body,..}=>vec![body],
 Statement::ScheduledConsumer{body,..}|Statement::Init{body}|Statement::Function{body,..}|Statement::Screen{body,..}|Statement::Handler{body,..}|Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}=>vec![body],Statement::If{then_branch,else_branch,..}=>vec![then_branch,else_branch],Statement::Sequence{branches}=>branches.iter().map(Vec::as_slice).collect(),Statement::StandardMacroDispatch{dispatch}=>dispatch.branches.iter().map(|branch|branch.body.as_slice()).collect(),Statement::Choice{options}=>options.iter().map(|o|o.body.as_slice()).collect(),Statement::Imagemap{hotspots,..}=>hotspots.iter().map(|h|h.body.as_slice()).collect(),_=>vec![]}}
pub(crate) fn scalar_engine_input(statement:&Statement)->bool{if matches!(rvn_parser::widget_create_effect::parts(statement),Ok(Some(_))){return false;}matches!(statement,Statement::SetVar{..}|Statement::LocalVar{..}|Statement::If{..}|Statement::While{..}|Statement::MotionWait{..}|Statement::VideoWait{..})}
fn validate_refs(expr: &Expr, captures: &HashSet<String>, inspectors: &HashSet<String>, depth: usize, nodes: &mut usize) -> EvalResult<()> {
    if depth > 256 { return Err(invalid("Scheduled expression exceeds 256 levels")); }
    *nodes += 1;
    if *nodes > crate::eval::MAX_COMPUTATION_STEPS {
        return Err(EvalError::ExecutionLimit { limit: "100 000 expression nodes per scheduled consumer plan" });
    }
    match expr {
        Expr::LibraryCall{call}=>for(_,argument)in call.arguments(){validate_refs(argument,captures,inspectors,depth+1,nodes)?;},
        Expr::Call { name, args } => {
            match name.as_str() {
                "__rvn_captured_v1" => match args.as_slice() {
                    [Expr::Str(id)] if captures.contains(id) => {},
                    _ => return Err(invalid("Scheduled capture must reference an earlier producer by literal identity")),
                },
                "__rvn_terminal_v1" => match args.as_slice() {
                    [Expr::Str(id), Expr::Str(port)] if inspectors.contains(id) && matches!(port.as_str(), "valid"|"table_id"|"key") => {},
                    _ => return Err(invalid("Scheduled terminal must reference an earlier inspector and a typed port")),
                },
                _ => {},
            }
            for arg in args { validate_refs(arg, captures, inspectors, depth+1, nodes)?; }
        },
        Expr::Neg(expr) | Expr::Not(expr) => validate_refs(expr, captures, inspectors, depth+1, nodes)?,
        Expr::And(left,right) | Expr::Or(left,right) | Expr::BinOp {left,right,..} => {
            validate_refs(left,captures,inspectors,depth+1,nodes)?; validate_refs(right,captures,inspectors,depth+1,nodes)?;
        },
        Expr::Index {target,index} => {
            validate_refs(target,captures,inspectors,depth+1,nodes)?; validate_refs(index,captures,inspectors,depth+1,nodes)?;
        },
        Expr::ListLit(items) => for item in items { validate_refs(item,captures,inspectors,depth+1,nodes)?; },
        _ => {},
    }
    Ok(())
}

pub(crate) struct ActiveConsumer {
    pub scope: String,
    pub captures: BTreeMap<String, Value>,
    pub inspectors: HashSet<String>,
}

/// Legacy computation entry points cannot silently choose an activation owner.
pub(crate) fn contains_owned_plan_declarations(block:&[Statement])->bool{
    block.iter().any(|statement|matches!(statement,Statement::ScheduledConsumer{..})||statement_children(statement).into_iter().any(contains_owned_plan_declarations))
}
pub(crate) fn contains_scheduled_consumer(block: &[Statement]) -> bool {
    let mut pending = vec![block];
    while let Some(block) = pending.pop() {
        for statement in block {
            match statement {
                Statement::ScheduledConsumer {..} => return true,
                Statement::If {then_branch,else_branch,..} => { pending.push(then_branch); pending.push(else_branch); },
                Statement::While {body,..} | Statement::ForEach {body,..} | Statement::ForLoopWithBreak {body,..} | Statement::Init {body} => pending.push(body),
                Statement::Sequence {branches} => pending.extend(branches.iter().map(Vec::as_slice)),
                Statement::StandardMacroDispatch{dispatch}=>pending.extend(dispatch.branches.iter().map(|branch|branch.body.as_slice())),
                Statement::Choice {options} => pending.extend(options.iter().map(|option| option.body.as_slice())),
                Statement::Imagemap {hotspots,..} => pending.extend(hotspots.iter().map(|hotspot| hotspot.body.as_slice())),
                _ => {},
            }
        }
    }
    false
}

#[cfg(test)]
mod review_storage_tests {
    use super::*;
    fn table(key: &str) -> Value {
        Value::Text(Box::new(rvn_parser::localized_text::DeferredText::NameStringTable {
            reference: rvn_parser::localized_text::StringTableReference {
                table: "asset:Table".into(), key: key.into(),
            },
        }))
    }
    #[test]
    fn incremental_storage_matches_validated_frame_and_survives_replacement() {
        let mut frame = ConsumerFrame::default();
        let mut storage = frame.storage().unwrap();
        for index in 0..1024 {
            frame.inspect(&mut storage, "scope", &format!("producer-{index}"), table("key")).unwrap();
        }
        assert_eq!(storage, frame.storage().unwrap());
        frame.inspect(&mut storage, "scope", "producer-0", table("longer-key")).unwrap();
        assert_eq!(storage, frame.storage().unwrap());
        frame.inspect(&mut storage, "scope", "producer-0", Value::Str("literal".into())).unwrap();
        assert_eq!(storage, frame.storage().unwrap());
        assert_eq!(frame.terminals("scope", "producer-0").unwrap().key, "longer-key");
        let before = frame.clone(); let before_storage = storage;
        assert!(frame.inspect(&mut storage, "new-scope", "overflow", table("key")).is_err());
        assert_eq!(frame, before); assert_eq!(storage, before_storage);
    }
    #[test]
    fn replacement_over_byte_limit_preserves_old_terminal_and_accounting() {
        let mut frame = ConsumerFrame::default();
        let mut storage = FrameStorage::default();
        for index in 0..241 {
            frame.inspect(&mut storage, "scope", &format!("producer-{index}"), table(&"k".repeat(4000))).unwrap();
        }
        // Add validated empty scopes to approach the byte boundary without
        // consuming the terminal count. Deserialization is allowed to contain them.
        while storage.bytes + 256 <= MAX_FRAME_BYTES {
            let id = format!("{:0256}", frame.scopes.len());
            frame.scopes.insert(id, BTreeMap::new());
            storage = frame.storage().unwrap();
        }
        let remaining = MAX_FRAME_BYTES - storage.bytes;
        if remaining > 1 {
            frame.scopes.insert("x".repeat(remaining - 1), BTreeMap::new());
            storage = frame.storage().unwrap();
        }
        let before = frame.clone(); let before_storage = storage;
        assert!(frame.inspect(&mut storage, "scope", "producer-0", table(&"k".repeat(4096))).is_err());
        assert_eq!(frame, before); assert_eq!(storage, before_storage);
    }
    #[test]
    fn invalid_updated_key_does_not_create_scope_or_terminal() {
        let mut frame = ConsumerFrame::default(); let mut storage = FrameStorage::default();
        for key in ["k".repeat(4097), "null\0key".into()] {
            assert!(frame.inspect(&mut storage, "scope", "producer", table(&key)).is_err());
            assert_eq!(frame, ConsumerFrame::default());
            assert_eq!(storage, FrameStorage::default());
        }
    }
}

/// Structural selection only: no evaluation, branch pre-gather or authority.
/// Visits only syntax below top-level Init blocks. Auxiliary memory is O(depth),
/// work is O(visited Init syntax); source/owned admission keeps its own budgets.
/// There is deliberately no new global AST/execution limit for legacy programs.
pub(crate) fn init_requires_owned_continuation(script: &[Statement]) -> bool {
    enum Frame<'a> { Block(&'a [Statement]), Children(&'a Statement, usize) }
    fn child(statement: &Statement, index: usize) -> Option<&[Statement]> {
        match statement {
            Statement::ScheduledConsumer { body, .. } | Statement::Init { body }
            | Statement::Function { body, .. } | Statement::Screen { body, .. }
            | Statement::Handler { body, .. } | Statement::While { body, .. }
            | Statement::ForEach { body, .. } | Statement::ForLoopWithBreak { body, .. }
                => (index == 0).then_some(body.as_slice()),
            Statement::If { then_branch, else_branch, .. } => match index {
                0 => Some(then_branch), 1 => Some(else_branch), _ => None,
            },
            Statement::Sequence { branches } => branches.get(index).map(Vec::as_slice),
            Statement::StandardMacroDispatch{dispatch}=>dispatch.branches.get(index).map(|branch|branch.body.as_slice()),
            Statement::Choice { options } => options.get(index).map(|option| option.body.as_slice()),
            Statement::Imagemap { hotspots, .. } => hotspots.get(index).map(|hotspot| hotspot.body.as_slice()),
            _ => None,
        }
    }
    for root in script {
        let Statement::Init { body } = root else { continue };
        let mut pending = vec![Frame::Block(body)];
        while let Some(frame) = pending.last_mut() {
            match frame {
                Frame::Block(block) => {
                    let Some((statement, rest)) = block.split_first() else { pending.pop(); continue };
                    *block = rest;
                    if matches!(statement, Statement::ScheduledConsumer { version: 2|3, body, .. }
                        if matches!(body.as_slice(), [Statement::While { .. } | Statement::ForEach { .. } | Statement::ForLoopWithBreak { .. } | Statement::UnlockEnding { .. } | Statement::SpriteAnimate { .. } | Statement::SpriteStopAnimation { .. } | Statement::SpriteEffect { .. }])) {
                        return true;
                    }
                    // Keep only one borrowed child iterator; wide siblings are not collected.
                    if child(statement, 0).is_some() { pending.push(Frame::Children(statement, 0)); }
                }
                Frame::Children(statement, index) => {
                    if let Some(block) = child(statement, *index) {
                        *index += 1;
                        pending.push(Frame::Block(block));
                    } else { pending.pop(); }
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod init_selection_tests {
    use super::*;
    fn native_loop() -> Statement {
        Statement::ScheduledConsumer { version: 2, scope: "structural-only".into(),
            consumer: "not-an-authority".into(), steps: vec![],
            body: vec![Statement::While { condition: Expr::Bool(false), body: vec![] }] }
    }
    #[test]
    fn normal_selector_wide_branches_preserve_the_last_child_without_gather() {
        let mut branches = vec![vec![]; 20_000];
        assert!(!init_requires_owned_continuation(&[Statement::Init { body: vec![Statement::Sequence { branches: branches.clone() }] }]));
        branches.last_mut().unwrap().push(native_loop());
        assert!(init_requires_owned_continuation(&[Statement::Init { body: vec![Statement::Sequence { branches }] }]));
    }
    #[test]
    fn normal_selector_deep_legacy_init_gains_no_new_numeric_limit() {
        fn nested(mut statement: Statement) -> Statement {
            for _ in 0..300 { statement = Statement::If { condition: Expr::Bool(false), then_branch: vec![], else_branch: vec![statement] }; }
            Statement::Init { body: vec![statement] }
        }
        assert!(!init_requires_owned_continuation(&[nested(Statement::Sequence { branches: vec![] })]));
        assert!(init_requires_owned_continuation(&[nested(native_loop())]));
    }
    #[test]
    fn normal_selector_ignores_over100k_function_statements_outside_init() {
        let mut roots = (0..1001).map(|index| Statement::Function { name: format!("old_{index}"), parameters: vec![],
            body: vec![Statement::SetVar { name: "a".into(), value: Expr::Int(0) }; 100] }).collect::<Vec<_>>();
        if let Statement::Function { body, .. } = &mut roots[0] { body.push(native_loop()); }
        roots.push(Statement::Init { body: vec![Statement::SetVar { name: "old_inline".into(), value: Expr::Int(1) }] });
        assert!(!init_requires_owned_continuation(&roots));
        roots.push(Statement::Init { body: vec![native_loop()] });
        assert!(init_requires_owned_continuation(&roots));
    }
}
