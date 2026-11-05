//! Pure typed Library participant for the existing Source transaction/recovery.
//! No filesystem access, graph body copy, writer, Registry or mutation grant.
use rvn_parser::function_library_asset1023::{FunctionLibraryAsset, FunctionLibraryId, MAX_FUNCTION_LIBRARY_BYTES, MAX_FUNCTION_LIBRARY_MEMBERS};
use rvn_parser::function_library_program1023::FunctionLibraryScope;
use rvn_parser::widget_asset::validate_relative_locator;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Two original UTF-8 JSON wires can each double in the enclosing JSON string;
/// relative locators use the existing 4096-byte bound and at most sixfold escaping.
pub const MAX_LIBRARY_RECOVERY_BYTES: usize = 4 * MAX_FUNCTION_LIBRARY_BYTES + 12 * 4096 + 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FunctionLibraryWire(String);
impl FunctionLibraryWire {
    pub fn read(wire: String) -> Result<Self, String> {
        FunctionLibraryAsset::decode(wire.as_bytes())?;
        Ok(Self(wire))
    }
    pub fn from_asset(asset: &FunctionLibraryAsset) -> Result<Self, String> {
        Self::read(String::from_utf8(asset.encode()?).map_err(|e| e.to_string())?)
    }
    pub fn as_str(&self) -> &str { &self.0 }
    pub fn asset(&self) -> Result<FunctionLibraryAsset, String> { FunctionLibraryAsset::decode(self.0.as_bytes()) }
}
impl<'de> Deserialize<'de> for FunctionLibraryWire {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = FunctionLibraryWire;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("exact bounded Library codec text") }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value,E> {
                FunctionLibraryAsset::decode(value.as_bytes()).map_err(E::custom)?;
                Ok(FunctionLibraryWire(value.to_owned()))
            }
        }
        d.deserialize_str(Visitor)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct LibraryLocator(String);
impl LibraryLocator {
    fn new(value: String) -> Result<Self,String> { validate_relative_locator(&value,Some("rvnlibrary"))?; Ok(Self(value)) }
}
impl<'de> Deserialize<'de> for LibraryLocator {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self,D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = LibraryLocator;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("a safe relative .rvnlibrary locator") }
            fn visit_str<E:serde::de::Error>(self, value:&str)->Result<Self::Value,E>{
                validate_relative_locator(value,Some("rvnlibrary")).map_err(E::custom)?;
                Ok(LibraryLocator(value.to_owned()))
            }
        }
        d.deserialize_str(Visitor)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryRecoveryState { locator: LibraryLocator, wire: FunctionLibraryWire }
