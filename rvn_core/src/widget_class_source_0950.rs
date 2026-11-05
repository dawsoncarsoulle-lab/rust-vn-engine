//! Compiler/source admission for Widget classes. This owns no second VM,
//! renderer, registry or instance collection. Metadata decoding is not a grant.
#[path="widget_macro_owner_1020.rs"]
pub mod standard_macros;
use rvn_parser::{widget_asset::{self,WidgetBlueprintV1},ParsedProgram,Statement,StatementOrigin};
use serde::{Serialize,Deserialize};
use std::collections::{BTreeMap,BTreeSet};
use sha2::{Digest,Sha256};

#[derive(Clone,Debug,PartialEq,Eq,Serialize)]
pub enum WidgetMemberKind{Screen,Handler,Function}
/// Typed compiler input only, deliberately not Deserialize. The Graph loader
/// obtains these rows from its opaque admitted SourceProject result; Core
/// independently checks every row against its actual frozen program below.
pub struct CompilerWidgetMember{pub graph:u64,pub kind:WidgetMemberKind,pub definition:String,pub root_index:usize,pub origin:StatementOrigin}
pub struct CompilerWidgetClass{pub index_uuid:String,pub locator:String,pub descriptor_bytes:Vec<u8>,pub members:Vec<CompilerWidgetMember>}

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidgetClassIdentity{pub uuid:String,pub source:String,pub source_ordinal:usize,pub definition:String,pub root_index:usize,pub receipt:[u8;32]}
/// Inert instance data. An acquired input source/wrapper/camera/user never
/// enters this structure. Existing ScreenInstance/ResourceRegistry own lifetime.
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidgetInstanceBinding{
 pub class:WidgetClassIdentity,pub viewport_attached:bool,
 /// Saved logical reconstruction work, never a physical handle/readiness grant.
 /// Restore inserts it only after exact trace hydration; its true owned VM
 /// operation clears it at a later completed transaction boundary.
 #[serde(default,skip_serializing_if="no_pending_materialization")]
 pub pending_materialization:bool,
}
fn no_pending_materialization(value:&bool)->bool{!*value}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidgetClassCatalogReceipt{pub source_content:[u8;32],pub classes:[u8;32]}
#[derive(Clone,Debug)]
pub struct AdmittedWidgetClass{pub(crate) identity:WidgetClassIdentity,descriptor:WidgetBlueprintV1,locator:String,members:Vec<(u64,WidgetMemberKind,String,usize)>}
impl AdmittedWidgetClass{pub fn identity(&self)->&WidgetClassIdentity{&self.identity}pub fn descriptor(&self)->&WidgetBlueprintV1{&self.descriptor}}
#[derive(Clone,Debug,Default)]
pub struct AdmittedWidgetClasses{classes:BTreeMap<String,AdmittedWidgetClass>,receipt:Option<WidgetClassCatalogReceipt>}
fn matches(statement:&Statement,kind:&WidgetMemberKind,name:&str)->bool{match(kind,statement){
 (WidgetMemberKind::Screen,Statement::Screen{name:actual,..})|(WidgetMemberKind::Handler,Statement::Handler{name:actual,..})|(WidgetMemberKind::Function,Statement::Function{name:actual,..})=>actual==name,_=>false}}
