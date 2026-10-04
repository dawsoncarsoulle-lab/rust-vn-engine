//! Project-wide callable rename. Only names used as symbols are changed;
//! ordinary strings and connected computed values are never globally replaced.
use crate::{GraphDocument, GraphId, GraphKind, NodeId, NodeKind, PinId, PropertyValue};
use rvn_parser::{Expr, TextSegment};

pub fn callable_name(kind: &GraphKind) -> Option<&str> {
    match kind { GraphKind::Function{name}|GraphKind::Screen{name}|GraphKind::Handler{name}=>Some(name), _=>None }
}
pub fn validate_callable_name(kind:&GraphKind,name:&str)->Result<(),String>{
    if name.is_empty()||!name.chars().next().is_some_and(|c|c.is_ascii_alphabetic()||c=='_')||!name.chars().all(|c|c.is_ascii_alphanumeric()||c=='_'){
        return Err("Le nom doit commencer par une lettre ou _, puis contenir uniquement lettres, chiffres et _.".into());
    }
    if rvn_parser::builtin_arity(name).is_some(){return Err(format!("Le nom {name} est réservé à une fonction intégrée."));}
    let declaration=match kind{GraphKind::Function{..}=>format!("function {name}(){{return 0}}"),GraphKind::Screen{..}=>format!("screen {name}(){{return component(\"root\",\"panel\",{{}},[])}}"),GraphKind::Handler{..}=>format!("handler {name}(event){{local accepted=true}}"),_=>return Err("Ce graphe n’est pas une fonction, un écran ou un gestionnaire.".into())};
    rvn_parser::parse(&declaration).map_err(|error|format!("Nom RVN invalide : {error}"))?;Ok(())
}

fn expression(expr:&mut Expr,old:&str,new:&str){
    match expr{
        Expr::Call{name,args}=>{if name==old{*name=new.into();}for arg in args{expression(arg,old,new);}},
        Expr::BinOp{left,right,..}|Expr::Index{target:left,index:right}|Expr::And(left,right)|Expr::Or(left,right)=>{expression(left,old,new);expression(right,old,new);},
        Expr::Neg(value)|Expr::Not(value)=>expression(value,old,new),Expr::ListLit(values)=>for value in values{expression(value,old,new)},_=>{}
    }
}
fn expression_text(text:&str,old:&str,new:&str)->Result<String,String>{
    let parsed=rvn_parser::parse(&format!("local value={text}" )).map_err(|error|error.to_string())?;
    let Some(rvn_parser::Statement::LocalVar{value,..})=parsed.first()else{return Err("Expression RVN attendue".into());};
    let mut next=value.clone();expression(&mut next,old,new);
    Ok(if next==*value{text.into()}else{crate::import::expression_source(&next)})
}
fn interpolation(text:&str,old:&str,new:&str)->Result<String,String>{
    let parsed=rvn_parser::parse_interpolated_str(text).map_err(|error|error.to_string())?;
    let mut next=parsed.clone();for segment in &mut next.0{if let TextSegment::Interp(expr)=segment{expression(expr,old,new);}}
    if parsed==next{return Ok(text.into());}
    Ok(next.0.iter().map(|segment|match segment{TextSegment::Lit(text)=>text.replace('[',"[["),TextSegment::Interp(expr)=>format!("[{}]",crate::import::expression_source(expr))}).collect())
}
fn literal_reference(graph:&mut GraphDocument,node:NodeId,key:&str,old:&str,new:&str)->Result<(),String>{
    match graph.literal_input_value(node,key){
        Ok(Some(PropertyValue::String(value)))if value==old=>graph.set_literal_input(node,key,PropertyValue::String(new.into())),
        Ok(_)=>Ok(()),
        Err(_)=>{
            // A dynamically constructed name may include the old symbol. Do
            // not guess at its value or overwrite its dataflow expression.
            let mut pending=graph.pin_by_key(node,key).map(|pin|vec![pin.id]).unwrap_or_default();let mut seen=std::collections::BTreeSet::new();
            while let Some(pin)=pending.pop(){if !seen.insert(pin){continue;}if graph.pins[&pin].default_value==Some(PropertyValue::String(old.into())){return Err(format!("La référence {old} est calculée dans le nœud {}. Sélectionnez son nom explicitement avant de le renommer.",node.get()));}
                for edge in graph.edges.values().filter(|edge|edge.input==pin){let source=&graph.nodes[&graph.pins[&edge.output].node];if source.properties.get("value")==Some(&PropertyValue::String(old.into())){return Err(format!("La référence {old} est calculée dans le nœud {}. Sélectionnez son nom explicitement avant de le renommer.",node.get()));}pending.extend(source.pins.iter().filter(|id|graph.pins[id].direction==crate::PinDirection::Input).copied());}
            }Ok(())
        }
    }
}

