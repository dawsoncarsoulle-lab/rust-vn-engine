//! Versioned data attached to the existing narrative continuation chain.
//! Source binding, records and branch ownership are checked together; the
//! numeric index or an authored node identity never authorizes a projection.
use crate::{eval::{EvalError,EvalResult,debugger_eval::PortableRecord},native_loop_cursor::DemandLoopCursor,program::{PreparedNarrative,InstructionOrigin},scene::{ActivationId,FrameKind,NarrativeFrame,ResumePosition,MAX_NARRATIVE_FRAMES}};
use rvn_parser::{Value,scheduled_source::native_loop_contract::{Binding,Kind}};
use serde::{Serialize,Deserialize};
use std::collections::HashMap;
#[cfg(test)]
#[path="scene_native_loop_tests.rs"]
mod tests;

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeLoopFrame {
    pub header:ResumePosition,
    pub cursor:DemandLoopCursor,
    pub gather:Vec<PortableRecord>,
    pub body_frame:usize,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub break_site:Option<NativeBreakSite>,
}

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NativeBreakSite {pub position:ResumePosition,pub visit:usize}

/// Public shape so existing public Scene state can carry it. Its entries and
/// operations remain private to the admitted runtime; deserialization is data.
#[derive(Clone,Debug,PartialEq,Default,Serialize,Deserialize)]
#[serde(transparent)]
pub struct NativeLoopFramesV1(pub(crate) Vec<NativeLoopFrame>);

fn invalid(message:&str)->EvalError{EvalError::InvalidFunction(message.into())}
fn same_body_source(header:&InstructionOrigin,body_route:&[crate::program::AstRoutePart],position:&ResumePosition)->bool{
    position.origin.as_ref().is_some_and(|origin|origin.source==header.source&&origin.route.starts_with(body_route))
}

impl NativeLoopFramesV1 {
    pub fn is_empty(&self)->bool{self.0.is_empty()}
    /// The real chain is supplied by SceneExecution. Parked helper/scene states
    /// use their actual caller continuation and only that caller's chain prefix.
    pub(crate) fn validate_chain(&self,program:&PreparedNarrative,chain:&[NarrativeFrame],owner:Option<ActivationId>,position:&ResumePosition)->EvalResult<()> {
        if self.0.len()>MAX_NARRATIVE_FRAMES{return Err(invalid("Native loops exceed the existing narrative continuation depth"));}
        let mut parent_body:Option<&[crate::program::AstRoutePart]>=None;let mut prior_frame=None;
        for (index,frame) in self.0.iter().enumerate(){
            program.validate_position(&frame.header).map_err(EvalError::InvalidFunction)?;
            let body=program.native_loops.body(frame.header.pc).ok_or_else(||invalid("Native narrative frame has no actual source header"))?;
            if frame.header.origin.as_ref()!=Some(&body.header){return Err(invalid("Native narrative frame differs from the real header origin"));}
            if let Some(parent)=parent_body{if !body.header.route.starts_with(parent){return Err(invalid("Native loop stack lost its actual lexical body ancestry"));}}
            if prior_frame.is_some_and(|previous|frame.body_frame<=previous){return Err(invalid("Native loop bodies do not follow the real continuation chain"));}
            if !matches!((&frame.cursor,body.binding.kind),(DemandLoopCursor::ArrayV1{..},Kind::Array)|(DemandLoopCursor::RangeV1{..},Kind::Range)){
                return Err(invalid("Native narrative cursor differs from its real source loop kind"));
            }
            frame.cursor.validate_body()?;
            if let Some(site)=&frame.break_site{
                if body.binding.kind!=Kind::Range||site.visit!=frame.cursor.visits(){return Err(invalid("Native Break differs from its entered Range visit"));}
                let nearest=program.native_loops.break_owner(program,&site.position)?;
                if nearest.header!=body.header{return Err(invalid("Native Break does not belong to its nearest actual Range source"));}
            }
            PortableRecord::validate_native_loop_cursor(&program.script[frame.header.pc],&frame.gather,&frame.cursor)?;
            let input=PortableRecord::loop_ports_unbound(&program.script[frame.header.pc],&frame.gather)?;
            // Arithmetic revalidation consumes retained data, never producers.
            // A false current comparison cannot authorize an entered body.
            let mut comparison=match &frame.cursor {
                DemandLoopCursor::ArrayV1{index,visits}=>DemandLoopCursor::ArrayV1{index:*index,visits:visits-1},
                DemandLoopCursor::RangeV1{first,index,visits}=>DemandLoopCursor::RangeV1{first:*first,index:*index,visits:visits-1},
            };
            if !comparison.enter(&input)?||comparison!=frame.cursor{return Err(invalid("Native body cursor is not the retained successful comparison"));}
            let at_returned_header=index+1==self.0.len()&&chain.len()==frame.body_frame&&*position==frame.header;
            if at_returned_header&&frame.break_site.is_some(){return Err(invalid("Latched Native body must complete at its actual Branch Return, without a published header visit"));}
            if !at_returned_header {
                let branch=chain.get(frame.body_frame).ok_or_else(||invalid("Native loop body has no live compiler branch frame"))?;
                if branch.kind!=FrameKind::Branch||branch.owner!=owner||branch.continuation!=frame.header||branch.parked.is_some(){
                    return Err(invalid("Native loop body is not owned by its actual Branch continuation"));
                }
                let at_real_scaffold=position==&program.position(body.body_label)||position==&program.position(body.body_return);
                if !at_real_scaffold&&!same_body_source(&body.header,&body.body_route,position){return Err(invalid("Native loop body position escaped its actual source ancestry"));}
            }
            parent_body=Some(&body.body_route);prior_frame=Some(frame.body_frame);
        }
        Ok(())
    }
    /// Called only after validate_chain with the genuine current position.
    /// Header evaluation inserts its own cursor/phase separately; an index is
    /// visible solely in its real authored body, never in GameState globals.
    pub(crate) fn project_body_indices(&self,program:&PreparedNarrative,position:&ResumePosition,variables:&mut HashMap<String,Value>)->EvalResult<()> {
        // Check the complete projection before changing the disposable view.
        let mut projected=std::collections::HashSet::new();
        for frame in &self.0 {
            let body=program.native_loops.body(frame.header.pc).ok_or_else(||invalid("Native body projection lost its actual source header"))?;
            if same_body_source(&body.header,&body.body_route,position){
                if variables.contains_key(&body.binding.index){return Err(invalid("Native loop body index collides with an authored binding"));}
                if !projected.insert(body.binding.index.as_str()){return Err(invalid("Native body projections have ambiguous simultaneous source ownership"));}
            }
        }
        for frame in &self.0 {
            let body=program.native_loops.body(frame.header.pc).ok_or_else(||invalid("Native body projection lost its actual source header"))?;
            if same_body_source(&body.header,&body.body_route,position){
                let Binding{index,..}=&body.binding;
                variables.insert(index.clone(),Value::Int(frame.cursor.index()));
            }
        }
        Ok(())
    }
}
