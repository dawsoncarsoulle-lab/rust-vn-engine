//! Shared genuine Graph -> strict identity reconstruction -> frozen source helper.
//! No manual scheduled wrapper, source grant, cached evaluator or owner fabrication.
use std::path::PathBuf;
use rvn_graph::*;
use rvn_parser::ParsedProgram;

pub fn finish_program(dir:PathBuf,original:ParsedProgram,mut graphs:Vec<GraphDocument>,expected_kinds:&[NodeKind])->ParsedProgram {

    for graph in &mut graphs {
        // The importer supplies an empty Init graph even when no Init was
        // authored. It has no consumer carrying a native graph identity.
        // Keep that historical empty graph without inventing an owned wrapper.
        if matches!(graph.kind,GraphKind::Init) && graph.nodes.len()==1 && graph.edges.is_empty() {
            assert!(!original.script.iter().any(|s|matches!(s,rvn_parser::Statement::Init{body}if !body.is_empty())));
            continue;
        }
        graph.enable_native_frame_preparation().unwrap();
        std::fs::write(dir.join(format!("graph-{}-before-strict.json",graph.graph_id)),graph.to_pretty_json().unwrap()).unwrap();
        let strict=prepare_native_frame_graph(graph).unwrap_or_else(|error|panic!("strict graph preparation {:?} {}: {error:?}",graph.kind,graph.graph_id));
        let restored=import_native_frame_script(&strict.ast).unwrap();
        let restored=restored.iter().find(|actual|actual.graph_id==graph.graph_id).unwrap();
        let emitted_again=prepare_native_frame_graph(restored).unwrap();
        assert_eq!(emitted_again.ast,strict.ast,"Command AST must survive strict reimport, including animation parameters and effect application flags");
        assert_eq!(restored.edges,graph.edges);
        assert_eq!(restored.nodes.keys().collect::<Vec<_>>(),graph.nodes.keys().collect::<Vec<_>>());
        assert_eq!(restored.pins.keys().collect::<Vec<_>>(),graph.pins.keys().collect::<Vec<_>>());
        for (id,pin) in &graph.pins {
            let actual=&restored.pins[id];assert_eq!((&actual.key,actual.direction),(&pin.key,pin.direction));
            assert_eq!(restored.effective_pin_type(*id),graph.effective_pin_type(*id),"Effective type at actual pin {id}");
        }
        for (id,node) in &graph.nodes {assert_eq!(restored.nodes[id].kind,node.kind);}

        std::fs::write(dir.join(format!("graph-{}.json",graph.graph_id)),graph.to_pretty_json().unwrap()).unwrap();

    }
    // The real exporter owns multi-document transport and Scene/Game boundaries.
    // Never hand-author a footer or concatenate unrelated source scopes.
    let project=transpile_project(&graphs).unwrap();let source=project.source;let ast=project.ast;
    let file=dir.join("main.rvn"); std::fs::write(&file,&source).unwrap();
    let parsed=rvn_parser::parse_file_program_with_uses(&file).unwrap();
    assert_eq!(parsed.script,ast);
    let origins=NativeDebugSourceMap::from_program(&parsed).unwrap();
    let commands:Vec<_>=origins.origins().iter().filter(|o|expected_kinds.contains(&o.kind)).collect();
    for kind in expected_kinds {assert!(commands.iter().any(|origin|origin.kind==*kind),"No actual source origin for {kind:?}");}
    for command in commands {
        assert!(command.span.start<command.span.end);
        assert_eq!(command.source,std::fs::canonicalize(&file).unwrap());
        assert!(graphs.iter().any(|g|g.graph_id==command.graph&&g.nodes.contains_key(&command.node)));
    }
    assert_eq!(new_command_semantics(&parsed.script),new_command_semantics(&original.script),"Meaningful newly imported command fields differ from their actual authored AST");
    parsed
}

// Compare only the newly reconstructed commands. Parameter order is not semantic;
// identity preludes and scheduling wrappers are deliberately not compared here.
fn new_command_semantics(script:&[rvn_parser::Statement])->Vec<rvn_parser::Statement>{
    use rvn_parser::Statement as S;
    let mut result=Vec::new();
    for statement in script{
        match statement{
            S::Init{body}|S::ScheduledConsumer{body,..}=>result.extend(new_command_semantics(body)),
            S::SpriteAnimate{character_id,animation,params}=>{
                let mut params=params.clone();params.sort_by(|a,b|a.name.cmp(&b.name));
                result.push(S::SpriteAnimate{character_id:character_id.clone(),animation:animation.clone(),params});
            },
            S::SpriteStopAnimation{..}|S::SpriteEffect{..}=>result.push(statement.clone()),
            _=>{},
        }
    }
    result
}
