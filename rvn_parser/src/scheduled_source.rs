//! Original scheduled-step bytes. These records are observations of a parsed
//! program, never executable statements or a replacement AST.
use crate::{lexer::Token, ConsumerStep, Expr, ParsedProgram, Statement, StatementSpan};
use logos::Logos;
#[cfg(test)]
#[path="native_string_sequence_source_tests_0761.rs"]
mod native_string_sequences0761;
#[path="native_loop_contract.rs"]
pub mod native_loop_contract;
#[path="standard_macro_contract.rs"]
pub mod standard_macro_contract;
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, BTreeSet}, ops::Range};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduledSourceRoute { LibraryMember(usize), Statement(usize), Body, Then, Else, Branch(usize), Choice(usize), Hotspot(usize) }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredPort { pub role:String, pub pin:u64, pub incoming:Option<u64>,
    #[serde(default,skip_serializing_if="Option::is_none")] pub source_pin:Option<u64>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub exec_incoming:Option<Vec<crate::scheduled_exec_links_v1::Incoming>> }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredEndpoint {
    pub graph:u64, pub node:u64, pub kind:String, pub ports:Vec<DeclaredPort>,
    pub legacy_terminal:Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub native_loop_output:Option<String>,
    #[serde(default,skip_serializing_if="is_false")]
    pub legacy_string_sequence:bool,
    #[serde(default,skip_serializing_if="is_false")]
    pub exec_links_v1:bool,
}
fn is_false(value:&bool)->bool{!*value}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduledStepSource {
    pub source_ordinal:usize, pub definition_root_ordinal:usize,
    pub scope:Option<String>, pub route:Vec<ScheduledSourceRoute>,
    pub consumer:Range<usize>, pub index:usize, pub span:Range<usize>,
    pub plan_scope:String, pub consumer_identity:String, pub producer_identity:String,
    pub expression:Range<usize>, pub endpoint:Option<DeclaredEndpoint>,
}
fn expression(raw:&str)->Result<Expr,String>{
    match crate::parse(&format!("set __origin_probe = {raw}\n")).map_err(|e|e.to_string())?.as_slice(){
        [Statement::SetVar{value,..}]=>Ok(value.clone()),_=>Err("Producer probe was not exactly one expression".into()),
    }
}
fn ranges(raw:&str,span:&StatementSpan,steps:&[ConsumerStep])->Result<Vec<(Range<usize>,Range<usize>)>,String>{
    if steps.is_empty(){return Ok(vec![]);}
    let end=span.children.first().ok_or("Producer has no actual body span")?.range.start;
    ranges_region(raw, span.range.start, end, steps, 1)
}
fn ranges_region(raw:&str, offset:usize, end:usize, steps:&[ConsumerStep], marker_depth:usize)->Result<Vec<(Range<usize>,Range<usize>)>,String>{
    if steps.is_empty(){return Ok(vec![]);}
    let prefix=raw.get(offset..end).ok_or("Producer source is not valid UTF-8 range")?;
    let mut tokens=Vec::new();
    for(token,range)in Token::lexer(prefix).spanned(){
        if tokens.len()>=262_144{return Err("Producer lexer token budget".into());}
        tokens.push((token.map_err(|_|"Producer lexical error")?,range));
    }
    // Parser::peek/advance permits newlines between producer-header tokens.
    // Keep every original token/range and the same token budget; skip only
    // that grammar whitespace while locating the literal identity and '='.
    let significant=|start:usize|(start..tokens.len()).find(|index|!matches!(tokens[*index].0,Token::Newline));
    let mut depth=0usize;let mut markers=Vec::new();
    for(i,(token,_))in tokens.iter().enumerate(){
        if depth==marker_depth&&matches!(token,Token::Ident("capture"|"inspect")){
            if let Some(identity)=significant(i+1){
                if let Some(assign)=significant(identity+1){
                    if matches!(tokens[identity].0,Token::String(_))&&matches!(tokens[assign].0,Token::Assign){
                        markers.push((i,identity,assign));
                    }
                }
            }
        }
        match token{
            Token::BraceOpen|Token::ParenOpen|Token::BracketOpen=>depth=depth.checked_add(1).ok_or("Producer delimiter overflow")?,
            Token::BraceClose|Token::ParenClose|Token::BracketClose=>depth=depth.checked_sub(1).ok_or("Producer delimiter mismatch")?,_=>{},
        }
    }
    if markers.len()!=steps.len(){return Err("Producer lexer/AST count differs".into());}
    let mut result=Vec::new();
    for(index,step)in steps.iter().enumerate(){
        let (begin,identity,assign)=markers[index];let next=markers.get(index+1).map(|marker|marker.0).unwrap_or(tokens.len());
        let(id,value,inspector)=match step{ConsumerStep::Capture{producer,value}=>(producer,value,false),ConsumerStep::InspectTextTable{producer,value}=>(producer,value,true)};
        let literal=&raw[offset+tokens[identity].1.start..offset+tokens[identity].1.end];
        if expression(literal)?!=Expr::Str(id.clone())||matches!(tokens[begin].0,Token::Ident("inspect"))!=inspector{return Err("Producer source identity/kind differs from AST".into());}
        let first=(assign+1..next).find(|i|!matches!(tokens[*i].0,Token::Newline)).ok_or("Empty producer expression")?;
        let last=(first..next).rev().find(|i|!matches!(tokens[*i].0,Token::Newline)).ok_or("Empty producer expression")?;
        let value_range=offset+tokens[first].1.start..offset+tokens[last].1.end;
        if expression(raw.get(value_range.clone()).ok_or("Producer value UTF-8 range")?)?!=*value{return Err("Producer source expression differs from AST".into());}
        result.push((offset+tokens[begin].1.start..value_range.end,value_range));
    }Ok(result)
}
fn id(value:&str)->Result<u64,String>{
    let n=value.parse::<u64>().map_err(|_|"Invalid declared integer identity")?;
    if n==0||n==u64::MAX||n.to_string()!=value{return Err("Noncanonical declared identity".into());}Ok(n)
}
fn node(value:&str,graph:u64)->Result<DeclaredEndpoint,String>{
    if value.len()>384||(!value.contains('~')&&value.len()>256)||value.contains('\0'){return Err("Declared node budget/content".into());}
    let(value,legacy_string_sequence)=if let Some(rest)=value.strip_prefix("l:"){
        let(mode,value)=rest.split_once('/').ok_or("Declared sequence mode requires a node identity")?;
        if mode!="string_list_v1"{return Err("Unknown declared sequence source mode".into());}(value,true)
    }else{(value,false)};
    let(head,ports)=value.split_once('|').ok_or("Declared node ports missing")?;
    let(number,kind)=head.strip_prefix('n').ok_or("Declared node prefix")?.split_once(':').ok_or("Declared node kind")?;
    let (kind,exec_links_v1)=if let Some(kind)=kind.strip_suffix("!exec_links_v1"){(kind,true)}else{(kind,false)};
    if exec_links_v1&&value.len()>256{return Err("ExecLinksV1 node retains its current256-byte source budget".into());}
    if kind.is_empty()||!kind.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b==b'_'){return Err("Declared node kind spelling".into());}
    let mut roles=BTreeSet::new();let mut pins=BTreeSet::new();let mut result=Vec::new();
    for port in ports.split(','){
        let(role,ids)=port.split_once('=').ok_or("Declared port role")?;
        let(pin,edge)=ids.split_once('@').ok_or("Declared edge role")?;
        if role.is_empty()||!role.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'_')||!roles.insert(role){return Err("Declared role collision/spelling".into());}
        let pin=id(pin)?;if !pins.insert(pin){return Err("Declared pin collision".into());}
        if edge.starts_with('[') {
            if !exec_links_v1||!crate::scheduled_exec_links_v1::is_execution_input_role(kind,role) {
                return Err("ExecLinksV1 list is not an actual supported execution input role".into());
            }
            let incoming=crate::scheduled_exec_links_v1::decode_fragment(edge,crate::scheduled_exec_links_v1::Limits{bytes:256,links:256,declarations:100_000}).map_err(|e|e.to_string())?;
            result.push(DeclaredPort{role:role.into(),pin,incoming:None,source_pin:None,exec_incoming:Some(incoming)});
            continue;
        }
        if exec_links_v1&&crate::scheduled_exec_links_v1::is_execution_input_role(kind,role) {
            return Err("ExecLinksV1 execution input requires its complete explicit list".into());
        }
        let (edge,source_pin)=match edge.split_once('~'){Some((edge,output))=>{
            if edge=="0"||!(static_audio_role(kind,role)||crate::scheduled_operand_contract::is_dynamic_source_role(kind,role)){return Err("Declared source output is not a connected constant audio operand".into());}
            (edge,Some(id(output)?))
        },None=>(edge,None)};
        if source_pin==Some(pin){return Err("Declared audio input cannot be its own output".into());}
        result.push(DeclaredPort{role:role.into(),pin,incoming:if edge=="0"{None}else{Some(id(edge)?)},source_pin,exec_incoming:None});
    }
    if legacy_string_sequence{
        let output=match kind{"list_literal"=>"list","function_call"=>"result",_=>return Err("Declared StringList mode has another node kind".into())};
        if !matches!(result.as_slice(),[port]if port.role==output&&port.incoming.is_none()&&port.source_pin.is_none()){
            return Err("Declared StringList mode cannot hide typed/connected input ports".into());
        }
    }
    Ok(DeclaredEndpoint{graph,node:id(number)?,kind:kind.into(),ports:result,legacy_terminal:None,native_loop_output:None,legacy_string_sequence,exec_links_v1})
}

