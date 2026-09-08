//! Optional image overrides per interaction state, independent of saved runtime state.
use crate::*;

#[derive(Clone,Debug,Default,Serialize,Deserialize,PartialEq)]
#[serde(default)]
pub struct StateImages {
    pub hover:Option<ResourceOverride>,
    pub pressed:Option<ResourceOverride>,
    pub disabled:Option<ResourceOverride>,
    pub focus:Option<ResourceOverride>,
    pub selected:Option<ResourceOverride>,
}
impl StateImages {
    pub fn get(&self,index:usize)->Option<&ResourceOverride>{match index{1=>self.hover.as_ref(),2=>self.pressed.as_ref(),3=>self.disabled.as_ref(),4=>self.focus.as_ref(),5=>self.selected.as_ref(),_=>None}}
    pub fn set(&mut self,index:usize,value:Option<ResourceOverride>){match index{1=>self.hover=value,2=>self.pressed=value,3=>self.disabled=value,4=>self.focus=value,5=>self.selected=value,_=>{}}}
    pub fn apply(&self,target:&mut Self){for index in 1..=5{if let Some(value)=self.get(index){target.set(index,Some(value.clone()));}}}
    pub fn is_empty(&self)->bool{(1..=5).all(|i|self.get(i).is_none())}
    pub fn resolve<'a>(&'a self,normal:Option<&'a str>,enabled:bool,pressed:bool,hover:bool,focus:bool,selected:bool)->Option<&'a str>{
        let idle=if selected{self.selected.as_ref()}else{None};
        let value=if !enabled{self.disabled.as_ref()}else if pressed{self.pressed.as_ref().or(self.hover.as_ref()).or(idle)}else if hover{self.hover.as_ref().or(idle)}else if focus{self.focus.as_ref().or(self.hover.as_ref()).or(idle)}else{idle};
        match value{Some(ResourceOverride::Path(path))=>Some(path),Some(ResourceOverride::Clear)=>None,None=>normal}
    }
    pub fn paths(&self)->impl Iterator<Item=&String>{(1..=5).filter_map(|i|match self.get(i){Some(ResourceOverride::Path(path))=>Some(path),_=>None})}
    pub fn remap(&mut self,prefix:&str){for i in 1..=5{if let Some(ResourceOverride::Path(path))=self.get(i){let path=format!("{prefix}/{path}");self.set(i,Some(ResourceOverride::Path(path)));}}}
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn state_priority_and_explicit_clear_preserve_inheritance(){
        let mut state=StateImages::default();state.hover=Some(ResourceOverride::Path("hover.png".into()));state.selected=Some(ResourceOverride::Path("selected.png".into()));
        assert_eq!(state.resolve(Some("normal.png"),true,true,false,false,false),Some("hover.png"));
        assert_eq!(state.resolve(Some("normal.png"),true,false,false,false,true),Some("selected.png"));
        let mut local=StateImages::default();local.pressed=Some(ResourceOverride::Clear);local.apply(&mut state);
        assert_eq!(state.resolve(Some("normal.png"),true,true,true,true,true),None);
        assert_eq!(state.resolve(Some("normal.png"),false,true,true,true,true),Some("normal.png"));
        assert_eq!(state.resolve(Some("normal.png"),true,false,false,true,false),Some("hover.png"));
        assert_eq!(state.paths().count(),2);
    }
    #[test]fn shared_images_local_clear_and_reset_roundtrip(){let mut d=Document::defaults();let mut style=StylePatch::default();style.image_states.hover=Some(ResourceOverride::Path("shared.png".into()));d.styles.insert("image_button".into(),style);let e=&mut d.pages[0].elements[0];e.style=Some("image_button".into());e.overrides.image_states.pressed=Some(ResourceOverride::Clear);let resolved=d.resolved_element(&d.pages[0].elements[0]);assert_eq!(resolved.appearance.image_states.resolve(None,true,false,true,false,false),Some("shared.png"));assert_eq!(resolved.appearance.image_states.resolve(None,true,true,false,false,false),None);d.pages[0].elements[0].overrides.image_states.pressed=None;assert_eq!(d.resolved_element(&d.pages[0].elements[0]).appearance.image_states.resolve(None,true,true,false,false,false),Some("shared.png"));assert_eq!(Document::from_json(&d.to_json().unwrap()).unwrap(),d);}
    #[test]fn remap_changes_every_image_reference_but_preserves_clear(){let mut states=StateImages::default();for index in 1..=5{states.set(index,Some(ResourceOverride::Path(format!("state{index}.png"))));}states.disabled=Some(ResourceOverride::Clear);states.remap("theme_2");assert_eq!(states.paths().count(),4);assert!(states.paths().all(|path|path.starts_with("theme_2/")));assert_eq!(states.disabled,Some(ResourceOverride::Clear));}
}
