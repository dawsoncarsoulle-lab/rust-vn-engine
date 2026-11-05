//! Derived preparation for Native demand-loop narrative branches.
//! This metadata alone never admits a source, frame or runtime operation.
use super::{invalid,AstRoutePart,BranchKind,EvalError,GeneratedRole,InstructionOrigin,PreparedNarrative};
use rvn_parser::{Script,Statement};
use rvn_parser::scheduled_source::native_loop_contract::{self,Binding};
use std::{collections::HashMap,sync::Arc};

pub(super) struct NativeBodyRequest {
    pub header:InstructionOrigin,
    pub target:String,
}

#[derive(Clone,Debug)]
pub(crate) struct NativeBody {
    pub header:InstructionOrigin,
    pub binding:Binding,
    pub body_target:String,
    pub body_label:usize,
    pub body_return:usize,
    pub body_route:Vec<AstRoutePart>,
}

#[derive(Clone,Debug,Default)]
pub(crate) struct PreparedNativeLoops {
    // Retained only for explicit source3. Historical ASTs acquire no clone.
    authored:Option<Arc<Script>>,
    bodies:HashMap<usize,NativeBody>,
}

impl PreparedNativeLoops {
    pub(super) fn resolve(authored:Option<Arc<Script>>,requests:Vec<NativeBodyRequest>,prepared:&PreparedNarrative)->Result<Self,EvalError>{
        let mut bodies=HashMap::new();
        for request in requests {
            let headers:Vec<_>=prepared.script.iter().enumerate().filter(|(pc,statement)|
                prepared.origins[*pc]==request.header&&matches!(statement,Statement::ScheduledConsumer{version:3,..})
            ).collect();
            let [(header_pc,statement)]=headers.as_slice() else{return Err(invalid("Native body has no unique actual compiled header"));};
            let binding=native_loop_contract::binding(statement).map_err(invalid)?.ok_or_else(||invalid("Native branch lost its actual loop binding"))?;
            let body_label=*prepared.label_table.get(&request.target).ok_or_else(||invalid("Native body generated target is absent"))?;
            if !prepared.generated_targets.contains(&request.target)||prepared.origins[body_label]!=request.header.generated(GeneratedRole::BranchLabel{branch:BranchKind::Loop,ordinal:0}){
                return Err(invalid("Native body label is not the actual compiler-owned loop branch"));
            }
            let expected_return=request.header.generated(GeneratedRole::BranchReturn{branch:BranchKind::Loop,ordinal:0});
            let returns:Vec<_>=prepared.origins.iter().enumerate().filter(|(_,origin)|**origin==expected_return).map(|(pc,_)|pc).collect();
            let [body_return]=returns.as_slice() else{return Err(invalid("Native loop branch has no unique actual Return"));};
            if *body_return<=body_label||!prepared.branch_returns.contains(body_return)||!matches!(prepared.script[*body_return],Statement::Return){
                return Err(invalid("Native branch Return differs from its actual prepared continuation"));
            }
            let mut body_route=request.header.route.clone();body_route.push(AstRoutePart::Body{index:0});
            let body=NativeBody{header:request.header,binding,body_target:request.target,body_label,body_return:*body_return,body_route};
            if bodies.insert(*header_pc,body).is_some(){return Err(invalid("Ambiguous actual Native narrative header"));}
        }
        if !bodies.is_empty()&&authored.is_none(){return Err(invalid("Native branch lost the original authored AST"));}
        Ok(Self{authored,bodies})
    }

    pub(crate) fn authored(&self)->Option<&[Statement]>{self.authored.as_deref().map(Vec::as_slice)}
    pub(crate) fn body(&self,pc:usize)->Option<&NativeBody>{self.bodies.get(&pc)}
    pub(crate) fn is_empty(&self)->bool{self.bodies.is_empty()}

    /// Resolve the nearest authored Range, including intervening historical
    /// ranges. A shallower Native frame cannot borrow an inner Legacy Break.
    pub(crate) fn break_owner<'a>(&'a self,prepared:&PreparedNarrative,position:&crate::scene::ResumePosition)->Result<&'a NativeBody,EvalError>{
        prepared.validate_position(position).map_err(invalid)?;
        if !matches!(prepared.script.get(position.pc),Some(Statement::LoopBreak)){return Err(invalid("Native Break is not the actual reached instruction"));}
        let origin=position.origin.as_ref().ok_or_else(||invalid("Native Break lost its actual authored position"))?;
        if origin.generated.is_some(){return Err(invalid("A generated Legacy Break cannot borrow Native ownership"));}
        let authored=self.authored().ok_or_else(||invalid("Native Break has no retained actual authored source"))?;
        let mut current=None;let mut previous:Option<&Statement>=None;let mut path=Vec::new();let mut previous_path=Vec::new();let mut nearest=None;
        for part in &origin.route{
            let statement=match (current,part){
                (None,AstRoutePart::Root{index})=>authored.get(*index),
                (Some(parent),_)=>super::child_statements(parent).into_iter().find_map(|(statement,actual)|(actual==*part).then_some(statement)),
                _=>None,
            }.ok_or_else(||invalid("Native Break route differs from its actual authored AST"))?;
            if matches!(statement,Statement::ForLoopWithBreak{..}){
                nearest=if previous.is_some_and(|parent|matches!(parent,Statement::ScheduledConsumer{version:3,..})){
                    self.bodies.values().find(|body|body.header.route==previous_path&&body.header.source==origin.source)
                }else{None};
            }
            previous=Some(statement);previous_path=path.clone();previous_path.push(part.clone());path=previous_path.clone();current=Some(statement);
        }
        if !matches!(current,Some(Statement::LoopBreak)){return Err(invalid("Native Break route does not end at its actual source instruction"));}
        nearest.ok_or_else(||invalid("Reached Break requires its nearest actual Range adapter; a Legacy range cannot borrow a Native latch"))
    }
}

#[cfg(test)]
#[path="program_native_loop_tests.rs"]
mod tests;
