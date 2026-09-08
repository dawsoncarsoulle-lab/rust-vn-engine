use rvn_ui::*;
#[test]
fn bound_card_text_is_measured_after_data_resolution(){
    let mut doc=Document::defaults();
    let mut root=Element::new("row".into(),Kind::Vertical);root.rect=[0.0,0.0,400.0,200.0];root.layout_options.gap=10.0;
    let mut text=Element::new("summary".into(),Kind::Text);text.rect=[0.0,0.0,400.0,20.0];text.text="Exemple".into();text.binding=Some("save.summary".into());text.layout_options.auto_height=true;
    let mut after=Element::new("after".into(),Kind::Text);after.rect=[0.0,0.0,400.0,20.0];root.children=vec![text,after];doc.components.insert("card".into(),root);
    let source=doc.clone();let data=[("save.summary".into(),"Une traduction beaucoup plus longue".into())].into_iter().collect();
    let result=doc.layout_item_measured("card",[400.0,200.0],&data,&mut |e,width,_|{assert_eq!(e.text,"Une traduction beaucoup plus longue");assert_eq!(width,400.0);Some(100.0)});
    assert_eq!(result.iter().find(|e|e.id=="summary").unwrap().rect[3],100.0);
    assert_eq!(result.iter().find(|e|e.id=="after").unwrap().rect[1],110.0);assert_eq!(doc,source);
}

#[test]
fn free_card_expands_for_long_summary_without_changing_source(){
    let mut doc=Document::defaults();let id=doc.add_save_card();let before=doc.clone();
    let data=[("save.summary".into(),"Une très longue traduction".into())].into_iter().collect();
    let rows=doc.layout_item_measured(&id,[480.0,300.0],&data,&mut |e,_,_|Some(if e.binding.as_deref()==Some("save.summary"){160.0}else{28.0}));
    let summary=rows.iter().find(|e|e.binding.as_deref()==Some("save.summary")).unwrap();
    assert_eq!(summary.rect[3],160.0);assert!(rows[0].rect[3]>=summary.rect[1]+summary.rect[3]);
    assert_eq!(doc,before);
}

#[test]
fn growing_field_moves_lower_text_but_preserves_side_actions(){
    let mut doc=Document::defaults();let mut root=Element::new("card".into(),Kind::Panel);root.rect=[0.0,0.0,400.0,120.0];
    let mut title=Element::new("title".into(),Kind::Text);title.rect=[10.0,10.0,200.0,20.0];title.layout_options.auto_height=true;
    let mut status=Element::new("status".into(),Kind::Text);status.rect=[10.0,40.0,200.0,20.0];
    let mut action=Element::new("action".into(),Kind::Button);action.rect=[250.0,40.0,120.0,30.0];
    root.children=vec![title,status,action];doc.components.insert("card".into(),root);
    for scale in [0.5,1.0,1.5] {
        let rows=doc.layout_item_measured("card",[400.0*scale,120.0*scale],&Default::default(),&mut |_,_,s|Some(100.0*s));
        assert_eq!(rows.iter().find(|e|e.id=="status").unwrap().rect[1],120.0*scale);
        assert_eq!(rows.iter().find(|e|e.id=="action").unwrap().rect[1],40.0*scale);
    }
}