fn property_pin(graph:&GraphDocument,component:NodeId,path:&[&str])->Option<PinId>{
    let mut pin=graph.pin_by_key(component,"properties")?.id;
    for key in path{
        let edge=graph.edges.values().find(|edge|edge.input==pin)?;let node=&graph.nodes[&graph.pins[&edge.output].node];
        if node.kind!=NodeKind::FunctionCall||node.properties.get("function")!=Some(&PropertyValue::String("dict".into())){return None;}
        let Some(PropertyValue::Int(count))=node.properties.get("input_count")else{return None;};
        pin=(0..*count as usize).step_by(2).find_map(|index|{(graph.literal_input_value(node.id,&format!("item_{index}")).ok().flatten()==Some(PropertyValue::String((*key).into()))).then(||graph.pin_by_key(node.id,&format!("item_{}",index+1)).map(|pin|pin.id)).flatten()})?;
    }Some(pin)
}
fn parameter_source(graph:&GraphDocument,pin:PinId,visited:&mut std::collections::BTreeSet<PinId>)->Option<String>{
    if !visited.insert(pin){return None;}let edge=graph.edges.values().find(|edge|edge.input==pin)?;let node=&graph.nodes[&graph.pins[&edge.output].node];
    if node.kind==NodeKind::VariableGet{if let Some(PropertyValue::String(name))=node.properties.get("name"){return Some(name.clone());}}
    if matches!(node.kind,NodeKind::Reroute|NodeKind::ConvertStringToText|NodeKind::ConvertTextToString){return parameter_source(graph,graph.pin_by_key(node.id,"value")?.id,visited);}None
}
fn reference_parameters(graphs:&[GraphDocument],kind:&GraphKind)->std::collections::BTreeSet<(String,usize)>{
    let mut result=std::collections::BTreeSet::<(String,usize)>::new();
    for graph in graphs{
        let Some(name)=callable_name(&graph.kind)else{continue;};
        let Some(parameters)=graph.nodes.values().find_map(|node|match node.properties.get("parameters"){Some(PropertyValue::StringList(parameters))=>Some(parameters),_=>None})else{continue;};
        for component in graph.nodes.values().filter(|node|node.kind==NodeKind::UiComponent){
            let paths:Vec<Vec<&str>>=if matches!(kind,GraphKind::Handler{..}){["click","activate","change","focus","key","open","close","pointer_down","pointer_move","pointer_up","pointer_cancel","wheel"].into_iter().map(|event|vec!["events",event]).collect()}else if matches!(kind,GraphKind::Function{..}){vec![vec!["draw"]]}else{vec![]};
            for path in paths{if let Some(parameter)=property_pin(graph,component.id,&path).and_then(|pin|parameter_source(graph,pin,&mut Default::default())){if let Some(index)=parameters.iter().position(|name|name==&parameter){result.insert((name.into(),index));}}}
        }
    }
    // Propagate a handler/drawing parameter through chains of pure helpers.
    loop{let before=result.len();for graph in graphs{let Some(name)=callable_name(&graph.kind)else{continue;};let Some(parameters)=graph.nodes.values().find_map(|node|match node.properties.get("parameters"){Some(PropertyValue::StringList(parameters))=>Some(parameters),_=>None})else{continue;};
        let bindings:Vec<_>=result.iter().cloned().collect();for node in graph.nodes.values().filter(|node|node.kind==NodeKind::FunctionCall){for(function,index)in &bindings{if node.properties.get("function")==Some(&PropertyValue::String(function.clone())){if let Some(parameter)=graph.pin_by_key(node.id,&format!("item_{index}")).and_then(|pin|parameter_source(graph,pin.id,&mut Default::default())){if let Some(index)=parameters.iter().position(|name|name==&parameter){result.insert((name.into(),index));}}}}}
    }if result.len()==before{break;}}
    result
}

