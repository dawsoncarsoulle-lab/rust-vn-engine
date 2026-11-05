//! Read-only metadata of one installed player, never Save data or input authority.
use crate::{scene::{ActivationId, ResumePosition}, scene_resources::TargetIdentity,
    scene_tasks::{TaskKey, TaskOwner}, GameState};
use serde::{Deserialize, Serialize, ser::{Error as _, SerializeSeq}};
use std::{collections::BTreeMap, io::Write};

pub use crate::editor_preview::MAX_INSPECTION_BYTES;
pub const MAX_INSPECTION_ROWS: usize = 8192;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameIdentity { pub instance:u64, pub epoch:u64, pub sequence:u64 }
impl FrameIdentity {
    pub fn of(scope:&crate::editor_preview::FrameScope)->Self {
        Self{instance:scope.instance,epoch:scope.epoch,sequence:scope.sequence}
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum InstanceKey {
    Game,
    Host{role:String},
    Activation{id:ActivationId},
    Frame{index:usize,owner:Option<ActivationId>,continuation:ResumePosition},
    Target{owner:TaskOwner,identity:TargetIdentity},
    Task{key:TaskKey,revision:u64,target:TargetIdentity},
    Timer{owner:TaskOwner,revision:u64},
    Interface{screen:String,order:u64,host_role:Option<rvn_ui::PageRole>,target:Option<TargetIdentity>},
    UiComponent{screen:String,order:u64,host_role:Option<rvn_ui::PageRole>,target:Option<TargetIdentity>,component:String},
    UiEntity{screen:String,order:u64,host_role:Option<rvn_ui::PageRole>,target:Option<TargetIdentity>,component:String,entity:u64},
    /// A physical renderer instance, identified by Bevy index and generation.
    /// Its logical association lives in the row; this grants no input authority.
    RenderedEntity{entity:u64},
}
impl InstanceKey {
    pub fn persists_across_images(&self)->bool { !matches!(self,Self::Frame{..}|Self::UiComponent{..}|Self::UiEntity{..}|Self::Interface{target:None,..}) }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instance {
    pub key:InstanceKey,pub parent:Option<InstanceKey>,pub label:String,
    pub parked:bool,pub details:serde_json::Value,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inspection {
    pub frame:FrameIdentity,pub position:ResumePosition,pub rows:Vec<Instance>,
    pub unavailable:Option<String>,
}

struct Counter{bytes:usize,limit:usize}
impl Write for Counter {
    fn write(&mut self,bytes:&[u8])->std::io::Result<usize>{
        self.bytes=self.bytes.checked_add(bytes.len()).filter(|bytes|*bytes<=self.limit)
            .ok_or_else(||std::io::Error::other("Inspection exceeds its byte budget"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self)->std::io::Result<()>{Ok(())}
}
struct Buffer{bytes:Vec<u8>,limit:usize}
impl Write for Buffer {
    fn write(&mut self,bytes:&[u8])->std::io::Result<usize>{
        let next=self.bytes.len().checked_add(bytes.len()).filter(|next|*next<=self.limit)
            .ok_or_else(||std::io::Error::other("Inspection exceeds its byte budget"))?;
        if next>self.bytes.capacity(){
            let capacity=next.max(self.bytes.capacity().saturating_mul(2)).min(self.limit);
            self.bytes.try_reserve_exact(capacity-self.bytes.len()).map_err(|_|std::io::Error::other("Inspection allocation refused"))?;
        }
        self.bytes.extend_from_slice(bytes);Ok(bytes.len())
    }
    fn flush(&mut self)->std::io::Result<()>{Ok(())}
}
pub fn encoded_size(value:&impl Serialize,limit:usize)->Result<usize,String>{
    let mut counter=Counter{bytes:0,limit:limit.min(MAX_INSPECTION_BYTES)};
    serde_json::to_writer(&mut counter,value).map_err(|error|error.to_string())?;Ok(counter.bytes)
}
pub fn bounded_json(value:&impl Serialize,limit:usize)->Result<Vec<u8>,String>{
    let limit=limit.min(MAX_INSPECTION_BYTES);
    // The borrowed pass checks the whole projected envelope before retaining
    // any runtime value. The second pass allocates only the bounded wire bytes.
    encoded_size(value,limit)?;
    let mut buffer=Buffer{bytes:Vec::new(),limit};
    serde_json::to_writer(&mut buffer,value).map_err(|error|error.to_string())?;Ok(buffer.bytes)
}

// Wire-equivalent borrowed keys avoid cloning alias/role/continuation strings
// while discovering whether this complete observation fits its budget.
#[derive(Serialize)]
#[serde(tag="kind",rename_all="snake_case")]
pub enum Key<'a> {
    Game, Host{role:&'a str}, Activation{id:ActivationId},
    Frame{index:usize,owner:Option<ActivationId>,continuation:&'a ResumePosition},
    Target{owner:&'a TaskOwner,identity:&'a TargetIdentity},
    Task{key:&'a TaskKey,revision:u64,target:&'a TargetIdentity},
    Timer{owner:&'a TaskOwner,revision:u64},
    Interface{screen:&'a str,order:u64,host_role:Option<rvn_ui::PageRole>,target:Option<&'a TargetIdentity>},
    UiComponent{screen:&'a str,order:u64,host_role:Option<rvn_ui::PageRole>,target:Option<&'a TargetIdentity>,component:&'a str},
    UiEntity{screen:&'a str,order:u64,host_role:Option<rvn_ui::PageRole>,target:Option<&'a TargetIdentity>,component:&'a str,entity:u64},
    RenderedEntity{entity:u64},
}
fn owner_parent(owner:&TaskOwner)->Key<'_>{
    match owner {TaskOwner::Game=>Key::Game,TaskOwner::Host{role}=>Key::Host{role},TaskOwner::Scene{activation}=>Key::Activation{id:*activation}}
}
#[derive(Serialize)]
pub struct Row<'a,D:Serialize>{pub key:Key<'a>,pub parent:Option<Key<'a>>,pub label:&'a str,pub parked:bool,pub details:D}
struct Rows<'a>(&'a GameState);
impl Serialize for Rows<'_> {
    fn serialize<S:serde::Serializer>(&self,serializer:S)->Result<S::Ok,S::Error>{
        let state=self.0;
        if state.ui.screens.len()>crate::scene_resources::MAX_INTERFACES{return Err(S::Error::custom("Inspection interface budget"));}
        if state.scene_execution.as_ref().is_some_and(|execution|execution.frames.len()>crate::scene::MAX_NARRATIVE_FRAMES
            ||execution.resources.targets.len()>crate::scene_resources::MAX_RESOURCES
            ||execution.resources.tasks.len()>crate::scene_resources::MAX_RESOURCES){return Err(S::Error::custom("Inspection resource budget"));}
        let mut seq=serializer.serialize_seq(None)?;let mut count=0usize;
        macro_rules! row{($value:expr)=>{{count+=1;if count>MAX_INSPECTION_ROWS{return Err(S::Error::custom("Inspection exceeds its instance budget"));}seq.serialize_element(&$value)?;}}}
        #[derive(Serialize)] struct Game<'a>{pc:usize,vars:&'a std::collections::HashMap<String,rvn_parser::Value>,random:&'a crate::random::RandomState,background:&'a str,music:&'a crate::MusicState}
        row!(Row{key:Key::Game,parent:None,label:"Game",parked:false,details:Game{pc:state.pc,vars:&state.vars,random:&state.random,background:&state.background_image,music:&state.music}});
        if let Some(execution)=&state.scene_execution {
            let mut roles=std::collections::BTreeSet::new();
            for record in &execution.resources.targets{if let TaskOwner::Host{role}=&record.owner{roles.insert(role.as_str());}}
            for role in roles{row!(Row{key:Key::Host{role},parent:Some(Key::Game),label:role,parked:false,details:role});}
            let game_owner=TaskOwner::Game;
            if let Some(timer)=&execution.game_tasks.timer{row!(Row{key:Key::Timer{owner:&game_owner,revision:timer.revision},parent:Some(Key::Game),label:"Timer",parked:false,details:timer});}
            #[derive(Serialize)] struct Activation<'a>{id:ActivationId,entry_pc:usize,locals:&'a std::collections::HashMap<String,rvn_parser::Value>,temporaries:&'a std::collections::HashMap<String,rvn_parser::Value>,resources_parked:bool}
            for (activation,parked) in execution.active.iter().map(|active|(active,false)).chain(execution.frames.iter().filter_map(|frame|frame.parked.as_ref()).map(|parked|(parked,true))) {
                row!(Row{key:Key::Activation{id:activation.id},parent:Some(Key::Game),label:&activation.scene,parked,
                    details:Activation{id:activation.id,entry_pc:activation.entry_pc,locals:&activation.locals,temporaries:&activation.temporaries,resources_parked:activation.tasks.resources_parked}});
                if let Some(timer)=&activation.tasks.timer{let owner=TaskOwner::Scene{activation:activation.id};row!(Row{key:Key::Timer{owner:&owner,revision:timer.revision},parent:Some(Key::Activation{id:activation.id}),label:"Timer",parked,details:timer});}
            }
            #[derive(Serialize)] struct Frame<'a>{kind:crate::scene::FrameKind,owner:Option<ActivationId>,called_activation:Option<ActivationId>,continuation:&'a ResumePosition}
            for (index,frame) in execution.frames.iter().enumerate(){
                row!(Row{key:Key::Frame{index,owner:frame.owner,continuation:&frame.continuation},parent:Some(frame.owner.map_or(Key::Game,|id|Key::Activation{id})),label:match frame.kind{crate::scene::FrameKind::Branch=>"Branch",crate::scene::FrameKind::Helper=>"Helper",crate::scene::FrameKind::SceneCall=>"Scene call"},parked:frame.parked.is_some(),details:Frame{kind:frame.kind,owner:frame.owner,called_activation:frame.called_activation,continuation:&frame.continuation}});
            }
            for record in &execution.resources.targets{
                row!(Row{key:Key::Target{owner:&record.owner,identity:&record.identity},parent:Some(owner_parent(&record.owner)),label:&record.identity.alias,
                    parked:matches!(&record.owner,TaskOwner::Scene{activation}if execution.owner()!=Some(*activation)),details:record});
            }
            for task in &execution.resources.tasks{
                let record=execution.resources.targets.iter().find(|record|record.identity==task.target).ok_or_else(||S::Error::custom("Inspection task has no exact resident target"))?;
                row!(Row{key:Key::Task{key:&task.key,revision:task.revision,target:&task.target},parent:Some(Key::Target{owner:&record.owner,identity:&record.identity}),label:&task.key.local_id,parked:task.parked,details:task});
            }
            #[derive(Serialize)] struct Screen<'a>{instance:&'a crate::ui::ScreenInstance,host_role:Option<rvn_ui::PageRole>,host_root:bool}
            for record in execution.resources.targets.iter().filter(|record|record.identity.kind==crate::scene_resources::TargetKind::Interface){
                // Existing residency validation is authoritative. Its small
                // identity copies are themselves preflighted before lookup.
                encoded_size(&(&record.owner,&record.identity),MAX_INSPECTION_BYTES).map_err(S::Error::custom)?;
                let resident=crate::ui_residency::resident_interface(state,&record.identity).map_err(S::Error::custom)?;
                let screen=resident.instance;
                row!(Row{key:Key::Interface{screen:&screen.name,order:screen.order,host_role:screen.host_role,target:Some(&record.identity)},parent:Some(Key::Target{owner:&record.owner,identity:&record.identity}),label:&screen.name,
                    parked:resident.residence==crate::ui_residency::ProjectionResidence::Parked,details:Screen{instance:screen,host_role:screen.host_role,host_root:screen.host_root}});
            }
        }else if let Some(registry)=&state.ui.instance_registry{
            registry.validate_interface_registry(&state.ui).map_err(S::Error::custom)?;
            let mut roles=std::collections::BTreeSet::new();for record in &registry.targets{if let TaskOwner::Host{role}=&record.owner{roles.insert(role.as_str());}}
            for role in roles{row!(Row{key:Key::Host{role},parent:Some(Key::Game),label:role,parked:false,details:role});}
            for record in &registry.targets{row!(Row{key:Key::Target{owner:&record.owner,identity:&record.identity},parent:Some(owner_parent(&record.owner)),label:&record.identity.alias,parked:false,details:record});}
            for task in &registry.tasks{let record=registry.targets.iter().find(|record|record.identity==task.target).ok_or_else(||S::Error::custom("Native UI inspection task has no target"))?;row!(Row{key:Key::Task{key:&task.key,revision:task.revision,target:&task.target},parent:Some(Key::Target{owner:&record.owner,identity:&record.identity}),label:&task.key.local_id,parked:false,details:task});}
            #[derive(Serialize)]struct Screen<'a>{instance:&'a crate::ui::ScreenInstance,host_role:Option<rvn_ui::PageRole>,host_root:bool}
            for record in &registry.targets{let resident=crate::ui_residency::resident_interface(state,&record.identity).map_err(S::Error::custom)?;let screen=resident.instance;row!(Row{key:Key::Interface{screen:&screen.name,order:screen.order,host_role:screen.host_role,target:Some(&record.identity)},parent:Some(Key::Target{owner:&record.owner,identity:&record.identity}),label:&screen.name,parked:false,details:Screen{instance:screen,host_role:screen.host_role,host_root:screen.host_root}});}
        }else{
            #[derive(Serialize)] struct Screen<'a>{instance:&'a crate::ui::ScreenInstance,host_role:Option<rvn_ui::PageRole>,host_root:bool}
            for screen in &state.ui.screens{row!(Row{key:Key::Interface{screen:&screen.name,order:screen.order,host_role:screen.host_role,target:None},parent:Some(Key::Game),label:&screen.name,parked:false,details:Screen{instance:screen,host_role:screen.host_role,host_root:screen.host_root}});}
        }
        seq.end()
    }
}

pub fn capture(state:&GameState,frame:FrameIdentity)->Inspection{
    let fallback=ResumePosition{pc:state.pc,origin:None};
    let position=state.scene_execution.as_ref().map_or(&fallback,|execution|&execution.live_position);
    #[derive(Serialize)] struct Projected<'a>{frame:&'a FrameIdentity,position:&'a ResumePosition,rows:Rows<'a>,unavailable:Option<&'a str>}
    let result=bounded_json(&Projected{frame:&frame,position:&position,rows:Rows(state),unavailable:None},MAX_INSPECTION_BYTES)
        .and_then(|bytes|serde_json::from_slice::<Inspection>(&bytes).map_err(|error|error.to_string()))
        .and_then(|inspection|{inspection.validate(false)?;Ok(inspection)});
    // The failed projection retains only a bounded source position. A corrupt
    // public state must not cause an unbounded origin clone on the error path.
    result.unwrap_or_else(|error|{let position=if encoded_size(position,4096).is_ok(){position.clone()}else{fallback.clone()};Inspection::unavailable(frame,position,&error)})
}
impl Inspection {
    pub fn unavailable(frame:FrameIdentity,position:ResumePosition,error:&str)->Self{
        let mut end=error.len().min(4096);while !error.is_char_boundary(end){end-=1;}
        Self{frame,position,rows:Vec::new(),unavailable:Some(error[..end].into())}
    }
    fn validate(&self,completed:bool)->Result<(),String>{
        if self.frame.instance==0||completed&&self.frame.sequence==0||self.rows.len()>MAX_INSPECTION_ROWS
            ||self.unavailable.as_ref().is_some_and(|error|error.len()>4096)||self.unavailable.is_some()&&!self.rows.is_empty(){return Err("Invalid player inspection".into());}
        if self.unavailable.is_some(){return Ok(());}
        let mut keys=BTreeMap::new();
        for (index,row) in self.rows.iter().enumerate(){
            if row.label.len()>4096||matches!(row.key,InstanceKey::Game)!=row.parent.is_none(){return Err("Invalid inspection root or label".into());}
            if keys.insert(bounded_json(&row.key,MAX_INSPECTION_BYTES)?,index).is_some(){return Err("Repeated inspection instance identity".into());}
        }
        if !keys.contains_key(&bounded_json(&InstanceKey::Game,MAX_INSPECTION_BYTES)?){return Err("Inspection has no Game root".into());}
        let parents=self.rows.iter().map(|row|row.parent.as_ref().map(|parent|{
            keys.get(&bounded_json(parent,MAX_INSPECTION_BYTES)?).copied().ok_or_else(||"Missing inspection parent".to_owned())
        }).transpose()).collect::<Result<Vec<_>,String>>()?;
        let mut colors=vec![0u8;self.rows.len()];
        for start in 0..self.rows.len(){
            if colors[start]!=0{continue;}let mut path=Vec::new();let mut current=Some(start);
            while let Some(index)=current{if colors[index]==2{break;}if colors[index]==1{return Err("Cyclic inspection parentage".into());}
                colors[index]=1;path.push(index);current=parents[index];}
            for index in path{colors[index]=2;}
        }
        Ok(())
    }
    pub fn encode_for(&self,frame:FrameIdentity)->Result<Vec<u8>,String>{
        if frame.instance!=self.frame.instance||frame.epoch!=self.frame.epoch||frame.sequence==0{return Err("Inspection belongs to a different installed player".into());}
        #[derive(Serialize)] struct Completed<'a>{frame:FrameIdentity,position:&'a ResumePosition,rows:&'a[Instance],unavailable:&'a Option<String>}
        bounded_json(&Completed{frame,position:&self.position,rows:&self.rows,unavailable:&self.unavailable},MAX_INSPECTION_BYTES)
    }
    /// Appends renderer-owned borrowed rows to the accepted logical image.
    /// The caller supplies the complete sequence (including existing rows),
    /// and all bytes are preflighted before any details are retained.
    pub fn project_rows(&self,frame:FrameIdentity,rows:&impl Serialize)->Result<Self,String>{
        if frame.instance!=self.frame.instance||frame.epoch!=self.frame.epoch||frame.sequence==0||self.unavailable.is_some(){return Err("Inspection has no matching installed player".into());}
        #[derive(Serialize)]struct Projected<'a,R:Serialize>{frame:FrameIdentity,position:&'a ResumePosition,rows:&'a R,unavailable:Option<&'a str>}
        let bytes=bounded_json(&Projected{frame,position:&self.position,rows,unavailable:None},MAX_INSPECTION_BYTES)?;
        Self::decode(&bytes)
    }
    pub fn has_interface(&self,screen:&str,order:u64,host_role:Option<rvn_ui::PageRole>,target:Option<&TargetIdentity>)->bool{
        self.unavailable.is_none()&&self.rows.iter().any(|row|!row.parked&&matches!(&row.key,InstanceKey::Interface{screen:name,order:actual,host_role:role,target:identity}
            if name==screen&&*actual==order&&*role==host_role&&identity.as_ref()==target))
    }
    pub fn has_resource_token(&self,token:&crate::scene_resources::ResourceToken)->bool{
        self.frame.epoch==token.task.runtime_epoch&&self.rows.iter().any(|row|matches!(&row.key,InstanceKey::Task{key,revision,target}
            if key==&token.task.key&&*revision==token.task.revision&&target==&token.target))
    }
    pub fn decode(bytes:&[u8])->Result<Self,String>{
        if bytes.len()>MAX_INSPECTION_BYTES{return Err("Inspection exceeds its byte budget".into());}
        let inspection:Self=serde_json::from_slice(bytes).map_err(|error|error.to_string())?;
        inspection.validate(true)?;Ok(inspection)
    }
    pub fn selected(&self,frame:&FrameIdentity,key:&InstanceKey)->Option<&Instance>{
        if &self.frame!=frame||self.unavailable.is_some(){return None;}
        self.rows.iter().find(|row|&row.key==key)
    }
}
