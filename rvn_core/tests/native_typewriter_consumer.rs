//! Private targeted tests: genuine graph -> strict preparation -> file program
//! -> source-bound constructor. No fabricated owner or publication token.
use rvn_core::{Engine, Renderer, renderer::SceneRenderBatch, types::SpriteState};
use rvn_graph::*;
use rvn_parser::{Hotspot, ParsedProgram, Position, Transition, Value};
use std::{path::PathBuf, sync::{Arc, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}}};

#[derive(Clone, Default)]
struct Host {
    batches: Arc<Mutex<Vec<SceneRenderBatch>>>,
    configs: Arc<Mutex<Vec<f32>>>,
    reject: Arc<AtomicBool>,
}
impl Renderer for Host {
    fn validate_scene_batch(&self, _: &SceneRenderBatch) -> Result<(),String> {
        if self.reject.load(Ordering::SeqCst) {Err("typewriter fixture rejects candidate".into())} else {Ok(())}
    }
    fn publish_scene_batch(&mut self, batch: SceneRenderBatch) {self.batches.lock().unwrap().push(batch);}
    fn set_typewriter_config(&mut self, speed: f32) {self.configs.lock().unwrap().push(speed);}
    fn set_background(&mut self, _: &str, _: &Transition) {}
    fn show_sprite(&mut self, _: &str, _: Option<&str>, _: &Position, _: &Transition, _: Option<&SpriteState>) {}
    fn hide_sprite(&mut self, _: &str, _: &Transition, _: &SpriteState) {}
    fn move_sprite(&mut self, _: &str, _: &Position, _: &Transition, _: &SpriteState) {}
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize {0}
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize {0}
    fn music_play(&mut self, _: &str, _: &Transition, _: Option<&str>) {}
    fn music_stop(&mut self, _: &Transition) {}
    fn music_set_volume(&mut self, _: f32) {}
    fn sfx_play(&mut self, _: &str, _: &Transition) {}
    fn sfx_stop(&mut self, _: &str, _: &Transition) {}
}

