//! Author-owned menu controls; their values live only in the UI session.
use crate::{Document,Element,Kind,Session};
use serde::{Deserialize,Serialize};
use serde_json::Value;
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
pub struct LocalControl{
    pub variable:String,
    pub initial:Value,
    #[serde(default)]pub minimum:f64,
    #[serde(default="maximum")]pub maximum:f64,
    #[serde(default="step")]pub step:f64,
    #[serde(default)]pub options:Vec<String>,
}
fn maximum()->f64{100.0}fn step()->f64{1.0}
impl Element {
    /// Call on a resolved element so inherited bindings use the same rules.
    pub fn supports_value_changed(&self) -> bool {
        matches!(self.kind, Kind::CheckBox | Kind::Slider | Kind::Select)
            && (self.local_control.is_some() || self.binding.as_deref().is_some_and(|binding|
                matches!((self.kind, binding),
                    (Kind::CheckBox, "typewriter" | "fullscreen") |
                    (Kind::Slider, "music_volume" | "sfx_volume" | "text_speed" | "auto_speed") |
                    (Kind::Select, "language"))))
    }
}
impl LocalControl{
    pub fn for_kind(kind:Kind,variable:String)->Self{Self{variable,initial:match kind{Kind::CheckBox=>false.into(),Kind::Select=>"Option 1".into(),_=>0.0.into()},minimum:0.0,maximum:100.0,step:1.0,options:if kind==Kind::Select{vec!["Option 1".into(),"Option 2".into()]}else{vec![]}}}
    pub fn validate(&self,kind:Kind)->Result<(),String>{
        if self.variable.trim().is_empty()||self.variable.starts_with("state.")||self.variable.chars().any(|c|!(c.is_alphanumeric()||matches!(c,'_'|'.'|'-'))){return Err("Variable d’interface invalide : utilisez un nom sans espace, hors du préfixe réservé state.".into());}
        if ![self.minimum,self.maximum,self.step].iter().all(|n|n.is_finite()&&n.abs()<=1.0e9)||self.minimum>=self.maximum||self.step<=0.0{return Err("Le curseur demande des valeurs finies entre −1 milliard et 1 milliard, un minimum inférieur au maximum et un pas positif".into());}
        if kind==Kind::Select&&(self.options.is_empty()||self.options.len()>200||self.options.iter().any(|s|s.trim().is_empty())||self.options.iter().collect::<std::collections::BTreeSet<_>>().len()!=self.options.len()){return Err("Le sélecteur demande de 1 à 200 options distinctes et non vides".into());}
        if !self.accepts(kind,&self.initial){return Err("Valeur initiale incompatible avec le contrôle d’interface".into());}Ok(())
    }
    pub fn accepts(&self,kind:Kind,value:&Value)->bool{match kind{Kind::CheckBox=>value.is_boolean(),Kind::Slider=>value.as_f64().is_some_and(|n|n.is_finite()&&n>=self.minimum&&n<=self.maximum),Kind::Select=>value.as_str().is_some_and(|s|self.options.iter().any(|o|o==s)),_=>false}}
    pub fn quantize(&self,value:f64)->f64{(self.minimum+((value-self.minimum)/self.step).round()*self.step).clamp(self.minimum,self.maximum)}
    pub fn value<'a>(&'a self,kind:Kind,session:&'a Session)->&'a Value{session.variables.get(&self.variable).filter(|v|self.accepts(kind,v)).unwrap_or(&self.initial)}
}
impl Session{
    pub fn initialize_controls(&mut self,doc:&Document)->Result<(),String>{for (name,(_,control)) in doc.local_controls()?{self.variables.entry(name).or_insert(control.initial);}Ok(())}
    pub fn set_control_value(&mut self,kind:Kind,control:&LocalControl,value:Value)->Result<bool,String>{
        control.validate(kind)?;if !control.accepts(kind,&value){return Err("Valeur incompatible avec le contrôle d’interface".into());}
        if control.value(kind,self)==&value{return Ok(false);}self.variables.insert(control.variable.clone(),value);Ok(true)
    }
    pub(crate) fn present_controls(&self,e:&mut Element){if let Some(control)=&mut e.local_control{control.initial=control.value(e.kind,self).clone();}for child in &mut e.children{self.present_controls(child);}}
}
impl Document{
    pub fn local_controls(&self)->Result<std::collections::BTreeMap<String,(Kind,LocalControl)>,String>{
        fn walk(doc:&Document,es:&[Element],out:&mut std::collections::BTreeMap<String,(Kind,LocalControl)>,depth:usize)->Result<(),String>{if depth>=64{return Err("Imbrication de contrôles trop profonde".into());}for source in es{let e=doc.resolved_element(source);if let Some(c)=&e.local_control{c.validate(e.kind)?;if let Some((kind,old))=out.get(&c.variable){if *kind!=e.kind||old!=c{return Err(format!("La variable {} est utilisée par des contrôles aux configurations différentes",c.variable));}}out.insert(c.variable.clone(),(e.kind,c.clone()));}walk(doc,&e.children,out,depth+1)?;}Ok(())}
        let mut out=std::collections::BTreeMap::new();for p in &self.pages{walk(self,&p.elements,&mut out,0)?;}Ok(out)
    }
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn controls_are_typed_and_session_only(){let mut s=Session::default();for kind in [Kind::Slider,Kind::CheckBox,Kind::Select]{let c=LocalControl::for_kind(kind,"ui.test".into());c.validate(kind).unwrap();assert!(!s.set_control_value(kind,&c,c.initial.clone()).unwrap());assert!(s.set_control_value(kind,&c,Value::Null).is_err());}let c=LocalControl::for_kind(Kind::CheckBox,"ui.check".into());assert!(s.set_control_value(Kind::CheckBox,&c,true.into()).unwrap());assert_eq!(c.initial,false);assert_eq!(s.variables["ui.check"],true);assert!(!s.set_control_value(Kind::CheckBox,&c,true.into()).unwrap());}
    #[test]fn invalid_configuration_and_reserved_names_are_rejected(){let mut c=LocalControl::for_kind(Kind::Slider,"state.has_save".into());assert!(c.validate(Kind::Slider).is_err());c.variable="ui.slider".into();c.step=0.0;assert!(c.validate(Kind::Slider).is_err());c.step=5.0;assert_eq!(c.quantize(12.0),10.0);assert_eq!(c.quantize(999.0),100.0);let mut c=LocalControl::for_kind(Kind::Select,"ui.select".into());c.options.push(c.options[0].clone());assert!(c.validate(Kind::Select).is_err());}
}