fn static_audio_role(kind:&str,role:&str)->bool{
 crate::scheduled_operand_contract::is_static_operand_role(kind,role)
}
use crate::scheduled_operand_contract::StaticOperand as AudioOperand;
fn audio_operand<'a>(statement:&'a Statement,kind:&str,role:&str)->Result<AudioOperand<'a>,String>{
 crate::scheduled_operand_contract::statement_static_operand(statement,kind,role)
}
fn transition_kind(value:&crate::Transition)->&'static str{use crate::Transition::*;match value{
 None=>"transition_none",Fade{..}=>"transition_fade",Dissolve{..}=>"transition_dissolve",SlideLeft{..}=>"transition_slide_left",SlideRight{..}=>"transition_slide_right",SlideUp{..}=>"transition_slide_up",SlideDown{..}=>"transition_slide_down",ZoomIn{..}=>"transition_zoom_in",ZoomOut{..}=>"transition_zoom_out",Wipe{..}=>"transition_wipe",Blur{..}=>"transition_blur"
}}
struct ActiveAudioConstant<'a>{endpoint:DeclaredEndpoint,value:&'a Expr,index:usize}
fn actual_static_output(endpoint:&DeclaredEndpoint)->Result<u64,String>{
 let role=match endpoint.kind.as_str(){"reroute"=>"value_out","convert_int_to_float"|"convert_bool_to_float"=>"result",_=>"value"};
 endpoint.ports.iter().find(|port|port.role==role&&port.incoming.is_none()&&port.source_pin.is_none()).map(|port|port.pin).ok_or_else(||"Static producer lacks its actual output role".into())
}
fn earlier_capture<'a>(value:&Expr,record:&ActiveAudioConstant<'_>,active:&'a BTreeMap<String,ActiveAudioConstant<'a>>)->Result<(&'a str,&'a ActiveAudioConstant<'a>),String>{
 let Expr::Call{name,args}=value else{return Err("Static adapter must reference its actual earlier capture".into());};
 let [Expr::Str(parent)]=args.as_slice()else{return Err("Static adapter capture reference arity".into());};
 if name!="__rvn_captured_v1"{return Err("Static adapter cannot evaluate a function as a constant".into());}
 let (id,prior)=active.get_key_value(parent).ok_or("Static adapter source is not active")?;
 if prior.index>=record.index{return Err("Static adapter source is forward or cyclic".into());}
 Ok((id.as_str(),prior))
}
fn static_integer_literal(value:&Expr)->Result<i64,String>{
 match value{Expr::Int(value)=>Ok(*value),Expr::Neg(value)=>match value.as_ref(){Expr::Int(value)=>value.checked_neg().ok_or_else(||"Static integer negation overflow".into()),_=>Err("Static IntToFloat cannot convert a computed negation".into())},_=>Err("Static IntToFloat cannot impersonate a float or computed source as an integer".into())}
}
fn prove_boolean_constant(id:&str,output:u64,active:&BTreeMap<String,ActiveAudioConstant<'_>>,chain:&mut BTreeSet<String>)->Result<bool,String>{
 if chain.len()>=128||!chain.insert(id.into()){return Err("Static boolean producer chain cycles or exceeds128".into());}
 let record=active.get(id).ok_or("Static boolean source is not active")?;let endpoint=&record.endpoint;
 if endpoint.legacy_terminal.is_some()||actual_static_output(endpoint)?!=output{return Err("Static boolean must retain its actual non-inspector output".into());}
 let value=if endpoint.kind=="reroute"{
  let input=endpoint.ports.iter().find(|port|port.role=="value").ok_or("Static boolean reroute input absent")?;
  if endpoint.ports.len()!=2||input.incoming.is_none()||input.source_pin.is_some(){return Err("Static boolean reroute port contract".into());}
  let(parent,prior)=earlier_capture(record.value,record,active)?;prove_boolean_constant(parent,actual_static_output(&prior.endpoint)?,active,chain)?
 }else{
  if endpoint.kind!="literal"||endpoint.ports.len()!=1{return Err("Static BoolToFloat requires a genuine boolean literal producer".into());}
  let Expr::Bool(value)=record.value else{return Err("Static BoolToFloat cannot treat a computed source as a boolean literal".into());};*value
 };chain.remove(id);Ok(value)
}
fn prove_integer_constant(id:&str,output:u64,active:&BTreeMap<String,ActiveAudioConstant<'_>>,chain:&mut BTreeSet<String>)->Result<i64,String>{
 if chain.len()>=128||!chain.insert(id.into()){return Err("Static integer producer chain cycles or exceeds128".into());}
 let record=active.get(id).ok_or("Static integer source is not active")?;let endpoint=&record.endpoint;
 if endpoint.legacy_terminal.is_some()||actual_static_output(endpoint)?!=output{return Err("Static integer must retain its actual non-inspector output".into());}
 let value=if endpoint.kind=="reroute"{
  let input=endpoint.ports.iter().find(|port|port.role=="value").ok_or("Static integer reroute input absent")?;
  if endpoint.ports.len()!=2||input.incoming.is_none()||input.source_pin.is_some(){return Err("Static integer reroute port contract".into());}
  let(parent,prior)=earlier_capture(record.value,record,active)?;prove_integer_constant(parent,actual_static_output(&prior.endpoint)?,active,chain)?
 }else{
  if !matches!(endpoint.kind.as_str(),"literal"|"int_value")||endpoint.ports.len()!=1{return Err("Static IntToFloat requires a genuine integer literal producer".into());}
  static_integer_literal(record.value)?
 };chain.remove(id);Ok(value)
}
fn prove_sprite_asset(id:&str,output:u64,active:&BTreeMap<String,ActiveAudioConstant<'_>>,chain:&mut BTreeSet<String>)->Result<String,String>{
 if chain.len()>=128||!chain.insert(id.into()){return Err("Static Sprite producer chain cycles or exceeds128".into());}
 let record=active.get(id).ok_or("Static Sprite source is not active")?;let endpoint=&record.endpoint;
 if endpoint.legacy_terminal.is_some()||actual_static_output(endpoint)?!=output{return Err("Static Sprite requires its actual non-inspector output".into());}
 let path=if endpoint.kind=="reroute"{
  let input=endpoint.ports.iter().find(|port|port.role=="value").ok_or("Static Sprite reroute input absent")?;
  if endpoint.ports.len()!=2||input.incoming.is_none()||input.source_pin.is_some(){return Err("Static Sprite reroute port contract".into());}
  let(parent,prior)=earlier_capture(record.value,record,active)?;prove_sprite_asset(parent,actual_static_output(&prior.endpoint)?,active,chain)?
 }else{
  if endpoint.kind!="sprite_asset"||endpoint.ports.len()!=1{return Err("Static Character Sprite requires a genuine SpriteAsset producer".into());}
  let Expr::Str(path)=record.value else{return Err("Static SpriteAsset cannot evaluate an expression as an image path".into());};
  if path.trim().is_empty(){return Err("Static SpriteAsset image path is empty".into());}path.clone()
 };chain.remove(id);Ok(path)
}
fn prove_audio_constant(id:&str,output:u64,wanted:AudioOperand<'_>,active:&BTreeMap<String,ActiveAudioConstant<'_>>,chain:&mut BTreeSet<String>)->Result<(),String>{
 if chain.len()>=128||!chain.insert(id.to_owned()){return Err("Constant audio producer chain cycles or exceeds128".into());}
 let record=active.get(id).ok_or("Constant audio output has no active captured producer")?;
 let e=&record.endpoint;
 if e.legacy_terminal.is_some(){return Err("Legacy inspector terminal cannot impersonate an audio constant".into());}
 if e.kind=="reroute"{
  let input=e.ports.iter().find(|p|p.role=="value").ok_or("Constant audio reroute lacks input")?;
  let result=e.ports.iter().find(|p|p.role=="value_out").ok_or("Constant audio reroute lacks output")?;
  if e.ports.len()!=2||input.incoming.is_none()||result.incoming.is_some()||result.pin!=output||input.source_pin.is_some()||result.source_pin.is_some(){return Err("Constant audio reroute output/input contract".into());}
  let Expr::Call{name,args}=record.value else{return Err("Audio reroute must reference its actual earlier capture".into());};
  let [Expr::Str(parent)]=args.as_slice() else{return Err("Audio reroute capture reference arity".into());};
  if name!="__rvn_captured_v1"{return Err("Audio reroute cannot evaluate a function as a constant".into());}
  let prior=active.get(parent).ok_or("Audio reroute source is not active")?;
  if prior.index>=record.index{return Err("Audio reroute source is forward or cyclic".into());}
  let parent_output=actual_static_output(&prior.endpoint)?;
  prove_audio_constant(parent,parent_output,wanted,active,chain)?;
 }else if e.kind=="convert_bool_to_float"{
  let input=e.ports.iter().find(|port|port.role=="value").ok_or("Static BoolToFloat input absent")?;
  if e.ports.len()!=2||input.source_pin.is_some()||actual_static_output(e)?!=output{return Err("Static BoolToFloat actual input/output contract".into());}
  let Expr::BinOp{op:crate::BinOpKind::Add,left,right}=record.value else{return Err("Static BoolToFloat requires its actual strict Bool intrinsic plus float zero".into());};
  if !matches!(right.as_ref(),Expr::Float(zero)if zero.to_bits()==0.0f32.to_bits()){return Err("Static BoolToFloat cannot alter its actual float-zero operand".into());}
  let Expr::Call{name,args}=left.as_ref() else{return Err("Static BoolToFloat requires its actual strict Bool intrinsic".into());};
  let [operand]=args.as_slice() else{return Err("Static BoolToFloat strict Bool intrinsic arity".into());};
  if name!="__rvn_bool_to_int"{return Err("Static BoolToFloat cannot replace its strict Bool intrinsic".into());}
  let boolean=if input.incoming.is_some(){
   let(parent,prior)=earlier_capture(operand,record,active)?;
   prove_boolean_constant(parent,actual_static_output(&prior.endpoint)?,active,chain)?
  }else{let Expr::Bool(value)=operand else{return Err("Static BoolToFloat requires an actual inline boolean literal".into());};*value};
  let converted=if boolean{1.0f32}else{0.0f32};
  let expected=match wanted{AudioOperand::Duration(value)if value>=0.0=>value,AudioOperand::FiniteNumber(value)=>value,AudioOperand::Number(value)if (0.0..=1.0).contains(&value)=>value,_=>return Err("Static BoolToFloat cannot satisfy this operand domain".into())};
  if !expected.is_finite()||converted.to_bits()!=expected.to_bits(){return Err("Static BoolToFloat differs from its actual f32 command operand".into());}
 }else if e.kind=="convert_int_to_float"{
  let input=e.ports.iter().find(|port|port.role=="value").ok_or("Static IntToFloat input absent")?;
  if e.ports.len()!=2||input.source_pin.is_some()||actual_static_output(e)?!=output{return Err("Static IntToFloat actual input/output contract".into());}
  let Expr::BinOp{op:crate::BinOpKind::Add,left,right}=record.value else{return Err("Static IntToFloat requires its actual emitted integer addition with float zero".into());};
  if !matches!(right.as_ref(),Expr::Float(zero)if zero.to_bits()==0.0f32.to_bits()){return Err("Static IntToFloat cannot alter its actual float-zero operand".into());}
  let integer=if input.incoming.is_some(){
   let(parent,prior)=earlier_capture(left,record,active)?;
   prove_integer_constant(parent,actual_static_output(&prior.endpoint)?,active,chain)?
  }else{static_integer_literal(left)?};
  let converted=integer as f32;
  let expected=match wanted{AudioOperand::Duration(value)if value>=0.0=>value,AudioOperand::FiniteNumber(value)=>value,AudioOperand::Number(value)if (0.0..=1.0).contains(&value)=>value,_=>return Err("Static IntToFloat cannot satisfy this operand domain".into())};
  if !converted.is_finite()||!expected.is_finite()||converted.to_bits()!=expected.to_bits(){return Err("Static IntToFloat differs from its actual f32 command operand".into());}
 }else if let AudioOperand::Character{id:expected,shown_sprite,shows_appearance}=wanted {
  let sprite=e.ports.iter().find(|p|p.role=="sprite").ok_or("Character source Sprite role absent")?;
  let value=e.ports.iter().find(|p|p.role=="value").ok_or("Character source output role absent")?;
  if e.kind!="character_value"||e.ports.len()!=2||value.pin!=output||value.incoming.is_some()||value.source_pin.is_some()||!matches!(record.value,Expr::Str(id) if id==expected){return Err("Static character source contract/id differs from the actual dialogue identity".into());}
  match (sprite.incoming,sprite.source_pin){
   (None,None)=>if shows_appearance&&shown_sprite.is_some(){return Err("Static ShowSprite appearance requires its actual Character Sprite source".into());},
   (Some(_),Some(sprite_output))=>{
    let selected=active.iter().filter(|(_,prior)|prior.index<record.index&&prior.endpoint.ports.iter().any(|port|port.pin==sprite_output&&port.incoming.is_none())).collect::<Vec<_>>();
    let [(source,_)]=selected.as_slice()else{return Err("Static Character Sprite source is missing, forward or ambiguous".into());};
    let path=prove_sprite_asset(source,sprite_output,active,chain)?;
    let canonical=if path.contains('/') {path}else{format!("./{path}")};
    if shows_appearance&&shown_sprite!=Some(canonical.as_str()){return Err("Static Character Sprite path differs from the actual ShowSprite appearance".into());}
   },
   _=>return Err("Static Character Sprite requires both its real incoming edge and source output".into()),
  }
 }else{
  let [port]=e.ports.as_slice() else{return Err("Audio constant must declare one actual output".into());};
  if port.role!="value"||port.pin!=output||port.incoming.is_some()||port.source_pin.is_some(){return Err("Audio constant output role/direction differs from declared pin".into());}
  match wanted{
   AudioOperand::Asset(kind,file)=>if e.kind!=kind||!matches!(record.value,Expr::Str(v) if v==file){return Err("Audio constant asset kind/path differs from actual command".into());},
   AudioOperand::Duration(seconds)=>{
    if !matches!(e.kind.as_str(),"literal"|"int_value"|"float_value"){return Err("Timer duration producer is not a real numeric constant".into());}
    let n=match record.value{Expr::Int(v)=>*v as f64,Expr::Float(v)=>f64::from(*v),_=>return Err("Timer duration constant cannot be textual or computed".into())};
    if !n.is_finite()||n<0.0||(n as f32).to_bits()!=seconds.to_bits(){return Err("Timer duration constant differs from its actual finite nonnegative f32 value".into());}
   },
   AudioOperand::FiniteNumber(expected)=>{
    if !matches!(e.kind.as_str(),"literal"|"int_value"|"float_value"){return Err("Static sprite number requires its actual numeric producer".into());}
    let value=match record.value{Expr::Int(v)=>*v as f64,Expr::Float(v)=>f64::from(*v),Expr::Neg(value)=>match value.as_ref(){Expr::Int(v)=>-(*v as f64),Expr::Float(v)=>f64::from(-*v),_=>return Err("Static sprite sign must apply to its actual numeric literal".into())},_=>return Err("Static sprite number cannot be textual or computed".into())};
    if !value.is_finite()||!expected.is_finite()||(value as f32).to_bits()!=expected.to_bits(){return Err("Static sprite number differs from the actual finite f32 operand".into());}
   },
   AudioOperand::Position(expected)=>{
    if e.kind!="position_value"||!matches!(record.value,Expr::Str(value) if value==&expected.to_string()){return Err("Static position producer differs from the actual position operand".into());}
   },
   AudioOperand::CinematicTransition(expected)=>{
    let (kind,raw)=match expected{None=> ("transition_none","none"),Some("fade")=>("transition_fade","fade(500)"),Some("dissolve")=>("transition_dissolve","dissolve(300)"),_=>return Err("Cinematic transition has no exact existing graph preset".into())};
    if e.kind!=kind||!matches!(record.value,Expr::Str(value) if value==raw){return Err("Static cinematic transition differs from its actual canonical preset".into());}
   },
   AudioOperand::Number(level)=>{
    if !matches!(e.kind.as_str(),"literal"|"int_value"|"float_value"){return Err("Audio volume producer is not a real numeric constant".into());}
    let n=match record.value{Expr::Int(v)=>*v as f64,Expr::Float(v)=>f64::from(*v),_=>return Err("Audio volume constant cannot be textual or computed".into())};
    if !n.is_finite()||!(0.0..=1.0).contains(&n)||(n as f32).to_bits()!=level.to_bits(){return Err("Audio volume constant differs from actual f32 command value".into());}
   },
   AudioOperand::Bool(expected)=>{
    if !matches!(e.kind.as_str(),"literal"|"bool_value")||!matches!(record.value,Expr::Bool(value) if *value==expected){return Err("Static boolean producer kind/value differs from the actual operand".into());}
   },
   AudioOperand::Unsigned(expected)=>{
    if !matches!(e.kind.as_str(),"literal"|"int_value")||!matches!(record.value,Expr::Int(value) if u32::try_from(*value)==Ok(expected)){return Err("Static unsigned operand must retain its actual in-range integer producer".into());}
   },
   AudioOperand::Character{..}=>unreachable!("Character sources are validated with their Sprite provenance above"),
   AudioOperand::Text(expected)=>{
    if !matches!(e.kind.as_str(),"literal"|"text_value")||!matches!(record.value,Expr::Str(value) if value==expected){return Err("Static text producer kind/value differs from the actual literal operand".into());}
   },
   AudioOperand::Label(expected)=>{
    if e.kind!="label_value"||!matches!(record.value,Expr::Str(value) if value==expected){return Err("Static label producer kind/value differs from the actual target".into());}
   },
   AudioOperand::Transition(expected)=>{
    let Expr::Str(raw)=record.value else{return Err("Audio transition constant requires its actual literal".into());};
    if raw.len()>4096||e.kind!=transition_kind(expected){return Err("Audio transition producer kind differs from actual command".into());}
    let actual=if raw=="none"{crate::Transition::None}else{match crate::parse(&format!("music.stop() with {raw}\n")).map_err(|e|e.to_string())?.as_slice(){[Statement::MusicStop{transition}]=>transition.clone(),_=>return Err("Audio transition literal is not one transition".into())}};
    if &actual!=expected{return Err("Audio transition constant differs from actual duration/kind".into());}
   }
  }
 }
 chain.remove(id);Ok(())
}
fn validate_audio_static(scope:&str,consumer:&str,steps:&[ConsumerStep],body:&[Statement])->Result<(),String>{
 if !scope.starts_with('g')||!scope.contains("/n"){return Ok(());}
 let(graph,_)=scope.strip_prefix('g').unwrap().split_once('/').ok_or("Declared scope shape")?;let graph=id(graph)?;
 let endpoint=node(consumer,graph)?;
 // Dynamic music volume has an expression/capture route, never a static suffix.
 // Validate the declared consumer too; a different catalogue kind cannot lend it ports.
 if matches!(body,[Statement::MusicVolumeValue{..}]) {
  let required=crate::scheduled_operand_contract::static_consumer_contract("music_volume").unwrap().ports;
  if endpoint.kind!="music_volume"||endpoint.ports.iter().map(|p|p.role.as_str()).collect::<BTreeSet<_>>()!=required.iter().copied().collect(){return Err("Dynamic music volume consumer identity differs from the actual command".into());}
  if endpoint.ports.iter().any(|p|p.source_pin.is_some()){return Err("Dynamic music volume cannot claim a static operand source output".into());}
 }
 if matches!(body,[Statement::TypewriterSetValue{..}]) {
  let required=crate::scheduled_operand_contract::static_consumer_contract("typewriter_set").unwrap().ports;
  if endpoint.kind!="typewriter_set"||endpoint.ports.iter().map(|p|p.role.as_str()).collect::<BTreeSet<_>>()!=required.iter().copied().collect(){return Err("Dynamic typewriter Boolean consumer identity differs from the actual command".into());}
  if endpoint.ports.iter().any(|p|p.source_pin.is_some()){return Err("Dynamic typewriter Boolean cannot claim a static operand source output".into());}
 }
 if matches!(body,[Statement::TypewriterSpeedValue{..}]) {
  let required=crate::scheduled_operand_contract::static_consumer_contract("typewriter_speed").unwrap().ports;
  if endpoint.kind!="typewriter_speed"||endpoint.ports.iter().map(|p|p.role.as_str()).collect::<BTreeSet<_>>()!=required.iter().copied().collect(){return Err("Dynamic typewriter speed consumer identity differs from the actual command".into());}
  if endpoint.ports.iter().any(|p|p.source_pin.is_some()){return Err("Dynamic typewriter speed cannot claim a static operand source output".into());}
 }
 // Known static consumer kinds authenticate their body and complete ports even
 // with inline defaults/no data inputs (notably TimerCancel); another node kind
 // cannot lend its declaration to an unrelated command before any publication.
 if crate::scheduled_operand_contract::static_consumer_contract(&endpoint.kind).is_some(){
  let [command]=body else{return Err("Static consumer identity requires one actual command".into());};
  let required=crate::scheduled_operand_contract::actual_static_consumer_ports(command,&endpoint.kind)?;
  if endpoint.ports.iter().map(|p|p.role.as_str()).collect::<BTreeSet<_>>()!=required.iter().map(String::as_str).collect(){return Err("Static consumer port roles differ from its actual node".into());}
 }
 let connected=endpoint.ports.iter().filter(|p|p.source_pin.is_some()&&static_audio_role(&endpoint.kind,&p.role)).collect::<Vec<_>>();if connected.is_empty(){return Ok(());}
 let [command]=body else{return Err("Static audio suffix requires one real audio command".into());};

 let mut active=BTreeMap::new();
 for(index,step)in steps.iter().enumerate(){if let ConsumerStep::Capture{producer,value}=step{
  // Parse all real declarations before selecting a source: equal values never
  // authorize substitution of another NodeId or of an inspector terminal.
  if producer.starts_with("p:"){continue;}
  let record=ActiveAudioConstant{endpoint:node(producer,graph)?,value,index};
  if active.insert(producer.clone(),record).is_some(){return Err("Duplicate active audio producer identity".into());}
 }}
 for port in connected{
  let output=port.source_pin.unwrap();let wanted=audio_operand(command,&endpoint.kind,&port.role)?;
  let selected=active.iter().filter(|(_,r)|r.endpoint.ports.iter().any(|p|p.pin==output&&p.incoming.is_none())).map(|(id,_)|id.as_str()).collect::<Vec<_>>();
  let [selected]=selected.as_slice() else{return Err("Declared audio source pin is missing, inactive or ambiguous".into());};
  prove_audio_constant(selected,output,wanted,&active,&mut BTreeSet::new())?;
 }
 Ok(())
}

