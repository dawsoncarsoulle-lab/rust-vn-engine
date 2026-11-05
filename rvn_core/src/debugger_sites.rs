//! AST sites are derived from the accepted program. Observations cannot turn
//! an expression, a nonexistent branch or another declaration into a stop.
use super::{Route, Site};
use crate::program::{AstRoutePart, InstructionOrigin};
use rvn_parser::{Statement, StatementOrigin, StatementSpan};
use serde::{Deserialize,Serialize};
use std::collections::{HashMap,HashSet};

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct DebugOrigin{
    pub scope:String,
    pub route:Vec<Route>,
    pub origin:InstructionOrigin,
    pub declaration:InstructionOrigin,
}
pub fn declaration_scope(statement:&Statement)->Option<String>{match statement{
    Statement::Function{name,..}|Statement::UnitFunction{name,..}=>Some(name.clone()),
    Statement::FunctionLibrary{scope}=>Some(format!("library:{}",scope.library().as_str())),
    Statement::Screen{name,..}=>Some(format!("screen:{name}")),
    Statement::Handler{name,..}=>Some(format!("handler:{name}")),_=>None,
}}
/// Diagnostic keys are derived from the actual accepted member declarations.
/// Runtime access never resolves a caller from this string.
pub fn declaration_scope_keys(statement:&Statement)->Vec<String>{
    if let Statement::FunctionLibrary{scope}=statement{
        return scope.members().iter().map(|member|format!("library:{}:{}",scope.library().as_str(),member.graph_id())).collect();
    }
    declaration_scope(statement).into_iter().collect()
}
fn library_member_body<'a>(root:&'a Statement,key:&str)->Option<&'a[Statement]>{
    let Statement::FunctionLibrary{scope}=root else{return None;};
    scope.members().iter().find_map(|member|{
        (key==format!("library:{}:{}",scope.library().as_str(),member.graph_id()))
            .then(||declaration_body(member.declaration())).flatten()
    })
}
fn declaration_body(statement:&Statement)->Option<&[Statement]>{match statement{
    Statement::Function{body,..}|Statement::UnitFunction{body,..}|Statement::Screen{body,..}|Statement::Handler{body,..}=>Some(body),_=>None,
}}
fn emitted(statement:&Statement)->bool{!matches!(statement,Statement::Use{..}|Statement::Init{..}|Statement::Function{..}|Statement::UnitFunction{..}|Statement::FunctionLibrary{..}|Statement::Screen{..}|Statement::Handler{..})}

pub(crate) fn validate_site(root:&Statement,site:&Site)->bool{
    if site.route.len()>256||site.depth>crate::eval::MAX_FUNCTION_DEPTH{return false;}
    let body=if let Some(scope)=&site.function{
        if matches!(root,Statement::FunctionLibrary{..}){
            let Some(body)=library_member_body(root,scope)else{return false;};body
        }else{
            if declaration_scope(root).as_ref()!=Some(scope){return false;}
            let Some(body)=declaration_body(root)else{return false;};body
        }
    }else if let Statement::Init{body}=root{body}else{std::slice::from_ref(root)};
    if !site.statement&&site.function.is_none()&&!matches!(root,Statement::Init{..}){
        return matches!((root,site.route.as_slice()),(Statement::ScheduledConsumer{steps,..},[Route::Producer(index)]) if *index<steps.len());
    }
    let mut body=body;let mut route=site.route.as_slice();
    loop{
        let Some((Route::Statement(index),tail))=route.split_first()else{return false;};
        let Some(statement)=body.get(*index)else{return false;};
        if tail.is_empty(){return site.statement&&emitted(statement);}
        if let [Route::Producer(index)]=tail{
            return !site.statement&&match statement{Statement::ScheduledConsumer{steps,..}=>*index<steps.len(),Statement::StandardMacroDispatch{dispatch}=>*index<dispatch.operand_steps().len(),_=>false};
        }
        let Some((part,next))=tail.split_first()else{return false;};
        body=match(statement,part){
            (Statement::If{then_branch,..},Route::Then)=>then_branch,
            (Statement::If{else_branch,..},Route::Else)=>else_branch,
            (Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}|Statement::ScheduledConsumer{body,..},Route::Body)=>body,
            (Statement::Sequence{branches},Route::Branch(index))=>{let Some(branch)=branches.get(*index)else{return false;};branch},
            (Statement::StandardMacroDispatch{dispatch},Route::Branch(index))=>{let Some(branch)=dispatch.branches.get(*index)else{return false;};&branch.body},
            (Statement::Choice{options},Route::Option(index))=>{let Some(option)=options.get(*index)else{return false;};&option.body},
            (Statement::Imagemap{hotspots,..},Route::Option(index))=>{let Some(hotspot)=hotspots.get(*index)else{return false;};&hotspot.body},
            _=>return false,
        };route=next;
    }
}

