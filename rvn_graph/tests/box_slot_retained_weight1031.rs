//! Genuine existing SourceWorkspace writer and cold Graph identity boundary.
use rvn_graph::*;
use rvn_ui::programmable::{Alignment,BoxSlotSize,ComponentKind,ParentSlot,SlotEdges};
fn slot(horizontal:bool,size:BoxSlotSize)->ParentSlot{
    if horizontal{ParentSlot::HorizontalBox{padding:SlotEdges::uniform(8.0),horizontal:Alignment::Stretch,vertical:Alignment::Center,size}}
    else{ParentSlot::VerticalBox{padding:SlotEdges::uniform(8.0),horizontal:Alignment::Stretch,vertical:Alignment::Center,size}}
}
fn source(horizontal:bool,dynamic:bool)->String{
    let panel=if horizontal{"row"}else{"column"};let kind=if horizontal{"horizontal_box"}else{"vertical_box"};let weight=if dynamic{"held"}else{"5"};
    r#"// Retain actual dormant slot state in this source.
screen form(title,held){return component("root","$PANEL",{"layout_contract":"widget_v1"},[component("a","text",{"text":title,"parent_slot":{"kind":"$KIND","padding":{"left":8,"top":8,"right":8,"bottom":8},"horizontal":"stretch","vertical":"center","size":{"rule":"auto","weight":$WEIGHT}}},[])])}
"#.replace("$PANEL",panel).replace("$KIND",kind).replace("$WEIGHT",weight)
}
fn graph(source:&str)->GraphDocument{import_script(&rvn_parser::parse(source).unwrap()).unwrap().into_iter().find(|g|matches!(g.kind,GraphKind::Screen{..})).unwrap()}
fn returned(graph:&GraphDocument)->NodeId{graph.nodes.values().find(|n|n.kind==NodeKind::FunctionReturn).unwrap().id}
#[test]
fn retained_weight1031_real_workspace_save_cold_reopen_both_axes_restores_auto_five_then_fill_five(){
    for horizontal in [false,true]{
        let root=std::env::temp_dir().join(format!("rvn-retained-weight1031-{}-{}-{horizontal}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();let path=root.join("main.rvn");let input=source(horizontal,false);std::fs::write(&path,&input).unwrap();
        let mut workspace=SourceWorkspace::link(&root,&path,&[]).unwrap();let mut graphs=workspace.project().graphs().to_vec();
        let g=graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Screen{..})).unwrap();let ids=g.interface_component_ids();let gid=g.graph_id;let ret=returned(g);
        assert_eq!(g.interface_widget_parent_slot(ret,ids["a"]).unwrap(),slot(horizontal,BoxSlotSize::Auto{weight:5.0}));
        // Real edit with a dormant weight different from the initial source.
        g.interface_widget_set_parent_slot(ret,ids["a"],slot(horizontal,BoxSlotSize::Auto{weight:7.0})).unwrap();workspace.save(&graphs).unwrap();
        let mut cold=SourceWorkspace::open(&root).unwrap().unwrap();assert_eq!(cold.project().graphs(),workspace.project().graphs());
        let mut after=cold.project().graphs().to_vec();let g=after.iter_mut().find(|g|g.graph_id==gid).unwrap();assert_eq!(g.interface_component_ids(),ids);
        let auto=g.interface_widget_parent_slot(ret,ids["a"]).unwrap();assert_eq!(auto,slot(horizontal,BoxSlotSize::Auto{weight:7.0}));
        g.interface_widget_set_parent_slot(ret,ids["a"],slot(horizontal,BoxSlotSize::Auto{weight:7.0}.into_fill())).unwrap();cold.save(&after).unwrap();
        let final_cold=SourceWorkspace::open(&root).unwrap().unwrap();let g=final_cold.project().graphs().iter().find(|g|g.graph_id==gid).unwrap();
        assert_eq!(g.interface_widget_parent_slot(ret,ids["a"]).unwrap(),slot(horizontal,BoxSlotSize::Fill{weight:7.0}));
        assert!(std::fs::read_to_string(&path).unwrap().contains("// Retain actual dormant slot state"));
    }
}
#[test]
fn retained_weight1031_dynamic_dormant_expression_stays_connected_and_constant_edit_refuses(){
    let input=source(false,true);let mut g=graph(&input);let ids=g.interface_component_ids();let ret=returned(&g);let before=g.clone();
    assert!(g.interface_widget_parent_slot(ret,ids["a"]).is_err());
    assert!(g.interface_widget_set_parent_slot(ret,ids["a"],slot(false,BoxSlotSize::Fill{weight:5.0})).is_err());
    assert_eq!(g,before);assert!(transpile(&g).unwrap().source.contains("held"));
}
#[test]
fn retained_weight1031_invalid_dormant_auto_refuses_before_original_graph_mutation(){
    let mut g=graph(&source(true,false));let ids=g.interface_component_ids();let ret=returned(&g);let before=g.clone();
    for weight in [f32::NAN,f32::INFINITY,-1.0,16385.0]{
        assert!(g.interface_widget_set_parent_slot(ret,ids["a"],slot(true,BoxSlotSize::Auto{weight})).is_err());assert_eq!(g,before);
    }
    assert!(matches!(ParentSlot::for_parent(ComponentKind::Row).unwrap(),ParentSlot::HorizontalBox{size:BoxSlotSize::Auto{weight:1.0},..}));
}
