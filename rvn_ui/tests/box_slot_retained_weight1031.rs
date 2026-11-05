//! Proposed typed schema/layout contracts; not native or pixel acceptance.
use rvn_ui::programmable::{BoxSlotSize,Component,ParentSlot,layout_widget_rects};
use serde_json::json;
fn tree(rule:&str,weight:f32)->Component{
    Component::parse(json!({"id":"root","kind":"row","layout_contract":"widget_v1","spacing":0,"children":[
        {"id":"a","kind":"text","text":"A","parent_slot":{"kind":"horizontal_box","size":{"rule":rule,"weight":weight}}},
        {"id":"b","kind":"text","text":"B","parent_slot":{"kind":"horizontal_box","size":{"rule":"fill","weight":1}}}
    ]})).unwrap()
}
#[test]
fn retained_weight1031_old_auto_and_absent_weight_decode_to_one_without_changing_old_auto_wire(){
    let auto:BoxSlotSize=serde_json::from_value(json!({"rule":"auto"})).unwrap();
    assert_eq!(auto,BoxSlotSize::default());assert_eq!(auto.weight(),1.0);
    assert_eq!(serde_json::to_value(auto).unwrap(),json!({"rule":"auto"}));
    assert_eq!(serde_json::from_value::<BoxSlotSize>(json!({"rule":"fill"})).unwrap(),BoxSlotSize::Fill{weight:1.0});
    let parent:ParentSlot=serde_json::from_value(json!({"kind":"vertical_box"})).unwrap();
    assert!(matches!(parent,ParentSlot::VerticalBox{size:BoxSlotSize::Auto{weight:1.0},..}));
}
#[test]
fn retained_weight1031_fill_auto_fill_roundtrip_retains_zero_five_and_upper_bound(){
    for weight in [0.0,1.0,5.0,16384.0]{
        let fill=BoxSlotSize::Fill{weight};let auto=fill.into_auto();assert_eq!(auto.weight(),weight);
        let wire=serde_json::to_value(auto).unwrap();let restored:BoxSlotSize=serde_json::from_value(wire).unwrap();
        assert_eq!(restored.into_fill(),fill);assert_eq!(restored.into_auto(),auto);
    }
}
#[test]
fn retained_weight1031_dormant_weight_survives_component_serde_and_does_not_affect_auto_layout(){
    let a=tree("auto",1.0);let b=tree("auto",5.0);
    let measure=|_:&rvn_ui::programmable::Component|Ok([20.0,12.0]);
    assert_eq!(layout_widget_rects(&a,[0.0,0.0,120.0,60.0],&mut {measure}).unwrap(),layout_widget_rects(&b,[0.0,0.0,120.0,60.0],&mut {measure}).unwrap());
    let wire=serde_json::to_value(&b).unwrap();assert_eq!(wire["children"][0]["parent_slot"]["size"],json!({"rule":"auto","weight":5.0}));
    assert_eq!(Component::parse(wire).unwrap(),b);
    let filled=tree("fill",5.0);let rects=layout_widget_rects(&filled,[0.0,0.0,120.0,60.0],&mut {measure}).unwrap();
    assert_eq!(rects.iter().find(|r|r.id=="a").unwrap().rect[2],100.0);
    assert_eq!(rects.iter().find(|r|r.id=="b").unwrap().rect[2],20.0);
}
#[test]
fn retained_weight1031_dormant_nonfinite_negative_oversized_and_unknown_fields_remain_invalid(){
    for weight in [f32::NAN,f32::INFINITY,-1.0,16385.0]{
        let mut value=tree("auto",1.0);let Some(ParentSlot::HorizontalBox{size,..})=&mut value.children[0].parent_slot else{unreachable!()};
        *size=BoxSlotSize::Auto{weight};assert!(value.validate().is_err());
    }
    assert!(serde_json::from_value::<BoxSlotSize>(json!({"rule":"auto","weight":5,"foreign":9})).is_err());
    assert!(serde_json::from_value::<BoxSlotSize>(json!({"rule":"bogus","weight":5})).is_err());
}