fn fixture_dir(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = PathBuf::from(std::env::var("RVN_NATIVE_TYPEWRITER_FIXTURES").expect("private fixture directory"));
    let dir = root.join(format!("{name}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    std::fs::create_dir_all(&dir).unwrap(); dir
}
// This fixture authors native graph input after opt-in. Legacy import of the
// original dynamic command remains refused: it cannot supply native identity.
fn native_fixture_seed_graphs(original:&ParsedProgram)->Vec<GraphDocument> {
    fn seed(block:&mut [rvn_parser::Statement],dynamic:&mut Vec<(NodeKind,rvn_parser::Expr)>) {
        for statement in block {
            match statement {
                rvn_parser::Statement::TypewriterSetValue{enabled}=>{
                    dynamic.push((NodeKind::TypewriterSet,enabled.clone()));
                    *statement=rvn_parser::Statement::TypewriterSet{enabled:true};
                }
                rvn_parser::Statement::TypewriterSpeedValue{chars_per_sec}=>{
                    dynamic.push((NodeKind::TypewriterSpeed,chars_per_sec.clone()));
                    *statement=rvn_parser::Statement::TypewriterSpeed{chars_per_sec:20};
                }
                rvn_parser::Statement::Init{body}=>seed(body,dynamic),
                _=>{}
            }
        }
    }
    fn random(graph:&mut GraphDocument)->PinId {
        let node=graph.add_catalog_node(NodeKind::FunctionCall,[160.,300.]).unwrap();
        graph.set_property(node,"function",PropertyValue::String("random".into())).unwrap();
        graph.resize_value_inputs(node,2).unwrap();
        for (key,value) in [("item_0",1),("item_1",9)] {
            let pin=graph.pin_by_key(node,key).unwrap().id;
            graph.pins.get_mut(&pin).unwrap().default_value=Some(PropertyValue::Int(value));
        }
        graph.pin_by_key(node,"result").unwrap().id
    }
    let mut script=original.script.clone();let mut dynamic=Vec::new();seed(&mut script,&mut dynamic);
    if !dynamic.is_empty() {
        let refusal=import_script(&original.script).expect_err("raw dynamic legacy import remains refused");
        assert!(refusal.contains("constante RVN"),"{refusal}");
    }
    let mut graphs=import_script(&script).unwrap();
    for (kind,expression) in dynamic {
        let matches:Vec<_>=graphs.iter().enumerate().flat_map(|(i,g)|g.nodes.values().filter(move|n|n.kind==kind).map(move|n|(i,n.id))).collect();
        assert_eq!(matches.len(),1,"each dynamic fixture has one actual command of its kind");
        let (index,command)=matches[0];let graph=&mut graphs[index];
        graph.enable_native_frame_preparation().unwrap();
        let key=if kind==NodeKind::TypewriterSet{"enabled"}else{"speed"};
        let input=graph.pin_by_key(command,key).unwrap().id;
        let old=graph.edges.values().find(|edge|edge.input==input).unwrap().clone();
        let literal=graph.pins[&old.output].node;assert_eq!(graph.nodes[&literal].kind,NodeKind::Literal);
        assert_eq!(graph.edges.values().filter(|edge|edge.output==old.output).count(),1);
        graph.remove_edge(old.id);graph.remove_node(literal);
        let output=match expression {
            rvn_parser::Expr::Not(value)=>{
                assert_eq!(*value,rvn_parser::Expr::Bool(false));
                let node=graph.add_catalog_node(NodeKind::LogicNot,[300.,300.]).unwrap();
                let input=graph.pin_by_key(node,"value").unwrap().id;
                graph.pins.get_mut(&input).unwrap().default_value=Some(PropertyValue::Bool(false));
                graph.pin_by_key(node,"result").unwrap().id
            }
            rvn_parser::Expr::Call{name,args}=>{
                assert_eq!(name,"random");assert_eq!(args,vec![rvn_parser::Expr::Int(1),rvn_parser::Expr::Int(9)]);
                random(graph)
            }
            rvn_parser::Expr::BinOp{op:rvn_parser::BinOpKind::Gt,left,right}=>{
                assert_eq!(*left,rvn_parser::Expr::Call{name:"random".into(),args:vec![rvn_parser::Expr::Int(1),rvn_parser::Expr::Int(9)]});
                assert_eq!(*right,rvn_parser::Expr::Int(4));
                let random=random(graph);let node=graph.add_catalog_node(NodeKind::MathGreater,[300.,300.]).unwrap();
                graph.connect(random,graph.pin_by_key(node,"left").unwrap().id).unwrap();
                let right=graph.pin_by_key(node,"right").unwrap().id;
                graph.pins.get_mut(&right).unwrap().default_value=Some(PropertyValue::Int(4));
                graph.pin_by_key(node,"value").unwrap().id
            }
            unsupported=>panic!("dynamic fixture must author its real typed producer: {unsupported:?}"),
        };
        graph.connect(output,input).unwrap();graph.refresh_variable_usage_bindings().unwrap();
    }
    graphs
}

fn program(name: &str, source: &str) -> ParsedProgram {
    let dir=fixture_dir(name); let input=dir.join("authored.rvn");
    std::fs::write(&input,source).unwrap();
    let original=rvn_parser::parse_file_program_with_uses(&input).unwrap();
    let graphs=native_fixture_seed_graphs(&original);
    finish_program(dir,original,graphs)
}
fn inline_program(name:&str,source:&str,changes:&[(&str,usize,bool)]) -> ParsedProgram {
    let dir=fixture_dir(name);let input=dir.join("authored.rvn");
    std::fs::write(&input,source).unwrap();
    let original=rvn_parser::parse_file_program_with_uses(&input).unwrap();
    let mut graphs=native_fixture_seed_graphs(&original);
    for (label,index,enabled) in changes {
        let graph=graphs.iter_mut().find(|g|(*label=="@init"&&matches!(g.kind,GraphKind::Init))||matches!(&g.kind,GraphKind::Label{name}if name==label)).unwrap();
        let attachment=if *label=="@init" {
            assert_eq!(*index,0);
            let outputs:Vec<_>=graph.pins.values().filter(|p|p.direction==PinDirection::Output&&p.key=="exec_out"&&!graph.edges.values().any(|e|e.output==p.id)).map(|p|p.id).collect();
            assert_eq!(outputs.len(),1); (outputs[0],None)
        }else{
            let dialogue=graph.nodes.values().filter(|n|n.kind==NodeKind::Dialogue).nth(*index).unwrap().id;
            let input=graph.pin_by_key(dialogue,"exec_in").unwrap().id;
            let prior=graph.edges.values().find(|e|e.input==input).unwrap().clone();
            graph.remove_edge(prior.id); (prior.output,Some(input))
        };
        let command=graph.add_catalog_node(NodeKind::TypewriterSet,[300.,300.]).unwrap();
        // Inline authored pin data is admitted by strict compilation. This is
        // not a conversion/removal of an existing connected Boolean producer.
        let enabled_pin=graph.pin_by_key(command,"enabled").unwrap().id;
        graph.pins.get_mut(&enabled_pin).unwrap().default_value=Some(PropertyValue::Bool(*enabled));
        graph.connect(attachment.0,graph.pin_by_key(command,"exec_in").unwrap().id).unwrap();
        if let Some(input)=attachment.1 {graph.connect(graph.pin_by_key(command,"exec_out").unwrap().id,input).unwrap();}
    }
    finish_program(dir,original,graphs)
}
fn finish_program(dir:PathBuf,original:ParsedProgram,mut graphs:Vec<GraphDocument>)->ParsedProgram {
    let mut source=String::new();let mut ast=Vec::new();
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
        assert_eq!(restored.edges,graph.edges);
        assert_eq!(restored.nodes.keys().collect::<Vec<_>>(),graph.nodes.keys().collect::<Vec<_>>());
        assert_eq!(restored.pins.keys().collect::<Vec<_>>(),graph.pins.keys().collect::<Vec<_>>());
        std::fs::write(dir.join(format!("graph-{}.json",graph.graph_id)),graph.to_pretty_json().unwrap()).unwrap();
        source.push_str(&strict.source);source.push('\n');ast.extend(strict.ast);
    }
    // Each actual graph was strictly prepared above. Concatenate its source
    // directly: project export adds a synthetic end Label/Jump which has no
    // authoring identity and is irrelevant to these targeted real programs.
    let file=dir.join("main.rvn"); std::fs::write(&file,&source).unwrap();
    let parsed=rvn_parser::parse_file_program_with_uses(&file).unwrap();
    assert_eq!(parsed.script,ast);
    let origins=NativeDebugSourceMap::from_program(&parsed).unwrap();
    let commands:Vec<_>=origins.origins().iter().filter(|o|matches!(o.kind,NodeKind::TypewriterSet|NodeKind::TypewriterSpeed)).collect();
    assert!(!commands.is_empty());
    for command in commands {
        assert!(command.span.start<command.span.end);
        assert_eq!(command.source,std::fs::canonicalize(&file).unwrap());
        assert!(graphs.iter().any(|g|g.graph_id==command.graph&&g.nodes.contains_key(&command.node)));
    }
    parsed
}

#[test]
fn graph_to_scene_typewriter_executes() {
    let p=inline_program("scene", "scene label start\n\"boundary\"\n",&[("start",0,true)]);
    let mut engine=Engine::from_program(p,Host::default(),8).expect("source-bound constructor admits TypewriterSet");
    engine.step_until_interaction().unwrap();
    assert!(engine.state.typewriter.enabled);
    let batches=engine.renderer.batches.lock().unwrap();
    assert_eq!(batches.iter().filter(|b|!b.previous.typewriter.enabled&&b.state.typewriter.enabled).count(),1);
    assert!(engine.renderer.configs.lock().unwrap().is_empty(),"scene effects must publish in the actual coherent batch");
}

fn frozen(e:&Engine<Host>)->serde_json::Value {serde_json::to_value(&e.state).unwrap()}
fn state_changes(host:&Host)->usize {host.batches.lock().unwrap().iter().filter(|b|b.previous.typewriter!=b.state.typewriter).count()}
fn expected_draw(mut random:rvn_core::random::RandomState)->rvn_core::random::RandomState {random.integer(1,9).unwrap();random}
fn direct_program(name:&str,source:&str)->ParsedProgram {
    let root=fixture_dir(name);let path=root.join("main.rvn");std::fs::write(&path,source).unwrap();
    rvn_parser::parse_file_program_with_uses(path).unwrap()
}

#[test]
fn graph_game_owner_in_narrative_program_keeps_one_draw_and_one_effect() {
    let graph=inline_program("game-owner-graph","label start\nset sampled=random(1,9)\n\"game\"\n",&[("start",0,true)]);
    // Keep the genuine native Game graph and load it in a real narrative
    // programme. Importing mixed fallthrough graphs currently inserts a Jump
    // whose connected LabelValue has a separate unresolved native identity gap.
    // This fixture does not delete or rewrite that Jump; it authors no Jump.
    let p=direct_program("game-owner",&format!("{}\nscene label later\n\"later\"\n",graph.sources[0].source));
    let mut e=Engine::from_program(p,Host::default(),8).unwrap();
    e.state.random=rvn_core::random::RandomState::seeded(17);let expected=expected_draw(e.state.random);
    e.step_silent().unwrap();assert!(e.state.typewriter.enabled);assert_eq!(e.state.random,expected);
    assert!(e.state.scene_execution.as_ref().unwrap().active.is_none());
    assert_eq!(state_changes(&e.renderer),1);assert!(e.renderer.configs.lock().unwrap().is_empty());
    let before=frozen(&e);let effects=state_changes(&e.renderer);e.step_silent().unwrap();
    assert_eq!(frozen(&e),before);assert_eq!(state_changes(&e.renderer),effects);
}

#[test]
fn graph_legacy_game_uses_historical_config_callback_once() {
    let p=inline_program("legacy-game","label start\nset sampled=random(1,9)\n\"boundary\"\n",&[("start",0,true)]);
    let mut e=Engine::from_program(p,Host::default(),8).unwrap();
    e.state.random=rvn_core::random::RandomState::seeded(17);let expected=expected_draw(e.state.random);
    e.step_silent().unwrap();assert_eq!(e.state.random,expected);assert!(e.state.typewriter.enabled);
    assert_eq!(*e.renderer.configs.lock().unwrap(),vec![40.]);
    e.step_silent().unwrap();assert_eq!(*e.renderer.configs.lock().unwrap(),vec![40.]);
}

#[test]
fn graph_scene_enables_then_disables_at_actual_consumer_sites() {
    let p=inline_program("on-off","scene label start\n\"on\"\n\"off\"\n",&[("start",0,true),("start",1,false)]);
    let mut e=Engine::from_program(p,Host::default(),8).unwrap();
    e.step_until_interaction().unwrap();assert!(e.state.typewriter.enabled);assert_eq!(state_changes(&e.renderer),1);
    e.advance_dialogue().unwrap();e.step_until_interaction().unwrap();assert!(!e.state.typewriter.enabled);
    assert_eq!(state_changes(&e.renderer),2);assert!(e.renderer.configs.lock().unwrap().is_empty());
}

#[test]
fn graph_renderer_refusal_keeps_frame_pc_rng_and_effects_unchanged() {
    let p=inline_program("reject","scene label start\n\"before\"\n\"after\"\n",&[("start",1,true)]);
    let mut e=Engine::from_program(p,Host::default(),8).unwrap();
    e.step_until_interaction().unwrap();e.advance_dialogue().unwrap();
    let before=frozen(&e);let count=e.renderer.batches.lock().unwrap().len();
    let session=e.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![]).unwrap();
    e.renderer.reject.store(true,Ordering::SeqCst);
    let error=e.step_silent().unwrap_err().to_string();assert!(error.contains("typewriter fixture rejects candidate"),"{error}");
    assert_eq!(frozen(&e),before);assert_eq!(e.renderer.batches.lock().unwrap().len(),count);assert_eq!(state_changes(&e.renderer),0);
    assert!(e.accepts_debug_session(&session));
    e.renderer.reject.store(false,Ordering::SeqCst);e.step_silent().unwrap();
    assert!(e.state.typewriter.enabled);assert_eq!(state_changes(&e.renderer),1);
}

