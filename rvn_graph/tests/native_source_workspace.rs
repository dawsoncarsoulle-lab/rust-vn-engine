use rvn_graph::*;
use std::{path::{Path,PathBuf},fs};
fn folder(name:&str)->PathBuf {
    let root=std::env::var("RVN_NATIVE_SOURCE_TEST_DIR").map(PathBuf::from).unwrap_or_else(|_|std::env::temp_dir().join("rvn-native-source-0292"));
    fs::create_dir_all(&root).unwrap();
    let dir=root.join(format!("{name}-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir(&dir).unwrap();dir
}
fn source()->String {
    include_str!("fixtures/native_source_single.rvn").to_owned()
}
fn link(root:&Path)->SourceWorkspace {
    fs::write(root.join("main.rvn"),source()).unwrap();
    SourceWorkspace::link_native_preparation(root,&root.join("main.rvn"),&[]).unwrap()
}
#[test]
fn actual_writer_roundtrip_then_cold_open_preserve_authoritative_ids() {
    let root=folder("writer-cold");let mut workspace=link(&root);
    let before=workspace.freeze_native_program().unwrap();
    let sites=native_source_sites(&before).unwrap();
    assert_eq!(sites.len(),1);assert_eq!(sites[0].graph,GraphId::new(900));assert_eq!(sites[0].consumer,NodeId::new(5));
    let mut edited=workspace.project().graphs().to_vec();
    let graph=&mut edited[0];let node=graph.nodes.values().find(|node|node.kind==NodeKind::TextFromStringTable).unwrap().id;
    graph.set_pin_default(node,"key",PropertyValue::String("writer changed 世界\nsecond".into())).unwrap();
    let ids=graph.nodes.keys().copied().collect::<Vec<_>>();let edges=graph.edges.clone();
    workspace.save(&edited).unwrap();
    assert!(fs::read_to_string(root.join("main.rvn")).unwrap().contains("writer changed"));
    let after=workspace.freeze_native_program().unwrap();assert_ne!(before.script,after.script);
    let cold=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
    assert_eq!(cold.freeze_native_program().unwrap(),after);
    assert_eq!(cold.project().graphs()[0].nodes.keys().copied().collect::<Vec<_>>(),ids);
    assert_eq!(cold.project().graphs()[0].edges,edges);
    let automatic=SourceWorkspace::open(&root).unwrap().unwrap();
    assert_eq!(automatic.freeze_native_program().unwrap(),after);
    assert_eq!(automatic.project().graphs()[0].nodes.keys().copied().collect::<Vec<_>>(),ids);
    assert_eq!(automatic.project().graphs()[0].edges,edges);
    mixed_legacy_neighbors_keep_real_authoring_history_on_native_cold_open();
}
#[test]
fn actual_source_inclusion_order_and_spans_are_frozen_without_semantic_sidecar() {
    let root=folder("inclusion");
    fs::write(root.join("second.rvn"),"function second(){return 2}\n").unwrap();
    fs::write(root.join("first.rvn"),"use \"second.rvn\"\nfunction first(){return second()}\n").unwrap();
    fs::write(root.join("main.rvn"),format!("use \"first.rvn\"\n{}",source())).unwrap();
    let workspace=SourceWorkspace::link_native_preparation(&root,&root.join("main.rvn"),&[]).unwrap();
    let program=workspace.freeze_native_program().unwrap();
    assert_eq!(program.sources.iter().map(|source|source.path.as_ref().unwrap().file_name().unwrap().to_str().unwrap()).collect::<Vec<_>>(),["main.rvn","first.rvn","second.rvn"]);
    let sites=native_source_sites(&program).unwrap();assert_eq!(sites[0].route,[2,0]);
    let raw=&program.sources[0].source;
    assert!(raw.get(sites[0].span.range.clone()).unwrap().starts_with("flow.consume_v1"));
    fs::write(root.join("first.rvn"),"function first(){return 99}\n").unwrap();
    assert!(workspace.freeze_native_program().is_err());
}
#[test]
fn source_modified_and_graph_cache_payload_modified_do_not_publish_hidden_semantics() {
    let root=folder("source-cache");let workspace=link(&root);
    let before=workspace.freeze_native_program().unwrap();
    let sidecar=root.join(".rvn-authoring.json");let mut cache:serde_json::Value=serde_json::from_str(&fs::read_to_string(&sidecar).unwrap()).unwrap();
    cache["graphs"][0]["pins"]["3"]["default_value"]["value"]=serde_json::json!("hidden cache payload");
    fs::write(&sidecar,serde_json::to_string_pretty(&cache).unwrap()).unwrap();
    let cold=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
    assert_eq!(cold.freeze_native_program().unwrap().script,before.script);
    fs::write(root.join("main.rvn"),source().replace("Hello\\n世界","actual source changed")).unwrap();
    assert!(workspace.freeze_native_program().is_err());
    let refreshed=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
    assert_ne!(refreshed.freeze_native_program().unwrap().script,before.script);
}
#[test]
fn source_collisions_and_forged_origin_spans_are_refused() {
    let root=folder("collisions");let workspace=link(&root);
    let mut program=workspace.freeze_native_program().unwrap();
    program.origins[0].span.range.start+=1;assert!(native_source_sites(&program).is_err());
    fs::write(root.join("main.rvn"),format!("{}\n{}",source(),source().replace("function probe()","function other()"))).unwrap();
    assert!(SourceWorkspace::link_native_preparation(&folder("other-collision"),&root.join("main.rvn"),&[]).is_err());
    let cold=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
    assert!(cold.source_error.is_some());assert!(cold.freeze_native_program().is_err());
}

#[test]
fn cold_native_workspace_discovers_real_explicit_scene_context_and_preserves_locals() {
    let root=folder("scene-context");
    fs::write(root.join("main.rvn"),format!("{}\nscene label start\nlocal marker=1\n\"[marker]\"\n",source())).unwrap();
    let workspace=SourceWorkspace::link_native_preparation(&root,&root.join("main.rvn"),&[]).unwrap();
    let frozen=workspace.freeze_native_program().unwrap();assert_eq!(frozen.context,rvn_parser::NarrativeContext::ExplicitScenes);
    let cold=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
    assert_eq!(cold.freeze_native_program().unwrap(),frozen);
}

// Real authoring transaction supplies Unit identities once; frozen projection
// receives only the actual saved Program, never these authoring graph owners.
fn frozen_unit1021_workspace(name:&str)->(PathBuf,SourceWorkspace){
    let root=fs::canonicalize(folder(name)).unwrap();
    let path=root.join("main.rvn");
    let raw=r#"init {character.create("hero","Héroïne世界") set score=0}
function scoped(alpha,beta) -> () {return}
handler clicked(event) {set score=score+1}
scene label start
call scoped(10,11)
return
"#;
    fs::write(&path,raw).unwrap();
    let mut allocated=0u64;
    let prepared=prepare_initial_unit_link_graphs(&rvn_parser::parse(raw).unwrap(),&[],||{
        allocated+=1;Ok(format!("{:032x}",0x4000+allocated))
    }).unwrap();
    let mut workspace=SourceWorkspace::link(&root,&path,&prepared).unwrap();
    let mut edited=workspace.project().graphs().to_vec();
    edited.iter_mut().find(|g|matches!(g.kind,GraphKind::Handler{..})).unwrap()
        .enable_native_frame_preparation().unwrap();
    workspace.save(&edited).unwrap();drop(workspace);
    let cold=SourceWorkspace::open(&root).unwrap().unwrap();
    assert!(cold.source_error.is_none(),"{:?}",cold.source_error);
    let unit=cold.project().graphs().iter().find(|g|matches!(g.kind,GraphKind::UnitFunction{..})).unwrap();
    let before=prepared.iter().find(|g|g.graph_id==unit.graph_id).unwrap();
    let identities=|g:&GraphDocument|g.pins.iter().filter_map(|(id,p)|
        p.parameter_id.as_ref().map(|parameter|(*id,parameter.clone()))).collect::<Vec<_>>();
    assert_eq!(identities(unit),identities(before));assert_eq!(identities(unit).len(),2);
    assert!(unit.inspector_evaluation.is_legacy());assert_eq!(unit.schema_version,GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION);
    assert_eq!(allocated,2);
    (root,cold)
}

#[test]
fn frozen_native1021_parameterized_unit_context_and_character_registry_use_the_actual_saved_source(){
    let(root,cold)=frozen_unit1021_workspace("frozen-unit-context");
    let path=root.join("main.rvn");let metadata=root.join(".rvn-authoring.json");
    let bytes=fs::read(&path).unwrap();let sidecar=fs::read(&metadata).unwrap();
    let graphs=cold.project().graphs().to_vec();
    let program=cold.freeze_native_program().unwrap();
    assert_eq!(program.context,rvn_parser::NarrativeContext::ExplicitScenes);
    assert!(program.script.iter().any(|s|matches!(s,rvn_parser::Statement::UnitFunction{parameters,..}if parameters.len()==2)));
    assert!(program.script.iter().any(|s|matches!(s,rvn_parser::Statement::Init{body}if body.iter().any(|s|matches!(s,rvn_parser::Statement::CharacterCreate{id,display_name}if id=="hero"&&display_name=="Héroïne世界")))));
    let handler=graphs.iter().find(|g|matches!(g.kind,GraphKind::Handler{..})).unwrap();
    assert_eq!(handler.characters.get("hero").map(String::as_str),Some("Héroïne世界"));
    assert_eq!(handler.variables["score"].value_type,ValueType::Int);
    let sites=native_source_sites(&program).unwrap();assert!(!sites.is_empty());
    assert!(sites.iter().all(|site|site.graph==handler.graph_id));
    let map=NativeDebugSourceMap::from_program(&program).unwrap();assert!(!map.origins().is_empty());
    assert!(map.origins().iter().all(|origin|origin.source==path&&origin.graph==handler.graph_id));
    assert_eq!(fs::read(&path).unwrap(),bytes);assert_eq!(fs::read(&metadata).unwrap(),sidecar);
    assert_eq!(cold.project().graphs(),graphs.as_slice());
    // A source-only successful projection does not grant authoring Unit owners.
    assert!(import_native_frame_source(std::str::from_utf8(&bytes).unwrap(),program.context)
        .unwrap_err().contains("allocator"));
    // Independently exercise the library's genuine resolved Use closure. The
    // authoring workspace is retired before moving a declaration into a file;
    // no sidecar model donates identities to this ParsedProgram projection.
    drop(cold);
    let index=program.script.iter().position(|s|matches!(s,rvn_parser::Statement::UnitFunction{..})).unwrap();
    let span=program.origins[index].span.range.clone();
    let saved=String::from_utf8(bytes).unwrap();let unit=saved[span.clone()].to_owned();
    let mut main=saved;main.replace_range(span,"");
    // Native A plus an unused Unit in B was already admitted by the complete
    // import: retain that case, without admitting new mixed projections.
    main=main.replace("call scoped(10,11)","");
    main.insert_str(0,"use \"unit.rvn\"\n");
    let unit_path=root.join("unit.rvn");fs::write(&unit_path,&unit).unwrap();fs::write(&path,&main).unwrap();
    let imported=rvn_parser::parse_file_program_with_uses(&path).unwrap();assert_eq!(imported.sources.len(),2);
    assert!(!native_source_sites(&imported).unwrap().is_empty());
    assert!(!NativeDebugSourceMap::from_program(&imported).unwrap().origins().is_empty());
    // Exact physical Use/span is still not a resolved executable root. Reject
    // its reinsertion without pretending that physical source loses its Use.
    let physical=rvn_parser::parse_with_origins(&main,imported.context).unwrap();
    assert!(matches!(physical.script[0],rvn_parser::Statement::Use{..}));
    let mut forged=imported.clone();
    forged.script.insert(0,physical.script[0].clone());
    forged.origins.insert(0,rvn_parser::StatementOrigin{source:Some(path.clone()),span:physical.spans[0].clone()});
    assert!(native_source_sites(&forged).unwrap_err().contains("fully resolved Program"));
    let nested=unit.replacen("{return}",r#"{use "other.rvn" return}"#,1);assert_ne!(nested,unit);
    fs::write(&unit_path,&nested).unwrap();
    assert!(rvn_parser::parse_file_program_with_uses(&path).is_err());
    assert!(rvn_parser::parse_with_origins(&nested,imported.context).is_err());
    // A forged resolved AST can still carry a residual Use. Reuse the real
    // physical main Use, retain the genuine origins, and demand its dedicated
    // recursive refusal before source/AST association could fail instead.
    let mut forged=imported.clone();
    let rvn_parser::Statement::UnitFunction{body,..}=forged.script.iter_mut().find(|s|matches!(s,rvn_parser::Statement::UnitFunction{..})).unwrap() else{unreachable!()};
    body.insert(0,physical.script[0].clone());
    assert!(native_source_sites(&forged).unwrap_err().contains("fully resolved Program"));
    fs::write(&unit_path,&unit).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(),main);
    assert_eq!(fs::read_to_string(&unit_path).unwrap(),unit);
}

#[test]
fn frozen_native1021_full_source_semantics_collisions_forgeries_and_unit_capabilities_stay_strict(){
    let(root,cold)=frozen_unit1021_workspace("frozen-unit-forgeries");
    let path=root.join("main.rvn");let saved=fs::read_to_string(&path).unwrap();
    let program=cold.project().resolved_program().unwrap();
    let handler=program.script.iter().zip(&program.origins).find(|(statement,_)|
        matches!(statement,rvn_parser::Statement::Handler{name,..}if name=="clicked")).unwrap();
    let fragment=&saved[handler.1.span.range.clone()];
    assert!(fragment.starts_with("handler clicked(event)"));
    let second=fragment.replacen("handler clicked(event)","handler other(event)",1);
    let unit_native=fragment.replacen("handler clicked(event)","function foreign(alpha) -> ()",1);
    let body_span=handler.1.span.children[0].children[0].range.clone();
    assert!(saved[body_span.clone()].trim_start().starts_with("set score"));
    let mut unit_call=saved.clone();unit_call.replace_range(body_span,"call scoped(10,11)");
    // Consumer bodies are computational at parse time. The genuine UnitCall
    // spelling must be refused there; do not claim it reached Native import.
    let error=rvn_parser::parse_with_origins(&unit_call,program.context).unwrap_err();
    assert!(error.to_string().contains("fonction de calcul"),"{error}");
    for (bad,expected) in [
        (format!("function scoped(extra) -> () {{return}}
{saved}"),""),
        (saved.replace("call scoped(10,11)","call scoped(10)"),""),
        (format!("{second}
{saved}"),"GraphId collision"),
        (saved.replace(fragment,&unit_native),"schema14 Unit"),
        (saved.replacen("init {",r#"init {character.create("hero","Duplicate") "#,1),"deux fois"),
        (saved.replacen("init {","init {
// @rvn-native-empty-init:v99 {}
",1),"declaration version"),
    ]{
        fs::write(&path,&bad).unwrap();
        let parsed=rvn_parser::parse_file_program_with_uses(&path).unwrap();
        let error=native_source_sites(&parsed).unwrap_err();
        assert!(expected.is_empty()||error.contains(expected),"{error}; expected {expected}");
        assert_eq!(fs::read_to_string(&path).unwrap(),bad);
    }
    fs::write(&path,&saved).unwrap();
    // Actual Graph allocation and Native lowering supply both macro identities.
    // Bare data in Unit is refused; identical spelling in literals/comments is inert.
    for (kind, method) in [(NodeKind::DoN,"flow.macro_counter_v1"),(NodeKind::FlipFlop,"flow.macro_is_a_v1")] {
        let script=rvn_parser::parse("handler unit_macro_probe(event){}").unwrap();
        let mut graph=import_script(&script).unwrap().into_iter().find(|g|matches!(&g.kind,GraphKind::Handler{name}if name=="unit_macro_probe")).unwrap();
        let root=graph.nodes.values().find(|n|n.kind==NodeKind::HandlerEntry).unwrap().id;
        let node=graph.add_catalog_node(kind,[300.,0.]).unwrap();
        graph.connect(graph.pin_by_key(root,"exec_out").unwrap().id,graph.pin_by_key(node,"exec_in").unwrap().id).unwrap();
        graph.enable_native_frame_preparation().unwrap();
        let generated=rvn_graph::prepare_native_frame_graph(&graph).unwrap();
        let script=rvn_parser::parse(&generated.source).unwrap();
        let body=script.iter().find_map(|s|match s{rvn_parser::Statement::Handler{name,body,..}if name=="unit_macro_probe"=>Some(body),_=>None}).unwrap();
        let [rvn_parser::Statement::StandardMacroDispatch{dispatch}]=body.as_slice() else{panic!("real macro dispatch missing")};
        dispatch.validate().unwrap();
        let data=format!("{method}({}, {})",serde_json::to_string(&dispatch.site.scope_source()).unwrap(),serde_json::to_string(&dispatch.site.node_source()).unwrap());
        for spelling in [data.clone(),data.replacen('.',"\n.\n",1),data.replacen('(',"\n(",1)] {
            let bad=saved.replacen("{return}",&format!("{{set value={spelling} return}}"),1);
            assert_ne!(bad,saved);
            fs::write(&path,&bad).unwrap();
            let parsed=rvn_parser::parse_file_program_with_uses(&path).unwrap();
            let unit=parsed.script.iter().find_map(|s|match s{rvn_parser::Statement::UnitFunction{name,body,..}if name=="scoped"=>Some(body),_=>None}).unwrap();
            assert!(matches!(&unit[0],rvn_parser::Statement::SetVar{value:rvn_parser::Expr::StandardMacroData{site,..},..}if site.as_ref()==&dispatch.site),"the real Parser must produce the same macro data site for newline spelling");
            assert!(native_source_sites(&parsed).unwrap_err().contains("Native macro data in a schema14 Unit"));
            let comment=spelling.replace('\n',"\n// ");
            let inert=saved.replacen("{return}",&format!("{{set value={} // {comment}\nreturn}}",serde_json::to_string(&spelling).unwrap()),1);
            fs::write(&path,&inert).unwrap();
            let parsed=rvn_parser::parse_file_program_with_uses(&path).unwrap();
            native_source_sites(&parsed).unwrap();
        }
    }
    fs::write(&path,&saved).unwrap();
    let mut forged=program.clone();forged.origins[0].span.range.start+=1;
    assert!(native_source_sites(&forged).unwrap_err().contains("AST/span"));
    let mut forged=program.clone();forged.sources[0].source=forged.sources[0].source.replace("Héroïne世界","Different");
    assert!(NativeDebugSourceMap::from_program(&forged).is_err());
    let unused=saved.replacen("function scoped", "function unused(extra) -> () {return}\nfunction scoped",1);
    fs::write(&path,&unused).unwrap();
    let actual=rvn_parser::parse_file_program_with_uses(&path).unwrap();
    let index=actual.script.iter().position(|s|matches!(s,rvn_parser::Statement::UnitFunction{name,..}if name=="unused")).unwrap();
    let mut forged=actual.clone();forged.script.remove(index);forged.origins.remove(index);
    assert!(native_source_sites(&forged).unwrap_err().contains("omits a physical source root"));
    let mut forged=actual.clone();forged.script.push(actual.script[index].clone());forged.origins.push(actual.origins[index].clone());
    assert!(native_source_sites(&forged).unwrap_err().contains("repeats a physical source root"));
    let mut forged=actual.clone();forged.script.swap(0,1);forged.origins.swap(0,1);
    assert!(native_source_sites(&forged).unwrap_err().contains("lexical root order"));
    let index=actual.script.iter().position(|s|matches!(s,rvn_parser::Statement::Init{..})).unwrap();
    let mut forged=actual.clone();forged.script.remove(index);forged.origins.remove(index);
    assert!(native_source_sites(&forged).unwrap_err().contains("omits a physical source root"));
    fs::write(&path,&saved).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(),saved);
}

#[test]
fn frozen_native1021_mixed_adapted_transport_keeps_the_complete_project_proof_refusal(){
    let(root,cold)=frozen_unit1021_workspace("frozen-unit-transport");
    let saved=fs::read_to_string(root.join("main.rvn")).unwrap();
    let helper=root.join("helper.rvn");fs::write(&helper,"function helper(){return 1}\n").unwrap();
    let path=root.join("mixed.rvn");
    fs::write(&path,format!("use \"helper.rvn\"\n{saved}")).unwrap();
    let program=rvn_parser::parse_file_program_with_uses(&path).unwrap();
    let bytes=fs::read(&path).unwrap();
    // Explicit Use now has a complete frozen order proof; the later adapted
    // export below must still retain its independent full-project refusal.
    assert!(!native_source_sites(&program).unwrap().is_empty());
    assert_eq!(fs::read(&path).unwrap(),bytes);
    let mut graphs=cold.project().graphs().to_vec();
    // A genuine project export with a Native narrative label needs the
    // existing complete-project footer proof, not a subset re-export.
    let raw="init {set score=0}
label tail
set score=1
";
    let mut tail=import_script(&rvn_parser::parse(raw).unwrap()).unwrap().into_iter()
        .find(|g|matches!(g.kind,GraphKind::Label{..})).unwrap();
    let max=graphs.iter().map(|g|g.graph_id.get()).max().unwrap();
    tail.graph_id=GraphId::new(max+1);tail.enable_native_frame_preparation().unwrap();graphs.push(tail);
    let generated=transpile_project(&graphs).unwrap();
    assert!(generated.source.contains("__blueprint_project_end"));
    let path=root.join("transport.rvn");fs::write(&path,&generated.source).unwrap();
    let program=rvn_parser::parse_file_program_with_uses(&path).unwrap();
    let bytes=fs::read(&path).unwrap();
    let error=native_source_sites(&program).unwrap_err();
    assert!(error.contains("complete project proof"),"{error}");
    assert_eq!(fs::read(&path).unwrap(),bytes);
}

// Retain the actual linked Unit owners in main: moving an unused Unit out of
// main would exercise only the older complete-import route, not this boundary.
fn frozen_explicit_use1021_workspace(name:&str)->(PathBuf,SourceWorkspace) {
    let (root,old)=frozen_unit1021_workspace(name);
    let saved=fs::read_to_string(root.join("main.rvn")).unwrap();drop(old);
    fs::create_dir_all(root.join("nested/deep")).unwrap();
    fs::write(root.join("nested/first.rvn"),r#"init {character.create("guide","Guide世界") set order=1}
use "../sibling.rvn"
init {set order=2}
function imported(alpha) -> () {return}
use "deep/leaf.rvn"
"#).unwrap();
    fs::write(root.join("sibling.rvn"),r#"init {set order=3}
use "nested/deep/leaf.rvn"
"#).unwrap();
    fs::write(root.join("nested/deep/leaf.rvn"),"init {set order=4}\nfunction leaf(){return 4}\n").unwrap();
    fs::write(root.join("main.rvn"),format!("use \"nested/first.rvn\"\nuse {{\"sibling.rvn\",\"nested/first.rvn\"}}\n{saved}")).unwrap();
    let workspace=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
    assert!(workspace.source_error.is_none(),"actual multi-source reopen: {:?}",workspace.source_error);
    (root,workspace)
}

#[test]
fn frozen_native1021_explicit_parent_sibling_nested_use_preserves_actual_order_unit_ids_and_cold_source() {
    let(root,mut workspace)=frozen_explicit_use1021_workspace("explicit-use-cold");
    let graphs=workspace.project().graphs().to_vec();
    let unit=graphs.iter().find(|g|matches!(&g.kind,GraphKind::UnitFunction{name}if name=="scoped")).unwrap();
    let parameters=unit.pins.values().filter_map(|p|p.parameter_id.as_ref().map(|id|(p.id,id.clone()))).collect::<Vec<_>>();
    assert_eq!(parameters.len(),2,"the original real link transaction still owns both parameters");
    assert!(!graphs.iter().any(|g|matches!(&g.kind,GraphKind::UnitFunction{name}if name=="imported")),"foreign Unit is context, not an editable ownership donation");
    workspace.save(&graphs).unwrap();drop(workspace);
    let cold=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
    assert!(cold.source_error.is_none(),"{:?}",cold.source_error);
    let metadata=fs::read(root.join(".rvn-authoring.json")).unwrap();
    let program=cold.freeze_native_program().unwrap();
    let expected=["main.rvn","nested/first.rvn","sibling.rvn","nested/deep/leaf.rvn"].map(|p|fs::canonicalize(root.join(p)).unwrap());
    assert_eq!(program.sources.iter().map(|s|s.path.as_ref().unwrap()).collect::<Vec<_>>(),expected.iter().collect::<Vec<_>>());
    let order=program.script.iter().filter_map(|s|match s {
        rvn_parser::Statement::Init{body}=>body.iter().find_map(|s|match s {
            rvn_parser::Statement::SetVar{name,value:rvn_parser::Expr::Int(value)}if name=="order"=>Some(*value),_=>None,
        }),_=>None,
    }).collect::<Vec<_>>();
    assert_eq!(order,[1,3,4,2],"true depth-first inclusion and dedup determine the final initialization order");
    let handler=cold.project().graphs().iter().find(|g|matches!(&g.kind,GraphKind::Handler{name}if name=="clicked")).unwrap();
    let definition=program.script.iter().position(|s|matches!(s,rvn_parser::Statement::Handler{name,..}if name=="clicked")).unwrap();
    let sites=native_source_sites(&program).unwrap();assert!(!sites.is_empty());
    assert!(sites.iter().all(|site|site.graph==handler.graph_id&&site.source==expected[0]&&site.definition_route==vec![definition]&&site.route[0]==definition));
    assert!(!NativeDebugSourceMap::from_program(&program).unwrap().origins().is_empty());
    let restored=cold.project().graphs().iter().find(|g|g.graph_id==unit.graph_id).unwrap();
    assert_eq!(restored.pins.values().filter_map(|p|p.parameter_id.as_ref().map(|id|(p.id,id.clone()))).collect::<Vec<_>>(),parameters);
    assert!(restored.inspector_evaluation.is_legacy());assert_eq!(restored.schema_version,GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION);
    for source in &program.sources {assert_eq!(fs::read_to_string(source.path.as_ref().unwrap()).unwrap(),source.source);}
    assert_eq!(fs::read(root.join(".rvn-authoring.json")).unwrap(),metadata);
    let path=root.join("nested/first.rvn");let original=fs::read_to_string(&path).unwrap();
    fs::write(&path,format!("{original}\n// actual imported source changed\n")).unwrap();
    assert!(cold.freeze_native_program().is_err());
    fs::write(&path,&original).unwrap();
    assert_eq!(fs::read(root.join(".rvn-authoring.json")).unwrap(),metadata);
    readonly_foreign_unit_global_dependencies_keep_ownership_and_publication_atomic();
}

#[test]
fn frozen_native1021_explicit_use_refuses_cross_file_order_source_order_and_cycle_forgeries() {
    let(root,workspace)=frozen_explicit_use1021_workspace("explicit-use-forgeries");
    let mut program=workspace.freeze_native_program().unwrap();
    // Keep exercising r3's exact explicit fallback independently of the new proof.
    program.resolution=None;
    let find_init=|value|program.script.iter().position(|s|matches!(s,rvn_parser::Statement::Init{body}
        if body.iter().any(|s|matches!(s,rvn_parser::Statement::SetVar{name,value:rvn_parser::Expr::Int(actual)}if name=="order"&&*actual==value)))).unwrap();
    let(first_last,sibling)=(find_init(2),find_init(3));
    assert_ne!(program.origins[first_last].source,program.origins[sibling].source);
    let mut forged=program.clone();forged.script.swap(first_last,sibling);forged.origins.swap(first_last,sibling);
    assert!(native_source_sites(&forged).unwrap_err().contains("ordered AST/origin stream"),"all physical roots and each file's lexical order survive this inter-file forgery");
    let mut forged=program.clone();forged.sources.swap(1,2);
    assert!(native_source_sites(&forged).unwrap_err().contains("first-inclusion"));
    let mut forged=program.clone();forged.sources.pop();
    assert!(native_source_sites(&forged).is_err());
    // A real path and a parsed Use can form a cycle in otherwise unchanged
    // frozen roots. The actual loader's cycle-before-dedup rule is essential.
    let mut forged=program.clone();
    let first=fs::canonicalize(root.join("nested/first.rvn")).unwrap();
    let file=forged.sources.iter_mut().find(|s|s.path.as_ref()==Some(&first)).unwrap();
    file.source.push_str("use \"../main.rvn\"\n");
    assert!(native_source_sites(&forged).unwrap_err().contains("cycle"));
    let empty=root.join("unused.rvn");fs::write(&empty,"").unwrap();
    let mut forged=program.clone();forged.sources.push(rvn_parser::ProgramSource{path:Some(fs::canonicalize(&empty).unwrap()),source:String::new()});
    assert!(native_source_sites(&forged).unwrap_err().contains("first-inclusion"));
    assert_eq!(workspace.freeze_native_program().unwrap(),program,"negative ParsedProgram copies never publish their forged bytes");
}


#[test]
fn frozen_native1021_captured_wildcard_alias_save_cold_and_uncertified_subset_are_exact() {
    let(root,old)=frozen_unit1021_workspace("captured-wildcard-alias");
    let saved=fs::read_to_string(root.join("main.rvn")).unwrap();drop(old);
    fs::create_dir(root.join("parts")).unwrap();
    let a="init {character.create(\"guide\",\"Guide世界\") set import_order=1}\n";
    let b="init {set import_order=2}\nfunction foreign(arg) -> () {return}\n";
    fs::write(root.join("parts/a.rvn"),a).unwrap();fs::write(root.join("parts/b.rvn"),b).unwrap();
    fs::write(root.join("main.rvn"),format!("use {{\"parts/*.rvn\",\"parts/../parts/b.rvn\"}}\n{saved}")).unwrap();
    let mut live=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
    assert!(live.source_error.is_none(),"{:?}",live.source_error);
    let graphs=live.project().graphs().to_vec();
    let unit=graphs.iter().find(|g|matches!(&g.kind,GraphKind::UnitFunction{name}if name=="scoped")).unwrap();
    let ids=unit.pins.values().filter_map(|p|p.parameter_id.as_ref().map(|id|(p.id,id.clone()))).collect::<Vec<_>>();assert_eq!(ids.len(),2);
    assert!(!graphs.iter().any(|g|matches!(&g.kind,GraphKind::UnitFunction{name}if name=="foreign")));
    let source_bytes=fs::read(root.join("main.rvn")).unwrap();
    assert!(graphs.iter().all(|graph|graph.characters.get("guide").map(String::as_str)==Some("Guide世界")),
        "the resolved registry reaches every genuine local owner before Save");
    live.save(&graphs).unwrap();
    assert_eq!(live.project().graphs(),graphs.as_slice());
    assert_eq!(fs::read(root.join("main.rvn")).unwrap(),source_bytes);
    assert_eq!(fs::read_to_string(root.join("parts/a.rvn")).unwrap(),a);
    assert_eq!(fs::read_to_string(root.join("parts/b.rvn")).unwrap(),b);
    drop(live);
    let cold=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();assert!(cold.source_error.is_none());
    let frozen=cold.freeze_native_program().unwrap();assert!(frozen.resolution.is_some());assert_eq!(frozen.sources.len(),3);
    let expected=["main.rvn","parts/a.rvn","parts/b.rvn"].map(|p|fs::canonicalize(root.join(p)).unwrap());
    assert_eq!(frozen.sources.iter().map(|s|s.path.as_ref().unwrap()).collect::<Vec<_>>(),expected.iter().collect::<Vec<_>>());
    let handler=cold.project().graphs().iter().find(|g|matches!(&g.kind,GraphKind::Handler{name}if name=="clicked")).unwrap();
    assert_eq!(handler.characters.get("guide").map(String::as_str),Some("Guide世界"));
    assert_eq!(cold.project().graphs(),graphs.as_slice(),"cold reopen keeps all real owners and imported character registries");
    assert_eq!(fs::read(root.join("main.rvn")).unwrap(),source_bytes);
    assert_eq!(fs::read_to_string(root.join("parts/a.rvn")).unwrap(),a);
    assert_eq!(fs::read_to_string(root.join("parts/b.rvn")).unwrap(),b);
    frozen.resolution.as_ref().unwrap().verify_for(&frozen).unwrap();
    assert!(!native_source_sites(&frozen).unwrap().is_empty());
    assert!(!NativeDebugSourceMap::from_program(&frozen).unwrap().origins().is_empty());
    let unit=cold.project().graphs().iter().find(|g|matches!(&g.kind,GraphKind::UnitFunction{name}if name=="scoped")).unwrap();
    assert_eq!(unit.pins.values().filter_map(|p|p.parameter_id.as_ref().map(|id|(p.id,id.clone()))).collect::<Vec<_>>(),ids);
    let mut none=frozen.clone();none.resolution=None;
    assert!(native_source_sites(&none).unwrap_err().contains("resolution certificate"));
    // The optional witness changes neither historical SourceMap identity nor its
    // portable v1 bytes/ordinals witness. Neither DTO grants reconstruction.
    let captured_map=rvn_core::debugger_provenance::SourceMap::from_program(&frozen).unwrap();
    let old_map=rvn_core::debugger_provenance::SourceMap::from_program(&none).unwrap();
    assert_eq!(captured_map.closure,old_map.closure);assert_eq!(captured_map.content,old_map.content);assert_eq!(captured_map.emitted,old_map.emitted);
    let portable=rvn_core::save::consumer_checkpoint::PortableConsumerProgram::from_program(&frozen).unwrap();
    portable.validate_current(&none).unwrap();
    let encoded=serde_json::to_value(&portable).unwrap();assert_eq!(encoded["version"],1);assert!(encoded.get("resolution").is_none());
    let decoded:rvn_core::save::consumer_checkpoint::PortableConsumerProgram=serde_json::from_value(encoded.clone()).unwrap();
    decoded.validate_current(&frozen).unwrap();assert_eq!(decoded,portable);
    let mut forged_dto=encoded;forged_dto["resolution"]=serde_json::json!({"claimed":"wildcard"});
    assert!(serde_json::from_value::<rvn_core::save::consumer_checkpoint::PortableConsumerProgram>(forged_dto).is_err());
    let metadata=fs::read(root.join(".rvn-authoring.json")).unwrap();
    let dto:serde_json::Value=serde_json::from_slice(&metadata).unwrap();assert_eq!(dto["version"],1);assert!(dto.get("resolution").is_none());
    fs::remove_file(root.join("parts/a.rvn")).unwrap();fs::remove_file(root.join("parts/b.rvn")).unwrap();
    frozen.resolution.as_ref().unwrap().verify_for(&frozen).unwrap();
    assert!(!native_source_sites(&frozen).unwrap().is_empty()); // Frozen replay does not reopen imports.
    assert!(cold.freeze_native_program().is_err());
    fs::write(root.join("parts/a.rvn"),a).unwrap();fs::write(root.join("parts/b.rvn"),b).unwrap();
    // Complete registries must still refuse a genuine foreign duplicate;
    // aliases of one physical source remain deduplicated by the real loader.
    fs::write(root.join("parts/a.rvn"),format!("{a}init {{character.create(\"guide\",\"Duplicate\")}}\n")).unwrap();
    let duplicate=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
    assert!(duplicate.source_error.as_deref().unwrap().contains("Personnage déclaré deux fois"));
    assert_eq!(fs::read(root.join("main.rvn")).unwrap(),source_bytes);
    assert_eq!(fs::read(root.join(".rvn-authoring.json")).unwrap(),metadata);
    fs::write(root.join("parts/a.rvn"),a).unwrap();
    fs::write(root.join("parts/c.rvn"),"").unwrap();assert!(cold.freeze_native_program().is_err());
    assert_eq!(fs::read(root.join(".rvn-authoring.json")).unwrap(),metadata);
}


// Extend the existing writer/cold regression with genuine Legacy authoring
// history. No sidecar or identity map is edited directly by the fixture.
fn mixed_legacy_neighbors_keep_real_authoring_history_on_native_cold_open(){
    let root=folder("mixed-legacy-history");let path=root.join("main.rvn");
    let raw=r#"init {set seen=0}
screen Widget_1(){return component("root","button",dict("layout_contract","widget_v1"),[])}
handler clicked(event){set seen=seen+1 return 0}
label start
"Before [seen]"
"#;
    fs::write(&path,raw).unwrap();
    let mut workspace=SourceWorkspace::link(&root,&path,&[]).unwrap();
    let mut graphs=workspace.project().graphs().to_vec();
    let screen=graphs.iter().position(|g|matches!(&g.kind,GraphKind::Screen{name}if name=="Widget_1")).unwrap();
    let scene=graphs.iter().position(|g|matches!(&g.kind,GraphKind::Label{name}if name=="start")).unwrap();
    let handler=graphs.iter().position(|g|matches!(&g.kind,GraphKind::Handler{name}if name=="clicked")).unwrap();
    let g=&mut graphs[screen];let parent=g.interface_component_ids()["root"];
    let caption=g.add_catalog_node(NodeKind::UiComponent,[440.,320.]).unwrap();
    g.set_pin_default(caption,"id",PropertyValue::String("history_caption".into())).unwrap();
    g.set_pin_default(caption,"kind",PropertyValue::String("text".into())).unwrap();
    g.set_component_property(caption,&["text"],PropertyValue::String("Untouched caption".into())).unwrap();
    g.set_component_property(caption,&["parent_slot","kind"],PropertyValue::String("button".into())).unwrap();
    g.append_list_input(parent,"children",caption,"value").unwrap();
    // The child is authored before append_list_input materializes its parent's
    // empty collection. A fresh source import allocates that list first, then
    // its child expression. Their provisional numbers are not slot owners.
    let g=&mut graphs[scene];
    let text=g.nodes.values().find(|n|n.kind==NodeKind::TextValue).unwrap().id;
    let outgoing=g.edges.values().find(|e|g.pins[&e.output].node==text).unwrap().clone();
    let dialogue=g.pins[&outgoing.input].node;
    g.remove_edge(outgoing.id);
    let cast=g.add_catalog_node(NodeKind::ConvertTextToString,[180.,260.]).unwrap();
    g.connect(outgoing.output,g.pin_by_key(cast,"value").unwrap().id).unwrap();
    let conversion=g.connect_with_conversions(g.pin_by_key(cast,"result").unwrap().id,outgoing.input,[380.,260.]).unwrap();
    assert_eq!(conversion.conversions.len(),1);
    assert_eq!(g.nodes[&conversion.conversions[0]].kind,NodeKind::ConvertStringToText);
    for id in std::iter::once(cast).chain(conversion.conversions){
        g.set_property(id,"legacy_passthrough",PropertyValue::Bool(true)).unwrap();
    }
    for index in [screen,scene]{
        for node in graphs[index].nodes.values_mut(){node.position=[-77.+node.id.get()as f64*13.,500.+node.id.get()as f64*19.];}
    }
    graphs[handler].enable_native_frame_preparation().unwrap();
    for node in graphs[handler].nodes.values().filter(|n|matches!(n.kind,NodeKind::VariableGet|NodeKind::SetVariable)
        && n.properties.get("name")==Some(&PropertyValue::String("seen".into()))){
        for key in ["value","value_out"]{
            if let Some(pin)=graphs[handler].pin_by_key(node.id,key){assert_eq!(pin.value_type,ValueType::Int,"precondition: source-derived Handler bindings are canonical, never preserved stale Any");}
        }
    }
    workspace.save(&graphs).unwrap();
    let saved=workspace.project().graphs().to_vec();let source_bytes=fs::read(&path).unwrap();
    let metadata_bytes=fs::read(root.join(".rvn-authoring.json")).unwrap();
    let script=workspace.project().resolved_script().unwrap();
    let fresh=import_native_frame_script(&script).unwrap();
    let saved_screen=saved.iter().find(|g|g.kind==graphs[screen].kind).unwrap();
    let fresh_screen=fresh.iter().find(|g|g.kind==saved_screen.kind).unwrap();
    assert_ne!(fresh_screen.interface_component_ids()["history_caption"],caption,
        "precondition: actual source import has a different child allocation order");
    let saved_scene=saved.iter().find(|g|g.kind==graphs[scene].kind).unwrap();
    let fresh_scene=fresh.iter().find(|g|g.kind==saved_scene.kind).unwrap();
    assert!(!fresh_scene.nodes.values().any(|n|matches!(n.kind,NodeKind::ConvertTextToString|NodeKind::ConvertStringToText)),
        "precondition: these real casts are presentation-only in Legacy source");
    assert!(saved_scene.nodes.values().any(|n|n.id==cast&&n.kind==NodeKind::ConvertTextToString));
    assert_ne!(fresh_scene.nodes.keys().collect::<Vec<_>>(),saved_scene.nodes.keys().collect::<Vec<_>>(),
        "precondition: real Legacy cast history has identities absent from source-only allocation");
    assert_eq!(transpile(fresh_screen).unwrap().ast,transpile(saved_screen).unwrap().ast);
    assert_eq!(transpile(fresh_scene).unwrap().ast,transpile(saved_scene).unwrap().ast);
    let saved_native=saved.iter().find(|g|g.kind==graphs[handler].kind).unwrap().clone();
    drop(workspace);
    let mut cold=SourceWorkspace::open(&root).unwrap().unwrap();
    assert!(cold.source_error.is_none(),"{:?}",cold.source_error);
    assert_eq!(cold.project().graphs(),saved.as_slice(),"all source-derived registries and actual saved Legacy/Native identities, casts and layout survive cold open");
    cold.save(&saved).unwrap();
    assert_eq!(fs::read(&path).unwrap(),source_bytes);
    assert_eq!(fs::read(root.join(".rvn-authoring.json")).unwrap(),metadata_bytes);
    // A valid external Legacy source edit must change semantics, not restore a
    // stale whole graph. The unchanged Screen and Native neighbor remain exact;
    // only uniquely compatible nodes may retain their genuine old identities.
    let changed=String::from_utf8(source_bytes).unwrap().replace("Before [seen]","After [seen]");
    assert_ne!(changed,cold.project().source());drop(cold);fs::write(&path,&changed).unwrap();
    let mut external=SourceWorkspace::open(&root).unwrap().unwrap();
    assert!(external.source_error.is_none(),"{:?}",external.source_error);
    let current=external.project().graphs().to_vec();
    assert_eq!(current.iter().find(|g|g.kind==saved_screen.kind).unwrap(),saved_screen);
    assert_eq!(current.iter().find(|g|g.kind==saved_native.kind).unwrap(),&saved_native);
    let changed_scene=current.iter().find(|g|g.kind==saved_scene.kind).unwrap();
    assert_ne!(transpile(changed_scene).unwrap().ast,transpile(saved_scene).unwrap().ast);
    assert!(transpile(changed_scene).unwrap().source.contains("After [seen]"));
    assert!(!transpile(changed_scene).unwrap().source.contains("Before [seen]"));
    assert_eq!(changed_scene.nodes[&dialogue].kind,NodeKind::Dialogue);
    assert_eq!(changed_scene.nodes[&dialogue].position,saved_scene.nodes[&dialogue].position);
    external.save(&current).unwrap();drop(external);
    let again=SourceWorkspace::open(&root).unwrap().unwrap();
    assert!(again.source_error.is_none(),"{:?}",again.source_error);
    assert_eq!(again.project().graphs(),current.as_slice(),"changed Legacy logic is authoritative at the next cold open too");
}

// The deterministic allocator is explicitly injected into the real initial link
// transaction; this fixture does not measure OS random identity generation.
fn readonly_foreign_unit_global_dependencies_keep_ownership_and_publication_atomic() {
    for (case, parameters, body, referenced) in [
        ("read", "arg", "set other=score+arg return", true),
        ("write", "arg", "set score=1 return", true),
        ("parameter", "score", "set other=score return", false),
        ("local-initializer", "arg", "local score=score+arg return", true),
        ("local-shadow", "arg", "local score=1 set other=score return", false),
        ("partial-branch", "arg", "if arg>0 {local score=1} set other=score return", true),
        ("both-branches", "arg", "if arg>0 {local score=1} else {local score=2} set other=score return", false),
        ("iterator", "arg", "for score in [1,2] {set other=score} return", false),
    ] {
        let (root, old) = frozen_unit1021_workspace(&format!("readonly-unit-{case}"));
        let saved = fs::read_to_string(root.join("main.rvn")).unwrap(); drop(old);
        let foreign = format!("init {{set foreign_fixed=7}}\nfunction foreign({parameters}) -> () {{{body}}}\n");
        fs::write(root.join("foreign.rvn"), &foreign).unwrap();
        fs::write(root.join("main.rvn"), format!("use \"foreign.rvn\"\n{saved}")).unwrap();
        let mut workspace = SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
        assert!(workspace.source_error.is_none(), "{case}: {:?}", workspace.source_error);
        let graphs = workspace.project().graphs().to_vec();
        assert!(!graphs.iter().any(|g| matches!(&g.kind, GraphKind::UnitFunction {name} if name=="foreign")));
        let identities = |graphs: &[GraphDocument]| graphs.iter()
            .find(|g| matches!(&g.kind, GraphKind::UnitFunction {name} if name=="scoped")).unwrap()
            .pins.values().filter_map(|p| p.parameter_id.as_ref().map(|id| (p.id,id.clone()))).collect::<Vec<_>>();
        let ids = identities(&graphs); assert_eq!(ids.len(), 2);
        workspace.save(&graphs).unwrap_or_else(|error| panic!("{case} unchanged save: {error}"));
        let cold = SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
        assert!(cold.source_error.is_none(), "{case}: {:?}", cold.source_error);
        assert_eq!(cold.project().graphs(), workspace.project().graphs(), "{case} initial cold");
        assert_eq!(identities(cold.project().graphs()), ids);
        let baseline = workspace.project().graphs().to_vec();
        let bytes = fs::read(root.join("main.rvn")).unwrap();
        let metadata = fs::read(root.join(".rvn-authoring.json")).unwrap();
        let mut imported_edit = baseline.clone();
        imported_edit.iter_mut().find(|g| g.kind==GraphKind::Init).unwrap()
            .rename_variable("foreign_fixed", "renamed_foreign").unwrap();
        assert!(workspace.save(&imported_edit).unwrap_err().contains("Imported global 'foreign_fixed'"));
        let mut conflict = baseline.clone();
        conflict.iter_mut().find(|g| g.kind==GraphKind::Init).unwrap()
            .add_variable("offered_conflict", ValueType::Int, PropertyValue::Int(1)).unwrap();
        conflict.iter_mut().find(|g| matches!(g.kind, GraphKind::Label {..})).unwrap()
            .add_variable("offered_conflict", ValueType::Int, PropertyValue::Int(2)).unwrap();
        assert!(workspace.save(&conflict).unwrap_err().contains("Conflicting variable declaration"));
        assert_eq!(workspace.project().graphs(), baseline.as_slice());
        assert_eq!(fs::read(root.join("main.rvn")).unwrap(), bytes);
        assert_eq!(fs::read(root.join(".rvn-authoring.json")).unwrap(), metadata);
        assert_eq!(fs::read_to_string(root.join("foreign.rvn")).unwrap(), foreign);
        let mut removed = baseline.clone();
        // Retire the owned Handler's real global use before probing the
        // foreign Unit. Its genuine lexical definition alone must decide.
        removed.retain(|g| !matches!(&g.kind, GraphKind::Handler {name} if name=="clicked"));
        for graph in &mut removed { graph.variables.remove("score"); }
        let result = workspace.save(&removed);
        if referenced {
            let error = result.unwrap_err();
            assert!(error.contains("still referenced by a read-only imported Unit"), "{case}: {error}");
            assert_eq!(workspace.project().graphs(), baseline.as_slice());
            assert_eq!(fs::read(root.join("main.rvn")).unwrap(), bytes);
            assert_eq!(fs::read(root.join(".rvn-authoring.json")).unwrap(), metadata);
        } else {
            result.unwrap_or_else(|error| panic!("{case} shadowed global removal: {error}"));
            let cold = SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
            assert!(cold.source_error.is_none(), "{case}: {:?}", cold.source_error);
            assert_eq!(cold.project().graphs(), workspace.project().graphs(), "{case} edited cold");
            assert_eq!(identities(cold.project().graphs()), ids);
            assert!(!cold.project().graphs().iter().any(|g| matches!(&g.kind, GraphKind::UnitFunction {name} if name=="foreign")));
        }
        assert_eq!(fs::read_to_string(root.join("foreign.rvn")).unwrap(), foreign);
        assert_eq!(identities(workspace.project().graphs()), ids);
    }

    // Read-only callers execute from the real resolved AST. They need no
    // writable foreign Graph/ParameterId to survive an owned Save/cold cycle.
    for (case, caller) in [
        ("unit", "function caller() -> () {call foreign(score) return}\n"),
        ("label", "scene label imported_caller\ncall foreign(score)\nreturn\n"),
        ("transitive-label", "function middle() -> () {call foreign(score) return}\nlabel imported_caller\ncall middle()\nreturn\n"),
    ] {
        let (root, old) = frozen_unit1021_workspace(&format!("readonly-unit-caller-{case}"));
        let saved = fs::read_to_string(root.join("main.rvn")).unwrap(); drop(old);
        let foreign = format!("function foreign(arg) -> () {{return}}\nfunction legacy_zero() -> () {{return}}\nfunction untouched_zero() -> () {{call legacy_zero() return}}\n{caller}");
        fs::write(root.join("foreign.rvn"), &foreign).unwrap();
        fs::write(root.join("main.rvn"), format!("use \"foreign.rvn\"\n{saved}")).unwrap();
        let mut workspace = SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
        assert!(workspace.source_error.is_none(), "{case}: {:?}", workspace.source_error);
        let program = workspace.freeze_native_program().unwrap();
        validate_project_script(&program.script, false).unwrap();
        let graphs = workspace.project().graphs().to_vec();
        assert!(!graphs.iter().any(|g|matches!(&g.kind,GraphKind::UnitFunction {name}if name=="foreign"||name=="caller")));
        let identities=|graphs:&[GraphDocument]|graphs.iter().find(|g|matches!(&g.kind,GraphKind::UnitFunction {name}if name=="scoped")).unwrap().pins.values().filter_map(|p|p.parameter_id.as_ref().map(|id|(p.id,id.clone()))).collect::<Vec<_>>();
        let ids=identities(&graphs);assert_eq!(ids.len(),2);
        let bytes = fs::read(root.join("main.rvn")).unwrap();
        workspace.save(&graphs).unwrap();
        assert_eq!(workspace.project().graphs(), graphs.as_slice());
        assert_eq!(workspace.freeze_native_program().unwrap(), program);
        assert_eq!(fs::read(root.join("main.rvn")).unwrap(), bytes);
        let mut edited=graphs.clone();
        let init=edited.iter_mut().find(|g|g.kind==GraphKind::Init).unwrap();
        let setter=init.nodes.values().find(|n|n.kind==NodeKind::SetVariable&&init.variable_node_name(n.id).as_deref()==Some("score")).unwrap().id;
        assert_eq!(init.literal_input_value(setter,"value").unwrap(),Some(PropertyValue::Int(0)));
        init.set_pin_default(setter,"value",PropertyValue::Int(3)).unwrap();
        workspace.save(&edited).unwrap();
        assert!(workspace.freeze_native_program().unwrap().script.iter().any(|s|matches!(s,rvn_parser::Statement::Init {body}if body.iter().any(|s|matches!(s,rvn_parser::Statement::SetVar {name,value:rvn_parser::Expr::Int(3)}if name=="score")))));
        assert_eq!(fs::read_to_string(root.join("foreign.rvn")).unwrap(),foreign);
        let cold=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();assert!(cold.source_error.is_none());
        assert_eq!(cold.project().graphs(),workspace.project().graphs());assert_eq!(identities(cold.project().graphs()),ids);
        let baseline=workspace.project().graphs().to_vec();let bytes=fs::read(root.join("main.rvn")).unwrap();let metadata=fs::read(root.join(".rvn-authoring.json")).unwrap();
        let mut removed=baseline.clone();removed.retain(|g|!matches!(&g.kind,GraphKind::Handler {name}if name=="clicked"));for g in &mut removed {g.variables.remove("score");}
        let error=workspace.save(&removed).unwrap_err();assert!(error.contains("read-only imported"),"{case}: {error}");
        assert_eq!(workspace.project().graphs(),baseline.as_slice());assert_eq!(fs::read(root.join("main.rvn")).unwrap(),bytes);assert_eq!(fs::read(root.join(".rvn-authoring.json")).unwrap(),metadata);
        // A genuine external change still fails CAS; a fresh reader separately
        // diagnoses the actual bad arity before any owning Save is attempted.
        fs::write(root.join("foreign.rvn"),foreign.replace("foreign(score)","foreign()")).unwrap();
        assert!(workspace.save(&baseline).is_err());
        let invalid=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();assert!(invalid.source_error.as_deref().unwrap().contains("attend 1 arguments"));
        assert_eq!(workspace.project().graphs(),baseline.as_slice());assert_eq!(fs::read(root.join("main.rvn")).unwrap(),bytes);assert_eq!(fs::read(root.join(".rvn-authoring.json")).unwrap(),metadata);
        fs::write(root.join("foreign.rvn"),format!("{foreign}function foreign(extra) -> () {{return}}\n")).unwrap();
        assert!(workspace.save(&baseline).is_err());
        let duplicate=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();assert!(duplicate.source_error.as_deref().unwrap().contains("déjà définie"));
        assert_eq!(fs::read(root.join("main.rvn")).unwrap(),bytes);assert_eq!(fs::read(root.join(".rvn-authoring.json")).unwrap(),metadata);
        fs::write(root.join("foreign.rvn"),&foreign).unwrap();
        assert_eq!(identities(workspace.project().graphs()),ids);
    }
    readonly_foreign_callers_keep_narrative_local_rename_dependencies();

    // Handler text cannot express a UnitCall in the current grammar. Do not
    // invent a supposedly valid authoring source just to exercise that route.
    assert!(rvn_parser::parse("function foreign(arg) -> () {return}\nhandler caller(event) {call foreign(1)}\n").is_err());

}

// The existing fragment dependency algorithm must see raw UnitCall arguments
// in foreign labels, including callers before and after a Local introduction.
fn readonly_foreign_callers_keep_narrative_local_rename_dependencies() {
    for (case, prefix, foreign_label, foreign_local, refused) in [
        ("inherited", "local token=1\ncall imported_caller\n", "label", "", true),
        ("before", "call imported_caller\nlocal token=1\n", "label", "", false),
        ("scene", "local token=1\ncall imported_caller\n", "scene label", "", false),
        ("callee-local", "call imported_caller\nlocal token=1\n", "label", "local token=2\n", true),
    ] {
        let(root,old)=frozen_unit1021_workspace(&format!("readonly-local-{case}"));
        let saved=fs::read_to_string(root.join("main.rvn")).unwrap();drop(old);
        // Keep both the owned UnitCall and the foreign UnitCall. Precise
        // narrative Local analysis must compile with the genuine owned project,
        // rather than deleting an unrelated UnitCall to make analysis succeed.
        let parsed=rvn_parser::parse_with_origins(&saved,rvn_parser::NarrativeContext::ExplicitScenes).unwrap();
        let index=parsed.script.iter().position(|s|matches!(s,rvn_parser::Statement::UnitCall {name,..}if name=="scoped")).unwrap();
        let mut saved=saved;saved.replace_range(parsed.spans[index].range.clone(),&format!("{prefix}call scoped(10,11)\n"));
        assert!(saved.contains(prefix));
        let foreign=format!("function foreign(arg) -> () {{return}}\n{foreign_label} imported_caller\n{foreign_local}call foreign(token)\nreturn\n");
        fs::write(root.join("foreign.rvn"),&foreign).unwrap();
        fs::write(root.join("main.rvn"),format!("use \"foreign.rvn\"\ninit {{set token=5}}\n{saved}")).unwrap();
        let mut workspace=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();assert!(workspace.source_error.is_none(),"{case}: {:?}",workspace.source_error);
        let before=workspace.project().graphs().to_vec();let bytes=fs::read(root.join("main.rvn")).unwrap();let metadata=fs::read(root.join(".rvn-authoring.json")).unwrap();
        let identities=|graphs:&[GraphDocument]|graphs.iter().find(|g|matches!(&g.kind,GraphKind::UnitFunction {name}if name=="scoped")).unwrap().pins.values().filter_map(|p|p.parameter_id.as_ref().map(|id|(p.id,id.clone()))).collect::<Vec<_>>();
        let ids=identities(&before);assert_eq!(ids.len(),2);
        let mut edited=before.clone();let caller=edited.iter_mut().find(|g|matches!(&g.kind,GraphKind::Label {name}if name=="start")).unwrap();
        assert_eq!(caller.variable_scope("token"),Some(VariableScope::Local));caller.rename_variable_in_project("token","renamed_token",&before).unwrap();
        let preflight=workspace.project().check_variable_dependencies(&edited);
        assert_eq!(workspace.project().graphs(),before.as_slice());assert_eq!(fs::read(root.join("main.rvn")).unwrap(),bytes);assert_eq!(fs::read(root.join(".rvn-authoring.json")).unwrap(),metadata);
        let result=workspace.save(&edited);
        if refused {assert!(preflight.unwrap_err().contains("read-only imported fragment 'imported_caller'"));let error=result.unwrap_err();assert!(error.contains("read-only imported fragment 'imported_caller'"),"{case}: {error}");assert_eq!(workspace.project().graphs(),before.as_slice());assert_eq!(fs::read(root.join("main.rvn")).unwrap(),bytes);assert_eq!(fs::read(root.join(".rvn-authoring.json")).unwrap(),metadata);}
        else {preflight.unwrap_or_else(|e|panic!("{case} preflight: {e}"));result.unwrap_or_else(|e|panic!("{case}: {e}"));let cold=SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();assert!(cold.source_error.is_none());assert_eq!(cold.project().graphs(),workspace.project().graphs());}
        assert_eq!(fs::read_to_string(root.join("foreign.rvn")).unwrap(),foreign);
        assert_eq!(identities(workspace.project().graphs()),ids);
    }
}

#[test]
fn owned_unit_call1021_local_rename_keeps_real_parameters_reads_and_cold_source() {
    for unit_caller in [false, true] {
        let root=folder(if unit_caller {"owned-unit-local-rename"} else {"owned-scene-local-rename"});
        let path=root.join("main.rvn");
        let body="local token=7\ncall scoped(token,11)\n";
        let caller=if unit_caller {format!("function caller() -> () {{{body}return}}\nscene label start\ncall caller()\n")}
            else {format!("scene label start\n{body}")};
        let raw=format!("init {{set result=0 set token=5}}\nfunction scoped(alpha,beta) -> () {{set result=alpha*10+beta return}}\nhandler clicked(event) {{set result=result+1}}\n{caller}\"[result]\"\nreturn\n");
        fs::write(&path,&raw).unwrap();
        let mut allocated=0u64;
        let parsed=rvn_parser::parse_program(&raw).unwrap();
        assert_eq!(parsed.context,rvn_parser::NarrativeContext::ExplicitScenes);
        let prepared=prepare_initial_unit_link_graphs(&parsed.script,&[],|| {
            allocated+=1;Ok(format!("{:032x}",0x9000+allocated))
        }).unwrap();
        assert_eq!(allocated,2,"only the explicitly injected initial link transaction supplies the actual Unit parameter IDs");
        let mut workspace=SourceWorkspace::link(&root,&path,&prepared).unwrap();
        let mut graphs=workspace.project().graphs().to_vec();
        graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Handler {..})).unwrap()
            .enable_native_frame_preparation().unwrap();
        workspace.save(&graphs).unwrap();
        let before=workspace.project().graphs().to_vec();
        let index=before.iter().position(|g| if unit_caller {
            matches!(&g.kind,GraphKind::UnitFunction {name} if name=="caller")
        } else {matches!(&g.kind,GraphKind::Label {name} if name=="start")}).unwrap();
        assert_eq!(before[index].variable_scope("token"),Some(VariableScope::Local));
        let declarations=before[index].local_declaration_initializers_for_binding_analysis_in_project(&before).unwrap();
        let [initial]=declarations.get("token").unwrap().as_slice() else {panic!("one real local declaration")};
        assert_eq!(initial.literal_value,Some(PropertyValue::Int(7)));
        assert!(before[index].local_declaration_initializers_for_binding_analysis().is_err(),"context-free UnitCall analysis stays conservative");
        let mut valued=before[index].clone();
        assert!(valued.replace_local_initializer_in_project("token",initial,PropertyValue::Int(8),&before).unwrap());
        let mut valued_project=before.clone();valued_project[index]=valued.clone();
        let changed=valued.local_declaration_initializers_for_binding_analysis_in_project(&valued_project).unwrap();
        assert_eq!(changed["token"][0].literal_value,Some(PropertyValue::Int(8)));
        assert!(transpile_project(&valued_project).unwrap().source.contains("local token = 8"));
        let proposal=before[index].prepare_local_variable_type_slot_in_project("token",ValueType::Float,&before).unwrap();
        let typed=workspace.project().reconstruct_local_variable_type(&proposal,&before[index],&before).unwrap();
        let mut typed_project=before.clone();typed_project[index]=typed.clone();
        assert_eq!(typed.local_declaration_initializers_for_binding_analysis_in_project(&typed_project).unwrap()["token"][0].literal_value,Some(PropertyValue::Float(0.0)));
        for bad in 0..3 {
            let mut bad_project=before.clone();
            if bad==0 {bad_project.remove(index);}
            if bad==1 {bad_project.push(before[index].clone());}
            if bad==2 {bad_project[index].variables.get_mut("token").unwrap().default_value=PropertyValue::Int(99);}
            let mut trial=before[index].clone();
            assert!(trial.replace_local_initializer_in_project("token",initial,PropertyValue::Int(8),&bad_project).is_err());
            assert_eq!(trial,before[index]);
            assert!(trial.prepare_local_variable_type_slot_in_project("token",ValueType::Float,&bad_project).is_err());
            assert!(trial.local_declaration_initializers_for_binding_analysis_in_project(&bad_project).is_err());
        }
        let call=before[index].nodes.values().find(|n|n.kind==NodeKind::UnitCall).unwrap().id;
        let inputs=before[index].nodes[&call].pins.iter().copied()
            .filter(|id|before[index].pins[id].parameter_id.is_some()).collect::<Vec<_>>();
        assert_eq!(inputs.len(),2);
        let bytes=fs::read(&path).unwrap();let metadata=fs::read(root.join(".rvn-authoring.json")).unwrap();
        let mut no_context=before[index].clone();
        assert!(no_context.rename_variable("token","renamed_token").unwrap_err().to_string()
            .contains("actual resolved project/source context"));
        assert_eq!(no_context,before[index]);
        for invalid in 0..4 {
            let mut context=before.clone();
            if invalid==0 {context[index].nodes.get_mut(&call).unwrap().properties.insert("function".into(),PropertyValue::String("clicked".into()));}
            if invalid==1 {context[index].add_pin(call,"unexpected_argument","unexpected_argument",PinDirection::Input,ValueType::Any,PinCardinality::One).unwrap();}
            if invalid==2 {
                // Reuse the other genuine parameter identity on the wrong
                // actual argument; no fabricated identity participates.
                let other=context[index].pins[&inputs[1]].parameter_id.clone();
                context[index].pins.get_mut(&inputs[0]).unwrap().parameter_id=other;
            }
            let mut trial=context[index].clone();let snapshot=trial.clone();
            if invalid==3 {context.retain(|g|g.graph_id!=trial.graph_id);}
            let error=trial.rename_variable_in_project("token","renamed_token",&context).unwrap_err().to_string();
            if invalid==0 {assert!(error.contains("value Function, Screen or Handler"),"{error}");}
            if invalid==3 {assert!(error.contains("one actual project graph owner"),"{error}");}
            assert_eq!(trial,snapshot,"a real ownership/arity refusal is atomic");
            assert_eq!(workspace.project().graphs(),before.as_slice());
            assert_eq!(fs::read(&path).unwrap(),bytes);assert_eq!(fs::read(root.join(".rvn-authoring.json")).unwrap(),metadata);
        }
        let mut renamed=before[index].clone();
        renamed.rename_variable_in_project("token","renamed_token",&before).unwrap();
        assert_eq!(renamed.nodes[&call],before[index].nodes[&call],"callee and actual argument port ownership do not change");
        for id in &inputs {assert_eq!(renamed.pins[id],before[index].pins[id]);}
        let mut edited=before.clone();edited[index]=renamed.clone();
        workspace.project().check_variable_dependencies(&edited).unwrap();
        let compiled=transpile_project(&edited).unwrap();
        assert!(compiled.source.contains("local renamed_token = 7"),"{}",compiled.source);
        assert!(compiled.source.contains("call scoped(renamed_token, 11)"),"{}",compiled.source);
        // An authentic external source edit must still fail the real writer
        // CAS even though the contextual rename itself was valid.
        let external=format!("{}\nlabel external_change\nreturn\n",String::from_utf8(bytes.clone()).unwrap());
        fs::write(&path,&external).unwrap();assert!(workspace.save(&edited).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(),external);assert_eq!(workspace.project().graphs(),before.as_slice());
        assert_eq!(fs::read(root.join(".rvn-authoring.json")).unwrap(),metadata);
        fs::write(&path,&bytes).unwrap();workspace.save(&edited).unwrap();
        let saved=workspace.project().graphs().to_vec();
        let cold=SourceWorkspace::open(&root).unwrap().unwrap();assert!(cold.source_error.is_none(),"{:?}",cold.source_error);
        assert_eq!(cold.project().graphs(),saved.as_slice());
        let parameters=|graphs:&[GraphDocument]|graphs.iter().flat_map(|g|g.pins.values())
            .filter_map(|p|p.parameter_id.as_ref().map(|id|(p.id,id.clone()))).collect::<Vec<_>>();
        assert_eq!(parameters(cold.project().graphs()),parameters(&before));
        assert_eq!(cold.project().graphs().iter().find(|g|matches!(g.kind,GraphKind::Handler {..})),before.iter().find(|g|matches!(g.kind,GraphKind::Handler {..})));
        let mut engine=rvn_core::Engine::from_program(cold.freeze_native_program().unwrap(),rvn_core::TerminalRenderer,16).unwrap();
        engine.enter_label("start").unwrap();
        assert_eq!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue {character:None,text:"81".into()}));
        assert_eq!(engine.state.vars["token"],rvn_parser::Value::Int(5),"the actual project global is not renamed with an activation local");
    }
}

#[test]
fn owned_unit_call1021_retained_scene_resolves_new_zero_parameter_unit_in_complete_native_context() {
    let (root, old) = frozen_unit1021_workspace("retained-scene-new-unit-context");
    let path = root.join("main.rvn");
    let before = old.project().graphs().to_vec();
    assert!(!before.iter().any(|graph| matches!(&graph.kind, GraphKind::UnitFunction { name } if name == "added")));
    let scene_id = before.iter().find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start")).unwrap().graph_id;
    let parameter_ids = |graphs: &[GraphDocument]| graphs.iter()
        .find(|graph| matches!(&graph.kind, GraphKind::UnitFunction { name } if name == "scoped")).unwrap()
        .pins.values().filter_map(|pin| pin.parameter_id.as_ref().map(|identity| (pin.id, identity.clone()))).collect::<Vec<_>>();
    let ids = parameter_ids(&before);
    assert_eq!(ids.len(), 2);
    let source = fs::read_to_string(&path).unwrap();
    let parsed = rvn_parser::parse_with_origins(&source, rvn_parser::NarrativeContext::ExplicitScenes).unwrap();
    let call = parsed.script.iter().position(|statement| matches!(statement, rvn_parser::Statement::UnitCall { name, .. } if name == "scoped")).unwrap();
    let mut source = source;
    source.replace_range(parsed.spans[call].range.clone(), "local token=7\ncall added()\ncall scoped(token,11)\n\"[score]:[token]\"\n");
    // A new zero-parameter declaration requires no opaque parameter allocator.
    // This is actual authored source, not a fake Graph inserted into context.
    let source = format!("init {{set token=5}}\nfunction added() -> () {{set score=score+1 return}}\n{source}");
    assert_eq!(rvn_parser::parse_program(&source).unwrap().context, rvn_parser::NarrativeContext::ExplicitScenes);
    fs::write(&path, &source).unwrap();
    drop(old);

    // The scene has a real retained presentation owner. The added Unit does
    // not, so it is absent from the selection reconciled by the old route.
    let mut workspace = SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
    assert!(workspace.source_error.is_none(), "{:?}", workspace.source_error);
    let graphs = workspace.project().graphs().to_vec();
    let scene = graphs.iter().find(|graph| matches!(&graph.kind, GraphKind::Label { name } if name == "start")).unwrap();
    assert_eq!(scene.graph_id, scene_id);
    assert_eq!(scene.variable_scope("token"), Some(VariableScope::Local));
    assert!(scene.nodes.values().any(|node| node.kind == NodeKind::UnitCall
        && node.properties.get("function") == Some(&PropertyValue::String("added".into()))));
    let added = graphs.iter().find(|graph| matches!(&graph.kind, GraphKind::UnitFunction { name } if name == "added")).unwrap();
    assert!(!added.pins.values().any(|pin| pin.parameter_id.is_some()));
    assert_eq!(parameter_ids(&graphs), ids);
    assert_eq!(fs::read_to_string(&path).unwrap(), source, "opening never rewrites the authored source");
    workspace.save(&graphs).unwrap();
    let cold = SourceWorkspace::open_native_preparation(&root).unwrap().unwrap();
    assert!(cold.source_error.is_none(), "{:?}", cold.source_error);
    assert_eq!(cold.project().graphs(), workspace.project().graphs());
    assert_eq!(parameter_ids(cold.project().graphs()), ids);
    let frozen = cold.freeze_native_program().unwrap();
    let mut engine = rvn_core::Engine::from_program(frozen, rvn_core::TerminalRenderer, 16).unwrap();
    engine.enter_label("start").unwrap();
    assert_eq!(engine.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { character: None, text: "1:7".into() }));
    assert_eq!(engine.state.vars["token"], rvn_parser::Value::Int(5));
}

#[test]
fn owned_unit_call1021_unit_global_inference_matches_resolved_and_cold_lexical_activation() {
    let root=folder("unit-global-lexical-cold");let path=root.join("main.rvn");
    let raw="init {set token=5 set before_local=0}\nfunction lexical(parameter) -> () {set before_local=token local token=7 local unit_only=parameter set token=8 set unit_only=9 set global_only=true return}\nscene label start\ncall lexical(3)\n\"[token]:[before_local]\"\nreturn\n";
    fs::write(&path,raw).unwrap();let parsed=rvn_parser::parse_program(raw).unwrap();let mut allocations=0;
    let prepared=prepare_initial_unit_link_graphs(&parsed.script,&[],||{allocations+=1;Ok(format!("{:032x}",0xD000+allocations))}).unwrap();assert_eq!(allocations,1);
    let direct=SourceProject::open(raw,&prepared).unwrap();let resolved=SourceProject::open_at(&path,raw,&prepared).unwrap();
    assert_eq!(direct.graphs(),resolved.graphs(),"actual resolved and bare source import share Unit activation analysis");
    let init=direct.graphs().iter().find(|g|g.kind==GraphKind::Init).unwrap();
    assert_eq!(init.variables["token"].value_type,ValueType::Int);assert_eq!(init.variables["token"].default_value,PropertyValue::Int(5));
    assert_eq!(init.variables["global_only"].value_type,ValueType::Bool);
    assert!(!init.variables.contains_key("unit_only"));assert!(!init.variables.contains_key("parameter"));
    let mut workspace=SourceWorkspace::link(&root,&path,&prepared).unwrap();let before=workspace.project().graphs().to_vec();
    let index=before.iter().position(|g|matches!(&g.kind,GraphKind::UnitFunction{name}if name=="lexical")).unwrap();
    let mut edited=before.clone();edited[index].rename_variable_in_project("unit_only","renamed_unit",&before).unwrap();
    workspace.save(&edited).unwrap();let saved=workspace.project().graphs().to_vec();
    let cold=SourceWorkspace::open(&root).unwrap().unwrap();assert!(cold.source_error.is_none(),"{:?}",cold.source_error);
    assert_eq!(cold.project().graphs(),saved.as_slice(),"retain exact complete graphs, no weakened cold equality");
    let init=cold.project().graphs().iter().find(|g|g.kind==GraphKind::Init).unwrap();assert!(!init.variables.contains_key("renamed_unit"));assert!(!init.variables.contains_key("parameter"));
    let mut engine=rvn_core::Engine::from_program(cold.project().resolved_program().unwrap(),rvn_core::TerminalRenderer,16).unwrap();engine.enter_label("start").unwrap();
    assert_eq!(engine.step_until_interaction().unwrap(),Some(rvn_core::Interaction::Dialogue{character:None,text:"5:5".into()}));
    assert_eq!(engine.state.vars["token"],rvn_parser::Value::Int(5));assert_eq!(engine.state.vars["global_only"],rvn_parser::Value::Bool(true));
    assert!(!engine.state.vars.contains_key("renamed_unit"));assert!(!engine.state.vars.contains_key("parameter"));
}
