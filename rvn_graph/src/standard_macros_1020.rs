//! Catalogue/source bridge for the four typed macros. Runtime cells stay in
//! Core's actual Widget owner. No Graph-owned execution instance is created.
use crate::*;
/// Local feature contract; existing source/signature containers own schemas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StandardMacroContract {
    WidgetInstanceV1,
}
impl GraphDocument {
    pub fn validate_standard_macro_contracts(&self) -> Result<(), String> {
        for node in self.nodes.values() {
            match (kind(node.kind), node.standard_macro_contract) {
                (None, None) => continue,
                (None, Some(_)) => return Err("StandardMacro contract on an unrelated node".into()),
                (Some(_), None) => {
                    return Err(
                        "StandardMacro node requires explicit WidgetInstanceV1 contract".into(),
                    )
                }
                (Some(_), Some(StandardMacroContract::WidgetInstanceV1)) => {}
            }
            if !matches!(self.kind, GraphKind::Handler { .. }) {
                return Err(
                    "WidgetInstanceV1 first slice requires an actual Widget Handler graph".into(),
                );
            }
            if self.inspector_evaluation != InspectorEvaluationContract::NativeFrameV1
                || ![
                    GRAPH_MIXED_INSPECTOR_SCHEMA_VERSION,
                    GRAPH_NATIVE_LOOP_DEMAND_SCHEMA_VERSION,
                ]
                .contains(&self.schema_version)
            {
                return Err("WidgetInstanceV1 requires explicit compatible NativeFrame source capability; Unit/signature combination remains unadmitted".into());
            }
        }
        Ok(())
    }
}
use rvn_parser::scheduled_source::standard_macro_contract::{Control, Kind, Site};
pub(crate) fn kind(node: NodeKind) -> Option<Kind> {
    Some(match node {
        NodeKind::DoOnce => Kind::DoOnce,
        NodeKind::DoN => Kind::DoN,
        NodeKind::FlipFlop => Kind::FlipFlop,
        NodeKind::Gate => Kind::Gate,
        _ => return None,
    })
}
fn input(
    key: &'static str,
    label: &'static str,
    value_type: ValueType,
    default_value: Option<PropertyValue>,
) -> PinDefinition {
    let cardinality = if value_type == ValueType::Execution {
        PinCardinality::Many
    } else {
        PinCardinality::One
    };
    PinDefinition {
        key,
        label,
        direction: PinDirection::Input,
        value_type,
        cardinality,
        default_value,
    }
}
fn output(key: &'static str, label: &'static str, value_type: ValueType) -> PinDefinition {
    let cardinality = if value_type == ValueType::Execution {
        PinCardinality::One
    } else {
        PinCardinality::Many
    };
    PinDefinition {
        key,
        label,
        direction: PinDirection::Output,
        value_type,
        cardinality,
        default_value: None,
    }
}
pub(crate) fn pins(node: NodeKind) -> Vec<PinDefinition> {
    use PropertyValue as V;
    use ValueType as T;
    match kind(node).expect("called only for the four catalogue variants") {
        Kind::DoOnce => vec![
            input("exec_in", "", T::Execution, None),
            input("reset", "Reset", T::Execution, None),
            input(
                "start_closed",
                "Start Closed",
                T::Bool,
                Some(V::Bool(false)),
            ),
            output("completed", "Completed", T::Execution),
        ],
        Kind::DoN => vec![
            input("exec_in", "Enter", T::Execution, None),
            input("n", "N", T::Int, Some(V::Int(0))),
            input("reset", "Reset", T::Execution, None),
            output("exit", "Exit", T::Execution),
            output("counter", "Counter", T::Int),
        ],
        Kind::FlipFlop => vec![
            input("exec_in", "", T::Execution, None),
            output("a", "A", T::Execution),
            output("b", "B", T::Execution),
            output("is_a", "Is A", T::Bool),
        ],
        Kind::Gate => vec![
            input("exec_in", "Enter", T::Execution, None),
            input("open", "Open", T::Execution, None),
            input("close", "Close", T::Execution, None),
            input("toggle", "Toggle", T::Execution, None),
            input("start_closed", "Start Closed", T::Bool, Some(V::Bool(true))),
            output("exit", "Exit", T::Execution),
        ],
    }
}
pub(crate) fn control(
    graph: &GraphDocument,
    node: NodeId,
    input: PinId,
) -> Result<Control, String> {
    let owner = graph.nodes.get(&node).ok_or("Macro node missing")?;
    let family = kind(owner.kind).ok_or("Execution input does not enter a macro")?;
    let pin = graph
        .pins
        .get(&input)
        .ok_or("Macro entered PinId missing")?;
    if pin.node != node
        || !owner.pins.contains(&input)
        || pin.direction != PinDirection::Input
        || !graph.resolved_pin_is_execution(input)
    {
        return Err("Macro control must be its actual reached execution input".into());
    }
    let result = match pin.key.as_str() {
        "exec_in" => Control::Enter,
        "reset" => Control::Reset,
        "open" => Control::Open,
        "close" => Control::Close,
        "toggle" => Control::Toggle,
        _ => return Err("Unknown macro execution input role".into()),
    };
    family.control_role(result)?;
    Ok(result)
}
pub(crate) fn source_identity(
    graph: &GraphDocument,
    node: NodeId,
) -> Result<(String, String), String> {
    graph.validate_standard_macro_contracts()?;
    if !matches!(graph.kind, GraphKind::Handler { .. }) {
        return Err("StandardMacro initial slice requires an actual Widget Handler graph".into());
    }
    let roots = graph
        .nodes
        .values()
        .filter(|node| node.kind == NodeKind::HandlerEntry)
        .collect::<Vec<_>>();
    let [root] = roots.as_slice() else {
        return Err("Macro source needs exactly one actual Handler entry".into());
    };
    let scope = format!(
        "g{}/{}",
        graph.graph_id,
        crate::native_frame_preparation::encode_node(graph, root.id)?
    );
    let node = crate::native_frame_preparation::encode_node(graph, node)?;
    Site::decode(&scope, &node)?;
    Ok((scope, node))
}
pub(crate) fn control_source(control: Control) -> &'static str {
    match control {
        Control::Enter => "enter",
        Control::Reset => "reset",
        Control::Open => "open",
        Control::Close => "close",
        Control::Toggle => "toggle",
    }
}
pub(crate) fn data_source(graph: &GraphDocument, output: PinId) -> Result<Option<String>, String> {
    let pin = graph.pins.get(&output).ok_or("Macro data output missing")?;
    let owner = graph
        .nodes
        .get(&pin.node)
        .ok_or("Macro data node missing")?;
    let Some(family) = kind(owner.kind) else {
        return Ok(None);
    };
    let method = match (family, pin.key.as_str()) {
        (Kind::DoN, "counter") => "flow.macro_counter_v1",
        (Kind::FlipFlop, "is_a") => "flow.macro_is_a_v1",
        _ => return Err("Macro node has no such data output".into()),
    };
    if pin.direction != PinDirection::Output
        || graph.resolved_pin_is_execution(output)
        || !owner.pins.contains(&output)
    {
        return Err("Macro live read must retain its actual data output pin".into());
    }
    let (scope, node) = source_identity(graph, owner.id)?;
    Ok(Some(format!(
        "{method}({}, {})",
        serde_json::to_string(&scope).map_err(|e| e.to_string())?,
        serde_json::to_string(&node).map_err(|e| e.to_string())?
    )))
}
/// Admit only real macro control fan-in for the first Widget Handler source
/// slice. The existing codec proves all endpoints again on full reconstruction.
/// This neither enables arbitrary shared consumers nor grants a runtime owner.
pub(crate) fn validate_shared_control_ingress(graph: &GraphDocument) -> Result<(), String> {
    graph.validate_standard_macro_contracts()?;
    if !matches!(graph.kind, GraphKind::Handler { .. }) {
        return Err("Macro ExecLinks requires its actual Widget Handler graph".into());
    }
    let mut shared = false;
    for input in graph.pins.values() {
        let links = graph
            .edges
            .values()
            .filter(|edge| edge.input == input.id)
            .collect::<Vec<_>>();
        if links.len() < 2 {
            continue;
        }
        let owner = graph
            .nodes
            .get(&input.node)
            .ok_or("Macro ExecLinks input owner missing")?;
        let family =
            kind(owner.kind).ok_or("Macro ExecLinks does not admit a shared non-macro input")?;
        let known = node_definition(owner.kind)
            .pins
            .into_iter()
            .find(|pin| pin.key == input.key)
            .ok_or("Macro ExecLinks input role is absent from its actual catalogue")?;
        if !owner.pins.contains(&input.id)
            || input.direction != PinDirection::Input
            || known.direction != input.direction
            || input.value_type != ValueType::Execution
            || known.value_type != input.value_type
            || input.cardinality != PinCardinality::Many
            || known.cardinality != input.cardinality
        {
            return Err("Macro ExecLinks requires its actual Many execution input".into());
        }
        let control = match input.key.as_str() {
            "exec_in" => Control::Enter,
            "reset" => Control::Reset,
            "open" => Control::Open,
            "close" => Control::Close,
            "toggle" => Control::Toggle,
            _ => return Err("Macro ExecLinks input is not an actual macro control".into()),
        };
        family.control_role(control)?;
        for edge in links {
            let output = graph
                .pins
                .get(&edge.output)
                .ok_or("Macro ExecLinks predecessor output missing")?;
            let predecessor = graph
                .nodes
                .get(&output.node)
                .ok_or("Macro ExecLinks predecessor owner missing")?;
            // Output cardinality is catalogue capacity, not its actual degree.
            // Historical Sequence outputs are Many; resized outputs can be One.
            // As in codegen::successor, exactly one real outgoing edge is required.
            if predecessor.id == owner.id
                || !predecessor.pins.contains(&output.id)
                || output.direction != PinDirection::Output
                || !graph.resolved_pin_is_execution(output.id)
                || graph
                    .edges
                    .values()
                    .filter(|candidate| candidate.output == output.id)
                    .count()
                    != 1
            {
                return Err("Macro ExecLinks predecessor must be one actual distinct Exec output with one successor".into());
            }
        }
        shared = true;
    }
    if !shared {
        return Err(
            "ExecLinks without a demand Range requires actual shared macro control ingress".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod exec_degree_tests1021 {
    use super::*;

    fn shared_graph(kind: NodeKind) -> (GraphDocument, NodeId) {
        let tail = if kind==NodeKind::FlipFlop { "set hits=hits+1 set branch_b=branch_b+1" } else { "set hits=hits+1" };
        let source = format!("handler allowed(event){{flow.sequence(){{{{set ingress_a=0}}{{set ingress_b=0}}}} {tail}}}");
        let mut graph = crate::import_script(&rvn_parser::parse(&source).unwrap()).unwrap().into_iter()
            .find(|graph| matches!(&graph.kind,GraphKind::Handler{name} if name=="allowed")).unwrap();
        let assignment = |graph:&GraphDocument,name:&str| {
            let nodes = graph.nodes.values().filter(|node| node.kind==NodeKind::SetVariable
                && node.properties.get("name")==Some(&PropertyValue::String(name.into()))).map(|node|node.id).collect::<Vec<_>>();
            assert_eq!(nodes.len(),1);nodes[0]
        };
        let ends = graph.nodes.values().filter(|node|node.kind==NodeKind::BranchEnd).map(|node|node.id).collect::<Vec<_>>();
        for end in ends { graph.remove_node(end).unwrap(); }
        let node = graph.add_catalog_node(kind,[500.0,0.0]).unwrap();
        if kind==NodeKind::DoN { graph.set_pin_default(node,"n",PropertyValue::Int(3)).unwrap(); }
        if matches!(kind,NodeKind::DoOnce|NodeKind::Gate) { graph.set_pin_default(node,"start_closed",PropertyValue::Bool(false)).unwrap(); }
        let enter = graph.pin_by_key(node,"exec_in").unwrap().id;
        for name in ["ingress_a","ingress_b"] {
            let marker = assignment(&graph,name);
            let input = graph.pin_by_key(marker,"exec_in").unwrap().id;
            let incoming = graph.edges.values().filter(|edge|edge.input==input).cloned().collect::<Vec<_>>();
            assert_eq!(incoming.len(),1);
            graph.remove_node(marker).unwrap();
            graph.connect(incoming[0].output,enter).unwrap();
        }
        let hits = assignment(&graph,"hits");
        let hit_input = graph.pin_by_key(hits,"exec_in").unwrap().id;
        let old = graph.edges.values().find(|edge|edge.input==hit_input).unwrap().id;
        graph.remove_edge(old).unwrap();
        let output = match kind {NodeKind::DoOnce=>"completed",NodeKind::FlipFlop=>"a",_=>"exit"};
        graph.connect(graph.pin_by_key(node,output).unwrap().id,hit_input).unwrap();
        if kind==NodeKind::FlipFlop {
            let branch_b = assignment(&graph,"branch_b");
            let input = graph.pin_by_key(branch_b,"exec_in").unwrap().id;
            let old = graph.edges.values().find(|edge|edge.input==input).unwrap().id;
            graph.remove_edge(old).unwrap();
            graph.connect(graph.pin_by_key(node,"b").unwrap().id,input).unwrap();
        }
        graph.enable_native_frame_preparation().unwrap();
        (graph,node)
    }

    #[test]
    fn actual_many_exec_outputs_admit_one_successor_but_refuse_fanout_and_macro_self_loop() {
        for kind in [NodeKind::DoOnce,NodeKind::DoN,NodeKind::FlipFlop,NodeKind::Gate] {
            let (graph,node) = shared_graph(kind);
            let enter = graph.pin_by_key(node,"exec_in").unwrap().id;
            let incoming = graph.edges.values().filter(|edge|edge.input==enter).collect::<Vec<_>>();
            assert_eq!(incoming.len(),2);
            for edge in &incoming {
                let output = &graph.pins[&edge.output];
                assert_eq!(graph.nodes[&output.node].kind,NodeKind::Sequence);
                assert_eq!(output.cardinality,PinCardinality::Many,"retain the historical catalogue");
                assert_eq!(graph.edges.values().filter(|other|other.output==output.id).count(),1);
            }
            let predecessor = incoming[0].output;
            let mut fanout = graph.clone();
            let second = fanout.add_catalog_node(kind,[800.0,200.0]).unwrap();
            let second_enter = fanout.pin_by_key(second,"exec_in").unwrap().id;
            fanout.connect(predecessor,second_enter).unwrap();
            assert_eq!(fanout.edges.values().filter(|edge|edge.output==predecessor).count(),2);
            let before = serde_json::to_vec(&fanout).unwrap();
            let error = fanout.enable_exec_links_source_preparation().unwrap_err();
            assert!(error.contains("one actual distinct Exec output with one successor"),"{kind:?}: {error}");
            assert_eq!(serde_json::to_vec(&fanout).unwrap(),before,"fan-out refuses before mutation");

            let mut self_loop = graph.clone();
            let output_key = match kind {NodeKind::DoOnce=>"completed",NodeKind::FlipFlop=>"a",_=>"exit"};
            let output = self_loop.pin_by_key(node,output_key).unwrap().id;
            let old = self_loop.edges.values().find(|edge|edge.output==output).unwrap().id;
            self_loop.remove_edge(old).unwrap();
            self_loop.connect(output,enter).unwrap();
            assert_eq!(self_loop.edges.values().filter(|edge|edge.output==output).count(),1);
            let before = serde_json::to_vec(&self_loop).unwrap();
            let error = self_loop.enable_exec_links_source_preparation().unwrap_err();
            assert!(error.contains("one actual distinct Exec output with one successor"),"{kind:?}: {error}");
            assert_eq!(serde_json::to_vec(&self_loop).unwrap(),before,"direct self-loop refuses before mutation");

            let mut prepared = graph.clone();
            let before_schema = graph.schema_version;
            prepared.enable_exec_links_source_preparation().unwrap();
            assert_eq!(prepared.schema_version,before_schema);
            assert_eq!(prepared.pins,graph.pins);
            assert_eq!(prepared.edges,graph.edges);
            let generated = crate::prepare_native_frame_graph(&prepared).unwrap();
            assert!(generated.source.contains("!exec_links_v1"));
            assert!(generated.source.contains("flow.macro_dispatch_v1("));
        }
    }

    #[test]
    fn native_source1021_repeated_connected_macro_operand_keeps_one_actual_wire_and_shared_dag() {
        for kind in [NodeKind::DoOnce,NodeKind::DoN,NodeKind::Gate] {
            for shared in [false,true] {
                let raw = if kind==NodeKind::DoN {
                    if shared {"handler allowed(event){set probe=random(1,1)+random(1,1)}"}
                    else {"handler allowed(event){set probe=random(1,1)+2}"}
                } else { "handler allowed(event){set probe=random(1,1)==1}" };
                let mut graph=crate::import_script(&rvn_parser::parse(raw).unwrap()).unwrap().into_iter()
                    .find(|g|matches!(g.kind,GraphKind::Handler{..})).unwrap();
                let probe=graph.nodes.values().find(|n|n.kind==NodeKind::SetVariable).unwrap().id;
                let input=graph.pin_by_key(probe,"value").unwrap().id;
                let producer=graph.edges.values().find(|e|e.input==input).unwrap().output;
                graph.remove_node(probe).unwrap();
                if kind==NodeKind::DoN && shared {
                    let sum=graph.pins[&producer].node;
                    let left=graph.pin_by_key(sum,"left").unwrap().id;
                    let right=graph.pin_by_key(sum,"right").unwrap().id;
                    let random=graph.edges.values().find(|e|e.input==left).unwrap().output;
                    let literal=graph.edges.values().find(|e|e.input==right).map(|e|graph.pins[&e.output].node).unwrap();
                    graph.remove_node(literal).unwrap();graph.connect(random,right).unwrap();
                }
                let root=graph.nodes.values().find(|n|n.kind==NodeKind::HandlerEntry).unwrap().id;
                let choose=graph.add_catalog_node(NodeKind::If,[300.,0.]).unwrap();
                let macro_node=graph.add_catalog_node(kind,[700.,0.]).unwrap();
                let reroute=graph.add_catalog_node(NodeKind::Reroute,[300.,200.]).unwrap();
                let wire=|g:&mut GraphDocument,a:NodeId,ak:&str,b:NodeId,bk:&str|{
                    g.connect(g.pin_by_key(a,ak).unwrap().id,g.pin_by_key(b,bk).unwrap().id).unwrap();
                };
                graph.connect(producer,graph.pin_by_key(reroute,"value").unwrap().id).unwrap();
                wire(&mut graph,reroute,"value_out",macro_node,if kind==NodeKind::DoN{"n"}else{"start_closed"});
                wire(&mut graph,root,"exec_out",choose,"exec_in");
                wire(&mut graph,choose,"then",macro_node,if kind==NodeKind::Gate{"open"}else{"reset"});
                if shared {
                    let sequence=graph.add_catalog_node(NodeKind::Sequence,[500.,0.]).unwrap();
                    wire(&mut graph,choose,"else",sequence,"exec_in");
                    wire(&mut graph,sequence,"then_0",macro_node,"exec_in");
                    wire(&mut graph,sequence,"then_1",macro_node,"exec_in");
                } else {wire(&mut graph,choose,"else",macro_node,"exec_in");}
                graph.enable_native_frame_preparation().unwrap();
                if shared {graph.enable_exec_links_source_preparation().unwrap();}
                let before=graph.clone();
                let generated=crate::prepare_native_frame_graph(&graph).unwrap();
                assert!(generated.source.contains("operand_v1 {"));
                let restored=crate::import_native_frame_source(&generated.source,rvn_parser::NarrativeContext::Legacy).unwrap();
                let actual=restored.iter().find(|g|g.graph_id==graph.graph_id).unwrap();
                assert_eq!(actual.edges,graph.edges,"{kind:?} shared={shared}");
                assert!(actual.nodes.keys().eq(graph.nodes.keys()));assert!(actual.pins.keys().eq(graph.pins.keys()));
                let operand=actual.pin_by_key(macro_node,if kind==NodeKind::DoN{"n"}else{"start_closed"}).unwrap().id;
                assert_eq!(actual.edges.values().filter(|e|e.input==operand).count(),1);
                assert_eq!(crate::prepare_native_frame_graph(actual).unwrap().ast,generated.ast);
                assert_eq!(graph,before);
            }
        }
    }

    #[test]
    fn native_source1021_nonempty_branches_preserve_actual_end_presence_instead_of_synthesizing() {
        for source in [
            "scene label start\nif true {set score=1}else{set score=2}\nreturn\n",
            "scene label start\nflow.sequence(){{set score=1}{set score=2}}\nreturn\n",
            "scene label start\nwhile false {set score=1}\nreturn\n",
            "scene label start\nfor item in [1,2] {set score=item}\nreturn\n",
            "scene label start\nflow.for_loop(i,1,2) {set score=i}\nreturn\n",
            "scene label start\nchoice {\"Real\" => {set score=1}}\nreturn\n",
            "scene label start\nimagemap {background: \"map.png\" hotspot {name: \"real\" area: (0,0,100,100)} => {set score=1}}\nreturn\n",
        ] {
            let original=crate::import_script(&rvn_parser::parse(source).unwrap()).unwrap().into_iter()
                .find(|g|matches!(g.kind,GraphKind::Label{..})).unwrap();
            assert!(original.nodes.values().any(|n|n.kind==NodeKind::BranchEnd));
            for keep_end in [false,true] {
                let mut graph=original.clone();
                if !keep_end {
                    let ends=graph.nodes.values().filter(|n|n.kind==NodeKind::BranchEnd).map(|n|n.id).collect::<Vec<_>>();
                    for end in ends {graph.remove_node(end).unwrap();}
                }
                graph.enable_native_frame_preparation().unwrap();
                let before=graph.clone();let generated=crate::prepare_native_frame_graph(&graph).unwrap();
                let restored=crate::import_native_frame_source(&generated.source,rvn_parser::NarrativeContext::ExplicitScenes).unwrap();
                let actual=restored.iter().find(|g|g.graph_id==graph.graph_id).unwrap();
                assert_eq!(actual.edges,graph.edges,"source={source} keep_end={keep_end}");
                assert!(actual.nodes.keys().eq(graph.nodes.keys()));assert!(actual.pins.keys().eq(graph.pins.keys()));
                assert_eq!(actual.nodes.values().filter(|n|n.kind==NodeKind::BranchEnd).count(),graph.nodes.values().filter(|n|n.kind==NodeKind::BranchEnd).count());
                assert_eq!(crate::prepare_native_frame_graph(actual).unwrap().ast,generated.ast);
                assert_eq!(graph,before);
            }
        }
    }

}
