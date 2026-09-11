//! Bounded, deterministic UI effects. They never alter the saved definition.
use serde::{Serialize,Deserialize};
#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord,Serialize,Deserialize)]
pub enum AnimationKind{Fade,Move,Scale,Color}
impl AnimationKind{pub fn label(self)->&'static str{match self{Self::Fade=>"Fondu",Self::Move=>"Déplacement",Self::Scale=>"Échelle",Self::Color=>"Couleur"}}}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize,Default)]
pub enum Curve{#[default] Linear,EaseIn,EaseOut,EaseInOut}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub struct AnimationClip{pub kind:AnimationKind,pub duration:f32,pub curve:Curve,pub from:[f32;4],pub to:[f32;4]}
impl AnimationClip{
    pub fn validate(&self)->Result<(),String>{
        if !self.duration.is_finite()||self.duration<=0.0||self.duration>60.0{return Err(diagnostic!("La durée d’un effet doit être comprise entre 0 et 60 secondes (exclusivement positive)", "Effect duration must be greater than 0 and at most 60 seconds").into());}
        if self.from.iter().chain(self.to.iter()).any(|v|!v.is_finite()){return Err(diagnostic!("Valeur d’effet non finie", "Non-finite effect value").into());}
        let valid=match self.kind{AnimationKind::Fade=>(0.0..=1.0).contains(&self.from[0])&&(0.0..=1.0).contains(&self.to[0]),AnimationKind::Scale=>(0.01..=20.0).contains(&self.from[0])&&(0.01..=20.0).contains(&self.to[0]),AnimationKind::Color=>self.from.iter().chain(self.to.iter()).all(|v|(0.0..=1.0).contains(v)),AnimationKind::Move=>self.from[..2].iter().chain(self.to[..2].iter()).all(|v|v.abs()<=10000.0)};
        if !valid{return Err(diagnostic!("Valeurs hors limites pour cet effet", "Values out of range for this effect").into());}Ok(())
    }
    pub fn sample(&self,elapsed:f32)->[f32;4]{let t=(elapsed/self.duration).clamp(0.0,1.0);let t=match self.curve{Curve::Linear=>t,Curve::EaseIn=>t*t,Curve::EaseOut=>1.0-(1.0-t)*(1.0-t),Curve::EaseInOut=>t*t*(3.0-2.0*t)};std::array::from_fn(|i|self.from[i]+(self.to[i]-self.from[i])*t)}
}
#[cfg(test)]mod tests{use super::*;#[test]fn bounded_endpoints_and_curves(){for curve in [Curve::Linear,Curve::EaseIn,Curve::EaseOut,Curve::EaseInOut]{let c=AnimationClip{kind:AnimationKind::Fade,duration:2.0,curve,from:[0.0;4],to:[1.0;4]};c.validate().unwrap();assert_eq!(c.sample(-1.0),c.from);assert_eq!(c.sample(3.0),c.to);assert!((0.0..=1.0).contains(&c.sample(0.5)[0]));}let c=AnimationClip{kind:AnimationKind::Fade,duration:0.0,curve:Curve::Linear,from:[0.0;4],to:[1.0;4]};assert!(c.validate().is_err());}}
