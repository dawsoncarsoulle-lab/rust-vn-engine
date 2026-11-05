//! Original plan steps are visible at ConsumerNext, before any value/effect.
use crate::{editor_debugger::{DebugOrigin,Route},program::{AstRoutePart,InstructionOrigin,GraphInstructionOrigin}};
use rvn_parser::{ParsedProgram,scheduled_source::{ScheduledSourceRoute as R,scheduled_step_sources}};

pub(crate) fn collect(program:&ParsedProgram)->Result<Vec<DebugOrigin>,String>{
    let mut sites=crate::editor_debugger::collect_origins(&program.script,Some(&program.origins))?;
    for step in scheduled_step_sources(program)?{
        let root=&program.origins[step.definition_root_ordinal];
        let source=root.source.clone();
        let mut declaration_route=vec![AstRoutePart::Root{index:step.definition_root_ordinal}];
        let declaration_span=if let Some(R::LibraryMember(index))=step.route.first(){
            let rvn_parser::Statement::FunctionLibrary{scope}= &program.script[step.definition_root_ordinal] else{return Err("Library producer has no actual scope".into());};
            let member=scope.members().get(*index).ok_or("Library producer member absent")?;
            let expected=format!("library:{}:{}",scope.library().as_str(),member.graph_id());
            if step.scope.as_deref()!=Some(expected.as_str()){return Err("Library producer scope differs from actual member".into());}
            declaration_route.push(AstRoutePart::Body{index:*index});
            &root.span.children.get(*index).ok_or("Library producer declaration span absent")?.range
        }else{&root.span.range};
        let mut authored=declaration_route.clone();
        let mut route=Vec::new();let mut previous:Option<&R>=None;
        for (ordinal,part) in step.route.iter().enumerate(){
            if let R::LibraryMember(_)=part{if ordinal!=0{return Err("Nested Library producer marker".into());}continue;}
            route.push(match part{R::Statement(i)=>Route::Statement(*i),R::Body=>Route::Body,R::Then=>Route::Then,R::Else=>Route::Else,R::Branch(i)=>Route::Branch(*i),R::Choice(i)|R::Hotspot(i)=>Route::Option(*i),R::LibraryMember(_)=>unreachable!()});
            if let R::Statement(index)=part{
                authored.push(match previous{Some(R::Then)=>AstRoutePart::Then{index:*index},Some(R::Else)=>AstRoutePart::Else{index:*index},Some(R::Branch(branch))=>AstRoutePart::Sequence{branch:*branch,index:*index},Some(R::Choice(option))=>AstRoutePart::Choice{option:*option,index:*index},Some(R::Hotspot(hotspot))=>AstRoutePart::Hotspot{hotspot:*hotspot,index:*index},_=>AstRoutePart::Body{index:*index}});
            }previous=Some(part);
        }
        // Narrative lowering emits the actual wrapper at its own PC. Its
        // continuation route starts at that wrapper, not the enclosing root.
        let initializer=matches!(program.script[step.definition_root_ordinal],rvn_parser::Statement::Init{..});
        if step.scope.is_none()&&!initializer{route.clear();}
        route.push(Route::Producer(step.index));authored.push(AstRoutePart::Producer{index:step.index});
        let declaration=InstructionOrigin{source:source.clone(),span:Some(if step.scope.is_some()||initializer{declaration_span.clone()}else{step.consumer.clone()}),
            route:declaration_route,generated:None,graph:None,scheduled_step:None};
        let graph=step.endpoint.as_ref().map(|e|GraphInstructionOrigin{source:source.clone(),graph:e.graph,node:Some(e.node)});
        let scope=step.scope.clone().unwrap_or_default();
        let origin=InstructionOrigin{source,span:Some(step.span.clone()),route:authored,generated:None,graph,scheduled_step:Some(step)};
        sites.push(DebugOrigin{scope,route,origin,declaration});
        if sites.len()>100_000{return Err("Debugger step catalogue budget".into());}
    }Ok(sites)
}