/// Recalculable source observation, never an activation/input capability.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct DialogueTextBinding { pub authored_slot:usize, pub producer:String, pub output_pin:u64, pub assignment_node:Option<u64>, pub variable:String }

/// Binds the original authored slot to an actual active captured output. No
/// user value participates in source selection. Callers still admit the real
/// frozen programme/source and consumer owner before execution.
pub fn dialogue_text_binding(statement:&Statement)->Result<Option<DialogueTextBinding>,String> {
    let Statement::ScheduledConsumer{version,scope,consumer,steps,body}=statement else{return Ok(None);};
    if !scope.starts_with('g')||!scope.contains("/n"){return Ok(None);}
    let (graph,_)=scope.strip_prefix('g').unwrap().split_once('/').ok_or("Dynamic Dialogue scope shape")?;
    let graph=id(graph)?;let endpoint=node(consumer,graph)?;
    let Some(port)=endpoint.ports.iter().find(|p|p.role=="text"&&p.source_pin.is_some()) else{return Ok(None);};
    let [Statement::Dialogue{text,..}]=body.as_slice() else{return Err("Dynamic Dialogue source requires its actual Dialogue body".into());};
    let [crate::TextSegment::Interp(Expr::Var(name))]=text.0.as_slice() else{return Err("Dynamic Dialogue variable requires exactly its original authored variable slot".into());};
    if *version!=2||endpoint.kind!="dialogue"||endpoint.ports.iter().map(|p|p.role.as_str()).collect::<BTreeSet<_>>() != ["exec_in","character","text","exec_out"].into_iter().collect() {
        return Err("Dynamic Dialogue consumer kind/roles differ from actual source".into());
    }
    let output=port.source_pin.unwrap();let mut active=BTreeMap::new();
    for (index,step) in steps.iter().enumerate() {if let ConsumerStep::Capture{producer,value}=step {
        if producer.starts_with("p:"){continue;}
        if active.insert(producer.clone(),ActiveAudioConstant{endpoint:node(producer,graph)?,value,index}).is_some(){return Err("Duplicate dynamic Dialogue producer".into());}
    }}
    let selected=active.iter().filter(|(_,r)|r.endpoint.ports.iter().any(|p|p.pin==output&&p.incoming.is_none())).map(|(id,_)|id.clone()).collect::<Vec<_>>();
    let [selected]=selected.as_slice() else{return Err("Dynamic Dialogue output is missing, inactive or ambiguous".into());};
    let assignment_node=prove_dialogue_variable(selected,output,name,&active,&mut BTreeSet::new())?;
    Ok(Some(DialogueTextBinding{authored_slot:0,producer:selected.clone(),output_pin:output,assignment_node,variable:name.clone()}))
}
fn prove_dialogue_variable(id:&str,output:u64,name:&str,active:&BTreeMap<String,ActiveAudioConstant<'_>>,chain:&mut BTreeSet<String>)->Result<Option<u64>,String>{
    if chain.len()>=128||!chain.insert(id.into()){return Err("Dynamic Dialogue producer chain cycles or exceeds128".into());}
    let r=active.get(id).ok_or("Dynamic Dialogue producer is not active")?;let e=&r.endpoint;
    let numeric=e.kind=="convert_number_to_text";
    let alias=matches!(e.kind.as_str(),"convert_string_to_text"|"convert_text_to_string"|"convert_name_to_string")||numeric;
    let role=if alias{"result"}else if e.kind=="reroute"||matches!(e.kind.as_str(),"set_variable"|"local_variable"){"value_out"}else{"value"};
    let output_port=e.ports.iter().find(|p|p.role==role).ok_or("Dynamic Dialogue producer lacks actual output role")?;
    if output_port.pin!=output||output_port.incoming.is_some()||output_port.source_pin.is_some(){return Err("Dynamic Dialogue output role/direction differs".into());}
    let assignment=if e.kind=="reroute"||alias{
        let input=e.ports.iter().find(|p|p.role=="value").ok_or("Dynamic Dialogue reroute lacks input")?;
        if e.ports.len()!=2||input.incoming.is_none()||input.source_pin.is_some(){return Err("Dynamic Dialogue reroute ports differ".into());}
        let reference=if numeric{
            let Expr::BinOp{op:crate::BinOpKind::Add,left,right}=r.value else{return Err("Dynamic Dialogue NumberToText requires its actual empty-string addition".into());};
            if !matches!(left.as_ref(),Expr::Str(text)if text.is_empty()){return Err("Dynamic Dialogue NumberToText formatting expression differs".into());}
            right.as_ref()
        }else{r.value};
        let Expr::Call{name:call,args}=reference else{return Err("Dynamic Dialogue reroute must reference an earlier capture".into());};
        let [Expr::Str(parent)]=args.as_slice() else{return Err("Dynamic Dialogue reroute reference arity".into());};
        if call!="__rvn_captured_v1"{return Err("Dynamic Dialogue reroute cannot evaluate another expression".into());}
        let prior=active.get(parent).ok_or("Dynamic Dialogue reroute parent missing")?;
        if prior.index>=r.index{return Err("Dynamic Dialogue reroute parent must precede it".into());}
        let role=if matches!(prior.endpoint.kind.as_str(),"convert_string_to_text"|"convert_text_to_string"|"convert_name_to_string"|"convert_number_to_text"){"result"}else if matches!(prior.endpoint.kind.as_str(),"reroute"|"set_variable"|"local_variable"){"value_out"}else{"value"};
        let parent_output=prior.endpoint.ports.iter().find(|p|p.role==role).ok_or("Dynamic Dialogue parent output missing")?;
        prove_dialogue_variable(parent,parent_output.pin,name,active,chain)?
    }else{
        let required:BTreeSet<_>=if e.kind=="variable_get"{["value"].into_iter().collect()}else if matches!(e.kind.as_str(),"set_variable"|"local_variable"){["exec_in","name","value","exec_out","value_out"].into_iter().collect()}else{return Err("Dynamic Dialogue source must be an actual variable read output".into());};
        if e.ports.iter().map(|p|p.role.as_str()).collect::<BTreeSet<_>>()!=required||!matches!(r.value,Expr::Var(actual)if actual==name){return Err("Dynamic Dialogue variable/ports differ from authored slot".into());}
        (e.kind!="variable_get").then_some(e.node)
    };
    chain.remove(id);Ok(assignment)
}

