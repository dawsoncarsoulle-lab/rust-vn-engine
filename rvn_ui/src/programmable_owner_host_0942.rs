//! UserWidget owner data is independent of its optional authored child tree.
//! Technical source reference: local UE5.8.3 UUserWidget/SObjectWidget/viewport
//! slot contracts. Independent Rust model; no Unreal code or resource reuse.
use super::{Component, ScreenView, SlotEdges};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum HostVisibility {
    Visible, Hidden, Collapsed, HitTestInvisible,
    #[default] SelfHitTestInvisible,
}
impl HostVisibility {
    pub fn visible(self)->bool { !matches!(self,Self::Hidden|Self::Collapsed) }
    pub fn owner_hit_test(self)->bool { self==Self::Visible }
    pub fn child_hit_test(self)->bool {matches!(self,Self::Visible|Self::SelfHitTestInvisible)}
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct ViewportSlot {
    /// [min x,min y,max x,max y] relative to the actual logical viewport.
    pub anchors:[f32;4],
    pub offsets:SlotEdges,
    pub alignment:[f32;2],
    pub z_order:i32,
}
impl Default for ViewportSlot {
    fn default()->Self{Self{anchors:[0.0,0.0,1.0,1.0],offsets:SlotEdges::default(),alignment:[0.0,0.0],z_order:0}}
}
#[derive(Debug,Clone,Copy,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostGeometry {pub rect:[f32;4],pub viewport:[f32;4],pub scale:f32}
impl HostGeometry{
    pub fn bounding_rect(&self)->[f32;4]{
        let [x,y,w,h]=self.rect;[x.min(x+w),y.min(y+h),w.abs(),h.abs()]
    }
    /// Axis-aligned form of the local parallelogram test. Signed nonzero
    /// extents retain their orientation; zero area is not a point hit test.
    pub fn contains_point(&self,point:[f32;2])->bool{
        let [x,y,w,h]=self.rect;
        if !point.iter().all(|v|v.is_finite())||w==0.0||h==0.0{return false;}
        let uv=[(point[0]-x)/w,(point[1]-y)/h];uv.iter().all(|v|v.is_finite()&&(0.0..=1.0).contains(v))
    }
    /// Distance to the normalized closed bounds is a separate hit-grid
    /// primitive. It intentionally differs from Contains for zero area.
    pub fn distance_squared(&self,point:[f32;2])->Option<f32>{
        if !point.iter().all(|v|v.is_finite()){return None;}
        let [x,y,w,h]=self.bounding_rect();let dx=(x-point[0]).max(0.0).max(point[0]-(x+w));let dy=(y-point[1]).max(0.0).max(point[1]-(y+h));Some(dx*dx+dy*dy)
    }
}
impl ViewportSlot {
    pub fn auto_size(&self)->bool{self.offsets.right==0.0&&self.offsets.bottom==0.0&&self.anchors[0]==self.anchors[2]&&self.anchors[1]==self.anchors[3]}
    pub fn validate(&self)->Result<(),String>{
        if self.anchors.iter().any(|v|!v.is_finite()||v.abs()>16.0)
            || self.alignment.iter().any(|v|!v.is_finite()||v.abs()>16.0)
            || self.anchors[0]>self.anchors[2] || self.anchors[1]>self.anchors[3]
            || !(-1000..=1000).contains(&self.z_order) {
            return Err("Invalid bounded UserWidget viewport anchors/alignment/order".into());
        }
        for v in [self.offsets.left,self.offsets.top,self.offsets.right,self.offsets.bottom]{
            if !v.is_finite()||v.abs()>16384.0{return Err("Invalid bounded UserWidget viewport offsets".into());}
        }
        Ok(())
    }
    /// Resolve the actual host slot independently of child presence. A desired
    /// size is used only for an un-stretched zero-size slot, never as a clamp
    /// on an explicit or stretched allocation. No guessed reference viewport.
    pub fn geometry(&self,viewport:[f32;4],desired:[f32;2],scale:f32)->Result<HostGeometry,String>{
        self.validate()?;
        if !scale.is_finite()||scale<=0.0||scale>64.0
            || viewport.iter().any(|v|!v.is_finite()||v.abs()>1.0e7)
            || viewport[2]<0.0||viewport[3]<0.0
            || desired.iter().any(|v|!v.is_finite()||*v<0.0||*v>16384.0){
            return Err("Invalid actual UserWidget viewport or desired size".into());
        }
        let stretch=[self.anchors[0]!=self.anchors[2],self.anchors[1]!=self.anchors[3]];
        let automatic=self.auto_size();
        let start=[self.offsets.left,self.offsets.top];let end=[self.offsets.right,self.offsets.bottom];
        let mut rect=[0.0;4];
        for axis in 0..2 {
            let anchor=viewport[axis+2]*self.anchors[axis];
            let extent=if stretch[axis]{viewport[axis+2]*(self.anchors[axis+2]-self.anchors[axis])-start[axis]-end[axis]}
                else if automatic{desired[axis]}else{end[axis]};
            let origin=viewport[axis]+anchor+start[axis]-if stretch[axis]{0.0}else{self.alignment[axis]*extent};
            // Preserve signed logical allocation. A renderer must not turn a
            // negative Slate-compatible extent into an invented positive hit
            // rectangle; native layout/input adaptation is separate.
            if !origin.is_finite()||!extent.is_finite()||origin.abs()>1.0e7||extent.abs()>1.0e7{return Err("Invalid overflowed UserWidget host allocation".into());}
            rect[axis]=origin;rect[axis+2]=extent;
        }
        Ok(HostGeometry{rect,viewport,scale})
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum HostEventKind {Initialized,PreConstruct,Construct,Destruct,FocusReceived,FocusLost,KeyDown,KeyUp,Tick}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="target",content="node",rename_all="snake_case",deny_unknown_fields)]
pub enum HostFocusTarget {Owner,Element(String)}
impl HostFocusTarget {
    pub fn validate(&self)->Result<(),String>{
        if let Self::Element(id)=self {if id.is_empty()||id.len()>128{return Err("Invalid UserWidget desired child focus identity".into());}}
        Ok(())
    }
}
/// A desired, ordered focus request; not the focus that the backend acquired.
/// It carries no producer token, wrapper generation or input authority.
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostFocusRequest {pub target:HostFocusTarget,pub user:u32,pub serial:u64}
impl HostFocusRequest {
    pub fn validate(&self)->Result<(),String>{self.target.validate()?;if self.user>4095||self.serial==0{return Err("Invalid bounded UserWidget focus request".into());}Ok(())}
}
/// Explicit target syntax accepted by the existing ui.focus expression. The
/// historical string operand still denotes its authored child element.
pub fn parse_host_focus_target(value:serde_json::Value)->Result<(HostFocusTarget,u32),String>{
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request{target:String,#[serde(default)]node:Option<String>,#[serde(default)]user:u32}
    let request:Request=serde_json::from_value(value).map_err(|e|e.to_string())?;
    let target=match(request.target.as_str(),request.node){("owner",None)=>HostFocusTarget::Owner,("element",Some(node))=>HostFocusTarget::Element(node),_=>return Err("Explicit focus target needs owner or an actual element node".into())};
    target.validate()?;if request.user>4095{return Err("Focus user exceeds the supported domain bound".into());}Ok((target,request.user))
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct ScreenHostProperties {
    pub focusable:bool,
    pub enabled:bool,
    pub visibility:HostVisibility,
    pub minimum_desired_size:[f32;2],
    pub viewport_slot:ViewportSlot,
    pub desired_focus:Option<HostFocusTarget>,
    pub events:BTreeMap<HostEventKind,String>,
}
impl Default for ScreenHostProperties {
    fn default()->Self{Self{focusable:false,enabled:true,visibility:HostVisibility::default(),minimum_desired_size:[0.0,0.0],viewport_slot:ViewportSlot::default(),desired_focus:None,events:BTreeMap::new()}}
}
impl ScreenHostProperties {
    /// Preserve the old save representation for exact legacy defaults. Float
    /// bits participate so a deliberately authored -0.0 is not silently lost.
    pub fn is_default(&self)->bool{
        fn floats(p:&ScreenHostProperties)->[u32;12]{[
            p.minimum_desired_size[0],p.minimum_desired_size[1],p.viewport_slot.anchors[0],p.viewport_slot.anchors[1],p.viewport_slot.anchors[2],p.viewport_slot.anchors[3],
            p.viewport_slot.offsets.left,p.viewport_slot.offsets.top,p.viewport_slot.offsets.right,p.viewport_slot.offsets.bottom,p.viewport_slot.alignment[0],p.viewport_slot.alignment[1],
        ].map(f32::to_bits)}
        let default=Self::default();self==&default&&floats(self)==floats(&default)
    }
    pub fn validate(&self)->Result<(),String>{
        self.viewport_slot.validate()?;
        if self.minimum_desired_size.iter().any(|v|!v.is_finite()||v.abs()>16384.0)
            || self.events.len()>16 || self.events.values().any(|name|name.is_empty()||name.len()>128){
            return Err("Invalid bounded UserWidget owner size or handler".into());
        }
        if let Some(target)=&self.desired_focus{target.validate()?;}
        Ok(())
    }
    pub fn desired_size(&self,child_desired:[f32;2])->Result<[f32;2],String>{
        self.validate()?;
        if child_desired.iter().any(|v|!v.is_finite()||*v<0.0||*v>16384.0){return Err("Invalid actual child desired size".into());}
        Ok([child_desired[0].max(self.minimum_desired_size[0]),child_desired[1].max(self.minimum_desired_size[1])])
    }
    /// Hit-test policy and keyboard/focus availability are distinct. Passive
    /// cancellation drains an original producer even after a visibility change.
    pub fn allows_child_event(&self,kind:super::ScreenEventKind)->bool{
        use super::ScreenEventKind as E;
        if kind==E::PointerCancel{return true;}
        if !self.enabled||!self.visibility.visible(){return false;}
        !matches!(kind,E::Click|E::PointerDown|E::PointerMove|E::PointerUp|E::Wheel)||self.visibility.child_hit_test()
    }
    pub fn supports_owner_focus(&self)->bool{self.focusable}
    /// Acquisition additionally needs an admitted live source, user and hosted
    /// path; this local predicate alone never grants focus.
    pub fn focus_properties_allow_acquisition(&self)->bool{self.focusable&&self.enabled&&self.visibility.visible()}
}
#[derive(Debug,Clone,PartialEq)]
pub struct ScreenDescription {pub host:ScreenHostProperties,pub root:Option<Component>}
/// The exact VM description: historical Component/[] is a default owner;
/// an explicit {host:properties,root:Component|[]} supplies owner class data.
/// A nullable portable cache is NOT an alternative authored source syntax.
pub fn parse_screen_description(value:serde_json::Value)->Result<ScreenDescription,String>{
    fn root(value:serde_json::Value)->Result<Option<Component>,String>{
        match value{serde_json::Value::Array(items)if items.is_empty()=>Ok(None),value=>Component::parse(value).map(Some)}
    }
    let description=if value.as_object().is_some_and(|object|object.contains_key("host")||object.contains_key("root")) {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Envelope{host:ScreenHostProperties,root:serde_json::Value}
        let envelope:Envelope=serde_json::from_value(value).map_err(|e|e.to_string())?;
        ScreenDescription{host:envelope.host,root:root(envelope.root)?}
    }else{ScreenDescription{host:ScreenHostProperties::default(),root:root(value)?}};
    description.host.validate()?;Ok(description)
}
/// Pure description of an actual resident owner, including one without any
/// child tree. Engine/renderer source and viewport-user authority are separate
/// typed envelopes; names here do not grant input or lifecycle permission.
#[derive(Debug,Clone,PartialEq)]
pub struct ScreenHostView {pub name:String,pub modal:bool,pub layer:i32,pub order:u64,pub properties:ScreenHostProperties,pub child:Option<ScreenView>}
impl ScreenHostView {
    pub fn covers(&self,other:&Self)->bool{self.modal&&(other.layer,other.order)<(self.layer,self.order)}
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum HostLifecycleEvent{Initialized,PreConstruct{design_time:bool},Construct,Destruct}
/// Actual wrapper notifications, not inferred child-tree transitions. The VM
/// transaction executes returned events before installing an accepted candidate.
/// Viewport detachment alone does not prove that the last wrapper was released.
#[derive(Debug,Clone,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct HostLifecycle{initialized:bool,wrapper_generation:u64,wrapper_alive:bool,wrapper_runtime:bool,viewport_attached:bool}
impl HostLifecycle {
    pub fn generation(&self)->u64{self.wrapper_generation}
    pub fn attached(&self)->bool{self.viewport_attached}
    pub fn initialize(&mut self,allowed_context:bool)->Vec<HostLifecycleEvent>{
        if self.initialized{return vec![];}self.initialized=true;
        if allowed_context{vec![HostLifecycleEvent::Initialized]}else{vec![]}
    }
    pub fn rebuild_wrapper(&mut self,design_time:bool,design_preconstruct_allowed:bool,safe_event_route:bool)->Result<Vec<HostLifecycleEvent>,String>{
        let generation=self.wrapper_generation.checked_add(1).ok_or("UserWidget wrapper generation exhausted")?;
        let mut events=Vec::new();if self.wrapper_alive&&self.wrapper_runtime&&safe_event_route{events.push(HostLifecycleEvent::Destruct);}
        if safe_event_route&&(!design_time||design_preconstruct_allowed){events.push(HostLifecycleEvent::PreConstruct{design_time});}
        if !design_time&&safe_event_route{events.push(HostLifecycleEvent::Construct);}
        self.wrapper_generation=generation;self.wrapper_alive=true;self.wrapper_runtime=!design_time;Ok(events)
    }
    pub fn attach_viewport(&mut self)->Result<(),String>{
        if !self.wrapper_alive||self.viewport_attached{return Err("UserWidget viewport requires one live, unattached owner wrapper".into());}
        self.viewport_attached=true;Ok(())
    }
    pub fn detach_viewport(&mut self){self.viewport_attached=false;}
    pub fn release_wrapper(&mut self,safe_event_route:bool)->Vec<HostLifecycleEvent>{
        let events=if self.wrapper_alive&&self.wrapper_runtime&&safe_event_route{vec![HostLifecycleEvent::Destruct]}else{vec![]};
        self.wrapper_alive=false;self.viewport_attached=false;events
    }
}

#[cfg(test)]
#[path="programmable_owner_host_tests_0942.rs"]
mod tests;