#[test]
fn graph_cold_save15_restores_setting_without_replaying_producer() {
    let p=inline_program("cold","scene label start\nset sampled=random(1,9)\n\"on\"\n\"off\"\n",&[("start",0,true),("start",1,false)]);
    let mut live=Engine::from_program(p.clone(),Host::default(),8).unwrap();live.state.random=rvn_core::random::RandomState::seeded(17);
    live.step_until_interaction().unwrap();let random=live.state.random;let sampled=live.state.vars["sampled"].clone();
    let save=live.capture_save15(&p,None,3,"typewriter".into()).unwrap();
    let encoded=save.encode().unwrap();let save=rvn_core::save::consumer_checkpoint::Save15::decode(&encoded).unwrap();
    let mut cold=Engine::from_program(p.clone(),Host::default(),8).unwrap();
    assert!(!cold.state.typewriter.enabled);cold.restore_save15(&p,save).unwrap();
    assert!(cold.state.typewriter.enabled);assert_eq!(cold.state.random,random);assert_eq!(cold.state.vars["sampled"],sampled);
    assert!(cold.renderer.configs.lock().unwrap().is_empty());
    let count=cold.renderer.batches.lock().unwrap().len();cold.step_silent().unwrap();
    assert_eq!(cold.state.random,random);assert_eq!(cold.renderer.batches.lock().unwrap().len(),count);
    cold.advance_dialogue().unwrap();cold.step_silent().unwrap();assert!(!cold.state.typewriter.enabled);assert_eq!(cold.state.random,random);
}