#[derive(Default)]
struct Identities {
    roots:BTreeMap<(usize,u64),usize>, nodes:BTreeMap<(usize,u64,u64),DeclaredEndpoint>,
    variable_outputs:BTreeMap<(usize,u64,u64),String>,
    pins:BTreeMap<(usize,u64,u64),(u64,String)>, edges:BTreeMap<(usize,u64,u64),(u64,Option<u64>)>,
}
impl Identities{
    fn admit(&mut self,source:usize,endpoint:&DeclaredEndpoint)->Result<(),String>{
        let mut identity=endpoint.clone();identity.legacy_terminal=None;identity.native_loop_output=None;
        let key=(source,identity.graph,identity.node);
        if self.nodes.get(&key).is_some_and(|old|old!=&identity){return Err("Declared node identity collision".into());}
        for port in &identity.ports{
            let key=(source,identity.graph,port.pin);let owner=(identity.node,port.role.clone());
            if self.pins.get(&key).is_some_and(|old|old!=&owner){return Err("Declared PinId owner collision".into());}
            self.pins.insert(key,owner);
            if let Some(links)=&port.exec_incoming {
                for link in links {
                    let key=(source,identity.graph,link.edge);let value=(port.pin,Some(link.output_pin));
                    if self.edges.get(&key).is_some_and(|old|*old!=value){return Err("Declared ExecLinksV1 incoming EdgeId collision".into());}
                    self.edges.insert(key,value);
                }
            }
            if let Some(edge)=port.incoming{
                if self.edges.get(&(source,identity.graph,edge)).is_some_and(|old|*old!=(port.pin,port.source_pin)){return Err("Declared incoming EdgeId collision".into());}
                self.edges.insert((source,identity.graph,edge),(port.pin,port.source_pin));
            }
        }
        self.nodes.insert((source,identity.graph,identity.node),identity);Ok(())
    }
    fn validate_exec_links(&self)->Result<(),String>{
        let mut successors=BTreeMap::new();
        for ((source,graph,_),endpoint) in &self.nodes {
            if !endpoint.exec_links_v1 {continue;}
            for port in &endpoint.ports {
                let Some(links)=&port.exec_incoming else{continue;};
                for link in links {
                    let (owner,role)=self.pins.get(&(*source,*graph,link.output_pin)).ok_or("ExecLinksV1 source output is absent from actual declarations")?;
                    let actual=self.nodes.get(&(*source,*graph,*owner)).ok_or("ExecLinksV1 source output owner is absent")?;
                    if !actual.exec_links_v1||!crate::scheduled_exec_links_v1::is_execution_output_role(&actual.kind,role) {
                        return Err("ExecLinksV1 source is not an actual execution output role".into());
                    }
                    if *owner==endpoint.node {return Err("ExecLinksV1 cannot declare a direct self connection".into());}
                    let value=(port.pin,link.edge);
                    if successors.insert((*source,*graph,link.output_pin),value).is_some_and(|old|old!=value){return Err("ExecLinksV1 source execution output has multiple successors".into());}
                }
            }
        }Ok(())
    }
    fn endpoint(&mut self,scope:&str,consumer:&str,producer:&str,source:usize,root:usize,root_kind:Option<&str>,inspector:bool)->Result<Option<DeclaredEndpoint>,String>{
        // Ordinary opaque plan names remain opaque: no guessed Graph/Node IDs.
        if !scope.starts_with('g')||!scope.contains("/n"){return Ok(None);}
        if scope.len()>256{return Err("Declared scope budget".into());}
        let(graph,root_node)=scope.strip_prefix('g').unwrap().split_once('/').ok_or("Declared scope shape")?;
        let graph=id(graph)?;
        if self.roots.get(&(source,graph)).is_some_and(|old|*old!=root){return Err("Declared GraphId definition collision".into());}
        let root_node=node(root_node,graph)?;
        if root_kind!=Some(root_node.kind.as_str()){return Err("Declared root kind differs from actual lexical definition".into());}
        if root_node.legacy_string_sequence{return Err("StringList representation cannot impersonate a definition root".into());}
        let topology=root_node.exec_links_v1;
        self.roots.insert((source,graph),root);self.admit(source,&root_node)?;
        let consumer_node=node(consumer,graph)?;
        if consumer_node.legacy_string_sequence{return Err("StringList representation is restricted to actual producer nodes".into());}
        if consumer_node.exec_links_v1!=topology{return Err("ExecLinksV1 graph/root consumer format mismatch".into());}
        self.admit(source,&consumer_node)?;
        let(mut endpoint,terminal)=if let Some(rest)=producer.strip_prefix("p:"){
            let(role,value)=rest.split_once('/').ok_or("Legacy terminal identity")?;
            if !matches!(role,"valid"|"table_id"|"key"){return Err("Legacy terminal role".into());}
            (node(value,graph)?,Some(role.to_owned()))
        }else{(node(producer,graph)?,None)};
        if endpoint.exec_links_v1!=topology{return Err("ExecLinksV1 graph/root producer format mismatch".into());}
        if inspector||terminal.is_some(){
            let roles:endpoint_roles::Roles=endpoint.ports.iter().map(|p|p.role.as_str()).collect();
            if endpoint.kind!="get_string_table_id_and_key"||roles!=["text","valid","table_id","key"].into_iter().collect(){return Err("Inspector declared port contract".into());}
        }
        if inspector&&terminal.is_some(){return Err("Native inspector cannot be a Legacy terminal".into());}
        endpoint.legacy_terminal=terminal;
        if endpoint.kind=="for_each"&&endpoint.ports.iter().any(|p|p.role=="array_element"){
            if inspector||endpoint.legacy_terminal.is_some(){return Err("Native loop output cannot borrow an inspector terminal role".into());}
            endpoint.native_loop_output=Some("array_element".into());
        }
        self.admit(source,&endpoint)?;Ok(Some(endpoint))
    }
}
mod endpoint_roles {pub type Roles<'a>=std::collections::BTreeSet<&'a str>;}

