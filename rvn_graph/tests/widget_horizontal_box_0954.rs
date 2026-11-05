//! Real Graph/RVN writer transactions. Prepared only, no native claim.
use rvn_graph::*;
use rvn_ui::programmable::{
    Alignment, BoxSlotSize, Component, ComponentKind, ParentSlot, SlotEdges,
};
const SOURCE: &str = r#"// Retain this authored HorizontalBox comment.
screen form(title) {
 return component("root","row",{"layout_contract":"widget_v1"},[
  component("left","column",{"parent_slot":{"kind":"horizontal_box"}},[
   component("moving","text",{"text":title,"parent_slot":{"kind":"vertical_box"}},[])
  ]),
  component("right","row",{"parent_slot":{"kind":"horizontal_box"}},[]),
  component("button","button",{"parent_slot":{"kind":"horizontal_box"}},[])
 ])
}
"#;
fn graph(source: &str) -> GraphDocument {
    import_script(&rvn_parser::parse(source).unwrap())
        .unwrap()
        .into_iter()
        .find(|g| matches!(g.kind, GraphKind::Screen { .. }))
        .unwrap()
}
fn returned(g: &GraphDocument) -> NodeId {
    g.nodes
        .values()
        .find(|n| n.kind == NodeKind::FunctionReturn)
        .unwrap()
        .id
}
fn evaluate(g: &GraphDocument) -> Component {
    let generated = transpile(g).unwrap();
    let library = rvn_core::eval::FunctionLibrary::from_script(&generated.ast).unwrap();
    let (parameters, body) = generated
        .ast
        .iter()
        .find_map(|s| match s {
            rvn_parser::Statement::Screen {
                parameters, body, ..
            } => Some((parameters, body)),
            _ => None,
        })
        .unwrap();
    let value = library
        .describe_screen(
            parameters,
            body,
            &[rvn_parser::Value::Str("Dynamic title".into())],
            &Default::default(),
            &mut rvn_core::random::RandomState::default(),
        )
        .unwrap();
    Component::parse(rvn_core::ui::value_to_json(&value).unwrap()).unwrap()
}
fn horizontal(weight: f32) -> ParentSlot {
    ParentSlot::HorizontalBox {
        padding: SlotEdges::uniform(3.0),
        horizontal: Alignment::End,
        vertical: Alignment::Center,
        size: BoxSlotSize::Fill { weight },
    }
}
struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "rvn-row-0954-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let p = self.0.canonicalize().unwrap();
        let t = std::env::temp_dir().canonicalize().unwrap();
        assert_eq!(p.parent(), Some(t.as_path()));
        assert!(p
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("rvn-row-0954-"));
        std::fs::remove_dir_all(p).unwrap();
    }
}
#[test]
fn horizontal0954_slot_edit_saves_through_existing_workspace_writer_and_reopens_stable_ids() {
    let temp = Temp::new();
    std::fs::write(temp.0.join("main.rvn"), SOURCE).unwrap();
    let mut workspace = SourceWorkspace::link(&temp.0, &temp.0.join("main.rvn"), &[]).unwrap();
    let mut graphs = workspace.project().graphs().to_vec();
    let g = graphs
        .iter_mut()
        .find(|g| matches!(g.kind, GraphKind::Screen { .. }))
        .unwrap();
    let ids = g.interface_component_ids();
    let ret = returned(g);
    g.interface_widget_set_parent_slot(ret, ids["right"], horizontal(2.0))
        .unwrap();
    let expected = evaluate(g);
    let gid = g.graph_id;
    workspace.save(&graphs).unwrap();
    assert!(std::fs::read_to_string(temp.0.join("main.rvn"))
        .unwrap()
        .contains("// Retain this authored HorizontalBox comment."));
    let reopened = SourceWorkspace::open(&temp.0).unwrap().unwrap();
    let saved = reopened
        .project()
        .graphs()
        .iter()
        .find(|g| g.graph_id == gid)
        .unwrap();
    assert_eq!(saved.interface_component_ids(), ids);
    assert_eq!(evaluate(saved), expected);
    assert_eq!(
        expected.find("right").unwrap().parent_slot,
        Some(horizontal(2.0))
    );
}
#[test]
fn horizontal0954_reparent_between_both_axes_and_button_retains_dynamic_source_and_component_ids() {
    let mut g = graph(SOURCE);
    let ids = g.interface_component_ids();
    let ret = returned(&g);
    g.interface_widget_reparent(ret, ids["moving"], ids["right"])
        .unwrap();
    assert_eq!(g.interface_component_ids(), ids);
    assert_eq!(
        evaluate(&g).find("moving").unwrap().parent_slot,
        Some(ParentSlot::for_parent(ComponentKind::Row).unwrap())
    );
    let same = g.clone();
    g.interface_widget_reparent(ret, ids["moving"], ids["right"])
        .unwrap();
    assert_eq!(g, same);
    g.interface_widget_reparent(ret, ids["moving"], ids["button"])
        .unwrap();
    assert_eq!(
        evaluate(&g).find("moving").unwrap().parent_slot,
        Some(ParentSlot::for_parent(ComponentKind::Button).unwrap())
    );
    g.interface_widget_reparent(ret, ids["moving"], ids["left"])
        .unwrap();
    assert_eq!(
        evaluate(&g).find("moving").unwrap().parent_slot,
        Some(ParentSlot::for_parent(ComponentKind::Column).unwrap())
    );
    let generated = transpile(&g).unwrap();
    assert!(generated.source.contains("title"));
    assert_eq!(evaluate(&g).find("moving").unwrap().text, "Dynamic title");
    let reopened =
        reimport_script(&rvn_parser::parse(&generated.source).unwrap(), &[g.clone()]).unwrap();
    let reopened = reopened.iter().find(|r| r.graph_id == g.graph_id).unwrap();
    assert_eq!(reopened.interface_component_ids(), ids);
    assert_eq!(evaluate(reopened), evaluate(&g));
}
#[test]
fn horizontal0954_wrong_axis_fill_cycle_and_root_refuse_before_graph_allocation_or_source_change() {
    let mut g = graph(SOURCE);
    let ids = g.interface_component_ids();
    let ret = returned(&g);
    let before = g.clone();
    let source = transpile(&g).unwrap().source;
    for slot in [
        ParentSlot::for_parent(ComponentKind::Column).unwrap(),
        horizontal(f32::NAN),
        horizontal(f32::INFINITY),
        horizontal(-1.0),
    ] {
        assert!(g
            .interface_widget_set_parent_slot(ret, ids["right"], slot)
            .is_err());
        assert_eq!(g, before);
    }
    assert!(g
        .interface_widget_reparent(ret, ids["root"], ids["right"])
        .is_err());
    assert!(g
        .interface_widget_reparent(ret, ids["left"], ids["moving"])
        .is_err());
    assert_eq!(g, before);
    assert_eq!(transpile(&g).unwrap().source, source);
}
#[test]
fn horizontal0954_historical_row_generic_reparent_remains_separate_from_widget_opt_in() {
    let mut g = graph(
        r#"screen form(title){return component("root","row",{},[component("left","column",{},[component("moving","text",{"text":title},[])]),component("right","row",{},[])])}"#,
    );
    let ids = g.interface_component_ids();
    let ret = returned(&g);
    let before = g.clone();
    assert!(g
        .interface_widget_reparent(ret, ids["moving"], ids["right"])
        .is_err());
    assert_eq!(g, before);
    g.interface_reparent(ret, ids["moving"], ids["right"])
        .unwrap();
    assert_eq!(g.interface_component_ids(), ids);
    assert!(!transpile(&g).unwrap().source.contains("horizontal_box"));
    assert_eq!(evaluate(&g).find("right").unwrap().children[0].id, "moving");
}
