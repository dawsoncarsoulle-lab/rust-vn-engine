//! Read-only slicing of an existing frozen capture DAG by a loop input root.
//! This does not change consume_v2 execution, introduce IR/wire, or prove an
//! owner. A future explicit loop backend can reuse these exact phase slices.
use crate::{ConsumerStep, Expr, Statement};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoopHeaderDependencies {
    Each { length: Vec<usize> },
    Range { first: Vec<usize>, last: Vec<usize> },
}

#[derive(Clone, Copy)]
enum ReferenceKind { Capture, Inspector }

fn references<'a>(root: &'a Expr) -> Result<Vec<(&'a str, ReferenceKind)>, String> {
    let mut pending = vec![(root,0usize)];
    let mut result = vec![];
    while let Some((value,depth)) = pending.pop() {
        if depth > 128 { return Err("Loop phase dependency exceeds existing128-level capture expression depth".into()); }
        match value {
            Expr::LibraryCall{call}=>pending.extend(call.arguments().iter().map(|(_,argument)|(argument,depth+1))),
            Expr::Call {name,args} if name == "__rvn_captured_v1" => {
                let [Expr::Str(id)] = args.as_slice() else { return Err("Loop phase capture requires its literal source identity".into()); };
                result.push((id.as_str(), ReferenceKind::Capture));
            },
            Expr::Call {name,args} if name == "__rvn_terminal_v1" => {
                let [Expr::Str(id),Expr::Str(port)] = args.as_slice() else { return Err("Loop phase inspector requires literal identity/port".into()); };
                if !matches!(port.as_str(),"valid"|"table_id"|"key") { return Err("Loop phase inspector terminal role differs".into()); }
                result.push((id.as_str(), ReferenceKind::Inspector));
            },
            Expr::Call {args,..} | Expr::ListLit(args) => pending.extend(args.iter().map(|arg|(arg,depth+1))),
            Expr::BinOp {left,right,..} | Expr::Index {target:left,index:right}
                | Expr::And(left,right) | Expr::Or(left,right) => { pending.push((left,depth+1)); pending.push((right,depth+1)); },
            Expr::Neg(value) | Expr::Not(value) => pending.push((value,depth+1)),
            _ => {},
        }
    }
    Ok(result)
}

struct Dag<'a> {
    steps: &'a [ConsumerStep],
    by_identity: BTreeMap<&'a str,usize>,
    predecessors: Vec<Vec<usize>>,
}
impl<'a> Dag<'a> {
    fn new(steps: &'a [ConsumerStep]) -> Result<Self,String> {
        let mut dag=Self {steps,by_identity:BTreeMap::new(),predecessors:vec![]};
        for (index,step) in steps.iter().enumerate() {
            let (producer,value) = match step { ConsumerStep::Capture {producer,value} | ConsumerStep::InspectTextTable {producer,value} => (producer,value) };
            if producer.is_empty() || dag.by_identity.contains_key(producer.as_str()) { return Err("Loop phase duplicate/empty producer identity".into()); }
            // Only previously defined captures are legal. Equal expressions or
            // equal values cannot substitute a missing source identity.
            let inputs=dag.resolve(value)?;
            dag.predecessors.push(inputs);
            dag.by_identity.insert(producer,index);
        }
        Ok(dag)
    }
    fn resolve(&self,value: &Expr) -> Result<Vec<usize>,String> {
        references(value)?.into_iter().map(|(id,kind)| {
            let index=*self.by_identity.get(id).ok_or("Loop phase has missing or forward source capture")?;
            match (kind,&self.steps[index]) {
                (ReferenceKind::Capture,ConsumerStep::Capture {..}) | (ReferenceKind::Inspector,ConsumerStep::InspectTextTable {..}) => Ok(index),
                _ => Err("Loop phase source reference kind differs from real capture".into()),
            }
        }).collect()
    }
    fn slice(&self,value: &Expr) -> Result<Vec<usize>,String> {
        let mut pending=self.resolve(value)?;
        let mut needed=BTreeSet::new();
        while let Some(index)=pending.pop() {
            if needed.insert(index) { pending.extend(self.predecessors[index].iter().copied()); }
        }
        // Preserve the original, already topological capture order. A common
        // producer can appear in BOTH phase results: each phase owns its cache.
        Ok(needed.into_iter().collect())
    }
}