#[test]
fn graph_rollback_preflight_refusal_then_exact_setting_restore() {
    let p=inline_program("rollback","scene label start\n\"before\"\n\"after\"\n",&[("start",1,true)]);
    let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();
    e.advance_dialogue().unwrap();e.step_silent().unwrap();assert!(e.state.typewriter.enabled);
    let before=frozen(&e);let len=e.history.len();let batches=e.renderer.batches.lock().unwrap().len();
    e.renderer.reject.store(true,Ordering::SeqCst);assert!(!e.rollback());assert_eq!(frozen(&e),before);assert_eq!(e.history.len(),len);assert_eq!(e.renderer.batches.lock().unwrap().len(),batches);
    e.renderer.reject.store(false,Ordering::SeqCst);assert!(e.rollback());
    assert!(!e.state.typewriter.enabled);assert!(e.renderer.configs.lock().unwrap().is_empty());
}

#[test]
fn graph_debugger_has_actual_consumer_source_and_one_publication() {
    let p=inline_program("debug","scene label start\n\"boundary\"\n",&[("start",0,true)]);
    let mut e=Engine::from_program_with_debugger(p.clone(),Host::default(),8).unwrap();e.enter_label("start").unwrap();
    assert!(matches!(e.peek_statement(),Some(rvn_parser::Statement::ScheduledConsumer{body,..})if matches!(body.as_slice(),[rvn_parser::Statement::TypewriterSet{..}])));
    let random=e.state.random;let mut session=e.debug_begin(rvn_core::engine::DebugOperation::Statement,vec![]).unwrap();
    let snapshot=session.snapshot();assert_eq!(snapshot.position.pc,e.state.pc);
    let origin=snapshot.position.origin.as_ref().expect("actual emitted position origin, without a fabricated expression site");
    let span=origin.span.as_ref().expect("actual wrapper span");assert!(p.sources[0].source[span.clone()].contains("typewriter"));
    let mut done=false;for _ in 0..32 {if e.debug_step(&mut session,rvn_core::editor_debugger::StepKind::Continue).unwrap().0==rvn_core::engine::DebugProgress::Complete {done=true;break;}}
    assert!(done);assert!(e.state.typewriter.enabled);assert_eq!(e.state.random,random);assert_eq!(state_changes(&e.renderer),1);
}

#[test]
fn graph_init_eager_constructor_applies_actual_setting_once() {
    let p=inline_program("init-eager","init{set sampled=random(1,9)}\nscene label start\n\"boundary\"\n",&[("@init",0,true)]);
    let e=Engine::from_program(p,Host::default(),8).unwrap();assert!(e.state.typewriter.enabled);
    assert!(matches!(e.state.vars["sampled"],Value::Int(1..=9)));assert_eq!(state_changes(&e.renderer),1);
}

#[test]
fn graph_init_debugger_candidate_and_stale_plan_do_not_replay_effects() {
    let p=inline_program("init-debug","init{set sampled=random(1,9)}\nscene label start\n\"boundary\"\n",&[("@init",0,true)]);
    let host=Host::default();let mut owner=Engine::prepare_debugger_startup(p,host.clone(),8,Default::default(),Default::default(),None).unwrap();
    let initial_random=owner.state().random;let mut observed_change=false;
    for _ in 0..32 {
        if owner.is_complete(){break;}
        let before=serde_json::to_value(owner.state()).unwrap();let source=owner.observation_source().unwrap();
        let plan=owner.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();
        assert_eq!(serde_json::to_value(owner.state()).unwrap(),before);assert!(owner.accepts_observation_source(&source));
        let stale=owner.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();
        owner.begin_prepared_step_commit(plan).unwrap().commit();
        let committed=serde_json::to_value(owner.state()).unwrap();assert!(owner.begin_prepared_step_commit(stale).is_err());assert_eq!(serde_json::to_value(owner.state()).unwrap(),committed);
        if owner.state().typewriter.enabled {observed_change=true;}
    }
    assert!(owner.is_complete());assert!(observed_change);assert_eq!(owner.state().random,expected_draw(initial_random));assert_eq!(state_changes(&host),1);
    let mut engine=owner.into_engine().unwrap();let changes=state_changes(&host);engine.step_silent().unwrap();assert_eq!(state_changes(&host),changes);
}

#[test]
fn frozen_program_or_ast_forgery_refuses_before_init_publication() {
    let p=inline_program("forged","init{set prior=1}\nscene label start\n\"boundary\"\n",&[("start",0,true)]);
    let host=Host::default();let mut changed=p.clone();changed.sources[0].source=changed.sources[0].source.replacen("typewriter = true","typewriter = false",1);
    assert!(Engine::from_program(changed,host.clone(),8).is_err());assert_eq!(host.batches.lock().unwrap().len(),0);assert!(host.configs.lock().unwrap().is_empty());
    let mut changed=p;for statement in &mut changed.script {if let rvn_parser::Statement::ScheduledConsumer{body,..}=statement {if let [rvn_parser::Statement::TypewriterSet{enabled}]=body.as_mut_slice(){*enabled=false;}}}
    assert!(Engine::from_program(changed,host.clone(),8).is_err());assert_eq!(host.batches.lock().unwrap().len(),0);
}