/// Validation reads frozen source bytes only. It never invents a graph ID or
/// reparses a changed file while a debugger session is suspended.
pub(crate) fn validate_program_origins(program:&rvn_parser::ParsedProgram)->Result<(),String>{
    if program.script.len()!=program.origins.len(){return Err("Debugger provenance is incomplete".into());}
    let mut sources=HashMap::new();
    for source in &program.sources{
        if sources.contains_key(&source.path){return Err("Debugger source identity is ambiguous".into());}
        let parsed=rvn_parser::parse_with_origins(&source.source,program.context).map_err(|error|error.to_string())?;
        sources.insert(source.path.clone(),parsed);
    }
    let mut used=HashSet::new();
    for(statement,origin)in program.script.iter().zip(&program.origins){
        let parsed=sources.get(&origin.source).ok_or("Debugger source snapshot is absent")?;
        if !used.insert((origin.source.clone(),origin.span.range.clone()))
            ||!parsed.script.iter().zip(&parsed.spans).any(|(actual,span)|actual==statement&&span==&origin.span){
            return Err("Debugger AST range does not match captured UTF-8 source".into());
        }
    }Ok(())
}

pub(crate) fn collect_origins(script:&[Statement],origins:Option<&[StatementOrigin]>)->Result<Vec<DebugOrigin>,String>{
    if origins.is_some_and(|origins|origins.len()!=script.len()){return Err("Debugger provenance is incomplete".into());}
    let mut out=Vec::new();
    for(index,statement)in script.iter().enumerate(){
        if let Statement::FunctionLibrary{scope}=statement{
            scope.validate()?;
            let root=origins.map(|origins|&origins[index]);
            if root.is_some_and(|root|root.span.children.len()!=scope.members().len()){
                return Err("Library member source ranges differ from actual declarations".into());
            }
            for(member_index,member)in scope.members().iter().enumerate(){
                let span=root.map(|root|&root.span.children[member_index]);
                let key=format!("library:{}:{}",scope.library().as_str(),member.graph_id());
                let declaration=InstructionOrigin{source:root.and_then(|root|root.source.clone()),
                    span:span.map(|span|span.range.clone()),
                    route:vec![AstRoutePart::Root{index},AstRoutePart::Body{index:member_index}],
                    generated:None,graph:Some(crate::program::GraphInstructionOrigin{
                        source:root.and_then(|root|root.source.clone()),graph:member.graph_id(),node:None}),scheduled_step:None};
                let body=declaration_body(member.declaration()).ok_or("Library member body absent")?;
                block(body,span.map(|span|span.children.as_slice()),&key,&declaration,&[],&declaration.route,None,&mut out)?;
            }
            continue;
        }
        let(scope,body)=if let Statement::Init{body}=statement{(String::new(),body.as_slice())}else{
            let Some(scope)=declaration_scope(statement)else{continue;};
            (scope,declaration_body(statement).ok_or("Debugger declaration body is absent")?)
        };
        let origin=origins.map(|origins|&origins[index]);
        let declaration=InstructionOrigin{source:origin.and_then(|origin|origin.source.clone()),span:origin.map(|origin|origin.span.range.clone()),
            route:vec![AstRoutePart::Root{index}],generated:None,graph:None,scheduled_step:None};
        block(body,origin.map(|origin|origin.span.children.as_slice()),&scope,&declaration,&[],&declaration.route,None,&mut out)?;
    }Ok(out)
}
fn block(body:&[Statement],spans:Option<&[StatementSpan]>,scope:&str,declaration:&InstructionOrigin,
    route:&[Route],ast_route:&[AstRoutePart],parent:Option<&Statement>,out:&mut Vec<DebugOrigin>)->Result<(),String>{
    if route.len()>256{return Err("Debugger nested site depth exceeds the program bound".into());}
    if spans.is_some_and(|spans|spans.len()!=body.len()){return Err("Debugger nested ranges differ from the actual body".into());}
    for(index,statement)in body.iter().enumerate(){
        if out.len()>=100_000{return Err("Debugger emitted sites exceed the program bound".into());}
        let mut route=route.to_vec();route.push(Route::Statement(index));
        let mut authored=ast_route.to_vec();authored.push(match route.get(route.len().saturating_sub(2)){
            Some(Route::Then)=>AstRoutePart::Then{index},Some(Route::Else)=>AstRoutePart::Else{index},
            Some(Route::Branch(branch))=>AstRoutePart::Sequence{branch:*branch,index},
            Some(Route::Option(branch))=>match parent{
                Some(Statement::Choice{..})=>AstRoutePart::Choice{option:*branch,index},
                Some(Statement::Imagemap{..})=>AstRoutePart::Hotspot{hotspot:*branch,index},
                _=>return Err("Debugger option route has no actual selection parent".into()),
            },_=>AstRoutePart::Body{index},
        });
        let span=spans.map(|spans|&spans[index]);
        let origin=InstructionOrigin{source:declaration.source.clone(),span:span.map(|span|span.range.clone()),
            route:authored.clone(),generated:None,graph:match statement {
                Statement::StandardMacroDispatch{dispatch}=>{dispatch.validate()?;Some(crate::program::GraphInstructionOrigin{
                    source:declaration.source.clone(),graph:dispatch.site.node.graph,node:Some(dispatch.site.node.node)})},_=>None,
            },scheduled_step:None};
        out.push(DebugOrigin{scope:scope.into(),route:route.clone(),origin,declaration:declaration.clone()});
        let children=span.map(|span|span.children.as_slice());
        let groups:Vec<(Route,&[Statement])>=match statement{
            Statement::If{then_branch,else_branch,..}=>vec![(Route::Then,then_branch),(Route::Else,else_branch)],
            Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}|Statement::ScheduledConsumer{body,..}=>vec![(Route::Body,body)],
            Statement::Sequence{branches}=>branches.iter().enumerate().map(|(index,body)|(Route::Branch(index),body.as_slice())).collect(),
            Statement::StandardMacroDispatch{dispatch}=>dispatch.branches.iter().enumerate().map(|(index,branch)|(Route::Branch(index),branch.body.as_slice())).collect(),
            Statement::Choice{options}=>options.iter().enumerate().map(|(index,option)|(Route::Option(index),option.body.as_slice())).collect(),
            Statement::Imagemap{hotspots,..}=>hotspots.iter().enumerate().map(|(index,hotspot)|(Route::Option(index),hotspot.body.as_slice())).collect(),
            _=>Vec::new(),
        };
        let count=groups.iter().map(|(_,body)|body.len()).sum::<usize>();
        if children.is_some_and(|children|children.len()!=count){return Err("Debugger nested ranges differ from the emitted branches".into());}
        let mut offset=0;
        for(part,body)in groups{
            let mut next=route.clone();next.push(part);
            block(body,children.map(|children|&children[offset..offset+body.len()]),scope,declaration,&next,&authored,Some(statement),out)?;
            offset+=body.len();
        }
    }Ok(())
}