/// Rewrite symbol references inside one history snapshot too. The caller
/// supplies the declaration family so a Screen never renames a Function call.
pub fn reconcile_callable_references(graph:&mut GraphDocument,kind:&GraphKind,old:&str,new:&str)->Result<(),String>{
    let nodes:Vec<_>=graph.nodes.keys().copied().collect();
    for id in nodes{
        let node_kind=graph.nodes[&id].kind;
        if matches!(kind,GraphKind::Function{..}){
            if node_kind==NodeKind::FunctionCall&&graph.nodes[&id].properties.get("function")==Some(&PropertyValue::String(old.into())){graph.nodes.get_mut(&id).unwrap().properties.insert("function".into(),PropertyValue::String(new.into()));}
            if node_kind==NodeKind::MotionCurve{literal_reference(graph,id,"function",old,new)?;}
            if node_kind==NodeKind::TextValue{if let Some(PropertyValue::String(value))=graph.nodes[&id].properties.get("value"){let next=interpolation(value,old,new)?;graph.nodes.get_mut(&id).unwrap().properties.insert("value".into(),PropertyValue::String(next));}}
            for(key,value)in &mut graph.nodes.get_mut(&id).unwrap().properties{if key.starts_with("option_condition_"){if let PropertyValue::String(text)=value{*text=expression_text(text,old,new)?;}}}
        }
        if matches!(kind,GraphKind::Screen{..})&&matches!(node_kind,NodeKind::UiOpen|NodeKind::UiClose|NodeKind::UiFocus|NodeKind::UiSetState){literal_reference(graph,id,"name",old,new)?;}
        if node_kind==NodeKind::UiComponent{
            let paths:Vec<Vec<&str>>=match kind{
                GraphKind::Function{..}=>vec![vec!["draw"]],
                GraphKind::Handler{..}=>["click","activate","change","focus","key","open","close","pointer_down","pointer_move","pointer_up","pointer_cancel","wheel"].into_iter().map(|event|vec!["events",event]).collect(),_=>Vec::new()
            };
            for path in paths{if graph.component_property(id,&path).ok().flatten()==Some(PropertyValue::String(old.into())){graph.set_component_property(id,&path,PropertyValue::String(new.into()))?;}}
        }
    }
    Ok(())
}
pub fn reconcile_callable_reference_history(graph:&mut GraphDocument,project:&[GraphDocument],kind:&GraphKind,old:&str,new:&str)->Result<(),String>{
    let bindings=reference_parameters(project,kind);
    let nodes:Vec<_>=graph.nodes.values().filter_map(|node|if node.kind==NodeKind::FunctionCall{if let Some(PropertyValue::String(function))=node.properties.get("function"){Some((node.id,function.clone()))}else{None}}else{None}).collect();
    for(node,function)in nodes{for(_,index)in bindings.iter().filter(|(name,_)|name==&function){literal_reference(graph,node,&format!("item_{index}"),old,new)?;}}
    reconcile_callable_references(graph,kind,old,new)
}

/// The source graph ID remains stable. Validation happens against every scope,
/// including closed graphs, before the caller can publish any file.
pub fn rename_callable_graphs(graphs:&[GraphDocument],id:GraphId,new:&str)->Result<Vec<GraphDocument>,String>{
    let target=graphs.iter().find(|graph|graph.graph_id==id).ok_or("La déclaration à renommer n’existe plus.")?;
    let old=callable_name(&target.kind).ok_or("Déclaration RVN attendue")?;validate_callable_name(&target.kind,new)?;
    if graphs.iter().filter(|graph|graph.graph_id==id).count()!=1{return Err("Deux graphes partagent l’identité de la déclaration ; le renommage n’a modifié aucun fichier.".into());}
    if graphs.iter().any(|graph|graph.graph_id!=id&&callable_name(&graph.kind)==Some(new)){return Err(format!("Une fonction, un écran ou un gestionnaire nommé {new} existe déjà."));}
    let mut result=graphs.to_vec();
    let bindings=reference_parameters(graphs,&target.kind);
    for graph in &mut result{
        let nodes:Vec<_>=graph.nodes.values().filter_map(|node|if node.kind==NodeKind::FunctionCall{if let Some(PropertyValue::String(function))=node.properties.get("function"){Some((node.id,function.clone()))}else{None}}else{None}).collect();
        for(node,function)in nodes{for(_,index)in bindings.iter().filter(|(name,_)|name==&function){literal_reference(graph,node,&format!("item_{index}"),old,new)?;}}
        reconcile_callable_references(graph,&target.kind,old,new)?;if graph.graph_id==id{graph.kind=match &graph.kind{GraphKind::Function{..}=>GraphKind::Function{name:new.into()},GraphKind::Screen{..}=>GraphKind::Screen{name:new.into()},GraphKind::Handler{..}=>GraphKind::Handler{name:new.into()},_=>unreachable!()};}
    }
    crate::transpile_project(&result).map_err(|error|error.to_string())?;Ok(result)
}