#[test]
fn computation_and_host_declarations_are_not_typewriter_owners() {
    for definition in ["function hidden(){flow.consume_v2(\"scope\",\"consumer\"){typewriter=true}}", "handler hidden(event){flow.consume_v2(\"scope\",\"consumer\"){typewriter=true}}", "screen hidden(){flow.consume_v2(\"scope\",\"consumer\"){typewriter=true} return component(\"r\",\"column\",{},[])}"] {
        let raw=format!("init{{typewriter=true}}\n{definition}\nscene label start\n\"boundary\"\n");
        let dir=fixture_dir("context");let file=dir.join("main.rvn");std::fs::write(&file,&raw).unwrap();let host=Host::default();
        match rvn_parser::parse_file_program_with_uses(&file) {
            Ok(p)=>{let error=Engine::from_program(p,host.clone(),8).err().expect("declaration is not an owner").to_string();assert!(error.contains("actual Game/Scene or Init command site"),"{error}");},
            Err(error)=>assert!(error.contains("Some(Typewriter)")&&(error.contains("fonction de calcul")||error.contains("handler")),"{error}"),
        }
        assert!(host.batches.lock().unwrap().is_empty());assert!(host.configs.lock().unwrap().is_empty());
    }
}

#[test]
fn connected_static_boolean_and_speed_reach_core_without_source_replay() {
 let p=program("connected-static","scene label start\ntypewriter=true\ntypewriter.speed(20)\n\"boundary\"\n");
 let mut e=Engine::from_program(p,Host::default(),8).unwrap();let random=e.state.random;e.step_until_interaction().unwrap();
 assert!(e.state.typewriter.enabled);assert_eq!(e.state.typewriter.chars_per_sec,20);assert_eq!(e.state.random,random);
 assert_eq!(state_changes(&e.renderer),2);assert!(e.renderer.configs.lock().unwrap().is_empty());
 let before=frozen(&e);e.step_silent().unwrap();assert_eq!(frozen(&e),before);assert_eq!(state_changes(&e.renderer),2);
}
#[test]
fn computed_boolean_genuine_logic_not_reaches_the_actual_consumer() {
 let parsed=rvn_parser::parse_program("scene label start\ntypewriter=true\n\"boundary\"\n").unwrap();
 let mut graph=import_script(&parsed.script).unwrap().into_iter().find(|g|matches!(g.kind,GraphKind::Label{..})).unwrap();
 let command=graph.nodes.values().find(|n|n.kind==NodeKind::TypewriterSet).unwrap().id;
 let input=graph.pin_by_key(command,"enabled").unwrap().id;let old=graph.edges.values().find(|e|e.input==input).unwrap().clone();
 let literal=graph.pins[&old.output].node;graph.remove_edge(old.id);graph.remove_node(literal);
 let not=graph.add_catalog_node(NodeKind::LogicNot,[400.,300.]).unwrap();
 let output=graph.pins.values().find(|p|p.node==not&&p.direction==PinDirection::Output&&p.value_type==ValueType::Bool).unwrap().id;
 graph.connect(output,input).unwrap();graph.enable_native_frame_preparation().unwrap();
 let compiled=prepare_native_frame_graph(&graph).expect("real LogicNot Boolean has a genuine typed gather");
 let dir=fixture_dir("computed-not");let file=dir.join("main.rvn");std::fs::write(&file,&compiled.source).unwrap();
 let p=rvn_parser::parse_file_program_with_uses(&file).unwrap();assert_eq!(p.script,compiled.ast);
 let mut engine=Engine::from_program(p,Host::default(),8).unwrap();engine.step_until_interaction().unwrap();
 assert!(engine.state.typewriter.enabled);assert_eq!(state_changes(&engine.renderer),1);
}

#[test]
fn legacy_unwrapped_typewriter_callbacks_are_unchanged() {
    let p=direct_program("legacy","label start\ntypewriter=true\n\"on\"\ntypewriter=false\n\"off\"\n");
    let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();assert_eq!(*e.renderer.configs.lock().unwrap(),vec![40.]);
    e.advance_dialogue().unwrap();e.step_until_interaction().unwrap();assert_eq!(*e.renderer.configs.lock().unwrap(),vec![40.,0.]);
}