fn hash(value:&impl Serialize)->Result<[u8;32],String>{Ok(Sha256::digest(serde_json::to_vec(value).map_err(|e|e.to_string())?).into())}
impl AdmittedWidgetClasses{
 pub(crate) fn state_has_instances(state:&crate::GameState)->bool{
  state.ui.screens.iter().any(|instance|instance.widget_class.is_some())||state.scene_execution.as_ref().is_some_and(|execution|execution.active.iter().chain(execution.frames.iter().filter_map(|frame|frame.parked.as_ref())).any(|active|active.tasks.suspended_ui.screens.iter().any(|instance|instance.widget_class.is_some())))
 }
 pub fn admit(program:&ParsedProgram,entries:Vec<CompilerWidgetClass>)->Result<Self,String>{
  crate::save::consumer_checkpoint::preflight_program(program).map_err(|e|e.to_string())?;
  let source=crate::debugger_provenance::SourceMap::from_program(program)?;
  if entries.len()>256{return Err("Widget classes exceed 256 admitted descriptors".into());}
  let mut classes=BTreeMap::new();let mut locators=BTreeSet::new();let mut graphs=BTreeSet::new();let mut roots=BTreeSet::new();let mut total_bytes=0usize;let mut total_members=0usize;
  for entry in entries{
   widget_asset::validate_relative_locator(&entry.locator,Some("rvnwidget"))?;
   total_bytes=total_bytes.checked_add(entry.descriptor_bytes.len()).and_then(|n|n.checked_add(entry.locator.len())).filter(|n|*n<=4*1024*1024).ok_or("Widget class descriptor byte budget")?;
   let descriptor=WidgetBlueprintV1::decode(&entry.descriptor_bytes)?;
   if descriptor.uuid!=entry.index_uuid||classes.contains_key(&descriptor.uuid)||!locators.insert(entry.locator.to_lowercase()){return Err("Widget class UUID/index/locator is ambiguous".into());}
   total_members=total_members.checked_add(entry.members.len()).filter(|n|*n<=4096).ok_or("Widget class membership budget")?;
   if entry.members.len()!=descriptor.members.len(){return Err("Widget compiler membership differs from descriptor".into());}
   let mut member_ids=BTreeSet::new();let mut members=Vec::new();let mut screen=None;let mut class_source=None;
   for member in entry.members{
    if member.definition.is_empty()||member.definition.len()>128||!member_ids.insert(member.graph)||!descriptor.members.iter().any(|id|id.get()==member.graph){return Err("Widget member identity/name differs from descriptor".into());}
    let statement=program.script.get(member.root_index).ok_or("Widget definition is outside actual frozen program")?;
    let origin=program.origins.get(member.root_index).ok_or("Widget definition lost its actual frozen origin")?;
    if *origin!=member.origin||!matches(statement,&member.kind,&member.definition){return Err("Widget member AST/kind/name/origin differs from actual program".into());}
    let path=origin.source.as_ref().ok_or("Widget class requires actual physical source provenance")?;
    let ordinal=program.sources.iter().position(|file|file.path.as_ref()==Some(path)).ok_or("Widget source is not in the actual program closure")?;
    if class_source.replace(ordinal).is_some_and(|old|old!=ordinal){return Err("Widget members cross actual source ownership".into());}
    // The Graph adapter has already admitted project-relative locator to its
    // real SourceProject. Core additionally verifies the full source suffix,
    // never only a basename, and preserves the exact frozen origin ordinal.
    if !path.ends_with(std::path::Path::new(&descriptor.source)){return Err("Widget descriptor source differs from its frozen definition source".into());}
    if !graphs.insert((ordinal,member.graph))||!roots.insert(member.root_index){return Err("Widget classes alias a real graph or source definition".into());}
    if (member.kind==WidgetMemberKind::Screen)!=(member.graph==descriptor.screen_graph.get()){return Err("Widget descriptor must designate its one actual Screen".into());}
    if member.kind==WidgetMemberKind::Screen{if screen.replace((member.definition.clone(),member.root_index,ordinal)).is_some(){return Err("Widget class repeats a Screen".into());}}
    members.push((member.graph,member.kind,member.definition,member.root_index));
   }
   let(definition,root_index,source_ordinal)=screen.ok_or("Widget class lacks its actual Screen")?;
   members.sort_by_key(|member|member.0);
   let receipt=hash(&(source.content,&entry.index_uuid,&entry.locator,&entry.descriptor_bytes,&members))?;
   let identity=WidgetClassIdentity{uuid:descriptor.uuid.clone(),source:descriptor.source.clone(),source_ordinal,definition,root_index,receipt};
   classes.insert(descriptor.uuid.clone(),AdmittedWidgetClass{identity,descriptor,locator:entry.locator,members});
  }
  let receipt=if classes.is_empty(){None}else{Some(WidgetClassCatalogReceipt{source_content:source.content,classes:hash(&classes.iter().map(|(uuid,class)|(uuid,&class.identity.receipt)).collect::<Vec<_>>())?})};
  Ok(Self{classes,receipt})
 }
 pub fn is_empty(&self)->bool{self.classes.is_empty()}
 pub fn class(&self,uuid:&str)->Option<&AdmittedWidgetClass>{self.classes.get(uuid)}
 pub fn receipt(&self)->Option<&WidgetClassCatalogReceipt>{self.receipt.as_ref()}
 pub fn matches_program(&self,program:&ParsedProgram)->Result<bool,String>{
  let Some(receipt)=&self.receipt else{return Ok(true);};
  if crate::debugger_provenance::SourceMap::from_program(program)?.content!=receipt.source_content{return Ok(false);}
  // SourceMap.content intentionally permits staging under a different absolute
  // root. Keep that relocation contract, while refusing to move a class onto
  // another relative source with identical bytes or a matching display label.
  for class in self.classes.values(){
   let Some(file)=program.sources.get(class.identity.source_ordinal)else{return Ok(false);};
   if file.path.as_ref().is_none_or(|path|!path.ends_with(std::path::Path::new(&class.identity.source))){return Ok(false);}
   for(_,kind,name,index)in &class.members{
    if program.script.get(*index).is_none_or(|statement|!matches(statement,kind,name))||program.origins.get(*index).is_none_or(|origin|origin.source!=file.path){return Ok(false);}
   }
  }Ok(true)
 }
 /// A host callback on a typed class instance resolves a real admitted
 /// method, not an unrelated global Handler with the same display label.
 pub(crate) fn validate_owner_handler(&self,binding:Option<&WidgetInstanceBinding>,handler:&str)->Result<(),String>{
  let Some(binding)=binding else{return Ok(());};let class=self.class(&binding.class.uuid).ok_or("Widget callback class is not admitted")?;
  if binding.class!=class.identity||!class.members.iter().any(|(_,kind,name,_)|*kind==WidgetMemberKind::Handler&&name==handler){return Err("Widget callback is not a member of its admitted source class".into());}Ok(())
 }
 pub(crate) fn accepts_receipt(&self,receipt:Option<&WidgetClassCatalogReceipt>)->bool{self.receipt.as_ref()==receipt}
 pub(crate) fn validate_state(&self,state:&crate::GameState)->Result<(),String>{
  let validate=|ui:&crate::ui::UiState|->Result<(),String>{for instance in &ui.screens{if let Some(binding)=&instance.widget_class{let class=self.class(&binding.class.uuid).ok_or("Widget instance class is not admitted in this Engine")?;if binding.class!=class.identity||instance.host_role.is_some(){return Err("Widget instance class/source receipt differs from actual admission".into());}
   if binding.pending_materialization&&!binding.viewport_attached{return Err("Detached Widget cannot retain viewport materialization work".into());}
   // Saved/restored frames remain class methods too. Frozen global ownership
   // alone cannot admit a different class's valid Handler into this instance.
   for frame in instance.consumer_frames.values(){
    if frame.owner.source_ordinal!=class.identity.source_ordinal{return Err("Widget consumer frame differs from its admitted class source".into());}
    match &frame.owner.kind{
     crate::consumer_schedule::HostDefinitionKind::Handler=>self.validate_owner_handler(Some(binding),&frame.owner.definition)?,
     crate::consumer_schedule::HostDefinitionKind::Screen=>if frame.owner.definition!=class.identity.definition{return Err("Widget Screen frame differs from its admitted definition".into());},
    }
   }
   if instance.consumer_projection.as_ref().is_some_and(|projection|projection.owner.definition!=class.identity.definition||projection.owner.source_ordinal!=class.identity.source_ordinal){return Err("Widget projection differs from its admitted Screen source".into());}}}Ok(())};
  validate(&state.ui)?;if let Some(execution)=&state.scene_execution{for active in execution.active.iter().chain(execution.frames.iter().filter_map(|frame|frame.parked.as_ref())){validate(&active.tasks.suspended_ui)?;}}Ok(())
 }
}