#[cfg(test)]mod tests{
    use super::*;
    fn graphs(source:&str)->Vec<GraphDocument>{crate::import_script(&rvn_parser::parse(source).unwrap()).unwrap()}
    #[test]fn rename_updates_closed_call_sites_and_leaves_ordinary_strings_and_identity(){
        let before=graphs("function caption(text){return text}\nscreen inventory(){return component(\"root\",\"panel\",{},[component(\"label\",\"text\",{\"text\":caption(\"caption\")},[])])}\nhandler open_inventory(event){ui.open_story(\"inventory\",[],true,5)}\nlabel start\n\"Value [caption(3)]\"\nreturn\n");
        let target=before.iter().find(|graph|callable_name(&graph.kind)==Some("caption")).unwrap().graph_id;
        let renamed=rename_callable_graphs(&before,target,"format_caption").unwrap();let source=crate::transpile_project(&renamed).unwrap().source;
        assert!(source.contains("format_caption("));assert!(source.contains("\"caption\""));assert_eq!(before.iter().map(|graph|graph.graph_id).collect::<Vec<_>>(),renamed.iter().map(|graph|graph.graph_id).collect::<Vec<_>>());
        let screen=renamed.iter().find(|graph|callable_name(&graph.kind)==Some("inventory")).unwrap().graph_id;let renamed=rename_callable_graphs(&renamed,screen,"bag").unwrap();assert!(crate::transpile_project(&renamed).unwrap().source.contains("ui.open_story(\"bag\""));
        for name in ["inventory","dict","label","bad-name"]{assert!(rename_callable_graphs(&before,target,name).is_err());}assert_eq!(callable_name(&before.iter().find(|graph|graph.graph_id==target).unwrap().kind),Some("caption"));
    }
    #[test]fn handler_and_drawing_symbols_do_not_replace_button_text(){
        let before=graphs(r#"function paint(state,props,frame){return []} handler accept(event){local accepted=true} screen form(){return component("root","panel",{},[component("a","button",{"text":"accept","events":{"click":"accept"}},[]),component("b","canvas",{"draw":"paint","state":{},"props":{}},[])])}"#);
        let handler=before.iter().find(|graph|callable_name(&graph.kind)==Some("accept")).unwrap().graph_id;let next=rename_callable_graphs(&before,handler,"confirm").unwrap();let screen=next.iter().find(|graph|matches!(graph.kind,GraphKind::Screen{..})).unwrap();let button=screen.nodes.values().find(|node|node.kind==NodeKind::UiComponent&&screen.literal_input_value(node.id,"id").unwrap()==Some(PropertyValue::String("a".into()))).unwrap().id;
        assert_eq!(screen.component_property(button,&["text"]).unwrap(),Some(PropertyValue::String("accept".into())));assert_eq!(screen.component_property(button,&["events","click"]).unwrap(),Some(PropertyValue::String("confirm".into())));
        let draw=next.iter().find(|graph|callable_name(&graph.kind)==Some("paint")).unwrap().graph_id;let next=rename_callable_graphs(&next,draw,"draw_surface").unwrap();assert!(crate::transpile_project(&next).unwrap().source.contains("draw_surface"));
    }
    #[test]fn handler_parameter_bindings_are_propagated_through_helpers_without_replacing_labels(){
        let before=graphs(r#"handler accept(event){local accepted=true} function button(text,event_handler){return component("a","button",{"text":text,"events":{"click":event_handler}},[])} function wrapped(text,event_handler){return button(text,event_handler)} screen form(){return component("root","panel",{},[wrapped("accept","accept")])}"#);
        let target=before.iter().find(|graph|callable_name(&graph.kind)==Some("accept")).unwrap().graph_id;let next=rename_callable_graphs(&before,target,"confirm").unwrap();let source=crate::transpile_project(&next).unwrap().source;
        assert!(source.contains("wrapped(\"accept\", \"confirm\")"));assert!(source.contains("event_handler"));assert!(source.contains("handler confirm"));
    }
}