#[test]
fn typewriter0985_real_graph_random_comparison_uses_once_only_actual_typed_producers() {
 let p=program("computed-rng", "scene label start\ntypewriter=random(1,9)>4\n\"boundary\"\n");
 let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.state.random=rvn_core::random::RandomState::seeded(17);
 let mut expected=e.state.random;let enabled=expected.integer(1,9).unwrap()>4;
 e.step_until_interaction().unwrap();assert_eq!(e.state.typewriter.enabled,enabled);assert_eq!(e.state.random,expected);
 let before=frozen(&e);let count=e.renderer.batches.lock().unwrap().len();e.step_silent().unwrap();assert_eq!(frozen(&e),before);assert_eq!(e.renderer.batches.lock().unwrap().len(),count);
}
#[test]
fn typewriter0985_typed_and_renderer_refusals_keep_real_frame_rng_and_publication_atomic() {
 for scene in [false,true] {for expression in ["draw()","bad","bad_list","bad_number"] {
  let source=format!("function draw(){{local n=random(1,9) return n>4}}\ninit{{set bad=\"false\" set bad_list=[] set bad_number=0}}\n{}\"before\"\nflow.consume_v2(\"actual\",\"setting\"){{typewriter={expression}}}\n\"after\"\n",if scene{"scene label start\n"}else{"label start\n"});
  let p=direct_program("computed-refuse",&source);let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.state.random=rvn_core::random::RandomState::seeded(17);e.step_until_interaction().unwrap();e.advance_dialogue().unwrap();
  let before=frozen(&e);let witness=e.debug_state_source().unwrap();let count=e.renderer.batches.lock().unwrap().len();let callbacks=e.renderer.configs.lock().unwrap().clone();e.renderer.reject.store(expression=="draw()",Ordering::SeqCst);
  let error=e.step_silent().unwrap_err().to_string();assert!(error.contains(if expression=="draw()"{"typewriter fixture rejects candidate"}else{"Boolean"}),"{error}");
  assert_eq!(frozen(&e),before);assert!(e.accepts_debug_state_source(&witness));assert_eq!(e.renderer.batches.lock().unwrap().len(),count);assert_eq!(*e.renderer.configs.lock().unwrap(),callbacks);
  if expression=="draw()" {e.renderer.reject.store(false,Ordering::SeqCst);let mut expected=e.state.random;let enabled=expected.integer(1,9).unwrap()>4;e.step_silent().unwrap();assert_eq!(e.state.random,expected);assert_eq!(e.state.typewriter.enabled,enabled);}
 }}
}
// The visible "before" dialogue has its own real display seed. That is
// separate from the later, paused computation producer's one integer draw.
fn present_typewriter0985_before_dialogue(e:&mut Engine<Host>)->rvn_core::random::RandomState {
 let mut expected=e.state.random;let display=rvn_core::random::RandomState::seeded(expected.next_u64());
 e.step_until_interaction().unwrap();assert!(matches!(e.peek_statement(),Some(rvn_parser::Statement::Dialogue{..})));
 assert_eq!(e.state.random,expected);assert_eq!(e.state.display_random,display);assert_eq!(e.state.display_random_pc,Some(e.state.pc));
 e.advance_dialogue().unwrap();assert_eq!(e.state.random,expected);assert!(e.state.display_random_pc.is_none());expected
}
fn assert_typewriter0985_silent_boundary_has_no_replay(e:&mut Engine<Host>) {
 assert!(matches!(e.peek_statement(),Some(rvn_parser::Statement::Dialogue{..})));
 let before=frozen(e);let batches=e.renderer.batches.lock().unwrap().len();let callbacks=e.renderer.configs.lock().unwrap().clone();
 e.step_silent().unwrap();assert_eq!(frozen(e),before);assert_eq!(e.renderer.batches.lock().unwrap().len(),batches);assert_eq!(*e.renderer.configs.lock().unwrap(),callbacks);
}
// Startup has published its one real Typewriter command. The next silent step
// enters the actual first Scene, which legitimately publishes its own batch.
// Prove that batch is activation rather than another setting or RNG sample.
fn assert_typewriter0985_init_handoff_and_actual_scene_entry(e:&mut Engine<Host>,host:&Host) {
 let random=e.state.random;let setting=e.state.typewriter.clone();let changes=state_changes(host);
 let count=host.batches.lock().unwrap().len();assert_eq!(count,1,"only the Init command has published before narrative entry");
 assert!(e.state.scene_execution.as_ref().unwrap().active.is_none());
 e.step_silent().unwrap();assert!(e.state.scene_execution.as_ref().unwrap().active.is_some());
 assert!(matches!(e.peek_statement(),Some(rvn_parser::Statement::ScheduledConsumer{body,..})if matches!(body.as_slice(),[rvn_parser::Statement::Dialogue{..}])));
 assert_eq!(e.state.random,random);assert_eq!(e.state.typewriter,setting);assert_eq!(state_changes(host),changes);
 {
  let batches=host.batches.lock().unwrap();let activation=&batches[count..];assert_eq!(activation.len(),1);
  let batch=&activation[0];assert!(batch.previous.scene_execution.as_ref().unwrap().active.is_none());assert!(batch.state.scene_execution.as_ref().unwrap().active.is_some());
  assert_eq!(batch.previous.random,random);assert_eq!(batch.state.random,random);assert_eq!(batch.previous.typewriter,setting);assert_eq!(batch.state.typewriter,setting);assert!(batch.display.is_none());
 }
 let before=frozen(e);let count=host.batches.lock().unwrap().len();e.step_silent().unwrap();
 assert_eq!(frozen(e),before);assert_eq!(host.batches.lock().unwrap().len(),count);assert!(host.configs.lock().unwrap().is_empty());
}