/// Validates each root against a unique actual frozen source and then locates
/// only the steps of that exact AST. No file is reread from a moved live path.
pub fn scheduled_step_sources(program:&ParsedProgram)->Result<Vec<ScheduledStepSource>,String>{
    if program.script.len()!=program.origins.len()||program.script.len()>100_000||program.sources.len()>100_000{return Err("Scheduled source origin count/budget".into());}
    if program.sources.iter().try_fold(0usize,|n,s|n.checked_add(s.source.len())).is_none_or(|n|n>16*1024*1024){return Err("Scheduled source byte budget".into());}
    native_loop_contract::validate_authored_bindings(&program.script)?;
    let mut parsed=BTreeMap::new();
    for(index,source)in program.sources.iter().enumerate(){
        if parsed.insert(source.path.clone(),(index,crate::parse_with_origins(&source.source,program.context).map_err(|e|e.to_string())?)).is_some(){return Err("Scheduled source identity ambiguous".into());}
    }
    let mut used=BTreeSet::new();let mut out=Vec::new();let mut ids=Identities::default();let mut labels=BTreeMap::new();
    let mut work=0;
    for (statement,origin) in program.script.iter().zip(&program.origins) {
        let (source,_)=parsed.get(&origin.source).ok_or("Assignment source absent")?;
        index_assignment_outputs(statement,*source,&mut ids,&mut work,0)?;
    }
    for(root,(statement,origin))in program.script.iter().zip(&program.origins).enumerate(){
        let(source,actual)=parsed.get(&origin.source).ok_or("Scheduled source absent")?;
        if !used.insert((origin.source.clone(),origin.span.range.start,origin.span.range.end))||!actual.script.iter().zip(&actual.spans).any(|(s,p)|s==statement&&p==&origin.span){return Err("Scheduled root source/AST mismatch".into());}
        if let Statement::FunctionLibrary{scope}=statement{
            scope.validate()?;
            if origin.span.children.len()!=scope.members().len(){return Err("Library producer declaration ranges differ".into());}
            for(index,member)in scope.members().iter().enumerate(){
                let member_scope=Some(format!("library:{}:{}",scope.library().as_str(),member.graph_id()));
                walk(member.declaration(),&origin.span.children[index],&program.sources[*source].source,
                    *source,root,Some("function_entry"),&member_scope,&[ScheduledSourceRoute::LibraryMember(index)],&mut ids,&mut out)?;
            }
            continue;
        }
        let scope=match statement{Statement::Function{name,..}=>Some(name.clone()),Statement::Screen{name,..}=>Some(format!("screen:{name}")),Statement::Handler{name,..}=>Some(format!("handler:{name}")),_=>None};
        if matches!(statement,Statement::Label{..}|Statement::SceneLabel{..}){labels.insert(*source,root);}
        let definition=if scope.is_none()&&!matches!(statement,Statement::Init{..}){labels.get(source).copied().unwrap_or(root)}else{root};
        let root_kind=match &program.script[definition]{Statement::Function{..}=>Some("function_entry"),Statement::Screen{..}=>Some("screen_entry"),Statement::Handler{..}=>Some("handler_entry"),Statement::Init{..}=>Some("init"),Statement::Label{..}|Statement::SceneLabel{..}=>Some("label"),_=>None};
        walk(statement,&origin.span,&program.sources[*source].source,*source,definition,root_kind,&scope,&[],&mut ids,&mut out)?;
    }ids.validate_exec_links()?;Ok(out)
}
// Source-local validation index only. It is rebuilt from the exact AST on
// every admission and never stores runtime values or grants an owner capability.
fn index_assignment_outputs(statement:&Statement,source:usize,ids:&mut Identities,work:&mut usize,depth:usize)->Result<(),String>{
    *work=work.checked_add(1).ok_or("Assignment source count overflow")?;
    if depth>128{return Err("Assignment source index budget".into());}
    if let Statement::ScheduledConsumer{scope,consumer,body,..}=statement {
        if scope.starts_with('g')&&scope.contains("/n") {
            if let [Statement::SetVar{name,..}|Statement::LocalVar{name,..}]=body.as_slice() {
                let (graph,_)=scope.strip_prefix('g').unwrap().split_once('/').ok_or("Assignment scope shape")?;
                let graph=id(graph)?;let endpoint=node(consumer,graph)?;
                let expected=if crate::widget_create_effect::parts(&body[0])?.is_some(){"widget_create"}else if matches!(&body[0],Statement::SetVar{..}){"set_variable"}else{"local_variable"};
                if endpoint.kind!=expected{return Err("Assignment identity differs from actual source kind".into());}
                let key=(source,graph,endpoint.node);
                if ids.variable_outputs.get(&key).is_some_and(|old|old!=name){return Err("Assignment identity changes actual variable".into());}
                ids.variable_outputs.insert(key,name.clone());
            }
        }
    }
    if let Statement::FunctionLibrary{scope}=statement{
        for member in scope.members(){index_assignment_outputs(member.declaration(),source,ids,work,depth+1)?;}
        return Ok(());
    }
    let children:Vec<&[Statement]>=match statement{
        Statement::Function{body,..}|Statement::UnitFunction{body,..}|Statement::Screen{body,..}|Statement::Handler{body,..}|Statement::Init{body}|Statement::ScheduledConsumer{body,..}|Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}=>vec![body],
        Statement::If{then_branch,else_branch,..}=>vec![then_branch,else_branch],
        Statement::Sequence{branches}=>branches.iter().map(Vec::as_slice).collect(),
        Statement::StandardMacroDispatch{dispatch}=>dispatch.branches.iter().map(|branch|branch.body.as_slice()).collect(),
        Statement::Choice{options}=>options.iter().map(|o|o.body.as_slice()).collect(),
        Statement::Imagemap{hotspots,..}=>hotspots.iter().map(|h|h.body.as_slice()).collect(),_=>vec![],
    };
    for block in children{for child in block{index_assignment_outputs(child,source,ids,work,depth+1)?;}}Ok(())
}
fn walk(statement:&Statement,span:&StatementSpan,raw:&str,source:usize,root:usize,root_kind:Option<&str>,scope:&Option<String>,route:&[ScheduledSourceRoute],ids:&mut Identities,out:&mut Vec<ScheduledStepSource>)->Result<(),String>{
    if route.len()>128{return Err("Scheduled origin depth budget".into());}
    if let Statement::StandardMacroDispatch{dispatch}=statement {
        dispatch.validate()?;
        if root_kind!=Some("handler_entry"){return Err("Macro source is outside its actual Handler root".into());}
        let plan_scope=dispatch.site.scope_source();let node=dispatch.site.node_source();
        ids.endpoint(&plan_scope,&node,&node,source,root,root_kind,false)?;
        for(index,(step,(range,expression)))in dispatch.operand_steps().iter().zip(macro_operand_ranges(raw,span,dispatch.operand_steps())?).enumerate(){
            if out.len()>=100_000{return Err("Scheduled origin count budget".into());}
            let(producer,inspector)=match step{ConsumerStep::Capture{producer,..}=>(producer,false),ConsumerStep::InspectTextTable{producer,..}=>(producer,true)};
            let endpoint=ids.endpoint(&plan_scope,&node,producer,source,root,root_kind,inspector)?;
            validate_sequence_projection(step,endpoint.as_ref())?;
            out.push(ScheduledStepSource{source_ordinal:source,definition_root_ordinal:root,scope:scope.clone(),route:route.to_vec(),consumer:span.range.clone(),index,span:range,expression,plan_scope:plan_scope.clone(),consumer_identity:node.clone(),producer_identity:producer.clone(),endpoint});
        }
    }
    if let Statement::ScheduledConsumer{version,scope:plan_scope,consumer,steps,body}=statement{
        if let [leaf]=body.as_slice(){if crate::widget_create_effect::parts(leaf)?.is_some(){
            if *version!=2{return Err("Widget factory requires its actual v2 Consumer".into());}
            if plan_scope.starts_with('g')&&plan_scope.contains("/n"){
                let(graph,_)=plan_scope.strip_prefix('g').unwrap().split_once('/').ok_or("Widget factory scope shape")?;
                let endpoint=node(consumer,id(graph)?)?;
                let expected:BTreeSet<_>=["exec_in","class","arguments","exec_out","value_out"].into_iter().collect();
                if endpoint.kind!="widget_create"||endpoint.ports.iter().map(|p|p.role.as_str()).collect::<BTreeSet<_>>()!=expected{return Err("Widget factory identity/ports differ from actual command".into());}
            }
        }}
        if let [leaf]=body.as_slice(){if let Some((kind,roles))=crate::widget_viewport_effect_1003::identity_contract(leaf){
            let context_allowed = root_kind == Some("label")
                || (root_kind == Some("handler_entry") && matches!(leaf, Statement::WidgetRemoveFromParent { .. }));
            if *version!=2||!context_allowed||!plan_scope.starts_with('g')||!plan_scope.contains("/n") {
                return Err("Widget attachment requires its actual Native v2 Label root; removal also admits its actual Native Handler root".into());
            }
            let(graph,_)=plan_scope.strip_prefix('g').unwrap().split_once('/').ok_or("Widget viewport source scope shape")?;
            let endpoint=node(consumer,id(graph)?)?;
            if endpoint.kind!=kind||endpoint.ports.iter().map(|port|port.role.as_str()).collect::<BTreeSet<_>>()!=roles.iter().copied().collect(){return Err("Widget viewport identity/ports differ from actual effect".into());}
        }}
        let marked=consumer.split_once('|').is_some_and(|(head,_)|head.ends_with("!exec_links_v1"));
        if marked&&*version!=3 {ids.endpoint(plan_scope,consumer,consumer,source,root,root_kind,false)?;}
        if *version==3 {
            native_loop_contract::binding(statement)?;
            // Even a zero-producer loop must prove its declared graph root is
            // this actual Init/definition, not merely a plausible string.
            ids.endpoint(plan_scope,consumer,consumer,source,root,root_kind,false)?;
        }
        if plan_scope.starts_with('g')&&plan_scope.contains("/n") {
            let (graph,_)=plan_scope.strip_prefix('g').unwrap().split_once('/').ok_or("Variable output scope shape")?;
            let graph=id(graph)?;let endpoint=node(consumer,graph)?;
            if let [Statement::SetVar{name,..}|Statement::LocalVar{name,..}]=body.as_slice() {
                let expected=if crate::widget_create_effect::parts(&body[0])?.is_some(){"widget_create"}else if matches!(&body[0],Statement::SetVar{..}){"set_variable"}else{"local_variable"};
                if endpoint.kind!=expected{return Err("Assignment consumer identity differs from actual statement kind".into());}
                let key=(source,graph,endpoint.node);
                if ids.variable_outputs.get(&key).is_some_and(|old|old!=name){return Err("Assignment source identity changes variable".into());}
                ids.variable_outputs.insert(key,name.clone());
            }
            if let Some(binding)=dialogue_text_binding(statement)? {
                if let Some(assignment)=binding.assignment_node {
                    if ids.variable_outputs.get(&(source,graph,assignment))!=Some(&binding.variable) {
                        return Err("Dialogue assignment output lacks its actual source assignment/name".into());
                    }
                }
            }
        }
        for(index,(step,(range,expression)))in steps.iter().zip(ranges(raw,span,steps)?).enumerate(){
            if out.len()>=100_000{return Err("Scheduled origin count budget".into());}
            let(producer,inspector)=match step{ConsumerStep::Capture{producer,..}=>(producer,false),ConsumerStep::InspectTextTable{producer,..}=>(producer,true)};
            let endpoint=ids.endpoint(plan_scope,consumer,producer,source,root,root_kind,inspector)?;
            if endpoint.as_ref().is_some_and(|endpoint|endpoint.legacy_string_sequence){
                let value=match step{ConsumerStep::Capture{value,..}=>value,_=>return Err("StringList representation cannot be an inspector".into())};
                let items=match(endpoint.as_ref().unwrap().kind.as_str(),value){
                    ("list_literal",Expr::ListLit(items))=>items,
                    ("function_call",Expr::Call{args,..})=>args,
                    _=>return Err("Declared property sequence differs from its actual expression kind".into()),
                };
                if items.iter().any(|item|!matches!(item,Expr::Str(_))){return Err("Declared property StringList requires actual string expressions".into());}
            }
            out.push(ScheduledStepSource{source_ordinal:source,definition_root_ordinal:root,scope:scope.clone(),route:route.to_vec(),consumer:span.range.clone(),index,span:range,expression,
                plan_scope:plan_scope.clone(),consumer_identity:consumer.clone(),producer_identity:producer.clone(),endpoint});
        }
        validate_audio_static(plan_scope,consumer,steps,body)?;
        dialogue_text_binding(statement)?;
    }
    let groups:Vec<(Option<ScheduledSourceRoute>,&[Statement])>=match statement{
        Statement::Function{body,..}|Statement::UnitFunction{body,..}|Statement::Screen{body,..}|Statement::Handler{body,..}|Statement::Init{body}=>vec![(None,body)],
        Statement::ScheduledConsumer{body,..}|Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}=>vec![(Some(ScheduledSourceRoute::Body),body)],
        Statement::If{then_branch,else_branch,..}=>vec![(Some(ScheduledSourceRoute::Then),then_branch),(Some(ScheduledSourceRoute::Else),else_branch)],
        Statement::Sequence{branches}=>branches.iter().enumerate().map(|(i,b)|(Some(ScheduledSourceRoute::Branch(i)),b.as_slice())).collect(),
        Statement::StandardMacroDispatch{dispatch}=>dispatch.branches.iter().enumerate().map(|(i,b)|(Some(ScheduledSourceRoute::Branch(i)),b.body.as_slice())).collect(),
        Statement::Choice{options}=>options.iter().enumerate().map(|(i,o)|(Some(ScheduledSourceRoute::Choice(i)),o.body.as_slice())).collect(),
        Statement::Imagemap{hotspots,..}=>hotspots.iter().enumerate().map(|(i,h)|(Some(ScheduledSourceRoute::Hotspot(i)),h.body.as_slice())).collect(),_=>vec![],
    };
    if groups.iter().map(|(_,b)|b.len()).sum::<usize>()!=span.children.len(){
        if groups.is_empty(){return Ok(());}return Err("Scheduled actual child spans differ".into());
    }
    let mut offset=0;
    for(part,body)in groups{
        for(index,child)in body.iter().enumerate(){
            let mut next=route.to_vec();if let Some(part)=&part{next.push(part.clone());}next.push(ScheduledSourceRoute::Statement(index));
            walk(child,&span.children[offset+index],raw,source,root,root_kind,scope,&next,ids,out)?;
        }offset+=body.len();
    }Ok(())
}