#[cfg(test)]
mod selection_origin_tests {
 use super::*;
 fn assert_selection_routes(source:&str,hotspot:bool){
  let parsed=rvn_parser::parse_with_origins(source,rvn_parser::NarrativeContext::ExplicitScenes).unwrap();
  let origins=parsed.spans.iter().cloned().map(|span|StatementOrigin{source:Some("authored-input.rvn".into()),span}).collect::<Vec<_>>();
  let collected=collect_origins(&parsed.script,Some(&origins)).unwrap();
  for (branch,needle) in [(0,"set selected=1"),(1,"set selected=2")] {
   let actual=collected.iter().find(|site|site.origin.span.as_ref().is_some_and(|span|source.get(span.clone())==Some(needle))).expect("exact selected-body UTF-8 span");
   assert!(actual.origin.route.iter().any(|part|if hotspot{matches!(part,AstRoutePart::Hotspot{hotspot,index:0}if *hotspot==branch)}else{matches!(part,AstRoutePart::Choice{option,index:0}if *option==branch)}));
   assert!(actual.route.contains(&Route::Option(branch)));
   let site=Site{function:None,route:actual.route.clone(),depth:0,statement:true};
   assert!(validate_site(&parsed.script[0],&site));
   let mut foreign=site;let part=foreign.route.iter_mut().find(|part|matches!(part,Route::Option(_))).unwrap();*part=Route::Option(2);
   assert!(!validate_site(&parsed.script[0],&foreign));
  }
  let mut truncated=origins.clone();
  // Keep the AST unchanged while dropping one actual hotspot/option range.
  truncated[0].span.children[0].children[0].children.pop();
  assert!(collect_origins(&parsed.script,Some(&truncated)).is_err());
 }
 #[test] fn init_imagemap_children_keep_actual_hotspot_routes_and_ranges(){
  assert_selection_routes("init{flow.consume_v2(\"actual\",\"map\"){imagemap{background:\"garden.png\" hotspot{area:(0,0,10,10)}=>{set selected=1} hotspot{name:\"é\" area:(10,0,20,10)}=>{set selected=2}}}}",true);
 }
 #[test] fn init_choice_children_keep_actual_option_routes_and_ranges(){
  assert_selection_routes("init{flow.consume_v2(\"actual\",\"choice\"){choice{\"é\"=>{set selected=1} \"B\"=>{set selected=2}}}}",false);
 }
}