fn finish_typewriter0985(e:&mut Engine<Host>,s:&mut rvn_core::engine::DebugSession) {
 for _ in 0..256 {if e.debug_step(s,rvn_core::editor_debugger::StepKind::Continue).unwrap().0==rvn_core::engine::DebugProgress::Complete{return;}}panic!("real input continuation bound");
}
#[test]
fn typewriter0985_paused_actual_input_roundtrips_save15_and_resumes_one_owned_gather() {
 use rvn_core::{engine::{DebugOperation,DebugProgress},editor_debugger::{StepKind,Route},save::consumer_checkpoint::Save15};
 for captured in [false,true] {
  let command=if captured{"flow.consume_v2(\"actual\",\"setting\"){capture \"actual-bool\"=draw() typewriter=__rvn_captured_v1(\"actual-bool\")}"}else{"typewriter=draw()"};
  let source=format!("function draw(){{local n=random(1,9) return n>4}}\nscene label start\n\"before\"\n{command}\n\"after\"\n");
  let p=direct_program("computed-pause",&source);let mut warm=Engine::from_program_with_debugger(p.clone(),Host::default(),8).unwrap();warm.state.random=rvn_core::random::RandomState::seeded(17);warm.enter_label("start").unwrap();let random_before_consumer=present_typewriter0985_before_dialogue(&mut warm);
  let mut entries=Vec::new();let mut offset=0;loop{let page=warm.debug_site_page(offset).unwrap();offset+=page.entries.len();entries.extend(page.entries);if offset==page.total{break;}}
  let point=entries.into_iter().find(|entry|entry.breakpoint.site.as_ref().is_some_and(|site|site.function.as_deref()==Some("draw")&&site.route==[Route::Statement(1)])).unwrap();assert!(point.origin.is_some());
  let before=frozen(&warm);let mut session=warm.debug_begin(DebugOperation::Statement,vec![point.breakpoint]).unwrap();assert_eq!(warm.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);assert_eq!(frozen(&warm),before);
  let save=warm.capture_save15(&p,Some(&session),98,"actual dynamic Boolean".into()).unwrap().encode().unwrap();finish_typewriter0985(&mut warm,&mut session);
  let mut cold=Engine::from_program_with_debugger(p.clone(),Host::default(),8).unwrap();let mut restored=cold.restore_save15(&p,Save15::decode(&save).unwrap()).unwrap().unwrap();finish_typewriter0985(&mut cold,&mut restored);
  assert_eq!(frozen(&cold),frozen(&warm));let mut expected=random_before_consumer;let enabled=expected.integer(1,9).unwrap()>4;assert_eq!(cold.state.random,expected);assert_eq!(cold.state.typewriter.enabled,enabled);assert_typewriter0985_silent_boundary_has_no_replay(&mut warm);assert_typewriter0985_silent_boundary_has_no_replay(&mut cold);
  let changed=direct_program("computed-changed",&source.replace("n>4","n>5"));let mut other=Engine::from_program_with_debugger(changed.clone(),Host::default(),8).unwrap();let before=frozen(&other);assert!(other.restore_save15(&changed,Save15::decode(&save).unwrap()).is_err());assert_eq!(frozen(&other),before);
 }
}
#[test]
fn typewriter0985_actual_init_preparation_stale_drop_and_one_boolean_effect_share_the_same_owner() {
 let p=program("computed-init","init{typewriter=random(1,9)>4}\nscene label start\n\"boundary\"\n");
 let host=Host::default();let mut owner=Engine::prepare_debugger_startup(p,host.clone(),8,Default::default(),Default::default(),None).unwrap();let mut expected=owner.state().random;let enabled=expected.integer(1,9).unwrap()>4;
 for _ in 0..64 {if owner.is_complete(){break;}let before=serde_json::to_value(owner.state()).unwrap();let drop_candidate=owner.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();drop(drop_candidate);assert_eq!(serde_json::to_value(owner.state()).unwrap(),before);let candidate=owner.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();let stale=owner.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();owner.begin_prepared_step_commit(candidate).unwrap().commit();assert!(owner.begin_prepared_step_commit(stale).is_err());}
 assert!(owner.is_complete());assert_eq!(owner.state().random,expected);assert_eq!(owner.state().typewriter.enabled,enabled);let mut e=owner.into_engine().unwrap();assert_typewriter0985_init_handoff_and_actual_scene_entry(&mut e,&host);
}
#[test]
fn typewriter0985_forged_program_and_unbound_capture_never_publish_a_boolean() {
 let p=program("computed-forged","scene label start\ntypewriter=not false\n\"boundary\"\n");let host=Host::default();let mut changed=p;changed.sources[0].source=changed.sources[0].source.replace("not ","");assert!(Engine::from_program(changed,host.clone(),8).is_err());assert!(host.batches.lock().unwrap().is_empty());assert!(host.configs.lock().unwrap().is_empty());
 let p=direct_program("computed-missing","flow.consume_v2(\"actual\",\"setting\"){typewriter=__rvn_captured_v1(\"absent\")}\n\"boundary\"\n");assert!(Engine::from_program(p,host.clone(),8).is_err());assert!(host.batches.lock().unwrap().is_empty());
}