#[cfg(test)]
mod multiline_producer_header_tests1002 {
    use super::*;
    // Source/AST/range compatibility only: these old unmarked plan names do
    // not claim Native Node/Pin/Edge authority, an Engine owner or execution.
    fn source(header: &str) -> String {
        format!("// été 🌿\nfunction probe(){{flow.consume_v1(\"range-unit\",\"return-unit\"){{\n{header}42\nreturn __rvn_captured_v1(\"value\")\n}}}}\n")
    }
    #[test]
    fn source_header1002_multiline_and_comments_keep_actual_expression_ranges_and_ast() {
        for header in ["capture \"value\" = ", "capture \"value\" \n = ",
            "capture\n\n\"value\"\n=\n", "capture // comment\r\n\"value\" // id\r\n= // value\r\n"] {
            let raw=source(header);let parsed=crate::parse_program(&raw).unwrap();let before=parsed.clone();
            let sites=scheduled_step_sources(&parsed).unwrap();assert_eq!(sites.len(),1);
            assert_eq!(&raw[sites[0].expression.clone()],"42");
            assert!(raw[sites[0].span.clone()].starts_with("capture"));
            assert_eq!(sites[0].producer_identity,"value");assert!(sites[0].endpoint.is_none());
            assert_eq!(parsed.script,before.script);assert_eq!(parsed.origins,before.origins);
            assert_eq!(parsed.sources[0].source,raw);
        }
    }
    #[test]
    fn source_header1002_capture_and_inspect_headers_do_not_count_nested_or_string_lookalikes() {
        let raw=r#"function probe(){flow.consume_v1("range-unit","return-unit"){
capture
"value"
= ["capture", "inspect", 42][2]
inspect "text"
= __rvn_invariant_text("probe")
return __rvn_terminal_v1("text","valid")
}}"#;
        let parsed=crate::parse_program(raw).unwrap();let sites=scheduled_step_sources(&parsed).unwrap();
        assert_eq!(sites.len(),2);assert_eq!(sites[0].producer_identity,"value");assert_eq!(sites[1].producer_identity,"text");
        assert_eq!(&raw[sites[0].expression.clone()],r#"["capture", "inspect", 42][2]"#);
        assert_eq!(&raw[sites[1].expression.clone()],r#"__rvn_invariant_text("probe")"#);
    }
    #[test]
    fn source_header1002_duplicate_false_assignment_and_forged_ast_still_refuse() {
        for header in ["capture\n\"value\"\n== 1\n", "capture\n\"value\"\n= 1\ncapture\n\"value\"\n= "] {
            assert!(crate::parse_program(&source(header)).is_err());
        }
        let raw=source("capture \"value\"\n= ");let mut parsed=crate::parse_program(&raw).unwrap();
        let Statement::Function{body,..}=&mut parsed.script[0]else{panic!("actual definition")};
        let Statement::ScheduledConsumer{steps,..}=&mut body[0]else{panic!("actual source consumer")};
        let ConsumerStep::Capture{value,..}=&mut steps[0]else{panic!("actual capture")};*value=Expr::Int(99);
        let before=parsed.clone();assert!(scheduled_step_sources(&parsed).is_err());
        assert_eq!(parsed.script,before.script);assert_eq!(parsed.sources[0].source,raw);
    }
}