#[cfg(test)]
#[path="debugger_unit_sites_tests1021.rs"]
mod unit_sites1021;

#[cfg(test)]
mod unit_macro_composition_tests1021 {
    use super::*;

    #[test]
    fn actual_graph_macro_and_unit_keep_branch_routes_ranges_and_graph_identity() {
        let script = rvn_parser::parse("handler allowed(event){set hits=hits+1}").unwrap();
        let mut graph = rvn_graph::import_script(&script).unwrap().into_iter()
            .find(|graph| matches!(&graph.kind, rvn_graph::GraphKind::Handler{name} if name=="allowed"))
            .unwrap();
        let entry = graph.nodes.values().find(|node| node.kind==rvn_graph::NodeKind::HandlerEntry).unwrap().id;
        let output = graph.pin_by_key(entry,"exec_out").unwrap().id;
        let edge = graph.edges.values().find(|edge| edge.output==output).unwrap().clone();
        let once = graph.add_catalog_node(rvn_graph::NodeKind::DoOnce,[400.0,0.0]).unwrap();
        let enter = graph.pin_by_key(once,"exec_in").unwrap().id;
        let completed = graph.pin_by_key(once,"completed").unwrap().id;
        graph.remove_edge(edge.id).unwrap();
        graph.connect(output,enter).unwrap();
        graph.connect(completed,edge.input).unwrap();
        graph.enable_native_frame_preparation().unwrap();
        let generated = rvn_graph::prepare_native_frame_graph(&graph).unwrap();
        let source = format!("function effect(n)->(){{set result=n}}\n{}\nscene label start\nreturn\n",generated.source);
        let program = rvn_parser::parse_program(&source).unwrap();
        let map = crate::debugger_provenance::SourceMap::from_program(&program).unwrap();
        let unit = program.script.iter().find(|s| matches!(s,Statement::UnitFunction{name,..} if name=="effect")).unwrap();
        let unit_site = map.sites.iter().find(|site| site.scope=="effect").unwrap();
        assert_eq!(source.get(unit_site.origin.span.clone().unwrap()),Some("set result=n"));
        assert!(validate_site(unit,&Site{function:Some("effect".into()),route:unit_site.route.clone(),depth:1,statement:true}));
        assert!(unit_site.origin.graph.is_none());
        let handler = program.script.iter().find(|s| matches!(s,Statement::Handler{name,..} if name=="allowed")).unwrap();
        let Statement::Handler{body,..} = handler else {unreachable!()};
        let (dispatch_index,dispatch) = body.iter().enumerate().find_map(|(index,s)| match s {
            Statement::StandardMacroDispatch{dispatch}=>Some((index,dispatch)),_=>None,
        }).unwrap();
        let dispatch_site = map.sites.iter().find(|site| site.scope=="handler:allowed" && site.route==vec![Route::Statement(dispatch_index)]).unwrap();
        let origin = dispatch_site.origin.graph.as_ref().expect("actual macro GraphInstructionOrigin");
        assert_eq!((origin.graph,origin.node),(dispatch.site.node.graph,Some(dispatch.site.node.node)));
        assert_eq!(origin.source,dispatch_site.origin.source);
        let branch_route = vec![Route::Statement(dispatch_index),Route::Branch(0),Route::Statement(0)];
        let branch = map.sites.iter().find(|site| site.scope=="handler:allowed" && site.route==branch_route).unwrap();
        let handler_index = program.script.iter().position(|s| std::ptr::eq(s,handler)).unwrap();
        let actual_branch_span = &program.origins[handler_index].span.children[dispatch_index].children[0];
        assert_eq!(branch.origin.span,Some(actual_branch_span.range.clone()));
        assert!(source.get(actual_branch_span.range.clone()).is_some_and(|text| text.contains("hits")));
        assert!(validate_site(handler,&Site{function:Some("handler:allowed".into()),route:branch_route,depth:1,statement:true}));
        assert!(!validate_site(handler,&Site{function:Some("handler:allowed".into()),route:vec![Route::Statement(dispatch_index),Route::Branch(1),Route::Statement(0)],depth:1,statement:true}));
        let mut truncated = program.origins.clone();
        truncated[handler_index].span.children[dispatch_index].children.clear();
        assert!(collect_origins(&program.script,Some(&truncated)).is_err());
        // Source-map collection grants no Widget or live Unit execution authority.
    }
}