impl LibraryRecoveryState {
    pub fn new(locator:String, wire:FunctionLibraryWire)->Result<Self,String>{Ok(Self{locator:LibraryLocator::new(locator)?,wire})}
    pub fn locator(&self)->&str{&self.locator.0}
    pub fn wire(&self)->&FunctionLibraryWire{&self.wire}
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionLibraryRecovery {
    id:FunctionLibraryId,
    before:Option<LibraryRecoveryState>,
    after:Option<LibraryRecoveryState>,
}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryWire{version:u32,id:FunctionLibraryId,before:Option<LibraryRecoveryState>,after:Option<LibraryRecoveryState>}
impl FunctionLibraryRecovery{
    pub fn new(id:FunctionLibraryId,before:Option<LibraryRecoveryState>,after:Option<LibraryRecoveryState>)->Result<Self,String>{
        for state in before.iter().chain(after.iter()){
            if state.wire.asset()?.id()!=&id{return Err("Library Recovery wire belongs to another original UUID".into());}
        }
        // None/None is an explicit net-zero unpublished removal state. It is
        // data only: asset birth itself is not an undoable editor transaction.
        Ok(Self{id,before,after})
    }
    pub fn id(&self)->&FunctionLibraryId{&self.id}
    pub fn before(&self)->Option<&LibraryRecoveryState>{self.before.as_ref()}
    pub fn after(&self)->Option<&LibraryRecoveryState>{self.after.as_ref()}
    pub fn decode(bytes:&[u8])->Result<Self,String>{
        if bytes.len()>MAX_LIBRARY_RECOVERY_BYTES{return Err("Library Recovery exceeds its exact wire byte budget".into());}
        #[derive(Deserialize)] struct VersionProbe{version:u32}
        let probe:VersionProbe=serde_json::from_slice(bytes).map_err(|e|e.to_string())?;
        if probe.version!=1{return Err("Unsupported Library Recovery version".into());}
        serde_json::from_slice(bytes).map_err(|e|e.to_string())
    }
    pub fn encode(&self)->Result<Vec<u8>,String>{
        let bytes=serde_json::to_vec_pretty(self).map_err(|e|e.to_string())?;
        if bytes.len()>MAX_LIBRARY_RECOVERY_BYTES{return Err("Library Recovery exceeds its exact wire byte budget".into());}
        Ok(bytes)
    }
    pub fn validate_source_scopes(&self,before:&[FunctionLibraryScope],after:&[FunctionLibraryScope])->Result<(),String>{
        self.validate_scope_side(self.before(),before)?;
        self.validate_scope_side(self.after(),after)
    }
    fn validate_scope_side(&self,state:Option<&LibraryRecoveryState>,scopes:&[FunctionLibraryScope])->Result<(),String>{
        let mut matches=scopes.iter().filter(|scope|scope.library()==&self.id);
        let scope=matches.next();
        if matches.next().is_some(){return Err("Library Source scope is ambiguous".into());}
        match (state,scope){
            (None,None)=>Ok(()),
            (Some(state),Some(scope))=>scope.validate_asset(&state.wire.asset()?),
            (Some(state),None) if state.wire.asset()?.members().is_empty()=>Ok(()),
            _=>Err("Library Recovery and actual Source scope have different presence/membership".into()),
        }
    }
}
impl Serialize for FunctionLibraryRecovery{
    fn serialize<S:serde::Serializer>(&self,s:S)->Result<S::Ok,S::Error>{
        RecoveryWire{version:1,id:self.id.clone(),before:self.before.clone(),after:self.after.clone()}.serialize(s)
    }
}
impl<'de> Deserialize<'de> for FunctionLibraryRecovery{
    fn deserialize<D:serde::Deserializer<'de>>(d:D)->Result<Self,D::Error>{
        let value=RecoveryWire::deserialize(d)?;
        if value.version!=1{return Err(serde::de::Error::custom("Unsupported Library Recovery version"));}
        Self::new(value.id,value.before,value.after).map_err(serde::de::Error::custom)
    }
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct FunctionLibraryBaseline{id:FunctionLibraryId,state:LibraryRecoveryState}
impl FunctionLibraryBaseline{
    pub fn new(id:FunctionLibraryId,state:LibraryRecoveryState)->Result<Self,String>{
        FunctionLibraryRecovery::new(id.clone(),Some(state.clone()),None)?;Ok(Self{id,state})
    }
    pub fn id(&self)->&FunctionLibraryId{&self.id}
    pub fn state(&self)->&LibraryRecoveryState{&self.state}
}
fn locator_key(locator:&str)->String{if cfg!(windows){locator.to_lowercase()}else{locator.to_owned()}}
/// Validate a full captured original baseline, never rediscover it from disk or
/// a current Registry. Returned rows are only a descriptive candidate index.
pub fn validate_set(baseline:&[FunctionLibraryBaseline],changes:&[FunctionLibraryRecovery])->Result<Vec<FunctionLibraryBaseline>,String>{
    if baseline.len()>MAX_FUNCTION_LIBRARY_MEMBERS||changes.len()>MAX_FUNCTION_LIBRARY_MEMBERS{return Err("Library owner set exceeds its declared membership bound".into());}
    let mut current=BTreeMap::new();let mut paths=BTreeSet::new();
    for row in baseline{
        FunctionLibraryBaseline::new(row.id.clone(),row.state.clone())?;
        if current.insert(row.id.clone(),row.clone()).is_some()||!paths.insert(locator_key(row.state.locator())){return Err("Library baseline repeats a UUID or physical locator".into());}
    }
    let mut ids=BTreeSet::new();let mut touched=BTreeMap::new();
    for change in changes{
        if !ids.insert(change.id.clone()){return Err("Library Recovery repeats the same UUID".into());}
        if current.get(&change.id).map(|row|&row.state)!=change.before(){return Err("Library Recovery BEFORE differs from the exact captured baseline".into());}
        for state in change.before.iter().chain(change.after.iter()){
            let key=locator_key(state.locator());
            if touched.get(&key).is_some_and(|owner|owner!=&change.id){return Err("Library Recovery groups collide at one physical locator".into());}
            touched.insert(key,change.id.clone());
            if baseline.iter().any(|row|row.id!=change.id&&locator_key(row.state.locator())==locator_key(state.locator())){return Err("Library target locator belongs to another baseline UUID".into());}
        }
    }
    for change in changes{
        current.remove(&change.id);
        if let Some(state)=&change.after{current.insert(change.id.clone(),FunctionLibraryBaseline::new(change.id.clone(),state.clone())?);}
    }
    let mut final_paths=BTreeSet::new();
    if current.values().any(|row|!final_paths.insert(locator_key(row.state.locator()))){return Err("Library candidate index repeats a physical locator".into());}
    Ok(current.into_values().collect())
}
/// Produce exact ordinary SourceRefactorFile plans. The existing Source publisher
/// must still perform canonical/reparse/permissions/readset/CAS/directory guards.
pub fn publication_files(changes:&[FunctionLibraryRecovery])->Result<Vec<super::SourceRefactorFile>,String>{
    if changes.len()>MAX_FUNCTION_LIBRARY_MEMBERS{return Err("Library change set exceeds its declared bound".into());}
    let mut files:BTreeMap<String,super::SourceRefactorFile>=BTreeMap::new();let mut owners=BTreeMap::new();let mut ids=BTreeSet::new();
    for change in changes{
        if !ids.insert(change.id.clone()){return Err("Library publication repeats an owner".into());}
        let mut local:BTreeMap<String,super::SourceRefactorFile>=BTreeMap::new();
        for (before,state) in [(true,change.before()),(false,change.after())]{
            if let Some(state)=state{
                let key=locator_key(state.locator());
                let entry=local.entry(key).or_insert_with(||super::SourceRefactorFile{path:state.locator().into(),before:None,after:None});
                // A case-only move cannot silently choose either original path.
                if entry.path!=state.locator(){return Err("Library case-only locator move requires an explicit filesystem rename plan".into());}
                if before{entry.before=Some(state.wire.as_str().into());}else{entry.after=Some(state.wire.as_str().into());}
            }
        }
        for (key,file) in local{
            if owners.insert(key.clone(),change.id.clone()).is_some(){return Err("Two Library owners collide in one publication plan".into());}
            if file.before!=file.after{files.insert(key,file);}
        }
    }
    Ok(files.into_values().collect())
}
/// For unrelated writer continuations: every original BEFORE remains exact.
/// A real operation may alter AFTER, including explicit net-zero UndoBirth.
pub fn validate_continuation(previous:&[FunctionLibraryRecovery],next:&[FunctionLibraryRecovery])->Result<(),String>{
    if previous.len()>MAX_FUNCTION_LIBRARY_MEMBERS||next.len()>MAX_FUNCTION_LIBRARY_MEMBERS{return Err("Library Recovery continuation exceeds its bound".into());}
    let mut original_ids=BTreeSet::new();
    if previous.iter().any(|entry|!original_ids.insert(entry.id())){return Err("Previous Library Recovery repeats an owner".into());}
    let mut seen=BTreeSet::new();
    if next.iter().any(|entry|!seen.insert(entry.id())){return Err("Library Recovery continuation repeats an owner".into());}
    for old in previous{
        let new=next.iter().find(|entry|entry.id==old.id).ok_or("A writer dropped retained Library Recovery")?;
        if old.before!=new.before{return Err("A writer replaced the original Library BEFORE baseline".into());}
    }
    Ok(())
}
#[cfg(test)]
#[path="source_function_library_recovery_tests1023.rs"]
mod tests;