#[cfg(test)]
mod bool_float_static_domains1003 {
    use super::*;
    // This is only a private numeric-domain unit seam. It grants no ParsedProgram,
    // producer identity, Engine authority or rendered/native acceptance.
    fn accepts(value: &Expr, wanted: AudioOperand<'_>) -> bool {
        let port = |role: &str, pin| DeclaredPort { role: role.into(), pin,
            incoming: None, source_pin: None, exec_incoming: None };
        let endpoint = DeclaredEndpoint { graph: 1, node: 1, kind: "convert_bool_to_float".into(),
            ports: vec![port("value", 1), port("result", 2)], legacy_terminal: None,
            native_loop_output: None, legacy_string_sequence: false, exec_links_v1: false };
        let active = BTreeMap::from([("unit".into(), ActiveAudioConstant { endpoint, value, index: 0 })]);
        prove_audio_constant("unit", 2, wanted, &active, &mut BTreeSet::new()).is_ok()
    }
    fn value(operand: Expr, zero: f32) -> Expr {
        Expr::BinOp { op: crate::BinOpKind::Add,
            left: Box::new(Expr::Call { name: "__rvn_bool_to_int".into(), args: vec![operand] }),
            right: Box::new(Expr::Float(zero)) }
    }
    #[test]
    fn bool_float1003_static_numeric_domains_keep_exact_zero_one_bounds_and_reject_computed_operands() {
        for flag in [false, true] {
            let expression = value(Expr::Bool(flag), 0.0);
            let exact = if flag { 1.0 } else { 0.0 };
            assert!(accepts(&expression, AudioOperand::Number(exact)));
            assert!(accepts(&expression, AudioOperand::FiniteNumber(exact)));
            assert!(accepts(&expression, AudioOperand::Duration(exact)));
            for wrong in [-1.0, -0.01, 0.5, 1.00001, 2.0, f32::MIN, f32::MAX, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                assert!(!accepts(&expression, AudioOperand::Number(wrong)));
                assert!(!accepts(&expression, AudioOperand::FiniteNumber(wrong)));
            }
        }
        assert!(!accepts(&value(Expr::Bool(false), 0.0), AudioOperand::Number(-0.0)));
        for operand in [Expr::Var("flag".into()),
            Expr::Call { name: "__rvn_int_to_bool".into(), args: vec![Expr::Int(1)] },
            Expr::Call { name: "random".into(), args: vec![Expr::Int(1), Expr::Int(9)] },
            Expr::Int(1)] {
            let expression = value(operand, 0.0);
            assert!(!accepts(&expression, AudioOperand::Number(1.0)));
            assert!(!accepts(&expression, AudioOperand::FiniteNumber(1.0)));
        }
        for zero in [-0.0, 1.0, f32::NAN, f32::INFINITY] {
            assert!(!accepts(&value(Expr::Bool(true), zero), AudioOperand::FiniteNumber(1.0)));
        }
    }
}