/// Original producer ordinals required by exactly one reached input root.
/// Values/equal expressions never establish source identity. Each caller owns
/// a fresh ActiveConsumer for this slice; the result is not a global cache.
pub fn capture_step_indices(steps:&[ConsumerStep],root:&Expr)->Result<Vec<usize>,String>{
    Dag::new(steps)?.slice(root)
}

pub fn analyze_loop_header_dependencies(statement: &Statement) -> Result<Option<LoopHeaderDependencies>,String> {
    let Statement::ScheduledConsumer {version:2,steps,body,..}=statement else { return Ok(None); };
    let [loop_statement]=body.as_slice() else { return Ok(None); };
    match loop_statement {
        Statement::ForEach {collection,..} => {
            let dag=Dag::new(steps)?;
            Ok(Some(LoopHeaderDependencies::Each {length:dag.slice(collection)?}))
        },
        Statement::ForLoopWithBreak {first,last,..} => {
            let dag=Dag::new(steps)?;
            Ok(Some(LoopHeaderDependencies::Range {first:dag.slice(first)?,last:dag.slice(last)?}))
        },
        _=>Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn capture(id: &str,value: Expr)->ConsumerStep {ConsumerStep::Capture{producer:id.into(),value}}
    fn read(id: &str)->Expr {Expr::Call{name:"__rvn_captured_v1".into(),args:vec![Expr::Str(id.into())]}}
    fn range(steps: Vec<ConsumerStep>,first:Expr,last:Expr)->Statement {
        Statement::ScheduledConsumer{version:2,scope:"analysis_only".into(),consumer:"not_an_owner".into(),steps,
            body:vec![Statement::ForLoopWithBreak{name:"i".into(),first,last,body:vec![]}]}
    }
    #[test]
    fn first_and_last_slices_do_not_merge_a_shared_supplier_across_phases() {
        let statement=range(vec![capture("shared",Expr::Int(7)),capture("last",read("shared"))],read("shared"),read("last"));
        assert_eq!(analyze_loop_header_dependencies(&statement).unwrap(),Some(LoopHeaderDependencies::Range{first:vec![0],last:vec![0,1]}));
    }
    #[test]
    fn equal_expressions_are_not_source_identity_and_unused_captures_are_not_selected() {
        let statement=range(vec![capture("A",Expr::Int(7)),capture("B",Expr::Int(7)),capture("unused",Expr::Int(7))],read("A"),read("B"));
        assert_eq!(analyze_loop_header_dependencies(&statement).unwrap(),Some(LoopHeaderDependencies::Range{first:vec![0],last:vec![1]}));
        let missing=range(vec![capture("A",Expr::Int(7))],read("B"),Expr::Int(9));
        assert!(analyze_loop_header_dependencies(&missing).is_err());
    }
    #[test]
    fn forward_duplicate_and_wrong_kind_references_are_rejected_before_any_slice() {
        for steps in [vec![capture("A",read("B")),capture("B",Expr::Int(0))],
            vec![capture("A",Expr::Int(0)),capture("A",Expr::Int(0))],
            vec![ConsumerStep::InspectTextTable{producer:"A".into(),value:Expr::Str("".into())}]] {
            assert!(analyze_loop_header_dependencies(&range(steps,read("A"),Expr::Int(0))).is_err());
        }
    }
    #[test]
    fn historical_loop_ast_remains_identical_after_analysis() {
        let statement=range(vec![capture("A",Expr::Int(1))],Expr::Int(0),read("A"));
        let before=statement.clone();
        analyze_loop_header_dependencies(&statement).unwrap();
        assert_eq!(statement,before);
        let legacy=Statement::ForEach{name:"item".into(),collection:Expr::ListLit(vec![]),body:vec![]};
        assert_eq!(analyze_loop_header_dependencies(&legacy).unwrap(),None);
    }
}