#[test]
fn speed0993_real_graph_integer_gather_changes_speed_and_rng_once_in_game_and_scene() {
 for scene in [false,true] {
  let source=format!("{}typewriter.speed(random(1,9))\n\"boundary\"\n",if scene{"scene label start\n"}else{"label start\n"});
  let p=program("speed-computed-rng",&source);let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.state.random=rvn_core::random::RandomState::seeded(17);let mut expected=e.state.random;let speed=expected.integer(1,9).unwrap()as u32;
  e.step_until_interaction().unwrap();assert_eq!(e.state.typewriter.chars_per_sec,speed);assert!(e.state.typewriter.enabled);assert_eq!(e.state.random,expected);
  if scene{assert_eq!(state_changes(&e.renderer),1);assert!(e.renderer.configs.lock().unwrap().is_empty());}else{assert_eq!(*e.renderer.configs.lock().unwrap(),vec![speed as f32]);}
  let before=frozen(&e);let count=e.renderer.batches.lock().unwrap().len();let callbacks=e.renderer.configs.lock().unwrap().clone();e.step_silent().unwrap();assert_eq!(frozen(&e),before);assert_eq!(e.renderer.batches.lock().unwrap().len(),count);assert_eq!(*e.renderer.configs.lock().unwrap(),callbacks);
 }
}
#[test]
fn speed0993_strict_integer_range_and_renderer_failures_keep_candidate_rng_frame_and_effects_atomic() {
 for scene in [false,true] {for value in ["-1","4294967296","0.5","true","\"30\"","[]","random(1,9)"] {
  let source=format!("function draw(){{local n=random(1,9) return {value}}}\n{}\"before\"\nflow.consume_v2(\"actual\",\"speed\"){{typewriter.speed(draw())}}\n\"after\"\n",if scene{"scene label start\n"}else{"label start\n"});
  let p=direct_program("speed-refuse",&source);let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.state.random=rvn_core::random::RandomState::seeded(17);e.step_until_interaction().unwrap();e.advance_dialogue().unwrap();let before=frozen(&e);let witness=e.debug_state_source().unwrap();let count=e.renderer.batches.lock().unwrap().len();let callbacks=e.renderer.configs.lock().unwrap().clone();e.renderer.reject.store(value=="random(1,9)",Ordering::SeqCst);
  let error=e.step_silent().unwrap_err().to_string();assert!(error.contains(if value=="random(1,9)"{"typewriter fixture rejects candidate"}else{"integer in 0..=4294967295"}),"{error}");assert_eq!(frozen(&e),before);assert!(e.accepts_debug_state_source(&witness));assert_eq!(e.renderer.batches.lock().unwrap().len(),count);assert_eq!(*e.renderer.configs.lock().unwrap(),callbacks);
  if value=="random(1,9)"{e.renderer.reject.store(false,Ordering::SeqCst);let mut expected=e.state.random;expected.integer(1,9).unwrap();let speed=expected.integer(1,9).unwrap()as u32;e.step_silent().unwrap();assert_eq!(e.state.random,expected);assert_eq!(e.state.typewriter.chars_per_sec,speed);}
 }}
}
#[test]
fn speed0993_zero_disables_and_unsigned_max_is_preserved_without_clamp_or_coercion() {
 for scene in [false,true] {for speed in [0,u32::MAX] {
  let source=format!("init{{set rate={speed}}}\n{}typewriter.speed(rate)\n\"boundary\"\n",if scene{"scene label start\n"}else{"label start\n"});let p=direct_program("speed-bounds",&source);let mut e=Engine::from_program(p,Host::default(),8).unwrap();e.step_until_interaction().unwrap();assert_eq!(e.state.typewriter.chars_per_sec,speed);assert_eq!(e.state.typewriter.enabled,speed>0);
  if !scene{assert_eq!(*e.renderer.configs.lock().unwrap(),vec![speed as f32]);}
 }}
}
#[test]
fn speed0993_actual_paused_input_roundtrips_save15_and_resumes_one_owned_integer_gather() {
 use rvn_core::{engine::{DebugOperation,DebugProgress},editor_debugger::{StepKind,Route},save::consumer_checkpoint::Save15};
 for captured in [false,true] {
  let command=if captured{"flow.consume_v2(\"actual\",\"speed\"){capture \"actual-speed\"=draw() typewriter.speed(__rvn_captured_v1(\"actual-speed\"))}"}else{"typewriter.speed(draw())"};
  let source=format!("function draw(){{local n=random(1,9) return n}}\nscene label start\n\"before\"\n{command}\n\"after\"\n");let p=direct_program("speed-pause",&source);let mut warm=Engine::from_program_with_debugger(p.clone(),Host::default(),8).unwrap();warm.state.random=rvn_core::random::RandomState::seeded(17);warm.enter_label("start").unwrap();let random_before_consumer=present_typewriter0985_before_dialogue(&mut warm);let mut entries=Vec::new();let mut offset=0;loop{let page=warm.debug_site_page(offset).unwrap();offset+=page.entries.len();entries.extend(page.entries);if offset==page.total{break;}}
  let point=entries.into_iter().find(|e|e.breakpoint.site.as_ref().is_some_and(|site|site.function.as_deref()==Some("draw")&&site.route==[Route::Statement(1)])).unwrap();assert!(point.origin.is_some());let before=frozen(&warm);let mut session=warm.debug_begin(DebugOperation::Statement,vec![point.breakpoint]).unwrap();assert_eq!(warm.debug_step(&mut session,StepKind::Continue).unwrap().0,DebugProgress::Stopped);assert_eq!(frozen(&warm),before);
  let saved=warm.capture_save15(&p,Some(&session),99,"actual dynamic integer speed".into()).unwrap().encode().unwrap();finish_typewriter0985(&mut warm,&mut session);let mut cold=Engine::from_program_with_debugger(p.clone(),Host::default(),8).unwrap();let mut restored=cold.restore_save15(&p,Save15::decode(&saved).unwrap()).unwrap().unwrap();finish_typewriter0985(&mut cold,&mut restored);assert_eq!(frozen(&cold),frozen(&warm));let mut expected=random_before_consumer;let speed=expected.integer(1,9).unwrap()as u32;assert_eq!(cold.state.random,expected);assert_eq!(cold.state.typewriter.chars_per_sec,speed);assert_typewriter0985_silent_boundary_has_no_replay(&mut warm);assert_typewriter0985_silent_boundary_has_no_replay(&mut cold);
  let changed=direct_program("speed-changed",&source.replace("return n}","return n+1}"));let mut other=Engine::from_program_with_debugger(changed.clone(),Host::default(),8).unwrap();let before=frozen(&other);assert!(other.restore_save15(&changed,Save15::decode(&saved).unwrap()).is_err());assert_eq!(frozen(&other),before);
 }
}
#[test]
fn speed0993_actual_init_drop_stale_commit_and_one_integer_effect_share_the_same_owner() {
 let p=program("speed-init","init{typewriter.speed(random(1,9))}\nscene label start\n\"boundary\"\n");let host=Host::default();let mut owner=Engine::prepare_debugger_startup(p,host.clone(),8,Default::default(),Default::default(),None).unwrap();let mut expected=owner.state().random;let speed=expected.integer(1,9).unwrap()as u32;
 for _ in 0..64{if owner.is_complete(){break;}let before=serde_json::to_value(owner.state()).unwrap();let dropped=owner.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();drop(dropped);assert_eq!(serde_json::to_value(owner.state()).unwrap(),before);let candidate=owner.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();let stale=owner.prepare_step(rvn_core::editor_debugger::StepKind::Continue).unwrap();owner.begin_prepared_step_commit(candidate).unwrap().commit();assert!(owner.begin_prepared_step_commit(stale).is_err());}
 assert!(owner.is_complete());assert_eq!(owner.state().random,expected);assert_eq!(owner.state().typewriter.chars_per_sec,speed);let mut e=owner.into_engine().unwrap();assert_typewriter0985_init_handoff_and_actual_scene_entry(&mut e,&host);
}
