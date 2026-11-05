//! Source-local reconstruction of the actual loop/output. This uses Builder's
//! existing identity map; it never creates a VariableGet carrying a loop ID.
use super::*;
use rvn_parser::scheduled_source::native_loop_contract::Binding;

pub(super) fn native_index_body(statement:&Statement)->Option<(&str,&[Statement])>{
    let Statement::ScheduledConsumer{version:3,body,..}=statement else{return None;};
    match body.as_slice(){[Statement::ForEach{name,body,..}|Statement::ForLoopWithBreak{name,body,..}]=>Some((name,body)),_=>None}
}

#[derive(Clone)]
pub(super) struct NativeLoopDeclaration {
    pub(super) identity:crate::native_frame_preparation::NativeNodeIdentity,
    pub(super) binding:Binding,
}
pub(super) struct NativeLoopBody {
    pub(super) node:NodeId,
    pub(super) declaration:NativeLoopDeclaration,
    pub(super) collection:Option<Expr>,
}
impl Builder<'_> {
    pub(super) fn install_native_loop_declaration(&mut self,node:NodeId)->Result<(),String>{
        let Some(declaration)=self.native_loop_pending.take() else{return Ok(());};
        if declaration.identity.kind!=self.graph.nodes[&node].kind{return Err("Native loop source kind differs before body reconstruction".into());}
        self.graph.install_native_loop_demand(node)?;
        self.native_nodes.insert(node,declaration.identity.clone());
        self.native_loop_declarations.insert(node,declaration);
        Ok(())
    }
    pub(super) fn enter_native_loop_source_body(&mut self,node:NodeId,collection:Option<&Expr>)->bool{
        let Some(declaration)=self.native_loop_declarations.get(&node).cloned() else{return false;};
        self.native_loop_bodies.push(NativeLoopBody{node,declaration,collection:collection.cloned()});true
    }
    pub(super) fn native_loop_index_source(&self,expression:&Expr)->Option<Result<ExpressionRef,String>>{
        let Expr::Var(name)=expression else{return None;};
        let owner=self.native_loop_bodies.iter().rev().find(|o|o.declaration.binding.index==*name)?;
        let role=if owner.declaration.identity.kind==NodeKind::ForEach{"array_index"}else{"index"};
        Some(self.graph.pin_by_key(owner.node,role).map(|p|ExpressionRef{node:owner.node,output:p.id}).ok_or_else(||"Actual Native index output is absent".into()))
    }
    pub(super) fn native_array_element_source(&mut self,identity:&crate::native_frame_preparation::NativeNodeIdentity,value:&Expr,pos:[f64;2])->Result<ExpressionRef,String>{
        let owner=self.native_loop_bodies.iter().rev().find(|o|o.declaration.identity.node==identity.node).ok_or("Native ArrayElement has no actual active source loop body")?;
        if owner.declaration.identity!=*identity||identity.kind!=NodeKind::ForEach{return Err("Native ArrayElement identity differs from the active loop".into());}
        let Expr::Index{target,index}=value else{return Err("Native ArrayElement must retain its real lazy indexed expression".into());};
        if **index!=Expr::Var(owner.declaration.binding.index.clone()){return Err("Native ArrayElement index belongs to another body".into());}
        let node=owner.node;
        let collection=self.graph.pin_by_key(node,"collection").ok_or("Actual Array input absent")?.id;
        let wire=self.graph.edges.values().find(|e|e.input==collection).map(|e|e.output);
        if let Some(output)=wire{
            let source=self.expression(target,pos)?;
            if source.output!=output{return Err("Native ArrayElement does not read its actual Array input wire".into());}
        }else if owner.collection.as_ref()!=Some(target.as_ref()){
            // This is exact authored expression/default identity, not equality
            // of evaluated values. No connected source can use this branch.
            return Err("Native ArrayElement default differs from its actual declared Array input expression".into());
        }
        let output=self.graph.pin_by_key(node,"array_element").ok_or("Actual ArrayElement output absent")?.id;
        Ok(ExpressionRef{node,output})
    }
}
