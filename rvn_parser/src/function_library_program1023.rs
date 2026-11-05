//! Scoped Source values and qualified call transport. Definitions are the SAME
//! Function/UnitFunction AST used by the existing computation. No symbol alias,
//! body copy in the asset codec, evaluator, Source writer or publication grant.
use crate::{Expr, Statement};
use crate::function_library_asset1023::{FunctionLibraryAsset, FunctionLibraryId, FunctionLibraryMember, LibraryFunctionAccess, QualifiedLibraryFunction, bounded_library_vec, MAX_FUNCTION_LIBRARY_BYTES, MAX_FUNCTION_LIBRARY_MEMBERS, MAX_FUNCTION_LIBRARY_PARAMETERS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from="String",into="String")]
pub struct LibraryParameterId(String);
impl LibraryParameterId {
    pub fn parse(value:impl Into<String>)->Result<Self,String>{
        let value=value.into();crate::localized_text::validate_text_asset_id(&value)?;Ok(Self(value))
    }
    pub fn as_str(&self)->&str{&self.0}
}
impl TryFrom<String> for LibraryParameterId{type Error=String;fn try_from(value:String)->Result<Self,Self::Error>{Self::parse(value)}}
impl From<LibraryParameterId> for String{fn from(value:LibraryParameterId)->Self{value.0}}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from="MemberWire")]
pub struct LibraryMemberSource {
    graph_id:u64,
    parameter_ids:Vec<LibraryParameterId>,
    declaration:Box<Statement>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberWire{graph_id:u64,#[serde(deserialize_with="bounded_parameter_ids")] parameter_ids:Vec<LibraryParameterId>,declaration:Box<Statement>}
fn bounded_parameter_ids<'de,D:serde::Deserializer<'de>>(d:D)->Result<Vec<LibraryParameterId>,D::Error>{bounded_library_vec::<D,LibraryParameterId,MAX_FUNCTION_LIBRARY_PARAMETERS>(d)}
impl TryFrom<MemberWire> for LibraryMemberSource{
    type Error=String;
    fn try_from(value:MemberWire)->Result<Self,Self::Error>{Self::new(value.graph_id,value.parameter_ids,*value.declaration)}
}
impl LibraryMemberSource {
    pub fn new(graph_id:u64,parameter_ids:Vec<LibraryParameterId>,declaration:Statement)->Result<Self,String>{
        let member=Self{graph_id,parameter_ids,declaration:Box::new(declaration)};member.validate()?;Ok(member)
    }
    pub fn graph_id(&self)->u64{self.graph_id}
    pub fn declaration(&self)->&Statement{&self.declaration}
    pub fn parameter_ids(&self)->&[LibraryParameterId]{&self.parameter_ids}
    pub fn name(&self)->&str{match self.declaration.as_ref(){Statement::Function{name,..}|Statement::UnitFunction{name,..}=>name,_=>unreachable!("validated actual function")}}
    pub fn parameters(&self)->&[String]{match self.declaration.as_ref(){Statement::Function{parameters,..}|Statement::UnitFunction{parameters,..}=>parameters,_=>unreachable!("validated actual signature")}}
    pub fn validate(&self)->Result<(),String>{
        if self.graph_id==0{return Err("Library member requires its nonzero Source GraphId".into());}
        let (name,parameters)=match self.declaration.as_ref(){
            Statement::Function{name,parameters,..}|Statement::UnitFunction{name,parameters,..}=>(name,parameters),
            _=>return Err("Library scope cannot contain an Init, scene, Screen, Handler or another Library".into()),
        };
        if !crate::is_binding_name(name)||crate::builtin_arity(name).is_some(){return Err("Library member has an invalid or builtin-reserved name".into());}
        if parameters.len()>MAX_FUNCTION_LIBRARY_PARAMETERS||parameters.len()!=self.parameter_ids.len()||parameters.iter().any(|name|!crate::is_variable_binding_name(name))
            ||parameters.iter().collect::<BTreeSet<_>>().len()!=parameters.len()
            ||self.parameter_ids.iter().collect::<BTreeSet<_>>().len()!=self.parameter_ids.len(){
            return Err("Library parameter identities must match the actual ordered declaration, without duplicates".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from="ScopeWire")]
pub struct FunctionLibraryScope {
    library:FunctionLibraryId,
    members:Vec<LibraryMemberSource>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeWire{library:FunctionLibraryId,#[serde(deserialize_with="bounded_source_members")] members:Vec<LibraryMemberSource>}
fn bounded_source_members<'de,D:serde::Deserializer<'de>>(d:D)->Result<Vec<LibraryMemberSource>,D::Error>{bounded_library_vec::<D,LibraryMemberSource,MAX_FUNCTION_LIBRARY_MEMBERS>(d)}
impl TryFrom<ScopeWire> for FunctionLibraryScope{
    type Error=String;
    fn try_from(value:ScopeWire)->Result<Self,Self::Error>{Self::new(value.library,value.members)}
}
impl FunctionLibraryScope {
    pub fn new(library:FunctionLibraryId,members:Vec<LibraryMemberSource>)->Result<Self,String>{
        let scope=Self{library,members};scope.validate()?;Ok(scope)
    }
    pub fn library(&self)->&FunctionLibraryId{&self.library}
    pub fn members(&self)->&[LibraryMemberSource]{&self.members}
    pub fn validate(&self)->Result<(),String>{
        if self.members.len()>MAX_FUNCTION_LIBRARY_MEMBERS{return Err("Library scope exceeds the declared membership limit".into());}
        let(mut ids,mut names,mut parameters)=(BTreeSet::new(),BTreeSet::new(),BTreeSet::new());
        for member in &self.members{
            member.validate()?;
            if !ids.insert(member.graph_id)||!names.insert(member.name())
                ||member.parameter_ids.iter().any(|id|!parameters.insert(id)){
                return Err("Library scope repeats a member, local function name or parameter identity".into());
            }
        }Ok(())
    }
    pub fn validate_asset(&self,asset:&FunctionLibraryAsset)->Result<(),String>{
        self.validate()?;
        if &self.library!=asset.id()||self.members.iter().map(|member|member.graph_id).collect::<BTreeSet<_>>()
            !=asset.members().iter().map(|member|member.graph_id).collect(){
            return Err("Library Source and its original nominal asset have different exact membership".into());
        }
        for member in &self.members{
            let metadata=asset.members().iter().find(|row|row.graph_id==member.graph_id).expect("equal exact member sets");
            if metadata.pure && matches!(member.declaration.as_ref(),Statement::UnitFunction{..}){
                return Err("Completion-only Library member cannot be declared Pure".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryCallMode { Value, Completion }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from="CallWire")]
pub struct QualifiedLibraryCall {
    reference:QualifiedLibraryFunction,
    arguments:Vec<(LibraryParameterId,Expr)>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CallWire{reference:QualifiedLibraryFunction,#[serde(deserialize_with="bounded_call_arguments")] arguments:Vec<(LibraryParameterId,Expr)>}
fn bounded_call_arguments<'de,D:serde::Deserializer<'de>>(d:D)->Result<Vec<(LibraryParameterId,Expr)>,D::Error>{bounded_library_vec::<D,(LibraryParameterId,Expr),MAX_FUNCTION_LIBRARY_PARAMETERS>(d)}
impl TryFrom<CallWire> for QualifiedLibraryCall{
    type Error=String;
    fn try_from(value:CallWire)->Result<Self,Self::Error>{Self::new(value.reference,value.arguments)}
}
impl QualifiedLibraryCall {
    pub fn new(reference:QualifiedLibraryFunction,arguments:Vec<(LibraryParameterId,Expr)>)->Result<Self,String>{
        let call=Self{reference,arguments};call.validate()?;Ok(call)
    }
    pub fn reference(&self)->&QualifiedLibraryFunction{&self.reference}
    pub fn arguments(&self)->&[(LibraryParameterId,Expr)]{&self.arguments}
    pub fn validate(&self)->Result<(),String>{
        self.reference.validate()?;
        if self.arguments.len()>MAX_FUNCTION_LIBRARY_PARAMETERS{return Err("Library call exceeds the existing parameter bound".into());}
        if self.arguments.iter().map(|(id,_)|id).collect::<BTreeSet<_>>().len()!=self.arguments.len(){return Err("Qualified Library call repeats an actual parameter identity".into());}Ok(())
    }
}

/// Whole Source candidate, not an independent runtime table. All borrows point
/// at the one source scope and strict codec supplied by the owning transaction.
pub struct ScopedLibraryBindings<'a>{scopes:&'a[FunctionLibraryScope],assets:&'a[FunctionLibraryAsset]}
pub struct ResolvedLibrarySource<'a>{
    pub member:&'a LibraryMemberSource,
    pub metadata:&'a FunctionLibraryMember,
    pub arguments:Vec<&'a Expr>,
}
impl<'a> ScopedLibraryBindings<'a>{
    pub fn prepare(scopes:&'a[FunctionLibraryScope],assets:&'a[FunctionLibraryAsset])->Result<Self,String>{
        if scopes.len()>MAX_FUNCTION_LIBRARY_MEMBERS||assets.len()>MAX_FUNCTION_LIBRARY_MEMBERS{
            return Err("Library candidate exceeds the declared owner limit".into());
        }
        let mut member_count=0usize;
        for scope in scopes{
            member_count=member_count.checked_add(scope.members.len()).ok_or("Library member count overflow")?;
            if member_count>MAX_FUNCTION_LIBRARY_MEMBERS{return Err("Library candidate exceeds the declared membership limit".into());}
        }
        let(mut libraries,mut graphs,mut parameters)=(BTreeSet::new(),BTreeSet::new(),BTreeSet::new());
        for scope in scopes{
            scope.validate()?;
            if !libraries.insert(scope.library())||scope.members.iter().any(|member|!graphs.insert(member.graph_id)||member.parameter_ids.iter().any(|id|!parameters.insert(id))){
                return Err("Scoped Library Source repeats a nominal owner, GraphId or actual ParameterId".into());
            }
            let mut matches=assets.iter().filter(|asset|asset.id()==scope.library());
            let asset=matches.next().ok_or("Library Source has no original strict nominal asset")?;
            if matches.next().is_some(){return Err("Library asset is ambiguous".into());}scope.validate_asset(asset)?;
        }
        let mut ids=BTreeSet::new();
        for asset in assets{
            if !ids.insert(asset.id()){return Err("Nominal Library index repeats an owner".into());}
            if !asset.members().is_empty()&&!libraries.contains(asset.id()){return Err("Nonempty Library has no actual Source scope".into());}
        }
        Ok(Self{scopes,assets})
    }
    fn member(&self,reference:&QualifiedLibraryFunction)->Result<&'a LibraryMemberSource,String>{
        reference.validate()?;
        self.scopes.iter().find(|scope|scope.library()==&reference.library)
            .and_then(|scope|scope.members.iter().find(|member|member.graph_id==reference.graph_id))
            .ok_or_else(||"Original qualified Library/member is absent; no equal-name fallback exists".into())
    }
    /// Resolve identity, mode/access and all actual parameter IDs BEFORE the VM
    /// evaluates any argument. Returned expressions are original borrows, not
    /// evaluated Values or frozen work. Existing stack/budget executes them once.
    pub fn resolve(&self,call:&'a QualifiedLibraryCall,mode:LibraryCallMode,caller:Option<&QualifiedLibraryFunction>)->Result<ResolvedLibrarySource<'a>,String>{
        call.validate()?;let member=self.member(call.reference())?;
        let metadata=self.assets.iter().find(|asset|asset.id()==&call.reference.library)
            .and_then(|asset|asset.members().iter().find(|row|row.graph_id==call.reference.graph_id)).ok_or("Original Library member metadata absent")?;
        if let Some(caller)=caller{self.member(caller)?;}
        if metadata.access!=LibraryFunctionAccess::Public&&caller.is_none_or(|caller|caller.library!=call.reference.library){return Err("Private Library function is outside the actual caller scope".into());}
        let compatible=matches!((mode,member.declaration.as_ref()),(LibraryCallMode::Value,Statement::Function{..})|(LibraryCallMode::Completion,Statement::UnitFunction{..}));
        if !compatible||metadata.pure!=(mode==LibraryCallMode::Value){return Err("Library result/exec/Pure shape differs from this typed call".into());}
        if call.arguments.len()!=member.parameter_ids.len(){return Err("Qualified Library argument count differs from its real declaration".into());}
        let arguments=member.parameter_ids.iter().map(|id|call.arguments.iter().find(|(candidate,_)|candidate==id).map(|(_,expr)|expr)
            .ok_or_else(||"Qualified Library argument has a stale/foreign ParameterId".into())).collect::<Result<Vec<_>,String>>()?;
        Ok(ResolvedLibrarySource{member,metadata,arguments})
    }
}

/// The existing graph exporter supplies each exact function declaration text.
/// Reparse it using the SAME parser before embedding; source bytes are not
/// fabricated from a copied second body or a display-name replacement.
pub fn emit_library_scope(scope:&FunctionLibraryScope,mut declaration_source:impl FnMut(&LibraryMemberSource)->Result<String,String>)->Result<String,String>{
    scope.validate()?;
    let mut source=format!("functions.library_v1({}) {{\n",serde_json::to_string(scope.library.as_str()).map_err(|error|error.to_string())?);
    for member in &scope.members{
        let body=declaration_source(member)?;
        if body.len()>MAX_FUNCTION_LIBRARY_BYTES{return Err("Library declaration exceeds the Source byte limit".into());}
        let actual=crate::parse(&body).map_err(|error|error.to_string())?;
        if actual.as_slice()!=[member.declaration.as_ref().clone()]{return Err("Exported Library member text differs from its original actual declaration".into());}
        let ids=member.parameter_ids.iter().map(LibraryParameterId::as_str).collect::<Vec<_>>();
        let chunk=format!("functions.member_v1(\"{}\", {}) {{\n{}\n}}\n",member.graph_id,serde_json::to_string(&ids).map_err(|error|error.to_string())?,body);
        if source.len().checked_add(chunk.len()).is_none_or(|len|len>MAX_FUNCTION_LIBRARY_BYTES){return Err("Library scope exceeds the Source byte limit".into());}
        source.push_str(&chunk);
    }
    source.push_str("}\n");
    if source.len()>MAX_FUNCTION_LIBRARY_BYTES{return Err("Library scope exceeds the Source byte limit".into());}
    Ok(source)
}

#[cfg(test)]
#[path="function_library_program_tests1023.rs"]
mod tests;