fn macro_operand_ranges(raw:&str,span:&StatementSpan,steps:&[ConsumerStep])->Result<Vec<(Range<usize>,Range<usize>)>,String>{
    if steps.is_empty(){return Ok(vec![]);}
    let prefix=raw.get(span.range.clone()).ok_or("Macro source UTF-8 range")?;
    let mut found=false;let mut depth=0usize;let mut begin=None;
    for(token,range)in Token::lexer(prefix).spanned(){
        let token=token.map_err(|_|"Macro operand lexical error")?;
        if !found {if matches!(token,Token::Ident("operand_v1")){found=true;}continue;}
        if begin.is_none(){if matches!(token,Token::Newline){continue;}if !matches!(token,Token::BraceOpen){return Err("Macro operand requires its actual source block".into());}begin=Some(span.range.start+range.end);depth=1;continue;}
        match token{Token::BraceOpen=>depth+=1,Token::BraceClose=>{depth=depth.checked_sub(1).ok_or("Macro operand delimiters")?;if depth==0{return ranges_region(raw,begin.unwrap(),span.range.start+range.start,steps,0);}},_=>{}}
    }
    Err("Actual macro operand source region is absent".into())
}
fn validate_sequence_projection(step:&ConsumerStep,endpoint:Option<&DeclaredEndpoint>)->Result<(),String>{
    if endpoint.is_some_and(|endpoint|endpoint.legacy_string_sequence){
        let value=match step{ConsumerStep::Capture{value,..}=>value,_=>return Err("StringList representation cannot be an inspector".into())};
        let items=match(endpoint.unwrap().kind.as_str(),value){("list_literal",Expr::ListLit(items))=>items,("function_call",Expr::Call{args,..})=>args,_=>return Err("Declared property sequence differs from its actual expression kind".into())};
        if items.iter().any(|item|!matches!(item,Expr::Str(_))){return Err("Declared property StringList requires actual string expressions".into());}
    }Ok(())
}
